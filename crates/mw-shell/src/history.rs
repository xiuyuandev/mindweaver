//! 本地版本历史（时间机器）：每次保存把上一版 `.zmind.bak` 收进
//! `<文档目录>/.history/<文件名>/`，每份文档最多保留 `KEEP` 个快照。
//!
//! 不联网、不传云端；恢复前会先把当前版本也收一份，避免“恢复即丢当前”。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const KEEP: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub modified_ms: u128,
    pub bytes: u64,
}

fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn dir_for(doc: &Path) -> Option<PathBuf> {
    let stem = doc.file_stem()?.to_string_lossy();
    Some(doc.parent()?.join(".history").join(stem.as_ref()))
}

fn mtime_ms(path: &Path) -> u128 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 保存成功后调用：把 `.bak`（上一版）收一份；没有 `.bak` 时收当前版本。
pub fn capture(doc: &Path) -> Option<PathBuf> {
    let dir = dir_for(doc)?;
    std::fs::create_dir_all(&dir).ok()?;
    let src = {
        let bak = doc.with_file_name(format!("{}.bak", doc.file_name()?.to_string_lossy()));
        if bak.is_file() {
            bak
        } else {
            doc.to_path_buf()
        }
    };
    if !src.is_file() {
        return None;
    }
    let mut n = 0u32;
    let out = loop {
        let name = format!("snap-{}-{n}.zmind", stamp());
        let p = dir.join(name);
        if !p.exists() {
            break p;
        }
        n += 1;
    };
    std::fs::copy(&src, &out).ok()?;
    prune(&dir, KEEP);
    Some(out)
}

fn prune(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut snaps: Vec<(u128, PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("zmind"))
        .map(|e| (mtime_ms(&e.path()), e.path()))
        .collect();
    snaps.sort_by_key(|(t, _)| *t);
    while snaps.len() > keep {
        let (_, p) = snaps.remove(0);
        let _ = std::fs::remove_file(p);
    }
}

pub fn list(doc: &Path) -> Vec<Snapshot> {
    let Some(dir) = dir_for(doc) else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Snapshot> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("zmind"))
        .filter_map(|p| {
            let meta = std::fs::metadata(&p).ok()?;
            Some(Snapshot {
                modified_ms: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis())
                    .unwrap_or(0),
                bytes: meta.len(),
                path: p,
            })
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.modified_ms));
    out
}

/// 恢复某份快照：先验证它能被解析，再把当前版本收进历史，最后原子替换。
pub fn restore(doc: &Path, snapshot: &Path) -> Result<(), String> {
    mw_format::load(snapshot).map_err(|e| format!("快照无法解析：{e}"))?;
    capture(doc);
    let tmp = doc.with_file_name(format!(
        "{}.restore.tmp",
        doc.file_name().and_then(|s| s.to_str()).unwrap_or("文档")
    ));
    std::fs::copy(snapshot, &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, doc).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("mw-history-{}", mw_core::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn doc_with(text: &str) -> mw_core::Document {
        let mut d = mw_core::Document::new(text);
        d.meta.title = text.into();
        d
    }

    #[test]
    fn saves_are_captured_and_capped_at_ten() {
        let dir = temp();
        let path = dir.join("方案.zmind");
        mw_format::save(&doc_with("v0"), &path).unwrap();
        for i in 0..14 {
            mw_format::save(&doc_with(&format!("v{i}")), &path).unwrap();
            capture(&path);
        }
        let snaps = list(&path);
        assert!(
            snaps.len() <= KEEP,
            "快照应被裁剪到 10 个，实际 {}",
            snaps.len()
        );
        assert!(!snaps.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn restore_brings_back_previous_content() {
        let dir = temp();
        let path = dir.join("方案.zmind");
        mw_format::save(&doc_with("旧版"), &path).unwrap();
        mw_format::save(&doc_with("新版"), &path).unwrap();
        let snap = capture(&path).unwrap();
        let loaded = mw_format::load(&path).unwrap();
        assert_eq!(loaded.doc.meta.title, "新版");
        restore(&path, &snap).unwrap();
        let back = mw_format::load(&path).unwrap();
        assert_eq!(back.doc.meta.title, "旧版");
        let _ = std::fs::remove_dir_all(dir);
    }
}
