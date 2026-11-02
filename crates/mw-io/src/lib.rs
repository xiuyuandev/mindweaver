//! 导入导出（文档 02 §8 矩阵、05 §7 映射）。
//!
//! 导入器：Markdown / 缩进大纲文本 / OPML / FreeMind(.mm)。
//! 导出器：Markdown / 缩进大纲文本 / OPML / FreeMind。
//! 画布类导出（PNG/SVG）由前端场景图负责（ADR-003）。

pub mod export;
pub mod import;
pub mod xmind;

pub mod cmap;
pub mod dispatch;

pub use export::{export_freemind, export_markdown, export_opml, export_outline};
pub use import::{import_freemind, import_markdown, import_opml, import_outline};
pub use xmind::{export_xmind, import_xmind, import_xmind_with_report};

/// 导入保真统计：导入了什么、哪些字段有内容。UI 可用来确认"带走了多少"。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportCounts {
    pub nodes: usize,
    pub notes: usize,
    pub links: usize,
    pub labels: usize,
    pub relationships: usize,
    pub collapsed: usize,
    pub styled: usize,
}

/// 一次导入的"丢失项/降级项"报告。UI 负责展示，导入器只如实记录。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// 图片/附件等资源条目数量（已收进资源包、主题图片引用已还原）。
    pub resources: usize,
    /// 成功迁移的字段统计。
    pub counts: ImportCounts,
    /// 人话警告；空表示没有丢失项。
    pub warnings: Vec<String>,
}

impl ImportReport {
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    pub fn is_empty(&self) -> bool {
        self.resources == 0 && self.warnings.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("文件内容为空，无法导入")]
    Empty,
    #[error("XML 解析失败：{0}")]
    Xml(String),
    #[error("导入结果无效：{0}")]
    Invalid(String),
}
