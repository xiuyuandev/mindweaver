//! 标签条纯逻辑：宽度、省略、拖拽插入槽与重排。
//!
//! 绘制留在 `ui_kit`/`app_methods_2`，这里只放可单测的算术——标签宽度和
//! 插入下标一旦写进绘制闭包，就只能靠截图像素反推，DPI 一变全作废。

use eframe::egui::{MouseWheelUnit, Vec2};
use mw_core::design::component as geo;

/// 标签宽 = 文字 + 脏点 + 关闭格 + 左右各 8px 内距，再夹进 [MIN, MAX]。
pub(crate) fn tab_width(text_w: f32, dirty: bool) -> f32 {
    let dot = if dirty { 12.0 } else { 0.0 };
    (text_w + dot + geo::TAB_CLOSE + geo::TAB_INSET * 2.0).clamp(geo::TAB_MIN_W, geo::TAB_MAX_W)
}

/// 只有横向真的移出 4px 才算开始拖拽：按下时的手抖仍是一次点击。
pub(crate) fn past_drag_threshold(dx: f32) -> bool {
    dx.abs() > 4.0
}

/// 尾部省略：保留能放下的最长前缀 + 「…」，绝不截半个字形。
///
/// `measure` 由调用方传入（真实绘制用字体度量，测试用线性桩），
/// 二分找前缀，保证最坏 O(log n) 次测量而不是逐字重排。
pub(crate) fn fit_label(measure: impl Fn(&str) -> f32, label: &str, max_w: f32) -> String {
    if measure(label) <= max_w {
        return label.to_owned();
    }
    const ELLIPSIS: &str = "…";
    if max_w <= measure(ELLIPSIS) {
        return ELLIPSIS.to_owned();
    }
    let chars: Vec<char> = label.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let mut probe: String = chars[..mid].iter().collect();
        probe.push('…');
        if measure(&probe) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut out: String = chars[..lo].iter().collect();
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('…');
    out
}

/// 标签条横向溢出时，左右边缘渐隐的宽度。
pub(crate) const EDGE_FADE_W: f32 = 20.0;

/// 左右渐隐强度（0 表示该侧没有隐藏内容，0.9 是上限）。
///
/// 标签条只有 34px，4px 滑块会和选中标签的 2px 下划线、1px 分隔线抢同一条边；
/// 这里用渐隐表达「还有内容」，与开始页 40px 渐隐同一种语言。
pub(crate) fn edge_fade_strengths(content_w: f32, viewport_w: f32, offset: f32) -> (f32, f32) {
    if viewport_w <= 0.0 || content_w <= viewport_w + 0.5 {
        return (0.0, 0.0);
    }
    let max_offset = content_w - viewport_w;
    let offset = offset.clamp(0.0, max_offset);
    let left = (offset / EDGE_FADE_W).clamp(0.0, 1.0) * 0.9;
    let right = ((max_offset - offset) / EDGE_FADE_W).clamp(0.0, 1.0) * 0.9;
    (left, right)
}

/// 一帧滚轮/触控板事件合成一个横向位移。
///
/// 每个事件是 `(delta, unit, usable)`；`usable=false` 表示带缩放类修饰键，归别处。
/// 单位必须换算：`Point` 是触控板/高精度设备（已是点），普通鼠标滚轮发 `Line`
/// （一格 ±1），`Page` 按视口高计——与 egui 自己的 `ScrollArea` 同口径，
/// 否则真实鼠标一格只滚 1px。
/// 返回 `None` 表示本帧不滚。**调用点必须在标签循环外**：`scroll_with_delta`
/// 只把增量累进本帧 pass state，循环里调用会按标签数重复累加。
pub(crate) fn wheel_scroll_x(
    events: impl IntoIterator<Item = (Vec2, MouseWheelUnit, bool)>,
    line_scroll_speed: f32,
    page_height: f32,
) -> Option<f32> {
    let mut total = 0.0;
    let mut any = false;
    for (delta, unit, usable) in events {
        if !usable {
            continue;
        }
        any = true;
        let delta = match unit {
            MouseWheelUnit::Point => delta,
            MouseWheelUnit::Line => delta * line_scroll_speed,
            MouseWheelUnit::Page => delta * page_height,
        };
        total += delta.x + delta.y;
    }
    (any && total.abs() > 0.5).then_some(total)
}

/// 指针落在哪个插入槽：0 = 第一个标签之前，n = 最后一个之后。
pub(crate) fn drop_slot(centers: &[f32], x: f32) -> usize {
    centers.iter().filter(|c| **c < x).count()
}

/// 把 `from` 移到插入槽 `slot`（0..=len），返回重排后的新下标。
///
/// 先移除再插入时要扣掉被移除项：从前往后拖，原槽位会左移一格。
pub(crate) fn reorder<T>(items: &mut Vec<T>, from: usize, slot: usize) -> usize {
    if items.is_empty() || from >= items.len() {
        return from;
    }
    let item = items.remove(from);
    let at = if slot > from { slot - 1 } else { slot };
    let at = at.min(items.len());
    items.insert(at, item);
    at
}

/// 重排后原激活项落在哪个下标。
///
/// 激活项不是被拖的那一页时，它也要跟着“让位”：先扣掉来源左侧的位移，
/// 再看插入点是否在它之前。公式错了的表现是“拖完 A，画布突然换成 B”。
pub(crate) fn reorder_active(from: usize, active: usize, slot: usize) -> usize {
    if active == from {
        return if slot > from { slot - 1 } else { slot };
    }
    let mut a = if from < active { active - 1 } else { active };
    let at = if slot > from { slot - 1 } else { slot };
    if at <= a {
        a += 1;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    #[test]
    fn width_clamps_short_and_long_titles() {
        assert_eq!(tab_width(10.0, false), geo::TAB_MIN_W);
        assert_eq!(tab_width(10.0, true), geo::TAB_MIN_W);
        assert_eq!(tab_width(400.0, true), geo::TAB_MAX_W);
        let mid = tab_width(120.0, true);
        assert!((mid - (120.0 + 12.0 + geo::TAB_CLOSE + geo::TAB_INSET * 2.0)).abs() < 0.01);
    }

    #[test]
    fn drag_needs_more_than_four_pixels() {
        assert!(!past_drag_threshold(0.0));
        assert!(!past_drag_threshold(3.9));
        assert!(!past_drag_threshold(-4.0), "恰好 4px 仍是点击");
        assert!(past_drag_threshold(4.1));
        assert!(past_drag_threshold(-8.0));
    }

    #[test]
    fn fit_label_keeps_full_text_when_it_fits() {
        assert_eq!(fit_label(linear, "产品路线图", 100.0), "产品路线图");
        assert_eq!(fit_label(linear, "", 10.0), "");
    }

    #[test]
    fn fit_label_truncates_on_char_boundary_with_ellipsis() {
        let out = fit_label(linear, "产品路线图最终版", 60.0);
        assert!(out.ends_with('…'));
        assert!(linear(&out) <= 60.0, "省略后仍超宽：{out}");
        assert_eq!(out, "产品路线图…");
        // 英文同样不许切半个字符，并去掉截断留下的尾空格。
        let out = fit_label(linear, "product roadmap final", 80.0);
        assert!(out.ends_with('…'));
        assert!(linear(&out) <= 80.0);
        assert!(!out.contains(" …"));
    }

    #[test]
    fn fit_label_degrades_to_a_single_ellipsis() {
        assert_eq!(fit_label(linear, "产品路线图", 5.0), "…");
    }

    #[test]
    fn edge_fades_only_when_horizontally_overflowing() {
        assert_eq!(edge_fade_strengths(600.0, 600.0, 0.0), (0.0, 0.0));
        assert_eq!(
            edge_fade_strengths(600.5, 600.0, 0.0),
            (0.0, 0.0),
            "1px 内不算溢出"
        );
        assert_eq!(
            edge_fade_strengths(900.0, 0.0, 0.0),
            (0.0, 0.0),
            "视口宽为 0 不画"
        );
        let (left, right) = edge_fade_strengths(610.0, 600.0, 0.0);
        assert_eq!(left, 0.0);
        assert!(right > 0.0, "刚溢出时右侧要露头，但不必满强度");
        assert!(right < 0.9);
    }

    #[test]
    fn edge_fades_track_both_ends_and_clamp_overshoot() {
        let (left, right) = edge_fade_strengths(1000.0, 600.0, 0.0);
        assert_eq!(left, 0.0);
        assert!((right - 0.9).abs() < 1e-6);

        let (left, right) = edge_fade_strengths(1000.0, 600.0, 400.0);
        assert!((left - 0.9).abs() < 1e-6);
        assert_eq!(right, 0.0);

        let (left, right) = edge_fade_strengths(1000.0, 600.0, 200.0);
        assert!((left - 0.9).abs() < 1e-6, "中段左侧满强度");
        assert!((right - 0.9).abs() < 1e-6, "中段右侧满强度");

        let (left, right) = edge_fade_strengths(1000.0, 600.0, 9999.0);
        assert!((left - 0.9).abs() < 1e-6, "越界 offset 夹到底");
        assert_eq!(right, 0.0, "滚到底后右侧不许再冒渐隐");

        let (left, right) = edge_fade_strengths(1000.0, 600.0, -50.0);
        assert_eq!(left, 0.0, "越界负 offset 夹回顶部");
        assert!((right - 0.9).abs() < 1e-6);
    }

    const LINE_SPEED: f32 = 40.0;
    const PAGE_H: f32 = 460.0;

    fn ev(dx: f32, dy: f32, unit: MouseWheelUnit, usable: bool) -> (Vec2, MouseWheelUnit, bool) {
        (Vec2::new(dx, dy), unit, usable)
    }

    #[test]
    fn wheel_scroll_sums_a_frame_once_and_drops_zoom_modified_events() {
        let empty = Vec::<(Vec2, MouseWheelUnit, bool)>::new();
        assert_eq!(wheel_scroll_x(empty, LINE_SPEED, PAGE_H), None);
        assert_eq!(
            wheel_scroll_x(
                [ev(0.0, -100.0, MouseWheelUnit::Point, false)],
                LINE_SPEED,
                PAGE_H
            ),
            None,
            "Ctrl/Alt/Win 组合归缩放，不滚标签条"
        );
        assert_eq!(
            wheel_scroll_x(
                [ev(0.0, -0.5, MouseWheelUnit::Point, true)],
                LINE_SPEED,
                PAGE_H
            ),
            None,
            "0.5px 内是抖动"
        );
        assert_eq!(
            wheel_scroll_x(
                [ev(0.0, -100.0, MouseWheelUnit::Point, true)],
                LINE_SPEED,
                PAGE_H
            ),
            Some(-100.0),
            "触控板 Point 原样"
        );
        assert_eq!(
            wheel_scroll_x(
                [ev(-40.0, 0.0, MouseWheelUnit::Point, true)],
                LINE_SPEED,
                PAGE_H
            ),
            Some(-40.0),
            "触控板横滑走 dx"
        );
        assert_eq!(
            wheel_scroll_x(
                [
                    ev(0.0, -60.0, MouseWheelUnit::Point, true),
                    ev(-40.0, 0.0, MouseWheelUnit::Point, true)
                ],
                LINE_SPEED,
                PAGE_H
            ),
            Some(-100.0),
            "同一帧多个事件只合成一个位移"
        );
        assert_eq!(
            wheel_scroll_x(
                [
                    ev(0.0, -60.0, MouseWheelUnit::Point, true),
                    ev(0.0, -60.0, MouseWheelUnit::Point, false)
                ],
                LINE_SPEED,
                PAGE_H
            ),
            Some(-60.0)
        );
    }

    #[test]
    fn wheel_scroll_normalizes_line_and_page_units_before_the_jitter_filter() {
        assert_eq!(
            wheel_scroll_x(
                [ev(0.0, -3.0, MouseWheelUnit::Line, true)],
                LINE_SPEED,
                PAGE_H
            ),
            Some(-120.0),
            "真实鼠标 3 格 = 3 × 40px，而不是 3px"
        );
        assert_eq!(
            wheel_scroll_x(
                [ev(0.0, -1.0, MouseWheelUnit::Page, true)],
                LINE_SPEED,
                PAGE_H
            ),
            Some(-460.0),
            "Page 与 egui ScrollArea 同口径：乘视口高"
        );
        assert_eq!(
            wheel_scroll_x(
                [ev(0.0, -0.01, MouseWheelUnit::Line, true)],
                LINE_SPEED,
                PAGE_H
            ),
            None,
            "换算后 0.4px 仍按抖动丢弃"
        );
    }

    #[test]
    fn drop_slot_covers_start_middle_and_end() {
        let centers = [50.0, 150.0, 250.0];
        assert_eq!(drop_slot(&centers, 0.0), 0);
        assert_eq!(drop_slot(&centers, 50.0), 0);
        assert_eq!(drop_slot(&centers, 120.0), 1);
        assert_eq!(drop_slot(&centers, 200.0), 2);
        assert_eq!(drop_slot(&centers, 999.0), 3);
        assert_eq!(drop_slot(&[], 10.0), 0);
    }

    #[test]
    fn reorder_returns_the_new_active_index() {
        let mut v = vec!["a", "b", "c"];
        assert_eq!(reorder(&mut v, 0, 3), 2);
        assert_eq!(v, ["b", "c", "a"]);

        let mut v = vec!["a", "b", "c"];
        assert_eq!(reorder(&mut v, 2, 0), 0);
        assert_eq!(v, ["c", "a", "b"]);

        let mut v = vec!["a", "b", "c"];
        assert_eq!(reorder(&mut v, 1, 1), 1);
        assert_eq!(v, ["a", "b", "c"]);
        assert_eq!(reorder(&mut v, 1, 2), 1, "拖到自己的右槽等于没动");
        assert_eq!(v, ["a", "b", "c"]);

        let mut v = vec!["a", "b", "c"];
        assert_eq!(reorder(&mut v, 1, 9), 2, "槽位越界夹到末尾");
        assert_eq!(v, ["a", "c", "b"]);
        assert_eq!(reorder(&mut v, 9, 0), 9, "非法来源原样返回");
    }

    #[test]
    fn active_follows_a_tab_that_is_not_being_dragged() {
        // 拖 0 → 末尾：原来的 1、2 各左移一格，激活的 2 变 1。
        assert_eq!(reorder_active(0, 2, 3), 1);
        // 拖 2 → 最前：原来的 0、1 各右移一格，激活的 0 变 1。
        assert_eq!(reorder_active(2, 0, 0), 1);
        // 激活项自己：落到移除后的实际槽位。
        assert_eq!(reorder_active(1, 1, 3), 2);
        assert_eq!(reorder_active(1, 1, 0), 0);
        // 拖到激活项右侧但不越过它：激活项不动。
        assert_eq!(reorder_active(0, 2, 2), 2);
        // 与原数组一致：重排后按下标取到的仍是同一份文档。
        let mut v = vec!["a", "b", "c", "d"];
        let active_before = 3;
        let new_active = reorder_active(0, active_before, 4);
        let moved = reorder(&mut v, 0, 4);
        assert_eq!(moved, 3);
        assert_eq!(v[new_active], "d");
    }
}
