//! 右键菜单（docs/06 §6、docs/24 X16）。
//!
//! 这里只定义「画布右键能发什么意图」和菜单条目工厂；卡片绘制、子菜单、
//! 边缘翻转与键盘导航都在 [`crate::menu`]。壳层按画布状态组装条目，模块不读文档。
//!
//! 键盘先于画布消费（`↑↓` 换行、`←→` 进出子菜单、`Enter` 执行、`Esc` 收层）；
//! 其它按键收起菜单但**不吞掉**——`Ctrl+Tab` 之类该继续生效。

use crate::icons::Icon;
use crate::menu::Entry;

/// 菜单能发出的意图。壳层负责把它落到画布命令或面板动作上。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    AddChild,
    AddSibling,
    Cut,
    Copy,
    Paste,
    PasteAtRoot,
    Delete,
    EditText,
    /// 进入「连线」态：下一次点击另一个主题/概念落地（`Ctrl+L`）。
    /// 概念图里选中两个概念时是「直连」，选中一个才是连线态（`docs/30` §5.4.2）。
    BeginRelationship,
    /// 概念图：新建一个概念并**立刻连一条命题**（CmapTools 的 New Concept 语义）。
    AddLinkedConcept,
    /// 反向当前选中的关系线（交换两端、箭头镜像、一次撤销）。
    ReverseRelationship,
    /// 改当前选中关系线的线型（实线 / 虚线，CM-16）。
    SetRelationshipDashed(bool),
    /// 把当前选中的概念建成一个分组（CM-18）。
    CreateGroup,
    /// 解组：只删框，概念一个不删。
    DeleteGroup,
    /// 改右键那个分组的框色（`None` = 跟随主题）。
    SetGroupColor(Option<&'static str>),
    /// 选中分组里的全部概念。
    SelectGroupMembers,
    /// 给主选主题生成/打开卡片笔记。
    NewCard,
    CopyStyle,
    PasteStyle,
    ToggleCollapse,
    CollapseSelected,
    MoveSameParent,
    SetStyle,
    /// 给选中的（同一父级）主题加概要括号。
    AddSummary,
    /// 给选中的（同一父级）主题加边界。
    AddBoundary,
    /// 删除当前选中的注解（概要/边界）。
    DeleteAnnotation,
    /// 删除当前选中的关系线。
    DeleteRelationship,
    /// 改当前选中关系线的端点箭头（CM-10 ⑦）。
    SetRelationshipHead(mw_core::model::ArrowHead),
    /// 编辑当前选中关系线的标签。
    EditRelationshipLabel,
    /// 对齐选中的自由主题。
    AlignFree(mw_ui::arrange::Align),
    /// 等距分布选中的自由主题。
    DistributeFree(mw_ui::arrange::Distribute),
    /// 概念图「整理布局」：分层重排整张图，一次撤销（CM-04）。
    ArrangeConceptMap,
    /// 概念图「整理选中概念」：仅重排选中的概念，其余原样保留（C18）。
    ArrangeSelectedConcepts,
    /// 概念图「钉住/取消钉住」概念位置（C18）。
    ToggleConceptPinned,
    /// 修改当前选中边界的颜色；`None` = 跟随主题。
    SetBoundaryColor(Option<&'static str>),
    /// 编辑当前选中概要的文字（画布内联编辑器）。
    EditSummary,
    /// 把主选原地分离成自由主题。
    Detach,
    /// 把主选提为中心主题（M31，仅思维导图）。
    SetRoot,
    /// 在画布空白处新建自由主题。
    AddFloating,
    Focus,
    FitView,
    ResetZoom,
    ExpandAll,
    CollapseAll,
    RevealFolder,
}

/// 画布右键菜单。引擎在 [`crate::menu`]，这里保留画布专用的动作类型。
pub type Menu = crate::menu::Menu<Action>;

/// 条目工厂：让 `build_context_menu` 读起来仍是「一行一个菜单项」。
pub struct Item;

impl Item {
    pub fn row(
        label: impl Into<String>,
        shortcut: &'static str,
        action: Action,
        enabled: bool,
    ) -> Entry<Action> {
        Entry::action(label, shortcut, action, enabled)
    }

    pub fn row_icon(
        label: impl Into<String>,
        shortcut: &'static str,
        icon: Icon,
        action: Action,
        enabled: bool,
    ) -> Entry<Action> {
        Entry::action_with_icon(label, shortcut, icon, action, enabled)
    }

    /// 一组互斥选项里的一行：当前生效的那个打勾（CM-10 ⑦ 的箭头四档用它）。
    pub fn row_checked(
        label: impl Into<String>,
        shortcut: &'static str,
        action: Action,
        enabled: bool,
        checked: bool,
    ) -> Entry<Action> {
        Entry::action_checked(label, shortcut, action, enabled, checked)
    }

    pub fn sep() -> Entry<Action> {
        Entry::separator()
    }

    pub fn submenu(
        label: impl Into<String>,
        enabled: bool,
        entries: Vec<Entry<Action>>,
    ) -> Entry<Action> {
        Entry::submenu(label, enabled, entries)
    }

    pub fn section(text: impl Into<String>) -> Entry<Action> {
        Entry::section(text)
    }
}
