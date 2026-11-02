//! 导出对话框的**记忆**：上次选的格式 / 范围 / 倍率 / 背景，下次打开直接沿用。
//!
//! 存在 %APPDATA%\织思\export.json。用「后缀字符串」记格式而不是 Debug 名，
//! 这样文件里看到的是 png / svg / md 这些人类可读的值，未知值也能安全回落。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::export::{Format, Scope};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// 格式后缀（png / svg / pdf / md / txt / opml / mm）。
    pub format: String,
    /// 范围（whole / selection / viewport）。
    pub scope: String,
    pub scale: f32,
    pub opaque: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            format: Format::Png.ext().to_string(),
            scope: Scope::Whole.key().to_string(),
            scale: 2.0,
            opaque: true,
        }
    }
}

impl Prefs {
    /// 从界面状态生成记忆。
    pub fn capture(fmt: Format, scope: Scope, scale: f32, opaque: bool) -> Self {
        Self {
            format: fmt.ext().to_string(),
            scope: scope.key().to_string(),
            scale: scale.clamp(0.5, 8.0),
            opaque,
        }
    }

    /// 还原成界面状态；认不出的值一律回落到默认（一份坏文件不该让对话框打不开）。
    pub fn restore(&self) -> (Format, Scope, f32, bool) {
        let fmt = Format::ALL
            .into_iter()
            .find(|f| f.ext() == self.format)
            .unwrap_or(Format::Png);
        let scope = Scope::ALL
            .into_iter()
            .find(|s| s.key() == self.scope)
            .unwrap_or(Scope::Whole);
        (fmt, scope, self.scale.clamp(0.5, 8.0), self.opaque)
    }

    pub fn load() -> Self {
        default_path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        // BOM 会让 serde_json 直接报错（v12 踩过），先剥掉。
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
    Some(base.join("织思").join("export.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_file_and_tolerates_bom_and_junk() {
        let dir = std::env::temp_dir().join(format!("mw-export-prefs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("export.json");
        let p = Prefs::capture(Format::Markdown, Scope::Selection, 3.0, false);
        p.save_to(&path);
        assert_eq!(Prefs::load_from(&path), p);
        assert_eq!(p.restore().0, Format::Markdown);

        std::fs::write(
            &path,
            "\u{feff}{\"format\":\"md\",\"scope\":\"viewport\",\"scale\":4.0,\"opaque\":true}",
        )
        .unwrap();
        let bom = Prefs::load_from(&path);
        assert_eq!(
            bom.restore(),
            (Format::Markdown, Scope::Viewport, 4.0, true)
        );

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Prefs::load_from(&path), Prefs::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_values_fall_back_instead_of_failing() {
        let junk = Prefs {
            format: "tiff".into(),
            scope: "galaxy".into(),
            scale: 99.0,
            opaque: false,
        };
        let (fmt, scope, scale, opaque) = junk.restore();
        assert_eq!(fmt, Format::Png);
        assert_eq!(scope, Scope::Whole);
        assert_eq!(scale, 8.0, "倍率钳到上限");
        assert!(!opaque);
    }

    #[test]
    fn every_format_and_scope_survives_the_round_trip() {
        for fmt in Format::ALL {
            for scope in Scope::ALL {
                let p = Prefs::capture(fmt, scope, 2.0, true);
                assert_eq!(p.restore(), (fmt, scope, 2.0, true));
            }
        }
    }
}
