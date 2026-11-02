//! 悬停反馈（打磨轮 37）：折叠圆点 / 概要 / 边界的「指得到」提示。
//!
//! 全部画在合成纹理之上的 egui 覆盖层，**不进 [`mw_render::Frame`]**：
//! 悬停是交互态，导出（PNG/SVG/PDF）不该看得见它；`Frame` 也不该为它加交互字段。
//! 节点与关系线已有自己的悬停反馈（渲染层环 / 强调线），这里不叠第二层。

use super::*;
use mw_core::layout::toggle_pos;
use mw_render::flatten::{flatten, tolerance};
use mw_render::instances::{TOGGLE_BAR, TOGGLE_HALO, TOGGLE_R, TOGGLE_W};

/// 悬停反馈的目标。下标都是本帧 `Scene` 的密集索引。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HoverDetail {
    /// 折叠圆点 → 节点下标。
    Toggle(u32),
    /// 概要（主题盒或花括号）→ `Scene::summaries` 下标。
    Summary(u32),
    /// 边界 → `Scene::boundaries` 下标。
    Boundary(u32),
}

/// 命中目标 → 悬停反馈目标。
///
/// 概要的主题盒与花括号指向**同一条**概要：用户指着花括号或标题，读到的都该是
/// 「这一整条概要」。节点 / 标签 / 关系线返回 `None`——它们已有自己的反馈。
pub(crate) fn hover_detail_of(t: Target) -> Option<HoverDetail> {
    match t {
        Target::Toggle(n) => Some(HoverDetail::Toggle(n)),
        Target::SummaryTopic(s) | Target::SummaryBrace(s) => Some(HoverDetail::Summary(s)),
        Target::Boundary(b) => Some(HoverDetail::Boundary(b)),
        // 分组框没有自己的悬停反馈：它圈住的是概念，指着框线时不该抢概念的反馈。
        Target::Relationship(_) | Target::Label(_, _) | Target::Node(_) | Target::Group(_) => None,
    }
}

/// 折叠圆点悬停时的放大倍数（`docs/06` §5「悬停放大 1.3 倍」）。
pub(crate) const TOGGLE_HOVER_SCALE: f32 = 1.3;

/// 按淡入进度插值放大倍数：alpha=0 时与原圆点重合，alpha=1 时 1.3 倍。
pub(crate) fn toggle_hover_scale(alpha: f32) -> f32 {
    1.0 + (TOGGLE_HOVER_SCALE - 1.0) * alpha.clamp(0.0, 1.0)
}

/// 折叠圆点在屏幕上的半径（逻辑 px）。
///
/// 与渲染层同一公式：世界下限 4.5（缩到很小时圆点不会退化成 0），再乘缩放回屏幕。
pub(crate) fn toggle_screen_radius(camera: &Camera) -> f32 {
    camera.chrome(TOGGLE_R, 4.5) * camera.zoom
}

impl Canvas {
    /// 悬停的注解/折叠点：圆点按 1.3 倍重绘，概要/边界描一圈 55% 强调色。
    ///
    /// 强调强度与节点悬停同一档（渲染层用 `hover_alpha × 0.55` 画环），一眼能分清
    /// 「指着」（半透明）与「选中」（实色 2px）。正在选中的注解不再叠一圈。
    pub(crate) fn paint_hover_detail(&self, ui: &egui::Ui, viewport: [f32; 2]) {
        let Some(detail) = self.hover_detail else {
            return;
        };
        if self.demo.is_some()
            || self.editor.is_some()
            || self.summary_edit.is_some()
            || self.rel_edit.is_some()
            || self.input_blocked
            || self.drag.is_some()
            || self.moving.is_some()
            || self.link.is_some()
        {
            return;
        }
        let alpha = self.hover_detail_alpha.clamp(0.0, 1.0);
        if alpha <= 0.002 {
            return;
        }
        let painter = ui.painter();
        let origin = self.last_rect.min;
        let zoom = self.camera.zoom;
        let accent = self.session.theme.ui.accent;
        let strength = 0.55 * alpha;
        let to_screen = |p: Point| {
            let s = self.camera.world_to_screen(p, viewport);
            Pos2::new(origin.x + s.x, origin.y + s.y)
        };
        match detail {
            HoverDetail::Toggle(n) => {
                let Some(node) = self.scene.nodes.get(n as usize) else {
                    return;
                };
                if !node.has_children {
                    return;
                }
                let parent_x = node
                    .parent
                    .and_then(|p| self.scene.nodes.get(p as usize))
                    .map(|p| p.geom.rect.x);
                let local = toggle_pos(node.geom.rect, self.scene.kind, parent_x);
                let c = to_screen(Point::new(
                    node.geom.rect.x + local.x,
                    node.geom.rect.y + local.y,
                ));
                // 重绘而不是只加一圈：`docs/06` 要的是「圆点放大」，而圆点的几何
                // 归渲染层所有。铺一颗画布色底把原圆点与穿过来的连线盖掉，再按
                // 插值后的半径画环与 +/−；alpha=0 时与渲染层逐像素重合，淡入即长大。
                let scale = toggle_hover_scale(alpha);
                let r = toggle_screen_radius(&self.camera) * scale;
                let halo = self.camera.css_to_world(TOGGLE_HALO) * zoom * scale;
                let w = (self.camera.css_to_world(TOGGLE_W) * zoom * scale).max(1.0);
                let bar = self.camera.css_to_world(TOGGLE_BAR) * zoom * scale;
                let branch = with_alpha(node.style.branch_color, (255.0 * node.alpha) as u8);
                let bg = with_alpha(self.session.theme.ui.canvas_bg, 255);
                painter.circle_filled(c, r + halo, bg);
                painter.circle_stroke(c, r, Stroke::new(w, branch));
                painter.line_segment(
                    [Pos2::new(c.x - bar, c.y), Pos2::new(c.x + bar, c.y)],
                    Stroke::new(w, branch),
                );
                if node.collapsed {
                    painter.line_segment(
                        [Pos2::new(c.x, c.y - bar), Pos2::new(c.x, c.y + bar)],
                        Stroke::new(w, branch),
                    );
                }
            }
            HoverDetail::Summary(i) => {
                let Some(s) = self.scene.summaries.get(i as usize) else {
                    return;
                };
                if self
                    .annotations
                    .iter()
                    .any(|a| a.kind == AnnotationKind::Summary && a.id == s.id)
                {
                    return;
                }
                // 概要自己淡入/淡出时，悬停环不能比本体更亮。
                let a = strength * s.alpha;
                if a <= 0.002 {
                    return;
                }
                let ink = with_alpha(accent, (255.0 * a) as u8);
                let tol = tolerance(self.camera.zoom * self.camera.ppp);
                for path in [&s.brace, &s.connector] {
                    let pts: Vec<Pos2> = flatten(path, tol).into_iter().map(to_screen).collect();
                    if pts.len() >= 2 {
                        painter.add(egui::Shape::line(pts, Stroke::new(2.0, ink)));
                    }
                }
                let p0 = to_screen(Point::new(s.topic.x, s.topic.y));
                let p1 = to_screen(Point::new(s.topic.right(), s.topic.bottom()));
                painter.rect_stroke(
                    egui::Rect::from_min_max(p0, p1).expand(2.0),
                    s.topic.h / 2.0 * zoom,
                    Stroke::new(1.5, ink),
                    egui::StrokeKind::Middle,
                );
            }
            HoverDetail::Boundary(i) => {
                let Some(b) = self.scene.boundaries.get(i as usize) else {
                    return;
                };
                if self
                    .annotations
                    .iter()
                    .any(|a| a.kind == AnnotationKind::Boundary && a.id == b.id)
                {
                    return;
                }
                let a = strength * b.alpha;
                if a <= 0.002 {
                    return;
                }
                let p0 = to_screen(Point::new(b.rect.x, b.rect.y));
                let p1 = to_screen(Point::new(b.rect.right(), b.rect.bottom()));
                painter.rect_stroke(
                    egui::Rect::from_min_max(p0, p1).expand(2.0),
                    design::radius::PANEL * zoom,
                    Stroke::new(1.5, with_alpha(accent, (255.0 * a) as u8)),
                    egui::StrokeKind::Middle,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hit_target_maps_to_its_own_hover_feedback() {
        assert_eq!(
            hover_detail_of(Target::Toggle(3)),
            Some(HoverDetail::Toggle(3))
        );
        assert_eq!(
            hover_detail_of(Target::SummaryTopic(4)),
            Some(HoverDetail::Summary(4))
        );
        assert_eq!(
            hover_detail_of(Target::SummaryBrace(4)),
            Some(HoverDetail::Summary(4))
        );
        assert_eq!(
            hover_detail_of(Target::Boundary(5)),
            Some(HoverDetail::Boundary(5))
        );
        // 已有自己反馈的目标不叠第二层。
        assert_eq!(hover_detail_of(Target::Node(1)), None);
        assert_eq!(hover_detail_of(Target::Label(1, 0)), None);
        assert_eq!(hover_detail_of(Target::Relationship(2)), None);
    }

    #[test]
    fn toggle_grows_from_exact_size_to_one_point_three() {
        assert_eq!(toggle_hover_scale(0.0), 1.0);
        assert!((toggle_hover_scale(1.0) - 1.3).abs() < 1e-6);
        assert!((toggle_hover_scale(0.5) - 1.15).abs() < 1e-6);
        // 越界输入按端点夹取，动画过冲也不会画出 0 或 2 倍。
        assert_eq!(toggle_hover_scale(-1.0), 1.0);
        assert_eq!(toggle_hover_scale(3.0), TOGGLE_HOVER_SCALE);
    }

    #[test]
    fn toggle_radius_keeps_the_world_floor() {
        let mut cam = Camera::new(Point::new(0.0, 0.0), 1.0, 1.0);
        assert!((toggle_screen_radius(&cam) - TOGGLE_R).abs() < 1e-6);
        // 缩小：屏幕恒定，仍是 8px。
        cam.zoom = 0.1;
        assert!((toggle_screen_radius(&cam) - TOGGLE_R).abs() < 1e-6);
        // 放大：世界尺寸不低于 4.5，所以 200% 下画到 9px（与渲染层同一公式）。
        cam.zoom = 2.0;
        assert!((toggle_screen_radius(&cam) - 9.0).abs() < 1e-6);
    }
}
