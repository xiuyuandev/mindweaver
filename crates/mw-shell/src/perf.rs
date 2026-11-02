//! 启动里程碑探针（docs/08 P1/P3）。
//!
//! 只有 `MW_PERF_LOG=<path>` 时才建文件，每一行是 `名字<TAB>相对毫秒`。
//! 默认路径完全不执行 IO——发布版不会被这个模块拖慢，也不会在用户机器上留文件。

use std::io::Write;
use std::time::Instant;

pub struct Perf {
    start: Instant,
    log: Option<std::fs::File>,
}

impl Perf {
    pub fn from_env() -> Self {
        let log = std::env::var_os("MW_PERF_LOG").and_then(|path| std::fs::File::create(path).ok());
        let mut perf = Self {
            start: Instant::now(),
            log,
        };
        perf.mark("process_start");
        perf
    }

    /// 只有 `MW_PERF_LOG` 打开时才写；写失败静默（探针不能反过来影响被测应用）。
    pub fn mark(&mut self, name: &str) {
        let Some(file) = self.log.as_mut() else {
            return;
        };
        let ms = self.start.elapsed().as_secs_f64() * 1000.0;
        let _ = writeln!(file, "{name}\t{ms:.1}");
        let _ = file.flush();
    }
}
