//! 应用数据路径：一处定义，自动保存、卡片库、备份都用同一套目录。
//!
//! 只认 `%APPDATA%`；没有就当没有（测试/便携环境不写用户目录）。不建目录，
//! 由调用方在真正写盘时创建——启动时不该因为读一个索引就建三个空目录。

use std::path::PathBuf;

/// `%APPDATA%\织思`。
pub fn appdata_root() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    Some(base.join("织思"))
}

/// 卡片库：`%APPDATA%\织思\notes`。
pub fn notes_dir() -> Option<PathBuf> {
    appdata_root().map(|d| d.join("notes"))
}

/// 复习状态：`%APPDATA%\织思\review.json`（本地、可手工删除）。
pub fn review_file() -> Option<PathBuf> {
    appdata_root().map(|d| d.join("review.json"))
}
