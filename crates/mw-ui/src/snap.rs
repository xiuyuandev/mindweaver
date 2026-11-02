//! 对齐吸附（docs/06 §4「拖拽落位的吸附感」）。
//!
//! 把「正在移动的那一坨」的左/中/右三条竖线与上/中/下三条横线，去贴屏上其它节点
//! 的同样六线，取**最近**的一对。阈值以屏幕像素给、换算成世界单位，所以任何缩放档位
//! 下手感一致——这也是本层刻意不吃 `Camera` 的原因：调用方只传一个 `threshold`。

use mw_core::Rect;

/// 吸附阈值（CSS px）。调用方除以 zoom 换成世界单位后传进来。
pub const SNAP_PX: f32 = 6.0;

/// 一条参考线。`axis` 由所在字段决定：`guide_x` 是竖线（x = `at`），`guide_y` 是横线。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    /// 线的坐标（世界单位）。
    pub at: f32,
    /// 线段起止（垂直于线方向的坐标），取「移动盒 ∪ 参照盒」的范围，看起来才像尺规。
    pub from: f32,
    pub to: f32,
}

/// 一次吸附的结果：把移动盒加 `(dx, dy)` 即落位。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snap {
    pub dx: f32,
    pub dy: f32,
    pub guide_x: Option<Guide>,
    pub guide_y: Option<Guide>,
}

impl Snap {
    pub fn is_none(&self) -> bool {
        self.guide_x.is_none() && self.guide_y.is_none() && !self.has_offset()
    }

    pub fn has_offset(&self) -> bool {
        self.dx.abs() > 1e-4 || self.dy.abs() > 1e-4
    }
}

/// 网格吸附：把移动盒的左/上角吸到最近的 `step` 倍数，超出阈值返回空。
pub fn snap_to_grid(moving: Rect, step: f32, threshold: f32) -> Snap {
    if step <= 1e-3 {
        return Snap::default();
    }
    let gx = (moving.x / step).round() * step;
    let gy = (moving.y / step).round() * step;
    let dx = gx - moving.x;
    let dy = gy - moving.y;
    Snap {
        dx: if dx.abs() <= threshold { dx } else { 0.0 },
        dy: if dy.abs() <= threshold { dy } else { 0.0 },
        guide_x: None,
        guide_y: None,
    }
}

/// 在 `refs` 中为 `moving` 找最近的横竖两条对齐线。
pub fn snap_offsets(moving: Rect, refs: &[Rect], threshold: f32) -> Snap {
    let mut best_x: Option<(f32, f32, Guide)> = None;
    let mut best_y: Option<(f32, f32, Guide)> = None;

    for r in refs {
        let mcols = [moving.x, moving.x + moving.w * 0.5, moving.right()];
        let rcols = [r.x, r.x + r.w * 0.5, r.right()];
        for (from, to) in mcols.into_iter().zip(rcols) {
            let d = to - from;
            let abs = d.abs();
            if abs <= threshold && best_x.as_ref().is_none_or(|(b, _, _)| abs < *b) {
                best_x = Some((
                    abs,
                    d,
                    Guide {
                        at: to,
                        from: moving.y.min(r.y),
                        to: moving.bottom().max(r.bottom()),
                    },
                ));
            }
        }

        let mrows = [moving.y, moving.y + moving.h * 0.5, moving.bottom()];
        let rrows = [r.y, r.y + r.h * 0.5, r.bottom()];
        for (from, to) in mrows.into_iter().zip(rrows) {
            let d = to - from;
            let abs = d.abs();
            if abs <= threshold && best_y.as_ref().is_none_or(|(b, _, _)| abs < *b) {
                best_y = Some((
                    abs,
                    d,
                    Guide {
                        at: to,
                        from: moving.x.min(r.x),
                        to: moving.right().max(r.right()),
                    },
                ));
            }
        }
    }

    let mut snap = Snap::default();
    if let Some((_, d, g)) = best_x {
        snap.dx = d;
        snap.guide_x = Some(g);
    }
    if let Some((_, d, g)) = best_y {
        snap.dy = d;
        snap.guide_y = Some(g);
    }
    snap
}

/// 吸附后的盒子——调用方拿它去算落点，不必自己加 dx/dy。
pub fn snapped(mut r: Rect, snap: &Snap) -> Rect {
    r.x += snap.dx;
    r.y += snap.dy;
    r
}

#[cfg(test)]
mod tests {
    use super::{snap_offsets, snap_to_grid, snapped, Guide, SNAP_PX};
    use mw_core::Rect;

    const T: f32 = SNAP_PX;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn nothing_snaps_beyond_the_threshold() {
        let moving = rect(0.0, 0.0, 100.0, 30.0);
        // 横向 200、纵向 200：六条线全在阈值外。若参照盒与移动盒同行，顶边就会以 0 误差吸上。
        let refs = [rect(200.0, 200.0, 100.0, 30.0)];
        let snap = snap_offsets(moving, &refs, T);
        assert!(snap.is_none());
        assert_eq!((snap.dx, snap.dy), (0.0, 0.0));
    }

    #[test]
    fn left_edges_within_the_threshold_snap_exactly() {
        let moving = rect(3.0, 50.0, 100.0, 30.0);
        let refs = [rect(0.0, 0.0, 80.0, 20.0)];
        let snap = snap_offsets(moving, &refs, T);
        assert_eq!(snap.dx, -3.0, "要把它推回去，不是推更远");
        assert_eq!(
            snap.guide_x,
            Some(Guide {
                at: 0.0,
                from: 0.0,
                to: 80.0
            })
        );
        assert_eq!(snapped(moving, &snap).x, 0.0);
    }

    #[test]
    fn the_nearest_reference_wins() {
        let moving = rect(100.0, 100.0, 40.0, 20.0);
        // 两根参照的顶边都在阈值内（差 4 与差 2），中线/底边差得太远。
        let refs = [
            rect(0.0, 104.0, 30.0, 100.0),
            rect(60.0, 102.0, 30.0, 100.0),
        ];
        let snap = snap_offsets(moving, &refs, T);
        assert_eq!(snap.dy, 2.0, "取更近的 102，不是先遍历到的 104");
        assert_eq!(snap.guide_y.map(|g| g.at), Some(102.0));
    }

    #[test]
    fn centres_snap_without_touching_edges() {
        let moving = rect(10.0, 0.0, 20.0, 10.0);
        let refs = [rect(0.0, 0.0, 40.0, 10.0)];
        let snap = snap_offsets(moving, &refs, T);
        // 移动盒中线 20，参照中线 20 ⇒ 已经对齐，且优先于 10 单位的左边差。
        assert_eq!(snap.dx, 0.0);
        assert_eq!(snap.guide_x.map(|g| g.at), Some(20.0));
    }

    #[test]
    fn the_two_axes_are_independent() {
        let moving = rect(2.0, 40.0, 50.0, 20.0);
        let refs = [rect(0.0, 0.0, 50.0, 30.0)];
        let snap = snap_offsets(moving, &refs, T);
        assert_eq!(snap.dx, -2.0, "左右差 2，吸上");
        assert_eq!(snap.dy, 0.0);
        assert!(snap.guide_y.is_none(), "纵向 10 单位，超出阈值");
    }

    #[test]
    fn a_multi_node_ghost_snaps_on_its_union_box() {
        let a = rect(0.0, 0.0, 30.0, 20.0);
        let b = rect(40.0, 30.0, 30.0, 20.0);
        let moving = a.union(b); // 外接盒 70×50
        let refs = [rect(48.0, 0.0, 20.0, 20.0)]; // 它的右缘在 68
        let snap = snap_offsets(moving, &refs, T);
        assert_eq!(
            moving.right() + snap.dx,
            68.0,
            "整组按外接盒右缘对齐，而不是按某个子盒"
        );
    }

    #[test]
    fn grid_snaps_left_and_top_independently() {
        let moving = rect(23.0, 41.0, 50.0, 20.0);
        let snap = snap_to_grid(moving, 20.0, 6.0);
        assert!((snap.dx - (-3.0)).abs() < 1e-4);
        assert!((snap.dy - (-1.0)).abs() < 1e-4);
        assert!(snap.has_offset() && !snap.is_none());
        let far = snap_to_grid(rect(30.0, 30.0, 50.0, 20.0), 40.0, 6.0);
        assert_eq!(far.dx, 0.0);
        assert_eq!(far.dy, 0.0);
        assert!(far.is_none());
    }
}
