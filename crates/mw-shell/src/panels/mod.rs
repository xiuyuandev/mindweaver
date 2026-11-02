//! 侧边面板（M3-c）。**只读会话、只发命令**，不直接改 `Document`。
//!
//! 面板与画布的分工：画布负责"看这块图"，面板负责"看这份文档的结构与属性"。
//! 两者读同一份 `Session`，写都走 `Command`——撤销栈因此不会断。

pub mod command;
pub mod context;
pub mod focus;
pub mod help;
pub mod inspector;
pub mod outline;
pub mod search;

/// 面板统一内边距（左/右）：取设计系统的 LG 档（16px）。
///
/// 以前各面板自己写 14 / 12 / 16，左边距差 2px 在多块面板并排时**看得出来**
/// ——文字起始线不在一条竖线上。现在只有一个来源。
pub const PAD: f32 = mw_core::design::space::LG;

pub use command::Palette;
pub use context::Menu;
pub use focus::{FocusMemory, PanelId};
pub use help::Help;
pub use inspector::Inspector;
pub use outline::Outline;
pub use search::SearchBar;
