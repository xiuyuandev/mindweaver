//! 织思手势层（M2b）——**纯逻辑**，不含任何窗口、GPU 或 egui 依赖。
//!
//! 拆出来只为一个理由：落点判定、子树禁令、多根插入索引、吸附阈值这几件事，
//! 是「拖拽手感」的全部风险所在，必须能在没有显示器的机器上跑断言。壳层只负责
//! 把指针事件翻译成这里的输入、把这里的输出画出来。
//!
//! 规格来自 docs/06 §4 交互表与旧 Svelte 实现（`ui/src/Canvas.svelte`），两处
//! 与旧版**故意不同**：
//! · 落点命中到自身子树时不再「什么都不显示」，而是给出 `illegal` 目标——
//!   docs/06 要求「指示变红 + 禁止光标」，静默会让用户以为拖拽坏了；
//! · 多根落点索引按「逐条模拟父子表」算，不再用 `+seq` 近似——旧算法在同父
//!   重排时会差一位。

pub mod arrange;
pub mod drag;
pub mod drill_click;
pub mod link;
pub mod marquee;
pub mod overview;
pub mod select;
pub mod snap;

pub use drag::{DropTarget, Tree, Zone};
pub use link::Side;
pub use marquee::intersecting;
pub use overview::{blocks_for, center_from_thumb, sample, view_rect, Map, Proxy};
pub use select::Selection;
pub use snap::{snap_offsets, snapped, Guide, Snap};
