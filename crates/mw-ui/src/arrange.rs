//! 对齐与等距分布：**纯几何**，不认识文档、命令或 egui。
//!
//! 只对「自由主题」有意义——树内节点的坐标由布局引擎算，不能被前端随意改。
//! 所以这里输入的是各自由主题当前的屏幕/世界矩形，输出是新的左上角；壳层再把
//! `new - old` 换算成 `SetNodePos`。
//!
//! 一条纪律：**第一个/最后一个不动**。分布只在中间制造等距，不整体平移选区，
//! 否则用户会觉得「我只是想均匀一点，怎么整组都跑了」。

use mw_core::Rect;

/// 六种对齐。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
}

/// 两个分布方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Distribute {
    Horizontal,
    Vertical,
}

/// 选区包围盒。
fn bounds(rects: &[Rect]) -> Option<Rect> {
    let first = rects.first()?;
    let (mut x0, mut y0) = (first.x, first.y);
    let (mut x1, mut y1) = (first.right(), first.bottom());
    for r in &rects[1..] {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.right());
        y1 = y1.max(r.bottom());
    }
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// 对齐后的左上角，顺序与输入一致。
///
/// 单条选区也返回原坐标（对齐没有意义，但不该报错）。
pub fn align(rects: &[Rect], how: Align) -> Vec<(f32, f32)> {
    let Some(b) = bounds(rects) else {
        return Vec::new();
    };
    rects
        .iter()
        .map(|r| match how {
            Align::Left => (b.x, r.y),
            Align::CenterX => (b.center().x - r.w * 0.5, r.y),
            Align::Right => (b.right() - r.w, r.y),
            Align::Top => (r.x, b.y),
            Align::CenterY => (r.x, b.center().y - r.h * 0.5),
            Align::Bottom => (r.x, b.bottom() - r.h),
        })
        .collect()
}

/// 等距分布后的左上角，顺序与输入一致。
///
/// 按中心坐标排序（同中心按原下标稳定），保留首尾位置，把剩余空间均分成 `n-1` 个
/// **盒间空隙**。空隙允许为负（盒子重叠时就是负的），但首尾永远不会被挪动。
pub fn distribute(rects: &[Rect], axis: Distribute) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = rects.iter().map(|r| (r.x, r.y)).collect();
    if rects.len() < 3 {
        return out;
    }
    let mut order: Vec<usize> = (0..rects.len()).collect();
    order.sort_by(|&a, &b| {
        let (ra, rb) = (&rects[a], &rects[b]);
        let (ca, cb) = match axis {
            Distribute::Horizontal => (ra.center().x, rb.center().x),
            Distribute::Vertical => (ra.center().y, rb.center().y),
        };
        ca.partial_cmp(&cb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let first = order[0];
    let last = *order.last().expect("非空");
    let (start, end, total) = match axis {
        Distribute::Horizontal => (
            rects[first].x,
            rects[last].right(),
            order.iter().map(|&i| rects[i].w).sum::<f32>(),
        ),
        Distribute::Vertical => (
            rects[first].y,
            rects[last].bottom(),
            order.iter().map(|&i| rects[i].h).sum::<f32>(),
        ),
    };
    let gap = (end - start - total) / (order.len() - 1) as f32;
    let mut cursor = start;
    for &i in &order {
        match axis {
            Distribute::Horizontal => {
                out[i].0 = cursor;
                cursor += rects[i].w + gap;
            }
            Distribute::Vertical => {
                out[i].1 = cursor;
                cursor += rects[i].h + gap;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects() -> Vec<Rect> {
        vec![
            Rect::new(0.0, 10.0, 20.0, 10.0),
            Rect::new(40.0, 30.0, 30.0, 10.0),
            Rect::new(100.0, 50.0, 10.0, 20.0),
        ]
    }

    #[test]
    fn align_uses_the_selection_bounds_on_the_chosen_edge() {
        let rs = rects();
        let left = align(&rs, Align::Left);
        assert_eq!(left, vec![(0.0, 10.0), (0.0, 30.0), (0.0, 50.0)]);
        let right = align(&rs, Align::Right);
        for (r, (x, _)) in rs.iter().zip(&right) {
            assert!((x + r.w - 110.0).abs() < 1e-4, "{right:?}");
        }
        let top = align(&rs, Align::Top);
        assert_eq!(top, vec![(0.0, 10.0), (40.0, 10.0), (100.0, 10.0)]);
        let bottom = align(&rs, Align::Bottom);
        for (r, (_, y)) in rs.iter().zip(&bottom) {
            assert!((y + r.h - 70.0).abs() < 1e-4, "{bottom:?}");
        }
        // 中心对齐：每条的「中点」落在包围盒中线上
        let cx = bounds(&rs).unwrap().center().x;
        let centered = align(&rs, Align::CenterX);
        for (r, (x, _)) in rs.iter().zip(&centered) {
            assert!((x + r.w * 0.5 - cx).abs() < 1e-4);
        }
    }

    #[test]
    fn distribute_keeps_the_ends_and_equalizes_gaps() {
        let rs = rects();
        let out = distribute(&rs, Distribute::Horizontal);
        assert_eq!(out[0], (rs[0].x, rs[0].y), "首条不动");
        assert_eq!(out[2], (rs[2].x, rs[2].y), "末条不动");
        // 三条等距：0..20、?..?、100..110；总宽 60，跨度 110 → 空隙 25
        assert!((out[1].0 - 45.0).abs() < 1e-4, "{out:?}");
    }

    #[test]
    fn distribute_vertical_and_short_lists() {
        let rs = rects();
        let out = distribute(&rs, Distribute::Vertical);
        // 首 10..20、末 50..70；总高 40，跨度 60 → 空隙 10
        assert!((out[1].1 - 30.0).abs() < 1e-4, "{out:?}");
        assert_eq!(
            distribute(&rs[..1], Distribute::Horizontal),
            vec![(0.0, 10.0)]
        );
        assert_eq!(
            distribute(&rs[..2], Distribute::Horizontal),
            vec![(0.0, 10.0), (40.0, 30.0)]
        );
    }

    #[test]
    fn empty_input_is_empty_not_a_panic() {
        assert!(align(&[], Align::Left).is_empty());
        assert!(distribute(&[], Distribute::Vertical).is_empty());
    }
}
