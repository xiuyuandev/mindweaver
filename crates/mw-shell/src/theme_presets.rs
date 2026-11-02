//! 我的主题：把「基础主题 + 文档级微调 + 强调色」存成一条命名预设，跨文档复用。
//!
//! 存在 %APPDATA%\织思\theme-presets.json。预设只描述「怎么组合」，不复制整套 Theme：
//! 打开时用 built-in 主题 + 覆盖实时算出来，内置主题改了配色预设也会跟着变。
//!
//! 应用预设 = 一条 Batch（SetTheme + SetThemeOverrides + SetThemeAccent），
//! 所以撤销一次就回到套用之前，不会在历史里留下三步。

use std::path::{Path, PathBuf};

use mw_core::model::ThemeOverrides;
use serde::{Deserialize, Serialize};

/// 一条命名主题预设。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    /// 内置主题 id（snow / party / …）。
    pub theme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<ThemeOverrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

impl Preset {
    /// 名字统一去空白、限长；空名字返回 None（调用方给个默认名）。
    pub fn sanitize_name(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.chars().take(24).collect())
    }
}

/// 套用一条预设要发的命令：换基础主题 + 微调 + 强调色，包成一条 Batch（一次撤销）。
///
/// 抽成纯函数是为了能单测：面板只发 `ApplyPreset(i)`，真正落地在这里。
pub fn apply_command(preset: &Preset) -> mw_core::Command {
    let mut commands = vec![mw_core::Command::SetTheme {
        theme: preset.theme.clone(),
    }];
    commands.push(mw_core::Command::SetThemeOverrides {
        overrides: preset.overrides.clone(),
    });
    if preset.accent.is_some() {
        commands.push(mw_core::Command::SetThemeAccent {
            accent: preset.accent.clone(),
        });
    }
    mw_core::Command::Batch { commands }
}

/// 预设集合。顺序即界面顺序。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Presets {
    pub items: Vec<Preset>,
}

impl Presets {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 空集合在测试里用来断言「坏 JSON 退回空表」。
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 加一条；重名时自动加序号，绝不让两条预设同名（点了分不清是哪条）。
    pub fn add(&mut self, mut preset: Preset) {
        let base = preset.name.clone();
        let mut n = 2;
        while self.items.iter().any(|p| p.name == preset.name) {
            preset.name = format!("{base} {n}");
            n += 1;
        }
        self.items.push(preset);
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.items.remove(index);
            true
        } else {
            false
        }
    }

    pub fn get(&self, index: usize) -> Option<&Preset> {
        self.items.get(index)
    }

    pub fn load() -> Self {
        match default_path() {
            Some(p) => Self::load_from(&p),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        // BOM 会让 serde_json 直接报错，整份预设静默丢失（v12 的教训）。
        let text = text.trim_start_matches('\u{feff}');
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = default_path() {
            self.save_to(&p);
        }
    }

    pub fn save_to(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    Some(base.join("织思").join("theme-presets.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(name: &str) -> Preset {
        Preset {
            name: name.into(),
            theme: "snow".into(),
            overrides: Some(ThemeOverrides {
                canvas_bg: Some("#fad8df".into()),
                ..Default::default()
            }),
            accent: None,
        }
    }

    #[test]
    fn names_are_trimmed_limited_and_never_duplicated() {
        assert_eq!(Preset::sanitize_name("  品牌蓝  "), Some("品牌蓝".into()));
        assert_eq!(Preset::sanitize_name("   "), None);
        let long = "很".repeat(40);
        assert_eq!(Preset::sanitize_name(&long).unwrap().chars().count(), 24);

        let mut set = Presets::default();
        set.add(preset("品牌"));
        set.add(preset("品牌"));
        set.add(preset("品牌"));
        assert_eq!(set.len(), 3);
        assert_eq!(set.items[1].name, "品牌 2");
        assert_eq!(set.items[2].name, "品牌 3");
    }

    #[test]
    fn a_saved_file_reloads_and_broken_json_falls_back_to_empty() {
        let dir = std::env::temp_dir().join(format!("mw-presets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("theme-presets.json");
        let mut set = Presets::default();
        set.add(preset("品牌"));
        set.save_to(&path);
        assert_eq!(Presets::load_from(&path), set);

        std::fs::write(&path, "not json").unwrap();
        assert!(Presets::load_from(&path).is_empty());

        // BOM 也要能读
        std::fs::write(&path, "\u{feff}{\"items\":[]}").unwrap();
        assert!(Presets::load_from(&path).is_empty());
        let _ = std::fs::remove_file(&path);
    }

    /// 套用预设 = 换主题 + 微调 + 强调色，一条 Batch 一次撤销。
    #[test]
    fn applying_a_preset_is_one_batch_and_undone_in_one_step() {
        let mut doc = mw_core::Document::new("根");
        let before = doc.meta.clone();
        let preset = Preset {
            name: "品牌粉".into(),
            theme: "sand".into(),
            overrides: Some(ThemeOverrides {
                canvas_bg: Some("#fad8df".into()),
                ..Default::default()
            }),
            accent: Some("#5b7db1".into()),
        };
        doc.apply(&apply_command(&preset)).expect("套用预设");
        assert_eq!(doc.meta.theme, "sand");
        assert_eq!(
            doc.meta
                .theme_overrides
                .as_ref()
                .and_then(|o| o.canvas_bg.clone()),
            Some("#fad8df".to_string())
        );
        assert_eq!(doc.meta.theme_accent.as_deref(), Some("#5b7db1"));

        doc.undo().expect("一次撤销");
        assert_eq!(doc.meta.theme, before.theme);
        assert_eq!(doc.meta.theme_overrides, None);
        assert_eq!(doc.meta.theme_accent, None);
    }

    #[test]
    fn remove_reports_whether_it_did_anything() {
        let mut set = Presets::default();
        set.add(preset("甲"));
        assert!(set.remove(0));
        assert!(!set.remove(0));
        assert!(set.is_empty());
    }
}
