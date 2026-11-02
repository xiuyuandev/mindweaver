//! 后台导入：UI 线程只发路径与收结果，读文件/解析/建文档全在工作线程。
//!
//! 为什么必须异步：`.xmind` 可能几十 MB，解析与建树会把 UI 冻住一两秒；这期间
//! 连"取消"按钮都画不出来。这里用一条独立线程 + 原子取消标志，UI 每帧轮询。
//!
//! 不做队列：同一时刻只允许一个导入。多选文件逐个导入的交互等 P24 批量再做。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mw_io::ImportReport;

use crate::session::Session;

/// 线程结果。
pub enum Done {
    Ok {
        /// 盒子只是为了让 `Done` 在通道里保持小尺寸；调用方解引用即可。
        session: Box<Session>,
        report: ImportReport,
    },
    Failed(String),
    Cancelled,
}

/// 在途导入。
pub struct ImportJob {
    rx: Receiver<Done>,
    cancel: Arc<AtomicBool>,
    label: String,
    started: Instant,
}

impl ImportJob {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// 用户点了取消：工作线程会在下一个检查点退出；结果通道即便回来也会被丢弃。
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// 非阻塞收结果。
    pub fn poll(&mut self) -> Option<Done> {
        match self.rx.try_recv() {
            Ok(done) => Some(done),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Done::Failed("导入线程意外退出".into())),
        }
    }

    /// 退出前最多等 `budget`；返回是否已收尾。
    pub fn settle(&mut self, budget: Duration) -> bool {
        let deadline = Instant::now() + budget;
        loop {
            if self.poll().is_some() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// 启动后台导入。线程起不来时返回一个立即失败的 job（不 panic）。
pub fn start(path: PathBuf) -> ImportJob {
    let label = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "导入".into());
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let started = Instant::now();
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("zmind-import".into())
        .spawn(move || {
            if flag.load(Ordering::Relaxed) {
                let _ = tx.send(Done::Cancelled);
                return;
            }
            let out = match Session::import_with_report(&path) {
                Ok((session, report)) if !flag.load(Ordering::Relaxed) => Done::Ok {
                    session: Box::new(session),
                    report,
                },
                Ok(_) => Done::Cancelled,
                Err(e) => Done::Failed(e),
            };
            let _ = tx.send(out);
        });
    if let Err(e) = spawned {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Done::Failed(format!("无法启动导入线程：{e}")));
        return ImportJob {
            rx,
            cancel,
            label,
            started,
        };
    }
    ImportJob {
        rx,
        cancel,
        label,
        started,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn a_markdown_import_finishes_on_the_worker_thread() {
        let dir = std::env::temp_dir().join(format!("mw-import-{}", mw_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("笔记.md");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "# 标题\n\n- 甲\n- 乙\n").unwrap();
        drop(f);

        let mut job = start(path);
        let mut done = None;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(d) = job.poll() {
                done = Some(d);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = std::fs::remove_dir_all(&dir);
        match done.expect("5 秒内应完成") {
            Done::Ok { session, report } => {
                assert_eq!(session.doc.meta.title, "标题");
                assert_eq!(session.doc.node_count(), 3);
                assert!(report.is_empty());
            }
            Done::Failed(e) => panic!("导入失败：{e}"),
            Done::Cancelled => panic!("没有取消却被取消"),
        }
    }

    #[test]
    fn a_missing_file_fails_without_panicking() {
        let mut job = start(PathBuf::from("不存在的文件.md"));
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match job.poll() {
                Some(Done::Failed(_)) => return,
                Some(_) => panic!("不该成功"),
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        panic!("5 秒内应有失败结果");
    }
}
