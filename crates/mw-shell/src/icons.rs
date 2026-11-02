//! 统一图标枚举：工具条、菜单、轨迹语义名共用一套变体。
//!
//! 两个旧枚举（`ToolIcon` / `MenuIcon`）变体不冲突，合并后保留类型别名，
//! 绘制函数仍按用途分组，但“图标有哪些”只有这一处。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Levels/Density 暂无消费者（CER-04 页头收纳进更多菜单后改用菜单行）；
/// 图标是全集注册（Icon::ALL 测试穷举），变体保留。
#[allow(dead_code)]
pub(crate) enum Icon {
    // 工具条
    Undo,
    Redo,
    AddChild,
    AddSibling,
    Delete,
    Fit,
    // 概念图工具条（`docs/33` §5）
    Concept,
    Link,
    Phrase,
    Arrange,
    Pin,
    FocusQ,
    // 菜单
    New,
    Open,
    Save,
    SaveAs,
    Export,
    File,
    Search,
    Panel,
    Home,
    Theme,
    Note,
    Library,
    Tag,
    Exit,
    // 导航与紧凑控件
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Close,
    More,
    Plus,
    // 大纲页头
    Back,
    Levels,
    Density,
    Focus,
    // 起始页结构图标：模板卡与「新建概念图」入口共用
    ConceptMap,
    Mindmap,
    Logic,
    Org,
    Fishbone,
    Timeline,
}

pub(crate) type ToolIcon = Icon;
pub(crate) type MenuIcon = Icon;

impl Icon {
    /// 全部图标：新增变体时这条列表与下面 match 必须同步（穷尽 match 会兜住）。
    #[cfg(test)]
    pub(crate) const ALL: [Icon; 43] = [
        Icon::Undo,
        Icon::Redo,
        Icon::AddChild,
        Icon::AddSibling,
        Icon::Delete,
        Icon::Fit,
        Icon::Concept,
        Icon::Link,
        Icon::Phrase,
        Icon::Arrange,
        Icon::Pin,
        Icon::FocusQ,
        Icon::New,
        Icon::Open,
        Icon::Save,
        Icon::SaveAs,
        Icon::Export,
        Icon::File,
        Icon::Search,
        Icon::Panel,
        Icon::Home,
        Icon::Theme,
        Icon::Note,
        Icon::Library,
        Icon::Tag,
        Icon::Exit,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::ChevronDown,
        Icon::ChevronUp,
        Icon::Close,
        Icon::More,
        Icon::Plus,
        Icon::Back,
        Icon::Levels,
        Icon::Density,
        Icon::Focus,
        Icon::ConceptMap,
        Icon::Mindmap,
        Icon::Logic,
        Icon::Org,
        Icon::Fishbone,
        Icon::Timeline,
    ];

    /// 轨迹里认格子用的语义名。
    pub(crate) fn trace_name(self) -> &'static str {
        match self {
            Icon::Undo => "undo",
            Icon::Redo => "redo",
            Icon::AddChild => "child",
            Icon::AddSibling => "sibling",
            Icon::Delete => "delete",
            Icon::Fit => "fit",
            Icon::Concept => "concept",
            Icon::Link => "link",
            Icon::Phrase => "phrase",
            Icon::Arrange => "arrange",
            Icon::Pin => "pin",
            Icon::FocusQ => "focus-q",
            Icon::New => "new",
            Icon::Open => "open",
            Icon::Save => "save",
            Icon::SaveAs => "save-as",
            Icon::Export => "export",
            Icon::File => "file",
            Icon::Search => "search",
            Icon::Panel => "panel",
            Icon::Home => "home",
            Icon::Theme => "theme",
            Icon::Note => "note",
            Icon::Library => "library",
            Icon::Tag => "tag",
            Icon::Exit => "exit",
            Icon::ChevronLeft => "chevron-left",
            Icon::ChevronRight => "chevron-right",
            Icon::ChevronDown => "chevron-down",
            Icon::ChevronUp => "chevron-up",
            Icon::Close => "close",
            Icon::More => "more",
            Icon::Plus => "plus",
            Icon::Back => "back",
            Icon::Levels => "levels",
            Icon::Density => "density",
            Icon::Focus => "focus",
            Icon::ConceptMap => "concept-map",
            Icon::Mindmap => "mindmap",
            Icon::Logic => "logic",
            Icon::Org => "org",
            Icon::Fishbone => "fishbone",
            Icon::Timeline => "timeline",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_icon_has_a_unique_trace_name() {
        assert_eq!(Icon::ALL.len(), 43);
        let mut seen = HashSet::new();
        for icon in Icon::ALL {
            assert!(seen.insert(icon.trace_name()), "{icon:?} 重名");
        }
    }
}
