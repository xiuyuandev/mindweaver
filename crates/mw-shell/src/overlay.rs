//! 浮层优先级：同一时刻只允许一个"抢指针"的浮层，Esc 从最上层逐层退。
//!
//! 这不是渲染栈，而是**决策表**：壳层每帧根据各浮层的 open 状态算出最上层，
//! 画布是否锁输入、Esc 该关谁，都以它为准。优先级从低到高：
//! 右键菜单 < 命令面板 < 搜索 < 导出 < 帮助 < 统计 < 快捷键 < 标签管理 <
//! 库路径对话框 < 笔记库 < 卡片阅读页 < 知识页 < 复习页 < 恢复点 < 导入提示 < 新手引导。
//!
//! 模态另有一条栈（[`Modal`] / [`State::top_modal`]），优先级从低到高：
//! 导出 < 快捷键 < 标签管理 < 库路径 < 文档信息 < 移出库确认 < 统计 < 设置 <
//! 主题工作室 < 历史 < 恢复中心 < 恢复提示 < 冲突 < 导入进度 < 正在打开 <
//! 打开失败 < 关闭确认 < 放弃确认 < 标签关闭确认 < 导入提示 < 新手引导。
//! 同一帧只绘制最上层模态；低层保留状态、不绘制也不接收输入，顶层关掉后自然恢复。
//!
//! 为什么导入提示最高：它通常在导入完成后才出现，用户刚做完动作，不能被旧页面盖住。
//! 恢复点提示是启动即弹出的模态，必须压过任何随后打开的面板——它开着时画布不能收键盘。

/// 浮层种类，顺序即优先级（`derive(Ord)` 按声明顺序）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    None,
    Context,
    Palette,
    Search,
    Export,
    Help,
    Stats,
    Shortcuts,
    TagManager,
    LibraryDialog,
    Library,
    Reader,
    Knowledge,
    Review,
    Recovery,
    ImportWarnings,
    Onboarding,
}

/// 模态种类，顺序即优先级（`derive(Ord)` 按声明顺序）。
///
/// 页面型浮层（笔记库、阅读页、知识页、复习页）不在这里：它们不参与弹窗互斥，
/// 只在最上层弹窗的暗幕后面继续作为背景存在。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Modal {
    Export,
    Shortcuts,
    TagManager,
    LibraryDialog,
    OutlineInfo,
    LibraryRemove,
    Stats,
    Settings,
    ThemeStudio,
    History,
    RecoveryCenter,
    RecoveryPrompt,
    Conflict,
    ImportBusy,
    Opening,
    OpenFailed,
    CloseConfirm,
    DiscardConfirm,
    TabCloseConfirm,
    ImportWarnings,
    Onboarding,
}

/// 各浮层的 open 状态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub context: bool,
    pub palette: bool,
    pub search: bool,
    pub export: bool,
    pub help: bool,
    pub stats: bool,
    pub shortcuts: bool,
    pub tag_manager: bool,
    pub library_dialog: bool,
    pub library: bool,
    pub reader: bool,
    pub knowledge: bool,
    pub review: bool,
    pub recovery: bool,
    pub import_warnings: bool,
    pub onboarding: bool,
    pub settings: bool,
    pub theme_studio: bool,
    pub history: bool,
    pub recovery_center: bool,
    pub conflict: bool,
    pub close_confirm: bool,
    pub discard_confirm: bool,
    pub tab_close_confirm: bool,
    pub import_busy: bool,
    pub opening: bool,
    pub open_failed: bool,
    pub outline_info: bool,
    pub library_remove: bool,
}

impl State {
    /// 最上层浮层；全关时是 [`Layer::None`]。
    pub fn top(self) -> Layer {
        if self.onboarding {
            Layer::Onboarding
        } else if self.import_warnings {
            Layer::ImportWarnings
        } else if self.recovery {
            Layer::Recovery
        } else if self.review {
            Layer::Review
        } else if self.knowledge {
            Layer::Knowledge
        } else if self.reader {
            Layer::Reader
        } else if self.library {
            Layer::Library
        } else if self.library_dialog {
            Layer::LibraryDialog
        } else if self.tag_manager {
            Layer::TagManager
        } else if self.shortcuts {
            Layer::Shortcuts
        } else if self.stats {
            Layer::Stats
        } else if self.help {
            Layer::Help
        } else if self.export {
            Layer::Export
        } else if self.search {
            Layer::Search
        } else if self.palette {
            Layer::Palette
        } else if self.context {
            Layer::Context
        } else {
            Layer::None
        }
    }

    /// 画布是否应锁掉指针与键盘手势。
    pub fn blocks_canvas(self) -> bool {
        self.top() != Layer::None || self.top_modal().is_some()
    }

    /// 最上层模态。`None` 表示当前没有模态弹窗（页面型浮层不算）。
    ///
    /// 只有最上层会被绘制并接收键盘；低层保留状态，顶层关掉后自然恢复。
    pub fn top_modal(self) -> Option<Modal> {
        if self.onboarding {
            Some(Modal::Onboarding)
        } else if self.import_warnings {
            Some(Modal::ImportWarnings)
        } else if self.tab_close_confirm {
            Some(Modal::TabCloseConfirm)
        } else if self.discard_confirm {
            Some(Modal::DiscardConfirm)
        } else if self.close_confirm {
            Some(Modal::CloseConfirm)
        } else if self.open_failed {
            Some(Modal::OpenFailed)
        } else if self.opening {
            Some(Modal::Opening)
        } else if self.import_busy {
            Some(Modal::ImportBusy)
        } else if self.conflict {
            Some(Modal::Conflict)
        } else if self.recovery {
            Some(Modal::RecoveryPrompt)
        } else if self.recovery_center {
            Some(Modal::RecoveryCenter)
        } else if self.history {
            Some(Modal::History)
        } else if self.theme_studio {
            Some(Modal::ThemeStudio)
        } else if self.settings {
            Some(Modal::Settings)
        } else if self.stats {
            Some(Modal::Stats)
        } else if self.library_remove {
            Some(Modal::LibraryRemove)
        } else if self.outline_info {
            Some(Modal::OutlineInfo)
        } else if self.library_dialog {
            Some(Modal::LibraryDialog)
        } else if self.tag_manager {
            Some(Modal::TagManager)
        } else if self.shortcuts {
            Some(Modal::Shortcuts)
        } else if self.export {
            Some(Modal::Export)
        } else {
            None
        }
    }

    /// 同时打开的模态数量。>1 说明有低层模态被压住，等顶层关掉后恢复。
    pub fn modal_count(self) -> usize {
        [
            self.export,
            self.shortcuts,
            self.tag_manager,
            self.library_dialog,
            self.outline_info,
            self.library_remove,
            self.stats,
            self.settings,
            self.theme_studio,
            self.history,
            self.recovery_center,
            self.recovery,
            self.conflict,
            self.import_busy,
            self.opening,
            self.open_failed,
            self.close_confirm,
            self.discard_confirm,
            self.tab_close_confirm,
            self.import_warnings,
            self.onboarding,
        ]
        .into_iter()
        .filter(|b| *b)
        .count()
    }

    /// 同时打开的浮层数量。>1 说明出现了本模块要防的"两个抢指针"。
    pub fn open_count(self) -> usize {
        [
            self.context,
            self.palette,
            self.search,
            self.export,
            self.help,
            self.stats,
            self.shortcuts,
            self.tag_manager,
            self.library_dialog,
            self.library,
            self.reader,
            self.knowledge,
            self.review,
            self.recovery,
            self.import_warnings,
            self.onboarding,
            self.settings,
            self.theme_studio,
            self.history,
            self.recovery_center,
            self.conflict,
            self.close_confirm,
            self.discard_confirm,
            self.tab_close_confirm,
            self.import_busy,
            self.opening,
            self.open_failed,
            self.outline_info,
            self.library_remove,
        ]
        .into_iter()
        .filter(|b| *b)
        .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_open_does_not_block() {
        let s = State::default();
        assert_eq!(s.top(), Layer::None);
        assert!(!s.blocks_canvas());
        assert_eq!(s.open_count(), 0);
    }

    #[test]
    fn priority_is_explicit_and_stable() {
        let mut s = State {
            context: true,
            search: true,
            ..Default::default()
        };
        assert_eq!(s.top(), Layer::Search, "搜索压过右键菜单");
        s.reader = true;
        assert_eq!(s.top(), Layer::Reader, "阅读页压过搜索");
        s.library = true;
        assert_eq!(s.top(), Layer::Reader, "阅读页仍高于笔记库");
        s.import_warnings = true;
        assert_eq!(s.top(), Layer::ImportWarnings);
        assert!(s.blocks_canvas());
    }

    #[test]
    fn recovery_prompt_owns_the_keyboard() {
        // 启动恢复点是模态：它开着时画布绝不能收 Tab / Enter，否则会在弹窗后面新建节点。
        let s = State {
            recovery: true,
            library: true,
            ..Default::default()
        };
        assert_eq!(s.top(), Layer::Recovery);
        assert!(s.blocks_canvas());
        assert_eq!(s.open_count(), 2);
    }

    #[test]
    fn open_count_catches_double_pointer_owners() {
        let s = State {
            library: true,
            library_dialog: true,
            ..Default::default()
        };
        assert_eq!(s.open_count(), 2);
        assert_eq!(s.top(), Layer::Library);
    }

    #[test]
    fn every_layer_is_reachable() {
        let cases = [
            (
                Layer::Context,
                State {
                    context: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Palette,
                State {
                    palette: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Search,
                State {
                    search: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Export,
                State {
                    export: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Help,
                State {
                    help: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Stats,
                State {
                    stats: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Shortcuts,
                State {
                    shortcuts: true,
                    ..Default::default()
                },
            ),
            (
                Layer::TagManager,
                State {
                    tag_manager: true,
                    ..Default::default()
                },
            ),
            (
                Layer::LibraryDialog,
                State {
                    library_dialog: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Library,
                State {
                    library: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Reader,
                State {
                    reader: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Knowledge,
                State {
                    knowledge: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Review,
                State {
                    review: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Recovery,
                State {
                    recovery: true,
                    ..Default::default()
                },
            ),
            (
                Layer::ImportWarnings,
                State {
                    import_warnings: true,
                    ..Default::default()
                },
            ),
            (
                Layer::Onboarding,
                State {
                    onboarding: true,
                    ..Default::default()
                },
            ),
        ];
        for (want, state) in cases {
            assert_eq!(state.top(), want);
        }
    }

    #[test]
    fn no_modal_means_none_even_with_pages_open() {
        let s = State {
            reader: true,
            library: true,
            knowledge: true,
            review: true,
            ..Default::default()
        };
        assert_eq!(s.top_modal(), None);
        assert_eq!(s.modal_count(), 0);
        assert!(s.blocks_canvas(), "页面型浮层仍要锁画布");
    }

    #[test]
    fn destructive_confirmation_sits_above_settings() {
        let s = State {
            settings: true,
            close_confirm: true,
            ..Default::default()
        };
        assert_eq!(s.top_modal(), Some(Modal::CloseConfirm));
        assert_eq!(s.modal_count(), 2, "低层模态保留状态，等顶层退场");
        assert!(s.blocks_canvas());
    }

    #[test]
    fn onboarding_and_import_warnings_own_the_top() {
        let s = State {
            onboarding: true,
            import_warnings: true,
            close_confirm: true,
            ..Default::default()
        };
        assert_eq!(s.top_modal(), Some(Modal::Onboarding));
        let s = State {
            onboarding: false,
            ..s
        };
        assert_eq!(s.top_modal(), Some(Modal::ImportWarnings));
    }

    #[test]
    fn every_modal_is_reachable() {
        let cases = [
            (
                Modal::Export,
                State {
                    export: true,
                    ..Default::default()
                },
            ),
            (
                Modal::Shortcuts,
                State {
                    shortcuts: true,
                    ..Default::default()
                },
            ),
            (
                Modal::TagManager,
                State {
                    tag_manager: true,
                    ..Default::default()
                },
            ),
            (
                Modal::LibraryDialog,
                State {
                    library_dialog: true,
                    ..Default::default()
                },
            ),
            (
                Modal::OutlineInfo,
                State {
                    outline_info: true,
                    ..Default::default()
                },
            ),
            (
                Modal::LibraryRemove,
                State {
                    library_remove: true,
                    ..Default::default()
                },
            ),
            (
                Modal::Stats,
                State {
                    stats: true,
                    ..Default::default()
                },
            ),
            (
                Modal::Settings,
                State {
                    settings: true,
                    ..Default::default()
                },
            ),
            (
                Modal::ThemeStudio,
                State {
                    theme_studio: true,
                    ..Default::default()
                },
            ),
            (
                Modal::History,
                State {
                    history: true,
                    ..Default::default()
                },
            ),
            (
                Modal::RecoveryCenter,
                State {
                    recovery_center: true,
                    ..Default::default()
                },
            ),
            (
                Modal::RecoveryPrompt,
                State {
                    recovery: true,
                    ..Default::default()
                },
            ),
            (
                Modal::Conflict,
                State {
                    conflict: true,
                    ..Default::default()
                },
            ),
            (
                Modal::ImportBusy,
                State {
                    import_busy: true,
                    ..Default::default()
                },
            ),
            (
                Modal::Opening,
                State {
                    opening: true,
                    ..Default::default()
                },
            ),
            (
                Modal::OpenFailed,
                State {
                    open_failed: true,
                    ..Default::default()
                },
            ),
            (
                Modal::CloseConfirm,
                State {
                    close_confirm: true,
                    ..Default::default()
                },
            ),
            (
                Modal::DiscardConfirm,
                State {
                    discard_confirm: true,
                    ..Default::default()
                },
            ),
            (
                Modal::TabCloseConfirm,
                State {
                    tab_close_confirm: true,
                    ..Default::default()
                },
            ),
            (
                Modal::ImportWarnings,
                State {
                    import_warnings: true,
                    ..Default::default()
                },
            ),
            (
                Modal::Onboarding,
                State {
                    onboarding: true,
                    ..Default::default()
                },
            ),
        ];
        for (want, state) in cases {
            assert_eq!(state.top_modal(), Some(want));
            assert_eq!(state.modal_count(), 1);
        }
    }

    #[test]
    fn modal_stack_orders_low_to_high() {
        // 声明顺序即优先级：低层保留状态，但只有高层被绘制与接收输入。
        let order = [
            Modal::Export,
            Modal::Shortcuts,
            Modal::TagManager,
            Modal::LibraryDialog,
            Modal::OutlineInfo,
            Modal::LibraryRemove,
            Modal::Stats,
            Modal::Settings,
            Modal::ThemeStudio,
            Modal::History,
            Modal::RecoveryCenter,
            Modal::RecoveryPrompt,
            Modal::Conflict,
            Modal::ImportBusy,
            Modal::Opening,
            Modal::OpenFailed,
            Modal::CloseConfirm,
            Modal::DiscardConfirm,
            Modal::TabCloseConfirm,
            Modal::ImportWarnings,
        ];
        let mut state = State::default();
        for (i, want) in order.into_iter().enumerate() {
            enable(&mut state, want);
            assert_eq!(
                state.top_modal(),
                Some(want),
                "第 {i} 层 {want:?} 没有压住下面所有层"
            );
            assert_eq!(state.modal_count(), i + 1);
        }
        state.onboarding = true;
        assert_eq!(state.top_modal(), Some(Modal::Onboarding));
        assert_eq!(state.modal_count(), 21);
    }

    fn enable(state: &mut State, modal: Modal) {
        match modal {
            Modal::Export => state.export = true,
            Modal::Shortcuts => state.shortcuts = true,
            Modal::TagManager => state.tag_manager = true,
            Modal::LibraryDialog => state.library_dialog = true,
            Modal::OutlineInfo => state.outline_info = true,
            Modal::LibraryRemove => state.library_remove = true,
            Modal::Stats => state.stats = true,
            Modal::Settings => state.settings = true,
            Modal::ThemeStudio => state.theme_studio = true,
            Modal::History => state.history = true,
            Modal::RecoveryCenter => state.recovery_center = true,
            Modal::RecoveryPrompt => state.recovery = true,
            Modal::Conflict => state.conflict = true,
            Modal::ImportBusy => state.import_busy = true,
            Modal::Opening => state.opening = true,
            Modal::OpenFailed => state.open_failed = true,
            Modal::CloseConfirm => state.close_confirm = true,
            Modal::DiscardConfirm => state.discard_confirm = true,
            Modal::TabCloseConfirm => state.tab_close_confirm = true,
            Modal::ImportWarnings => state.import_warnings = true,
            Modal::Onboarding => state.onboarding = true,
        }
    }
}
