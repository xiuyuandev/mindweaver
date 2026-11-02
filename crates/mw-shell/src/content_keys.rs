//! 内容表面键盘 / IME / 焦点底座（CER-03）。
//!
//! [`page_owns_bare_keys`] 是「此刻裸键归谁」的唯一判定点：模态、标题栏菜单、
//! 右键菜单、斜杠菜单、拖拽态任一占用，内容页就不收裸键；IME 组合态单独归
//! 输入法（[`editor_owns_text_keys`]）。CER-04 起的大纲页、CER-13 起的卡片页、
//! CER-22 起的笔记库都从这扇门过，不允许各自再写一套 if 链——开始页的
//! [`crate::app_methods_1::home_bare_keys_owned`] 与笔记库的
//! [`crate::note_library::page_keys_owned`] 已委托到这里（同源）。
//!
//! IME 判定是**事件式**的：只看当帧输入里有没有非空 `Preedit`，与
//! `canvas.rs`、`panels/search.rs`、`panels/outline.rs` 三处既有口径一致，
//! 不维护跨帧状态；需要跨帧记忆的编辑器（CER-07）自己保存。

use eframe::egui::{self, Event, Id, ImeEvent};

/// 前台占用者的快照。由调用方每帧收集一次；`Default` 是「无人占用」。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Foreground {
    /// 模态弹窗在栈顶（含 `LibraryRemove` / 设置 / 打开中等）或演示弹窗开着。
    pub modal_open: bool,
    /// 标题栏菜单（egui Memory 弹层）、右键菜单或速查表开着。
    pub menu_open: bool,
    /// 卡片页斜杠菜单开着（CER-18 起有真实来源）。
    pub slash_open: bool,
    /// 画布或大纲拖拽进行中。
    pub drag_active: bool,
    /// IME 组合态（当帧有非空 Preedit）。
    pub ime_composing: bool,
}

impl Foreground {
    /// 快捷构造：只有模态占用（笔记库旧门的两个入参都归这一位）。
    pub fn modal(open: bool) -> Self {
        Self {
            modal_open: open,
            ..Self::default()
        }
    }

    /// 快捷构造：只有菜单占用（开始页旧门的菜单 + 速查表都归这一位）。
    pub fn menu(open: bool) -> Self {
        Self {
            menu_open: open,
            ..Self::default()
        }
    }
}

/// 内容页面此刻能否收裸键（Esc / 方向键 / Enter / 单字母）。
///
/// docs/28 §4.6：「弹窗、菜单、斜杠菜单打开时，内容表面不收裸键」；
/// 拖拽进行中同样让键（X20 门后维护⑨⑪的既有口径）。IME 组合态**不**进入
/// 这个判定：组合态让掉的是编辑键（Enter/Tab/Esc），见
/// [`editor_owns_text_keys`]——组合中的候选窗归输入法，但页面其余部分
/// 的裸键归属不受它影响。
pub fn page_owns_bare_keys(fg: Foreground) -> bool {
    !fg.modal_open && !fg.menu_open && !fg.slash_open && !fg.drag_active
}

/// 活动行内编辑此刻能否把 Enter / Tab / Esc 当编辑键处理。
///
/// docs/28 §4.6：组合态 Enter 确认候选、Esc 取消组合、Tab 先归输入法；
/// 组合期间不切换行、不提交。调用方仍需先过 [`page_owns_bare_keys`]：
/// 模态在前台时编辑器根本不该看到按键。
///
/// CER-07 行内编辑接线前的前瞻 API，故放行 dead_code；门矩阵单测保证语义。
#[allow(dead_code)]
pub fn editor_owns_text_keys(fg: Foreground) -> bool {
    !fg.ime_composing
}

/// 当帧输入事件里是否有进行中的 IME 组合（非空 Preedit）。
///
/// 空文本 Preedit 是组合的开始/结束边界，不算组合态——与既有三处一致。
/// CER-07 接线前的前瞻 API（既有三处事件判定本包不动），故放行 dead_code。
#[allow(dead_code)]
pub fn ime_composing(events: &[Event]) -> bool {
    events.iter().any(|e| {
        matches!(
            e,
            Event::Ime(ImeEvent::Preedit { text, .. }) if !text.is_empty()
        )
    })
}

/// 从 egui 上下文读当帧组合态（调用方每帧收集 `Foreground` 时用）。
/// CER-07 接线前的前瞻 API，故放行 dead_code。
#[allow(dead_code)]
pub fn ime_composing_now(ctx: &egui::Context) -> bool {
    ctx.input(|i| ime_composing(&i.raw.events))
}

/// 保存当前键盘焦点，供浮层关闭后归还。没有焦点时返回 `None`。
/// CER-04+ 页面接线前的前瞻 API，故放行 dead_code。
#[allow(dead_code)]
pub fn save_focus(ctx: &egui::Context) -> Option<Id> {
    ctx.memory(|mem| mem.focused())
}

/// 把焦点还给 `save_focus` 存下的控件；`None` 是无操作。
/// docs/28 §4.6：「关闭后焦点回到原行/原列表项」。
/// CER-04+ 页面接线前的前瞻 API，故放行 dead_code。
#[allow(dead_code)]
pub fn restore_focus(ctx: &egui::Context, focus: Option<Id>) {
    if let Some(id) = focus {
        ctx.memory_mut(|mem| mem.request_focus(id));
    }
}

/// Tab 顺序助手：在固定槽位序列里循环前进/后退，返回下一个该聚焦的槽位。
///
/// `current: None`（或不在序列里）时从头开始；`backwards` 反向且首尾相接。
/// 槽位用语义名（与 `FocusKey::new` 的 base 同风格），页面包负责把槽位
/// 映射回具体控件。空序列返回 `None`。
/// CER-04/29 页面槽位清单定稿前的前瞻 API，故放行 dead_code。
#[allow(dead_code)]
pub fn tab_cycle<'a>(
    slots: &'a [&'a str],
    current: Option<&str>,
    backwards: bool,
) -> Option<&'a str> {
    if slots.is_empty() {
        return None;
    }
    let idx = current
        .and_then(|c| slots.iter().position(|s| *s == c))
        .map_or_else(
            || if backwards { slots.len() - 1 } else { 0 },
            |i| {
                if backwards {
                    (i + slots.len() - 1) % slots.len()
                } else {
                    (i + 1) % slots.len()
                }
            },
        );
    slots.get(idx).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 门矩阵：32 种前台组合 × 两个判定，逐格断言（docs/29 CER-03「门矩阵单测」）。
    #[test]
    fn gate_matrix_covers_every_foreground_combination() {
        for bits in 0..32u32 {
            let fg = Foreground {
                modal_open: bits & 1 != 0,
                menu_open: bits & 2 != 0,
                slash_open: bits & 4 != 0,
                drag_active: bits & 8 != 0,
                ime_composing: bits & 16 != 0,
            };
            let any_foreground = fg.modal_open || fg.menu_open || fg.slash_open || fg.drag_active;
            assert_eq!(
                page_owns_bare_keys(fg),
                !any_foreground,
                "bits={bits:05b}：裸键归属必须只由四个前台位决定"
            );
            assert_eq!(
                editor_owns_text_keys(fg),
                !fg.ime_composing,
                "bits={bits:05b}：编辑键只让给 IME 组合态"
            );
        }
    }

    /// 同源改造的护栏：两个旧函数委托后真值表逐位不变。
    #[test]
    fn legacy_page_gates_delegate_bit_for_bit() {
        // 开始页：menu_open || help_open 归 menu 位。
        for menu in [false, true] {
            for help in [false, true] {
                let legacy = crate::app_methods_1::home_bare_keys_owned(menu, help);
                assert_eq!(
                    legacy,
                    page_owns_bare_keys(Foreground::menu(menu || help)),
                    "menu={menu} help={help}"
                );
            }
        }
        // 笔记库：modal_top || dialog_demo 归 modal 位。
        for modal in [false, true] {
            for demo in [false, true] {
                let legacy = crate::note_library::page_keys_owned(modal, demo);
                assert_eq!(
                    legacy,
                    page_owns_bare_keys(Foreground::modal(modal || demo)),
                    "modal={modal} demo={demo}"
                );
            }
        }
    }

    #[test]
    fn ime_composing_only_for_nonempty_preedit() {
        use egui::Event;
        let preedit = |t: &str| {
            Event::Ime(ImeEvent::Preedit {
                text: t.to_owned(),
                active_range_chars: None,
            })
        };
        assert!(!ime_composing(&[]));
        assert!(!ime_composing(&[preedit("")])); // 边界：组合开始/结束
        assert!(ime_composing(&[preedit("内")]));
        assert!(ime_composing(&[
            Event::Key {
                key: egui::Key::A,
                pressed: true,
                repeat: false,
                physical_key: None,
                modifiers: egui::Modifiers::default(),
            },
            preedit("中文"),
        ]));
        // 提交 / 删除环绕不是组合态。
        assert!(!ime_composing(&[Event::Ime(ImeEvent::Commit(
            "好".to_owned()
        ))]));
        assert!(!ime_composing(&[Event::Ime(ImeEvent::DeleteSurrounding {
            before_chars: 1,
            after_chars: 0,
        })]));
    }

    #[test]
    fn tab_cycle_walks_slots_and_wraps_both_ways() {
        let slots = ["sidebar", "list", "preview"];
        assert_eq!(tab_cycle(&slots, None, false), Some("sidebar"));
        assert_eq!(tab_cycle(&slots, Some("sidebar"), false), Some("list"));
        assert_eq!(tab_cycle(&slots, Some("list"), false), Some("preview"));
        assert_eq!(tab_cycle(&slots, Some("preview"), false), Some("sidebar"));
        assert_eq!(tab_cycle(&slots, Some("sidebar"), true), Some("preview"));
        // 不在序列里的当前槽位按起点处理。
        assert_eq!(tab_cycle(&slots, Some("elsewhere"), false), Some("sidebar"));
        assert_eq!(tab_cycle(&slots, Some("elsewhere"), true), Some("preview"));
        assert_eq!(tab_cycle(&[], None, false), None);
    }
}
