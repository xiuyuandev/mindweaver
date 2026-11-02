//! 画布渲染层：把 `mw-core` 的 `Scene` 画成 wgpu 四层合成结果。
//!
//! 分工写死在这里，别混：
//! - `instances` 出**实例数据**（纯函数，不看 GPU，可单测、可快照比对）；
//! - `compositor` 只管管线、缓冲与混合，收到什么实例就画什么；
//! - `camera` 负责世界↔物理像素，以及「铬件 ÷缩放 / 形状随缩放」这条分界；
//! - `flatten` 把三阶贝塞尔细分成屏幕恒定容差的折线段。
//!
//! 文本度量在 `dwrite`（真实字体宽度 + 中文禁则，docs/07 §4/§5）；字形光栅化与图集
//! 在 `glyph`，场景文字 → 文本实例在 `text`（M1b-2b，痛点 C3）；命中在 `hit`
//! （uniform grid，纯 CPU，M1b-3）。

pub mod budget;
pub mod camera;
pub mod compositor;
pub mod dwrite;
pub mod flatten;
pub mod glyph;
pub mod hit;
pub mod instances;
pub mod outline;
pub mod text;

pub use budget::{FrameBudget, LOW_AFTER, LOW_FPS};
pub use camera::{Camera, CameraUni};
pub use compositor::{Compositor, Image, RenderError};
pub use dwrite::{DWrite, DWriteMeasurer, FALLBACK_FAMILY, FONT_CANDIDATES, LOCALE};
pub use glyph::{AtlasUpdate, GlyphAtlas, GlyphKey, GlyphSlot, ATLAS_SIZE};
pub use hit::{owner_node, HitIndex, Target, CELL, REL_HIT_CSS};
pub use instances::{
    build_layers, corner, corner_style, rgba, Frame, Ghost, GuideLine, LayerGeom, RectInst,
    SegInst, TextInst, TriInst, DROP_BAD, ELLIPSE_CODE, LAYERS, L_BACKDROP, L_LINES, L_NODES,
    L_OVERLAY, SELECT_RING_GAP, SELECT_RING_W,
};
pub use text::{text_jobs, OutlinedText, TextAlign, TextEngine, TextJob};
