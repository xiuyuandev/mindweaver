//! PDF 导出：**真矢量**多页 PDF（2026-10-21 第二班）。
//!
//! 内容流与画布同源：`Scene` 提供盒/连线/边界几何，`mw_render::TextEngine::outlines`
//! 把 DirectWrite 的字形轮廓变成路径。于是 PDF 里没有一张位图：放大到 800% 仍锐利，
//! 换机器也不依赖字体文件。文字因此不可选中（路径不是文本）；这是与「位图 + 文本层」
//! 的取舍，后续可用 Type3/ToUnicode 补上搜索。
//!
//! 分页：A4 横向（842×595 pt），36 pt 页边距。内容按 96dpi→72pt（×0.75）换算，
//! 超过一页时按行列切片，每页用 PDF 裁剪矩形限定内容窗口，互不串页。
//! 节点图片暂不嵌入（`SceneNode.image` 未进内容流），文档里如实标注。
//!
//! 颜色：PDF 没有混合模式，半透明颜色先按画布底色混合成实色，观感与画布一致；
//! 字形轮廓按非零环绕填充，粗体再描一圈细边补偿 DWrite 的合成加粗。

use std::fmt::Write as _;
use std::io::Write as _;

use mw_core::layout::toggle_pos;
use mw_core::theme::{Rgba, Shape, Theme};
use mw_core::{Path, PathSeg, Point, Rect, Scene};
use mw_render::camera::Camera;
use mw_render::instances::{
    BADGE_OFFSET, BADGE_R, BOUNDARY_RADIUS, BOUNDARY_W, BRACE_W, CONNECTOR_W, REL_LABEL_H,
    REL_LABEL_MIN_W, REL_START_FLOOR, REL_START_R, REL_W, SUMMARY_DASH, SUMMARY_TOPIC_W,
    TOGGLE_BAR, TOGGLE_R, TOGGLE_W,
};
use mw_render::{Frame, OutlinedText, TextEngine};

/// A4 横向（点）。
pub const PAGE_W: f32 = 842.0;
pub const PAGE_H: f32 = 595.0;
/// 页边距（点）。
pub const MARGIN: f32 = 36.0;
/// 96 dpi 世界单位 → 72 dpi PDF 点。
const S: f32 = 0.75;

/// 矢量 PDF 的编码结果（页数用于 UI/测试取证）。
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub pages: usize,
}

/// 整张图 → 多页矢量 PDF。`bounds` 覆盖取景框（「可视区域」导出）。
pub fn encode_scene(scene: &Scene, theme: &Theme, bounds: Option<Rect>) -> Result<Encoded, String> {
    let b = bounds.unwrap_or_else(|| crate::export::content_bounds(scene));
    let frame = Frame {
        scene,
        theme,
        camera: Camera::new(b.center(), 1.0, 1.0),
        selected: Vec::new(),
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        // `None` = 不做视口剔除：导出要整张图，而不是当前屏幕。
        viewport: None,
    };
    let mut text =
        TextEngine::new().ok_or_else(|| "DirectWrite 不可用，无法生成字形轮廓".to_string())?;
    let texts = text.outlines(&frame);

    let inner_w = PAGE_W - MARGIN * 2.0;
    let inner_h = PAGE_H - MARGIN * 2.0;
    // 点阵与画布 / SVG 同源（X21b）：三家都只认 `Theme::canvas_dots()` 这一个出口。
    let dots = theme.canvas_dots();
    let (xs, ys) = page_bands(b, scene, inner_w / S, inner_h / S);
    let mut pages = Vec::with_capacity(xs.len() * ys.len());
    for (y0, y1) in &ys {
        for (x0, x1) in &xs {
            let cw = ((x1 - x0) * S).min(inner_w);
            let ch = ((y1 - y0) * S).min(inner_h);
            let mut page = String::with_capacity(4096);
            fill_rect(
                &mut page,
                Rect::new(0.0, 0.0, PAGE_W, PAGE_H),
                theme.ui.canvas_bg,
                theme.ui.canvas_bg,
            );
            // 裁剪框必须与 CTM 同源：内容窗口的顶边固定落在 PAGE_H-MARGIN，
            // 底边随带高下移。若锚在 MARGIN，带高不足一页时会把顶部整段裁掉。
            let y_clip = MARGIN + (inner_h - ch);
            let _ = writeln!(
                page,
                "q\n{} {} {} {} re W n",
                num(MARGIN),
                num(y_clip),
                num(cw),
                num(ch)
            );
            // 翻转 y：之后所有几何直接用世界坐标书写。
            let tx = MARGIN - x0 * S;
            let ty = PAGE_H - MARGIN + y0 * S;
            let _ = writeln!(
                page,
                "{} 0 0 -{} {} {} cm",
                num(S),
                num(S),
                num(tx),
                num(ty)
            );
            // 内容窗口的世界坐标矩形：只把它附近的图元写进这一页，
            // 否则同一份上千个字形轮廓会在每一页重复一遍，文件按页数翻倍。
            let page_rect = Rect::new(*x0, *y0, x1 - x0, y1 - y0);
            if dots.is_some() {
                // 图案对象的参数已经进了图案对象；这里只负责「在世界坐标里铺一层」
                paint_dots(&mut page, page_rect);
            }
            draw_scene(&mut page, scene, theme, &texts, page_rect);
            page.push_str("Q\n");
            pages.push(page);
        }
    }
    let bytes = assemble(&pages, dots, theme.ui.canvas_bg);
    Ok(Encoded {
        bytes,
        pages: pages.len(),
    })
}

/// 把内容切成若干世界坐标带，优先让切线落在节点之间的空隙里，
/// 而不是从卡片或文字中间穿过去。返回每页的 [x0,x1] / [y0,y1]。
fn page_bands(
    b: Rect,
    scene: &Scene,
    page_w: f32,
    page_h: f32,
) -> (Vec<(f32, f32)>, Vec<(f32, f32)>) {
    let mut xiv: Vec<(f32, f32)> = Vec::new();
    let mut yiv: Vec<(f32, f32)> = Vec::new();
    for idx in &scene.draw_order {
        let r = scene.node(*idx).geom.rect;
        // 右侧留出折叠圆点/角标的余量，别把「+」切成半圆。
        xiv.push((r.x - 4.0, r.right() + 24.0));
        yiv.push((r.y - 10.0, r.bottom() + 10.0));
    }
    (
        bands(b.x, b.right(), page_w, &mut xiv),
        bands(b.y, b.bottom(), page_h, &mut yiv),
    )
}

fn bands(min: f32, max: f32, step: f32, iv: &mut Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    iv.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut start = min;
    while start < max - 0.01 {
        let mut end = (start + step).min(max);
        // 切线落在某个节点带内：退到该节点左侧的空隙；退得太多（< 35% 页宽）就不退，
        // 宁可切开一张超宽卡片也不能让页面大半是空白。
        if end < max {
            if let Some((left, _)) = iv.iter().find(|(l, r)| *l < end - 0.01 && end < *r + 0.01) {
                let cut = *left - 2.0;
                if cut > start + step * 0.35 {
                    end = cut;
                }
            }
        }
        out.push((start, end));
        start = end;
    }
    if out.is_empty() {
        out.push((min, max));
    }
    out
}

fn draw_scene(
    out: &mut String,
    scene: &Scene,
    theme: &Theme,
    texts: &[OutlinedText],
    page_rect: Rect,
) {
    let bg = theme.ui.canvas_bg;
    let page = page_rect.expanded(24.0);
    // 点阵不在这里画：它是 L0 背景，`encode_scene` 在 `cm` 之后、内容之前用平铺图案铺一遍
    // （`paint_dots`）。位置必须在**世界坐标**里，否则图案的相位与 SVG / 画布对不上。

    // 分组框（CM-18）：6% 淡底 + 6/4 虚线描边。虚线是描边状态，必须在 q…Q 里设，
    // 否则会漏到后面所有线上（见 `stroke_path_styled` 的注释）。
    for g in &scene.groups {
        if !page.intersects(g.rect) {
            continue;
        }
        fill_round(
            out,
            g.rect,
            mw_render::instances::GROUP_RADIUS,
            g.color,
            bg,
            0.06,
            g.color,
            bg,
            0.0,
            0.0,
        );
        out.push_str("q\n");
        let _ = writeln!(
            out,
            "[{} {}] 0 d",
            num(mw_core::REL_DASH[0]),
            num(mw_core::REL_DASH[1])
        );
        let c = mix_rgba(bg, g.color, 0.55);
        let _ = writeln!(
            out,
            "{} {} {} RG {} w",
            num(c[0]),
            num(c[1]),
            num(c[2]),
            num(mw_render::instances::GROUP_W)
        );
        round_rect_path(out, g.rect, mw_render::instances::GROUP_RADIUS);
        out.push_str("S\nQ\n");
    }
    for bnd in &scene.boundaries {
        if !page.intersects(bnd.rect) {
            continue;
        }
        fill_round(
            out,
            bnd.rect,
            BOUNDARY_RADIUS,
            bnd.color,
            bg,
            0.07 * bnd.alpha,
            bnd.color,
            bg,
            0.5 * bnd.alpha,
            BOUNDARY_W,
        );
    }
    for s in &scene.spines {
        if !page.intersects(path_bounds(&s.path)) {
            continue;
        }
        stroke_path(out, &s.path, s.color, bg, s.width, s.alpha);
    }
    for e in &scene.edges {
        // X24：与 SVG 同源——形状取 `SceneEdge.shape`、锥形用两侧轮廓填充、
        // 箭头调同一个 `tree_edge_arrow`（不再有 `AL = 9.0 / AW = 6.5` 那第二套几何）。
        let path = crate::export_svg::tree_edge_path(e);
        if !page.intersects(path_bounds(&path)) {
            continue;
        }
        fill_polygon(
            out,
            &crate::export_svg::tree_edge_outline(e, 1.0),
            e.color,
            bg,
            e.alpha,
        );
        if let Some(tri) = crate::export_svg::tree_edge_arrow(e, 1.0) {
            polygon(out, &tri, e.color, bg, e.alpha);
        }
    }
    for s in &scene.summaries {
        if !page.intersects(s.topic)
            && !page.intersects(path_bounds(&s.brace))
            && !page.intersects(path_bounds(&s.connector))
        {
            continue;
        }
        stroke_path(out, &s.brace, s.color, bg, BRACE_W, s.alpha);
        stroke_path(out, &s.connector, s.color, bg, CONNECTOR_W, s.alpha);
        dashed_round(
            out,
            s.topic,
            s.topic.h * 0.5,
            bg,
            s.color,
            bg,
            SUMMARY_TOPIC_W,
            SUMMARY_DASH,
            s.alpha,
        );
    }

    for idx in &scene.draw_order {
        let n = scene.node(*idx);
        if !page.intersects(n.geom.rect) {
            continue;
        }
        // 卡片投影：偏移 2pt 的柔和底影在 PDF 里用实色近似。
        if n.style.shape != Shape::Underline {
            fill_round(
                out,
                Rect::new(
                    n.geom.rect.x,
                    n.geom.rect.y + 2.0,
                    n.geom.rect.w,
                    n.geom.rect.h,
                ),
                corner(&n.style, n.geom.rect),
                shadow_color(bg),
                bg,
                0.10 * n.alpha,
                Rgba::TRANSPARENT,
                bg,
                0.0,
                0.0,
            );
        }
        if n.style.shape == Shape::Underline {
            let y = n.geom.rect.bottom() - 1.0;
            let path = {
                let mut p = Path::new(Point::new(n.geom.rect.x, y));
                p.line(Point::new(n.geom.rect.right(), y));
                p
            };
            stroke_path(
                out,
                &path,
                n.style.stroke,
                bg,
                n.style.stroke_width.max(1.0),
                n.alpha,
            );
        } else {
            fill_round(
                out,
                n.geom.rect,
                corner(&n.style, n.geom.rect),
                n.style.fill,
                bg,
                n.alpha,
                n.style.stroke,
                bg,
                n.alpha,
                n.style.stroke_width,
            );
        }
        let origin = Point::new(n.geom.rect.x, n.geom.rect.y);
        for p in &n.labels {
            let r = Rect::new(p.rect.x + origin.x, p.rect.y + origin.y, p.rect.w, p.rect.h);
            fill_round(
                out,
                r,
                r.h * 0.5,
                n.style.branch_color,
                bg,
                0.92 * n.alpha,
                Rgba::TRANSPARENT,
                bg,
                0.0,
                0.0,
            );
        }
        if n.image.is_some() {
            // 图片不进矢量内容流：画一个浅色占位框，避免读者以为节点是空的。
            fill_round(
                out,
                Rect::new(
                    n.geom.rect.x + 8.0,
                    n.geom.rect.y + 8.0,
                    (n.geom.rect.w - 16.0).max(1.0),
                    (n.geom.image_h - 16.0).max(1.0),
                ),
                4.0,
                n.style.stroke,
                bg,
                0.08,
                n.style.stroke,
                bg,
                0.4,
                1.0,
            );
        }
    }

    for r in &scene.relationships {
        if !page.intersects(path_bounds(&r.path)) {
            continue;
        }
        // CM-16：线宽与虚线都从 `Scene` 读，PDF 与画布同一份几何。
        stroke_path_styled(
            out,
            &r.path,
            r.color,
            bg,
            r.width,
            r.alpha,
            r.dashed.then_some(mw_core::REL_DASH),
        );
        if let Some(_label) = &r.label {
            let pill_w = r.label_w.max(REL_LABEL_MIN_W);
            let pill = Rect::new(
                r.label_at.x - pill_w * 0.5,
                r.label_at.y - REL_LABEL_H * 0.5,
                pill_w,
                REL_LABEL_H,
            );
            fill_round(
                out,
                pill,
                REL_LABEL_H * 0.5,
                bg,
                bg,
                1.0,
                r.color,
                bg,
                r.alpha,
                REL_W,
            );
        }
        circle(
            out,
            r.start,
            REL_START_R.max(REL_START_FLOOR),
            r.color,
            bg,
            r.alpha,
            0.0,
        );
        for tri in [r.arrow, r.arrow_back].into_iter().flatten() {
            // 与 SVG 同口径：导出固定 100% 的箭头长度。
            let tri = mw_core::geom::arrow_at_zoom(tri, 1.0);
            polygon(out, &tri, r.color, bg, r.alpha);
        }
    }

    for idx in &scene.draw_order {
        let n = scene.node(*idx);
        let Some(p) = n.parent else { continue };
        if !n.has_children || !page.intersects(n.geom.rect) {
            continue;
        }
        let local = toggle_pos(n.geom.rect, scene.kind, Some(scene.node(p).geom.rect.x));
        let c = Point::new(n.geom.rect.x + local.x, n.geom.rect.y + local.y);
        circle(out, c, TOGGLE_R, bg, bg, 1.0, 0.0);
        circle(out, c, TOGGLE_R, bg, bg, 1.0, TOGGLE_W);
        let mut bar = Path::new(Point::new(c.x - TOGGLE_BAR, c.y));
        bar.line(Point::new(c.x + TOGGLE_BAR, c.y));
        stroke_path(out, &bar, n.style.branch_color, bg, TOGGLE_W, 1.0);
        if n.collapsed {
            let mut v = Path::new(Point::new(c.x, c.y - TOGGLE_BAR));
            v.line(Point::new(c.x, c.y + TOGGLE_BAR));
            stroke_path(out, &v, n.style.branch_color, bg, TOGGLE_W, 1.0);
        }
    }

    for idx in &scene.draw_order {
        let n = scene.node(*idx);
        if !page.intersects(n.geom.rect) {
            continue;
        }
        let r = BADGE_R;
        let dx = n.geom.rect.right() - BADGE_OFFSET;
        let dy = n.geom.rect.y + BADGE_OFFSET;
        if n.has_note {
            circle(
                out,
                Point::new(dx, dy),
                r,
                n.style.branch_color,
                bg,
                0.85,
                0.0,
            );
        }
        if n.has_link {
            circle(
                out,
                Point::new(dx - r * 3.0, dy),
                r,
                n.style.branch_color,
                bg,
                0.6,
                0.0,
            );
        }
    }

    for t in texts {
        if !page.intersects(text_bounds(t)) {
            continue;
        }
        draw_text(out, t, bg);
    }
}

/// 字形轮廓的粗包围盒（控制点也计入，只用于分页剔除）。
fn text_bounds(t: &OutlinedText) -> Rect {
    let mut out: Option<Rect> = None;
    for p in &t.paths {
        let r = path_bounds(p);
        out = Some(match out {
            Some(a) => a.union(r),
            None => r,
        });
    }
    out.unwrap_or_default()
}

fn path_bounds(p: &Path) -> Rect {
    let mut min = p.start;
    let mut max = p.start;
    let mut grow = |q: Point| {
        min.x = min.x.min(q.x);
        min.y = min.y.min(q.y);
        max.x = max.x.max(q.x);
        max.y = max.y.max(q.y);
    };
    for seg in &p.segs {
        match *seg {
            PathSeg::Line { to } => grow(to),
            PathSeg::Quad { ctrl, to } => {
                grow(ctrl);
                grow(to);
            }
            PathSeg::Cubic { c1, c2, to } => {
                grow(c1);
                grow(c2);
                grow(to);
            }
        }
    }
    Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
}

fn draw_text(out: &mut String, t: &OutlinedText, bg: Rgba) {
    if t.paths.is_empty() {
        return;
    }
    let col = Rgba([
        (t.color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (t.color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (t.color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
        (t.color[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ]);
    let rgb = mix_rgba(bg, col, 1.0);
    out.push_str("q\n1 J 1 j\n");
    let _ = writeln!(out, "{} {} {} rg", num(rgb[0]), num(rgb[1]), num(rgb[2]));
    // 伪粗描边只在字重 >= 600 时加：判据从 `bold`（二值）改成 `weight`（SE-13），
    // 否则显式 `weight=400` 的层级粗体节点在画布上是常规、PDF 却多一道描边。
    // 既有 `bold` 文字经 `WEIGHT_BOLD_ALIAS` 仍是 600，所以这处改动不改任何既有导出。
    let pseudo_bold = t.weight >= mw_core::theme::WEIGHT_SEMIBOLD;
    if pseudo_bold {
        let _ = writeln!(
            out,
            "{} {} {} RG {} w",
            num(rgb[0]),
            num(rgb[1]),
            num(rgb[2]),
            num(t.font_size * 0.028)
        );
    }
    for p in &t.paths {
        emit_contour(out, p);
    }
    out.push_str(if pseudo_bold { "B\nQ\n" } else { "f\nQ\n" });
}

fn emit_contour(out: &mut String, p: &Path) {
    let _ = writeln!(out, "{} {} m", num(p.start.x), num(p.start.y));
    let mut cur = p.start;
    for seg in &p.segs {
        match *seg {
            PathSeg::Line { to } => {
                let _ = writeln!(out, "{} {} l", num(to.x), num(to.y));
                cur = to;
            }
            PathSeg::Quad { ctrl, to } => {
                let c1 = Point::new(
                    cur.x + (ctrl.x - cur.x) * 2.0 / 3.0,
                    cur.y + (ctrl.y - cur.y) * 2.0 / 3.0,
                );
                let c2 = Point::new(
                    to.x + (ctrl.x - to.x) * 2.0 / 3.0,
                    to.y + (ctrl.y - to.y) * 2.0 / 3.0,
                );
                let _ = writeln!(
                    out,
                    "{} {} {} {} {} {} c",
                    num(c1.x),
                    num(c1.y),
                    num(c2.x),
                    num(c2.y),
                    num(to.x),
                    num(to.y)
                );
                cur = to;
            }
            PathSeg::Cubic { c1, c2, to } => {
                let _ = writeln!(
                    out,
                    "{} {} {} {} {} {} c",
                    num(c1.x),
                    num(c1.y),
                    num(c2.x),
                    num(c2.y),
                    num(to.x),
                    num(to.y)
                );
                cur = to;
            }
        }
    }
    out.push_str("h\n");
}

#[allow(clippy::too_many_arguments)]
fn fill_round(
    out: &mut String,
    r: Rect,
    radius: f32,
    fill: Rgba,
    bg: Rgba,
    fill_a: f32,
    stroke: Rgba,
    _bg2: Rgba,
    stroke_a: f32,
    width: f32,
) {
    round_rect_path(out, r, radius);
    let _ = writeln!(
        out,
        "{} {} {} rg",
        num(mix_rgba(bg, fill, fill_a)[0]),
        num(mix_rgba(bg, fill, fill_a)[1]),
        num(mix_rgba(bg, fill, fill_a)[2])
    );
    let stroked = stroke_a > 0.0 && width > 0.0;
    if stroked {
        let _ = writeln!(
            out,
            "{} {} {} RG {} w",
            num(mix_rgba(bg, stroke, stroke_a)[0]),
            num(mix_rgba(bg, stroke, stroke_a)[1]),
            num(mix_rgba(bg, stroke, stroke_a)[2]),
            num(width)
        );
        out.push_str("B\n");
    } else {
        out.push_str("f\n");
    }
}

#[allow(clippy::too_many_arguments)]
fn dashed_round(
    out: &mut String,
    r: Rect,
    radius: f32,
    fill: Rgba,
    stroke: Rgba,
    bg: Rgba,
    width: f32,
    dash: [f32; 2],
    alpha: f32,
) {
    out.push_str("q\n");
    let _ = writeln!(out, "[{} {}] 0 d", num(dash[0]), num(dash[1]));
    fill_round(out, r, radius, fill, bg, 1.0, stroke, bg, alpha, width);
    out.push_str("Q\n");
}

fn fill_rect(out: &mut String, r: Rect, color: Rgba, _bg: Rgba) {
    let _ = writeln!(
        out,
        "{} {} {} rg\n{} {} {} {} re f",
        num(color.0[0] as f32 / 255.0),
        num(color.0[1] as f32 / 255.0),
        num(color.0[2] as f32 / 255.0),
        num(r.x),
        num(r.y),
        num(r.w),
        num(r.h)
    );
}

fn round_rect_path(out: &mut String, r: Rect, radius: f32) {
    if radius <= mw_render::ELLIPSE_CODE + 0.5 {
        // 椭圆（概念图节点）：四段三次贝塞尔，控制点用标准的 0.5522848 近圆系数，
        // 误差 < 0.02% 半径，印刷尺度看不出来。与 wgsl `sd_ellipse`、SVG `<ellipse>`
        // 是同一条边界。
        let (cx, cy) = (r.center().x, r.center().y);
        let (rx, ry) = (r.w.max(0.0) * 0.5, r.h.max(0.0) * 0.5);
        let (kx, ky) = (rx * 0.552_284_8, ry * 0.552_284_8);
        let _ = writeln!(out, "{} {} m", num(cx + rx), num(cy));
        let arcs = [
            (cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
            (cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
            (cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
            (cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        ];
        for (c1x, c1y, c2x, c2y, ex, ey) in arcs {
            let _ = writeln!(
                out,
                "{} {} {} {} {} {} c",
                num(c1x),
                num(c1y),
                num(c2x),
                num(c2y),
                num(ex),
                num(ey)
            );
        }
        return;
    }
    let rad = radius.min(r.w * 0.5).min(r.h * 0.5).max(0.0);
    let k = rad * 0.552_284_8;
    let x0 = r.x;
    let y0 = r.y;
    let x1 = r.right();
    let y1 = r.bottom();
    let _ = writeln!(out, "{} {} m", num(x0 + rad), num(y0));
    let _ = writeln!(out, "{} {} l", num(x1 - rad), num(y0));
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(x1 - rad + k),
        num(y0),
        num(x1),
        num(y0 + rad - k),
        num(x1),
        num(y0 + rad)
    );
    let _ = writeln!(out, "{} {} l", num(x1), num(y1 - rad));
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(x1),
        num(y1 - rad + k),
        num(x1 - rad + k),
        num(y1),
        num(x1 - rad),
        num(y1)
    );
    let _ = writeln!(out, "{} {} l", num(x0 + rad), num(y1));
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(x0 + rad - k),
        num(y1),
        num(x0),
        num(y1 - rad + k),
        num(x0),
        num(y1 - rad)
    );
    let _ = writeln!(out, "{} {} l", num(x0), num(y0 + rad));
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(x0),
        num(y0 + rad - k),
        num(x0 + rad - k),
        num(y0),
        num(x0 + rad),
        num(y0)
    );
    out.push_str("h\n");
}

fn stroke_path(out: &mut String, p: &Path, color: Rgba, bg: Rgba, width: f32, alpha: f32) {
    stroke_path_styled(out, p, color, bg, width, alpha, None);
}

/// 带线型的描边：`dash = Some([实长, 空长])` 时先写 `[on off] 0 d`（CM-16）。
///
/// PDF 的虚线是**描边状态**，必须在 `q` 之后、`S` 之前设；写完随 `Q` 一起还原，
/// 不会漏到后面别的线上去。
fn stroke_path_styled(
    out: &mut String,
    p: &Path,
    color: Rgba,
    bg: Rgba,
    width: f32,
    alpha: f32,
    dash: Option<[f32; 2]>,
) {
    let c = mix_rgba(bg, color, alpha);
    let _ = writeln!(
        out,
        "q\n{} {} {} RG {} w\n",
        num(c[0]),
        num(c[1]),
        num(c[2]),
        num(width.max(0.2))
    );
    if let Some([on, off]) = dash {
        let _ = writeln!(out, "[{} {}] 0 d", num(on), num(off));
    }
    emit_contour(out, p);
    out.push_str("S\nQ\n");
}

fn circle(out: &mut String, c: Point, r: f32, color: Rgba, bg: Rgba, alpha: f32, width: f32) {
    let k = r * 0.552_284_8;
    let rgb = mix_rgba(bg, color, alpha);
    let _ = writeln!(
        out,
        "q\n{} {} {} rg\n{} {} {} RG {} w",
        num(rgb[0]),
        num(rgb[1]),
        num(rgb[2]),
        num(rgb[0]),
        num(rgb[1]),
        num(rgb[2]),
        num(width.max(0.2))
    );
    let _ = writeln!(out, "{} {} m", num(c.x + r), num(c.y));
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(c.x + r),
        num(c.y + k),
        num(c.x + k),
        num(c.y + r),
        num(c.x),
        num(c.y + r)
    );
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(c.x - k),
        num(c.y + r),
        num(c.x - r),
        num(c.y + k),
        num(c.x - r),
        num(c.y)
    );
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(c.x - r),
        num(c.y - k),
        num(c.x - k),
        num(c.y - r),
        num(c.x),
        num(c.y - r)
    );
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        num(c.x + k),
        num(c.y - r),
        num(c.x + r),
        num(c.y - k),
        num(c.x + r),
        num(c.y)
    );
    out.push_str(if width > 0.0 { "B\nQ\n" } else { "f\nQ\n" });
}

fn polygon(out: &mut String, pts: &[Point; 3], color: Rgba, bg: Rgba, alpha: f32) {
    fill_polygon(out, pts, color, bg, alpha);
}

/// 任意多边形填充（锥形分支线的两侧轮廓）：`m`/`l` 之后 `h f`。
///
/// 与 `polygon`（三角形）共用同一个出口——以前分支线是 `stroke_path` 描一条等宽线，
/// 锥形宽度在这里没法表达；改成轮廓填充后，宽度沿弧长真的变了（`export_svg` 同源）。
fn fill_polygon(out: &mut String, pts: &[Point], color: Rgba, bg: Rgba, alpha: f32) {
    if pts.len() < 3 {
        return;
    }
    let c = mix_rgba(bg, color, alpha);
    let _ = writeln!(out, "q\n{} {} {} rg", num(c[0]), num(c[1]), num(c[2]));
    let _ = writeln!(out, "{} {} m", num(pts[0].x), num(pts[0].y));
    for p in &pts[1..] {
        let _ = writeln!(out, "{} {} l", num(p.x), num(p.y));
    }
    out.push_str("h f\nQ\n");
}

fn corner(style: &mw_core::ResolvedStyle, r: Rect) -> f32 {
    // 单一出口：形状→半径只在 `mw_render::corner_style` 里定义一次（含椭圆哨兵与半径覆盖）。
    mw_render::corner_style(style, r)
}

fn shadow_color(_bg: Rgba) -> Rgba {
    Rgba([0, 0, 0, 255])
}

fn mix_rgba(bg: Rgba, fg: Rgba, alpha: f32) -> [f32; 3] {
    let a = (alpha * fg.0[3] as f32 / 255.0).clamp(0.0, 1.0);
    let mut out = [0.0; 3];
    for i in 0..3 {
        let b = bg.0[i] as f32 / 255.0;
        let f = fg.0[i] as f32 / 255.0;
        out[i] = b * (1.0 - a) + f * a;
    }
    out
}

/// 用平铺图案铺一层点阵（X21b）：与 SVG 的 `<pattern>`、画布的 `lattice.wgsl` 同一份令牌。
///
/// 三个数字都对得上画布：**格距**用 `dots.spacing`（世界单位，与画布同一套）、
/// **半径**用 `dots.vector_radius()`（导出按 100% 缩放换算成矢量半径，与 SVG 同一个值）、
/// **颜色**用 `dots.blended(bg)`（PDF 没有透明度，先按画布底色混成实色——与画布上
/// 预乘混合的结果是同一个不透明色）。
///
/// 相位：图案对象的 `/Matrix` 平移半格，使圆心落在世界坐标的 `k × spacing` 上，
/// 与画布、SVG 完全重合。区域只铺在本页的内容窗口（页边距仍是纯底色——那是纸边，
/// 没有内容自然也不该有点）。
fn paint_dots(out: &mut String, area: Rect) {
    out.push_str("/Pattern cs /P1 scn\n");
    let _ = writeln!(
        out,
        "{} {} {} {} re f",
        num(area.x),
        num(area.y),
        num(area.w),
        num(area.h)
    );
}

/// 点阵图案对象的内容流（`/PatternType 1` 平铺图案）：一个格子 + 一枚居中的圆点。
///
/// 整个 PDF 只要一个图案对象，页数再多也不重复——逐点写圆（一页约 1200 个）会让
/// 内容流涨到百 KB 级，而那正是「PDF 干脆不画点阵」的老理由。图案把代价摊回常数。
fn dots_pattern_stream(dots: mw_core::theme::CanvasDots, bg: Rgba) -> String {
    let s = dots.spacing.max(1.0);
    let half = s * 0.5;
    let r = dots.vector_radius();
    let mut stream = String::with_capacity(512);
    // `circle` 已经把颜色按 alpha 混成实色（alpha = 1.0 × 点色自己的 alpha），与画布同色。
    circle(
        &mut stream,
        Point::new(half, half),
        r,
        dots.color,
        bg,
        1.0,
        0.0,
    );
    stream
}

fn assemble(pages: &[String], dots: Option<mw_core::theme::CanvasDots>, bg: Rgba) -> Vec<u8> {
    // 图案对象排在所有页之后：页对象占 3..(2+2n)，图案就接着往后编号。
    let pattern_obj = 3 + pages.len() * 2;
    let resources = if dots.is_some() {
        format!("/Resources << /Pattern << /P1 {pattern_obj} 0 R >> >>")
    } else {
        "/Resources << >>".to_string()
    };
    let mut w = Writer::new();
    // 1 Catalog
    w.obj(|o| o.extend_from_slice(b"<< /Type /Catalog /Pages 2 0 R >>"));
    // 2 Pages
    w.obj(|o| {
        let kids: Vec<String> = (0..pages.len())
            .map(|i| format!("{} 0 R", 3 + i * 2))
            .collect();
        write!(
            o,
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .unwrap();
    });
    for (i, content) in pages.iter().enumerate() {
        let page_obj = 3 + i * 2;
        let content_obj = page_obj + 1;
        w.obj(|o| {
            write!(
                o,
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] {resources} /Contents {content_obj} 0 R >>",
                num(PAGE_W),
                num(PAGE_H)
            )
            .unwrap();
        });
        w.obj(|o| {
            write!(o, "<< /Length {} >>\nstream\n", content.len()).unwrap();
            o.extend_from_slice(content.as_bytes());
            o.extend_from_slice(b"\nendstream");
        });
    }
    if let Some(d) = dots {
        let stream = dots_pattern_stream(d, bg);
        // `/Matrix` 取单位阵：图案空间就是世界坐标，圆心落在格子中心 —— 与 SVG 的
        // `<pattern>` 从 (0,0) 铺起、圆心在 (spacing/2, spacing/2) 完全同相。
        w.obj(|o| {
            write!(
                o,
                "<< /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
                 /BBox [0 0 {s} {s}] /XStep {s} /YStep {s} \
                 /Matrix [1 0 0 1 0 0] /Resources << >> /Length {} >>\nstream\n",
                stream.len(),
                s = num(d.spacing.max(1.0))
            )
            .unwrap();
            o.extend_from_slice(stream.as_bytes());
            o.extend_from_slice(b"\nendstream");
        });
    }
    let xref_at = w.out.len();
    let count = w.offsets.len() + 1;
    write!(w.out, "xref\n0 {count}\n0000000000 65535 f \n").unwrap();
    for off in &w.offsets {
        writeln!(w.out, "{off:010} 00000 n ").unwrap();
    }
    write!(
        w.out,
        "trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
    )
    .unwrap();
    w.out
}

struct Writer {
    out: Vec<u8>,
    offsets: Vec<usize>,
}

impl Writer {
    fn new() -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        Self {
            out,
            offsets: Vec::new(),
        }
    }

    fn obj(&mut self, build: impl FnOnce(&mut Vec<u8>)) {
        self.offsets.push(self.out.len());
        let n = self.offsets.len();
        writeln!(self.out, "{n} 0 obj").unwrap();
        build(&mut self.out);
        self.out.extend_from_slice(b"\nendobj\n");
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::{build_scene, get_theme, new_id, Document, LayoutSettings, MonoMeasurer, Node};

    fn doc(children: usize) -> Document {
        let mut d = Document::new("根");
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().text = "根".into();
        for i in 0..children {
            let id = new_id();
            let n = Node::new(id.clone(), Some(root.clone()), format!("分支 {i} 内容"));
            d.nodes.insert(id.clone(), n);
            d.nodes.get_mut(&root).unwrap().children.push(id);
        }
        d
    }

    fn pdf(d: &Document) -> Encoded {
        let theme = get_theme("snow");
        let scene = build_scene(
            d,
            &d.root_id.clone(),
            &LayoutSettings::default(),
            &theme,
            &MonoMeasurer,
            None,
        );
        encode_scene(&scene, &theme, None).unwrap()
    }

    /// AC-6 / AC-7（X21b）：**SVG 与 PDF 对「画布有没有点阵」的判断必须一致**，
    /// 而且两家都与 `Theme::canvas_dots()` 一致；有的时候就都得把圆心落在世界原点。
    ///
    /// 这是 X21b 前最刺眼的一处「不同源」：同一份 `ink` 主题，SVG 里有 `<pattern id="zdots">`，
    /// PDF 里一个字都没有。现在判据收敛到一个纯函数（`Theme::canvas_dots`），
    /// 这条测试就是防止两个导出器再各写一份 `if`。
    #[test]
    fn svg_and_pdf_agree_on_whether_the_canvas_has_dots() {
        let d = doc(2);
        for id in ["snow", "ink", "mist", "sand", "cm-paper", "cm-slate"] {
            let theme = get_theme(id);
            let scene = build_scene(
                &d,
                &d.root_id.clone(),
                &LayoutSettings::default(),
                &theme,
                &MonoMeasurer,
                None,
            );
            let svg = crate::export_svg::render(
                &scene,
                &theme,
                &d.assets,
                crate::export_svg::Options::default(),
            );
            let bytes = encode_scene(&scene, &theme, None).unwrap().bytes;
            let pdf = String::from_utf8_lossy(&bytes).to_string();

            let on = theme.canvas_dots().is_some();
            let svg_has = svg.contains("id=\"zdots\"");
            let pdf_has = pdf.contains("/PatternType 1");
            assert_eq!(svg_has, on, "{id}：SVG 与主题令牌不一致");
            assert_eq!(pdf_has, on, "{id}：PDF 与主题令牌不一致");
            assert_eq!(svg_has, pdf_has, "{id}：SVG 与 PDF 不一致（同源破了）");

            // 取证用（只在显式设了环境变量时才落盘，正常跑测试零副作用）：
            // `MW_X21B_EVIDENCE=1 cargo test -p mw-shell svg_and_pdf_agree` →
            // `shots/x100/X21b/evidence/export-<主题>.{svg,pdf}`，供报告逐字段引用。
            if id == "ink" && std::env::var_os("MW_X21B_EVIDENCE").is_some() {
                let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../shots/x100/X21b/evidence");
                std::fs::create_dir_all(&dir).ok();
                std::fs::write(dir.join("export-ink.svg"), svg.as_bytes()).ok();
                std::fs::write(dir.join("export-ink.pdf"), &bytes).ok();
            }

            let Some(dots) = theme.canvas_dots() else {
                continue;
            };
            let half = dots.spacing * 0.5;
            let r = dots.vector_radius();
            // 相位：点落在**格子中心** `(k + 0.5) × spacing`——画布 `lattice.wgsl` 的定义
            // 就是 `off = cell - floor(cell) - 0.5`（离格点最近处即小数部分 0.5）。
            // 两家必须同相，否则「同源」只剩密度相同、位置错半格。
            assert!(
                svg.contains(&format!(
                    "width=\"{}\" height=\"{}\" patternUnits=\"userSpaceOnUse\"",
                    dots.spacing, dots.spacing
                )),
                "{id}：SVG pattern 的格距与令牌不一致"
            );
            assert!(
                svg.contains(&format!("cx=\"{half}\" cy=\"{half}\" r=\"{r}\"")),
                "{id}：SVG 的圆心/半径与画布格心不一致"
            );
            assert!(
                pdf.contains("/Matrix [1 0 0 1 0 0]"),
                "{id}：PDF 图案必须与 SVG 同相（铺在格心，不额外平移）"
            );
            assert!(
                pdf.contains(&format!(
                    "/BBox [0 0 {} {}] /XStep {} /YStep {}",
                    dots.spacing, dots.spacing, dots.spacing, dots.spacing
                )),
                "{id}：PDF 图案的格距与令牌不一致"
            );
            // 颜色与几何：图案流里那枚圆必须画在格心、半径 = `vector_radius()`，
            // 颜色 = 「令牌点色叠在画布底上」的等效色（PDF 没有透明度）。
            let c = dots.blended(theme.ui.canvas_bg);
            let stream = dots_pattern_stream(dots, theme.ui.canvas_bg);
            assert!(
                stream.contains(&format!(
                    "{} {} {} rg",
                    num(c.0[0] as f32 / 255.0),
                    num(c.0[1] as f32 / 255.0),
                    num(c.0[2] as f32 / 255.0)
                )),
                "{id}：PDF 的点色不是「令牌点色叠在画布底上」的那个值"
            );
            assert!(
                stream.contains(&format!("{} {} m", num(half + r), num(half))),
                "{id}：PDF 图案里的圆不在格心 / 半径不对"
            );
        }

        // 透明底：SVG 不画点阵（PDF 没有透明底模式，页底永远是画布色）。
        let theme = get_theme("snow");
        let scene = build_scene(
            &d,
            &d.root_id.clone(),
            &LayoutSettings::default(),
            &theme,
            &MonoMeasurer,
            None,
        );
        let svg = crate::export_svg::render(
            &scene,
            &theme,
            &d.assets,
            crate::export_svg::Options {
                transparent: true,
                bounds: None,
            },
        );
        assert!(!svg.contains("id=\"zdots\""), "透明底导出不该带点阵");
        assert!(
            !svg.contains(&format!(
                "fill=\"#{:02x}{:02x}{:02x}\"",
                theme.ui.canvas_bg.0[0], theme.ui.canvas_bg.0[1], theme.ui.canvas_bg.0[2]
            )),
            "透明底导出不该铺画布底色"
        );
    }

    fn page_count(pdf: &[u8]) -> usize {
        let s = String::from_utf8_lossy(pdf);
        let at = s.find("/Type /Pages").expect("没有 Pages");
        let tail = &s[at..];
        let count = tail.find("/Count ").expect("没有 Count");
        tail[count + 7..]
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    /// ⑥ 导出补齐：带子进 PDF 走的是同一份排版，字形落成矢量轮廓。
    ///
    /// PDF 里没有可搜的文字（全是轮廓），所以这里锁两件事：页数不变、内容真的变多。
    /// 「画的是不是那几个字」由 `export_svg` 那条按文字断言的测试保证——两边同一份 `text_jobs`。
    #[test]
    fn the_focus_note_adds_vector_content_to_the_pdf() {
        let d = doc(2);
        let theme = get_theme("snow");
        let mut scene = build_scene(
            &d,
            &d.root_id.clone(),
            &LayoutSettings::default(),
            &theme,
            &MonoMeasurer,
            None,
        );
        let before = encode_scene(&scene, &theme, None).unwrap();
        scene.attach_focus_note("这张图在回答什么？", 16.0, theme.ui.text);
        let after = encode_scene(&scene, &theme, None).unwrap();
        assert_eq!(
            page_count(&before.bytes),
            page_count(&after.bytes),
            "带子不许把页数改掉"
        );
        assert!(
            after.bytes.len() > before.bytes.len() + 500,
            "带子要真的画进 PDF：{} vs {}",
            after.bytes.len(),
            before.bytes.len()
        );
    }
    #[test]
    fn output_is_a_real_vector_pdf_without_images() {
        let e = pdf(&doc(2));
        let s = String::from_utf8_lossy(&e.bytes);
        assert!(s.starts_with("%PDF-1.4\n"));
        assert!(s.contains("/Type /Catalog"));
        assert!(s.contains("/Type /Page "));
        assert!(s.contains("/MediaBox [0 0 842 595]"));
        assert!(!s.contains("/Subtype /Image"), "矢量 PDF 不应再嵌位图");
        assert!(!s.contains("FlateDecode"), "内容流应保持可读");
        assert!(s.trim_end().ends_with("%%EOF"));
    }

    #[test]
    fn text_is_outlined_into_curves() {
        let e = pdf(&doc(0));
        let s = String::from_utf8_lossy(&e.bytes);
        let curves = s.matches(" c\n").count();
        assert!(curves > 20, "字形轮廓应产生大量三次曲线，实际 {curves}");
        assert!(s.matches(" m\n").count() > 5);
    }

    #[test]
    fn wide_content_is_paginated_on_a4_pages() {
        let e = pdf(&doc(48));
        assert!(e.pages > 1, "48 个分支应超过一页，实际 {} 页", e.pages);
        assert_eq!(page_count(&e.bytes), e.pages);
        let s = String::from_utf8_lossy(&e.bytes);
        assert_eq!(
            s.matches("/MediaBox [0 0 842 595]").count(),
            e.pages,
            "每页都应是 A4 横向"
        );
        assert!(s.matches(" re W n").count() >= e.pages, "每页应有裁剪窗口");
    }

    #[test]
    fn xref_offsets_point_at_their_objects() {
        let e = pdf(&doc(1));
        let pdf = &e.bytes;
        let needle = b"startxref\n";
        let mark = pdf
            .windows(needle.len())
            .rposition(|w| w == needle)
            .unwrap();
        let tail = std::str::from_utf8(&pdf[mark + needle.len()..]).unwrap();
        let at: usize = tail.lines().next().unwrap().trim().parse().unwrap();
        assert_eq!(&pdf[at..at + 5], b"xref\n");
    }

    #[test]
    fn small_content_stays_one_page() {
        let e = pdf(&doc(1));
        assert_eq!(e.pages, 1);
    }

    #[test]
    fn clip_is_top_anchored_so_bands_are_not_cut() {
        let e = pdf(&doc(48));
        let s = String::from_utf8_lossy(&e.bytes);
        let mut clips = 0;
        for part in s.split(" re W n") {
            let line = part.rsplit('\n').next().unwrap_or("");
            let nums: Vec<f32> = line
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            if nums.len() != 4 {
                continue;
            }
            let (x, y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
            assert!((x - MARGIN).abs() < 0.01, "裁剪框左边应贴页边距");
            assert!(
                (y + h - (PAGE_H - MARGIN)).abs() < 0.01,
                "裁剪框顶边应贴内容带顶部：y={y} h={h}"
            );
            assert!(w <= PAGE_W - MARGIN * 2.0 + 0.01);
            assert!(h <= PAGE_H - MARGIN * 2.0 + 0.01);
            clips += 1;
        }
        assert_eq!(clips, e.pages, "每页恰好一个裁剪窗口");
    }

    /// BUG-5：PDF 内容流里的箭头坐标必须等于 `zoom = 1.0` 的共用换算。
    ///
    /// 内容流按世界坐标书写、由每页 `cm` 统一缩放，所以这里直接对账三点坐标。
    /// 用概念图（`straight_between` 的 6×4 世界三角）：6 < `ARROW_MIN_CSS_LEN`，
    /// `arrow_at_zoom(tri, 1.0)` 会真的放大到 7，测试才咬得住导出确实走了共用函数。
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
            .push(mw_core::Relationship {
                id: "rel".into(),
                to: id,
                label: Some("转化".into()),
                color: None,
                head: Default::default(),
                ..Default::default()
            });
        let scene = build_scene(
            &d,
            &d.root_id.clone(),
            &d.meta.layout,
            &theme,
            &MonoMeasurer,
            None,
        );
        let rel = scene.relationships.first().expect("有一条关系线");
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
            "zoom=1 时箭头要撑到 {} css px，实际 {exp_len}",
            mw_core::ARROW_MIN_CSS_LEN
        );
        let e = encode_scene(&scene, &theme, None).unwrap();
        let s = String::from_utf8_lossy(&e.bytes);
        let head = format!("{} {} m\n", num(expected[0].x), num(expected[0].y));
        assert!(s.contains(&head), "PDF 缺少箭头顶点：{head}");
        for p in &expected[1..] {
            let line = format!("{} {} l\n", num(p.x), num(p.y));
            assert!(s.contains(&line), "PDF 缺少箭头底角：{line}");
        }
    }

    #[test]
    fn every_node_falls_inside_exactly_one_page_band() {
        let d = doc(48);
        let theme = get_theme("snow");
        let scene = build_scene(
            &d,
            &d.root_id.clone(),
            &LayoutSettings::default(),
            &theme,
            &MonoMeasurer,
            None,
        );
        let b = crate::export::content_bounds(&scene);
        let (xs, ys) = page_bands(
            b,
            &scene,
            (PAGE_W - MARGIN * 2.0) / S,
            (PAGE_H - MARGIN * 2.0) / S,
        );
        assert!(xs.len() * ys.len() > 1);
        for idx in &scene.draw_order {
            let n = scene.node(*idx);
            let r = n.geom.rect;
            let nx = xs
                .iter()
                .filter(|(a, c)| r.x >= *a - 0.01 && r.right() <= *c + 0.01)
                .count();
            let ny = ys
                .iter()
                .filter(|(a, c)| r.y >= *a - 0.01 && r.bottom() <= *c + 0.01)
                .count();
            assert_eq!((nx, ny), (1, 1), "节点 #{idx} 应恰好被一页包住");
        }
    }

    // ---- X24：导出同源（G-03/G-04/G-06） ------------------------------

    /// 同一条文档，切到 `edge_shape = elbow` + 箭头：PDF 里的分支线必须与
    /// SVG/画布**同一份几何**——直角是直角、锥形真的收细、箭头是画布那枚 7 css。
    #[test]
    fn pdf_fills_the_same_tapered_band_and_arrow() {
        let theme = get_theme("snow");
        let d = doc(2);
        let settings = LayoutSettings {
            edge_shape: "elbow".into(),
            edge_arrow: true,
            ..Default::default()
        };
        let scene = build_scene(
            &d,
            &d.root_id.clone(),
            &settings,
            &theme,
            &MonoMeasurer,
            None,
        );
        let enc = encode_scene(&scene, &theme, None).unwrap();
        let s = String::from_utf8_lossy(&enc.bytes);
        assert!(!scene.edges.is_empty());
        for e in &scene.edges {
            let outline = crate::export_svg::tree_edge_outline(e, 1.0);
            assert!(outline.len() > 4, "轮廓应有足够的顶点");
            // 轮廓点逐个以**世界坐标**写进内容流：形状与锥形都在里面（G-03/G-04）。
            for p in outline.iter().step_by(3) {
                let as_m = format!("{} {} m", num(p.x), num(p.y));
                let as_l = format!("{} {} l", num(p.x), num(p.y));
                assert!(
                    s.contains(&as_m) || s.contains(&as_l),
                    "轮廓点 ({:.2}, {:.2}) 没进 PDF",
                    p.x,
                    p.y
                );
            }
            // 箭头：尖端锚在终点、长度 7（不再是 9×6.5 那第二套几何，G-06）。
            let tri = crate::export_svg::tree_edge_arrow(e, 1.0).expect("箭头");
            let axis = Point::new((tri[1].x + tri[2].x) * 0.5, (tri[1].y + tri[2].y) * 0.5);
            assert!(
                (axis.dist(e.end) - 7.0).abs() < 1e-4,
                "PDF 箭头长度应与画布一致（7），实际 {}",
                axis.dist(e.end)
            );
            assert!(s.contains(&format!("{} {} m", num(tri[0].x), num(tri[0].y))));
        }
    }

    /// 世界→页面只缩放一次：`S 0 0 -S tx ty cm` 之后几何全部写世界坐标。
    ///
    /// 这条是 G-05/G-06 的护栏：如果哪天给轮廓/箭头再乘一遍 `S`，
    /// 分支线会比画布小 25%、箭头也会缩进去，而且肉眼极难看出来。
    #[test]
    fn pdf_applies_the_world_to_page_scale_exactly_once() {
        let theme = get_theme("snow");
        let d = doc(2);
        let scene = build_scene(
            &d,
            &d.root_id.clone(),
            &LayoutSettings::default(),
            &theme,
            &MonoMeasurer,
            None,
        );
        let enc = encode_scene(&scene, &theme, None).unwrap();
        let s = String::from_utf8_lossy(&enc.bytes);
        let ctm = format!("{} 0 0 -{}", num(S), num(S));
        assert_eq!(
            s.matches(&ctm).count(),
            enc.pages,
            "每页只应有一处世界→页面缩放"
        );
        let e = scene.edges[0];
        let p = crate::export_svg::tree_edge_outline(&e, 1.0)[0];
        let world = format!("{} {}", num(p.x), num(p.y));
        let scaled = format!("{} {}", num(p.x * S), num(p.y * S));
        assert!(s.contains(&world), "轮廓点应以世界坐标出现：{world}");
        assert!(
            !s.contains(&scaled),
            "轮廓点不该以 0.75 倍坐标出现：{scaled}"
        );
    }
}
