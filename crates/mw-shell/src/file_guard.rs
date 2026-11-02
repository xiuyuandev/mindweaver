//! 文件占用与外部变更：同目录 `<文件>.lock` 原子创建表示「本进程正在编辑」。
//!
//! 不是安全锁，是**协作锁**：第二个实例看到锁就只读，避免两边的自动保存互相覆盖。
//! 锁随 `Guard` drop 删除；进程崩溃留下的锁超过 12 小时视为陈旧，可被接管。
//! 外部变更用「长度 + 修改时间」指纹判断，够快也够稳（不需要读整份文件）。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub len: u64,
    pub modified_ms: u128,
}

pub fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let meta = std::fs::metadata(path).ok()?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Some(Fingerprint {
        len: meta.len(),
        modified_ms,
    })
}

/// 写盘前问一句：磁盘上的这份东西在我们打开/上次写盘之后变过吗？
///
/// 从没见过（`known = None`）不算冲突，文件这时读不到（`now = None`）也不算——
/// 那是「刚建还没落盘」，硬判冲突会让新建卡片第一次保存就弹提示条。
pub fn changed_since(known: Option<Fingerprint>, now: Option<Fingerprint>) -> bool {
    match (known, now) {
        (Some(k), Some(n)) => k != n,
        _ => false,
    }
}

fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

/// 持有一把协作锁。`None` 表示别的实例已经持有。
pub struct Guard {
    lock: PathBuf,
}

impl Guard {
    pub fn try_acquire(path: &Path) -> Option<Self> {
        let lock = lock_path(path);
        // 陈旧锁（>12h）先清掉：崩溃后不该永远只读。
        if let Ok(meta) = std::fs::metadata(&lock) {
            if meta
                .modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > Duration::from_secs(12 * 3600))
            {
                let _ = std::fs::remove_file(&lock);
            }
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
        {
            Ok(mut f) => {
                use std::io::Write;
                let _ = writeln!(f, "pid={} at={:?}", std::process::id(), SystemTime::now());
                Some(Self { lock })
            }
            Err(_) => None,
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_file_is_a_conflict_but_a_missing_one_is_not() {
        let a = Fingerprint {
            len: 10,
            modified_ms: 1,
        };
        assert!(!changed_since(Some(a), Some(a)), "一模一样不算冲突");
        assert!(changed_since(
            Some(a),
            Some(Fingerprint {
                len: 11,
                modified_ms: 1
            })
        ));
        assert!(changed_since(
            Some(a),
            Some(Fingerprint {
                len: 10,
                modified_ms: 2
            })
        ));
        // 没见过 / 现在读不到：都不是「被别人改过」，别吓用户。
        assert!(!changed_since(None, Some(a)));
        assert!(!changed_since(Some(a), None));
        assert!(!changed_since(None, None));
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-lock-{}", mw_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn second_instance_cannot_take_the_same_lock() {
        let file = temp("方案.zmind");
        std::fs::write(&file, b"x").unwrap();
        let a = Guard::try_acquire(&file).expect("第一个实例应拿到锁");
        assert!(Guard::try_acquire(&file).is_none(), "第二个实例应只读");
        drop(a);
        assert!(Guard::try_acquire(&file).is_some(), "释放后可以再拿");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn fingerprint_changes_when_the_file_changes() {
        let file = temp("方案.zmind");
        std::fs::write(&file, b"a").unwrap();
        let a = fingerprint(&file).unwrap();
        std::fs::write(&file, b"abcd").unwrap();
        let b = fingerprint(&file).unwrap();
        assert_ne!(a, b);
        assert!(b.len > a.len);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }
}
