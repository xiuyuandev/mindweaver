//! 主题编辑器：编辑现有主题微调槽位（ThemeOverrides），即时预览，保存为命名预设。
//!
//! 预设存在 `%APPDATA%\织思\themes.json`。文档本身仍只存 base 主题 id + overrides，
//! 所以换机器打开文档不需要预设文件也能还原效果；预设只是“快速套用”的入口。

use std::path::{Path, PathBuf};

use eframe::egui;
use mw_core::theme::Rgba;
use mw_core::{Theme, ThemeOverrides};

use crate::chrome::Chrome;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preset {
    pub name: String,
    pub base: String,
    pub overrides: ThemeOverrides,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Store {
    presets: Vec<Preset>,
}

fn store_path() -> Option<PathBuf> {
    crate::paths::appdata_root().map(|d| d.join("themes.json"))
}

pub fn load_presets() -> Vec<Preset> {
    store_path()
        .map(|p| load_presets_at(&p))
        .unwrap_or_default()
}

pub fn load_presets_at(path: &Path) -> Vec<Preset> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Store>(text.trim_start_matches('\u{feff}'))
        .map(|s| s.presets)
        .unwrap_or_default()
}

pub fn save_presets(presets: &[Preset]) -> Result<(), String> {
    let Some(path) = store_path() else {
        return Err("找不到应用数据目录".into());
    };
    save_presets_at(&path, presets)
}

pub fn save_presets_at(path: &Path, presets: &[Preset]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&Store {
        presets: presets.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub struct Studio {
    pub name: String,
    pub base: String,
    pub draft: ThemeOverrides,
    pub presets: Vec<Preset>,
    /// 打开时的基线：取消时恢复，避免“点进来就被改了”。
    pub baseline: Option<ThemeOverrides>,
    pub baseline_base: String,
    pub last_applied: Option<ThemeOverrides>,
    /// 待确认删除的预设下标：点一次叉号先问一句，避免误删。
    pub pending_delete: Option<usize>,
}

impl Studio {
    pub fn new(base: String, current: Option<ThemeOverrides>) -> Self {
        let baseline = current.clone();
        Self {
            name: "我的主题".into(),
            base: base.clone(),
            draft: current.unwrap_or_default(),
            presets: load_presets(),
            baseline,
            baseline_base: base,
            last_applied: None,
            pending_delete: None,
        }
    }

    /// 草稿相对打开时是否有改动（换基底也算）。
    pub fn is_dirty(&self) -> bool {
        self.base != self.baseline_base || self.draft != self.baseline.clone().unwrap_or_default()
    }

    /// 把当前草稿存成命名预设；同名覆盖。返回 `true` 表示覆盖了旧预设。
    pub fn upsert_named(&mut self) -> Result<bool, String> {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err("预设名不能为空".into());
        }
        let preset = Preset {
            name: name.clone(),
            base: self.base.clone(),
            overrides: self.draft.clone(),
        };
        self.name = name.clone();
        if let Some(old) = self.presets.iter_mut().find(|p| p.name == name) {
            *old = preset;
            Ok(true)
        } else {
            self.presets.push(preset);
            Ok(false)
        }
    }

    /// 套用预设到草稿：基底、槽位、名字一并就位，接着改再保存就是更新该预设。
    pub fn use_preset(&mut self, i: usize) -> Option<Preset> {
        let p = self.presets.get(i)?.clone();
        self.base = p.base.clone();
        self.draft = p.overrides.clone();
        self.name = p.name.clone();
        Some(p)
    }

    pub fn delete_at(&mut self, i: usize) -> Option<Preset> {
        self.pending_delete = None;
        if i < self.presets.len() {
            Some(self.presets.remove(i))
        } else {
            None
        }
    }

    /// 清空全部微调，只留基底主题。
    pub fn clear_overrides(&mut self) {
        self.draft = ThemeOverrides::default();
    }

    pub fn restore_baseline(&mut self) {
        self.base = self.baseline_base.clone();
        self.draft = self.baseline.clone().unwrap_or_default();
    }

    /// 预览条要画的实际颜色：基底主题 + 当前草稿。
    pub fn resolved(&self) -> Theme {
        mw_core::get_theme(&self.base).with_overrides(Some(&self.draft))
    }
}

#[cfg(test)]
pub fn hex(rgba: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", rgba.0[0], rgba.0[1], rgba.0[2])
}

/// 一个颜色槽位：开关启用 + 取色器 + 只读 hex。
pub fn color_slot(ui: &mut egui::Ui, ch: Chrome, label: &str, slot: &mut Option<String>) {
    ui.horizontal(|ui| {
        let mut on = slot.is_some();
        if crate::ui_kit::switch(ui, ch, label, &mut on, true).changed() {
            *slot = if on { Some("#808080".into()) } else { None };
        }
        ui.label(label);
        if let Some(hex) = slot.as_mut() {
            let parsed = Rgba::parse(hex);
            let mut rgb = parsed
                .map(|c| [c.0[0], c.0[1], c.0[2]])
                .unwrap_or([128, 128, 128]);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                *hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
            }
            ui.monospace(hex.as_str());
            if parsed.is_none() {
                ui.colored_label(egui::Color32::RED, "无效颜色");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_roundtrip_through_json() {
        let p = Preset {
            name: "暖夜".into(),
            base: "ink".into(),
            overrides: ThemeOverrides {
                canvas_bg: Some("#101014".into()),
                text_sub: Some("#d0d0d0".into()),
                edge_scale: Some(1.4),
                edge_taper: Some(false),
                ..Default::default()
            },
        };
        let text = serde_json::to_string(&Store {
            presets: vec![p.clone()],
        })
        .unwrap();
        let back: Store = serde_json::from_str(&text).unwrap();
        assert_eq!(back.presets, vec![p]);
    }

    #[test]
    fn hex_formats_without_alpha() {
        assert_eq!(hex(Rgba::rgb(1, 2, 3)), "#010203");
    }

    fn temp_store(tag: &str) -> (PathBuf, PathBuf) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("zhisi-theme-{tag}-{}-{stamp}", std::process::id()));
        (dir.join("themes.json"), dir)
    }

    #[test]
    fn presets_persist_create_apply_delete_at_an_explicit_path() {
        let (path, dir) = temp_store("persist");
        let mut s = Studio::new("snow".into(), None);
        s.draft.canvas_bg = Some("#101014".into());
        s.name = "  暖夜  ".into();
        assert!(!s.upsert_named().unwrap(), "首次保存应是新建");
        assert_eq!(s.presets[0].name, "暖夜");
        save_presets_at(&path, &s.presets).unwrap();

        let loaded = load_presets_at(&path);
        assert_eq!(loaded, s.presets);

        s.draft.edge_scale = Some(1.3);
        assert!(s.upsert_named().unwrap(), "同名保存应覆盖");
        assert_eq!(s.presets.len(), 1);

        let mut t = Studio::new("ink".into(), None);
        t.presets = loaded;
        let applied = t.use_preset(0).unwrap();
        assert_eq!(applied.name, "暖夜");
        assert_eq!(t.base, "snow");
        assert_eq!(t.draft.canvas_bg.as_deref(), Some("#101014"));
        assert_eq!(t.name, "暖夜", "套用后名字应跟着预设，便于继续更新");

        assert!(t.delete_at(0).is_some());
        save_presets_at(&path, &t.presets).unwrap();
        assert!(load_presets_at(&path).is_empty(), "删除后应写回空列表");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn blank_preset_name_is_rejected() {
        let mut s = Studio::new("snow".into(), None);
        s.name = "   ".into();
        assert!(s.upsert_named().is_err());
        assert!(s.presets.is_empty());
    }

    #[test]
    fn dirty_tracks_draft_and_base_against_the_opening_baseline() {
        let mut s = Studio::new("snow".into(), None);
        assert!(!s.is_dirty());
        s.draft.text_sub = Some("#333333".into());
        assert!(s.is_dirty());
        s.restore_baseline();
        assert!(!s.is_dirty());
        s.base = "ink".into();
        assert!(s.is_dirty());
        s.base = "snow".into();
        assert!(!s.is_dirty());
        s.clear_overrides();
        assert!(!s.is_dirty(), "清空空微调回到基线也应视为干净");
    }
}
