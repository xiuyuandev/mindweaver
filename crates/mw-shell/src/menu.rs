//! 统一弹出菜单引擎（docs/24 X16）。
//!
//! 菜单只有四种条目：动作、子菜单、分隔线、分组标题。右键菜单、标题菜单和
//! 大纲页头菜单共用这里的行绘制、快捷键列、边缘翻转与键盘状态机——四处各写
//! 一份 30px 行和一套 `↑↓` 逻辑，正是上一轮视觉漂移的来源。
//!
//! 两个宿主：
//! - `Menu<T>`：画布右键菜单，自管 `Area`、多级子菜单与全部键盘；
//! - `PopupNav`：标题/大纲菜单挂在 egui `Popup` 里，只接管键盘与高亮，
//!   容器、滚动与关闭仍由 `Popup` 负责。
//!
//! 引擎只发意图，不读文档：`T` 由调用方定义，动作落地留在壳层。

use eframe::egui::scroll_area::{ScrollBarVisibility, ScrollSource};
use eframe::egui::{
    Align, Align2, Color32, CursorIcon, FontId, Id, Key, Modifiers, Pos2, Rect, Response,
    ScrollArea, Sense, Stroke, Vec2,
};

use crate::chrome::{Chrome, Tone, Type};
use crate::icons::Icon;
use mw_core::design;

/// 卡片距内容区边缘的安全边。
const EDGE: f32 = 8.0;
/// 根菜单卡片宽度（含内边距）。
pub(crate) const ROOT_W: f32 = 240.0;
/// 子菜单卡片宽度（含内边距）。
pub(crate) const SUB_W: f32 = 200.0;
/// 卡片内边距：标题菜单的 `menu_frame` 与右键菜单共用同一档。
const PAD: f32 = 6.0;
/// 15/22 文字 + 上下各 5。与 `design::component::MENU_ROW_H` 同值；
/// 命令面板读同一档行高，三处一起改。
const ROW_H: f32 = 32.0;
const SEP_H: f32 = 9.0;
const SECTION_H: f32 = 24.0;
const ICON_X: f32 = 16.0;
const LABEL_X: f32 = 32.0;
const SHORTCUT_RIGHT: f32 = 12.0;
/// 子菜单 chevron 占掉的快捷键列宽度：快捷键右移会让两者叠在一起。
const SUBMENU_GUTTER: f32 = 18.0;
const CHEVRON_RIGHT: f32 = 10.0;
/// 相邻两级卡片的重叠量：子菜单贴住父行，鼠标斜着走也不会掉进缝里。
const SUBMENU_OVERLAP: f32 = 2.0;
/// 安全上限：菜单层级再深也不该无限铺下去。
const MAX_DEPTH: usize = 4;
/// 超高菜单的可视滑块：与开始页同一档视觉。
const SCROLL_W: f32 = 4.0;
const SCROLL_MIN_H: f32 = 28.0;
const SCROLL_PAD: f32 = 4.0;
const SCROLL_ALPHA: u8 = 130;

/// 一条菜单项。`T` 是调用方的动作类型（右键菜单是 `context::Action`）。
#[derive(Clone, Debug)]
pub(crate) enum Entry<T> {
    Action {
        label: String,
        shortcut: &'static str,
        icon: Option<Icon>,
        check: Option<bool>,
        enabled: bool,
        action: T,
    },
    Submenu {
        label: String,
        icon: Option<Icon>,
        enabled: bool,
        entries: Vec<Entry<T>>,
    },
    Separator,
    Section(String),
}

impl<T> Entry<T> {
    pub(crate) fn action(
        label: impl Into<String>,
        shortcut: &'static str,
        action: T,
        enabled: bool,
    ) -> Self {
        Self::Action {
            label: label.into(),
            shortcut,
            icon: None,
            check: None,
            enabled,
            action,
        }
    }

    pub(crate) fn action_with_icon(
        label: impl Into<String>,
        shortcut: &'static str,
        icon: Icon,
        action: T,
        enabled: bool,
    ) -> Self {
        Self::Action {
            label: label.into(),
            shortcut,
            icon: Some(icon),
            check: None,
            enabled,
            action,
        }
    }

    /// 带勾选的条目：一组互斥选项里，当前生效的那个打勾（其余留空，不占位差）。
    pub(crate) fn action_checked(
        label: impl Into<String>,
        shortcut: &'static str,
        action: T,
        enabled: bool,
        checked: bool,
    ) -> Self {
        Self::Action {
            label: label.into(),
            shortcut,
            icon: None,
            check: Some(checked),
            enabled,
            action,
        }
    }

    pub(crate) fn submenu(label: impl Into<String>, enabled: bool, entries: Vec<Entry<T>>) -> Self {
        Self::Submenu {
            label: label.into(),
            icon: None,
            enabled,
            entries,
        }
    }

    pub(crate) fn separator() -> Self {
        Self::Separator
    }

    pub(crate) fn section(text: impl Into<String>) -> Self {
        Self::Section(text.into())
    }

    fn navigable(&self) -> bool {
        matches!(
            self,
            Self::Action { enabled: true, .. } | Self::Submenu { enabled: true, .. }
        )
    }

    fn label(&self) -> &str {
        match self {
            Self::Action { label, .. } | Self::Submenu { label, .. } | Self::Section(label) => {
                label
            }
            Self::Separator => "",
        }
    }

    fn is_submenu(&self) -> bool {
        matches!(self, Self::Submenu { .. })
    }

    fn height(&self) -> f32 {
        match self {
            Self::Separator => SEP_H,
            Self::Section(_) => SECTION_H,
            _ => ROW_H,
        }
    }
}

/// 一级菜单里第一个可用条目。
fn first_enabled<T>(entries: &[Entry<T>]) -> Option<usize> {
    entries.iter().position(Entry::navigable)
}

/// 从 `current` 起按 `delta` 找下一个可用条目，跳过禁用项/分隔线/标题。
fn step_enabled<T>(entries: &[Entry<T>], current: Option<usize>, delta: isize) -> Option<usize> {
    let rows: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.navigable())
        .map(|(i, _)| i)
        .collect();
    if rows.is_empty() {
        return None;
    }
    let at = current.and_then(|c| rows.iter().position(|&i| i == c));
    let next = match at {
        Some(p) => (p as isize + delta).rem_euclid(rows.len() as isize) as usize,
        None if delta < 0 => rows.len() - 1,
        None => 0,
    };
    Some(rows[next])
}

/// 卡片高度：所有条目 + 上下内边距。
pub(crate) fn level_height<T>(entries: &[Entry<T>]) -> f32 {
    entries.iter().map(Entry::height).sum::<f32>() + PAD * 2.0
}

/// 卡片高度夹到内容区：比视口高的菜单不能把最后一行顶到屏幕外。
pub(crate) fn card_height(natural_h: f32, content: Rect) -> f32 {
    natural_h.min((content.height() - EDGE * 2.0).max(0.0))
}

/// 溢出时的滑块几何：`(轨道内顶部比例, 长度比例)`；不溢出返回 `None`。
pub(crate) fn scroll_thumb(content_h: f32, viewport_h: f32, offset: f32) -> Option<(f32, f32)> {
    if viewport_h <= 0.0 || content_h <= viewport_h + 1.0 {
        return None;
    }
    let len = (viewport_h / content_h)
        .max((SCROLL_MIN_H / viewport_h).min(1.0))
        .clamp(0.0, 1.0);
    let travel = 1.0 - len;
    if travel <= f32::EPSILON {
        return Some((0.0, 1.0));
    }
    let t = (offset / (content_h - viewport_h)).clamp(0.0, 1.0);
    Some((t * travel, len))
}

/// 根菜单落点：默认向右下展开；右侧不够翻左、下方不够翻上，最后夹进内容区。
pub(crate) fn place_root(anchor: Pos2, size: Vec2, content: Rect) -> Pos2 {
    let mut x = anchor.x;
    let mut y = anchor.y;
    if x + size.x > content.right() - EDGE {
        x = anchor.x - size.x;
    }
    if y + size.y > content.bottom() - EDGE {
        y = anchor.y - size.y;
    }
    let min_x = content.left() + EDGE;
    let max_x = (content.right() - size.x - EDGE).max(min_x);
    let min_y = content.top() + EDGE;
    let max_y = (content.bottom() - size.y - EDGE).max(min_y);
    Pos2::new(x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

/// 子菜单落点：优先贴父行右侧，右侧不够翻到左侧；垂直按父行顶对齐后夹住。
pub(crate) fn place_submenu(parent_row: Rect, size: Vec2, content: Rect) -> Pos2 {
    let mut x = parent_row.right() - SUBMENU_OVERLAP;
    if x + size.x > content.right() - EDGE {
        x = parent_row.left() - size.x + SUBMENU_OVERLAP;
    }
    let y = parent_row.top() - PAD;
    let min_x = content.left() + EDGE;
    let max_x = (content.right() - size.x - EDGE).max(min_x);
    let min_y = content.top() + EDGE;
    let max_y = (content.bottom() - size.y - EDGE).max(min_y);
    Pos2::new(x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

/// 快捷键列右边缘：带子菜单的行要再让出 chevron 的位置。
pub(crate) fn shortcut_right(row: Rect, submenu: bool) -> f32 {
    row.right() - SHORTCUT_RIGHT - if submenu { SUBMENU_GUTTER } else { 0.0 }
}

/// 一行动作的绘制参数。
#[derive(Clone, Copy)]
pub(crate) struct RowSpec<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
    pub shortcut: &'a str,
    pub check: Option<bool>,
    pub enabled: bool,
    pub submenu: bool,
}

/// 画一行菜单动作。返回响应；是否激活由调用方决定（右键菜单看点击，
/// Popup 菜单还要看键盘 Enter）。
pub(crate) fn paint_row(
    ui: &mut eframe::egui::Ui,
    ch: Chrome,
    spec: RowSpec<'_>,
    keyboard_lit: bool,
) -> Response {
    let sense = if spec.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), sense);
    let radius = design::radius::SM;
    if spec.enabled {
        let hovered = resp.hovered();
        let fill = if hovered {
            if resp.is_pointer_button_down_on() {
                ch.accent_soft
            } else {
                ch.hover
            }
        } else if keyboard_lit {
            ch.accent_soft
        } else {
            Color32::TRANSPARENT
        };
        if fill != Color32::TRANSPARENT {
            ui.painter()
                .rect_filled(rect, eframe::egui::CornerRadius::same(radius as u8), fill);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }

    let ink = if spec.enabled { ch.text } else { ch.faint };
    let icon_ink = if spec.enabled { ch.dim } else { ch.icon_off };
    let icon_c = Pos2::new(rect.left() + ICON_X, rect.center().y);
    match spec.check {
        Some(true) => paint_check(
            ui.painter(),
            icon_c,
            if spec.enabled { ch.accent } else { icon_ink },
        ),
        None => {
            if let Some(icon) = spec.icon {
                crate::icon_paint::paint_icon(ui.painter(), icon_c, icon, icon_ink);
            }
        }
        Some(false) => {
            if let Some(icon) = spec.icon {
                crate::icon_paint::paint_icon(ui.painter(), icon_c, icon, icon_ink);
            }
        }
    }

    ui.painter().text(
        Pos2::new(rect.left() + LABEL_X, rect.center().y),
        Align2::LEFT_CENTER,
        spec.label,
        FontId::proportional(Type::LABEL),
        ink,
    );
    let shortcut_color = if spec.enabled { ch.dim } else { ch.icon_off };
    if !spec.shortcut.is_empty() {
        ui.painter().text(
            Pos2::new(shortcut_right(rect, spec.submenu), rect.center().y),
            Align2::RIGHT_CENTER,
            spec.shortcut,
            FontId::proportional(Type::SMALL),
            shortcut_color,
        );
    }
    if spec.submenu {
        crate::icon_paint::paint_icon_sized(
            ui.painter(),
            Pos2::new(rect.right() - CHEVRON_RIGHT - 6.0, rect.center().y),
            12.0,
            Icon::ChevronRight,
            if spec.enabled { ch.faint } else { ch.icon_off },
        );
    }
    resp
}

fn paint_check(painter: &eframe::egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.7, color);
    painter.line_segment(
        [
            Pos2::new(c.x - 4.2, c.y + 0.2),
            Pos2::new(c.x - 1.2, c.y + 3.4),
        ],
        s,
    );
    painter.line_segment(
        [
            Pos2::new(c.x - 1.2, c.y + 3.4),
            Pos2::new(c.x + 4.6, c.y - 3.6),
        ],
        s,
    );
}

/// 分隔线：9px 高，左右各留 8px。
pub(crate) fn paint_separator(ui: &mut eframe::egui::Ui, ch: Chrome) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), SEP_H), Sense::hover());
    ui.painter().hline(
        (rect.left() + 8.0)..=(rect.right() - 8.0),
        rect.center().y,
        Stroke::new(design::stroke::HAIRLINE, ch.sep),
    );
}

/// 分组标题：13px 小字，不进键盘序列。
pub(crate) fn paint_section(ui: &mut eframe::egui::Ui, ch: Chrome, text: &str) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), SECTION_H), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(Type::SMALL),
        ch.dim,
    );
}

// ---------------------------------------------------------------------------
// 右键菜单：自管 Area、子菜单和全部键盘
// ---------------------------------------------------------------------------

pub(crate) struct Menu<T> {
    anchor: Pos2,
    entries: Vec<Entry<T>>,
    /// 每层当前高亮条目下标；与 `path.len() + 1` 同步。
    highlights: Vec<Option<usize>>,
    /// 高亮是 `↑↓` 走出来的（强调淡底），还是鼠标悬停（灰底）。
    keyboard: bool,
    /// 键盘刚移动过高亮：本帧把它滚进视口；打开菜单或鼠标悬停不算。
    reveal: bool,
    /// 已展开的子菜单链：每层选中的 Submenu 条目下标。
    path: Vec<usize>,
    open: bool,
    just_opened: bool,
    /// 上一帧各层卡片矩形，用来判断「点在外面」。
    rects: Vec<Rect>,
}

struct LevelOut {
    rect: Rect,
    row_rects: Vec<Rect>,
    hovered: Option<usize>,
    clicked: Option<usize>,
}

impl<T: Copy> Menu<T> {
    pub(crate) fn new(anchor: Pos2, entries: Vec<Entry<T>>) -> Self {
        crate::trace::event(&format!(
            "menu_open anchor={:.0},{:.0} entries={}",
            anchor.x,
            anchor.y,
            entries.len()
        ));
        Self {
            anchor,
            entries,
            highlights: vec![None],
            keyboard: false,
            reveal: false,
            path: Vec::new(),
            open: true,
            just_opened: true,
            rects: Vec::new(),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// 键盘先于画布消费（main 在画布快捷键之前调用）。返回要执行的动作。
    pub(crate) fn handle_keys(&mut self, ctx: &eframe::egui::Context) -> Option<T> {
        if !self.open {
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            let level = self.path.len();
            if self.path.pop().is_some() {
                self.highlights.pop();
                crate::trace::event(&format!("menu_submenu_close key=escape level={level}"));
            } else {
                self.open = false;
                crate::trace::event("menu_close reason=escape");
            }
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            self.step(1);
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            self.step(-1);
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Home)) {
            self.jump(false);
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::End)) {
            self.jump(true);
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowRight)) {
            self.enter_submenu();
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowLeft)) {
            let level = self.path.len();
            if self.path.pop().is_some() {
                self.highlights.pop();
                crate::trace::event(&format!("menu_submenu_close key=left level={level}"));
            }
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            return self.activate_highlighted();
        }
        // 其余按键收起整张菜单，但不吞事件（Ctrl+Tab 之类必须继续生效）。
        if ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, eframe::egui::Event::Key { pressed: true, .. }))
        }) {
            self.open = false;
        }
        None
    }

    fn current_entries(&self) -> &[Entry<T>] {
        let mut entries: &[Entry<T>] = &self.entries;
        for &i in &self.path {
            match entries.get(i) {
                Some(Entry::Submenu { entries: sub, .. }) => entries = sub,
                _ => break,
            }
        }
        entries
    }

    fn current_highlight(&self) -> Option<usize> {
        self.highlights.last().copied().flatten()
    }

    fn step(&mut self, delta: isize) {
        let (next, label) = {
            let entries = self.current_entries();
            let next = step_enabled(entries, self.current_highlight(), delta);
            let label = next.map(|i| entries[i].label().to_string());
            (next, label)
        };
        if let Some(slot) = self.highlights.last_mut() {
            *slot = next;
        }
        self.keyboard = true;
        self.reveal = true;
        if let Some(label) = label {
            crate::trace::event(&format!(
                "menu_nav key={} label={label}",
                if delta < 0 { "up" } else { "down" }
            ));
        }
    }

    fn jump(&mut self, last: bool) {
        let (next, label) = {
            let entries = self.current_entries();
            let rows: Vec<usize> = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.navigable())
                .map(|(i, _)| i)
                .collect();
            let next = if last {
                rows.last().copied()
            } else {
                rows.first().copied()
            };
            (next, next.map(|i| entries[i].label().to_string()))
        };
        if let Some(slot) = self.highlights.last_mut() {
            *slot = next;
        }
        self.keyboard = true;
        self.reveal = true;
        if let Some(label) = label {
            crate::trace::event(&format!(
                "menu_nav key={} label={label}",
                if last { "end" } else { "home" }
            ));
        }
    }

    fn enter_submenu(&mut self) {
        let Some(i) = self.current_highlight() else {
            return;
        };
        let entries = self.current_entries();
        let Some(Entry::Submenu {
            enabled: true,
            entries: sub,
            label,
            ..
        }) = entries.get(i)
        else {
            return;
        };
        let label = label.clone();
        let first = first_enabled(sub);
        self.path.push(i);
        self.highlights.push(first);
        self.keyboard = true;
        self.reveal = true;
        crate::trace::event(&format!("menu_submenu_open key=right label={label}"));
    }

    fn activate_highlighted(&mut self) -> Option<T> {
        let i = self.current_highlight()?;
        enum Hit<T> {
            Sub(Option<usize>, String),
            Act(T, String),
        }
        let hit = {
            let entries = self.current_entries();
            match entries.get(i) {
                Some(Entry::Submenu {
                    enabled: true,
                    entries: sub,
                    label,
                    ..
                }) => Hit::Sub(first_enabled(sub), label.clone()),
                Some(Entry::Action {
                    enabled: true,
                    action,
                    label,
                    ..
                }) => Hit::Act(*action, label.clone()),
                _ => return None,
            }
        };
        match hit {
            Hit::Sub(first, label) => {
                self.path.push(i);
                self.highlights.push(first);
                crate::trace::event(&format!("menu_submenu_open key=enter label={label}"));
                None
            }
            Hit::Act(action, label) => {
                self.open = false;
                crate::trace::event(&format!("menu_activate key=enter label={label}"));
                Some(action)
            }
        }
    }

    /// 画卡片并读取鼠标。返回被点击的动作。
    pub(crate) fn show(&mut self, ctx: &eframe::egui::Context, tone: Tone) -> Option<T> {
        if !self.open {
            return None;
        }
        let ch = tone.ch;
        let content = ctx.input(|i| i.content_rect());
        let entries = self.entries.clone();
        let mut pos = place_root(
            self.anchor,
            Vec2::new(ROOT_W, card_height(level_height(&entries), content)),
            content,
        );
        let mut action = None;
        let mut rects = Vec::new();
        let mut level = 0usize;

        loop {
            let level_entries = entries_at(&entries, &self.path[..level]);
            let width = if level == 0 { ROOT_W } else { SUB_W };
            let open_child = self.path.get(level).copied().filter(|&i| {
                level_entries
                    .get(i)
                    .is_some_and(|e| matches!(e, Entry::Submenu { .. }))
            });
            let highlighted = self.highlights.get(level).copied().flatten();
            let out = paint_level(
                ctx,
                ch,
                level,
                level_entries,
                pos,
                width,
                content,
                highlighted,
                self.keyboard,
                self.reveal,
                open_child,
            );
            rects.push(out.rect);

            if let Some(i) = out.hovered {
                self.keyboard = false;
                if let Some(slot) = self.highlights.get_mut(level) {
                    *slot = Some(i);
                }
                let submenu = level_entries
                    .get(i)
                    .is_some_and(|e| e.is_submenu() && e.navigable());
                let opening = submenu && self.path.get(level) != Some(&i);
                self.path.truncate(level);
                self.highlights.truncate(level + 1);
                if submenu {
                    self.path.push(i);
                    let first = level_entries.get(i).and_then(|e| match e {
                        Entry::Submenu { entries, .. } => first_enabled(entries),
                        _ => None,
                    });
                    self.highlights.push(first);
                    if opening {
                        crate::trace::event(&format!(
                            "menu_submenu_open via=hover label={}",
                            level_entries[i].label()
                        ));
                    }
                }
            }

            if let Some(i) = out.clicked {
                match level_entries.get(i) {
                    Some(Entry::Action {
                        enabled: true,
                        action: a,
                        label,
                        ..
                    }) => {
                        crate::trace::event(&format!("menu_click label={label}"));
                        action = Some(*a);
                        self.open = false;
                        break;
                    }
                    Some(Entry::Submenu {
                        enabled: true,
                        entries: sub,
                        ..
                    }) => {
                        let first = first_enabled(sub);
                        self.path.truncate(level);
                        self.path.push(i);
                        self.highlights.truncate(level + 1);
                        self.highlights.push(first);
                        self.keyboard = false;
                    }
                    _ => {}
                }
            }

            if !self.open || self.path.len() <= level || level + 1 >= MAX_DEPTH {
                break;
            }
            let child = self.path[level];
            let Some(row) = out.row_rects.get(child) else {
                break;
            };
            let Some(Entry::Submenu { entries: sub, .. }) = level_entries.get(child) else {
                break;
            };
            pos = place_submenu(
                *row,
                Vec2::new(SUB_W, card_height(level_height(sub), content)),
                content,
            );
            // 形状证据：子菜单到底落在哪、有没有被夹住（§3 第 8 条要的就是这几个数）。
            // 只在 `MW_TRACE` 打开时格式化，正常运行零开销。
            if crate::trace::is_enabled() {
                crate::trace::event(&format!(
                    "menu_submenu_rect row={},{},{},{} pos={},{} size={},{} content={},{},{},{}",
                    row.min.x,
                    row.min.y,
                    row.max.x,
                    row.max.y,
                    pos.x,
                    pos.y,
                    SUB_W,
                    card_height(level_height(sub), content),
                    content.min.x,
                    content.min.y,
                    content.max.x,
                    content.max.y
                ));
            }
            level += 1;
        }

        self.rects = rects;
        self.reveal = false;
        if self.just_opened {
            self.just_opened = false;
        } else if ctx.input(|i| i.pointer.any_click()) {
            let inside = ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| self.rects.iter().any(|r| r.contains(p)));
            if !inside {
                self.open = false;
                crate::trace::event("menu_close reason=outside");
            }
        }
        action
    }

    /// 视觉验收：展开指定下标的子菜单（证据包用，不参与交互逻辑）。
    pub(crate) fn demo_open_submenu(&mut self, index: usize) {
        let Some(Entry::Submenu { entries, .. }) = self.entries.get(index) else {
            return;
        };
        self.path = vec![index];
        self.highlights = vec![Some(index), first_enabled(entries)];
        self.keyboard = true;
        self.just_opened = false;
    }

    /// 按文案找根层条目（仅视觉验收用）。
    pub(crate) fn index_of(&self, label: &str) -> Option<usize> {
        self.entries.iter().position(|entry| match entry {
            Entry::Action { label: l, .. }
            | Entry::Submenu { label: l, .. }
            | Entry::Section(l) => l == label,
            Entry::Separator => false,
        })
    }
}

fn entries_at<'a, T>(entries: &'a [Entry<T>], path: &[usize]) -> &'a [Entry<T>] {
    let mut list = entries;
    for &i in path {
        match list.get(i) {
            Some(Entry::Submenu { entries: sub, .. }) => list = sub,
            _ => break,
        }
    }
    list
}

#[allow(clippy::too_many_arguments)]
fn paint_level<T>(
    ctx: &eframe::egui::Context,
    ch: Chrome,
    level: usize,
    entries: &[Entry<T>],
    pos: Pos2,
    width: f32,
    content: Rect,
    highlighted: Option<usize>,
    keyboard: bool,
    reveal: bool,
    open_child: Option<usize>,
) -> LevelOut {
    let inner_w = width - PAD * 2.0;
    let natural_h = level_height(entries);
    // 菜单比视口高时卡片夹到内容区，条目在卡片内部滚动；不超高走原绘制路径，
    // 既有 golden（menu 等）必须逐字节不变。
    let viewport_h = (card_height(natural_h, content) - PAD * 2.0).max(0.0);
    let scrolls = natural_h - PAD * 2.0 > viewport_h + 0.5;
    let mut hovered = None;
    let mut clicked = None;
    let mut row_rects = Vec::new();
    let mut paint_rows = |ui: &mut eframe::egui::Ui| {
        ui.set_width(inner_w);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for (i, entry) in entries.iter().enumerate() {
            let row = match entry {
                Entry::Separator => {
                    paint_separator(ui, ch);
                    None
                }
                Entry::Section(text) => {
                    paint_section(ui, ch, text);
                    None
                }
                Entry::Action {
                    label,
                    shortcut,
                    icon,
                    check,
                    enabled,
                    ..
                } => {
                    let lit = keyboard && highlighted == Some(i);
                    let resp = paint_row(
                        ui,
                        ch,
                        RowSpec {
                            icon: *icon,
                            label,
                            shortcut,
                            check: *check,
                            enabled: *enabled,
                            submenu: false,
                        },
                        lit,
                    );
                    if resp.hovered() && *enabled {
                        hovered = Some(i);
                    }
                    if resp.clicked() && *enabled {
                        clicked = Some(i);
                    }
                    Some(resp.rect)
                }
                Entry::Submenu {
                    label,
                    icon,
                    enabled,
                    ..
                } => {
                    let is_open = open_child == Some(i);
                    let lit = (keyboard && highlighted == Some(i)) || is_open;
                    let resp = paint_row(
                        ui,
                        ch,
                        RowSpec {
                            icon: *icon,
                            label,
                            shortcut: "",
                            check: None,
                            enabled: *enabled,
                            submenu: true,
                        },
                        lit,
                    );
                    if resp.hovered() && *enabled {
                        hovered = Some(i);
                    }
                    if resp.clicked() && *enabled {
                        clicked = Some(i);
                    }
                    Some(resp.rect)
                }
            };
            match row {
                Some(rect) => {
                    // 键盘移动过才滚动：演示态/鼠标悬停不改变滚动位置。
                    if reveal && highlighted == Some(i) && !ui.clip_rect().contains_rect(rect) {
                        ui.scroll_to_rect(rect, Some(Align::Center));
                    }
                    row_rects.push(rect);
                }
                None => row_rects.push(Rect::NOTHING),
            }
        }
    };
    let area = eframe::egui::Area::new(Id::unique(("mw-menu", level)))
        .order(eframe::egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            crate::ui_kit::menu_frame(ch).show(ui, |ui| {
                if !scrolls {
                    paint_rows(ui);
                    return;
                }
                ui.set_width(inner_w);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                // Area 的可用高度取上一帧卡片尺寸；不先撑起来，ScrollArea 会把
                // 视口锁在上一帧高度，卡片永远长不到夹高后的目标。
                ui.set_min_height(viewport_h);
                let scroll = ScrollArea::vertical()
                    .id_salt(("mw-menu-scroll", level))
                    .max_height(viewport_h)
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .scroll_source(ScrollSource {
                        scroll_bar: false,
                        ..Default::default()
                    })
                    .show(ui, |ui| paint_rows(ui));
                let view = scroll.inner_rect;
                if let Some((top, len)) = scroll_thumb(
                    scroll.content_size.y,
                    view.height(),
                    scroll.state.clamped_offset().y,
                ) {
                    let track = Rect::from_min_max(
                        Pos2::new(view.right() - SCROLL_W, view.top() + SCROLL_PAD),
                        Pos2::new(view.right(), view.bottom() - SCROLL_PAD),
                    );
                    if track.height() >= SCROLL_MIN_H {
                        let thumb_h = (len * track.height()).clamp(SCROLL_MIN_H, track.height());
                        let travel = track.height() - thumb_h;
                        let rect = Rect::from_min_size(
                            Pos2::new(track.left(), track.top() + top * travel),
                            Vec2::new(SCROLL_W, thumb_h),
                        );
                        let dim = ch.dim;
                        ui.painter().rect_filled(
                            rect,
                            eframe::egui::CornerRadius::same(2),
                            Color32::from_rgba_unmultiplied(
                                dim.r(),
                                dim.g(),
                                dim.b(),
                                SCROLL_ALPHA,
                            ),
                        );
                    }
                }
            });
        });
    LevelOut {
        rect: area.response.rect,
        row_rects,
        hovered,
        clicked,
    }
}

// ---------------------------------------------------------------------------
// Popup 菜单：标题栏 / 大纲页头
// ---------------------------------------------------------------------------

/// Popup 菜单的键盘状态。容器仍是 egui `Popup`，这里只补它没有的自绘行导航。
#[derive(Clone, Default)]
struct PopupNavState {
    /// 上一帧可用行数；首帧为 0，方向键不会越界。
    nav_count: usize,
    /// 本帧已登记的可导航行数。
    seen: usize,
    highlight: Option<usize>,
    keyboard: bool,
    /// 本帧按下 Enter 时要执行的行。
    enter_row: Option<usize>,
    frame: u64,
}

const NAV_ACTIVE: &str = "mw-popup-nav-active";

/// 打开一个 Popup 菜单导航作用域。必须在菜单行绘制之前创建、之后 `end`。
pub(crate) struct PopupNav {
    id: Id,
    frame: u64,
}

impl PopupNav {
    pub(crate) fn begin(ui: &eframe::egui::Ui, id_salt: &str) -> Self {
        let ctx = ui.ctx();
        let id = Id::unique(("mw-popup-nav", id_salt));
        let frame = ctx.cumulative_frame_nr();
        let mut state = ctx
            .data_mut(|d| d.get_temp::<PopupNavState>(id))
            .unwrap_or_default();
        // 上一帧没画（菜单刚打开或刚关闭）就清掉旧高亮，避免「打开即落在第三行」。
        if frame > state.frame + 1 || state.highlight.is_some_and(|h| h >= state.nav_count) {
            state.highlight = None;
            state.keyboard = false;
        }
        state.seen = 0;
        state.enter_row = None;
        state.frame = frame;

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            ui.close();
            crate::trace::event("popup_menu key=escape");
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            state.highlight = step_enabled_rows(state.nav_count, state.highlight, 1);
            state.keyboard = true;
            crate::trace::event(&format!(
                "popup_menu key=down highlight={:?} count={}",
                state.highlight, state.nav_count
            ));
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            state.highlight = step_enabled_rows(state.nav_count, state.highlight, -1);
            state.keyboard = true;
            crate::trace::event(&format!(
                "popup_menu key=up highlight={:?} count={}",
                state.highlight, state.nav_count
            ));
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Home)) {
            state.highlight = (state.nav_count > 0).then_some(0);
            state.keyboard = true;
            crate::trace::event(&format!(
                "popup_menu key=home highlight={:?} count={}",
                state.highlight, state.nav_count
            ));
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::End)) {
            state.highlight = state.nav_count.checked_sub(1);
            state.keyboard = true;
            crate::trace::event(&format!(
                "popup_menu key=end highlight={:?} count={}",
                state.highlight, state.nav_count
            ));
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            state.enter_row = state.highlight;
            crate::trace::event(&format!(
                "popup_menu key=enter highlight={:?}",
                state.highlight
            ));
        } else if ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, eframe::egui::Event::Key { pressed: true, .. }))
        }) {
            ui.close();
        }

        ctx.data_mut(|d| {
            d.insert_temp(id, state);
            d.insert_temp(Id::unique(NAV_ACTIVE), id);
        });
        Self { id, frame }
    }

    pub(crate) fn end(self, ui: &eframe::egui::Ui) {
        let ctx = ui.ctx();
        let mut state = ctx
            .data_mut(|d| d.get_temp::<PopupNavState>(self.id))
            .unwrap_or_default();
        state.nav_count = state.seen;
        state.seen = 0;
        state.enter_row = None;
        state.frame = self.frame;
        if state.highlight.is_some_and(|h| h >= state.nav_count) {
            state.highlight = None;
        }
        ctx.data_mut(|d| {
            d.insert_temp(self.id, state);
            d.remove::<Id>(Id::unique(NAV_ACTIVE));
        });
    }
}

fn step_enabled_rows(count: usize, current: Option<usize>, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let next = match current {
        Some(c) => (c as isize + delta).rem_euclid(count as isize) as usize,
        None if delta < 0 => count - 1,
        None => 0,
    };
    Some(next)
}

/// Popup 菜单里画一行：登记导航、画行、处理 Enter。
pub(crate) fn popup_row(ui: &mut eframe::egui::Ui, ch: Chrome, spec: RowSpec<'_>) -> bool {
    let active = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Id>(Id::unique(NAV_ACTIVE)));
    let Some(nav_id) = active else {
        let resp = paint_row(ui, ch, spec, false);
        return spec.enabled && crate::ui_kit::activated(ui, &resp);
    };
    let ctx = ui.ctx().clone();
    let mut state = ctx
        .data_mut(|d| d.get_temp::<PopupNavState>(nav_id))
        .unwrap_or_default();
    let idx = spec.enabled.then(|| {
        let i = state.seen;
        state.seen += 1;
        i
    });
    let keyboard_lit = idx.is_some_and(|i| state.keyboard && state.highlight == Some(i));
    let resp = paint_row(ui, ch, spec, keyboard_lit);
    if spec.enabled && resp.hovered() {
        state.highlight = idx;
        state.keyboard = false;
    }
    let enter = idx.is_some_and(|i| state.enter_row == Some(i));
    if keyboard_lit {
        ui.scroll_to_rect(resp.rect, Some(Align::Center));
    }
    ctx.data_mut(|d| d.insert_temp(nav_id, state));
    spec.enabled && (resp.clicked() || enter || crate::ui_kit::activated(ui, &resp))
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// 勾选档位是「互斥选项组」的表达：带勾的条目必须走到 `RowSpec.check`，
    /// 不带勾的必须留 `None`（否则图标槽会被当成空格子，行高会差一个像素）。
    #[test]
    fn checked_rows_carry_their_flag_and_plain_rows_stay_none() {
        let on = Entry::action_checked("反向 ←", "", 1u8, true, true);
        let off = Entry::action_checked("单向 →", "", 2u8, true, false);
        let plain = Entry::action("编辑标签", "F2", 3u8, true);
        match (on, off, plain) {
            (
                Entry::Action { check: a, .. },
                Entry::Action { check: b, .. },
                Entry::Action { check: c, .. },
            ) => {
                assert_eq!(a, Some(true));
                assert_eq!(b, Some(false));
                assert_eq!(c, None);
            }
            _ => panic!("三行都应是动作条目"),
        }
    }

    use super::*;
    use eframe::egui::{Modifiers, Pos2, Rect, Vec2};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Act {
        A,
        B,
        C,
    }

    fn sample() -> Vec<Entry<Act>> {
        vec![
            Entry::action("第一项", "Ctrl+A", Act::A, true),
            Entry::separator(),
            Entry::section("分组"),
            Entry::action("禁用项", "", Act::B, false),
            Entry::action("第三项", "Ctrl+C", Act::C, true),
        ]
    }

    fn key_press(key: Key) -> eframe::egui::Event {
        eframe::egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn with_key(ctx: &eframe::egui::Context, key: Key, f: impl FnOnce(&eframe::egui::Context)) {
        ctx.begin_pass(eframe::egui::RawInput {
            events: vec![key_press(key)],
            ..Default::default()
        });
        f(ctx);
        let mut out = ctx.end_pass();
        out.textures_delta.clear();
    }

    #[test]
    fn navigation_skips_separators_sections_and_disabled_rows() {
        let mut menu = Menu::new(Pos2::ZERO, sample());
        menu.step(1);
        assert_eq!(menu.current_highlight(), Some(0), "第一行");
        assert!(menu.keyboard);
        menu.step(1);
        assert_eq!(
            menu.current_highlight(),
            Some(4),
            "跳过禁用项、分隔线和分组"
        );
        menu.step(1);
        assert_eq!(menu.current_highlight(), Some(0), "循环回第一行");
        menu.jump(true);
        assert_eq!(menu.current_highlight(), Some(4), "End 落到末行");
        menu.jump(false);
        assert_eq!(menu.current_highlight(), Some(0), "Home 落到首行");
    }

    #[test]
    fn enter_activates_and_closes() {
        let ctx = eframe::egui::Context::default();
        let mut menu = Menu::new(Pos2::ZERO, sample());
        with_key(&ctx, Key::ArrowDown, |ctx| {
            assert_eq!(menu.handle_keys(ctx), None);
        });
        let mut action = None;
        with_key(&ctx, Key::Enter, |ctx| {
            action = menu.handle_keys(ctx);
        });
        assert_eq!(action, Some(Act::A));
        assert!(!menu.is_open());
    }

    #[test]
    fn disabled_row_never_activates() {
        let mut menu = Menu::new(Pos2::ZERO, sample());
        menu.highlights[0] = Some(3);
        assert_eq!(menu.activate_highlighted(), None);
        assert!(menu.is_open(), "点在禁用项上不关菜单");
    }

    #[test]
    fn submenu_opens_with_right_and_closes_with_left() {
        let entries = vec![
            Entry::action("普通", "", Act::A, true),
            Entry::submenu(
                "子菜单",
                true,
                vec![
                    Entry::action("子项一", "", Act::B, true),
                    Entry::action("子项二", "", Act::C, true),
                ],
            ),
        ];
        let mut menu = Menu::new(Pos2::ZERO, entries);
        menu.step(1);
        menu.step(1);
        assert_eq!(menu.current_highlight(), Some(1));
        menu.enter_submenu();
        assert_eq!(menu.path, vec![1]);
        assert_eq!(menu.current_highlight(), Some(0), "落到子菜单第一个可用项");

        let ctx = eframe::egui::Context::default();
        with_key(&ctx, Key::ArrowLeft, |ctx| {
            menu.handle_keys(ctx);
        });
        assert!(menu.path.is_empty(), "← 先收子菜单");
        assert!(menu.is_open());

        with_key(&ctx, Key::Escape, |ctx| {
            menu.handle_keys(ctx);
        });
        assert!(!menu.is_open(), "根层 Esc 才关整张菜单");
    }

    #[test]
    fn enter_on_submenu_opens_it_instead_of_activating() {
        let entries = vec![Entry::submenu(
            "子菜单",
            true,
            vec![Entry::action("子项", "", Act::B, true)],
        )];
        let mut menu = Menu::new(Pos2::ZERO, entries);
        menu.highlights[0] = Some(0);
        assert_eq!(menu.activate_highlighted(), None);
        assert_eq!(menu.path, vec![0]);
        assert!(menu.is_open());
    }

    #[test]
    fn root_flips_left_and_up_near_the_edges() {
        let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
        let size = Vec2::new(240.0, 300.0);
        let pos = place_root(Pos2::new(900.0, 700.0), size, content);
        assert_eq!(pos.x, 900.0 - 240.0, "右侧不够翻到指针左边");
        assert_eq!(pos.y, 700.0 - 300.0, "下方不够翻到指针上方");

        let pos = place_root(Pos2::new(500.0, 300.0), size, content);
        assert_eq!(pos, Pos2::new(500.0, 300.0), "空间足够时不动");
    }

    #[test]
    fn root_is_clamped_into_the_content_rect() {
        let content = Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(600.0, 400.0));
        let size = Vec2::new(240.0, 300.0);
        let pos = place_root(Pos2::new(105.0, 55.0), size, content);
        assert_eq!(pos.x, 108.0, "左安全边 8px");
        assert_eq!(pos.y, 58.0, "上安全边 8px");

        let pos = place_root(Pos2::new(995.0, 445.0), size, content);
        assert!(pos.x + size.x <= 700.0 - 8.0 + 0.01);
        assert!(pos.y + size.y <= 450.0 - 8.0 + 0.01);
    }

    #[test]
    fn submenu_flips_to_the_left_when_the_right_side_is_full() {
        let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 600.0));
        let row = Rect::from_min_size(Pos2::new(350.0, 100.0), Vec2::new(228.0, 30.0));
        let pos = place_submenu(row, Vec2::new(200.0, 120.0), content);
        assert_eq!(pos.x, 350.0 - 200.0 + 2.0, "翻到父行左侧，保留 2px 重叠");
        assert_eq!(pos.y, 94.0, "顶对齐父行（减内边距）");
    }

    #[test]
    fn submenu_stays_on_the_right_when_it_fits() {
        let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        let row = Rect::from_min_size(Pos2::new(300.0, 100.0), Vec2::new(228.0, 30.0));
        let pos = place_submenu(row, Vec2::new(200.0, 120.0), content);
        assert_eq!(pos.x, 526.0, "贴父行右侧，重叠 2px");
        assert_eq!(pos.y, 94.0);
    }

    #[test]
    fn submenu_clamps_up_when_the_parent_row_is_near_the_bottom() {
        // §3 第 8 条：竖直方向从来没有单测——只有「翻左/贴右」两条横向用例。
        // 父行贴底时 `place_submenu` 不翻到上方（那是根菜单的做法），而是夹进内容区底边，
        // 于是最下面那行仍然看得见；这里把夹取后的坐标钉住。
        let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let row = Rect::from_min_size(Pos2::new(300.0, 760.0), Vec2::new(228.0, 30.0));
        let pos = place_submenu(row, Vec2::new(200.0, 300.0), content);
        assert_eq!(pos.y, 800.0 - 300.0 - 8.0, "夹到底边内，最后一行不被切掉");
        assert_eq!(pos.x, 526.0, "横向不受影响：贴父行右侧");
        // 取舍写明白：夹取后子菜单会盖住父菜单下半部分（这里父行 760..790 落在 492..792 里），
        // 但横向只重叠 2px、指针仍在父行右半边之外，所以父行照样可点、不会来回抖。
        assert!(pos.y < row.top(), "夹取就是往上推，不做「翻到父行上方」");
        assert!(
            pos.x > row.center().x,
            "横向不重叠父行中心，指针不会同时落在两处"
        );
    }

    #[test]
    fn submenu_clamps_to_the_top_when_the_row_is_above_the_content() {
        // 反向也钉一下：父行被顶出内容区（滚动/缩放的极端情况）时夹到上边距。
        let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let row = Rect::from_min_size(Pos2::new(300.0, -40.0), Vec2::new(228.0, 30.0));
        let pos = place_submenu(row, Vec2::new(200.0, 300.0), content);
        assert_eq!(pos.y, 8.0, "夹到 content.top() + EDGE");
    }
    #[test]
    fn shortcut_column_moves_left_for_submenus() {
        let row = Rect::from_min_size(Pos2::ZERO, Vec2::new(228.0, 30.0));
        assert_eq!(shortcut_right(row, false), 216.0);
        assert_eq!(shortcut_right(row, true), 198.0);
        assert!(shortcut_right(row, true) <= row.right() - CHEVRON_RIGHT - 18.0);
    }

    #[test]
    fn height_counts_padding_and_row_kinds() {
        let h = level_height(&sample());
        assert_eq!(h, ROW_H * 3.0 + 9.0 + 24.0 + 12.0);
    }

    #[test]
    fn card_height_clamps_to_the_content_rect() {
        let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 460.0));
        assert_eq!(card_height(300.0, content), 300.0, "不超高不动");
        assert_eq!(card_height(477.0, content), 444.0, "上下各留 8px 安全边");
        let tiny = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 10.0));
        assert_eq!(
            card_height(477.0, tiny),
            0.0,
            "视口比安全边还窄时不出现负数"
        );
    }

    #[test]
    fn overflow_menu_stays_inside_the_content_rect() {
        let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 460.0));
        let size = Vec2::new(ROOT_W, card_height(477.0, content));
        let pos = place_root(Pos2::new(360.0, 120.0), size, content);
        assert!(
            pos.y + size.y <= content.bottom() - EDGE + 0.01,
            "底边不出内容区"
        );
        assert!(pos.y >= EDGE, "顶边保留安全边");
    }

    #[test]
    fn scroll_thumb_only_appears_when_overflowing() {
        assert_eq!(scroll_thumb(400.0, 444.0, 0.0), None);
        assert_eq!(scroll_thumb(444.5, 444.0, 0.0), None, "1px 内算不溢出");
        assert_eq!(scroll_thumb(500.0, 0.0, 0.0), None, "视口高为 0 不画");

        let (top, len) = scroll_thumb(645.0, 444.0, 0.0).unwrap();
        assert_eq!(top, 0.0, "顶部时滑块贴轨道顶");
        assert!((len - 444.0 / 645.0).abs() < 1e-6);

        let (top, _) = scroll_thumb(645.0, 444.0, 201.0).unwrap();
        assert!((top - (1.0 - 444.0 / 645.0)).abs() < 1e-6, "滚到底贴轨道底");

        let (top, len) = scroll_thumb(645.0, 444.0, 900.0).unwrap();
        assert!((top - (1.0 - len)).abs() < 1e-6, "越界 offset 夹到轨道底");

        let (_, len) = scroll_thumb(100_000.0, 444.0, 0.0).unwrap();
        assert!(
            (len - SCROLL_MIN_H / 444.0).abs() < 1e-6,
            "再长也不短于 28px"
        );
    }
}
