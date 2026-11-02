//! 自动保存与崩溃恢复点。
//!
//! 四条纪律：
//! 1. **不动正本**。恢复点写在 `<文档目录>/.autosave/` 下，只有用户确认"恢复"之后
//!    按 Ctrl+S 才写回正本；
//! 2. **比正本新才算数**。启动时拿 mtime 比一比，不比正本新的恢复点直接忽略——
//!    否则一次正常退出之后又弹"要恢复吗"，比丢数据更烦人；
//! 3. **写失败不打断用户，但也不许瞒着**。磁盘满/目录没权限时不弹模态框、不打断编辑，
//!    可是失败本身要说出来：第一次失败给一条提示条，然后按退避阶梯（15s→30s→
//!    60s→120s→240s，封顶 300s，最多 5 次）重试，`revision` 一变就重置次数；
//!    保存文档这件事本身不该被恢复点拖累，但"以为有崩溃保护、其实没有"更糟；
//! 4. **序列化不在 UI 线程**。20k 文档一次落盘 50ms 起，每 3 秒卡一帧是能感觉到的。
//!    UI 线程只克隆一份 [`Snapshot`]（微秒级到几毫秒），压缩、写盘交给常驻工作线程；
//!    同一时刻最多一个在途写入，后续修订合并成"最新一份"，不会排出一条长队。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use mw_core::{DocMeta, Document, Node};

use crate::session::Session;

/// 自动保存间隔（秒）。手速再快也不会 3 秒丢一个小时的活。
pub const INTERVAL: f64 = 3.0;

/// 未命名文档（没落过盘）的恢复点放在这儿。
fn appdata_dir() -> Option<PathBuf> {
    crate::paths::appdata_root().map(|d| d.join("autosave"))
}

/// 这份会话的恢复点路径。
pub fn slot_for(session: &Session) -> Option<PathBuf> {
    match session.path.as_ref() {
        Some(doc) => {
            let dir = doc.parent()?.join(".autosave");
            let stem = doc.file_stem().and_then(|s| s.to_str()).unwrap_or("文档");
            Some(dir.join(format!("{stem}.autosave.zmind")))
        }
        None => appdata_dir().map(|d| d.join(format!("未命名-{}.autosave.zmind", session.doc.id))),
    }
}

/// 同步写一份恢复点（测试与后台线程降级路径的参照实现）。
/// 正常自动保存走 [`Background`]，UI 线程不该调这个。
#[cfg(test)]
pub fn write(session: &Session) -> Option<PathBuf> {
    let slot = slot_for(session)?;
    if let Some(dir) = slot.parent() {
        std::fs::create_dir_all(dir).ok()?;
    }
    mw_format::save(&session.doc, &slot).ok()?;
    Some(slot)
}

/// 正本被正常保存之后，恢复点就没有意义了——删掉，免得下次启动又问一遍。
/// 运行期走 [`Background::invalidate`]（还要挡住迟到的在途写）。
#[cfg(test)]
pub fn clear(session: &Session) {
    if let Some(slot) = slot_for(session) {
        let _ = std::fs::remove_file(slot);
    }
}

/// 启动时用：这份文档有没有**比正本更新**的恢复点。
pub fn pending_for(doc: &Path) -> Option<PathBuf> {
    let dir = doc.parent()?.join(".autosave");
    let stem = doc.file_stem().and_then(|s| s.to_str())?;
    let slot = dir.join(format!("{stem}.autosave.zmind"));
    if newer_than(&slot, doc) {
        Some(slot)
    } else {
        None
    }
}

/// `candidate` 存在且比 `reference` 新。`reference` 不存在时也算"新"（正本被删了）。
fn newer_than(candidate: &Path, reference: &Path) -> bool {
    let Ok(cand) = std::fs::metadata(candidate).and_then(|m| m.modified()) else {
        return false;
    };
    match std::fs::metadata(reference).and_then(|m| m.modified()) {
        Ok(orig) => cand > orig,
        Err(_) => true,
    }
}

/// 一个可恢复的恢复点（恢复中心列表项）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryPoint {
    pub slot: PathBuf,
    /// 对应的正本路径；未命名文档为 `None`。
    pub doc: Option<PathBuf>,
    pub title: String,
    pub modified_ms: u128,
    pub bytes: u64,
}

/// 扫描若干根目录下的 `.autosave/*.autosave.zmind`（去重、按修改时间倒序）。
pub fn scan(roots: &[PathBuf]) -> Vec<RecoveryPoint> {
    let mut out: Vec<RecoveryPoint> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        let dirs = [root.join(".autosave"), root.clone()];
        for dir in dirs {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let slot = e.path();
                if slot.extension().and_then(|s| s.to_str()) != Some("zmind")
                    || !slot
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n.ends_with(".autosave.zmind"))
                {
                    continue;
                }
                if !seen.insert(slot.clone()) {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                let modified_ms = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let stem = slot
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("恢复点")
                    .trim_end_matches(".autosave.zmind")
                    .to_string();
                let title = mw_format::peek_manifest(&slot)
                    .map(|m| m.title)
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| stem.clone());
                // 有正本的恢复点：`<目录>/.autosave/<stem>.autosave.zmind` → `<目录>/<stem>.zmind`
                let doc = if stem.starts_with("未命名") {
                    None
                } else {
                    slot.parent().and_then(|p| p.parent()).map(|parent| {
                        parent.join(format!(
                            "{}.zmind",
                            slot.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("文档")
                                .trim_end_matches(".autosave")
                        ))
                    })
                };
                out.push(RecoveryPoint {
                    slot,
                    doc,
                    title,
                    modified_ms,
                    bytes: meta.len(),
                });
            }
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.modified_ms));
    out
}

/// 保留策略：每个正本最多 `keep` 份（未命名文档按标题各算一份）。
pub fn prune(points: &[RecoveryPoint], keep: usize) -> Vec<RecoveryPoint> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for p in points {
        let key = p
            .doc
            .as_ref()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| p.title.clone());
        let n = counts.entry(key).or_insert(0);
        if *n < keep {
            out.push(p.clone());
            *n += 1;
        }
    }
    out
}

///
/// 克隆发生在 UI 线程，所以这里**刻意不带**撤销栈、重做栈和检索缓存——
/// 它们对恢复点没有意义，却可能比节点表还大。
pub struct Snapshot {
    doc_id: String,
    meta: DocMeta,
    root_id: String,
    nodes: HashMap<String, Node>,
    cards: Vec<mw_core::NoteCard>,
    revision: u64,
}

impl Snapshot {
    pub fn of(session: &Session) -> Self {
        let doc = &session.doc;
        Self {
            doc_id: doc.id.clone(),
            meta: doc.meta.clone(),
            root_id: doc.root_id.clone(),
            nodes: doc.nodes.clone(),
            cards: doc.cards.clone(),
            revision: doc.revision,
        }
    }
}

struct Job {
    epoch: u64,
    slot: PathBuf,
    snapshot: Snapshot,
}

struct Done {
    epoch: u64,
    slot: PathBuf,
    revision: u64,
    /// 失败原因（`Ok` = 写成功）。SR-02 问题 6：失败要能说清楚为什么。
    result: Result<(), String>,
}

struct InFlight {
    epoch: u64,
    slot: PathBuf,
}

/// 一次 [`Background::poll`] 可能收回多条完成结果（多个标签并行排队）。
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Wrote {
        slot: PathBuf,
        revision: u64,
    },
    /// 写恢复点失败。带上 `revision`（哪一版没写进去）与 `reason`（为什么）：
    /// 以前这里只有槽位，于是"失败"只能被静默地记成 `rev=0`，用户永远不知道
    /// 这一刻其实没有崩溃保护（SR-02 问题 6）。
    Failed {
        slot: PathBuf,
        revision: u64,
        reason: String,
    },
}

/// 自动保存的后台写线程。
///
/// 用法：脏了就 `submit`，每帧 `poll` 收结果；正本保存后 `invalidate`，
/// 切文档时 `bump_epoch` 让旧文档的完成结果不再算到新文档头上。
pub struct Background {
    tx: Sender<Job>,
    rx: Receiver<Done>,
    in_flight: Option<InFlight>,
    /// 等待写盘的队列：**每个槽位只留最新一份**，不同标签各自排队。
    /// 以前只有一个 `Option<Job>`，多标签时后提交的会把先提交的挤掉。
    pending: Vec<Job>,
    /// 正本已保存、却可能被在途任务重新创建的槽位。
    stale: Vec<PathBuf>,
    /// 文档代际：只有当前代的结果才更新"已自动保存修订号"。
    epoch: u64,
    /// 工作线程起不来时的降级结果（同步写）：(槽位, 修订号, 结果)。
    degraded: Option<(PathBuf, u64, Result<(), String>)>,
}

impl Default for Background {
    fn default() -> Self {
        Self::new()
    }
}

impl Background {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::channel::<Job>();
        let (done_tx, rx) = mpsc::channel::<Done>();
        // 线程起不来也不致命：submit 收到 send 错误时会降级成同步写。
        let _ = std::thread::Builder::new()
            .name("zmind-autosave".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let revision = job.snapshot.revision;
                    let result = write_snapshot(&job.slot, job.snapshot);
                    let done = Done {
                        epoch: job.epoch,
                        slot: job.slot,
                        revision,
                        result,
                    };
                    let _ = done_tx.send(done);
                }
            });
        Self {
            tx,
            rx,
            in_flight: None,
            pending: Vec::new(),
            stale: Vec::new(),
            epoch: 0,
            degraded: None,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.in_flight.is_some() || !self.pending.is_empty() || self.degraded.is_some()
    }

    /// 这个槽位还有没有未收尾的写（在途或排队）。
    ///
    /// 壳层用它把「已提交」这张账对齐现实：被 `invalidate` 或 `stale` 吃掉的提交
    /// 永远不会有 [`Outcome`] 回来，不抹掉就会让那个修订号再也不重投（SR-02 问题 7）。
    pub fn is_pending(&self, slot: &Path) -> bool {
        // 只算真会回来结果的写：`stale`（正本刚存过）的那一份本来就不该再等，
        // 把它也算进"在途"只会让壳层的账多挂一轮。
        self.in_flight.as_ref().is_some_and(|f| f.slot == slot)
            || self.pending.iter().any(|j| j.slot == slot)
    }

    /// 提交一份快照。返回是否已受理（`false` = 算不出槽位/槽位刚被作废）。
    ///
    /// 同一槽位在队列里只保留最新一版；不同槽位各排各的，谁也不挤掉谁。
    pub fn submit(&mut self, session: &Session) -> bool {
        let Some(slot) = slot_for(session) else {
            return false;
        };
        // 刚被正本保存作废、且旧写还在途：这一版先不排，等旧写收尾再重来。
        if self.stale.contains(&slot) {
            return false;
        }
        let job = Job {
            epoch: self.epoch,
            slot,
            snapshot: Snapshot::of(session),
        };
        if let Some(pos) = self.pending.iter().position(|j| j.slot == job.slot) {
            self.pending[pos] = job;
        } else {
            self.pending.push(job);
        }
        self.kick();
        true
    }

    /// 正本已保存：删掉恢复点，并保证在途写完成之后不会再把它"复活"。
    pub fn invalidate(&mut self, session: &Session) {
        let Some(slot) = slot_for(session) else {
            return;
        };
        self.pending.retain(|j| j.slot != slot);
        if self.in_flight.as_ref().is_some_and(|f| f.slot == slot) && !self.stale.contains(&slot) {
            self.stale.push(slot.clone());
        }
        let _ = std::fs::remove_file(slot);
    }

    /// 收回已完成的后台写。一次可能有多条（多个标签的槽位并行收尾）。
    pub fn poll(&mut self) -> Vec<Outcome> {
        let mut out = Vec::new();
        // 上一轮可能刚把在途写收完、队列里还有别的标签：先续上。
        self.kick();
        if let Some((slot, rev, result)) = self.degraded.take() {
            out.push(match result {
                Ok(()) => Outcome::Wrote {
                    slot,
                    revision: rev,
                },
                Err(reason) => Outcome::Failed {
                    slot,
                    revision: rev,
                    reason,
                },
            });
        }
        loop {
            let done = match self.rx.try_recv() {
                Ok(done) => done,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.in_flight = None;
                    break;
                }
            };
            if let Some(pos) = self.stale.iter().position(|s| s == &done.slot) {
                let _ = std::fs::remove_file(&done.slot);
                self.stale.swap_remove(pos);
            }
            if self
                .in_flight
                .as_ref()
                .is_some_and(|f| f.epoch == done.epoch && f.slot == done.slot)
            {
                self.in_flight = None;
            }
            if done.epoch == self.epoch {
                match done.result {
                    Ok(()) => out.push(Outcome::Wrote {
                        slot: done.slot.clone(),
                        revision: done.revision,
                    }),
                    Err(reason) => out.push(Outcome::Failed {
                        slot: done.slot.clone(),
                        revision: done.revision,
                        reason,
                    }),
                }
            }
            self.kick();
        }
        out
    }

    /// 退出前把在途写等完（最多 `timeout`）。分离线程不会拖住进程，
    /// 但最后一份恢复点值得等这几十毫秒。
    pub fn settle(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            let _ = self.poll();
            if !self.is_busy() || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        for slot in self.stale.drain(..) {
            let _ = std::fs::remove_file(slot);
        }
    }

    /// 在途为空时从队列取下一份。
    fn kick(&mut self) {
        if self.in_flight.is_none() && !self.pending.is_empty() {
            let job = self.pending.remove(0);
            self.start(job);
        }
    }

    fn start(&mut self, job: Job) {
        self.in_flight = Some(InFlight {
            epoch: job.epoch,
            slot: job.slot.clone(),
        });
        let slot = job.slot.clone();
        if let Err(err) = self.tx.send(job) {
            // 工作线程没起来：同步写一份，别把恢复点弄丢。
            let job = err.0;
            let revision = job.snapshot.revision;
            let result = write_snapshot(&job.slot, job.snapshot);
            self.in_flight = None;
            self.degraded = Some((slot, revision, result));
        }
    }
}

/// 工作线程侧：把快照还原成 `Document` 再走格式层（`from_parts` 顺带校验结构）。
///
/// 返回失败原因而不是 `bool`：壳层要把它原样说给用户听（SR-02 问题 6）。
/// 格式层的错误语义不能改，所以只在这里转一层可读的说明。
fn write_snapshot(slot: &Path, snapshot: Snapshot) -> Result<(), String> {
    let doc = Document::from_parts_with_cards(
        Some(snapshot.doc_id),
        snapshot.meta,
        snapshot.root_id,
        snapshot.nodes,
        snapshot.cards,
    )
    .map_err(|e| format!("恢复点结构不一致：{e}"))?;
    mw_format::save(&doc, slot).map_err(|e| format!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::{clear, pending_for, slot_for, write, Background, InFlight};
    use crate::session::Session;
    use mw_core::Command;
    use std::path::PathBuf;

    /// 每个用例一个独占目录：并行跑测试时用毫秒当名字会撞车，撞了就是"拒绝访问"。
    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mw-autosave-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scan_lists_three_points_and_retention_keeps_two() {
        let dir = temp_dir();
        for i in 0..3 {
            let mut doc = mw_core::Document::new(&format!("文档 {i}"));
            doc.meta.title = format!("文档 {i}");
            let s = Session::from_doc(doc, Some(dir.join(format!("doc{i}.zmind"))));
            write(&s).expect("写恢复点");
        }
        let points = super::scan(std::slice::from_ref(&dir));
        assert_eq!(points.len(), 3, "三份文档三个恢复点");
        let kept = super::prune(&points, 2);
        assert_eq!(kept.len(), 3, "三个不同正本各保留 2 份，三个都留下");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_untitled_session_autosaves_into_a_private_slot() {
        let s = Session::blank();
        let slot = slot_for(&s).expect("未命名也有槽位");
        assert!(slot.to_string_lossy().contains("未命名-"), "{slot:?}");
        assert!(slot.to_string_lossy().ends_with(".autosave.zmind"));
        assert!(!s.dirty(), "写恢复点不该让会话变脏");
    }

    #[test]
    fn the_slot_lives_next_to_the_document_not_on_top_of_it() {
        let dir = temp_dir();
        let doc = dir.join("方案.zmind");
        let mut s = Session::sample();
        s.save_to(&doc).unwrap();
        let slot = slot_for(&s).unwrap();
        assert_eq!(slot, dir.join(".autosave").join("方案.autosave.zmind"));
        assert_ne!(slot, doc, "绝不能覆盖正本");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_pending_tracks_queued_and_in_flight_writes() {
        let dir = temp_dir();
        let doc = dir.join("方案.zmind");
        let mut s = Session::sample();
        s.save_to(&doc).unwrap();
        let slot = slot_for(&s).unwrap();
        let mut bg = Background::new();
        assert!(!bg.is_pending(&slot), "还没提交就不算在途");
        assert!(bg.submit(&s));
        assert!(bg.is_pending(&slot), "提交完要么在队列、要么在途");

        // 结果回来后就不该再算在途：壳层的「已提交」账靠这个对齐现实（SR-02 问题 7）。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while bg.is_pending(&slot) && std::time::Instant::now() < deadline {
            let _ = bg.poll();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!bg.is_pending(&slot), "写完之后不该再算在途");
        assert!(slot.exists(), "恢复点应当真的写出来了");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_written_recovery_point_is_newer_and_then_cleared_on_save() {
        let dir = temp_dir();
        let doc = dir.join("方案.zmind");
        let mut s = Session::sample();
        s.save_to(&doc).unwrap();
        // 让正本的时间确定地更早
        std::thread::sleep(std::time::Duration::from_millis(20));
        let slot = write(&s).expect("写恢复点");
        assert!(slot.exists());
        assert_eq!(
            pending_for(&doc),
            Some(slot.clone()),
            "恢复点比正本新，要提示"
        );

        // 正常保存一次 → 恢复点应当被清掉，下次启动不再问
        s.save_to(&doc).unwrap();
        clear(&s);
        assert_eq!(pending_for(&doc), None, "正本已经更新，恢复点作废");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_recovery_point_is_ignored() {
        let dir = temp_dir();
        let doc = dir.join("方案.zmind");
        let mut s = Session::sample();
        s.save_to(&doc).unwrap();
        let slot = write(&s).unwrap();
        // 正本再写一次（比恢复点新）
        std::thread::sleep(std::time::Duration::from_millis(20));
        s.save_to(&doc).unwrap();
        assert_eq!(
            pending_for(&doc),
            None,
            "不比正本新的恢复点不该弹提示——那比丢数据更烦"
        );
        assert!(slot.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_background_write_lands_with_the_latest_text() {
        let dir = temp_dir();
        let doc = dir.join("方案.zmind");
        let mut s = Session::sample();
        s.save_to(&doc).unwrap();
        let root = s.doc.root_id.clone();
        s.doc
            .apply(&Command::SetText {
                id: root,
                text: "后台写盘".into(),
            })
            .unwrap();

        let mut bg = Background::new();
        assert!(bg.submit(&s), "有路径的会话必须受理");
        bg.settle(std::time::Duration::from_secs(5));
        assert!(!bg.is_busy(), "settle 之后不该还有在途任务");
        let slot = slot_for(&s).unwrap();
        assert!(slot.exists(), "恢复点要真的落盘");
        let loaded = mw_format::load(&slot).unwrap();
        assert_eq!(
            loaded.doc.node(&loaded.doc.root_id).unwrap().text,
            "后台写盘"
        );
        assert!(s.dirty(), "后台写恢复点不该改会话的脏状态");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_save_invalidates_an_in_flight_recovery_point() {
        let dir = temp_dir();
        let doc = dir.join("方案.zmind");
        let mut s = Session::sample();
        s.save_to(&doc).unwrap();
        let root = s.doc.root_id.clone();
        s.doc
            .apply(&Command::SetText {
                id: root,
                text: "还没写完".into(),
            })
            .unwrap();

        let mut bg = Background::new();
        assert!(bg.submit(&s));
        // 模拟用户立刻按 Ctrl+S：槽位作废，但在途写停不下来
        bg.invalidate(&s);
        bg.settle(std::time::Duration::from_secs(5));
        assert_eq!(
            pending_for(&doc),
            None,
            "在途写不能在正本保存之后复活恢复点"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_busy_worker_keeps_only_the_newest_snapshot() {
        let mut s = Session::sample();
        let root = s.doc.root_id.clone();
        s.doc
            .apply(&Command::SetText {
                id: root.clone(),
                text: "第一版".into(),
            })
            .unwrap();
        let mut bg = Background::new();
        // 假装有一份在途写（不真的喂给工作线程），后续提交只能排队。
        bg.in_flight = Some(InFlight {
            epoch: bg.epoch,
            slot: PathBuf::from("busy"),
        });
        assert!(bg.submit(&s));
        s.doc
            .apply(&Command::SetText {
                id: root.clone(),
                text: "第二版".into(),
            })
            .unwrap();
        assert!(bg.submit(&s));

        let pending = bg.pending.first().expect("应有一份排队快照");
        assert_eq!(pending.snapshot.revision, s.doc.revision);
        assert_eq!(
            pending.snapshot.nodes.get(&root).unwrap().text,
            "第二版",
            "排队时旧修订应被最新一份替换"
        );
    }

    /// 多标签的核心：不同槽位各排各的，同一槽位只留最新一版。
    #[test]
    fn two_documents_queue_independently_and_latest_wins_per_slot() {
        let dir = temp_dir();
        let mut a = Session::sample();
        a.save_to(&dir.join("甲.zmind")).unwrap();
        let mut b = Session::sample();
        b.save_to(&dir.join("乙.zmind")).unwrap();
        let ra = a.doc.root_id.clone();
        let rb = b.doc.root_id.clone();
        {
            let doc = &mut a.doc;
            doc.apply(&Command::SetText {
                id: ra.clone(),
                text: "甲一".into(),
            })
            .unwrap();
        }
        b.doc
            .apply(&Command::SetText {
                id: rb,
                text: "乙一".into(),
            })
            .unwrap();

        let mut bg = Background::new();
        // 假装有一份在途写，后续只能排队。
        bg.in_flight = Some(InFlight {
            epoch: bg.epoch,
            slot: PathBuf::from("busy"),
        });
        assert!(bg.submit(&a));
        assert!(bg.submit(&b));
        assert_eq!(bg.pending.len(), 2, "两个不同槽位都要排");
        a.doc
            .apply(&Command::SetText {
                id: ra,
                text: "甲二".into(),
            })
            .unwrap();
        assert!(bg.submit(&a));
        assert_eq!(bg.pending.len(), 2, "同一槽位再次提交只替换");
        let sa = slot_for(&a).unwrap();
        assert_eq!(
            bg.pending
                .iter()
                .find(|j| j.slot == sa)
                .unwrap()
                .snapshot
                .nodes
                .values()
                .find(|n| n.parent_id.is_none())
                .unwrap()
                .text,
            "甲二",
            "同一槽位要留最新修订"
        );

        // 放掉假在途，两台都要真的落盘。
        bg.in_flight = None;
        bg.settle(std::time::Duration::from_secs(5));
        assert!(!bg.is_busy(), "settle 之后队列应清空");
        let la = mw_format::load(&slot_for(&a).unwrap()).unwrap();
        let lb = mw_format::load(&slot_for(&b).unwrap()).unwrap();
        assert_eq!(
            la.doc.node(&la.doc.root_id).unwrap().text,
            "甲二",
            "甲槽位要写最新修订"
        );
        assert_eq!(lb.doc.node(&lb.doc.root_id).unwrap().text, "乙一");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
