//! 验收用的键盘焦点轨迹：`MW_TRACE=<文件路径>` 时，把「这一帧谁握着焦点」逐条写进文件。
//!
//! 为什么不让探针看像素：焦点环画得再准，「这一圈环属于第几格」仍要靠坐标反推，
//! 缩放或面板宽度一变就得重算。焦点本身是壳层在绘制那一刻就知道的事，写一行文本
//! 比猜像素便宜得多，也让无头测试能断言 Tab 的落点。
//!
//! 记的是**每一次**焦点变化，不是只有少数几个带名字的控件：`Tab` 走没走进去，只有
//! 看得到整条环才说得清（见 [`observe`]）。带语义名的控件用 [`name`] 登记自己的 id。
//!
//! 这**不是用户功能**，是验收工具，所以和 `MW_STATS` 一样：不设环境变量时零开销，
//! 只有第一条轨迹到来才打开文件。发布版默认没有任何输出。

use std::io::Write;
use std::sync::{Mutex, OnceLock};

use eframe::egui;

struct Sink {
    file: std::fs::File,
    /// 焦点没变时不重复写：一帧重绘一次，写起来会盖掉真正有意义的那几行。
    last: Option<String>,
}

impl Sink {
    /// 记录一次焦点；返回是否真的落盘（用于测试断言语义，不必暴露文件）。
    fn note(&mut self, label: &str) -> bool {
        if self.last.as_deref() == Some(label) {
            return false;
        }
        self.last = Some(label.to_owned());
        let _ = writeln!(self.file, "focus {label}");
        let _ = self.file.flush();
        true
    }
}

static SINK: OnceLock<Mutex<Option<Sink>>> = OnceLock::new();

/// 环境变量只在这里读一次：没设 `MW_TRACE` 时整个进程留一个 `None`，
/// `focus` 退化成一个原子读加一次空锁，绘制路径上感觉不到它。
fn sink() -> &'static Mutex<Option<Sink>> {
    SINK.get_or_init(|| {
        let path = std::env::var_os("MW_TRACE").filter(|v| v != "0");
        let sink = path
            .and_then(|p| std::fs::File::create(p).ok())
            .map(|file| Sink { file, last: None });
        Mutex::new(sink)
    })
}

/// 控件在「本帧拿到焦点」时不用自己操心：把语义名挂在它自己的 id 上，
/// 由 [`observe`] 在帧末统一落盘。名字示例 `accent-chip 3 · 122°`，不要写内部 id。
pub fn name(ctx: &egui::Context, id: egui::Id, label: &str) {
    if !enabled() {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, label.to_owned()));
}

/// 每帧结束时调用一次：谁握着焦点就写一行。没有语义名的控件写 id。
///
/// 为什么要壳层统一写、而不是每个控件自己写一行：焦点环里绝大多数控件（标题栏按钮、
/// 大纲行、面板小标题）根本没有名字可写。探针只看得到少数几个，就分不清「`Tab` 正在
/// 往前走」和「`Tab` 卡在一个吃掉它的输入框里」——踩过的坑：每按十次 `Tab` 就按一次
/// `Esc`，把刚走出的半圈清空，永远摸不到检查器最底下的强调色色板。
pub fn observe(ctx: &egui::Context) {
    if !enabled() {
        return;
    }
    let Some(id) = ctx.memory(|m| m.focused()) else {
        focus("none");
        return;
    };
    let label = ctx.data(|d| {
        d.get_temp::<String>(id)
            .unwrap_or_else(|| format!("id {:x}", id.value()))
    });
    focus(&label);
}

/// 焦点之外的**事件**：`F6` 把键盘交给谁这类判断，像素分不出来（画布不按焦点画东西，
/// 两种情况的画面一模一样）。写成 `event` 前缀而不是 `focus`，探针才不会把一次模式
/// 切换误认成「焦点动了」，也不会覆盖掉它正要读的那一行焦点状态。
pub fn event(label: &str) {
    let Ok(mut guard) = sink().lock() else { return };
    if let Some(sink) = guard.as_mut() {
        let _ = writeln!(sink.file, "event {label}");
        let _ = sink.file.flush();
    }
}

/// 没设 `MW_TRACE` 时是 `false`，`name`/`observe` 退化成一个原子读加一次空锁。
pub(crate) fn enabled() -> bool {
    sink().lock().map(|g| g.is_some()).unwrap_or(false)
}

/// 卡片页帧耗时探针：`MW_CARD_PERF=<文件>` 时这一帧画卡片页花了多少毫秒。
///
/// 与 `MW_TRACE` 分开：性能探针要连拍 200 多帧、每帧一行，混进焦点轨迹会把证据文件撑爆。
/// 第一行就是「打开成本」——读者页第一次 `show` 要排 20k 字符的字。
pub fn card_frame_ms(ms: f32) {
    use std::io::Write as _;
    static FILE: OnceLock<Option<std::sync::Mutex<std::fs::File>>> = OnceLock::new();
    let f = FILE.get_or_init(|| {
        std::env::var_os("MW_CARD_PERF")
            .and_then(|p| std::fs::File::create(p).ok())
            .map(std::sync::Mutex::new)
    });
    if let Some(m) = f {
        if let Ok(mut file) = m.lock() {
            let _ = writeln!(file, "{ms:.3}");
            let _ = file.flush();
        }
    }
}

/// 笔记库帧耗时探针：`MW_LIBRARY_PERF=<文件>` 时这一帧画库页花了多少毫秒。
///
/// 与 `card_frame_ms` 同形：第一行是库页第一次上屏的成本。10k 夹具
/// （`MW_LIBRARY_DEMO=long`）只建内存元数据、不读磁盘，所以这一行**不含冷启动索引**。
pub fn library_frame_ms(ms: f32) {
    use std::io::Write as _;
    static FILE: OnceLock<Option<std::sync::Mutex<std::fs::File>>> = OnceLock::new();
    let f = FILE.get_or_init(|| {
        std::env::var_os("MW_LIBRARY_PERF")
            .and_then(|p| std::fs::File::create(p).ok())
            .map(std::sync::Mutex::new)
    });
    if let Some(m) = f {
        if let Ok(mut file) = m.lock() {
            let _ = writeln!(file, "{ms:.3}");
            let _ = file.flush();
        }
    }
}

/// 轨迹是否开启。给调用方一个机会**先问再构造**语义名：`name` 的入参是 `&str`，
/// 参数会在进入函数体之前算出来，关闭轨迹时那几次 `format!` 就白花了。
pub fn is_enabled() -> bool {
    enabled()
}

/// 控件在「本帧拿到焦点」时调用。`label` 要写成探针能直接对上的语义名，
/// 例如 `accent-chip 3 · 122°`，不要写内部 id。
pub fn focus(label: &str) {
    let Ok(mut guard) = sink().lock() else { return };
    if let Some(sink) = guard.as_mut() {
        sink.note(label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_focus_is_written_once_per_change() {
        let dir = std::env::temp_dir().join("mw-trace-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("focus-{}.log", std::process::id()));
        let mut sink = Sink {
            file: std::fs::File::create(&path).unwrap(),
            last: None,
        };
        assert!(sink.note("accent-chip 0"), "第一次必须落盘");
        assert!(!sink.note("accent-chip 0"), "重绘不能重复写同一格");
        assert!(sink.note("accent-chip 1"), "换了格子要写");
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines, ["focus accent-chip 0", "focus accent-chip 1"]);
        let _ = std::fs::remove_file(&path);
    }
}
