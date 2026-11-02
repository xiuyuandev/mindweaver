//! 窗口级快捷键：默认表 → 用户覆盖 → 落盘 %APPDATA%\织思\shortcuts.json。
//!
//! 只覆盖**窗口级**命令（保存 / 查找 / 面板 / 标签……）。画布内的编辑键（Tab、Enter、
//! 方向键、Ctrl+方向键）属于手势语义，随布局与选择变化，不在这里重绑——把那些也做成
//! 可配置，等于把画布的交互契约拆成一张随时会被改坏的表。
//!
//! 冲突在**绑定时**就拒绝：同一个和弦不允许挂两个动作。匹配仍沿用原来的判断顺序
//! （先 Ctrl+Shift+S 再 Ctrl+S），所以「更具体的和弦先判」这条老规矩不用变。

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Modifiers, RichText};

use crate::chrome::{Tone, Type};

/// 可重绑的窗口级命令。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    /// 新建概念图（`docs/30` §3.6 的第四个入口）。
    NewConcept,
    Open,
    Save,
    SaveAs,
    Export,
    ExportPdf,
    Undo,
    Redo,
    RedoAlt,
    Find,
    /// 全屏大纲内的筛选条（docs/28 §5.7）；Ctrl+F 仍是全局搜索。
    OutlineFilter,
    Palette,
    /// 检查器开合（`docs/30` §4.4：`Ctrl+Shift+I`）。
    Inspector,
    /// 检查器的备用键：老习惯的 `F9`，与 `Inspector` 同义。
    InspectorAlt,
    Outline,
    PaneCycle,
    Help,
    Demo,
    FocusMode,
    Reading,
    NewTab,
    CloseTab,
    PrevTab,
    NextTab,
    PrevTabAlt,
    NextTabAlt,
    Preferences,
}

impl Action {
    /// 配置文件里的稳定键：改中文文案不会让用户的配置失效。
    pub fn id(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::NewConcept => "newConcept",
            Self::Open => "open",
            Self::Save => "save",
            Self::SaveAs => "saveAs",
            Self::Export => "export",
            Self::ExportPdf => "exportPdf",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::RedoAlt => "redoAlt",
            Self::Find => "find",
            Self::OutlineFilter => "outlineFilter",
            Self::Palette => "palette",
            Self::Inspector => "inspector",
            Self::InspectorAlt => "inspectorAlt",
            Self::Outline => "outline",
            Self::PaneCycle => "paneCycle",
            Self::Help => "help",
            Self::Demo => "demo",
            Self::FocusMode => "focusMode",
            Self::Reading => "reading",
            Self::NewTab => "newTab",
            Self::CloseTab => "closeTab",
            Self::PrevTab => "prevTab",
            Self::NextTab => "nextTab",
            Self::PrevTabAlt => "prevTabAlt",
            Self::NextTabAlt => "nextTabAlt",
            Self::Preferences => "preferences",
        }
    }

    /// 配置 / 验收旋钮里的稳定键。未知 id 返回 `None`。
    pub fn from_id(id: &str) -> Option<Self> {
        DEFAULTS.iter().map(|(a, _)| *a).find(|a| a.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::New => "新建",
            Self::NewConcept => "新建概念图",
            Self::Open => "打开",
            Self::Save => "保存",
            Self::SaveAs => "另存为",
            Self::Export => "导出",
            Self::ExportPdf => "导出 PDF",
            Self::Undo => "撤销",
            Self::Redo => "重做",
            Self::RedoAlt => "重做（备用）",
            Self::Find => "查找",
            Self::OutlineFilter => "大纲筛选",
            Self::Palette => "命令面板",
            Self::Inspector => "检查器",
            Self::InspectorAlt => "检查器（备用）",
            Self::Outline => "大纲",
            Self::PaneCycle => "键盘交接画布/面板",
            Self::Help => "快捷键速查",
            Self::Demo => "演示模式",
            Self::FocusMode => "全屏",
            Self::Reading => "阅读模式",
            Self::NewTab => "新建标签页",
            Self::CloseTab => "关闭标签页",
            Self::PrevTab => "上一个标签页",
            Self::NextTab => "下一个标签页",
            Self::PrevTabAlt => "上一个标签页（备用）",
            Self::NextTabAlt => "下一个标签页（备用）",
            Self::Preferences => "快捷键设置",
        }
    }
}

/// 全部动作与出厂和弦。顺序即设置面板的顺序。
pub const DEFAULTS: &[(Action, &str)] = &[
    (Action::New, "Ctrl+N"),
    (Action::NewConcept, "Ctrl+Shift+N"),
    (Action::Open, "Ctrl+O"),
    (Action::Save, "Ctrl+S"),
    (Action::SaveAs, "Ctrl+Shift+S"),
    (Action::Export, "Ctrl+E"),
    (Action::ExportPdf, "Ctrl+P"),
    (Action::Undo, "Ctrl+Z"),
    (Action::Redo, "Ctrl+Shift+Z"),
    (Action::RedoAlt, "Ctrl+Y"),
    (Action::Find, "Ctrl+F"),
    (Action::OutlineFilter, "Ctrl+Shift+F"),
    (Action::Palette, "Ctrl+K"),
    // `docs/30` §4.4 的合同键是 `Ctrl+Shift+I`；`F9` 留作备用动作，不废掉老肌肉记忆。
    (Action::Inspector, "Ctrl+Shift+I"),
    (Action::InspectorAlt, "F9"),
    (Action::Outline, "F8"),
    (Action::PaneCycle, "F6"),
    (Action::Help, "F1"),
    (Action::Demo, "F5"),
    (Action::FocusMode, "F11"),
    (Action::Reading, "F10"),
    (Action::NewTab, "Ctrl+T"),
    (Action::CloseTab, "Ctrl+W"),
    (Action::PrevTab, "Ctrl+Shift+Tab"),
    (Action::NextTab, "Ctrl+Tab"),
    (Action::PrevTabAlt, "Ctrl+PgUp"),
    (Action::NextTabAlt, "Ctrl+PgDn"),
    (Action::Preferences, "Ctrl+Comma"),
];

/// 一个按键组合。Windows 上 Ctrl 同时是 egui 的 COMMAND。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub fn display(self) -> String {
        let mut out = String::new();
        if self.ctrl {
            out.push_str("Ctrl+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        out.push_str(key_name(self.key).unwrap_or("?"));
        out
    }

    /// 进 consume_key 的修饰键。与旧代码里的 Modifiers::COMMAND | SHIFT 同构。
    pub fn modifiers(self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.ctrl {
            m |= Modifiers::COMMAND;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        m
    }

    pub fn parse(text: &str) -> Option<Self> {
        let mut chord = Self {
            key: Key::A,
            ctrl: false,
            shift: false,
            alt: false,
        };
        let mut have_key = false;
        for part in text.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" | "command" | "control" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" | "option" => chord.alt = true,
                other => {
                    if have_key {
                        return None;
                    }
                    chord.key = key_from_name(other)?;
                    have_key = true;
                }
            }
        }
        have_key.then_some(chord)
    }
}

/// 可绑定的键与它的显示名（同一张表，避免「能解析但显示成 ?」）。
const KEYS: &[(&str, Key)] = &[
    ("A", Key::A),
    ("B", Key::B),
    ("C", Key::C),
    ("D", Key::D),
    ("E", Key::E),
    ("F", Key::F),
    ("G", Key::G),
    ("H", Key::H),
    ("I", Key::I),
    ("J", Key::J),
    ("K", Key::K),
    ("L", Key::L),
    ("M", Key::M),
    ("N", Key::N),
    ("O", Key::O),
    ("P", Key::P),
    ("Q", Key::Q),
    ("R", Key::R),
    ("S", Key::S),
    ("T", Key::T),
    ("U", Key::U),
    ("V", Key::V),
    ("W", Key::W),
    ("X", Key::X),
    ("Y", Key::Y),
    ("Z", Key::Z),
    ("0", Key::Num0),
    ("1", Key::Num1),
    ("2", Key::Num2),
    ("3", Key::Num3),
    ("4", Key::Num4),
    ("5", Key::Num5),
    ("6", Key::Num6),
    ("7", Key::Num7),
    ("8", Key::Num8),
    ("9", Key::Num9),
    ("F1", Key::F1),
    ("F2", Key::F2),
    ("F3", Key::F3),
    ("F4", Key::F4),
    ("F5", Key::F5),
    ("F6", Key::F6),
    ("F7", Key::F7),
    ("F8", Key::F8),
    ("F9", Key::F9),
    ("F10", Key::F10),
    ("F11", Key::F11),
    ("F12", Key::F12),
    ("Tab", Key::Tab),
    ("Esc", Key::Escape),
    ("Enter", Key::Enter),
    ("Space", Key::Space),
    ("Backspace", Key::Backspace),
    ("Delete", Key::Delete),
    ("Home", Key::Home),
    ("End", Key::End),
    ("PgUp", Key::PageUp),
    ("PgDn", Key::PageDown),
    ("Left", Key::ArrowLeft),
    ("Right", Key::ArrowRight),
    ("Up", Key::ArrowUp),
    ("Down", Key::ArrowDown),
    ("Comma", Key::Comma),
    ("Period", Key::Period),
    ("Slash", Key::Slash),
    ("Semicolon", Key::Semicolon),
    ("Quote", Key::Quote),
    ("Minus", Key::Minus),
    ("Equals", Key::Equals),
    ("Backtick", Key::Backtick),
    ("[", Key::OpenBracket),
    ("]", Key::CloseBracket),
];

fn key_name(key: Key) -> Option<&'static str> {
    KEYS.iter().find(|(_, k)| *k == key).map(|(n, _)| *n)
}

fn key_from_name(name: &str) -> Option<Key> {
    KEYS.iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, k)| *k)
}

/// 一套绑定。entries 的顺序与 DEFAULTS 一致，匹配顺序沿用旧代码。
/// `None` = 用户显式禁用（`shortcuts.json` 里的 `"none"`）。
#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    entries: Vec<(Action, Option<Chord>)>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self::defaults()
    }
}

impl Bindings {
    pub fn defaults() -> Self {
        let entries = DEFAULTS
            .iter()
            .filter_map(|(a, s)| Chord::parse(s).map(|c| (*a, Some(c))))
            .collect();
        Self { entries }
    }

    pub fn chord(&self, action: Action) -> Option<Chord> {
        self.entries
            .iter()
            .find(|(a, _)| *a == action)
            .and_then(|(_, c)| *c)
    }

    /// 菜单/命令面板里要显示的提示；没绑定或被禁用时给空串。
    pub fn hint(&self, action: Action) -> String {
        self.chord(action).map(|c| c.display()).unwrap_or_default()
    }

    /// 这个动作当前绑的和弦是否被按下（按下即消费）。
    pub fn consume(&self, ctx: &egui::Context, action: Action) -> bool {
        let Some(chord) = self.chord(action) else {
            return false;
        };
        ctx.input_mut(|i| i.consume_key(chord.modifiers(), chord.key))
    }

    /// 重绑。和弦已被别的动作占用时返回占用者，调用方据此提示、不落盘。
    /// 重绑一个已禁用的动作等于重新启用它。
    pub fn set(&mut self, action: Action, chord: Chord) -> Result<(), Action> {
        if let Some((owner, _)) = self
            .entries
            .iter()
            .find(|(a, c)| *c == Some(chord) && *a != action)
        {
            return Err(*owner);
        }
        if let Some(slot) = self.entries.iter_mut().find(|(a, _)| *a == action) {
            slot.1 = Some(chord);
        }
        Ok(())
    }

    /// 禁用一个动作；已经禁用时返回 `false`。
    pub fn disable(&mut self, action: Action) -> bool {
        let Some(slot) = self.entries.iter_mut().find(|(a, _)| *a == action) else {
            return false;
        };
        if slot.1.is_none() {
            return false;
        }
        slot.1 = None;
        true
    }

    pub fn is_disabled(&self, action: Action) -> bool {
        self.entries
            .iter()
            .find(|(a, _)| *a == action)
            .is_some_and(|(_, c)| c.is_none())
    }

    /// 重新启用并恢复出厂和弦。出厂键被别的启用动作占用时返回占用者、
    /// 保持禁用——调用方应先让用户改掉占用者。
    pub fn enable(&mut self, action: Action) -> Result<(), Action> {
        let Some((_, default)) = DEFAULTS.iter().find(|(a, _)| *a == action) else {
            return Ok(());
        };
        let Some(chord) = Chord::parse(default) else {
            return Ok(());
        };
        if let Some((owner, _)) = self
            .entries
            .iter()
            .find(|(a, c)| *c == Some(chord) && *a != action)
        {
            return Err(*owner);
        }
        if let Some(slot) = self.entries.iter_mut().find(|(a, _)| *a == action) {
            slot.1 = Some(chord);
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        *self = Self::defaults();
    }

    pub fn entries(&self) -> &[(Action, Option<Chord>)] {
        &self.entries
    }

    /// 从默认位置读；文件缺失/损坏/含未知项都只影响对应条目，其余用出厂值。
    pub fn load() -> Self {
        match default_path() {
            Some(path) => Self::load_from(&path),
            None => Self::defaults(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        let mut out = Self::defaults();
        let Ok(text) = std::fs::read_to_string(path) else {
            return out;
        };
        // 去掉 UTF-8 BOM：PowerShell 5.1 的 `Set-Content -Encoding UTF8` 会写 BOM，
        // 而 serde_json 见到 BOM 直接报错——那会让整份配置静默失效（2026-10-19 探针踩过）。
        let text = text.trim_start_matches('﻿');
        let Ok(map) = serde_json::from_str::<std::collections::BTreeMap<String, String>>(text)
        else {
            return out;
        };
        let mut pending: Vec<(Action, Option<Chord>)> = Vec::new();
        for (id, text) in &map {
            let Some(action) = DEFAULTS.iter().map(|(a, _)| *a).find(|a| a.id() == id) else {
                continue;
            };
            if text.is_empty() || text.eq_ignore_ascii_case("none") {
                pending.push((action, None));
                continue;
            }
            let Some(chord) = Chord::parse(text) else {
                continue;
            };
            pending.push((action, Some(chord)));
        }
        // 先落禁用、再落改绑：否则 `save:"none"` + `open:"Ctrl+S"` 会按字母序
        // 先看到 `open`，被仍在默认位的 `save` 挡住，读回来等于没改。
        for (action, _) in pending.iter().filter(|(_, c)| c.is_none()) {
            out.disable(*action);
        }
        for (action, chord) in pending {
            // 重复和弦按「先到先得」，后来的退回默认；禁用项不占位。
            if let Some(chord) = chord {
                if out.set(action, chord).is_err() {
                    continue;
                }
            }
        }
        out
    }

    pub fn save(&self) {
        if let Some(path) = default_path() {
            self.save_to(&path);
        }
    }

    pub fn save_to(&self, path: &Path) {
        let map: std::collections::BTreeMap<&str, String> = self
            .entries
            .iter()
            .map(|(a, c)| {
                let text = c.map(|c| c.display()).unwrap_or_else(|| "none".into());
                (a.id(), text)
            })
            .collect();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&map) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    Some(base.join("织思").join("shortcuts.json"))
}

/// 设置面板：列出全部动作，点「改」之后捕获下一个组合键。
#[derive(Default)]
pub struct Dialog {
    open: bool,
    capturing: Option<Action>,
    message: Option<String>,
}

impl Dialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing.is_some()
    }

    pub fn open(&mut self) {
        self.open = true;
        self.capturing = None;
        self.message = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.capturing = None;
    }

    /// 捕获态下读一次按键。返回是否消费了这次输入。
    pub fn capture(&mut self, ctx: &egui::Context, bindings: &mut Bindings) -> bool {
        let Some(action) = self.capturing else {
            return false;
        };
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        let Some((key, mods)) = pressed else {
            // 这一帧没有按键：继续等。返回 true 表示「捕获态占着键盘」。
            return true;
        };
        // 捕获到的键不能再流到弹窗按钮/画布：否则改键的同时会触发一次 Enter。
        ctx.input_mut(|i| {
            i.events
                .retain(|e| !matches!(e, egui::Event::Key { pressed: true, .. }));
        });
        if key == Key::Escape {
            self.capturing = None;
            self.message = Some("已取消".into());
            return true;
        }
        if key_name(key).is_none() {
            self.message = Some("这个键不支持绑定".into());
            self.capturing = None;
            return true;
        }
        let chord = Chord {
            key,
            ctrl: mods.ctrl || mods.command,
            shift: mods.shift,
            alt: mods.alt,
        };
        match bindings.set(action, chord) {
            Ok(()) => {
                bindings.save();
                crate::trace::event(&format!(
                    "shortcut set action={} chord={}",
                    action.id(),
                    chord.display()
                ));
                self.message = Some(format!("{} → {}", action.label(), chord.display()));
                self.capturing = None;
            }
            Err(owner) => {
                crate::trace::event(&format!(
                    "shortcut reject action={} owner={}",
                    action.id(),
                    owner.id()
                ));
                self.message = Some(format!("{} 已被「{}」占用", chord.display(), owner.label()));
                self.capturing = None;
            }
        }
        true
    }

    /// 画面板。返回绑定是否被改过。
    pub fn show(&mut self, ctx: &egui::Context, tone: Tone, bindings: &mut Bindings) -> bool {
        if !self.open {
            return false;
        }
        let mut changed = false;
        let actions = [
            crate::dialog::DialogAction::new("恢复默认", crate::ui_kit::ButtonTone::Secondary),
            crate::dialog::DialogAction::new("关闭", crate::ui_kit::ButtonTone::Primary),
        ];
        let spec = crate::dialog::DialogSpec::new("快捷键")
            .width(520.0)
            .default_action(1);
        let out = crate::dialog::show(ctx, tone.ch, "shortcuts", spec, &actions, |ui| {
            ui.label(
                RichText::new(
                    "点「改」再按新的组合键；Esc 取消。被输入法占用时点「禁用」，\
                     重新启用恢复出厂键。画布内的编辑键不在此列。",
                )
                .size(Type::SMALL)
                .color(tone.ch.dim),
            );
            ui.add_space(4.0);
            for (action, chord) in bindings.entries().to_vec() {
                let disabled = bindings.is_disabled(action);
                let chord_text = chord
                    .map(|c| c.display())
                    .unwrap_or_else(|| "未绑定".into());
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [230.0, 30.0],
                        egui::Label::new(
                            RichText::new(action.label())
                                .size(Type::BODY)
                                .color(tone.ink),
                        ),
                    );
                    ui.add_sized(
                        [110.0, 30.0],
                        egui::Label::new(
                            RichText::new(chord_text)
                                .size(Type::BODY)
                                .monospace()
                                .color(tone.muted),
                        ),
                    );
                    let capturing = self.capturing == Some(action);
                    let text = if capturing { "按下…" } else { "改" };
                    if ui.small_button(text).clicked() {
                        self.capturing = Some(action);
                        self.message = None;
                    }
                    let toggle = if disabled { "启用" } else { "禁用" };
                    if ui.small_button(toggle).clicked() {
                        self.capturing = None;
                        if disabled {
                            match bindings.enable(action) {
                                Ok(()) => {
                                    bindings.save();
                                    crate::trace::event(&format!(
                                        "shortcut enable action={}",
                                        action.id()
                                    ));
                                    let key = bindings.hint(action);
                                    self.message =
                                        Some(format!("{} 已启用（{key}）", action.label()));
                                }
                                Err(owner) => {
                                    crate::trace::event(&format!(
                                        "shortcut enable_reject action={} owner={}",
                                        action.id(),
                                        owner.id()
                                    ));
                                    self.message = Some(format!(
                                        "{} 的出厂键已被「{}」占用，先改占用者",
                                        action.label(),
                                        owner.label()
                                    ));
                                }
                            }
                        } else {
                            bindings.disable(action);
                            bindings.save();
                            crate::trace::event(&format!(
                                "shortcut disable action={}",
                                action.id()
                            ));
                            self.message = Some(format!("{} 已禁用", action.label()));
                        }
                    }
                });
                ui.add_space(2.0);
            }
            if let Some(msg) = &self.message {
                ui.add_space(6.0);
                ui.label(RichText::new(msg).size(Type::SMALL).color(tone.ch.dim));
            }
        });
        if out.action == Some(0) {
            bindings.reset();
            bindings.save();
            crate::trace::event("shortcut reset");
            self.message = Some("已恢复默认".into());
            changed = true;
        }
        if out.closed {
            self.close();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_action_without_collisions() {
        let bindings = Bindings::defaults();
        assert_eq!(bindings.entries().len(), DEFAULTS.len());
        for (action, _) in DEFAULTS {
            assert!(
                bindings.chord(*action).is_some(),
                "{} 没有默认和弦",
                action.label()
            );
        }
        let mut seen = std::collections::HashSet::new();
        for (_, chord) in bindings.entries() {
            let chord = chord.expect("默认表不能有禁用项");
            assert!(
                seen.insert(chord),
                "默认表里有重复和弦：{}",
                chord.display()
            );
        }
    }

    /// 具体和弦必须先判。egui 的 `matches_logically` 只查「模式要 Shift 而没按」，
    /// 不查「按了多余的 Shift」——所以按下 Ctrl+Shift+N 同样能满足 Ctrl+N 的模式，
    /// 反过来不成立。判别顺序一旦回到「`New` 在前」，Ctrl+Shift+N 会静默变成
    /// 「新建思维导图」（SE-03 的轨迹里真发生过）。
    #[test]
    fn the_shifted_chord_is_swallowed_by_the_plain_one_unless_judged_first() {
        let bindings = Bindings::defaults();
        let concept = bindings.chord(Action::NewConcept).expect("有默认和弦");
        let plain = bindings.chord(Action::New).expect("有默认和弦");
        let both = Modifiers::COMMAND | Modifiers::SHIFT;
        assert!(
            both.matches_logically(concept.modifiers()),
            "Ctrl+Shift+N 应命中概念图的模式"
        );
        assert!(
            both.matches_logically(plain.modifiers()),
            "这正是必须把 NewConcept 排在 New 之前的原因"
        );
        assert!(
            !Modifiers::COMMAND.matches_logically(concept.modifiers()),
            "只按 Ctrl+N 不该被概念图的模式摘走"
        );
    }

    #[test]
    fn chord_text_round_trips() {
        for (_, text) in DEFAULTS {
            let chord = Chord::parse(text).expect("默认表必须能解析");
            assert_eq!(chord.display(), *text, "显示名与配置名必须一致");
        }
        assert!(Chord::parse("Ctrl+Shift+Z").is_some());
        assert!(Chord::parse("Ctrl+").is_none(), "只有修饰键不算和弦");
        assert!(Chord::parse("Ctrl+Nope").is_none());
        assert!(
            Chord::parse("Ctrl+A+B").is_none(),
            "一个和弦只能有一个普通键"
        );
    }

    #[test]
    fn rebinding_rejects_a_chord_used_by_another_action() {
        let mut bindings = Bindings::defaults();
        let err = bindings.set(Action::Save, Chord::parse("Ctrl+O").unwrap());
        assert_eq!(err, Err(Action::Open), "冲突要报出占用者");
        assert!(bindings
            .set(Action::Save, Chord::parse("Ctrl+Alt+S").unwrap())
            .is_ok());
        assert_eq!(
            bindings.chord(Action::Save).unwrap().display(),
            "Ctrl+Alt+S"
        );
        bindings.reset();
        assert_eq!(bindings.chord(Action::Save).unwrap().display(), "Ctrl+S");
    }

    /// 禁用要真的释放和弦：它不再占用占用者名单，菜单提示也要消失。
    #[test]
    fn a_disabled_shortcut_frees_its_chord_and_survives_a_round_trip() {
        let dir = std::env::temp_dir().join(format!("mw-shortcuts-off-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shortcuts.json");

        let mut bindings = Bindings::defaults();
        assert!(bindings.disable(Action::Save));
        assert!(!bindings.disable(Action::Save), "重复禁用不算变化");
        assert!(!bindings.is_disabled(Action::Open));
        assert_eq!(bindings.chord(Action::Save), None);
        assert_eq!(bindings.hint(Action::Save), "");
        assert!(
            bindings
                .set(Action::Open, Chord::parse("Ctrl+S").unwrap())
                .is_ok(),
            "禁用项不该继续占着 Ctrl+S"
        );

        bindings.save_to(&path);
        let loaded = Bindings::load_from(&path);
        assert_eq!(loaded.entries().len(), DEFAULTS.len(), "禁用项仍占一行");
        assert!(loaded.is_disabled(Action::Save), "\"none\" 要读成禁用");
        assert_eq!(loaded.chord(Action::Open).unwrap().display(), "Ctrl+S");

        let mut restored = loaded;
        restored.reset();
        assert!(!restored.is_disabled(Action::Save));
        assert_eq!(restored.chord(Action::Save).unwrap().display(), "Ctrl+S");
        let _ = std::fs::remove_file(&path);
    }

    /// 启用恢复的是出厂键；出厂键被启用中的动作占用时不能硬塞回去。
    #[test]
    fn enabling_restores_the_factory_key_unless_it_is_taken() {
        let mut bindings = Bindings::defaults();
        bindings.disable(Action::Save);
        bindings
            .set(Action::Open, Chord::parse("Ctrl+S").unwrap())
            .unwrap();
        assert_eq!(
            bindings.enable(Action::Save),
            Err(Action::Open),
            "要报出占用者"
        );
        assert!(bindings.is_disabled(Action::Save), "冲突时保持禁用");

        bindings
            .set(Action::Open, Chord::parse("Ctrl+Alt+O").unwrap())
            .unwrap();
        assert_eq!(bindings.enable(Action::Save), Ok(()));
        assert_eq!(bindings.chord(Action::Save).unwrap().display(), "Ctrl+S");
    }

    /// `"none"` 与空串都是禁用；坏值仍退回出厂值，不能让整份配置失效。
    #[test]
    fn none_and_blank_values_disable_while_broken_values_fall_back() {
        let dir = std::env::temp_dir().join(format!("mw-shortcuts-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shortcuts.json");
        std::fs::write(
            &path,
            r#"{"save":"none","undo":"","open":"Ctrl+","inspector":"Ctrl+Alt+I"}"#,
        )
        .unwrap();
        let loaded = Bindings::load_from(&path);
        assert!(loaded.is_disabled(Action::Save));
        assert!(loaded.is_disabled(Action::Undo));
        assert_eq!(
            loaded.chord(Action::Open).unwrap().display(),
            "Ctrl+O",
            "坏和弦退回默认"
        );
        assert_eq!(
            loaded.chord(Action::Inspector).unwrap().display(),
            "Ctrl+Alt+I",
            "正常覆盖照旧生效"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// 菜单显示的就是用户改过的键，不是出厂值。
    #[test]
    fn hints_follow_the_configured_chord() {
        let mut bindings = Bindings::defaults();
        assert_eq!(bindings.hint(Action::Inspector), "Ctrl+Shift+I");
        assert_eq!(bindings.hint(Action::InspectorAlt), "F9");
        bindings
            .set(Action::Inspector, Chord::parse("F12").unwrap())
            .unwrap();
        assert_eq!(bindings.hint(Action::Inspector), "F12");
    }

    #[test]
    fn a_saved_file_reloads_and_ignores_unknown_or_broken_entries() {
        let dir = std::env::temp_dir().join(format!("mw-shortcuts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shortcuts.json");
        let mut bindings = Bindings::defaults();
        bindings
            .set(Action::Inspector, Chord::parse("F12").unwrap())
            .unwrap();
        bindings.save_to(&path);

        let loaded = Bindings::load_from(&path);
        assert_eq!(loaded.chord(Action::Inspector).unwrap().display(), "F12");
        assert_eq!(loaded.chord(Action::Save).unwrap().display(), "Ctrl+S");

        std::fs::write(
            &path,
            r#"{"inspector":"F12","nope":"Ctrl+Q","save":"Ctrl+"}"#,
        )
        .unwrap();
        let loaded = Bindings::load_from(&path);
        assert_eq!(loaded.chord(Action::Inspector).unwrap().display(), "F12");
        assert_eq!(
            loaded.chord(Action::Save).unwrap().display(),
            "Ctrl+S",
            "坏条目退回默认"
        );

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Bindings::load_from(&path), Bindings::defaults());

        // BOM：PowerShell 写的配置照样要能读
        let mut text = String::new();
        text.push('\u{feff}');
        text.push_str(r#"{"outline":"F2"}"#);
        std::fs::write(&path, text).unwrap();
        assert_eq!(
            Bindings::load_from(&path)
                .chord(Action::Outline)
                .unwrap()
                .display(),
            "F2"
        );
        let _ = std::fs::remove_file(&path);
    }
}
