//! 画布 Z 序：谁压谁、谁先拿指针，集中成一张表，别再靠"画到后面自然在上面"。
//!
//! 画布由四层合成纹理（背景 / 连线 / 节点 / 覆盖）加 egui 屏幕空间浮层组成。
//! 屏幕空间从低到高：
//! 连线 < 节点 < 注解（边界/概要） < 卡片角标 < 四边锚点 < 悬停工具条 < 右键菜单/浮层。
//!
//! 规则只有两条：
//! 1. 上层几何与下层相交时，**上层负责让开**（角标给工具条让、工具条给菜单让），
//!    而不是把下层藏起来；
//! 2. 浮层矩形（缩放控件/面包屑）压住的地方，画布不再画角标、也不收命中。

use eframe::egui::{Pos2, Rect};

/// 画布屏幕空间层级，声明顺序即由低到高。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)] // 层级表是文档：当前只直接用到 Badges/HoverBar，其余给后续浮层。
pub enum Plane {
    Lines,
    Nodes,
    Annotations,
    Badges,
    Anchors,
    HoverBar,
    Menus,
}

impl Plane {
    pub fn rank(self) -> u8 {
        self as u8
    }

    /// `self` 是否压在 `other` 上面。
    pub fn above(self, other: Plane) -> bool {
        self.rank() > other.rank()
    }
}

/// 角标是否该画：必须在画布内，且不落在缩放控件/面包屑等浮层矩形上。
pub fn badge_visible(center: Pos2, canvas: Rect, overlays: &[Rect]) -> bool {
    canvas.expand(24.0).contains(center) && !overlays.iter().any(|r| r.contains(center))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    #[test]
    fn the_table_is_strictly_ordered() {
        let all = [
            Plane::Lines,
            Plane::Nodes,
            Plane::Annotations,
            Plane::Badges,
            Plane::Anchors,
            Plane::HoverBar,
            Plane::Menus,
        ];
        for w in all.windows(2) {
            assert!(w[1].rank() > w[0].rank(), "{:?} 应高于 {:?}", w[1], w[0]);
            assert!(w[1].above(w[0]));
        }
        assert!(Plane::HoverBar.above(Plane::Badges));
        assert!(!Plane::Badges.above(Plane::HoverBar));
    }

    #[test]
    fn badges_hide_under_overlays_and_outside_the_canvas() {
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let map = Rect::from_min_size(pos2(600.0, 400.0), vec2(160.0, 120.0));
        assert!(badge_visible(pos2(100.0, 100.0), canvas, &[]));
        assert!(!badge_visible(pos2(650.0, 450.0), canvas, &[map]));
        assert!(!badge_visible(pos2(-40.0, 100.0), canvas, &[]));
    }
}
