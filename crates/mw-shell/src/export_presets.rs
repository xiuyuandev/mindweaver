//! 导出预设与批量命名。
//!
//! 预设存在 `%APPDATA%\织思\export-presets.json`；批量导出时用
//! [`unique_path`] 生成不冲突的文件名（`方案.png` → `方案-2.png`）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preset {
    pub name: String,
    /// png / svg / pdf / md / txt / opml / mm
    pub format: String,
    /// whole / selection / viewport
    pub scope: String,
    pub scale: f32,
    pub opaque: bool,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "默认".into(),
            format: "png".into(),
            scope: "whole".into(),
            scale: 2.0,
            opaque: true,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Store {
    presets: Vec<Preset>,
}

fn store_path() -> Option<PathBuf> {
    crate::paths::appdata_root().map(|d| d.join("export-presets.json"))
}

pub fn load() -> Vec<Preset> {
    let Some(path) = store_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Store>(text.trim_start_matches('\u{feff}'))
        .map(|s| s.presets)
        .unwrap_or_default()
}

pub fn save(presets: &[Preset]) -> Result<(), String> {
    let Some(path) = store_path() else {
        return Err("找不到应用数据目录".into());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&Store {
        presets: presets.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// 在目录里找一个不冲突的名字：基础名被占用就加 `-2`、`-3`……
/// 只检查磁盘上是否存在；不创建文件，调用方负责真正落盘。
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let safe = if stem.trim().is_empty() {
        "导出"
    } else {
        stem
    };
    let mut n = 1u32;
    loop {
        let name = if n == 1 {
            format!("{safe}.{ext}")
        } else {
            format!("{safe}-{n}.{ext}")
        };
        let path = dir.join(name);
        if !path.exists() {
            return path;
        }
        n += 1;
        if n > 9999 {
            return dir.join(format!("{safe}-{}.{ext}", mw_core::new_id()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-export-{}", mw_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn preset_roundtrips_through_json() {
        let p = Preset {
            name: "高清 PNG".into(),
            format: "png".into(),
            scope: "whole".into(),
            scale: 3.0,
            opaque: false,
        };
        let text = serde_json::to_string(&Store {
            presets: vec![p.clone()],
        })
        .unwrap();
        let back: Store = serde_json::from_str(&text).unwrap();
        assert_eq!(back.presets, vec![p]);
    }

    #[test]
    fn batch_names_do_not_collide() {
        let dir = temp_dir();
        std::fs::write(dir.join("方案.png"), b"x").unwrap();
        std::fs::write(dir.join("方案-2.png"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "方案", "png"), dir.join("方案-3.png"));
        assert_eq!(unique_path(&dir, "", "md"), dir.join("导出.md"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
