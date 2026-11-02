//! 织思 MindWeaver 领域核心。
//!
//! 本 crate 不含任何 UI 依赖：文档模型、命令与撤销、拼音检索都在这里，
//! 可独立 `cargo test` 验证（见文档 04 §1 铁律 2）。

pub mod assets;
pub mod command;
pub mod concept_layout;
pub mod concept_tree;
pub mod design;
pub mod document;
pub mod drill;
pub mod geom;
pub mod icons;
pub mod image;
pub mod layout;
pub mod model;
pub mod note;
pub mod phrase;
pub mod references;
pub mod scene;
pub mod search;
pub mod text;
pub mod theme;

#[cfg(test)]
mod tfix;

pub use command::Command;
pub use concept_tree::{concept_rows, ConceptRow};
pub use document::{
    Backlink, ChangeSet, CoreError, Document, WireChange, HISTORY_LIMIT, MAX_HISTORY_LIMIT,
    MIN_HISTORY_LIMIT,
};
pub use geom::{arrow_at_zoom, Path, PathSeg, Point, Rect, ARROW_MIN_CSS_LEN};
pub use layout::{compute_layout, EdgeShape, Layout, LayoutKind, NodeBox, NodeMeasure};
pub use model::{
    new_id, now_ms, Boundary, DocMeta, LayoutSettings, Node, Position, Relationship, StyleOverride,
    Summary, TaskState, ThemeOverrides,
};
pub use note::NoteCard;
pub use phrase::{linking_phrases, PHRASE_SUGGEST_LIMIT};
pub use scene::{
    blend_scenes, build_scene, build_scene_cached, IconMark, LabelPill, Scene, SceneBoundary,
    SceneCache, SceneEdge, SceneGroup, SceneNode, SceneRelationship, SceneSummary, Side,
    GROUP_LABEL_H, GROUP_PAD, REL_DASH, REL_LABEL_FONT, SUMMARY_FONT,
};
pub use search::{MatchMode, RelationshipHit, SearchHit};
pub use text::{
    enforce_kinsoku, forbidden_line_end, forbidden_line_start, is_cjk, line_height, Measured,
    MonoMeasurer, TextMeasurer, FORBIDDEN_LINE_END, FORBIDDEN_LINE_START, NO_WRAP,
};
pub use theme::{get_theme, resolve_all, theme_for, ResolvedStyle, Theme};
