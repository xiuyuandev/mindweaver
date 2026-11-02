//! SVG 导出：`Scene` → 矢量标记（M4 后半）。
//!
//! 这不是「把 `Frame` 丢给 GPU」能得到的：矢量导出要的是**可编辑的标记**——真实的
//! `<text>` 元素、原始贝塞尔曲线、主题令牌；渲染一张位图再包进 `<image>` 是骗人。
//! 所以这里直接遍历 `Scene`（与画布同源的布局结果），按四层顺序输出：
//! L1 边界/连线/概要括号 → L2 节点/标签胶囊/摘要盒 → L2 文字 →
//! L3 关系线端点/箭头/标签胶囊/折叠圆点/备注角标 → L3 文字。
//!
//! 文字位置来自 [`mw_render::text_jobs`]——和画布**同一份排版结果**（换行、基线、锚点
//! 都在那里定死），这里只负责选 `<text>` 而不是字形图集。字体族按 DWrite 的候选栈写进
//! `font-family`，查看器缺字体时会自行回退；这是矢量导出相对位图导出唯一无法保证逐像素
//! 一致的地方（字号、基线、锚点仍然一致）。
//!
//! 取舍：选中环/悬停/对齐线不进导出（与 PNG 同一条规矩）；折叠圆点与备注角标保留——
//! PNG 导出里也有，两张图必须对得上。透明底不画背景，也不画点阵。

use std::fmt::Write as _;

use mw_core::layout::{edge_path_with, toggle_pos, Edge};
use mw_core::theme::{Rgba, Shape, Theme};
use mw_core::{Path, PathSeg, Point, Rect, Scene, SceneEdge};
use mw_render::camera::Camera;
use mw_render::instances::{
    BADGE_OFFSET, BADGE_R, BOUNDARY_RADIUS, BOUNDARY_W, BRACE_W, CONNECTOR_W, LOD_NODE_COUNT,
    REL_LABEL_H, REL_LABEL_MIN_W, REL_LABEL_W, REL_START_FLOOR, REL_START_R, SUMMARY_DASH,
    SUMMARY_TOPIC_W, TOGGLE_BAR, TOGGLE_R, TOGGLE_W,
};
use mw_render::{text_jobs, Frame, TextAlign, TextJob, L_NODES, L_OVERLAY};

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// 与 PNG 的「透明背景」同一语义：不画底色，也不画点阵。
    pub transparent: bool,
    /// 取景框覆盖（「可视区域」导出）；`None` 时用内容包围盒。
    pub bounds: Option<mw_core::Rect>,
}

/// 整张图 → 一个自包含的 SVG 文档。坐标即世界坐标（`viewBox` 就是取景框），
/// 所以内部不需要任何平移/缩放变换。
pub fn render(
    scene: &Scene,
    theme: &Theme,
    assets: &mw_core::assets::Assets,
    opts: Options,
) -> String {
    let b = opts
        .bounds
        .unwrap_or_else(|| crate::export::content_bounds(scene));
    let w = b.w.max(1.0);
    let h = b.h.max(1.0);
    let cam = Camera::new(b.center(), 1.0, 1.0);

    let frame = Frame {
        scene,
        theme,
        camera: cam,
        selected: Vec::new(),
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport: None,
    };
    let jobs = text_jobs(&frame);
    let font = font_stack();
    // 点阵走主题的**唯一出口**（X21b）：与画布合成器读的是同一个 `CanvasDots`。
    let dots = theme.canvas_dots();

    let mut out = String::with_capacity(8192);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        &mut out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">",
        num(w),
        num(h),
        num(b.x),
        num(b.y),
        num(w),
        num(h)
    )
    .unwrap();

    // ---- 背景：点阵与画布同一套令牌（`Theme::canvas_dots()`，间距/半径/颜色三样都来自它）----
    //
    // **相位**：画布的格点落在**格子中心** `(k + 0.5) × spacing`——`lattice.wgsl` 写的是
    // `off = cell - floor(cell) - 0.5`，离格点最近处即「小数部分 = 0.5」。
    // 所以 pattern 从 (0,0) 铺起、圆心放在 `(spacing/2, spacing/2)`，点才和画布重合。
    // 半径同理走 `vector_radius()`（= 令牌半径，导出按 1 CSS px = 1 世界单位）。
    if !opts.transparent {
        if let Some(d) = dots {
            let half = d.spacing * 0.5;
            writeln!(
                &mut out,
                "<defs><pattern id=\"zdots\" width=\"{}\" height=\"{}\" patternUnits=\"userSpaceOnUse\">\
                 <circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}/></pattern></defs>",
                num(d.spacing),
                num(d.spacing),
                num(half),
                num(half),
                num(d.vector_radius()),
                fill_attr(d.color)
            )
            .unwrap();
        }
        rect_elt(
            &mut out,
            b,
            0.0,
            theme.ui.canvas_bg,
            Rgba::TRANSPARENT,
            0.0,
            None,
        );
        if dots.is_some() {
            writeln!(
                &mut out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"url(#zdots)\"/>",
                num(b.x),
                num(b.y),
                num(w),
                num(h)
            )
            .unwrap();
        }
    }

    // ---- L1：边界、轴线、父子连线、概要括号/短连接线、关系线曲线 ----
    out.push_str("<g id=\"lines\">\n");
    // 分组框（CM-18）：最底层，圆角 + 虚线 + 6% 淡底。
    for g in &scene.groups {
        rect_elt(
            &mut out,
            g.rect,
            mw_render::instances::GROUP_RADIUS,
            fade(g.color, 0.06),
            fade(g.color, 0.55),
            cam.css_to_world(mw_render::instances::GROUP_W),
            Some(mw_core::REL_DASH),
        );
    }
    for bnd in &scene.boundaries {
        rect_elt(
            &mut out,
            bnd.rect,
            BOUNDARY_RADIUS,
            fade(bnd.color, 0.07),
            fade(bnd.color, 0.5),
            cam.css_to_world(BOUNDARY_W),
            None,
        );
    }
    for s in &scene.spines {
        path_elt(&mut out, &s.path, s.color, cam.css_to_world(s.width));
    }
    for e in &scene.edges {
        // X24：形状取自 `SceneEdge.shape`（与画布同一个 `edge_path_with`），
        // 锥形宽度用两侧轮廓填充，箭头与画布同一枚三角形——三者都不再各算一份。
        let k = cam.css_to_world(1.0);
        polygon_path_elt(&mut out, &tree_edge_outline(e, k), e.color);
        if let Some(tri) = tree_edge_arrow(e, k) {
            polygon_elt(&mut out, &tri, e.color);
        }
    }
    for s in &scene.summaries {
        path_elt(&mut out, &s.brace, s.color, cam.css_to_world(BRACE_W));
        path_elt(
            &mut out,
            &s.connector,
            s.color,
            cam.css_to_world(CONNECTOR_W),
        );
    }
    for r in &scene.relationships {
        // CM-16：线宽与虚线都从 `Scene` 读，导出与画布同一份几何。
        path_elt_styled(
            &mut out,
            &r.path,
            r.color,
            cam.css_to_world(r.width),
            r.dashed.then_some(mw_core::REL_DASH),
        );
    }
    out.push_str("</g>\n");

    // ---- L2 几何：节点盒、标签胶囊、摘要主题盒（虚线） ----
    out.push_str("<g id=\"nodes\">\n");
    let labels_on = scene.nodes.len() < LOD_NODE_COUNT;
    for idx in &scene.draw_order {
        let n = scene.node(*idx);
        if n.style.shape == Shape::Underline {
            // 下划线主题：没有卡片，只有底边一条线（与画布同一个几何）。
            let y = n.geom.rect.bottom() - cam.css_to_world(1.0);
            line_elt(
                &mut out,
                Point::new(n.geom.rect.x, y),
                Point::new(n.geom.rect.right(), y),
                cam.css_to_world(n.style.stroke_width)
                    .max(cam.css_to_world(1.0)),
                n.style.stroke,
            );
        } else {
            rect_elt(
                &mut out,
                n.geom.rect,
                corner(&n.style, n.geom.rect),
                n.style.fill,
                n.style.stroke,
                cam.css_to_world(n.style.stroke_width),
                None,
            );
        }
        if let Some(bytes) = n
            .image
            .as_deref()
            .and_then(|v| v.strip_prefix("asset:"))
            .and_then(|h| assets.get(h))
        {
            let pad = cam.css_to_world(8.0);
            let inner_w = (n.geom.rect.w - pad * 2.0).max(1.0);
            let slot_h = (n.geom.image_h - pad * 2.0).max(1.0);
            let ratio = n.geom.image_ratio.max(0.05);
            let ih = (inner_w / ratio).min(slot_h);
            let img = Rect::new(
                n.geom.rect.x + pad,
                n.geom.rect.y + pad + (slot_h - ih) * 0.5,
                inner_w,
                ih,
            );
            out.push_str(&format!(
                "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid slice\" href=\"data:{};base64,{}\"/>\n",
                num(img.x),
                num(img.y),
                num(img.w),
                num(img.h),
                image_mime(bytes),
                base64(bytes)
            ));
        }
        if !labels_on {
            continue;
        }
        let origin = Point::new(n.geom.rect.x, n.geom.rect.y);
        for p in &n.labels {
            let world = offset(p.rect, origin);
            rect_elt(
                &mut out,
                world,
                world.h * 0.5,
                fade(n.style.branch_color, 0.92),
                Rgba::TRANSPARENT,
                0.0,
                None,
            );
        }
    }
    for s in &scene.summaries {
        rect_elt(
            &mut out,
            s.topic,
            s.topic.h * 0.5,
            fade(theme.ui.canvas_bg, 0.85),
            s.color,
            cam.css_to_world(SUMMARY_TOPIC_W),
            Some(SUMMARY_DASH),
        );
    }
    out.push_str("</g>\n");

    // ---- L2 文字：节点各行、图标、标签胶囊、摘要 ----
    out.push_str("<g id=\"text\">\n");
    for job in jobs.iter().filter(|j| j.layer == L_NODES) {
        text_elt(&mut out, job, &font);
    }
    out.push_str("</g>\n");

    // ---- L3 几何：关系线标签胶囊/起点圆点/箭头、折叠圆点、备注与链接角标 ----
    out.push_str("<g id=\"overlay\">\n");
    for r in &scene.relationships {
        if r.label.is_some() {
            let box_h = REL_LABEL_H;
            let pill_w = r.label_w.max(REL_LABEL_MIN_W);
            let pill = Rect::new(
                r.label_at.x - pill_w * 0.5,
                r.label_at.y - box_h * 0.5,
                pill_w,
                box_h,
            );
            rect_elt(
                &mut out,
                pill,
                box_h * 0.5,
                theme.ui.canvas_bg,
                r.color,
                cam.css_to_world(REL_LABEL_W),
                None,
            );
        }
        circle_elt(
            &mut out,
            r.start,
            cam.chrome(REL_START_R, REL_START_FLOOR),
            r.color,
            Rgba::TRANSPARENT,
            0.0,
        );
        for tri in [r.arrow, r.arrow_back].into_iter().flatten() {
            // 导出固定 100% 口径：与 zoom=1.0 的画布逐点一致，尺寸不随视口变。
            let tri = mw_core::geom::arrow_at_zoom(tri, 1.0);
            polygon_elt(&mut out, &tri, r.color);
        }
    }
    for idx in &scene.draw_order {
        let n = scene.node(*idx);
        let Some(p) = n.parent else { continue };
        if !n.has_children {
            continue;
        }
        let local = toggle_pos(n.geom.rect, scene.kind, Some(scene.node(p).geom.rect.x));
        let c = Point::new(n.geom.rect.x + local.x, n.geom.rect.y + local.y);
        let r = cam.chrome(TOGGLE_R, 4.5);
        let w = cam.css_to_world(TOGGLE_W);
        circle_elt(&mut out, c, r, theme.ui.canvas_bg, n.style.branch_color, w);
        let bar = cam.css_to_world(TOGGLE_BAR);
        line_elt(
            &mut out,
            Point::new(c.x - bar, c.y),
            Point::new(c.x + bar, c.y),
            w,
            n.style.branch_color,
        );
        if n.collapsed {
            line_elt(
                &mut out,
                Point::new(c.x, c.y - bar),
                Point::new(c.x, c.y + bar),
                w,
                n.style.branch_color,
            );
        }
    }
    if scene.nodes.len() < LOD_NODE_COUNT {
        for idx in &scene.draw_order {
            let n = scene.node(*idx);
            let r = cam.chrome(BADGE_R, 2.4);
            let dx = n.geom.rect.right() - cam.chrome(BADGE_OFFSET, 3.0);
            let dy = n.geom.rect.y + cam.chrome(BADGE_OFFSET, 3.0);
            if n.has_note {
                circle_elt(
                    &mut out,
                    Point::new(dx, dy),
                    r,
                    fade(n.style.branch_color, 0.85),
                    Rgba::TRANSPARENT,
                    0.0,
                );
            }
            if n.has_link {
                circle_elt(
                    &mut out,
                    Point::new(dx - r * 3.0, dy),
                    r,
                    fade(n.style.branch_color, 0.6),
                    Rgba::TRANSPARENT,
                    0.0,
                );
            }
        }
    }
    out.push_str("</g>\n");

    // ---- L3 文字：关系线标签 ----
    for job in jobs.iter().filter(|j| j.layer == L_OVERLAY) {
        text_elt(&mut out, job, &font);
    }

    out.push_str("</svg>\n");
    out
}

fn corner(style: &mw_core::ResolvedStyle, r: Rect) -> f32 {
    // 单一出口：形状→半径只在 `mw_render::corner_style` 里定义一次（含椭圆哨兵与半径覆盖）。
    mw_render::corner_style(style, r)
}

fn offset(local: Rect, origin: Point) -> Rect {
    Rect::new(local.x + origin.x, local.y + origin.y, local.w, local.h)
}

fn fade(c: Rgba, k: f32) -> Rgba {
    Rgba::rgba(c.0[0], c.0[1], c.0[2], (c.0[3] as f32 * k).round() as u8)
}

/// 渲染层的颜色是 0–1 的 sRGB 浮点；写标记前换回字节，避免 `0.92` 这类值在
/// 各查看器里出现不同的取整结果。
fn from_f32(c: [f32; 4]) -> Rgba {
    let b = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba::rgba(b(c[0]), b(c[1]), b(c[2]), b(c[3]))
}

/// 与 DWrite 的候选栈同序：查看器里有哪个用哪个。族名里的空格在 CSS 里可以不引号。
fn font_stack() -> String {
    let mut s = String::new();
    for f in mw_render::FONT_CANDIDATES {
        if !s.is_empty() {
            s.push_str(", ");
        }
        s.push_str(f);
    }
    s.push_str(", sans-serif");
    s
}

/// 两三位小数就够：世界单位下再多位只是噪声，还会把文件撑大。
fn num(v: f32) -> String {
    let mut s = format!("{v:.2}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0[0], c.0[1], c.0[2])
}

fn opacity_attr(name: &str, a: u8) -> String {
    if a == 255 {
        String::new()
    } else {
        format!(" {name}-opacity=\"{}\"", num(a as f32 / 255.0))
    }
}

fn fill_attr(c: Rgba) -> String {
    if c.0[3] == 0 {
        return " fill=\"none\"".to_string();
    }
    format!(" fill=\"{}\"{}", hex(c), opacity_attr("fill", c.0[3]))
}

fn stroke_attr(c: Rgba, w: f32, dash: Option<[f32; 2]>) -> String {
    if c.0[3] == 0 || w <= 0.0 {
        return String::new();
    }
    let mut s = format!(
        " stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
        hex(c),
        num(w)
    );
    s.push_str(&opacity_attr("stroke", c.0[3]));
    if let Some([on, off]) = dash {
        s.push_str(&format!(" stroke-dasharray=\"{} {}\"", num(on), num(off)));
    }
    s
}

fn image_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

/// 标准 Base64（带 `=` 填充）。只给 SVG 内嵌 data URI 用，不做 URL-safe。
fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn rect_elt(
    out: &mut String,
    r: Rect,
    radius: f32,
    fill: Rgba,
    stroke: Rgba,
    stroke_w: f32,
    dash: Option<[f32; 2]>,
) {
    if radius <= mw_render::ELLIPSE_CODE + 0.5 {
        // 椭圆（概念图节点）：SVG 原生 `<ellipse>`，半轴就是盒半轴——
        // 与 wgsl 的 `sd_ellipse`、PDF 的四段贝塞尔是同一条边界。
        write!(
            out,
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"",
            num(r.center().x),
            num(r.center().y),
            num(r.w.max(0.0) * 0.5),
            num(r.h.max(0.0) * 0.5)
        )
        .unwrap();
        out.push_str(&fill_attr(fill));
        out.push_str(&stroke_attr(stroke, stroke_w, dash));
        out.push_str("/>\n");
        return;
    }
    write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
        num(r.x),
        num(r.y),
        num(r.w.max(0.0)),
        num(r.h.max(0.0))
    )
    .unwrap();
    let rx = radius.clamp(0.0, r.w.min(r.h) * 0.5);
    if rx > 0.01 {
        write!(out, " rx=\"{}\" ry=\"{}\"", num(rx), num(rx)).unwrap();
    }
    out.push_str(&fill_attr(fill));
    out.push_str(&stroke_attr(stroke, stroke_w, dash));
    out.push_str("/>\n");
}

fn circle_elt(out: &mut String, c: Point, r: f32, fill: Rgba, stroke: Rgba, stroke_w: f32) {
    writeln!(
        out,
        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}{}/>",
        num(c.x),
        num(c.y),
        num(r.max(0.0)),
        fill_attr(fill),
        stroke_attr(stroke, stroke_w, None)
    )
    .unwrap();
}

fn line_elt(out: &mut String, a: Point, b: Point, w: f32, color: Rgba) {
    writeln!(
        out,
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" fill=\"none\"{}/>",
        num(a.x),
        num(a.y),
        num(b.x),
        num(b.y),
        stroke_attr(color, w, None)
    )
    .unwrap();
}

fn polygon_elt(out: &mut String, pts: &[Point; 3], color: Rgba) {
    let points = pts
        .iter()
        .map(|p| format!("{},{}", num(p.x), num(p.y)))
        .collect::<Vec<_>>()
        .join(" ");
    writeln!(out, "<polygon points=\"{points}\"{}/>", fill_attr(color)).unwrap();
}

fn path_elt(out: &mut String, path: &Path, color: Rgba, w: f32) {
    path_elt_styled(out, path, color, w, None);
}

/// 带线型的路径：`dash = Some([实长, 空长])` 时写 `stroke-dasharray`（CM-16）。
fn path_elt_styled(out: &mut String, path: &Path, color: Rgba, w: f32, dash: Option<[f32; 2]>) {
    writeln!(
        out,
        "<path d=\"{}\" fill=\"none\"{}/>",
        path_d(path),
        stroke_attr(color, w, dash)
    )
    .unwrap();
}

fn path_d(p: &Path) -> String {
    let mut d = format!("M {} {}", num(p.start.x), num(p.start.y));
    for seg in &p.segs {
        match *seg {
            PathSeg::Line { to } => {
                write!(&mut d, " L {} {}", num(to.x), num(to.y)).unwrap();
            }
            PathSeg::Quad { ctrl, to } => {
                write!(
                    &mut d,
                    " Q {} {} {} {}",
                    num(ctrl.x),
                    num(ctrl.y),
                    num(to.x),
                    num(to.y)
                )
                .unwrap();
            }
            PathSeg::Cubic { c1, c2, to } => {
                write!(
                    &mut d,
                    " C {} {} {} {} {} {}",
                    num(c1.x),
                    num(c1.y),
                    num(c2.x),
                    num(c2.y),
                    num(to.x),
                    num(to.y)
                )
                .unwrap();
            }
        }
    }
    d
}

/// 闭合多边形轮廓（锥形分支线）：**填充**而不是描边，顶点就是世界坐标。
///
/// 为什么不能描边：锥形要求一条线从父端到子端连续变宽，而 SVG/PDF 的
/// `stroke-width` 是整段一个值。把两侧轮廓算出来填成一条闭合路径，
/// 宽度就沿弧长真的变了（与画布 `push_curve` 同一套插值）。
fn polygon_path_elt(out: &mut String, pts: &[Point], color: Rgba) {
    if pts.len() < 3 {
        return;
    }
    let mut d = format!("M {} {}", num(pts[0].x), num(pts[0].y));
    for p in &pts[1..] {
        write!(&mut d, " L {} {}", num(p.x), num(p.y)).unwrap();
    }
    d.push_str(" Z");
    writeln!(
        out,
        "<path d=\"{}\"{} fill-rule=\"nonzero\"/>",
        d,
        fill_attr(color)
    )
    .unwrap();
}

// ---- 父子连线（分支线）的导出几何 ----------------------------------------
//
// 这是 X24「导出同源」的收口点：**导出侧只有这一份树边几何**，SVG 与 PDF 都走它。
// 以前两个导出器各自调 `edge_path(…)`（默认 `Curve`）再描一条等宽线，于是
// ① `elbow` / `straight` 在导出里被画成曲线；② 锥形宽度整个丢掉；
// ③ SVG 连箭头都没有、PDF 的箭头是另一套 9×6.5 世界单位的三角形。
//
// 控制点公式仍然只有 `mw_core::layout::edge_path_with` 一份；`SceneEdge` 不带
// `path`，也没有为本轮新增任何字段（`docs/32` §12 没有批准过）。

/// 树边的路径：形状由 `SceneEdge.shape` 决定（与画布同一函数、同一份控制点）。
pub(crate) fn tree_edge_path(e: &SceneEdge) -> Path {
    edge_path_with(
        &Edge {
            from: e.from,
            to: e.to,
            start: e.start,
            end: e.end,
            orientation: e.orientation,
        },
        e.shape,
    )
}

/// 轮廓扁平化容差（世界单位）。导出按 100% 写字，0.15 已远小于半像素；
/// 画布那份是 `0.3 / (zoom × ppp)`（`mw_render::flatten::tolerance`）。
const TAPER_TOL: f32 = 0.15;
/// 端点圆头的采样步数（画布的分支线是一串胶囊，两头是圆的；导出要看起来一样）。
const CAP_STEPS: usize = 8;
/// 画布那枚箭头的两个常量，逐字对应 `mw_render::instances`：
/// `let size = cam.css_to_world(7.0);` 与 `let half = size * 0.42;`。
pub(crate) const TREE_ARROW_CSS_LEN: f32 = 7.0;
pub(crate) const TREE_ARROW_HALF: f32 = 0.42;

/// 树边的**锥形填充轮廓**（世界坐标，闭合多边形）。
///
/// `world_per_css` 只在这里出现一次：把 CSS 宽度换成世界宽度，口径与描边时代
/// 的 `cam.css_to_world(e.width)` 完全相同（SVG 传 `cam.css_to_world(1.0)`，
/// PDF 的 CTM 已经带过 0.75，传 `1.0`）。
pub(crate) fn tree_edge_outline(e: &SceneEdge, world_per_css: f32) -> Vec<Point> {
    let pts = mw_render::flatten::flatten(&tree_edge_path(e), TAPER_TOL);
    edge_outline(&pts, world_per_css * e.width, world_per_css * e.width_end)
}

/// 折线 + 两端宽度 → 闭合轮廓：左轮廓 → 末端圆头 → 右轮廓反向 → 起端圆头。
pub(crate) fn edge_outline(pts: &[Point], w0: f32, w1: f32) -> Vec<Point> {
    if pts.len() < 2 || w0 <= 0.0 || w1 <= 0.0 {
        return Vec::new();
    }
    let (left, right) = edge_outline_sides(pts, w0, w1);
    let n = left.len();
    let mut out = Vec::with_capacity(2 * n + 2 * CAP_STEPS);
    out.extend_from_slice(&left);
    // 末端圆头：left[n-1] →（绕过 end + u·w1/2）→ right[n-1]
    out.extend(cap_points(pts[n - 1], pts[n - 2], w1 * 0.5));
    out.extend(right.iter().rev().copied());
    // 起端圆头：right[0] →（绕过 start − u·w0/2）→ left[0]，所以顺序要反过来
    let mut cap = cap_points(pts[0], pts[1], w0 * 0.5);
    cap.reverse();
    out.extend(cap);
    out
}

/// 左右两条轮廓（不含圆头）。中轴线就是 `pts`——测试靠它反推几何是否同源。
pub(crate) fn edge_outline_sides(pts: &[Point], w0: f32, w1: f32) -> (Vec<Point>, Vec<Point>) {
    let mut cum = Vec::with_capacity(pts.len());
    let mut total = 0.0f32;
    cum.push(0.0);
    for w in pts.windows(2) {
        total += w[0].dist(w[1]);
        cum.push(total);
    }
    let total = total.max(1e-6);
    let mut left = Vec::with_capacity(pts.len());
    let mut right = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        // 与画布 `push_curve` 同一套弧长口径：曲线细分后段长不等，按序号会让锥度在弯道变陡。
        let t = (cum[i] / total).clamp(0.0, 1.0);
        let half = 0.5 * (w0 + (w1 - w0) * t);
        let (nx, ny) = unit_normal(pts, i);
        left.push(Point::new(p.x + nx * half, p.y + ny * half));
        right.push(Point::new(p.x - nx * half, p.y - ny * half));
    }
    (left, right)
}

/// 第 `i` 个点的法线（已按 miter 拉长）。转角处若直接用平均法线，垂直厚度会缩到
/// `half·cos(θ/2)`——90° 直角的分支线会在拐角瘦掉三成；除以 `cos` 补回来。
fn unit_normal(pts: &[Point], i: usize) -> (f32, f32) {
    let mut sum = (0.0f32, 0.0f32);
    let mut first: Option<(f32, f32)> = None;
    if i > 0 {
        if let Some(n) = seg_normal(pts[i - 1], pts[i]) {
            sum.0 += n.0;
            sum.1 += n.1;
            first = Some(n);
        }
    }
    if i + 1 < pts.len() {
        if let Some(n) = seg_normal(pts[i], pts[i + 1]) {
            sum.0 += n.0;
            sum.1 += n.1;
            if first.is_none() {
                first = Some(n);
            }
        }
    }
    let len = sum.0.hypot(sum.1);
    if len <= 1e-6 {
        return first.unwrap_or((0.0, 1.0));
    }
    let n = (sum.0 / len, sum.1 / len);
    let miter = match first {
        Some(f) => (1.0 / (n.0 * f.0 + n.1 * f.1)).clamp(1.0, 2.0),
        None => 1.0,
    };
    (n.0 * miter, n.1 * miter)
}

/// 线段的左法线（`(x,y)` 方向逆时针 90°）。
fn seg_normal(a: Point, b: Point) -> Option<(f32, f32)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l = dx.hypot(dy);
    if l <= 1e-6 {
        None
    } else {
        Some((-dy / l, dx / l))
    }
}

/// 端点圆头的采样点：从 `+法线` 侧绕「外端」半圈到 `-法线` 侧（不含两个端点本身）。
fn cap_points(center: Point, neighbor: Point, r: f32) -> Vec<Point> {
    let (dx, dy) = (center.x - neighbor.x, center.y - neighbor.y);
    let l = dx.hypot(dy);
    if l <= 1e-6 || r <= 0.0 {
        return Vec::new();
    }
    let (ux, uy) = (dx / l, dy / l); // 外向（由邻点指向端点）
    let (nx, ny) = (-uy, ux); // 与左轮廓同侧的法线
    let mut out = Vec::with_capacity(CAP_STEPS - 1);
    for k in 1..CAP_STEPS {
        let phi = std::f32::consts::PI * (k as f32) / (CAP_STEPS as f32);
        let (c, s) = (phi.cos(), phi.sin());
        out.push(Point::new(
            center.x + r * (c * nx + s * ux),
            center.y + r * (c * ny + s * uy),
        ));
    }
    out
}

/// 树边箭头三角形（世界坐标）：与画布 `instances.rs` 里那段逐字同款——
/// 尖端锚在终点、长度 `7 css px`、半宽 `0.42 × 长度`。
///
/// 导出固定按 100% 写：`world_per_css` 为 1 时三角形就是 7 长 × 5.88 宽，
/// 与画布在 100% 下的屏幕尺寸一致（这就是「屏幕恒定」在矢量文件里的等价物）。
pub(crate) fn tree_edge_arrow(e: &SceneEdge, world_per_css: f32) -> Option<[Point; 3]> {
    if !e.arrow {
        return None;
    }
    let (dx, dy) = (e.end.x - e.start.x, e.end.y - e.start.y);
    let len = dx.hypot(dy);
    if len <= 0.01 {
        return None;
    }
    let size = world_per_css * TREE_ARROW_CSS_LEN;
    let (ux, uy) = (dx / len, dy / len);
    let base = Point::new(e.end.x - ux * size, e.end.y - uy * size);
    let half = size * TREE_ARROW_HALF;
    let (px, py) = (-uy * half, ux * half);
    Some([
        e.end,
        Point::new(base.x + px, base.y + py),
        Point::new(base.x - px, base.y - py),
    ])
}

fn text_elt(out: &mut String, job: &TextJob, font: &str) {
    // 字重与对齐都从 `TextJob` 来（SE-13）：SVG 与屏幕共用同一份排版结果，
    // 所以 `font-weight` 写真实字重、`text-anchor` 跟对齐走，不在导出侧另算一遍。
    let (x, anchor) = match job.align {
        TextAlign::Left(x) => (x, "start"),
        TextAlign::Center(x) => (x, "middle"),
        TextAlign::Right(x) => (x, "end"),
    };
    write!(
        out,
        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\"",
        num(x),
        num(job.baseline),
        font,
        num(job.font_size)
    )
    .unwrap();
    if job.weight != mw_core::theme::WEIGHT_REGULAR {
        write!(out, " font-weight=\"{}\"", job.weight).unwrap();
    }
    out.push_str(&fill_attr(from_f32(job.color)));
    writeln!(out, " text-anchor=\"{anchor}\">{}</text>", escape(job.text)).unwrap();
}

/// XML 转义。控制字符（除制表/换行）在 XML 1.0 里非法，宁可丢掉也不能让文档坏掉。
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::{
        build_scene, get_theme, new_id, Document, LayoutSettings, MonoMeasurer, Node, Position,
        Relationship,
    };

    fn doc() -> Document {
        let mut d = Document::new("根");
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().text = "根".into();
        for t in ["甲", "乙"] {
            let id = new_id();
            let n = Node::new(id.clone(), Some(root.clone()), t.to_string());
            d.nodes.insert(id.clone(), n);
            d.nodes.get_mut(&root).unwrap().children.push(id);
        }
        d
    }

    fn scene(d: &Document, theme: &Theme) -> Scene {
        build_scene(
            d,
            &d.root_id.clone(),
            &LayoutSettings::default(),
            theme,
            &MonoMeasurer,
            None,
        )
    }

    fn svg(d: &Document, theme: &Theme, transparent: bool) -> String {
        let assets = d.assets.clone();
        render(
            &scene(d, theme),
            theme,
            &assets,
            Options {
                transparent,
                bounds: None,
            },
        )
    }

    /// ⑥ 导出补齐：聚焦问题要进导出图，而且只往**上**长——既有几何一格不动。
    #[test]
    fn the_focus_question_is_exported_above_the_content() {
        let theme = get_theme("snow");
        let d = doc();
        let mut sc = scene(&d, &theme);
        let before = sc.bounds;
        let rects: Vec<_> = sc.nodes.iter().map(|n| n.geom.rect).collect();
        sc.attach_focus_note("这张图在回答什么？", 16.0, theme.ui.text);
        let note = sc.focus_note.clone().expect("带子要挂上");
        assert!(sc.bounds.y < before.y, "bounds 要向上扩");
        assert!(
            (sc.bounds.h - (before.h + note.rect.h)).abs() < 0.01,
            "高度只多一条带子"
        );
        assert!(
            (note.rect.y + note.rect.h - before.y).abs() < 0.01,
            "带子贴在内容顶上"
        );
        for (i, n) in sc.nodes.iter().enumerate() {
            assert_eq!(n.geom.rect, rects[i], "既有几何不许被带子挤动");
        }
        let svg = render(
            &sc,
            &theme,
            &d.assets,
            Options {
                transparent: true,
                bounds: None,
            },
        );
        assert!(
            svg.contains("这张图在回答什么？"),
            "导出的 SVG 里要能搜到问题"
        );
    }
    #[test]
    fn document_is_well_formed_and_contains_every_line() {
        let s = svg(&doc(), &get_theme("snow"), false);
        assert!(s.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg "));
        assert!(s.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(s.contains("viewBox=\""));
        assert!(s.trim_end().ends_with("</svg>"));
        for t in ["根", "甲", "乙"] {
            assert!(s.contains(&format!(">{t}</text>")), "缺文字：{t}");
        }
        assert_eq!(s.matches("<text").count(), s.matches("</text>").count());
    }

    /// BUG-5：SVG 导出的箭头必须走 `zoom = 1.0` 的共用换算。
    ///
    /// 用概念图（`straight_between` 的 6×4 世界三角）而不是思维导图的 9×4.2：
    /// 只有 6 < `ARROW_MIN_CSS_LEN` 时 `arrow_at_zoom` 才会真的放大到 7，这条测试
    /// 才咬得住「导出调用共用函数」——改前导出直接 `polygon_elt(&tri)`，会比屏幕短一截。
    #[test]
    fn exported_arrow_matches_arrow_at_zoom_one() {
        use mw_core::{Command, Position};
        let theme = get_theme("snow");
        let mut d = Document::new("概念图");
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().text = "用户痛点".into();
        d.nodes.get_mut(&root).unwrap().pos = Some(Position { x: 0.0, y: 0.0 });
        let id = new_id();
        let mut child = Node::new(id.clone(), None, "增长策略");
        child.pos = Some(Position { x: 260.0, y: 0.0 });
        d.nodes.insert(id.clone(), child);
        d.nodes
            .get_mut(&root)
            .unwrap()
            .relationships
            .push(Relationship {
                id: "rel".into(),
                to: id,
                label: Some("转化".into()),
                color: None,
                head: Default::default(),
                ..Default::default()
            });
        let sc = build_scene(
            &d,
            &d.root_id.clone(),
            &d.meta.layout,
            &theme,
            &MonoMeasurer,
            None,
        );
        let rel = sc.relationships.first().expect("有一条关系线");
        let tri = rel.arrow.expect("默认前向箭头");
        let raw_len = tri[0].dist(Point::new(
            (tri[1].x + tri[2].x) * 0.5,
            (tri[1].y + tri[2].y) * 0.5,
        ));
        assert!((raw_len - 6.0).abs() < 1e-3, "Scene 基准是 6 世界单位");
        let expected = mw_core::geom::arrow_at_zoom(tri, 1.0);
        let exp_len = expected[0].dist(Point::new(
            (expected[1].x + expected[2].x) * 0.5,
            (expected[1].y + expected[2].y) * 0.5,
        ));
        assert!(
            (exp_len - mw_core::ARROW_MIN_CSS_LEN).abs() < 1e-3,
            "zoom=1 时 6 世界单位的箭头要撑到 {} css px，实际 {exp_len}",
            mw_core::ARROW_MIN_CSS_LEN
        );
        let points = expected
            .iter()
            .map(|p| format!("{},{}", num(p.x), num(p.y)))
            .collect::<Vec<_>>()
            .join(" ");
        let out = render(
            &sc,
            &theme,
            &d.assets,
            Options {
                transparent: true,
                bounds: None,
            },
        );
        assert!(
            out.contains(&format!("<polygon points=\"{points}\"")),
            "导出的箭头必须是 zoom=1 的共用三角：{points}"
        );
    }

    /// CM-07：概念图不需要第二条导出路径——`Scene` 里已经有概念节点与带短语的连线，
    /// 所以 SVG（以及同吃 `Scene` 的 PNG/PDF）自动带上它们。这条测试守的就是「零新增」：
    /// 哪天有人给概念图单开一条导出分支，这里会因为重复文字/漏字先红。
    #[test]
    fn concept_map_exports_through_the_same_scene() {
        use mw_core::{Command, Position};
        let mut d = Document::new("概念图");
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().text = "用户痛点".into();
        d.nodes.get_mut(&root).unwrap().pos = Some(Position { x: 0.0, y: 0.0 });
        let id = new_id();
        let mut child = Node::new(id.clone(), None, "增长策略");
        child.pos = Some(Position { x: 260.0, y: 0.0 });
        d.nodes.insert(id.clone(), child);
        d.nodes
            .get_mut(&root)
            .unwrap()
            .relationships
            .push(Relationship {
                id: new_id(),
                to: id,
                label: Some("转化".into()),
                color: None,
                head: Default::default(),
                ..Default::default()
            });

        let s = svg(&d, &get_theme("snow"), false);
        assert!(s.contains(">用户痛点</text>"), "概念文字要进 SVG");
        assert!(s.contains(">增长策略</text>"), "自由概念也要进 SVG");
        assert!(s.contains(">转化</text>"), "链接短语要进 SVG");
    }

    #[test]
    fn a_dashed_relationship_exports_a_dash_array() {
        let mut d = doc();
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().pos = Some(Position { x: 0.0, y: 0.0 });
        let id = new_id();
        let mut child = Node::new(id.clone(), None, "增长策略");
        child.pos = Some(Position { x: 260.0, y: 0.0 });
        d.nodes.insert(id.clone(), child);
        d.nodes
            .get_mut(&root)
            .unwrap()
            .relationships
            .push(Relationship {
                id: new_id(),
                to: id,
                label: Some("转化".into()),
                dashed: true,
                width: Some(2.5),
                ..Default::default()
            });
        let s = svg(&d, &get_theme("snow"), false);
        // CM-16：线型与线宽都从 `Scene` 读，导出与画布同源。
        assert!(
            s.contains(r#"stroke-dasharray="6 4""#),
            "虚线要写进导出：{s}"
        );
    }

    /// SE-13：三个新样式键必须原样落到 SVG 上——这是「导出与屏幕同源」的下半句。
    /// 字重走 `font-weight`、对齐走 `text-anchor`、圆角走 `rx`、椭圆走 `<ellipse>`。
    #[test]
    fn the_new_style_keys_reach_the_svg() {
        use mw_core::model::StyleOverride;
        let theme = get_theme("snow");
        let mut d = doc();
        let id = d.nodes[&d.root_id].children[0].clone();
        let style = StyleOverride {
            weight: Some(mw_core::theme::WEIGHT_SEMIBOLD),
            align: Some("right".into()),
            radius: Some(14.0),
            ..Default::default()
        };
        d.apply(&mw_core::Command::SetStyle {
            ids: vec![id.clone()],
            style: Some(style),
        })
        .unwrap();
        let out = svg(&d, &theme, true);
        assert!(out.contains("font-weight=\"600\""), "字重没落盘");
        assert!(out.contains("text-anchor=\"end\""), "右对齐没落盘");
        assert!(out.contains("rx=\"14\""), "圆角没落盘");

        // 形状换成椭圆：不再是带 rx 的 <rect>，而是 <ellipse>。
        d.apply(&mw_core::Command::SetStyle {
            ids: vec![id],
            style: Some(StyleOverride {
                shape: Some("ellipse".into()),
                ..Default::default()
            }),
        })
        .unwrap();
        let out = svg(&d, &theme, true);
        assert!(out.contains("<ellipse "), "椭圆没走 ellipse 元素：{out}");
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        let mut d = doc();
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().text = "A & B <C> \"D\"".into();
        let s = svg(&d, &get_theme("snow"), false);
        assert!(s.contains("A &amp; B &lt;C&gt; &quot;D&quot;"));
        assert!(!s.contains("A & B <C>"));
    }

    #[test]
    fn transparent_background_omits_the_fill_and_the_dot_lattice() {
        let theme = get_theme("ink");
        assert!(theme.ui.dot.0[3] > 0, "墨夜应有点阵，测试才有意义");
        let opaque = svg(&doc(), &theme, false);
        assert!(opaque.contains("<pattern id=\"zdots\""));
        assert!(opaque.contains("fill=\"url(#zdots)\""));
        assert!(opaque.contains("fill=\"#17191c\""), "应有画布底色");

        let clear = svg(&doc(), &theme, true);
        assert!(!clear.contains("<pattern"));
        assert!(!clear.contains("url(#zdots)"));
        assert!(
            !clear.contains("fill=\"#17191c\""),
            "透明底不该再画画布底色"
        );
    }

    #[test]
    fn transparent_svg_omits_the_canvas_pattern() {
        let s = svg(&doc(), &get_theme("snow"), true);
        assert!(!s.contains("id=\"zdots\""));
        assert!(!s.contains("patternUnits"));
    }

    #[test]
    fn image_assets_are_embedded_as_real_svg_images() {
        let mut d = doc();
        let png = vec![
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0,
            0, 0, 10, 0, 0, 0, 10,
        ];
        let key = d.assets.insert(png);
        let root = d.root_id.clone();
        d.apply(&mw_core::Command::SetImage {
            id: root,
            image: Some(format!("asset:{key}")),
        })
        .unwrap();
        let theme = get_theme("snow");
        let sc = scene(&d, &theme);
        let s = render(
            &sc,
            &theme,
            &d.assets,
            Options {
                transparent: true,
                bounds: None,
            },
        );
        assert!(s.contains("<image "), "应嵌入真实 <image>：{s}");
        assert!(s.contains("data:image/png;base64,"));
        assert!(s.contains("preserveAspectRatio=\"xMidYMid slice\""));
    }

    #[test]
    fn node_cards_use_the_scene_rects_verbatim() {
        let d = doc();
        let theme = get_theme("snow");
        let sc = scene(&d, &theme);
        let s = render(
            &sc,
            &theme,
            &mw_core::assets::Assets::new(),
            Options::default(),
        );
        for n in &sc.nodes {
            let expected = format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                num(n.geom.rect.x),
                num(n.geom.rect.y),
                num(n.geom.rect.w),
                num(n.geom.rect.h)
            );
            assert!(s.contains(&expected), "节点盒坐标没对齐场景：{expected}");
        }
    }

    #[test]
    fn relationship_becomes_a_bezier_with_arrow_dot_and_label() {
        let mut d = doc();
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
        let theme = get_theme("snow");
        let s = svg(&d, &theme, false);
        assert!(s.contains("<path d=\"M "), "关系线应是路径");
        assert!(s.contains(" Q "), "关系线应是二次曲线");
        assert!(s.contains("<polygon points="), "箭头应是实心多边形");
        assert!(s.contains("<circle "), "起点应是一个圆点");
        assert!(s.contains(">关联</text>"), "标签文字必须出现");
    }

    #[test]
    fn the_arrow_head_choice_reaches_the_svg_in_both_directions() {
        // ⑦ 的导出不变量：屏幕上画几个三角形，SVG 里就得有几个 <polygon>。
        // 用差量断言而不是绝对个数——将来别的图元也用 polygon 时不至于误报。
        let mut d = doc();
        let root = d.root_id.clone();
        let second = d.nodes[&root].children[1].clone();
        d.nodes
            .get_mut(&root)
            .unwrap()
            .relationships
            .push(Relationship {
                id: "rel".into(),
                to: second,
                label: None,
                color: None,
                head: mw_core::model::ArrowHead::Forward,
                ..Default::default()
            });
        let theme = get_theme("snow");
        let count =
            |d: &mw_core::Document| svg(d, &theme, false).matches("<polygon points=").count();
        let forward = count(&d);
        d.nodes.get_mut(&root).unwrap().relationships[0].head = mw_core::model::ArrowHead::Both;
        assert_eq!(count(&d), forward + 1, "双向要两个箭头");
        d.nodes.get_mut(&root).unwrap().relationships[0].head = mw_core::model::ArrowHead::Off;
        assert_eq!(count(&d), forward - 1, "无箭头就一个都不画");
        d.nodes.get_mut(&root).unwrap().relationships[0].head = mw_core::model::ArrowHead::Backward;
        assert_eq!(count(&d), forward, "反向换边不换数量");
    }

    #[test]
    fn path_d_serializes_every_segment_kind() {
        let mut p = Path::new(Point::new(1.0, 2.0));
        p.line(Point::new(3.0, 4.0));
        p.quad(Point::new(5.0, 6.0), Point::new(7.0, 8.0));
        p.cubic(
            Point::new(9.0, 10.0),
            Point::new(11.0, 12.0),
            Point::new(13.0, 14.0),
        );
        let d = path_d(&p);
        assert_eq!(d, "M 1 2 L 3 4 Q 5 6 7 8 C 9 10 11 12 13 14");
    }

    #[test]
    fn axis_spines_are_exported_as_plain_lines() {
        let d = doc();
        let theme = get_theme("snow");
        let settings = LayoutSettings {
            kind: "fishbone".into(),
            ..LayoutSettings::default()
        };
        let sc = build_scene(
            &d,
            &d.root_id.clone(),
            &settings,
            &theme,
            &MonoMeasurer,
            None,
        );
        assert_eq!(sc.spines.len(), 1);
        let s = render(
            &sc,
            &theme,
            &mw_core::assets::Assets::new(),
            Options::default(),
        );
        let spine = path_d(&sc.spines[0].path);
        assert!(s.contains(&spine), "脊线路径应在 SVG 里：{spine}");
        assert!(s.contains(&format!("stroke=\"{}\"", hex(sc.spines[0].color))));
    }

    #[test]
    fn labels_become_chips_before_the_text_pass() {
        let mut d = doc();
        let root = d.root_id.clone();
        let first = d.nodes[&root].children[0].clone();
        d.nodes.get_mut(&first).unwrap().labels.push("标签".into());
        let theme = get_theme("snow");
        let s = svg(&d, &theme, false);
        let chip_at = s.find("<g id=\"nodes\">").unwrap();
        let text_at = s.find("<g id=\"text\">").unwrap();
        // 只在「节点层」里找：树边的锥形填充也用同一个 RGB（X24），全文件搜会先撞到连线。
        let fill_at = s[chip_at..]
            .find(&format!("fill=\"{}\"", hex(fade(theme.palette[0], 0.92))))
            .map(|i| chip_at + i)
            .expect("标签胶囊的填充色应在节点层里");
        assert!(chip_at < fill_at && fill_at < text_at);
        assert!(s.contains(">标签</text>"));
    }

    // ---- X24：导出同源（G-01…G-07） ------------------------------------
    //
    // 六条差距的机器判据都在这里：形状走 `edge_path_with`、锥形走两侧轮廓、
    // 箭头与画布同一枚三角形、SVG/PDF 两个导出器共用这一份几何。
    // 判据是**关键几何数值**（控制点、两端宽度、三角形顶点），不是字节比对。

    use mw_core::layout::{EdgeShape, Orientation};

    /// 一条只用来量几何的边：颜色与端点都写死，避免测试依赖布局结果。
    fn probe_edge(shape: EdgeShape, arrow: bool) -> SceneEdge {
        SceneEdge {
            from: 0,
            to: 1,
            start: Point::new(100.0, 100.0),
            end: Point::new(300.0, 220.0),
            orientation: Orientation::Horizontal,
            color: Rgba::rgb(0x3a, 0x7d, 0x6c),
            // snow 主题的一级边：3.5 → 1.75（`docs/06` §2.2 的锥形口径）
            width: 3.5,
            width_end: 1.75,
            shape,
            arrow,
            alpha: 1.0,
        }
    }

    #[test]
    fn tree_edge_path_keeps_the_shape_the_canvas_draws() {
        // 曲线：一个三次段，控制点在两端的水平中点（与画布同一个函数）。
        let curve = tree_edge_path(&probe_edge(EdgeShape::Curve, false));
        assert_eq!(curve.segs.len(), 1, "曲线边应只有一个三次段");
        match curve.segs[0] {
            PathSeg::Cubic { c1, c2, to } => {
                assert!((c1.x - 200.0).abs() < 1e-4 && (c1.y - 100.0).abs() < 1e-4);
                assert!((c2.x - 200.0).abs() < 1e-4 && (c2.y - 220.0).abs() < 1e-4);
                assert_eq!(to, Point::new(300.0, 220.0));
            }
            other => panic!("曲线边应是 Cubic，实际 {other:?}"),
        }

        // 直角：两段折线，拐点在中点竖线上（以前导出也画成曲线，这就是 G-01）。
        let elbow = tree_edge_path(&probe_edge(EdgeShape::Elbow, false));
        assert_eq!(elbow.segs.len(), 3, "直角边应是两段折线");
        assert_eq!(
            elbow.segs[0],
            PathSeg::Line {
                to: Point::new(200.0, 100.0)
            }
        );
        assert_eq!(
            elbow.segs[1],
            PathSeg::Line {
                to: Point::new(200.0, 220.0)
            }
        );
        assert_eq!(
            elbow.segs[2],
            PathSeg::Line {
                to: Point::new(300.0, 220.0)
            }
        );

        // 直线：一段。
        let straight = tree_edge_path(&probe_edge(EdgeShape::Straight, false));
        assert_eq!(straight.segs.len(), 1);
        assert_eq!(
            straight.segs[0],
            PathSeg::Line {
                to: Point::new(300.0, 220.0)
            }
        );
    }

    #[test]
    fn tapered_outline_has_the_documented_widths_at_both_ends() {
        let e = probe_edge(EdgeShape::Curve, false);
        let pts = mw_render::flatten::flatten(&tree_edge_path(&e), TAPER_TOL);
        let (left, right) = edge_outline_sides(&pts, 3.5, 1.75);
        assert_eq!(left.len(), pts.len());
        assert_eq!(right.len(), pts.len());

        // 父端 3.5、子端 1.75：横向距离就是线宽（不是半径）。
        assert!(
            (left[0].dist(right[0]) - 3.5).abs() < 1e-3,
            "父端应是 3.5，实际 {}",
            left[0].dist(right[0])
        );
        let n = left.len();
        assert!(
            (left[n - 1].dist(right[n - 1]) - 1.75).abs() < 1e-3,
            "子端应是 1.75，实际 {}",
            left[n - 1].dist(right[n - 1])
        );

        // 中间某点：宽度必须按弧长线性插值（与画布 `push_curve` 同一套 cum/total）。
        let mut total = 0.0f32;
        for w in pts.windows(2) {
            total += w[0].dist(w[1]);
        }
        let mut cum = 0.0f32;
        let mut picked = (0usize, 0.0f32);
        for (i, w) in pts.windows(2).enumerate() {
            cum += w[0].dist(w[1]);
            if cum >= total * 0.5 {
                picked = (i + 1, cum / total);
                break;
            }
        }
        let want = 3.5 + (1.75 - 3.5) * picked.1;
        let got = left[picked.0].dist(right[picked.0]);
        assert!(
            (got - want).abs() < 1e-2,
            "t={:.3} 处应是 {want:.3}，实际 {got:.3}",
            picked.1
        );

        // 两侧轮廓的中点必须落回画布那条路径上（同一个 `edge_path_with`）。
        for i in [0, n / 2, n - 1] {
            let mid = Point::new(
                (left[i].x + right[i].x) * 0.5,
                (left[i].y + right[i].y) * 0.5,
            );
            assert!(
                mid.dist(pts[i]) < 1e-3,
                "第 {i} 点的中轴线偏离 {}，说明锥形不是绕着画布路径长的",
                mid.dist(pts[i])
            );
        }
    }

    #[test]
    fn equal_widths_give_a_straight_band_not_a_cone() {
        // 概念图主题 `taper: false`：两端同宽，轮廓就该是等宽带。
        let pts = vec![Point::new(0.0, 0.0), Point::new(120.0, 0.0)];
        let (left, right) = edge_outline_sides(&pts, 1.5, 1.5);
        for (l, r) in left.iter().zip(right.iter()) {
            assert!((l.dist(*r) - 1.5).abs() < 1e-4);
        }
        // 闭合轮廓首尾相接（圆头把两端补圆了，不是断头）。
        let outline = edge_outline(&pts, 1.5, 1.5);
        assert_eq!(outline.len(), 2 * pts.len() + 2 * (CAP_STEPS - 1));
        assert!(outline[0].dist(pts[0]) < 0.8, "起端应被圆头包住");
    }

    #[test]
    fn elbow_outline_turns_at_the_corner() {
        let e = probe_edge(EdgeShape::Elbow, false);
        let pts = mw_render::flatten::flatten(&tree_edge_path(&e), TAPER_TOL);
        assert_eq!(pts.len(), 4, "直角边应有四个折点");
        assert!(pts[1].dist(Point::new(200.0, 100.0)) < 1e-4);
        assert!(pts[2].dist(Point::new(200.0, 220.0)) < 1e-4);

        // 同一条端点换成曲线：不该再经过拐点（否则「导出也画曲线」的 bug 回来了）。
        let curve = probe_edge(EdgeShape::Curve, false);
        let cpts = mw_render::flatten::flatten(&tree_edge_path(&curve), TAPER_TOL);
        assert!(
            cpts.iter()
                .all(|p| (p.x - 200.0).abs() > 1.0 || (p.y - 100.0).abs() > 1.0),
            "曲线边不该经过直角的拐点"
        );
        // 直角拐点处的法线要按 miter 拉长，否则 90° 转角会瘦掉三成。
        let (nx, ny) = unit_normal(&pts, 1);
        assert!(
            (nx.hypot(ny) - std::f32::consts::SQRT_2).abs() < 0.05,
            "拐角法线应 ≈ ×1.414，实际 {:.3}",
            nx.hypot(ny)
        );
    }

    #[test]
    fn tree_edge_arrow_matches_the_canvas_triangle() {
        let mut e = probe_edge(EdgeShape::Curve, true);
        let tri = tree_edge_arrow(&e, 1.0).expect("arrow=true 应有三角形");
        assert_eq!(tri[0], e.end, "尖端必须锚在终点");
        // 画布：size = css_to_world(7.0)、half = size * 0.42（底边半宽）。
        // 长度沿轴线量（到三角底边中点），不是量到三角底角——那是 sqrt(7²+2.94²)。
        let axis = |t: [Point; 3]| Point::new((t[1].x + t[2].x) * 0.5, (t[1].y + t[2].y) * 0.5);
        assert!((axis(tri).dist(e.end) - 7.0).abs() < 1e-4);
        assert!((tri[1].dist(tri[2]) - 2.0 * 7.0 * 0.42).abs() < 1e-4);
        // 屏幕恒定：导出按 100% 写 7 世界单位；缩小一半的相机下世界长度翻倍。
        let half_zoom = tree_edge_arrow(&e, 0.5).expect("arrow=true 应有三角形");
        assert!((axis(half_zoom).dist(e.end) - 3.5).abs() < 1e-4);

        e.arrow = false;
        assert!(tree_edge_arrow(&e, 1.0).is_none(), "arrow=false 不该画箭头");
    }

    #[test]
    fn svg_fills_a_tapered_band_and_draws_the_arrow() {
        let theme = get_theme("snow");
        let d = doc();
        let settings = LayoutSettings {
            edge_arrow: true,
            ..Default::default()
        };
        let sc = build_scene(
            &d,
            &d.root_id.clone(),
            &settings,
            &theme,
            &MonoMeasurer,
            None,
        );
        let out = render(&sc, &theme, &d.assets, Options::default());

        // 1) 树边不再是一条等宽描边线（G-02 的病根）。
        for e in &sc.edges {
            assert!(
                !out.contains(&format!("stroke=\"{}\"", hex(e.color))),
                "树边不该再以 stroke 出现"
            );
        }
        // 2) 每条边一个填充闭合轮廓，顶点首尾都是我们自己算的轮廓点。
        assert_eq!(
            out.matches("fill-rule=\"nonzero\"").count(),
            sc.edges.len(),
            "每条树边应有一个填充轮廓"
        );
        for e in &sc.edges {
            let outline = tree_edge_outline(e, 1.0);
            assert!(outline.len() > 4);
            let head = format!("M {} {}", num(outline[0].x), num(outline[0].y));
            assert!(out.contains(&head), "轮廓起点没写进 SVG：{head}");
        }
        // 3) 箭头：画布同款三角形，SVG 以前一个都没有（G-05）。
        assert_eq!(
            out.matches("<polygon points=").count(),
            sc.edges.len(),
            "每条 arrow=true 的树边应有一个三角形"
        );
        let e = sc.edges[0];
        let tri = tree_edge_arrow(&e, 1.0).expect("箭头");
        let tip = format!(
            "<polygon points=\"{}",
            format!("{},{}", num(tri[0].x), num(tri[0].y))
        );
        assert!(out.contains(&tip), "箭头尖端不在 SVG 里：{tip}");
    }
}
