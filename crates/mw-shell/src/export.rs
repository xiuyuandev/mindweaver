//! 导出：格式清单、文本写出、PNG 尺寸预算与 alpha 还原。
//!
//! 这里只放**与 GPU 无关**的那一半——枚举、文件名、纯文本序列化、像素后处理，
//! 都能脱离窗口单测。真正的离屏渲染在 `canvas::Canvas::export_png`（要合成器），
//! 原生保存对话框在 `files::save_export`。
//!
//! 文本四种格式的序列化本身在 `mw-io`（`export_markdown` / `export_outline` /
//! `export_opml` / `export_freemind`，各有往返测试），这里只做「挑了哪个格式 → 调哪个函数
//! → 写哪个后缀」的接线。
//!
//! **导出的是整棵主题树，不是「当前可见部分」**：折叠起来的子树同样导出（用户要的是内容，
//! 不是屏幕上那一屏）。视口缩放、选中高亮、悬停锚点一律不进导出图（docs/13 §4.23）。

use std::io::Write;
use std::path::{Path, PathBuf};

use mw_core::Document;

/// 导出格式。顺序即对话框里的排布顺序：图像/矢量/文档在前，文本在后。
///
/// SVG 走**独立的几何→标记管线**（`export_svg`），不是把 `Frame` 丢给 GPU；
/// PDF 先以「嵌一张离屏位图」落地（`export_pdf`）。枚举里每一个变体都必须真的能落盘，
/// 不许出现「选项在、点了没反应」。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Svg,
    Pdf,
    Markdown,
    Outline,
    Opml,
    Freemind,
}

impl Format {
    /// 全部已实现落地的格式（顺序即界面顺序）。
    pub const ALL: [Format; 7] = [
        Format::Png,
        Format::Svg,
        Format::Pdf,
        Format::Markdown,
        Format::Outline,
        Format::Opml,
        Format::Freemind,
    ];

    /// 文件后缀（小写，不带点）。
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Svg => "svg",
            Format::Pdf => "pdf",
            Format::Markdown => "md",
            Format::Outline => "txt",
            Format::Opml => "opml",
            Format::Freemind => "mm",
        }
    }

    /// 对话框与菜单里的名字（也是 `IFileSaveDialog` 的过滤器名）。
    pub fn label(self) -> &'static str {
        match self {
            Format::Png => "PNG 图片",
            Format::Svg => "SVG 矢量图",
            Format::Pdf => "PDF 文档",
            Format::Markdown => "Markdown",
            Format::Outline => "缩进大纲文本",
            Format::Opml => "OPML",
            Format::Freemind => "FreeMind",
        }
    }

    /// 是不是位图管线（决定对话框要不要露出「比例」选项）：PNG 与 PDF 共用
    /// `Canvas::render_export` 那张离屏位图，比例对两者都只管清晰度。
    pub fn is_image(self) -> bool {
        matches!(self, Format::Png | Format::Pdf)
    }

    /// 有没有背景底色可选。SVG 没有比例，但透明背景同样有意义。
    pub fn has_background(self) -> bool {
        matches!(self, Format::Png | Format::Svg | Format::Pdf)
    }

    /// 文本格式的序列化结果；图像/矢量/PDF 返回 `None`（走各自的管线）。
    pub fn text(self, doc: &Document) -> Option<String> {
        match self {
            Format::Markdown => Some(mw_io::export_markdown(doc)),
            Format::Outline => Some(mw_io::export_outline(doc)),
            Format::Opml => Some(mw_io::export_opml(doc)),
            Format::Freemind => Some(mw_io::export_freemind(doc)),
            Format::Png | Format::Svg | Format::Pdf => None,
        }
    }
}

/// 导出范围。只对位图/矢量/PDF 有意义：文本格式永远导出整份文档。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// 整张导图（主根 + 自由主题）。
    Whole,
    /// 只导出主选节点的子树（XMind 的「导出选中」）。
    Selection,
    /// 只导出当前视口（相机看到的范围）。
    Viewport,
}

impl Scope {
    pub const ALL: [Scope; 3] = [Scope::Whole, Scope::Selection, Scope::Viewport];

    pub fn label(self) -> &'static str {
        match self {
            Self::Whole => "整张导图",
            Self::Selection => "选中子树",
            Self::Viewport => "可视区域",
        }
    }

    /// 存盘用的稳定键（`label` 是给人看的中文，不能拿来当键）。
    pub fn key(self) -> &'static str {
        match self {
            Self::Whole => "whole",
            Self::Selection => "selection",
            Self::Viewport => "viewport",
        }
    }
}

/// 把文档标题擦干净当默认文件名：Windows 不允许 `\ / : * ? " < > |`，
/// 另外控制字符与首尾空格、结尾的点也要去掉（`SetFileName` 收到这些会拒收）。
///
/// 还要**剥掉文档/导入格式的后缀**：`Session::title()` 返回带后缀的文件名，
/// 原样拿去当默认名会连文档后缀一起带出去——选 PNG 就存出一个叫「未命名.zmind」的 PNG，
/// 导入的 `大纲.md` 会变成 `大纲.md.png`。
pub fn file_stem(title: &str) -> String {
    // 后缀都是纯 ASCII，按字节长度截断必落在字符边界上。
    const SOURCE_EXTS: [&str; 7] = [
        ".markdown",
        ".zmind",
        ".xmind",
        ".opml",
        ".mm",
        ".md",
        ".txt",
    ];
    let lower = title.to_ascii_lowercase();
    let base = SOURCE_EXTS
        .iter()
        .find_map(|ext| {
            lower
                .ends_with(ext)
                .then(|| &title[..title.len() - ext.len()])
        })
        .unwrap_or(title);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() {
        "未命名".to_string()
    } else {
        trimmed.chars().take(48).collect()
    }
}

/// 文本落盘。统一 UTF-8，**多字节字符原样写出**（旧版曾用系统 ANSI，中文在别的编辑器里会花）。
///
/// 走「同目录 `.part` → rename」：rename 是原子的，读侧要么看到旧文件、要么看到完整的新文件。
/// 直接 `std::fs::write` 会在写一半时留下半截 SVG / 半截 Markdown，而它看起来还像「导出成功了」。
/// 失败一律清掉 `.part`（半成品导出比没有文件更糟）。
pub fn write_text(path: &Path, text: &str) -> Result<(), String> {
    let result = write_text_atomic(path, text.as_bytes());
    if result.is_err() {
        // 半截 `.part` 不能留：它长得就像一份真导出。
        let _ = std::fs::remove_file(part_path(path));
    }
    result.map_err(|e| format!("写入失败：{e}"))
}

/// 同目录下的临时名：`.{文件名}.part`。
fn part_path(path: &Path) -> PathBuf {
    let dir = parent_dir(path);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("export");
    dir.join(format!(".{name}.part"))
}

/// `path` 的父目录；只有裸文件名（没有父目录）时按当前目录算。
fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn write_text_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let part = part_path(path);
    let mut file = std::fs::File::create(&part)?;
    file.write_all(bytes)?;
    // 内容先落盘再改名：掉电时不会留下一个「名字对了、内容空」的导出文件。
    file.sync_all()?;
    drop(file);
    std::fs::rename(&part, path)
}

/// 给个后缀兜底：对话框返回的路径没后缀就补上，别存出一个没后缀的文件。
pub fn with_ext(mut p: PathBuf, ext: &str) -> PathBuf {
    if p.extension().is_none() {
        p = p.with_extension(ext);
    }
    p
}

/// 合成器回读的是**预乘** alpha（管线、片元都是这一套口径）。PNG 存的是直通 alpha，
/// 透明背景时必须先除回去，否则半透明的抗锯齿边缘会整圈发暗。
///
/// 不透明导出（底色 alpha=255）下每个像素 alpha 都是 255，这一步是恒等变换——但仍然走同一条
/// 路径，免得两套分支各写一遍。
pub fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        if a == 0 {
            // 全透明像素：预乘后 RGB 必为 0，直通值取 0（取原值在部分编辑器里会渗出黑边）。
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        } else if a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// 单边像素上限。D3D12 的纹理上限是 16384，但 8192 已经够大（8K 宽的地图），
/// 再大回读缓冲就要几百 MB 了。
pub const PNG_MAX_SIDE: u32 = 8192;

/// 单块最大边长：32MP 图拆成多块渲染，避开单张纹理上限。
pub const TILE_MAX: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// 把 `w×h` 切成不超过 `max` 的块，行优先、无重叠、无缝隙。
pub fn tile_plan(w: u32, h: u32, max: u32) -> Vec<Tile> {
    let max = max.max(1);
    let mut out = Vec::new();
    let mut y = 0;
    while y < h.max(1) {
        let mut x = 0;
        let th = max.min(h.max(1) - y);
        while x < w.max(1) {
            let tw = max.min(w.max(1) - x);
            out.push(Tile { x, y, w: tw, h: th });
            x += tw;
        }
        y += th;
    }
    out
}

/// 把一块 RGBA（紧密排列）拷进整图。调用方保证块不越界。
pub fn blit_tile(dst: &mut [u8], dst_w: u32, tile: Tile, src: &[u8], src_w: u32) {
    let dst_stride = dst_w as usize * 4;
    let src_stride = src_w as usize * 4;
    for row in 0..tile.h as usize {
        let d = (tile.y as usize + row) * dst_stride + tile.x as usize * 4;
        let s = row * src_stride;
        let n = tile.w as usize * 4;
        if d + n <= dst.len() && s + n <= src.len() {
            dst[d..d + n].copy_from_slice(&src[s..s + n]);
        }
    }
}

/// 总像素上限（≈128MB 回读缓冲）。长条形地图光卡单边是拦不住的。
pub const PNG_MAX_PIXELS: f32 = 32_000_000.0;

/// 由世界包围盒尺寸与用户选的比例，算出**实际**像素尺寸与生效比例。
///
/// 超过上限时按比例缩回去并如实回报——对话框拿它给一句「已按 N 倍导出」的提示，
/// 而不是默默出一张被裁掉的图。
pub fn png_size(w: f32, h: f32, scale: f32) -> (u32, u32, f32) {
    let w = w.max(1.0);
    let h = h.max(1.0);
    let mut eff = scale.clamp(0.05, 8.0);

    let (pw, ph) = ((w * eff), (h * eff));
    let side_k = (PNG_MAX_SIDE as f32 / pw.max(ph)).min(1.0);
    let area_k = (PNG_MAX_PIXELS / (pw * ph)).sqrt().min(1.0);
    eff *= side_k.min(area_k);

    let pw = (w * eff).round().clamp(1.0, PNG_MAX_SIDE as f32);
    let ph = (h * eff).round().clamp(1.0, PNG_MAX_SIDE as f32);
    (pw as u32, ph as u32, eff)
}

/// ⑥ 导出补齐（docs/32 §6）：把「聚焦问题」挂到导出画面上。
///
/// 画布那条带子贴视口顶、滚动不走；导出没有视口，只能锚在内容上方。
/// 两边几何本就不同，共用的是文字排版。所有导出入口（画布三条、CLI 的 PDF）都要过这里，
/// 否则「画布导出有、命令行导出没有」这种缺口会静悄悄存在。
pub fn attach_focus_note(
    scene: &mut mw_core::Scene,
    doc: &mw_core::Document,
    theme: &mw_core::Theme,
) {
    if !scene.kind.is_concept_map() {
        return;
    }
    let question = doc
        .meta
        .layout
        .focus_question
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty());
    if let Some(q) = question {
        // 用 `ui.dim`：画布那条带子不悬停时就是这个颜色（`paint_focus_question`），
        // 导出换个更黑的墨，就成了「屏幕上一种、导出来另一种」。
        scene.attach_focus_note(q, mw_core::design::content::READING_BODY.size, theme.ui.dim);
    }
}

/// 导出取景框：`Scene::bounds` 只圈节点与摘要盒（画布靠视口裁剪，够用），
/// 但导出必须把边界框、关系线鼓出的弧、折叠圆点这些「盒子之外」的几何也框进来，
/// 否则图边会被切掉一截——关系线的垂直偏置最多能鼓出 28 世界单位。
///
/// 最后统一留 2 世界单位给描边。PNG 与 SVG 共用这一个取景框，两种导出不许各算各的。
pub fn content_bounds(scene: &mw_core::Scene) -> mw_core::Rect {
    use mw_core::geom::Enclosing;
    use mw_core::layout::{edge_path, toggle_pos, Edge};
    use mw_core::{PathSeg, Point};
    use mw_render::instances::{
        REL_LABEL_H, REL_LABEL_MIN_W, REL_START_FLOOR, REL_START_R, TOGGLE_R,
    };

    fn add_point(e: &mut Enclosing, p: Point) {
        e.add(mw_core::Rect::new(p.x, p.y, 0.0, 0.0));
    }
    fn add_path(e: &mut Enclosing, path: &mw_core::Path) {
        add_point(e, path.start);
        for seg in &path.segs {
            match *seg {
                PathSeg::Line { to } => add_point(e, to),
                PathSeg::Quad { ctrl, to } => {
                    add_point(e, ctrl);
                    add_point(e, to);
                }
                PathSeg::Cubic { c1, c2, to } => {
                    add_point(e, c1);
                    add_point(e, c2);
                    add_point(e, to);
                }
            }
        }
    }

    let mut e = Enclosing::new();
    e.add(scene.bounds);
    // 聚焦问题条在 `bounds` 之外（向上扩），显式加一次，别只靠扩 bounds 那本账。
    if let Some(note) = &scene.focus_note {
        e.add(note.rect);
    }
    for b in &scene.boundaries {
        e.add(b.rect);
    }
    for edge in &scene.edges {
        let path = edge_path(&Edge {
            from: edge.from,
            to: edge.to,
            start: edge.start,
            end: edge.end,
            orientation: edge.orientation,
        });
        add_path(&mut e, &path);
    }
    for s in &scene.summaries {
        add_path(&mut e, &s.brace);
        add_path(&mut e, &s.connector);
        e.add(s.topic);
    }
    for r in &scene.relationships {
        add_path(&mut e, &r.path);
        for tri in [r.arrow, r.arrow_back].into_iter().flatten() {
            // 取景框与 SVG/PDF 用同一个 100% 口径的箭头，否则外探的底边会被裁掉。
            for p in mw_core::geom::arrow_at_zoom(tri, 1.0) {
                add_point(&mut e, p);
            }
        }
        let dot = REL_START_R.max(REL_START_FLOOR);
        e.add(mw_core::Rect::new(
            r.start.x - dot,
            r.start.y - dot,
            dot * 2.0,
            dot * 2.0,
        ));
        if r.label.is_some() {
            let w = r.label_w.max(REL_LABEL_MIN_W);
            e.add(mw_core::Rect::new(
                r.label_at.x - w * 0.5,
                r.label_at.y - REL_LABEL_H * 0.5,
                w,
                REL_LABEL_H,
            ));
        }
    }
    for idx in &scene.draw_order {
        let n = scene.node(*idx);
        if !n.has_children {
            continue;
        }
        let Some(p) = n.parent else { continue };
        let local = toggle_pos(n.geom.rect, scene.kind, Some(scene.node(p).geom.rect.x));
        let c = Point::new(n.geom.rect.x + local.x, n.geom.rect.y + local.y);
        e.add(mw_core::Rect::new(
            c.x - TOGGLE_R,
            c.y - TOGGLE_R,
            TOGGLE_R * 2.0,
            TOGGLE_R * 2.0,
        ));
    }
    e.rect().unwrap_or(scene.bounds).expanded(2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::Node;

    fn doc_with(children: &[&str]) -> Document {
        let mut d = Document::new("根");
        let root = d.root_id.clone();
        // `Document::new(title)` 只写 meta.title，根节点文字是空的——导出读的是节点文字。
        d.nodes.get_mut(&root).unwrap().text = "根".into();
        for t in children {
            let id = mw_core::new_id();
            let n = Node::new(id.clone(), Some(root.clone()), *t);
            d.nodes.insert(id.clone(), n);
            d.nodes.get_mut(&root).unwrap().children.push(id);
        }
        d
    }

    /// 范围只对「有画面」的格式有意义；文本四种永远导整份文档。
    #[test]
    fn scope_belongs_to_drawing_formats_only() {
        assert_eq!(Scope::Whole.label(), "整张导图");
        assert_eq!(Scope::Selection.label(), "选中子树");
        assert_eq!(Scope::Viewport.label(), "可视区域");
        for f in Format::ALL {
            assert_eq!(
                f.has_background(),
                matches!(f, Format::Png | Format::Svg | Format::Pdf),
                "{f:?} 的「范围」可见性与背景选项不一致"
            );
        }
    }

    #[test]
    fn formats_have_distinct_ext_and_label() {
        for (i, a) in Format::ALL.iter().enumerate() {
            assert!(!a.ext().is_empty() && !a.label().is_empty());
            for b in &Format::ALL[i + 1..] {
                assert_ne!(a.ext(), b.ext(), "{a:?} 与 {b:?} 后缀撞了");
                assert_ne!(a.label(), b.label());
            }
        }
    }

    #[test]
    fn raster_formats_have_scale_and_all_drawings_have_a_background() {
        for f in Format::ALL {
            assert_eq!(f.is_image(), matches!(f, Format::Png | Format::Pdf));
            assert_eq!(
                f.has_background(),
                matches!(f, Format::Png | Format::Svg | Format::Pdf)
            );
        }
    }

    #[test]
    fn text_formats_serialize_and_drawings_do_not() {
        let d = doc_with(&["甲", "乙"]);
        for f in [Format::Png, Format::Svg, Format::Pdf] {
            assert_eq!(f.text(&d), None, "{f:?} 不走文本管线");
        }
        for f in [
            Format::Markdown,
            Format::Outline,
            Format::Opml,
            Format::Freemind,
        ] {
            let out = f.text(&d).expect("文本格式必须有内容");
            assert!(out.contains('根') && out.contains('甲'), "{f:?} 丢了内容");
        }
    }

    #[test]
    fn file_stem_scrubs_illegal_chars_and_falls_back() {
        assert_eq!(file_stem("季度:计划"), "季度 计划");
        assert_eq!(file_stem("a/b\\c"), "a b c");
        assert_eq!(file_stem("尾巴点..."), "尾巴点");
        assert_eq!(file_stem("   "), "未命名");
        assert_eq!(file_stem("换\n行"), "换 行");
        // `Session::title()` 带 .zmind 后缀：默认导出名不该继承它，否则选 PNG 会存成 .zmind。
        assert_eq!(file_stem("未命名.zmind"), "未命名");
        assert_eq!(file_stem("季度计划.ZMIND"), "季度计划");
        assert_eq!(file_stem(".zmind"), "未命名");
        // 导入的文档标题是来源文件名：导出默认名不该带着 .md/.xmind 出去。
        assert_eq!(file_stem("大纲.md"), "大纲");
        assert_eq!(file_stem("笔记.MARKDOWN"), "笔记");
        assert_eq!(file_stem("脑图.xmind"), "脑图");
        assert_eq!(file_stem("讨论.opml"), "讨论");
        assert_eq!(file_stem("旧版.mm"), "旧版");
        assert_eq!(file_stem("清单.txt"), "清单");
        // 再长也截断，免得默认名把对话框撑爆
        assert!(file_stem(&"长".repeat(200)).chars().count() <= 48);
    }

    #[test]
    fn with_ext_only_fills_missing_suffix() {
        assert_eq!(
            with_ext(PathBuf::from("D:/a/计划"), "png"),
            PathBuf::from("D:/a/计划.png")
        );
        assert_eq!(
            with_ext(PathBuf::from("D:/a/计划.md"), "png"),
            PathBuf::from("D:/a/计划.md")
        );
    }

    #[test]
    fn unpremultiply_inverts_the_shader_convention() {
        // 预乘 50% 白 → 直通白
        let mut px = vec![128, 128, 128, 128];
        unpremultiply(&mut px);
        assert_eq!(px, vec![255, 255, 255, 128]);
        // 全透明：RGB 归零
        let mut px = vec![9, 9, 9, 0];
        unpremultiply(&mut px);
        assert_eq!(px, vec![0, 0, 0, 0]);
        // 不透明：原样
        let mut px = vec![10, 20, 30, 255];
        unpremultiply(&mut px);
        assert_eq!(px, vec![10, 20, 30, 255]);
    }

    #[test]
    fn tiles_cover_the_image_without_gaps_and_blit_seamlessly() {
        let (w, h) = (5000u32, 3000u32);
        let tiles = tile_plan(w, h, 4096);
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].w + tiles[1].w, w);
        assert_eq!((tiles[0].x, tiles[1].x), (0, 4096));
        // 合成一张渐变图：逐块切出再拼回，逐像素应完全一致（接缝差 0）。
        let mut full = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                full[i] = (x % 251) as u8;
                full[i + 1] = (y % 241) as u8;
                full[i + 2] = ((x + y) % 239) as u8;
                full[i + 3] = 255;
            }
        }
        let mut out = vec![0u8; full.len()];
        for t in &tiles {
            let mut piece = vec![0u8; (t.w * t.h * 4) as usize];
            for row in 0..t.h as usize {
                let src = ((t.y as usize + row) * w as usize + t.x as usize) * 4;
                let dst = row * t.w as usize * 4;
                piece[dst..dst + t.w as usize * 4]
                    .copy_from_slice(&full[src..src + t.w as usize * 4]);
            }
            blit_tile(&mut out, w, *t, &piece, t.w);
        }
        assert_eq!(out, full, "分块合成必须与整图逐字节一致");
    }

    #[test]
    fn png_size_caps_at_32_megapixels() {
        let (w, h, _) = png_size(8000.0, 4000.0, 1.0);
        assert_eq!((w, h), (8000, 4000));
        assert!(w as f32 * h as f32 <= PNG_MAX_PIXELS + 1.0);
        let (w, h, _) = png_size(12000.0, 8000.0, 1.0);
        assert!((w as f32 * h as f32) <= PNG_MAX_PIXELS + 1000.0);
        assert!(w <= PNG_MAX_SIDE && h <= PNG_MAX_SIDE);
    }

    #[test]
    fn png_size_honors_scale_and_clamps_oversize() {
        let (w, h, s) = png_size(400.0, 300.0, 2.0);
        assert_eq!((w, h), (800, 600));
        assert!((s - 2.0).abs() < 1e-6);

        // 单边超限：比例被缩回去，而不是裁掉内容
        let (w, h, s) = png_size(20000.0, 1000.0, 1.0);
        assert!(w <= PNG_MAX_SIDE && h >= 1);
        assert!(s < 1.0 && s > 0.0);
        assert!((h as f32 / 1000.0 - s).abs() < 0.01);

        // 退化的极小包围盒也要出至少 1×1
        let (w, h, _) = png_size(0.0, 0.0, 1.0);
        assert_eq!((w, h), (1, 1));
    }

    #[test]
    fn content_bounds_cover_off_box_geometry() {
        use mw_core::{
            build_scene, get_theme, LayoutSettings, MonoMeasurer, PathSeg, Relationship,
        };

        let mut d = doc_with(&["甲", "乙"]);
        let root = d.root_id.clone();
        let second = d.nodes[&root].children[1].clone();
        d.nodes
            .get_mut(&root)
            .unwrap()
            .relationships
            .push(Relationship {
                id: "rel".into(),
                to: second,
                label: Some("关联".into()),
                color: None,
                head: Default::default(),
                ..Default::default()
            });
        let scene = build_scene(
            &d,
            &root,
            &LayoutSettings::default(),
            &get_theme("snow"),
            &MonoMeasurer,
            None,
        );
        let b = content_bounds(&scene);
        assert!(b.x <= scene.bounds.x && b.y <= scene.bounds.y);
        assert!(b.right() >= scene.bounds.right() && b.bottom() >= scene.bounds.bottom());

        let rel = &scene.relationships[0];
        let mut points = vec![rel.start, rel.label_at];
        for tri in [rel.arrow, rel.arrow_back].into_iter().flatten() {
            points.extend(mw_core::geom::arrow_at_zoom(tri, 1.0));
        }
        points.push(rel.path.start);
        for seg in &rel.path.segs {
            match *seg {
                PathSeg::Line { to } => points.push(to),
                PathSeg::Quad { ctrl, to } => {
                    points.push(ctrl);
                    points.push(to);
                }
                PathSeg::Cubic { c1, c2, to } => {
                    points.push(c1);
                    points.push(c2);
                    points.push(to);
                }
            }
        }
        for p in points {
            assert!(
                p.x >= b.x && p.x <= b.right() && p.y >= b.y && p.y <= b.bottom(),
                "{p:?} 超出取景框 {b:?}"
            );
        }

        // 折叠圆点向外探出节点盒 11 + 半径 8，取景框必须兜住。
        for idx in &scene.draw_order {
            let n = scene.node(*idx);
            if !n.has_children {
                continue;
            }
            let Some(p) = n.parent else { continue };
            let local = mw_core::layout::toggle_pos(
                n.geom.rect,
                scene.kind,
                Some(scene.node(p).geom.rect.x),
            );
            let cx = n.geom.rect.x + local.x;
            let cy = n.geom.rect.y + local.y;
            assert!(
                cx + 8.0 <= b.right() + 1e-3 && cx - 8.0 >= b.x - 1e-3,
                "折叠圆点横向被切：{cx} vs {b:?}"
            );
            assert!(
                cy + 8.0 <= b.bottom() + 1e-3 && cy - 8.0 >= b.y - 1e-3,
                "折叠圆点纵向被切：{cy} vs {b:?}"
            );
        }
    }

    // ---------------------------------------------------------------- SR-01 文本导出原子性

    /// 导出落地目录：仓库内 `target/tmp/tests`（与 `mw-format` 的测试同一口径，
    /// 本机用户 Temp 偶发拒写会让写盘用例假红）。
    fn export_scratch(prefix: &str) -> std::path::PathBuf {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/tests");
        let dir = base.join(format!("{}-{}", prefix, mw_core::new_id()));
        std::fs::create_dir_all(&dir).expect("测试临时目录必须能创建");
        dir
    }

    /// `.part` 不带出去：成功不残留，失败也不残留。
    fn part_of(dir: &Path, name: &str) -> std::path::PathBuf {
        dir.join(format!(".{name}.part"))
    }

    /// 文本导出必须原子：正常导出写成整份内容、不留 `.part`；覆盖导出同样不留。
    #[test]
    fn text_export_writes_atomically_without_leaving_a_part_file() {
        let dir = export_scratch("mw-export");
        let out = dir.join("导图.md");

        write_text(&out, "# 根\n\n- 甲\n- 乙\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "# 根\n\n- 甲\n- 乙\n",
            "导出的内容要一字不差"
        );
        assert!(!part_of(&dir, "导图.md").exists(), "成功后不该残留 .part");

        // 覆盖导出：整份换成新版内容，旧的一半都不能留。
        write_text(&out, "第二版").unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "第二版");
        assert!(!part_of(&dir, "导图.md").exists(), "覆盖后不该残留 .part");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 失败导出（目标是已存在的目录 → rename 必然失败）：报中文错误、清掉 `.part`、
    /// 而且**不许**在目标位置留下半成品。
    #[test]
    fn a_failed_text_export_reports_and_cleans_up_the_part_file() {
        let dir = export_scratch("mw-export-fail");
        let blocked = dir.join("导出目标");
        std::fs::create_dir_all(&blocked).unwrap();

        let err = write_text(&blocked, "写不进去").unwrap_err();
        assert!(err.starts_with("写入失败："), "错误文案变了：{err}");
        assert!(
            !part_of(&dir, "导出目标").exists(),
            "失败后必须清掉 .part，半成品导出比没有文件更糟"
        );
        assert!(blocked.is_dir(), "目标本身不该被改动");

        // 父目录不存在也一样：报错、不造目录、不留 .part。
        let missing = dir.join("没有这个目录").join("导图.txt");
        let err = write_text(&missing, "x").unwrap_err();
        assert!(err.starts_with("写入失败："), "{err}");
        assert!(!missing.exists());
        assert!(!dir.join("没有这个目录").exists(), "不该顺手造目录");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
