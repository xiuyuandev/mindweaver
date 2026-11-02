//! 内容表面的细滚动条：几何是纯函数，绘制与拖动在这里收口。
//!
//! 为什么不用 egui 自带的：0.37 的默认是 `ScrollStyle::floating()`
//! ——休眠态 `dormant_handle_opacity = 0.0`（完全看不见），滚动时才浮出来，
//! 而且**盖在文字上**、位置跟着 `ScrollArea` 的宽度走（卡片页里就是版心右缘，
//! 而不是窗口右缘）。`docs/28` §4.5 要的是「6px 宽、4px 拇指、圆角 8、
//! 默认透明、悬停/滚动 120ms 淡入、空闲 300ms 淡出」，所以自己画。
//!
//! `start.rs` 与 `menu.rs` 各有一份同类实现（常量与手感略有分歧）。
//! 这里先服务卡片页，口径与 `mw_core::design::content::scrollbar` 对齐；
//! 迁移那两处属独立改动，不在本包范围内动它们。

use eframe::egui::{self, Color32, Rect, Sense, Vec2};
use mw_core::design::content::scrollbar as token;

/// 一条滚动条的几何。`track` 是滑块能走的整段，`thumb` 是当前滑块。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub track: Rect,
    pub thumb: Rect,
}

/// 视觉参数。默认值取 `design::content::scrollbar` 的令牌。
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    /// 轨道宽（滑块所在那条竖带的宽度）
    pub width: f32,
    /// 滑块宽（比轨道窄，居中放）
    pub thumb: f32,
    pub radius: f32,
    /// 滑块最短长度：太短就抓不住
    pub min_len: f32,
    /// 轨道离视口右边缘的距离
    pub inset: f32,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            width: token::WIDTH,
            thumb: token::THUMB,
            radius: token::RADIUS,
            min_len: 28.0,
            inset: 4.0,
        }
    }
}

/// 内容比视口高出 1px 以上才算可滚动，返回最大偏移。
///
/// 留这 1px 余量是为了躲开像素取整：内容刚好等于视口高时，`content_h`
/// 常常是 719.6 这种数，硬判 `>` 会让滑块忽隐忽现。
pub fn overflow(content_h: f32, viewport_h: f32) -> Option<f32> {
    // `is_nan` 单独挡一道：`f32::NAN <= 0.0` 是 false，光写 `<= 0.0` 会让
    // NaN 混过这道门、在后面变成 NaN 坐标画到屏幕上。
    if !content_h.is_finite() || viewport_h.is_nan() || viewport_h <= 0.0 {
        return None;
    }
    let max = content_h - viewport_h;
    (max > 1.0).then_some(max)
}

/// 滑块的归一化长度 `0..=1`（相对轨道）。
pub fn thumb_len(content_h: f32, viewport_h: f32, min_len: f32) -> Option<f32> {
    overflow(content_h, viewport_h)?;
    let min = (min_len / viewport_h).clamp(0.0, 1.0);
    Some((viewport_h / content_h).clamp(min, 1.0))
}

/// 给定视口与滚动位置，算出轨道与滑块。
///
/// `viewport` 决定轨道的上下端点与右缘；滑块贴着右缘往里收 `inset`。
pub fn layout(viewport: Rect, content_h: f32, offset: f32, spec: Spec) -> Option<Bar> {
    let len = thumb_len(content_h, viewport.height(), spec.min_len)?;
    let track = Rect::from_min_max(
        egui::pos2(viewport.right() - spec.inset - spec.width, viewport.top()),
        egui::pos2(viewport.right() - spec.inset, viewport.bottom()),
    );
    if track.height() <= 0.0 {
        return None;
    }
    let thumb_h = (len * track.height()).max(spec.min_len.min(track.height()));
    let travel = (track.height() - thumb_h).max(0.0);
    let max = overflow(content_h, viewport.height())?;
    let t = (offset / max).clamp(0.0, 1.0);
    let thumb = Rect::from_min_size(
        egui::pos2(
            track.center().x - spec.thumb * 0.5,
            track.top() + t * travel,
        ),
        Vec2::new(spec.thumb, thumb_h),
    );
    Some(Bar { track, thumb })
}

/// 拖到某个滑块顶边后反算滚动偏移。
pub fn offset_for_thumb_top(
    top: f32,
    track: Rect,
    thumb_h: f32,
    content_h: f32,
    viewport_h: f32,
) -> f32 {
    let Some(max) = overflow(content_h, viewport_h) else {
        return 0.0;
    };
    let travel = track.height() - thumb_h;
    if travel <= f32::EPSILON {
        return 0.0;
    }
    let t = ((top - track.top()) / travel).clamp(0.0, 1.0);
    t * max
}

/// 一帧的淡入淡出状态。`alpha` 是当前不透明度（0 = 完全透明）。
#[derive(Clone, Copy, Debug, Default)]
pub struct Fade {
    pub alpha: f32,
    /// 最后一次「该亮着」的时刻（egui 时间，秒）
    last_active: f64,
}

impl Fade {
    /// 推进一帧。`hot` = 指针在条上 / 正在拖 / 刚滚过。
    ///
    /// 亮起来用 `fade_in`，灭下去在 `idle_fade` 之后按同样的速度走。
    /// 两个时长都取 `design::content::scrollbar`。
    pub fn advance(&mut self, now: f64, hot: bool, dt: f32) -> f32 {
        if hot {
            self.last_active = now;
        }
        let want = if now - self.last_active <= token::IDLE_FADE as f64 {
            1.0
        } else {
            0.0
        };
        let step = (dt.max(0.0) / token::FADE_IN).clamp(0.0, 1.0);
        if self.alpha < want {
            self.alpha = (self.alpha + step).min(want);
        } else if self.alpha > want {
            self.alpha = (self.alpha - step).max(want);
        }
        self.alpha
    }
}

/// 一次绘制的结果。
#[derive(Clone, Copy, Debug, Default)]
pub struct Output {
    /// 这一帧请求的滚动偏移（拖动时才有）。
    pub request: Option<f32>,
    /// 指针在条上或正在拖——调用方据此把淡入续上。
    pub hot: bool,
}

/// 画一条已算好的滚动条，并处理拖动。
///
/// **命中区是整个轨道**（左右各放宽 4px），不是那颗 4px 宽的滑块：
/// 休眠态是透明的，只让小滑块能抓等于没法抓。按在滑块上就保住相对位置，
/// 按在轨道的空白处就把滑块居中过去——这是滚动条的通行手感。
///
/// `fill` 的 alpha 就是这一帧的淡入透明度（0 = 不画但**仍登记命中区**，
/// 这样指针扫到窗口右缘还能把它唤出来）。
pub fn paint(
    ui: &mut egui::Ui,
    bar: Bar,
    spec: Spec,
    fill: Color32,
    id: egui::Id,
    content_h: f32,
    viewport_h: f32,
) -> Output {
    let hit = Rect::from_min_max(
        egui::pos2(bar.track.left() - 4.0, bar.track.top()),
        egui::pos2(bar.track.right() + 4.0, bar.track.bottom()),
    );
    let resp = ui.interact(hit, id, Sense::DRAG);
    let grab_id = id.with("grab");

    let dragging = resp.dragged();
    // 抓取点：按在滑块哪一格，拖动时就保住那个相对位置，滑块不会跳到指针中心。
    let grab = if resp.drag_started() {
        let p = resp.interact_pointer_pos();
        let g = p
            .map(|p| {
                if bar.thumb.y_range().contains(p.y) {
                    (p.y - bar.thumb.top()).clamp(0.0, bar.thumb.height())
                } else {
                    bar.thumb.height() * 0.5
                }
            })
            .unwrap_or(bar.thumb.height() * 0.5);
        ui.ctx().data_mut(|d| d.insert_temp(grab_id, g));
        g
    } else {
        ui.ctx()
            .data_mut(|d| d.get_temp::<f32>(grab_id))
            .unwrap_or(bar.thumb.height() * 0.5)
    };

    if fill.a() > 0 {
        ui.painter().rect_filled(bar.thumb, spec.radius, fill);
    }

    let hot = resp.hovered() || dragging;
    if !dragging {
        if resp.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove_temp::<f32>(grab_id));
        }
        return Output { request: None, hot };
    }
    let Some(p) = resp.interact_pointer_pos() else {
        return Output { request: None, hot };
    };
    Output {
        request: Some(offset_for_thumb_top(
            p.y - grab,
            bar.track,
            bar.thumb.height(),
            content_h,
            viewport_h,
        )),
        hot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    fn viewport() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(900.0, 600.0))
    }

    #[test]
    fn nothing_to_scroll_means_no_bar() {
        assert!(overflow(600.0, 600.0).is_none());
        assert!(overflow(600.5, 600.0).is_none(), "1px 内当不溢出");
        assert!(overflow(601.1, 600.0).is_some());
        assert!(overflow(600.0, 0.0).is_none(), "视口高 0 不画");
        assert!(layout(viewport(), 300.0, 0.0, Spec::default()).is_none());
    }

    #[test]
    fn thumb_hugs_the_window_right_edge() {
        let spec = Spec::default();
        let bar = layout(viewport(), 2400.0, 0.0, spec).expect("该有滑块");
        // 轨道右缘 = 视口右缘 − inset；滑块比轨道窄，居中。
        assert!((bar.track.right() - (900.0 - spec.inset)).abs() < 0.01);
        assert!((bar.track.width() - spec.width).abs() < 0.01);
        assert!((bar.thumb.width() - spec.thumb).abs() < 0.01);
        assert!((bar.thumb.center().x - bar.track.center().x).abs() < 0.01);
        // 轨道上下贯通整个视口。
        assert!((bar.track.top() - 0.0).abs() < 0.01);
        assert!((bar.track.bottom() - 600.0).abs() < 0.01);
    }

    #[test]
    fn thumb_tracks_both_ends_and_keeps_a_minimum() {
        let spec = Spec::default();
        let top = layout(viewport(), 2400.0, 0.0, spec).unwrap();
        assert!((top.thumb.top() - top.track.top()).abs() < 0.01, "在顶");
        let max = overflow(2400.0, 600.0).unwrap();
        let bottom = layout(viewport(), 2400.0, max, spec).unwrap();
        assert!(
            (bottom.thumb.bottom() - bottom.track.bottom()).abs() < 0.01,
            "在底"
        );
        // 极长内容：归一化长度会小到看不出，但滑块不许短过 min_len。
        let tiny = layout(viewport(), 60_000.0, 0.0, spec).unwrap();
        assert!((tiny.thumb.height() - spec.min_len).abs() < 0.01);
    }

    #[test]
    fn dragging_maps_back_to_an_offset() {
        let spec = Spec::default();
        let bar = layout(viewport(), 2400.0, 0.0, spec).unwrap();
        let max = overflow(2400.0, 600.0).unwrap();
        // 拖到最底：偏移必须是内容高 − 视口高。
        let at_bottom = offset_for_thumb_top(
            bar.track.bottom(),
            bar.track,
            bar.thumb.height(),
            2400.0,
            600.0,
        );
        assert!(
            (at_bottom - max).abs() < 0.01,
            "实测 {at_bottom}，期望 {max}"
        );
        // 拖到最顶：回到 0。
        let at_top = offset_for_thumb_top(
            bar.track.top(),
            bar.track,
            bar.thumb.height(),
            2400.0,
            600.0,
        );
        assert!(at_top.abs() < 0.01);
        // 不可滚动时钳成 0，不产生 NaN。
        assert_eq!(
            offset_for_thumb_top(100.0, bar.track, bar.thumb.height(), 300.0, 600.0),
            0.0
        );
    }

    #[test]
    fn fade_comes_in_and_goes_out_after_idle() {
        let mut f = Fade::default();
        // 一上来是透明的：进页面不该先闪一条灰线。
        assert_eq!(f.alpha, 0.0);
        // 连续「热」：0.12s 淡入，一帧一帧走，不跳变。
        let mut t = 0.0;
        for _ in 0..20 {
            t += 0.016;
            f.advance(t, true, 0.016);
        }
        assert!(f.alpha > 0.99, "热了 0.32s 该全亮，实测 {}", f.alpha);
        // 松手后先保持一会儿（IDLE 0.30s），再淡出。
        f.advance(t + 0.1, false, 0.016);
        assert!(f.alpha > 0.99, "空闲不足 0.3s 不该开始淡");
        let mut t2 = t + 0.35;
        for _ in 0..30 {
            t2 += 0.016;
            f.advance(t2, false, 0.016);
        }
        assert!(f.alpha < 0.01, "空闲够久必须完全透明，实测 {}", f.alpha);
    }
}
