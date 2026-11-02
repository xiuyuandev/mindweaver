//! 壳层组件库 v2。
//!
//! 按钮、胶囊、工具按钮、分段、开关、输入和下拉都在这里收口：几何读 `design::component`，
//! 状态色读主题，调用方只传语义（tone / enabled / selected），不再各画各的圆角和悬停色。
//!
//! 模态壳在 [`crate::dialog`]；这里只提供它用到的色阶与焦点环。

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Margin, Pos2, Rect, Response, RichText, Sense,
    Stroke, StrokeKind, Vec2,
};
use mw_core::design;

use crate::chrome::{Chrome, Tone, Type};
use crate::icons::Icon;

/// 文字按钮语义色：主操作、次操作、平面、危险。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonTone {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

impl ButtonTone {
    /// 按钮文字的墨色；禁用态统一走 `faint`，语义色只在可点时出现。
    pub fn ink(self, ch: Chrome, enabled: bool) -> Color32 {
        if !enabled {
            return ch.faint;
        }
        match self {
            Self::Primary => ch.panel,
            Self::Secondary => ch.text,
            Self::Ghost => ch.dim,
            Self::Danger => ch.warn,
        }
    }
}

/// 在两段线的交点上画一个矢量叉，供各处「关闭 / 删除」小按钮复用。
pub fn paint_x(painter: &egui::Painter, c: egui::Pos2, d: f32, stroke: Stroke) {
    painter.line_segment(
        [c + egui::Vec2::new(-d, -d), c + egui::Vec2::new(d, d)],
        stroke,
    );
    painter.line_segment(
        [c + egui::Vec2::new(d, -d), c + egui::Vec2::new(-d, d)],
        stroke,
    );
}

/// 紧凑图标按钮：命中区 `size`，图标占 62%，悬停给淡底、正文色与手型。
///
/// 所有小控件（关闭、加号、更多、箭头）都走这里，不再各画各的线。
pub fn icon_button(
    ui: &mut egui::Ui,
    ch: Chrome,
    size: f32,
    icon: crate::icons::Icon,
    tip: &str,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, design::radius::XS, ch.hover);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let color = if resp.hovered() { ch.text } else { ch.dim };
    crate::icon_paint::paint_icon_sized(ui.painter(), rect.center(), size * 0.62, icon, color);
    resp.on_hover_text(tip).clicked()
}

/// 16–20px 的关闭叉：悬停给淡底和手型，字符「×」在 125% 缩放下会偏半像素。
pub fn close_x(ui: &mut egui::Ui, ch: Chrome, size: f32, tip: &str) -> bool {
    icon_button(ui, ch, size, crate::icons::Icon::Close, tip)
}

/// 标签页的两块命中区：本体（切换/拖拽）与关闭格。
pub struct TabResponse {
    pub body: Response,
    pub close: Response,
}

/// 标签页：选中态强调淡底 + 2px 下划线，未选中只给悬停底色。
///
/// 宽度由 [`crate::tab_strip::tab_width`] 夹取；超宽标题走尾部省略，关闭格固定在
/// 右侧，文字永远不会压到关闭叉上。脏点用实心圆点，不用字形。
pub fn tab(ui: &mut egui::Ui, ch: Chrome, label: &str, dirty: bool, selected: bool) -> TabResponse {
    let font = FontId::proportional(Type::SMALL);
    let measure = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), ch.text)
            .size()
            .x
    };
    let inset = design::component::TAB_INSET;
    let dot_w = if dirty { 12.0 } else { 0.0 };
    let max_text =
        design::component::TAB_MAX_W - dot_w - design::component::TAB_CLOSE - inset * 2.0;
    let text = if measure(label) > max_text {
        crate::tab_strip::fit_label(measure, label, max_text)
    } else {
        label.to_owned()
    };
    let width = crate::tab_strip::tab_width(measure(&text), dirty);
    let (rect, body) = ui.allocate_exact_size(
        Vec2::new(width, design::component::TAB_H),
        Sense::click_and_drag(),
    );
    let radius = design::component::TAB_RADIUS;
    let close_size = design::component::TAB_CLOSE;
    let close_rect = Rect::from_center_size(
        Pos2::new(rect.right() - inset - close_size * 0.5, rect.center().y),
        Vec2::splat(close_size),
    );
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if selected {
            p.rect_filled(rect, corner_top(radius), ch.accent_soft);
        } else if body.hovered() {
            p.rect_filled(rect, corner_top(radius), ch.hover);
        }
        paint_focus_ring(p, ch, rect, radius, &body);
        if selected {
            let underline = Rect::from_min_max(
                Pos2::new(rect.left() + 8.0, rect.bottom() - 2.0),
                Pos2::new(rect.right() - 8.0, rect.bottom()),
            );
            p.rect_filled(underline, egui::CornerRadius::ZERO, ch.accent);
        }
        let mut ink_left = rect.left() + inset;
        if dirty {
            p.circle_filled(Pos2::new(ink_left + 3.0, rect.center().y), 3.0, ch.warn);
            ink_left += 12.0;
        }
        let color = if selected || body.hovered() {
            ch.text
        } else {
            ch.dim
        };
        let text_clip = Rect::from_min_max(
            Pos2::new(ink_left, rect.top()),
            Pos2::new(close_rect.left() - 3.0, rect.bottom()),
        )
        .intersect(p.clip_rect());
        p.with_clip_rect(text_clip).text(
            Pos2::new(ink_left, rect.center().y),
            Align2::LEFT_CENTER,
            &text,
            font,
            color,
        );
    }
    if body.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    body.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), selected, &text)
    });

    let close = ui.interact(close_rect, body.id.with("close"), Sense::click());
    if ui.is_rect_visible(close_rect) {
        if close.hovered() {
            ui.painter()
                .rect_filled(close_rect, corner(design::radius::XS), ch.hover);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        paint_focus_ring(ui.painter(), ch, close_rect, design::radius::XS, &close);
        let ink = if close.hovered() || selected {
            ch.text
        } else {
            ch.dim
        };
        crate::icon_paint::paint_icon_sized(
            ui.painter(),
            close_rect.center(),
            14.0,
            Icon::Close,
            ink,
        );
    }
    close.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "关闭标签页")
    });

    TabResponse { body, close }
}

fn corner_top(radius: f32) -> egui::CornerRadius {
    let r = radius.round().clamp(0.0, 255.0) as u8;
    egui::CornerRadius {
        nw: r,
        ne: r,
        sw: 0,
        se: 0,
    }
}

/// 与 [`close_x`] 同尺寸的加号，用于「新建标签页」这类紧邻的小按钮。
pub fn plus(ui: &mut egui::Ui, ch: Chrome, size: f32, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, design::radius::XS, ch.hover);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    paint_focus_ring(ui.painter(), ch, rect, design::radius::XS, &resp);
    let color = if resp.hovered() { ch.text } else { ch.dim };
    crate::icon_paint::paint_icon_sized(ui.painter(), rect.center(), size * 0.5, Icon::Plus, color);
    resp.on_hover_text(tip)
}

fn corner(radius: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8)
}

/// 自定义绘制控件不会自动获得键盘激活：焦点在控件上时，
/// 把 Enter / Space 按一次主键点击处理。文本框握着键盘时不抢。
pub fn activated(ui: &egui::Ui, resp: &Response) -> bool {
    let keyboard = resp.has_focus()
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.input_mut(|i| {
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
        });
    resp.clicked() || keyboard
}

/// 焦点环：键盘用户得看得见自己在哪；只在真正有焦点时出现，不占常态视觉。
pub fn paint_focus_ring(
    painter: &egui::Painter,
    ch: Chrome,
    rect: Rect,
    radius: f32,
    resp: &Response,
) {
    if resp.has_focus() {
        painter.rect_stroke(
            rect.expand(design::component::FOCUS_RING_OFFSET),
            corner(radius + 1.0),
            Stroke::new(design::component::FOCUS_RING, ch.accent),
            StrokeKind::Outside,
        );
    }
}

/// 文字按钮：主/次/平面/危险四态，尺寸与圆角读组件令牌。
pub fn text_button(
    ui: &mut egui::Ui,
    ch: Chrome,
    label: &str,
    tone: ButtonTone,
    enabled: bool,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(
        text_button_size(ui, ch, label, tone, enabled),
        Sense::hover(),
    );
    text_button_at(ui, rect, ch, label, tone, enabled)
}

/// 一颗文字按钮要多宽多高（按当前字体重排一次，标点随字号走）。
pub fn text_button_size(
    ui: &egui::Ui,
    ch: Chrome,
    label: &str,
    tone: ButtonTone,
    enabled: bool,
) -> Vec2 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::new(Type::BODY, FontFamily::Proportional),
        tone.ink(ch, enabled),
    );
    let pad = design::component::BUTTON_PAD;
    galley.size() + Vec2::new(pad[0], pad[1]) * 2.0
}

/// 在**给定矩形**里画一颗文字按钮并接管命中测试：不占布局。
///
/// 为什么要有这个：开始页「清除列表」的视觉位置在区段标题行右边，但 `docs/30` §3.5 的
/// Tab 序要求它排在最近文件行之后。egui 的 Tab 序跟**注册顺序**走，注册顺序又跟布局走——
/// 所以标题行里只按 `text_button_size` 占一块地（`Sense::hover`，不算控件），
/// 行与卡片都画完之后再到这儿 `interact` 一次：位置归布局、顺序归注册，各归各位。
/// 用 `scope_builder` 原地重开一格不行——那格会在父布局里再占一份高度，
/// 整页下面的所有段落会被推下去（SE-03 第一版就是这么漂的）。
pub fn text_button_at(
    ui: &mut egui::Ui,
    rect: Rect,
    ch: Chrome,
    label: &str,
    tone: ButtonTone,
    enabled: bool,
) -> Response {
    let color = tone.ink(ch, enabled);
    let font = FontId::new(Type::BODY, FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let galley_size = galley.size();
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(rect, ui.next_auto_id(), sense);
    let bg = if !enabled {
        Color32::TRANSPARENT
    } else {
        match tone {
            ButtonTone::Primary => {
                if resp.is_pointer_button_down_on() {
                    ch.accent.gamma_multiply(0.82)
                } else if resp.hovered() {
                    ch.accent.gamma_multiply(0.92)
                } else {
                    ch.accent
                }
            }
            ButtonTone::Secondary => {
                if resp.hovered() {
                    ch.hover
                } else {
                    ch.panel2
                }
            }
            ButtonTone::Ghost | ButtonTone::Danger => {
                if resp.hovered() {
                    ch.hover
                } else {
                    Color32::TRANSPARENT
                }
            }
        }
    };
    if bg != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, corner(design::component::BUTTON_RADIUS), bg);
    }
    ui.painter()
        .galley(rect.center() - galley_size * 0.5, galley, color);
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// 浅色画布上的描边主按钮（开始页「返回文档」）：带一支左箭头。
pub fn outline_button(ui: &mut egui::Ui, label: &str, tone: Tone) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(Type::BODY),
        tone.ink,
    );
    let w = galley.size().x + 34.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(w, design::component::OUTLINE_HEIGHT),
        Sense::click(),
    );
    let hover = resp.hovered();
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().rect(
        rect,
        corner(design::component::OUTLINE_RADIUS),
        if hover { tone.hover } else { tone.card },
        Stroke::new(design::stroke::HAIRLINE, tone.stroke),
        StrokeKind::Inside,
    );
    let c = Pos2::new(rect.left() + 16.0, rect.center().y);
    let s = Stroke::new(1.3, tone.muted);
    ui.painter()
        .line_segment([c + Vec2::new(3.5, -4.0), c + Vec2::new(-0.5, 0.0)], s);
    ui.painter()
        .line_segment([c + Vec2::new(-0.5, 0.0), c + Vec2::new(3.5, 4.0)], s);
    ui.painter()
        .line_segment([c + Vec2::new(-0.5, 0.0), c + Vec2::new(8.0, 0.0)], s);
    ui.painter().text(
        Pos2::new(rect.left() + 28.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(Type::BODY),
        tone.ink,
    );
    resp
}

/// 工具按钮的语义状态：`active` 同时承担「当前工具」和「菜单已展开」。
#[derive(Clone, Copy)]
pub struct ToolButtonState<'a> {
    pub active: bool,
    pub enabled: bool,
    pub tip: &'a str,
}

/// 28×28 工具按钮，命中区 32：画布上方/大纲页头共用。
///
/// 弹层本身仍由调用方挂到返回的 Response 上。
pub fn tool_button(
    ui: &mut egui::Ui,
    ch: Chrome,
    rect: Rect,
    id: egui::Id,
    icon: Icon,
    state: ToolButtonState<'_>,
) -> Response {
    let ToolButtonState {
        active,
        enabled,
        tip,
    } = state;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let hit = Rect::from_center_size(
        rect.center(),
        Vec2::splat(rect.width().max(design::component::TOOL_HIT)),
    );
    let resp = ui.interact(hit, id, sense);
    if enabled && (resp.hovered() || active) {
        let fill = if resp.is_pointer_button_down_on() || active {
            ch.accent_soft
        } else {
            ch.hover
        };
        ui.painter()
            .rect_filled(rect, corner(design::radius::SM), fill);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let ink = if active {
        ch.accent
    } else if !enabled {
        ch.icon_off
    } else {
        ch.text
    };
    crate::icon_paint::paint_icon_sized(ui.painter(), rect.center(), design::icon::SIZE, icon, ink);
    paint_focus_ring(ui.painter(), ch, rect, design::radius::SM, &resp);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

/// 分段 / 开关选中块的颜色：画法统一，色板由调用方给（检查器与大纲各有语义）。
#[derive(Clone, Copy)]
pub struct ChipPaint {
    pub selected: bool,
    pub fill: Color32,
    pub ring: Color32,
    pub ink: Color32,
    /// 混合态：左上角画一个小点。
    pub mixed: bool,
    /// 伪粗体：再描一遍文字，egui 默认字体没有粗体族。
    pub bold: bool,
}

/// 自绘芯片按钮：检查器的分段、B/I 开关共用。返回是否被点击或键盘激活。
pub fn chip(
    ui: &mut egui::Ui,
    ch: Chrome,
    rect: Rect,
    resp: &Response,
    label: &str,
    font: FontId,
    paint: ChipPaint,
) -> bool {
    let ChipPaint {
        selected,
        fill,
        ring,
        ink,
        mixed,
        bold,
    } = paint;
    {
        let p = ui.painter();
        let radius = design::component::BUTTON_RADIUS;
        if selected {
            p.rect_filled(rect, corner(radius), fill);
            p.rect_stroke(
                rect,
                corner(radius),
                Stroke::new(design::stroke::HAIRLINE, ring),
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            p.rect_filled(rect, corner(radius), ch.hover);
        }
        paint_focus_ring(p, ch, rect, radius, resp);
        let color = if selected { ink } else { ch.text };
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            color,
        );
        if bold {
            p.text(
                rect.center() + Vec2::new(0.7, 0.0),
                Align2::CENTER_CENTER,
                label,
                font,
                color,
            );
        }
        if mixed {
            p.circle_filled(
                Pos2::new(rect.left() + 5.0, rect.top() + 5.0),
                1.6,
                ch.faint,
            );
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    activated(ui, resp)
}

/// 分段控件：等宽选项组，选中块用面板色而不是强调色，避免整条都在喊。
///
/// 返回 `(被点中的序号, 整组悬停响应)`；选中项自身不再响应点击。
/// 分段控件：CER-04 移除大纲视图分段后暂无页面消费者；通用组件保留。
#[allow(dead_code)]
pub fn segmented(
    ui: &mut egui::Ui,
    ch: Chrome,
    rect: Rect,
    id_salt: &str,
    options: &[&str],
    selected: usize,
) -> (Option<usize>, Response) {
    if options.is_empty() {
        return (
            None,
            ui.interact(rect, egui::Id::unique((id_salt, "group")), Sense::hover()),
        );
    }
    let radius = design::component::SEGMENT_RADIUS;
    ui.painter().rect_filled(rect, corner(radius), ch.panel2);
    ui.painter().rect_stroke(
        rect,
        corner(radius),
        Stroke::new(design::stroke::HAIRLINE, ch.border),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(design::component::SEGMENT_GAP);
    let mut clicked = None;
    for (i, (label, seg)) in options
        .iter()
        .zip(segment_rects(inner, options.len()))
        .enumerate()
    {
        let font = FontId::proportional(Type::BODY);
        if i == selected {
            ui.painter().rect_filled(
                seg,
                corner(design::component::SEGMENT_INNER_RADIUS),
                ch.panel,
            );
            ui.painter().rect_stroke(
                seg,
                corner(design::component::SEGMENT_INNER_RADIUS),
                Stroke::new(design::stroke::HAIRLINE, ch.border_hot),
                StrokeKind::Inside,
            );
            ui.painter()
                .text(seg.center(), Align2::CENTER_CENTER, *label, font, ch.text);
            continue;
        }
        let resp = ui.interact(seg, egui::Id::unique((id_salt, i)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(
                seg,
                corner(design::component::SEGMENT_INNER_RADIUS),
                ch.hover,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        paint_focus_ring(
            ui.painter(),
            ch,
            seg,
            design::component::SEGMENT_INNER_RADIUS,
            &resp,
        );
        ui.painter().text(
            seg.center(),
            Align2::CENTER_CENTER,
            *label,
            font,
            if resp.hovered() { ch.text } else { ch.dim },
        );
        if activated(ui, &resp) {
            clicked = Some(i);
        }
    }
    let group = ui.interact(rect, egui::Id::unique((id_salt, "group")), Sense::hover());
    (clicked, group)
}

/// segmented 的几何助手（测试仍在用），随 segmented 一并放行。
#[allow(dead_code)]
fn segment_rects(inner: Rect, n: usize) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let width = inner.width() / n as f32;
    (0..n)
        .map(|i| {
            Rect::from_min_size(
                Pos2::new(inner.left() + width * i as f32, inner.top()),
                Vec2::new(width, inner.height()),
            )
        })
        .collect()
}

fn switch_knob_x(track: Rect, t: f32) -> f32 {
    let knob_r = design::component::SWITCH_KNOB * 0.5;
    let pad = design::component::SWITCH_PAD;
    let x0 = track.left() + pad + knob_r;
    let x1 = track.right() - pad - knob_r;
    x0 + (x1 - x0) * t.clamp(0.0, 1.0)
}

#[derive(Clone, Copy)]
struct SwitchVisual {
    on: bool,
    enabled: bool,
    hovered: bool,
    focused: bool,
    anim: f32,
}

fn paint_switch(painter: &egui::Painter, ch: Chrome, track: Rect, v: SwitchVisual) {
    let SwitchVisual {
        on,
        enabled,
        hovered,
        focused,
        anim,
    } = v;
    let radius = track.height() * 0.5;
    let fill = if !enabled {
        if on {
            ch.accent_soft
        } else {
            ch.panel2
        }
    } else if on {
        if hovered {
            ch.accent.gamma_multiply(0.94)
        } else {
            ch.accent
        }
    } else if hovered {
        ch.hover
    } else {
        ch.panel2
    };
    painter.rect_filled(track, corner(radius), fill);
    painter.rect_stroke(
        track,
        corner(radius),
        Stroke::new(design::stroke::HAIRLINE, if on { fill } else { ch.border }),
        StrokeKind::Inside,
    );
    if focused {
        painter.rect_stroke(
            track.expand(design::component::FOCUS_RING_OFFSET),
            corner(radius + 1.0),
            Stroke::new(design::component::FOCUS_RING, ch.accent),
            StrokeKind::Outside,
        );
    }
    painter.circle_filled(
        Pos2::new(switch_knob_x(track, anim), track.center().y),
        design::component::SWITCH_KNOB * 0.5,
        if enabled { Color32::WHITE } else { ch.faint },
    );
}

fn switch_track(rect: Rect) -> Rect {
    Rect::from_center_size(rect.center(), Vec2::from(design::component::SWITCH))
}

/// 开关：轨道 36×20，命中区高度按 24 控件下限放大。
pub fn switch(
    ui: &mut egui::Ui,
    ch: Chrome,
    id_salt: &str,
    on: &mut bool,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, mut resp) = ui.allocate_exact_size(
        Vec2::new(design::component::SWITCH[0], design::control::HIT),
        sense,
    );
    let track = switch_track(rect);
    let anim = ui.ctx().animate_bool_with_time(
        egui::Id::unique((id_salt, "anim")),
        *on,
        design::motion::FAST,
    );
    paint_switch(
        ui.painter(),
        ch,
        track,
        SwitchVisual {
            on: *on,
            enabled,
            hovered: enabled && resp.hovered(),
            focused: resp.has_focus(),
            anim,
        },
    );
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if enabled && activated(ui, &resp) {
        *on = !*on;
        resp.mark_changed();
    }
    resp
}

/// 整行开关：开关在左、标签在右，点整行都能切换（设置页用）。
pub fn switch_row(
    ui: &mut egui::Ui,
    ch: Chrome,
    id_salt: &str,
    on: &mut bool,
    label: &str,
) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), design::control::ROW),
        Sense::click(),
    );
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, corner(design::radius::SM), ch.hover);
    }
    let track = switch_track(Rect::from_min_size(
        Pos2::new(
            rect.left() + 4.0,
            rect.center().y - design::control::HIT * 0.5,
        ),
        Vec2::new(design::component::SWITCH[0], design::control::HIT),
    ));
    let anim = ui.ctx().animate_bool_with_time(
        egui::Id::unique((id_salt, "anim")),
        *on,
        design::motion::FAST,
    );
    paint_switch(
        ui.painter(),
        ch,
        track,
        SwitchVisual {
            on: *on,
            enabled: true,
            hovered: resp.hovered(),
            focused: resp.has_focus(),
            anim,
        },
    );
    ui.painter().text(
        Pos2::new(track.right() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(Type::BODY),
        ch.text,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if activated(ui, &resp) {
        *on = !*on;
        resp.mark_changed();
    }
    resp
}

/// 输入框底板：与卡片库搜索框同色同描边，供复合输入（搜索 + 清空）复用。
pub fn input_frame(ch: Chrome) -> egui::Frame {
    egui::Frame::NONE
        .fill(ch.panel2)
        .stroke(Stroke::new(design::stroke::HAIRLINE, ch.border))
        .corner_radius(corner(design::radius::CONTROL))
        .inner_margin(Margin::symmetric(10, 4))
}

/// 单行输入：底板、悬停/焦点色、hint 全部统一，避免每处 TextEdit 各写一套边距。
pub fn text_input(
    ui: &mut egui::Ui,
    ch: Chrome,
    id_salt: &str,
    value: &mut String,
    hint: &str,
    width: f32,
) -> Response {
    input_frame(ch)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .id(egui::Id::unique(id_salt))
                    .hint_text(RichText::new(hint).size(Type::BODY).color(ch.faint))
                    .desired_width(width)
                    .frame(egui::Frame::NONE)
                    .margin(Margin::symmetric(0, 6)),
            )
        })
        .inner
}

/// 下拉选择的全部内容参数。
pub struct SelectSpec<'a> {
    pub current: Option<usize>,
    pub selected_text: &'a str,
    pub options: &'a [&'a str],
    pub font: f32,
    pub width: Option<f32>,
}

/// 下拉选择：选中行统一字色与宽度，选项行沿用 egui 的 selectable 反馈。
pub fn select(ui: &mut egui::Ui, ch: Chrome, id_salt: &str, spec: SelectSpec<'_>) -> Option<usize> {
    let mut picked = None;
    let mut combo = egui::ComboBox::from_id_salt(id_salt).selected_text(
        RichText::new(spec.selected_text)
            .size(spec.font)
            .color(ch.text),
    );
    if let Some(w) = spec.width {
        combo = combo.width(w);
    }
    combo.show_ui(ui, |ui| {
        for (i, label) in spec.options.iter().enumerate() {
            if ui
                .selectable_label(spec.current == Some(i), *label)
                .clicked()
            {
                picked = Some(i);
            }
        }
    });
    picked
}

/// 菜单卡片底板：标题菜单、大纲页头菜单、命令面板共用同一档圆角与投影。
pub fn menu_frame(ch: Chrome) -> egui::Frame {
    let shadow = design::elevation::POPOVER;
    egui::Frame::new()
        .inner_margin(Margin::symmetric(6, 6))
        .corner_radius(design::radius::PANEL)
        .fill(ch.panel)
        .stroke(Stroke::new(design::stroke::HAIRLINE, ch.border))
        .shadow(egui::epaint::Shadow {
            offset: shadow.offset,
            blur: shadow.blur,
            spread: shadow.spread,
            color: Color32::from_black_alpha(shadow.alpha),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome() -> Chrome {
        crate::chrome::chrome_of(&mw_core::get_theme("snow"))
    }

    #[test]
    fn button_tones_map_to_theme_roles_and_disable_to_faint() {
        let ch = chrome();
        assert_eq!(ButtonTone::Primary.ink(ch, true), ch.panel);
        assert_eq!(ButtonTone::Secondary.ink(ch, true), ch.text);
        assert_eq!(ButtonTone::Ghost.ink(ch, true), ch.dim);
        assert_eq!(ButtonTone::Danger.ink(ch, true), ch.warn);
        for tone in [
            ButtonTone::Primary,
            ButtonTone::Secondary,
            ButtonTone::Ghost,
            ButtonTone::Danger,
        ] {
            assert_eq!(tone.ink(ch, false), ch.faint, "{tone:?} 禁用态应统一变淡");
        }
    }

    #[test]
    fn segment_rects_tile_the_inner_rect_without_gaps() {
        let inner = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(150.0, 24.0));
        for n in 1..=4 {
            let rects = segment_rects(inner, n);
            assert_eq!(rects.len(), n);
            assert!((rects[0].left() - inner.left()).abs() < 0.001);
            assert!((rects[n - 1].right() - inner.right()).abs() < 0.001);
            for pair in rects.windows(2) {
                assert!(
                    (pair[0].right() - pair[1].left()).abs() < 0.001,
                    "分段之间漏缝"
                );
            }
            for rect in rects {
                assert!((rect.width() - inner.width() / n as f32).abs() < 0.001);
            }
        }
        assert!(segment_rects(inner, 0).is_empty());
    }

    #[test]
    fn switch_knob_stays_inside_the_track_and_travels_its_full_width() {
        let track = Rect::from_min_size(Pos2::ZERO, Vec2::from(design::component::SWITCH));
        let knob_r = design::component::SWITCH_KNOB * 0.5;
        let pad = design::component::SWITCH_PAD;
        let off = switch_knob_x(track, 0.0);
        let on = switch_knob_x(track, 1.0);
        assert!((off - (track.left() + pad + knob_r)).abs() < 0.001);
        assert!((on - (track.right() - pad - knob_r)).abs() < 0.001);
        assert!(off - knob_r >= track.left());
        assert!(on + knob_r <= track.right());
        assert!(
            (on - off - (track.width() - 2.0 * (pad + knob_r))).abs() < 0.001,
            "圆钮行程不等于轨道去掉两侧内距"
        );
        assert_eq!(switch_knob_x(track, -1.0), off, "动画时间越界应钳制");
        assert_eq!(switch_knob_x(track, 2.0), on, "动画时间越界应钳制");
    }

    #[test]
    fn text_button_meets_the_minimum_hit_height() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let mut size = Vec2::ZERO;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            size = text_button(ui, ch, "保存", ButtonTone::Primary, true)
                .rect
                .size();
        })
        .drop_without_applying_deltas();
        assert!(
            size.y >= design::control::HIT,
            "按钮高 {} 低于命中下限",
            size.y
        );
        assert!(size.x > size.y);
    }

    #[test]
    fn switch_click_toggles_exactly_once() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let mut on = false;
        let mut rect = Rect::NOTHING;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            rect = switch(ui, ch, "test/switch", &mut on, true).rect;
        })
        .drop_without_applying_deltas();
        let pos = rect.center();
        let mut changed = false;
        ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                changed = switch(ui, ch, "test/switch", &mut on, true).changed();
            },
        )
        .drop_without_applying_deltas();
        assert!(changed && on, "点击开关应切换状态并上报 changed");
    }

    #[test]
    fn segmented_returns_the_clicked_index() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(120.0, 28.0));
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = segmented(ui, ch, rect, "test/seg", &["大纲", "画布"], 0);
        })
        .drop_without_applying_deltas();
        let pos = Pos2::new(rect.center().x + rect.width() * 0.25, rect.center().y);
        let mut clicked = None;
        ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                clicked = segmented(ui, ch, rect, "test/seg", &["大纲", "画布"], 0).0;
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(clicked, Some(1), "点右半段应返回索引 1");
    }

    #[test]
    fn tab_click_reports_activation() {
        let ctx = egui::Context::default();
        let ch = chrome();
        let mut rect = Rect::NOTHING;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            rect = tab(ui, ch, "标签", true, true).body.rect;
        })
        .drop_without_applying_deltas();
        let pos = rect.center();
        let mut hit = false;
        ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                let resp = tab(ui, ch, "标签", true, true);
                hit = activated(ui, &resp.body);
            },
        )
        .drop_without_applying_deltas();
        assert!(hit, "点击标签应上报激活");
    }

    /// 组件只能有一套实现：绕过 ui_kit 再写一份 ComboBox / checkbox / 按钮，就是下一轮漂移的种子。
    #[test]
    fn shell_sources_use_ui_kit_instead_of_reimplementing_components() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("读源码目录") {
                let path = entry.expect("目录项").path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&root, &mut files);
        for path in files {
            if path.ends_with("ui_kit.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("读源码");
            for needle in [
                "egui::ComboBox",
                ".checkbox(",
                "fn flat_button",
                "fn ghost_button",
                "\"⌕\"",
                "selectable_label(i == active",
            ] {
                assert!(
                    !text.contains(needle),
                    "{} 仍在绕过 ui_kit 重写组件：{needle}",
                    path.display()
                );
            }
        }
    }
}
