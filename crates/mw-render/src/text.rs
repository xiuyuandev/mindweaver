//! 场景文字 → DirectWrite 字形 → 文本实例（docs/13 §4.9、痛点 C3）。
//!
//! 职责边界：
//! - **排版**在 `mw-core`（断行、禁则、盒宽）；这里只对**最终每一行**做字形选择；
//! - 字形选择走 `IDWriteTextLayout::Draw` + 自定义 `IDWriteTextRenderer`，回调里的
//!   `fontFace` 已经是字体回退后的结果（中英混排可能一行两个 face）；
//! - 光栅化与图集在 [`crate::glyph`]；这里负责键、缓存、对齐与基线；
//! - 基线公式在 `mw_core::layout::line_baseline`（旧 SVG 的 `line_height × 0.72`），
//!   行内编辑器共用同一条，所以 DWrite 的 14.22 与我们的 20px 行盒之间的差在这里被
//!   吸收，不回流到布局层。
//!
//! 中英之间 0.1 em 的间隙也在这一层加：它是字距不是文本内容，不能污染 `lines`
//! （docs/13 §4.8 决定 5）。做法是给每个字形记一个 `gap_before` 步进，居中时
//! 总宽也算上间隙，保证视觉居中而不是「字面居中、看起来偏左」。

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use mw_core::layout::line_baseline_with_image;
use mw_core::text::{is_cjk, NO_WRAP};
use mw_core::theme::Rgba;
use windows::core::{implement, IUnknown, Interface, Ref, Result, BOOL};
use windows::Win32::Graphics::DirectWrite::*;

use crate::dwrite::{utf16_units, DWrite};
use crate::glyph::{quantize_ppd, AtlasUpdate, GlyphAtlas, GlyphKey};
use crate::instances::{
    rgba, Frame, LayerGeom, TextInst, GHOST_ALPHA, GHOST_ORIGIN_ALPHA, LAYERS, L_NODES, L_OVERLAY,
    REL_LABEL_H, REL_LABEL_MIN_W,
};

/// 形状缓存上限（按不同字符串计）：与度量缓存同策略，超了整张丢。
const SHAPE_CACHE_LIMIT: usize = 20_000;
/// 中英之间的视觉间隙（DIP，按字号缩放）。
const CJK_LATIN_GAP_EM: f32 = 0.1;
/// 按系数压暗一个颜色（幽灵与其原位残影共用）。只动 alpha，不动 RGB——
/// 预乘混合下这样才是「同一块颜色变淡」，不会发灰发白。
fn dim(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * k]
}

#[derive(Clone, Copy, Debug)]
struct ShapedGlyph {
    face: u32,
    glyph: u16,
    /// 相对行首笔位的水平偏移（DIP，含 DWrite 的 glyph offset）
    x: f32,
    /// 垂直偏移（DIP，向下为正；来自 ascenderOffset）
    dy: f32,
    /// 本字形之前要额外推进的中英间隙
    gap_before: f32,
}

struct ShapedLine {
    glyphs: Vec<ShapedGlyph>,
    width: f32,
}

/// 一次 Draw 回调收到的原始字形。face 在回调后统一去重。
struct RawGlyph {
    face: IDWriteFontFace,
    glyph: u16,
    x: f32,
    advance: f32,
    dy: f32,
    /// 在整行文本里的 UTF-16 起始下标，用来判断中英边界
    text_index: u32,
}

#[implement(IDWriteTextRenderer, Agile = false)]
struct Collector {
    glyphs: Rc<RefCell<Vec<RawGlyph>>>,
}

impl IDWritePixelSnapping_Impl for Collector_Impl {
    fn IsPixelSnappingDisabled(&self, _: *const c_void) -> Result<BOOL> {
        Ok(BOOL(1))
    }
    fn GetCurrentTransform(&self, _: *const c_void, transform: *mut DWRITE_MATRIX) -> Result<()> {
        unsafe {
            *transform = DWRITE_MATRIX {
                m11: 1.0,
                m12: 0.0,
                m21: 0.0,
                m22: 1.0,
                dx: 0.0,
                dy: 0.0,
            };
        }
        Ok(())
    }
    fn GetPixelsPerDip(&self, _: *const c_void) -> Result<f32> {
        Ok(1.0)
    }
}

impl IDWriteTextRenderer_Impl for Collector_Impl {
    fn DrawGlyphRun(
        &self,
        _: *const c_void,
        baseline_x: f32,
        _baseline_y: f32,
        _: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        description: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        let run = unsafe { &*run };
        if run.isSideways.as_bool() || run.glyphCount == 0 {
            return Ok(());
        }
        let Some(face) = <Option<IDWriteFontFace> as Clone>::clone(&run.fontFace) else {
            return Ok(());
        };
        let desc = unsafe { description.as_ref() };
        let text_position = desc.map(|d| d.textPosition).unwrap_or(0);
        let cluster_map = desc.map(|d| d.clusterMap).unwrap_or(std::ptr::null());
        let mut x = baseline_x;
        let mut glyphs = self.glyphs.borrow_mut();
        for i in 0..run.glyphCount as usize {
            let glyph = unsafe { *run.glyphIndices.add(i) };
            let advance = unsafe { *run.glyphAdvances.add(i) };
            let (dx, dy) = if run.glyphOffsets.is_null() {
                (0.0, 0.0)
            } else {
                let off = unsafe { *run.glyphOffsets.add(i) };
                (off.advanceOffset, -off.ascenderOffset)
            };
            let text_index = if cluster_map.is_null() {
                text_position + i as u32
            } else {
                text_position + unsafe { *cluster_map.add(i) } as u32
            };
            glyphs.push(RawGlyph {
                face: face.clone(),
                glyph,
                x: x + dx,
                advance,
                dy,
                text_index,
            });
            x += advance;
        }
        Ok(())
    }
    fn DrawUnderline(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_UNDERLINE,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
    fn DrawStrikethrough(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_STRIKETHROUGH,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
    fn DrawInlineObject(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: Ref<IDWriteInlineObject>,
        _: BOOL,
        _: BOOL,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FaceKey {
    family: String,
    weight: i32,
    stretch: i32,
    style: i32,
    simulations: i32,
}

struct ShapeEntry {
    font_size: u32,
    weight: u16,
    line: Rc<ShapedLine>,
}

/// 文本绘制引擎：持有 DWrite、字形图集与形状缓存。与 UI 同线程使用。
pub struct TextEngine {
    dwrite: DWrite,
    faces: Vec<IDWriteFontFace>,
    face_ids: HashMap<FaceKey, u32>,
    shapes: RefCell<HashMap<String, Vec<ShapeEntry>>>,
    atlas: GlyphAtlas,
}

impl TextEngine {
    /// 非 Windows / 无 DirectWrite 时返回 `None`；产品里不应发生。
    pub fn new() -> Option<Self> {
        Some(Self::with_dwrite(DWrite::new()?))
    }

    pub fn with_dwrite(dwrite: DWrite) -> Self {
        let atlas = GlyphAtlas::new(dwrite.factory());
        Self {
            dwrite,
            faces: Vec::new(),
            face_ids: HashMap::new(),
            shapes: RefCell::new(HashMap::new()),
            atlas,
        }
    }

    pub fn dwrite(&self) -> &DWrite {
        &self.dwrite
    }

    /// 丢弃图集缓存。合成器报告代次断档时调用；下一帧 `paint` 会重排全部字形，
    /// 旧 UV 随旧槽位一起作废。
    pub fn reset_atlas(&mut self) {
        self.atlas.reset();
    }

    /// 把场景里的所有文字排进四层的 `texts`，返回图集增量。
    /// **两趟**：先收集/形状化全部行并确保字形在位，再统一发实例——于是图集即使
    /// 中途满了重新排，也不会让已经发出的 UV 失效。
    pub fn paint(&mut self, f: &Frame, layers: &mut [LayerGeom; LAYERS]) -> AtlasUpdate {
        for layer in layers.iter_mut() {
            layer.texts.clear();
        }
        let ppd = quantize_ppd(f.camera.zoom * f.camera.ppp);
        let jobs = text_jobs(f);

        let mut prepared = Vec::with_capacity(jobs.len());
        for job in jobs {
            let Some(line) = self.shape(job.text, job.font_size, job.weight) else {
                continue;
            };
            let pen_x = match job.align {
                TextAlign::Left(x) => x,
                TextAlign::Center(c) => c - line.width * 0.5,
                TextAlign::Right(r) => r - line.width,
            };
            prepared.push(Prepared {
                line,
                font_size: job.font_size,
                color: job.color,
                pen_x,
                baseline: job.baseline,
                layer: job.layer,
            });
        }

        let mut keys = Vec::new();
        for p in &prepared {
            for g in &p.line.glyphs {
                keys.push(GlyphKey::new(g.face, g.glyph, p.font_size, ppd));
            }
        }
        self.ensure_glyphs(&keys);

        for p in &prepared {
            let mut gap = 0.0f32;
            for g in &p.line.glyphs {
                gap += g.gap_before;
                let key = GlyphKey::new(g.face, g.glyph, p.font_size, ppd);
                let Some(slot) = self.atlas.get(&key) else {
                    continue;
                };
                if slot.is_empty() {
                    continue;
                }
                let (pos, size) = slot.quad(p.pen_x + gap + g.x, p.baseline + g.dy);
                layers[p.layer].texts.push(TextInst {
                    pos,
                    size,
                    uv: slot.uv(),
                    color: p.color,
                });
            }
        }

        self.atlas.take_update()
    }

    /// 场景文字 → 矢量轮廓（PDF 导出用）。与 [`Self::paint`] 共用同一份排版，
    /// 每个字形按最终笔位平移；返回按 TextJob 分组的路径与颜色。
    pub fn outlines(&mut self, f: &Frame) -> Vec<OutlinedText> {
        let mut out = Vec::new();
        let mut cache: HashMap<(u32, u16, u32), Vec<mw_core::Path>> = HashMap::new();
        for job in text_jobs(f) {
            let Some(line) = self.shape(job.text, job.font_size, job.weight) else {
                continue;
            };
            let pen_x = match job.align {
                TextAlign::Left(x) => x,
                TextAlign::Center(c) => c - line.width * 0.5,
                TextAlign::Right(r) => r - line.width,
            };
            let mut gap = 0.0f32;
            let mut paths = Vec::new();
            for g in &line.glyphs {
                gap += g.gap_before;
                let key = (g.face, g.glyph, job.font_size.to_bits());
                let glyph_paths = cache.entry(key).or_insert_with(|| {
                    crate::outline::glyph_paths(
                        &self.faces[g.face as usize],
                        g.glyph,
                        job.font_size,
                    )
                });
                let mut moved = glyph_paths.clone();
                crate::outline::translate(&mut moved, pen_x + gap + g.x, job.baseline + g.dy);
                paths.append(&mut moved);
            }
            if !paths.is_empty() {
                out.push(OutlinedText {
                    paths,
                    color: job.color,
                    bold: job.bold,
                    weight: job.weight,
                    font_size: job.font_size,
                    layer: job.layer,
                });
            }
        }
        out
    }

    fn ensure_glyphs(&mut self, keys: &[GlyphKey]) {
        for _ in 0..2 {
            let mut full = false;
            for key in keys {
                if self.atlas.contains(key) {
                    continue;
                }
                let face = &self.faces[key.face as usize];
                match self.atlas.insert(*key, face) {
                    crate::glyph::Insert::Ready(_) | crate::glyph::Insert::Failed => {}
                    crate::glyph::Insert::Full => {
                        full = true;
                        break;
                    }
                }
            }
            if !full {
                return;
            }
            self.atlas.reset();
        }
    }

    fn shape(&mut self, text: &str, font_size: f32, weight: u16) -> Option<Rc<ShapedLine>> {
        if text.is_empty() {
            return None;
        }
        let size_bits = font_size.to_bits();
        if let Some(hit) = self.shapes.borrow().get(text).and_then(|list| {
            list.iter()
                .find(|e| e.font_size == size_bits && e.weight == weight)
                .map(|e| e.line.clone())
        }) {
            return Some(hit);
        }
        let line = self.shape_uncached(text, font_size, weight)?;
        let mut cache = self.shapes.borrow_mut();
        if cache.len() >= SHAPE_CACHE_LIMIT {
            cache.clear();
        }
        cache.entry(text.to_string()).or_default().push(ShapeEntry {
            font_size: size_bits,
            weight,
            line: line.clone(),
        });
        Some(line)
    }

    fn shape_uncached(
        &mut self,
        text: &str,
        font_size: f32,
        weight: u16,
    ) -> Option<Rc<ShapedLine>> {
        let format = self.dwrite.format(font_size, weight).ok()?;
        let units = utf16_units(text);
        let layout = unsafe {
            self.dwrite
                .factory()
                .CreateTextLayout(&units, &format, NO_WRAP, f32::MAX)
                .ok()?
        };
        unsafe { layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP) }.ok()?;
        let raw = Rc::new(RefCell::new(Vec::new()));
        let renderer: IDWriteTextRenderer = Collector {
            glyphs: raw.clone(),
        }
        .into();
        unsafe { layout.Draw(None, &renderer, 0.0, 0.0) }.ok()?;

        let gaps = cjk_latin_gaps(text);
        let raws = raw.borrow();
        let mut glyphs = Vec::with_capacity(raws.len());
        let mut width = 0.0f32;
        let mut gap_sum = 0.0f32;
        for raw in raws.iter() {
            let gap_before = if gaps.iter().any(|p| *p == raw.text_index) {
                CJK_LATIN_GAP_EM * font_size
            } else {
                0.0
            };
            gap_sum += gap_before;
            width = width.max(raw.x + raw.advance + gap_sum);
            glyphs.push(ShapedGlyph {
                face: self.intern_face(&raw.face),
                glyph: raw.glyph,
                x: raw.x,
                dy: raw.dy,
                gap_before,
            });
        }
        Some(Rc::new(ShapedLine { glyphs, width }))
    }

    fn intern_face(&mut self, face: &IDWriteFontFace) -> u32 {
        let key = face_key(face);
        if let Some(key) = &key {
            if let Some(&id) = self.face_ids.get(key) {
                return id;
            }
        }
        let id = self.faces.len() as u32;
        if let Some(key) = key {
            self.face_ids.insert(key, id);
        }
        self.faces.push(face.clone());
        id
    }
}

struct Prepared {
    line: Rc<ShapedLine>,
    font_size: f32,
    color: [f32; 4],
    pen_x: f32,
    baseline: f32,
    layer: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left(f32),
    Center(f32),
    Right(f32),
}

/// 节点文字的水平锚点：显式 `align` 覆盖优先；概念图默认居中；其余左对齐。
///
/// 概念是短语，盒宽正好是「文字 + 28」，居中与左对齐在单行时看不出差别；
/// 双行时左对齐会让第二行明显偏左（`docs/30` §5.3 要求居中）。
fn node_text_align(n: &mw_core::scene::SceneNode, ox: f32) -> TextAlign {
    use mw_core::theme::TextAlign as NodeAlign;
    match n.style.align {
        Some(NodeAlign::Center) => TextAlign::Center(ox + n.geom.rect.w * 0.5),
        // 右锚点用**内距**而不是 `text_x`：`text_x = pad_x + 图标宽`，有图标的节点若拿它
        // 当右内距，文字会被图标那一截挤左（右内距比左内距多出一个图标宽）。
        Some(NodeAlign::Right) => TextAlign::Right(ox + n.geom.rect.w - n.geom.pad.0),
        Some(NodeAlign::Left) => TextAlign::Left(ox + n.text_x),
        None if n.center_text => TextAlign::Center(ox + n.geom.rect.w * 0.5),
        None => TextAlign::Left(ox + n.text_x),
    }
}

/// 一组矢量字形轮廓：一条排版行（或图标/标签）的全部路径，世界坐标。
#[derive(Clone, Debug)]
pub struct OutlinedText {
    pub paths: Vec<mw_core::Path>,
    pub color: [f32; 4],
    pub bold: bool,
    /// 生效字重（400/500/600/700）；导出按它写 `font-weight` / 决定描边（SE-13）。
    pub weight: u16,
    pub font_size: f32,
    pub layer: usize,
}

/// 一条排版好的文字：内容、字号、颜色、锚点与基线都已定死。
///
/// 谁来决定「字形怎么落地」由调用方负责——画布走 DirectWrite 图集，SVG 导出走
/// `<text>` 标记。两边共用这一份排版结果，否则「导出的字比画布偏一格」这类漂移
/// 迟早会出现（docs/13 §4.24）。
#[derive(Clone, Copy, Debug)]
pub struct TextJob<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub bold: bool,
    /// 生效字重（400/500/600/700）。节点文字取 `ResolvedStyle.weight`，其余按 `bold` 折算。
    pub weight: u16,
    pub color: [f32; 4],
    pub align: TextAlign,
    pub baseline: f32,
    pub layer: usize,
}

/// `bold` → 旧行为字重（600/400）；新控件用 `weight` 覆盖（SE-13）。
///
/// 600 不是 700：TYPO-01 定下的画布粗体字面就是 Semibold，取值见
/// `mw_core::theme::WEIGHT_BOLD_ALIAS`。
pub fn weight_of_bold(bold: bool) -> u16 {
    if bold {
        mw_core::theme::WEIGHT_BOLD_ALIAS
    } else {
        mw_core::theme::WEIGHT_REGULAR
    }
}

/// 把当前帧要画的文字排成任务清单。**GPU 无关**：画布与 SVG 导出共用同一份结果。
pub fn text_jobs<'a>(f: &Frame<'a>) -> Vec<TextJob<'a>> {
    let mut jobs = Vec::new();
    collect_jobs(f, &mut jobs);
    jobs
}

/// 收集顺序即层内绘制顺序：节点（按 draw_order）→ 摘要 → 关系线标签。
///
/// 与 `build_layers` 走同一套可见性判定：视口外的盒不排字，缩放太小（字形 < 7 物理 px）
/// 时只留盒不留字，节点数到 LOD 阈值后标签胶囊整体不排字。
fn collect_jobs<'a>(f: &Frame<'a>, jobs: &mut Vec<TextJob<'a>>) {
    let canvas_text = rgba(f.theme.ui.canvas_bg);
    let labels = f.labels_visible();
    // 被拖动的子树：原位的字要跟着盒子一起淡化，否则会出现「框淡了字还黑着」的半截效果。
    let mask = f.ghost_mask();
    let origin_alpha = |idx: u32| -> f32 {
        let node = f.scene.nodes.get(idx as usize).map_or(1.0, |n| n.alpha);
        let ghost = match &mask {
            Some(m) if m.get(idx as usize).copied().unwrap_or(false) => GHOST_ORIGIN_ALPHA,
            _ => 1.0,
        };
        node * ghost
    };
    for idx in f.scene.draw_order.iter() {
        let n = f.scene.node(*idx);
        if !f.sees(n.geom.rect) {
            continue;
        }
        let ox = n.geom.rect.x;
        let oy = n.geom.rect.y;
        let text_on = f.text_visible(n.geom.font_size);
        // 行内编辑时节点文字由 egui 接管，这里再画一遍会在编辑器底下透出重影。
        let editing = f.editing == Some(*idx);
        // 基线只有一个真源（`mw_core::layout::line_baseline`）：行内编辑器用它把文字摆在
        // 同一个位置，这里再抄一份公式，按 F2 的瞬间就会跳。
        let base = |i: usize| {
            oy + line_baseline_with_image(
                n.geom.rect.h,
                n.geom.label_h,
                n.geom.image_h,
                n.geom.lines.len(),
                n.geom.line_height,
                i,
            )
        };
        if text_on && !editing {
            for (i, line) in n.geom.lines.iter().enumerate() {
                if line.is_empty() {
                    continue;
                }
                jobs.push(TextJob {
                    text: line,
                    font_size: n.geom.font_size,
                    bold: n.geom.bold,
                    weight: n.style.weight,
                    color: dim(rgba(n.style.text), origin_alpha(*idx)),
                    align: node_text_align(n, ox),
                    baseline: base(i),
                    layer: L_NODES,
                });
            }
            if let Some(icon) = n.icon {
                if !icon.glyph.is_empty() {
                    jobs.push(TextJob {
                        text: icon.glyph,
                        font_size: n.geom.font_size,
                        bold: false,
                        // 图标槽宽是 `字号 + 6`（`icons::icon_width`，与字重无关），
                        // 跟着节点字重画会让 600 的字形超出自己的槽、压到标题上。
                        weight: weight_of_bold(false),
                        color: dim(
                            rgba(Rgba::parse(icon.color).unwrap_or(n.style.text)),
                            origin_alpha(*idx),
                        ),
                        align: TextAlign::Left(ox + n.geom.pad.0),
                        baseline: base(0),
                        layer: L_NODES,
                    });
                }
            }
        }
        if !labels {
            continue;
        }
        for p in n.labels.iter() {
            if p.text.is_empty() || !f.text_visible(p.font_size) {
                continue;
            }
            let x = ox + p.rect.x;
            let y = oy + p.rect.y;
            if !f.sees(mw_core::Rect::new(x, y, p.rect.w, p.rect.h)) {
                continue;
            }
            jobs.push(TextJob {
                text: &p.text,
                font_size: p.font_size,
                bold: false,
                weight: weight_of_bold(false),
                color: dim(canvas_text, origin_alpha(*idx)),
                align: TextAlign::Center(x + p.rect.w * 0.5),
                baseline: y + p.rect.h * 0.5 + p.font_size * 0.36,
                layer: L_NODES,
            });
        }
    }

    // ---- 幽灵副本的文字：跟盒子一起平移、一起压暗 ----
    if let Some(g) = f.ghost.as_ref() {
        let (dx, dy) = (g.offset.x, g.offset.y);
        for &i in &g.nodes {
            let Some(n) = f.scene.nodes.get(i as usize) else {
                continue;
            };
            let r = mw_core::Rect::new(
                n.geom.rect.x + dx,
                n.geom.rect.y + dy,
                n.geom.rect.w,
                n.geom.rect.h,
            );
            if !f.sees(r) || !f.text_visible(n.geom.font_size) {
                continue;
            }
            // 原位正在编辑的那个节点，幽灵也不能出字：字形归 egui 编辑器，这里画会重影。
            if f.editing == Some(i) {
                continue;
            }
            let ox = r.x;
            let oy = r.y;
            let ghost_alpha = GHOST_ALPHA * n.alpha;
            for (li, line) in n.geom.lines.iter().enumerate() {
                if line.is_empty() {
                    continue;
                }
                jobs.push(TextJob {
                    text: line,
                    font_size: n.geom.font_size,
                    bold: n.geom.bold,
                    weight: n.style.weight,
                    color: dim(rgba(n.style.text), ghost_alpha),
                    align: node_text_align(n, ox),
                    baseline: oy
                        + line_baseline_with_image(
                            n.geom.rect.h,
                            n.geom.label_h,
                            n.geom.image_h,
                            n.geom.lines.len(),
                            n.geom.line_height,
                            li,
                        ),
                    layer: L_NODES,
                });
            }
            if !labels {
                continue;
            }
            for p in n.labels.iter() {
                if p.text.is_empty() || !f.text_visible(p.font_size) {
                    continue;
                }
                let x = ox + p.rect.x;
                let y = oy + p.rect.y;
                jobs.push(TextJob {
                    text: &p.text,
                    font_size: p.font_size,
                    bold: false,
                    weight: weight_of_bold(false),
                    color: dim(canvas_text, ghost_alpha),
                    align: TextAlign::Center(x + p.rect.w * 0.5),
                    baseline: y + p.rect.h * 0.5 + p.font_size * 0.36,
                    layer: L_NODES,
                });
            }
        }
    }
    let summary_text = f.text_visible(mw_core::SUMMARY_FONT);
    for s in f.scene.summaries.iter() {
        if s.text.is_empty() || !summary_text || !f.sees(s.topic) {
            continue;
        }
        jobs.push(TextJob {
            text: &s.text,
            font_size: mw_core::SUMMARY_FONT,
            bold: true,
            weight: weight_of_bold(true),
            color: dim(rgba(s.color), s.alpha),
            align: TextAlign::Center(s.topic.x + s.topic.w * 0.5),
            baseline: s.topic.y + s.topic.h * 0.5 + 4.5,
            layer: L_NODES,
        });
    }
    let rel_text = f.text_visible(mw_core::REL_LABEL_FONT);
    for r in f.scene.relationships.iter() {
        // 概念图的空短语要留占位：不然新拖出来的线是一根没头没脑的箭头，
        // 用户不知道「这条线还差一句话」（`docs/30` §5.4 空短语）。
        if r.label.as_deref().is_none_or(|l| l.is_empty()) {
            if !rel_text || !f.scene.kind.is_concept_map() {
                continue;
            }
            if !f.sees(mw_core::Rect::new(
                r.label_at.x - 30.0,
                r.label_at.y - 10.0,
                60.0,
                20.0,
            )) {
                continue;
            }
            jobs.push(TextJob {
                text: "＋ 链接词",
                font_size: mw_core::REL_LABEL_FONT,
                bold: false,
                weight: weight_of_bold(false),
                color: dim(rgba(f.theme.ui.dim), r.alpha),
                align: TextAlign::Center(r.label_at.x),
                baseline: r.label_at.y + 4.0,
                layer: L_OVERLAY,
            });
            continue;
        }
        let Some(label) = r.label.as_deref() else {
            continue;
        };
        if label.is_empty() || !rel_text {
            continue;
        }
        let box_h = REL_LABEL_H;
        let pill = mw_core::Rect::new(
            r.label_at.x - r.label_w.max(REL_LABEL_MIN_W) * 0.5,
            r.label_at.y - box_h * 0.5,
            r.label_w.max(REL_LABEL_MIN_W),
            box_h,
        );
        if !f.sees(pill) {
            continue;
        }
        jobs.push(TextJob {
            text: label,
            font_size: mw_core::REL_LABEL_FONT,
            bold: false,
            weight: weight_of_bold(false),
            color: dim(rgba(f.theme.ui.text), r.alpha),
            align: TextAlign::Center(r.label_at.x),
            baseline: r.label_at.y + 4.0,
            layer: L_OVERLAY,
        });
    }
    // 导出专用的「聚焦问题」条：画布从不填 `focus_note`，所以这段只在 PNG/PDF 导出里生效。
    if let Some(note) = &f.scene.focus_note {
        if !note.text.is_empty() && f.sees(note.rect) {
            jobs.push(TextJob {
                text: &note.text,
                font_size: note.font_size,
                bold: false,
                weight: weight_of_bold(false),
                color: rgba(note.color),
                align: TextAlign::Center(note.rect.x + note.rect.w * 0.5),
                // 与关系线标签同一套基线口径：胶囊中心 + 半个 x-height。
                baseline: note.rect.y + note.rect.h * 0.5 + note.font_size * 0.35,
                layer: L_OVERLAY,
            });
        }
    }
}

/// 行内「汉字 ↔ ASCII 字母数字」边界处的 UTF-16 起始下标。
fn cjk_latin_gaps(text: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut prev: Option<char> = None;
    let mut units = 0u32;
    for ch in text.chars() {
        if let Some(p) = prev {
            if crosses_script(p, ch) {
                out.push(units);
            }
        }
        prev = Some(ch);
        units += ch.len_utf16() as u32;
    }
    out
}

fn crosses_script(a: char, b: char) -> bool {
    (is_cjk(a) && b.is_ascii_alphanumeric()) || (a.is_ascii_alphanumeric() && is_cjk(b))
}

fn face_key(face: &IDWriteFontFace) -> Option<FaceKey> {
    let f3 = face.cast::<IDWriteFontFace3>().ok()?;
    let names = unsafe { f3.GetFamilyNames() }.ok()?;
    let len = unsafe { names.GetStringLength(0) }.ok()?;
    let mut buf = vec![0u16; len as usize + 1];
    unsafe { names.GetString(0, &mut buf) }.ok()?;
    Some(FaceKey {
        family: String::from_utf16_lossy(&buf[..len as usize]),
        weight: unsafe { f3.GetWeight() }.0,
        stretch: unsafe { f3.GetStretch() }.0,
        style: unsafe { f3.GetStyle() }.0,
        simulations: unsafe { face.GetSimulations() }.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_latin_boundaries_are_detected() {
        assert_eq!(cjk_latin_gaps("中文"), Vec::<u32>::new());
        assert_eq!(cjk_latin_gaps("中文abc"), vec![2]);
        // 空格已经承担了分隔，不再额外加 0.1 em
        assert_eq!(cjk_latin_gaps("Rust 中文"), Vec::<u32>::new());
        assert_eq!(cjk_latin_gaps("中文Rust"), vec![2]);
        assert_eq!(cjk_latin_gaps("中A文B"), vec![1, 2, 3]);
        assert_eq!(cjk_latin_gaps("中文，标点"), Vec::<u32>::new());
        assert_eq!(cjk_latin_gaps("A中"), vec![1]);
    }
}
