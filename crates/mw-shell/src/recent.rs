//! 最近文件列表。
//!
//! 存在 `%APPDATA%\织思\recent.json`——**不写进文档目录**：用户的资料夹不该被塞进
//! 一个只对本机有意义的列表文件。
//!
//! 只记路径 / 标题 / 时间，不缓存内容。列表本身坏了不算错误（当作空列表重来），
//! 但列表里的项指向的文件可能已经被删了，打开时按"文件不存在"正常报错。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub title: String,
    /// 最近一次打开/保存的时间（毫秒）。
    pub at: i64,
    /// 文件还在不在盘上（`docs/30` §3.4：**缺的那一行留着并标灰**，不删）。
    ///
    /// 只进内存：`recent.json` 的结构与语义不变（`docs/31` §4.1），所以序列化时跳过。
    #[serde(skip)]
    pub missing: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    entries: Vec<Entry>,
}

pub struct Recent {
    store: Store,
    file: Option<PathBuf>,
}

impl Recent {
    /// 读默认位置。读不到/解析失败都退化成空列表——**不能因为一个缓存文件坏了就起不来**。
    pub fn load() -> Self {
        match default_path() {
            Some(p) => Self::load_from(&p),
            None => Self {
                store: Store::default(),
                file: None,
            },
        }
    }

    /// 还没读完时占位用的空列表。`file` 仍指向默认位置——这期间若先发生一次 `touch`，
    /// 也写得回去，不会因为「还没读完」把这一笔丢掉。
    pub fn pending() -> Self {
        Self {
            store: Store::default(),
            file: default_path(),
        }
    }

    /// 异步读默认位置：起始页首帧就能画骨架，不必等磁盘（`docs/30` §3.4 的 3 行加载骨架）。
    ///
    /// 线程只读一次、读的是同一份 `recent.json`。调用方拿到结果整份替换即可：
    /// 列表就绪前起始页的最近文件区是骨架，用户点不到任何一条，不存在「谁覆盖谁」。
    pub fn load_async() -> std::sync::mpsc::Receiver<Self> {
        match default_path() {
            Some(p) => Self::load_async_from(p),
            None => {
                let (tx, rx) = std::sync::mpsc::channel();
                let _ = tx.send(Self::pending());
                rx
            }
        }
    }

    /// 同 [`Recent::load_async`]，但读指定文件（测试用；真实路径由 [`default_path`] 决定）。
    pub fn load_async_from(file: PathBuf) -> std::sync::mpsc::Receiver<Self> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(Self::load_from(&file));
        });
        rx
    }

    pub fn load_from(file: &Path) -> Self {
        let mut store = std::fs::read_to_string(file)
            .ok()
            .and_then(|s| serde_json::from_str::<Store>(&s).ok())
            .unwrap_or_default();
        // 老版本直接存调用方给的路径，同一份文件可能同时留下相对和绝对两条；
        // 读盘时顺手归一化并去重，下次写回就干净了。
        let mut clean: Vec<Entry> = Vec::with_capacity(store.entries.len());
        for mut entry in store.entries.drain(..) {
            entry.path = absolute(&entry.path);
            if !clean.iter().any(|e| same_file(&e.path, &entry.path)) {
                clean.push(entry);
            }
        }
        store.entries = clean;
        Self {
            store,
            file: Some(file.to_path_buf()),
        }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.store.entries
    }

    /// 这份路径是否还挂在最近列表里（打开失败弹窗据此决定要不要给「移除」）。
    pub fn contains(&self, path: &Path) -> bool {
        let path = absolute(path);
        self.store.entries.iter().any(|e| same_file(&e.path, &path))
    }

    /// 记一笔：同一个文件只留一条，永远排最前。
    pub fn touch(&mut self, path: &Path, title: &str, at: i64) {
        let entry = Entry {
            path: absolute(path),
            title: title.to_string(),
            at,
            missing: false,
        };
        self.store
            .entries
            .retain(|e| !same_file(&e.path, &entry.path));
        self.store.entries.insert(0, entry);
        self.store.entries.truncate(MAX_ENTRIES);
        self.save();
    }

    /// 文件已经不在了：从列表里摘掉（用户点它只会得到一句"打开失败"）。
    pub fn forget(&mut self, path: &Path) {
        let path = absolute(path);
        let before = self.store.entries.len();
        self.store.entries.retain(|e| !same_file(&e.path, &path));
        if self.store.entries.len() != before {
            self.save();
        }
    }

    /// 标出「文件已经不在盘上」的那几行——**不摘掉**。
    ///
    /// 以前这里是 `retain(|e| e.path.exists())`：文档被移动/改名之后，列表里少了一条，
    /// 用户看到的是「它不见了」，而不是「它去哪儿了」。`docs/30` §3.4 要的是后者：
    /// 行留着、标题转 secondary、副行写「文件已移动」、点它走打开失败态（弹窗里还能
    /// 「从最近列表移除」，那才是真正要删的时候）。
    ///
    /// 只在载入/触摸时算一次：每帧 stat 八条磁盘没必要，也没人愿意为它卡一帧。
    pub fn mark_missing(&mut self) {
        for e in &mut self.store.entries {
            e.missing = !e.path.exists();
        }
    }

    /// 清空列表：只动 `recent.json`，不碰任何文档。
    pub fn clear(&mut self) {
        if self.store.entries.is_empty() {
            return;
        }
        self.store.entries.clear();
        self.save();
    }

    /// 验收/截图用的固定种子：三条相对时间、通用示例路径，**只进内存不写盘**。
    pub fn seed_demo(&mut self, now_ms: i64) {
        self.store.entries = vec![
            Entry {
                path: PathBuf::from("C:/示例/工作/产品路线图.zmind"),
                title: "产品路线图".into(),
                at: now_ms - 120_000,
                missing: false,
            },
            Entry {
                path: PathBuf::from("C:/示例/学习/读书笔记.zmind"),
                title: "读书笔记".into(),
                at: now_ms - 3 * 3_600_000,
                // 虚构路径：demo 里一律当「在盘上」，否则九张起始页基线会一起变红。
                missing: false,
            },
            Entry {
                path: PathBuf::from("C:/示例/生活/旅行计划.zmind"),
                title: "旅行计划".into(),
                at: now_ms - 2 * 86_400_000,
                missing: false,
            },
        ];
    }

    /// 验收钩子：把种子里第 n 条标成「文件已移动」（`MW_RECENTS_MISSING=<n>`，0 起）。
    pub fn mark_demo_missing(&mut self, index: usize) {
        if let Some(e) = self.store.entries.get_mut(index) {
            e.missing = true;
        }
    }

    fn save(&self) {
        let Some(file) = self.file.as_ref() else {
            return;
        };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.store) {
            // 写失败只是丢掉列表，不该影响"保存文档"这件事的成功与否。
            let _ = std::fs::write(file, json);
        }
    }
}

/// 统一成绝对路径：同一份文件可能一会儿以相对路径、一会儿以绝对路径进来。
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Windows 路径大小写不敏感；能 canonicalize 就按真实文件身份比较。
fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || match (a.canonicalize(), b.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
}

fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    Some(base.join("织思").join("recent.json"))
}

#[cfg(test)]
mod tests {
    use super::Recent;
    use std::path::PathBuf;

    /// 每个用例一个独占目录（同 autosave：并行测试不能用毫秒当名字）。
    fn temp_file() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mw-recent-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("recent.json")
    }

    #[test]
    fn touching_puts_the_most_recent_first_and_deduplicates() {
        let file = temp_file();
        let mut r = Recent::load_from(&file);
        r.touch(&PathBuf::from("C:/a.zmind"), "a.zmind", 1);
        r.touch(&PathBuf::from("C:/b.zmind"), "b.zmind", 2);
        r.touch(&PathBuf::from("C:/a.zmind"), "a.zmind", 3);
        let paths: Vec<_> = r.entries().iter().map(|e| e.path.clone()).collect();
        assert_eq!(
            paths,
            [PathBuf::from("C:/a.zmind"), PathBuf::from("C:/b.zmind")]
        );
        assert_eq!(r.entries()[0].at, 3, "时间要更新到最新一次");

        // 重新读盘还能读回来（列表是持久的）
        let again = Recent::load_from(&file);
        assert_eq!(again.entries().len(), 2);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn the_list_is_capped() {
        let file = temp_file();
        let mut r = Recent::load_from(&file);
        for i in 0..12 {
            r.touch(&PathBuf::from(format!("C:/{i}.zmind")), "x", i);
        }
        assert_eq!(r.entries().len(), super::MAX_ENTRIES);
        assert_eq!(r.entries()[0].path, PathBuf::from("C:/11.zmind"));
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn a_corrupt_list_degrades_to_empty_instead_of_panicking() {
        let file = temp_file();
        std::fs::write(&file, "{ this is not json").unwrap();
        let r = Recent::load_from(&file);
        assert!(r.entries().is_empty());
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    /// `docs/30` §3.4：缺文件的那一行**留着并标灰**。删掉它等于把「它去哪儿了」也删了。
    #[test]
    fn missing_files_stay_in_the_list_and_get_marked() {
        let file = temp_file();
        let mut r = Recent::load_from(&file);
        let gone = file.parent().unwrap().join("gone.zmind");
        let here = file.parent().unwrap().join("here.zmind");
        std::fs::write(&here, b"x").unwrap();
        r.touch(&gone, "gone.zmind", 2);
        r.touch(&here, "here.zmind", 1);
        assert_eq!(r.entries().len(), 2);
        r.mark_missing();
        assert_eq!(r.entries().len(), 2, "缺文件的条目不能消失");
        let by = |name: &str| {
            r.entries()
                .iter()
                .find(|e| e.title == name)
                .expect("条目还在")
                .missing
        };
        assert!(by("gone.zmind"), "不在盘上的要标出来");
        assert!(!by("here.zmind"), "在盘上的不能标");
        // 标记只进内存：落盘内容里没有这个字段，重开仍是两条、且重新算过。
        let again = Recent::load_from(&file);
        assert_eq!(again.entries().len(), 2);
        assert!(
            again.entries().iter().all(|e| !e.missing),
            "载入时先当正常的，mark_missing 才判断"
        );
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn clear_empties_the_list_and_persists_it() {
        let file = temp_file();
        let mut r = Recent::load_from(&file);
        r.touch(&PathBuf::from("C:/a.zmind"), "a.zmind", 1);
        r.clear();
        assert!(r.entries().is_empty());
        assert!(
            Recent::load_from(&file).entries().is_empty(),
            "清空要落盘，重开不能复活"
        );
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn demo_seed_is_in_memory_only() {
        let file = temp_file();
        let mut r = Recent::load_from(&file);
        let now = 1_000_000_000_000i64;
        r.seed_demo(now);
        assert_eq!(r.entries().len(), 3);
        assert_eq!(r.entries()[0].title, "产品路线图");
        assert_eq!(r.entries()[0].at, now - 120_000);
        assert_eq!(r.entries()[1].at, now - 3 * 3_600_000);
        assert_eq!(r.entries()[2].at, now - 2 * 86_400_000);
        assert!(!file.exists(), "演示种子不能写盘，否则会污染真实最近文件");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn contains_matches_the_same_file_in_either_form() {
        let file = temp_file();
        let mut r = Recent::load_from(&file);
        let rel = PathBuf::from("target").join("mw-contains.zmind");
        let abs = std::env::current_dir().unwrap().join(&rel);
        r.touch(&abs, "mw-contains.zmind", 1);
        assert!(r.contains(&rel), "相对路径要归一到同一条目");
        assert!(r.contains(&abs));
        assert!(!r.contains(&PathBuf::from("C:/nope.zmind")));
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn relative_and_absolute_paths_to_the_same_file_collapse() {
        let file = temp_file();
        let rel = PathBuf::from("target").join("mw-dup.zmind");
        let abs = std::env::current_dir().unwrap().join(&rel);
        let json = serde_json::json!({
            "entries": [
                { "path": rel, "title": "相对路径", "at": 2 },
                { "path": abs, "title": "绝对路径", "at": 1 },
            ]
        });
        std::fs::write(&file, json.to_string()).unwrap();

        let mut r = Recent::load_from(&file);
        assert_eq!(r.entries().len(), 1, "读盘时就要合并同一条目");
        assert_eq!(r.entries()[0].title, "相对路径", "保留靠前的一条");

        r.touch(&rel, "再次打开", 3);
        assert_eq!(r.entries().len(), 1, "运行中再记也不能长出第二条");
        assert_eq!(r.entries()[0].title, "再次打开");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    /// SE-02：起始页首帧用的是「还没读完」的空列表——它必须是**空**的（这样才画骨架），
    /// 但 `file` 得指着默认位置，这期间真要 `touch` 一笔也写得回去。
    #[test]
    fn the_pending_list_is_empty_but_still_knows_where_to_write() {
        let r = Recent::pending();
        assert!(
            r.entries().is_empty(),
            "占位列表必须是空的，不然会画成「没有最近文件」"
        );
    }

    /// SE-02：异步读取在真正读完之后，给出的就是同步读会给出的那一份。
    #[test]
    fn async_load_hands_back_what_the_sync_one_would() {
        let file = temp_file();
        let mut seed = Recent::load_from(&file);
        seed.touch(&PathBuf::from("C:/a.zmind"), "甲.zmind", 7);
        seed.touch(&PathBuf::from("C:/b.zmind"), "乙.zmind", 9);

        let loaded = Recent::load_async_from(file.clone())
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("后台线程五秒内必须把列表送回来");
        let expected = Recent::load_from(&file);
        let got: Vec<_> = loaded
            .entries()
            .iter()
            .map(|e| (e.path.clone(), e.at))
            .collect();
        let want: Vec<_> = expected
            .entries()
            .iter()
            .map(|e| (e.path.clone(), e.at))
            .collect();
        assert_eq!(got, want, "异步与同步读到的必须是同一份（顺序也一样）");
        assert_eq!(got.len(), 2);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }
}
