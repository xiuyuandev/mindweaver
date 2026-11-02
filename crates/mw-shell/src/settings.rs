//! 全局设置：外观 / 编辑 / 自动保存 / 导出默认值 / 库（vault）。
//!
//! 存在 `%APPDATA%\织思\settings.json`。设置是**壳层**的，不写进文档：
//! 同一份文档换机器打开，不该被别人的“减少动画”或库路径影响。
//!
//! 库（`docs/34` §7）：Obsidian 式文件夹，先选才能用、可多库切换；卡片/文档都住库。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 一个库（vault）注册项：用户选定的一个文件夹，是文档（`.xmind`/`.cxl`/`.md`）的家。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VaultEntry {
    pub id: String,
    /// 显示名（默认取文件夹名）。
    pub name: String,
    /// 库根目录（绝对路径）。
    pub path: String,
    pub created_at: i64,
}

impl Default for VaultEntry {
    fn default() -> Self {
        Self {
            id: mw_core::new_id(),
            name: String::new(),
            path: String::new(),
            created_at: mw_core::now_ms(),
        }
    }
}

// 设置 UI（`docs/34` §11）接线前只有单测在用。
#[cfg_attr(not(test), allow(dead_code))]
impl VaultEntry {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    /// 覆盖系统“减少动态效果”检测。
    pub reduce_motion: bool,
    /// 自动保存间隔（秒）。1–30。
    pub autosave_secs: f64,
    /// 撤销栈上限。100–2000，默认 500（`docs/02` §2 P0）。
    pub undo_limit: usize,
    /// 笔记库根目录；空表示跟随当前库（`docs/34` §7.3）。
    pub notes_dir: String,
    /// 导出默认倍率与背景（和上次导出记忆是同一组值）。
    pub export_scale: f32,
    pub export_opaque: bool,
    /// 全局 UI 主题 id（`snow`/`ink`/`mist`/`sand`，`mw_core::theme::THEME_IDS`），
    /// 所有界面统一（`docs/34` §8）。
    pub theme: String,
    /// 卡片笔记编辑界面字号（12–32，默认 16）。
    pub card_font_size: u16,
    /// 库注册表（多库）。
    pub vaults: Vec<VaultEntry>,
    /// 当前库 id；空 = 未选 → 启动门（`docs/34` §7）。
    pub active_vault_id: String,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            reduce_motion: false,
            autosave_secs: crate::autosave::INTERVAL,
            undo_limit: mw_core::HISTORY_LIMIT,
            notes_dir: String::new(),
            export_scale: 2.0,
            export_opaque: true,
            theme: "snow".to_string(),
            card_font_size: 16,
            vaults: Vec::new(),
            active_vault_id: String::new(),
        }
    }
}

fn default_path() -> Option<PathBuf> {
    crate::paths::appdata_root().map(|d| d.join("settings.json"))
}

impl Store {
    pub fn load() -> Self {
        default_path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = default_path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// 笔记库根目录（`docs/34` §8.1）：显式 `notes_dir` 非空 > 活动库根 > 默认目录。
    /// 显式路径是用户覆盖，优先级最高；库内卡片 `.md` 住库根；老配置行为不变。
    pub fn notes_root(&self) -> Option<PathBuf> {
        let s = self.notes_dir.trim();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
        self.vault_root().or_else(crate::paths::notes_dir)
    }

    pub fn autosave_interval(&self) -> f64 {
        self.autosave_secs.clamp(1.0, 30.0)
    }

    /// 撤销栈上限的生效值：老配置文件缺 `undoLimit` 时回落到默认 500，
    /// 手改坏了（0 / 99999）也夹到合法区间。
    pub fn undo_limit(&self) -> usize {
        self.undo_limit
            .clamp(mw_core::MIN_HISTORY_LIMIT, mw_core::MAX_HISTORY_LIMIT)
    }
}

// 设置 UI（`docs/34` §11）接线前只有单测在用：卡片字号 + 全部库（vault）API。
#[cfg_attr(not(test), allow(dead_code))]
impl Store {
    /// 卡片编辑字号的生效值（12–32）。
    pub fn card_font_size(&self) -> u16 {
        self.card_font_size.clamp(12, 32)
    }

    // -- 库（vault，docs/34 §7） ------------------------------------------------

    /// 当前库注册项。
    pub fn active_vault(&self) -> Option<&VaultEntry> {
        self.vaults.iter().find(|v| v.id == self.active_vault_id)
    }

    /// 当前库根目录。
    pub fn vault_root(&self) -> Option<PathBuf> {
        self.active_vault().map(|v| PathBuf::from(&v.path))
    }

    /// 需要用户先选库（未选 / 当前库路径已不存在）→ 启动门。
    pub fn needs_vault(&self) -> bool {
        match self.active_vault() {
            Some(v) => !Path::new(&v.path).is_dir(),
            None => true,
        }
    }

    /// 新增一个库并设为当前库；返回库 id。`name` 空则取文件夹名。
    pub fn add_vault(&mut self, name: &str, path: &str) -> String {
        let name = if name.trim().is_empty() {
            Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("库")
                .to_string()
        } else {
            name.trim().to_string()
        };
        let entry = VaultEntry::new(name, path);
        let id = entry.id.clone();
        self.vaults.push(entry);
        self.active_vault_id = id.clone();
        id
    }

    /// 切换当前库。id 不存在则不动。
    pub fn set_active_vault(&mut self, id: &str) {
        if self.vaults.iter().any(|v| v.id == id) {
            self.active_vault_id = id.to_string();
        }
    }

    /// 移除一个库；若移除的是当前库，则清空当前（触发启动门）。
    pub fn remove_vault(&mut self, id: &str) {
        self.vaults.retain(|v| v.id != id);
        if self.active_vault_id == id {
            self.active_vault_id.clear();
        }
    }
}

/// 调低撤销栈上限**真正会丢掉**的步数（M28b）。
///
/// - 抬高或持平（`new >= old`）→ 0；
/// - 当前撤销 + 重做深度还没顶到新上限（`depth <= new`）→ 0（裁剪不会碰到任何一条）；
/// - 否则 `depth - new`。
///
/// 只用来决定设置中心那条警告行是否出现、写几步；不参与 `Document` 的裁剪语义。
pub(crate) fn undo_limit_dropped(old: usize, new: usize, depth: usize) -> usize {
    if new >= old {
        return 0;
    }
    depth.saturating_sub(new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试落地目录：**仓库内 target/tmp 优先**（同 `mw-format::tests::scratch`）——
    /// 本机用户 Temp 对测试进程偶发拒写（os error 5），会让写盘用例假失败。
    fn scratch(prefix: &str) -> PathBuf {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/tests");
        let base = if std::fs::create_dir_all(&base).is_ok() {
            base
        } else {
            std::env::temp_dir()
        };
        let dir = base.join(format!("{}-{}", prefix, mw_core::new_id()));
        std::fs::create_dir_all(&dir).expect("测试临时目录必须能创建");
        dir
    }

    /// M28b：只有「真的丢记录」才算数——抬高/持平、深度没顶到新上限都不提醒。
    #[test]
    fn undo_limit_dropped_counts_only_real_losses() {
        assert_eq!(super::undo_limit_dropped(500, 500, 900), 0, "持平不丢");
        assert_eq!(super::undo_limit_dropped(500, 1200, 900), 0, "抬高不丢");
        assert_eq!(
            super::undo_limit_dropped(500, 100, 17),
            0,
            "深度 17 没顶到 100"
        );
        assert_eq!(super::undo_limit_dropped(500, 100, 110), 10);
        assert_eq!(super::undo_limit_dropped(500, 100, 100), 0, "正好等于上限");
        assert_eq!(super::undo_limit_dropped(500, 100, 0), 0, "空历史");
    }

    #[test]
    fn settings_roundtrip_and_clamp() {
        let dir = scratch("mw-settings");
        let path = dir.join("settings.json");
        let s = Store {
            reduce_motion: true,
            autosave_secs: 99.0,
            undo_limit: 1200,
            notes_dir: "D:/笔记".into(),
            export_scale: 3.0,
            export_opaque: false,
            theme: "inkNight".into(),
            card_font_size: 20,
            vaults: vec![VaultEntry::new("主库", "D:/库")],
            active_vault_id: String::new(),
        };
        // 直接写临时文件，避免依赖 APPDATA。
        if let Ok(json) = serde_json::to_string_pretty(&s) {
            assert!(json.contains("\"undoLimit\": 1200"), "{json}");
            assert!(json.contains("\"cardFontSize\": 20"), "{json}");
            std::fs::write(&path, json).unwrap();
        }
        let back = Store::load_from(&path);
        assert_eq!(back, s);
        assert_eq!(back.autosave_interval(), 30.0);
        assert_eq!(back.undo_limit(), 1200);
        assert_eq!(back.notes_root(), Some(PathBuf::from("D:/笔记")));
        assert_eq!(back.theme, "inkNight");
        assert_eq!(back.card_font_size(), 20);
        assert_eq!(back.vaults.len(), 1);
        assert_eq!(back.vaults[0].name, "主库");
        // active_vault_id 空 → needs_vault（启动门）。
        assert!(back.needs_vault());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vault_add_select_remove() {
        let mut s = Store::default();
        assert!(s.needs_vault(), "无库 → 要选");
        let id = s.add_vault("", "D:/知识库");
        assert_eq!(s.active_vault_id, id);
        assert_eq!(
            s.active_vault().unwrap().name,
            "知识库",
            "name 空取文件夹名"
        );
        // 指到不存在的目录仍算需要选（路径得真实存在）。
        assert!(s.needs_vault(), "路径不存在 → 仍要选");
        // 用临时真实目录当库。
        let real = scratch("mw-vault");
        let id2 = s.add_vault("真库", &real.to_string_lossy());
        assert!(!s.needs_vault(), "真实目录 → 不用再选");
        assert_eq!(s.active_vault_id, id2);
        assert_eq!(s.vault_root().unwrap(), real, "vault_root 指向当前库");
        // §8.1 优先级：显式 notes_dir > 活动库根 > 默认。未显式设置时跟库根。
        assert_eq!(
            s.notes_root().unwrap(),
            real,
            "无显式 notes_dir 时笔记库住库根"
        );
        s.notes_dir = "D:/自定义".into();
        assert_eq!(
            s.notes_root().unwrap(),
            PathBuf::from("D:/自定义"),
            "显式 notes_dir 优先于库根"
        );
        s.notes_dir.clear();
        // 切回旧库（不存在）→ 又要选。
        s.set_active_vault(&id);
        assert!(s.needs_vault());
        assert_eq!(s.vault_root().unwrap(), PathBuf::from("D:/知识库"));
        // 移除当前库 → 清空当前。
        s.remove_vault(&id);
        assert!(s.active_vault_id.is_empty());
        assert_eq!(s.vaults.len(), 1);
        let _ = std::fs::remove_dir_all(&real);
    }

    #[test]
    fn undo_limit_missing_uses_default_and_clamps() {
        let dir = scratch("mw-settings");
        let path = dir.join("settings.json");
        // 老配置文件没有 undoLimit：缺键即老行为（默认 500）。
        std::fs::write(&path, "{\"reduceMotion\": true}").unwrap();
        let back = Store::load_from(&path);
        assert_eq!(back.undo_limit(), mw_core::HISTORY_LIMIT);
        assert_eq!(back.undo_limit(), 500);
        let _ = std::fs::remove_dir_all(&dir);

        let s = Store {
            undo_limit: 0,
            ..Store::default()
        };
        assert_eq!(s.undo_limit(), mw_core::MIN_HISTORY_LIMIT);
        let s = Store {
            undo_limit: 99_999,
            ..Store::default()
        };
        assert_eq!(s.undo_limit(), mw_core::MAX_HISTORY_LIMIT);
    }
}
