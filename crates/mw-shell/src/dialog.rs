//! 统一模态对话框：暗幕、三段式卡片、焦点陷阱与 Esc 路由。
//!
//! 为什么用 `egui::Modal` 而不是自绘 `Area` + 暗幕：它会把自己的层登记为
//! modal layer，背景控件当帧就不参与命中与焦点顺序，Tab 也拿不到焦点兴趣。
//! 焦点陷阱因此是框架保证的事实，不是每个弹窗各补一遍的补丁。
//!
//! Esc 由壳层每帧**先消费一次**再交给弹窗：画布、菜单、搜索、页面都晚于
//! [`route_escape`]，所以同一帧不会出现「弹窗关了、底下的页面也退了一层」。

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Pos2, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use mw_core::design;

use crate::chrome::{Chrome, Type};

const ESC_KEY: &str = "mw-dialog-escape";

/// Esc 路由结果绑定到 frame，避免上一帧的命中漏到下一帧的叠层弹窗；
/// 同一帧的多 pass 里首个 pass 的命中要保留给 `show`。
#[derive(Clone, Copy)]
struct EscRoute {
    frame: u64,
    hit: bool,
}

/// 弹窗文案与行为。
#[derive(Clone, Copy)]
pub struct DialogSpec<'a> {
    pub title: &'a str,
    /// 期望宽度；实际会夹到 [`dialog_width`] 的安全区间。
    pub width: f32,
    /// `false` 时没有关闭叉，Esc 也不关（导入进度这类不能中途退的弹窗）。
    pub dismissible: bool,
    /// 回车触发 `default_action`。目前只有导出弹窗需要。
    pub enter_confirms: bool,
    /// 默认动作下标（动作按从左到右排列），也是打开时的焦点落点。
    pub default_action: Option<usize>,
}

impl<'a> DialogSpec<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            width: 460.0,
            dismissible: true,
            enter_confirms: false,
            default_action: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    pub fn enter_confirms(mut self, enter_confirms: bool) -> Self {
        self.enter_confirms = enter_confirms;
        self
    }

    pub fn default_action(mut self, index: usize) -> Self {
        self.default_action = Some(index);
        self
    }
}

/// 底部动作按钮。顺序即从左到右；最后一个通常放主操作。
#[derive(Clone, Copy)]
pub struct DialogAction<'a> {
    pub label: &'a str,
    pub tone: crate::ui_kit::ButtonTone,
    pub enabled: bool,
}

impl<'a> DialogAction<'a> {
    pub fn new(label: &'a str, tone: crate::ui_kit::ButtonTone) -> Self {
        Self {
            label,
            tone,
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// 弹窗结果。`action` 是动作下标；`closed` 覆盖 Esc、关闭叉、正文请求关闭与动作点击。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub closed: bool,
    pub action: Option<usize>,
}

/// 每帧开头调用一次：有弹窗时先把 Esc 拿走，别的键盘路径就不会再看见它。
///
/// 内部 Popup 开着时**不拿**：第一下 Esc 归 Popup，第二下才关弹窗。
/// 返回这帧的 Esc 是否属于弹窗；`show` 从临时数据里读同一个值。
pub fn route_escape(ctx: &egui::Context, any_modal: bool) -> bool {
    let id = Id::unique(ESC_KEY);
    let frame = ctx.cumulative_frame_nr();
    if !any_modal {
        ctx.data_mut(|d| d.remove::<EscRoute>(id));
        return false;
    }
    if ctx
        .data(|d| d.get_temp::<EscRoute>(id))
        .is_some_and(|prev| prev.frame == frame)
    {
        return false;
    }
    let hit = !egui::Popup::is_any_open(ctx)
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    ctx.data_mut(|d| d.insert_temp(id, EscRoute { frame, hit }));
    hit
}

/// 弹窗宽度：先夹到令牌区间，再让屏幕窄于 408 时按「屏幕减 48」收缩。
pub fn dialog_width(requested: f32, screen_w: f32) -> f32 {
    requested
        .clamp(
            design::component::DIALOG_MIN_W,
            design::component::DIALOG_MAX_W,
        )
        .min((screen_w - 48.0).max(280.0))
}

/// 动作按钮宽度：文字加左右各 16，且不小于令牌下限。
pub fn action_width(label_width: f32) -> f32 {
    (label_width + 32.0).max(design::component::DIALOG_ACTION_MIN_W)
}

/// 动作按钮从左到右排进按钮区；返回的矩形与 `widths` 一一对应。
pub fn action_layout(right: f32, center_y: f32, widths: &[f32]) -> Vec<Rect> {
    let h = design::component::DIALOG_ACTION_H;
    let gap = design::component::DIALOG_ACTION_GAP;
    let mut cursor = right;
    let mut out = vec![Rect::NOTHING; widths.len()];
    for (i, w) in widths.iter().enumerate().rev() {
        let min = Pos2::new(cursor - w, center_y - h * 0.5);
        out[i] = Rect::from_min_size(min, Vec2::new(*w, h));
        cursor -= w + gap;
    }
    out
}

/// 画一个统一弹窗，返回这帧的决定。
///
/// `id` 必须稳定：焦点记忆、滚动位置与关闭后的焦点恢复都挂在它上面。
pub fn show(
    ctx: &egui::Context,
    ch: Chrome,
    id: &str,
    spec: DialogSpec<'_>,
    actions: &[DialogAction<'_>],
    body: impl FnOnce(&mut egui::Ui),
) -> Outcome {
    let id = Id::unique(("mw-dialog", id));
    let last_shown = id.with("open");
    let first = ctx.data(|d| d.get_temp::<bool>(last_shown)).is_none();
    ctx.data_mut(|d| d.insert_temp(last_shown, true));

    let screen = ctx.content_rect();
    let width = dialog_width(spec.width, screen.width());
    let max_h = (screen.height() - 48.0).max(240.0);
    let body_max = (max_h
        - design::component::DIALOG_HEADER_H
        - design::component::DIALOG_FOOTER_H
        - design::component::DIALOG_PAD_TOP
        - design::component::DIALOG_PAD_BOTTOM)
        .max(80.0);

    let prev_focus = id.with("prev-focus");
    let previous = if first {
        let prev = ctx.memory(|m| m.focused());
        match prev {
            Some(prev) => {
                ctx.data_mut(|d| d.insert_temp(prev_focus, prev));
            }
            None => {
                ctx.data_mut(|d| d.remove::<Id>(prev_focus));
            }
        }
        prev
    } else {
        None
    };

    let mut outcome = Outcome::default();
    // 回车确认要在正文之前消费：文本框里的回车也应该直接导出，与 X16 之前一致。
    let enter = spec.enter_confirms
        && spec
            .default_action
            .is_some_and(|i| actions.get(i).is_some_and(|a| a.enabled))
        && !egui::Popup::is_any_open(ctx)
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let esc = ctx
        .data(|d| d.get_temp::<EscRoute>(Id::unique(ESC_KEY)))
        .is_some_and(|route| route.frame == ctx.cumulative_frame_nr() && route.hit);

    let shadow = design::elevation::DIALOG;
    let frame = egui::Frame::new()
        .fill(ch.panel)
        .stroke(Stroke::new(design::stroke::HAIRLINE, ch.border))
        .corner_radius(egui::CornerRadius::same(design::radius::DIALOG as u8))
        .shadow(egui::epaint::Shadow {
            offset: shadow.offset,
            blur: shadow.blur,
            spread: shadow.spread,
            color: Color32::from_black_alpha(shadow.alpha),
        })
        .inner_margin(egui::Margin::ZERO);
    let modal = egui::Modal::new(id)
        .backdrop_color(Color32::from_black_alpha(design::scrim::ALPHA))
        .frame(frame);

    let body_h_id = id.with("body-h");
    let resp = modal.show(ctx, |ui| {
        ui.set_width(width);
        ui.set_max_width(width);
        // 三段之间不留默认 8px 行距：标题/正文/按钮区的间距由各自内距决定，
        // 否则页脚会被额外推开，卡片比规范高出一截。
        ui.spacing_mut().item_spacing.y = 0.0;
        header(ui, ch, id, &spec, &mut outcome);
        // Modal 的 Area 用上一帧尺寸推本帧可用高度，ScrollArea 若只看 `max_height`
        // 会先把视口压到上一帧卡高，正文最后一行被页脚裁掉。这里把上一帧量到的
        // 内容高当作最小视口（再夹到 body_max），第二帧就收敛；内容超高时仍滚动。
        let known_h = ctx
            .data(|d| d.get_temp::<f32>(body_h_id))
            .unwrap_or(0.0)
            .clamp(0.0, body_max);
        ui.horizontal(|ui| {
            ui.add_space(design::component::DIALOG_PAD_X);
            ui.vertical(|ui| {
                ui.set_width(width - design::component::DIALOG_PAD_X * 2.0);
                ui.add_space(design::component::DIALOG_PAD_TOP);
                ui.spacing_mut().item_spacing.y = design::space::SM;
                let scroll = egui::ScrollArea::vertical()
                    .id_salt(id.with("body"))
                    .max_height(body_max)
                    .min_scrolled_height(known_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(width - design::component::DIALOG_PAD_X * 2.0);
                        body(ui);
                    });
                ctx.data_mut(|d| d.insert_temp(body_h_id, scroll.content_size.y.min(body_max)));
                if first {
                    crate::trace::event(&format!(
                        "dialog id={id:?} body={:?} content={:?}",
                        scroll.inner_rect, scroll.content_size
                    ));
                }
                ui.add_space(design::component::DIALOG_PAD_BOTTOM);
            });
            ui.add_space(design::component::DIALOG_PAD_X);
        });
        // 没有动作的弹窗（「正在打开」）不画空页脚：一条分隔线加 56px 空白
        // 会让人以为按钮还没加载出来。
        if !actions.is_empty() {
            footer(ui, ch, id, actions, &mut outcome);
        }
        // 打开首帧：正文控件（输入框等）没有主动要焦点时，把背景焦点收回弹窗。
        if first {
            let current = ui.ctx().memory(|m| m.focused());
            if current.is_none() || current == previous {
                let target = spec
                    .default_action
                    .filter(|i| actions.get(*i).is_some_and(|a| a.enabled))
                    .or_else(|| actions.iter().position(|a| a.enabled))
                    .map(|i| id.with(("action", i)))
                    .or_else(|| spec.dismissible.then_some(id.with("close")));
                if let Some(target) = target {
                    ui.ctx().memory_mut(|m| m.request_focus(target));
                }
            }
        }
        // egui 的 Tab 越过最后一个控件会先把焦点清空，下一帧才回卷。这里当场接回
        // 第一个可聚焦控件；`interested_in_focus` 同时清掉 egui 留下的 give_to_next，
        // 否则下一帧会悄悄跳到第二个控件。
        let tab_forward = ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none()
                )
            })
        });
        if tab_forward && ui.ctx().memory(|m| m.focused()).is_none() {
            let first_focus = spec.dismissible.then_some(id.with("close")).or_else(|| {
                actions
                    .iter()
                    .position(|a| a.enabled)
                    .map(|i| id.with(("action", i)))
            });
            if let Some(first_focus) = first_focus {
                let layer = ui.layer_id();
                ui.ctx().memory_mut(|m| {
                    m.request_focus(first_focus);
                    m.interested_in_focus(first_focus, layer);
                });
            }
        }
    });
    if first {
        crate::trace::event(&format!(
            "dialog id={id:?} card={:?} top={}",
            resp.response.rect, resp.is_top_modal
        ));
    }
    if esc && spec.dismissible {
        outcome.closed = true;
    }
    if enter {
        outcome.action = spec.default_action;
        outcome.closed = true;
    }
    if outcome.action.is_some() {
        outcome.closed = true;
    }

    if outcome.closed {
        if let Some(prev) = ctx.data(|d| d.get_temp::<Id>(prev_focus)) {
            ctx.memory_mut(|m| m.request_focus(prev));
        }
        ctx.data_mut(|d| d.remove_temp::<bool>(last_shown));
        ctx.data_mut(|d| d.remove_temp::<f32>(body_h_id));
    }
    outcome
}

fn header(ui: &mut egui::Ui, ch: Chrome, id: Id, spec: &DialogSpec<'_>, outcome: &mut Outcome) {
    let h = design::component::DIALOG_HEADER_H;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    ui.painter().text(
        Pos2::new(
            rect.left() + design::component::DIALOG_PAD_X,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        spec.title,
        FontId::new(
            Type::HEADING,
            FontFamily::Name(crate::fonts::SEMIBOLD.into()),
        ),
        ch.text,
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(design::stroke::HAIRLINE, ch.sep),
    );
    if !spec.dismissible {
        return;
    }
    let close = Rect::from_center_size(
        Pos2::new(
            rect.right() - design::space::MD - design::control::ICON_BUTTON * 0.5,
            rect.center().y,
        ),
        Vec2::splat(design::control::ICON_BUTTON),
    );
    let resp = ui
        .interact(close, id.with("close"), Sense::click())
        .on_hover_text("关闭（Esc）");
    if resp.hovered() {
        ui.painter()
            .rect_filled(close, design::radius::XS, ch.hover);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let ink = if resp.hovered() { ch.text } else { ch.dim };
    crate::icon_paint::paint_icon_sized(
        ui.painter(),
        close.center(),
        close.width() * 0.62,
        crate::icons::Icon::Close,
        ink,
    );
    crate::ui_kit::paint_focus_ring(ui.painter(), ch, close, design::radius::XS, &resp);
    if resp.has_focus() {
        crate::trace::name(ui.ctx(), resp.id, "dialog close");
    }
    if activate(ui, &resp) {
        outcome.closed = true;
    }
}

fn footer(
    ui: &mut egui::Ui,
    ch: Chrome,
    id: Id,
    actions: &[DialogAction<'_>],
    outcome: &mut Outcome,
) {
    let h = design::component::DIALOG_FOOTER_H;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(design::stroke::HAIRLINE, ch.sep),
    );
    if actions.is_empty() {
        return;
    }

    let font = FontId::new(Type::BODY, FontFamily::Proportional);
    let widths: Vec<f32> = actions
        .iter()
        .map(|a| {
            let w = ui
                .painter()
                .layout_no_wrap(a.label.to_owned(), font.clone(), ch.text)
                .size()
                .x;
            action_width(w)
        })
        .collect();
    let right = rect.right() - design::component::DIALOG_PAD_X;
    let rects = action_layout(right, rect.center().y, &widths);
    for (i, action) in actions.iter().enumerate() {
        let r = rects[i];
        let sense = if action.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let resp = ui.interact(r, id.with(("action", i)), sense);
        paint_action(ui, ch, r, action, &resp);
        if action.enabled && activate(ui, &resp) {
            outcome.action = Some(i);
        }
        if resp.has_focus() {
            crate::trace::name(
                ui.ctx(),
                resp.id,
                &format!("dialog action {i} · {}", action.label),
            );
        }
    }
}

fn paint_action(ui: &egui::Ui, ch: Chrome, rect: Rect, action: &DialogAction<'_>, resp: &Response) {
    use crate::ui_kit::ButtonTone;
    let enabled = action.enabled;
    let hovered = resp.hovered() && enabled;
    let pressed = resp.is_pointer_button_down_on() && enabled;
    let (fill, ink) = match action.tone {
        ButtonTone::Primary => (
            Some(if !enabled {
                ch.panel2
            } else if pressed {
                ch.accent.gamma_multiply(0.82)
            } else if hovered {
                ch.accent.gamma_multiply(0.92)
            } else {
                ch.accent
            }),
            if enabled { ch.panel } else { ch.dim },
        ),
        ButtonTone::Secondary => (
            Some(if hovered { ch.hover } else { ch.panel2 }),
            if enabled { ch.text } else { ch.dim },
        ),
        ButtonTone::Ghost | ButtonTone::Danger => (
            if hovered { Some(ch.hover) } else { None },
            action.tone.ink(ch, enabled),
        ),
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, design::radius::SM, fill);
    }
    if matches!(action.tone, ButtonTone::Secondary) {
        ui.painter().rect_stroke(
            rect,
            design::radius::SM,
            Stroke::new(design::stroke::HAIRLINE, ch.border),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        action.label,
        FontId::new(Type::BODY, FontFamily::Proportional),
        ink,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    crate::ui_kit::paint_focus_ring(ui.painter(), ch, rect, design::radius::SM, resp);
}

/// 自定义按钮不会自己响应键盘：焦点在按钮上时把 Enter / Space 当一次点击。
fn activate(ui: &egui::Ui, resp: &Response) -> bool {
    resp.clicked()
        || (resp.has_focus()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
            }))
}

/// 演示弹窗：`MW_DIALOG_DEMO=1` 时打开，给 golden 与行为脚本一个稳定的靶子。
pub fn demo(ctx: &egui::Context, ch: Chrome) -> bool {
    let mut open = true;
    let spec = DialogSpec::new("对话框演示").width(440.0).default_action(1);
    let actions = [
        DialogAction::new("取消", crate::ui_kit::ButtonTone::Secondary),
        DialogAction::new("确定", crate::ui_kit::ButtonTone::Primary),
    ];
    let out = show(ctx, ch, "demo", spec, &actions, |ui| {
        ui.label(
            RichText::new("统一标题、内容、按钮区与焦点陷阱。")
                .size(Type::BODY)
                .color(ch.text),
        );
        ui.label(
            RichText::new("Tab 只在弹窗内循环；Esc 关闭；点暗幕不关闭。")
                .size(Type::SMALL)
                .color(ch.dim),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("示例选项").size(Type::SMALL).color(ch.dim));
            let mut on = true;
            let _ = crate::ui_kit::switch(ui, ch, "dialog-demo-switch", &mut on, true);
        });
    });
    if out.closed {
        crate::trace::event("dialog demo closed");
        open = false;
    }
    if let Some(i) = out.action {
        crate::trace::event(&format!("dialog demo action {i}"));
        open = false;
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome() -> Chrome {
        crate::chrome::chrome_of(&mw_core::get_theme("snow"))
    }

    fn run_frame(ctx: &egui::Context, events: Vec<egui::Event>, f: impl FnMut(&mut egui::Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, f).drop_without_applying_deltas();
    }

    #[test]
    fn width_is_clamped_to_tokens_and_screen() {
        assert_eq!(dialog_width(440.0, 1600.0), 440.0, "正常宽度不该被改");
        assert_eq!(dialog_width(200.0, 1600.0), design::component::DIALOG_MIN_W);
        assert_eq!(dialog_width(900.0, 1600.0), design::component::DIALOG_MAX_W);
        assert_eq!(dialog_width(440.0, 380.0), 332.0, "窄屏按屏幕减 48");
        assert!(
            dialog_width(440.0, 200.0) >= 280.0,
            "再窄也要留一条能读的宽度"
        );
    }

    #[test]
    fn actions_tile_right_to_left_without_overlap() {
        let widths = [action_width(40.0), action_width(40.0), action_width(40.0)];
        let rects = action_layout(600.0, 300.0, &widths);
        assert!((rects[2].right() - 600.0).abs() < 0.001);
        for pair in rects.windows(2) {
            let gap = pair[1].left() - pair[0].right();
            assert!(
                (gap - design::component::DIALOG_ACTION_GAP).abs() < 0.001,
                "动作按钮间距不是令牌值"
            );
        }
        for r in rects {
            assert!((r.height() - design::component::DIALOG_ACTION_H).abs() < 0.001);
        }
        assert!(action_width(0.0) >= design::component::DIALOG_ACTION_MIN_W);
    }

    #[test]
    fn escape_consumed_once_per_frame() {
        let ctx = egui::Context::default();
        let key = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut hit = false;
        let mut second = false;
        run_frame(&ctx, vec![key.clone()], |_ui| {
            hit = route_escape(&ctx, true);
            second = route_escape(&ctx, true);
        });
        assert!(hit, "第一次必须拿到 Esc");
        assert!(!second, "同一帧不能被第二条路径再拿一次");

        let mut none = true;
        run_frame(&ctx, vec![key], |_ui| {
            none = route_escape(&ctx, false);
        });
        assert!(!none, "没有弹窗时不能吞 Esc");
    }

    #[test]
    fn escape_does_not_leak_into_the_next_frame_or_lower_dialog() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let spec = DialogSpec {
            title: "叠层",
            width: 380.0,
            dismissible: true,
            enter_confirms: false,
            default_action: None,
        };
        let actions = [DialogAction::new("好", crate::ui_kit::ButtonTone::Primary)];
        let esc = || egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };

        let mut top = Outcome::default();
        run_frame(&ctx, vec![esc()], |_ui| {
            route_escape(&ctx, true);
            top = show(&ctx, ch, "top", spec, &actions, |ui| {
                ui.label("顶层");
            });
        });
        assert!(top.closed, "Esc 应关闭栈顶弹窗");

        // 下一帧：底层弹窗仍在，但这一帧没有新的 Esc，不能被上一帧的命中带走。
        let mut lower = Outcome::default();
        run_frame(&ctx, vec![], |_ui| {
            route_escape(&ctx, true);
            lower = show(&ctx, ch, "lower", spec, &actions, |ui| {
                ui.label("底层");
            });
        });
        assert!(!lower.closed, "上一帧的 Esc 漏进了下一帧的弹窗");
    }

    #[test]
    fn modal_keeps_tab_focus_inside_and_restores_it() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let bg_id = Id::unique("bg-button");
        let close_id = Id::unique(("mw-dialog", "trap")).with("close");
        let action0 = Id::unique(("mw-dialog", "trap")).with(("action", 0));
        let action1 = Id::unique(("mw-dialog", "trap")).with(("action", 1));
        let inside =
            |id: Option<Id>| matches!(id, Some(x) if x == close_id || x == action0 || x == action1);

        // 先把焦点放在背景按钮上，再打开弹窗：背景焦点必须被收进弹窗。
        run_frame(&ctx, vec![], |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = ui.interact(
                    Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(80.0, 24.0)),
                    bg_id,
                    Sense::click(),
                );
                r.request_focus();
                let _ = ui.button("正文按钮");
            });
        });
        let spec = DialogSpec {
            title: "焦点陷阱",
            width: 380.0,
            dismissible: true,
            enter_confirms: false,
            default_action: None,
        };
        let actions = [
            DialogAction::new("取消", crate::ui_kit::ButtonTone::Secondary),
            DialogAction::new("确定", crate::ui_kit::ButtonTone::Primary),
        ];
        let mut outcome = Outcome::default();
        run_frame(&ctx, vec![], |_ui| {
            outcome = show(&ctx, ch, "trap", spec, &actions, |ui| {
                ui.label("正文");
            });
        });
        assert!(!outcome.closed);
        assert!(
            inside(ctx.memory(|m| m.focused())),
            "打开弹窗后背景不能继续握着焦点"
        );

        for _ in 0..6 {
            let tab = egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            run_frame(&ctx, vec![tab], |_ui| {
                let spec = DialogSpec {
                    title: "焦点陷阱",
                    width: 380.0,
                    dismissible: true,
                    enter_confirms: false,
                    default_action: None,
                };
                let _ = show(&ctx, ch, "trap", spec, &actions, |ui| {
                    ui.label("正文");
                });
            });
            let focused = ctx.memory(|m| m.focused());
            assert!(inside(focused), "Tab 跑出了弹窗：{focused:?}");
        }

        for _ in 0..2 {
            let back = egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::SHIFT,
            };
            run_frame(&ctx, vec![back], |_ui| {
                let _ = show(&ctx, ch, "trap", spec, &actions, |ui| {
                    ui.label("正文");
                });
            });
            let focused = ctx.memory(|m| m.focused());
            assert!(inside(focused), "Shift+Tab 跑出了弹窗：{focused:?}");
        }

        // Esc 关闭后，焦点回到打开前的背景按钮。
        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        run_frame(&ctx, vec![esc], |_ui| {
            route_escape(&ctx, true);
            let spec = DialogSpec {
                title: "焦点陷阱",
                width: 380.0,
                dismissible: true,
                enter_confirms: false,
                default_action: None,
            };
            let out = show(&ctx, ch, "trap", spec, &actions, |ui| {
                ui.label("正文");
            });
            assert!(out.closed, "Esc 应关闭可关闭的弹窗");
        });
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(bg_id),
            "关闭后焦点没回到打开前的控件"
        );
    }

    #[test]
    fn non_dismissible_ignores_escape() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let spec = DialogSpec {
            title: "正在导入",
            width: 420.0,
            dismissible: false,
            enter_confirms: false,
            default_action: Some(0),
        };
        let actions = [DialogAction::new(
            "取消导入",
            crate::ui_kit::ButtonTone::Secondary,
        )];
        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut out = Outcome::default();
        run_frame(&ctx, vec![esc], |_ui| {
            route_escape(&ctx, true);
            out = show(&ctx, ch, "busy", spec, &actions, |ui| {
                ui.label("进度");
            });
        });
        assert!(!out.closed, "不可关闭的弹窗不能吃 Esc");
    }

    #[test]
    fn enter_confirms_even_with_text_focus() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let spec = DialogSpec {
            title: "导出",
            width: 460.0,
            dismissible: true,
            enter_confirms: true,
            default_action: Some(1),
        };
        let actions = [
            DialogAction::new("取消", crate::ui_kit::ButtonTone::Secondary),
            DialogAction::new("导出", crate::ui_kit::ButtonTone::Primary),
        ];
        let mut text = String::from("note.md");
        run_frame(&ctx, vec![], |_ui| {
            let _ = show(&ctx, ch, "export-enter", spec, &actions, |ui| {
                let r = ui.text_edit_singleline(&mut text);
                r.request_focus();
            });
        });
        assert!(ctx.text_edit_focused(), "正文输入框应握着焦点");
        let enter = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut out = Outcome::default();
        run_frame(&ctx, vec![enter], |_ui| {
            out = show(&ctx, ch, "export-enter", spec, &actions, |ui| {
                ui.text_edit_singleline(&mut text);
            });
        });
        assert_eq!(out.action, Some(1), "回车应直接确认导出");
        assert!(out.closed);
    }
}
