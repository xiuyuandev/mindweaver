//! M0 验收 harness：离屏渲染 + golden image 快照比对（docs/13 §7；§8 风险表
//! 「失去 WebView 后自动化验收变难」的对策，也是 ADR-010 之后取代 CDP 冒烟的基建）。
//!
//! 像素来自 `egui::Context::request_screenshot` → `egui-wgpu::CaptureState`：帧先渲染到一张
//! 自有纹理，再 copy 进 CPU 可读缓冲。因此**隐藏窗口也拿得到像素**——合成器不给它交换链纹理
//! 也无妨（`egui-wgpu/src/capture.rs` 顶注）。于是快照与壁纸、窗口遮挡、任务栏位置无关，
//! Mica 区域回读成 alpha=0 的原值，同样与桌面无关。这就是「离屏」的由来。
//!
//! 基线文件名带 `ppp`：`pixels_per_point` 由显示器缩放决定，跨缩放档次的像素不可能相同，
//! 150% 与 200% 各存各的基线（docs/13 §4.3 残留风险里的「200% 补拍」因此只要换缩放重跑一遍）。
//!
//! 逐像素比对、PNG 读写与差异热力图在 `mw_golden`——M1b 起画布纹理要用同一套口径。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, ColorImage};
use mw_golden::{read_png, write_png, Drift};

/// 退出码：一致 / 漂移 / harness 自身出错。
const OK: u8 = 0;
const DRIFT: u8 = 1;
const ERROR: u8 = 2;

/// 命令行分派：交互式壳层 / 快照 harness / 用法。
pub enum Invocation {
    Interactive,
    Golden(Options),
    Help,
}

pub struct Options {
    /// 基线目录；实际文件是 `dir/shell-ppp{scale}.png`。
    pub dir: PathBuf,
    pub update: bool,
    /// 单通道容差。默认 0：驱动或 wgpu 版本变动本该被看见，而不是被容差吃掉。
    pub tol: u8,
    /// 截之前空跑的帧数，留给 DWM 材质与字体回退落地。
    pub warmup: u32,
    /// 拿到回读像素的等待上限。
    pub timeout: Duration,
    /// 关掉离屏、真开一个窗口，供人眼核对 harness 本身。
    pub shown: bool,
    /// 强制 `pixels_per_point`（多 DPI 基线）；`None` 跟随系统。
    pub ppp: Option<f32>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("shots/golden"),
            update: false,
            tol: 0,
            warmup: 3,
            timeout: Duration::from_secs(10),
            shown: false,
            ppp: None,
        }
    }
}

pub fn invocation() -> Invocation {
    let mut opts = Options::default();
    let mut golden = false;
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--golden" => golden = true,
            "--update" => opts.update = true,
            "--shown" => opts.shown = true,
            "--dir" => opts.dir = PathBuf::from(required(&mut argv, "--dir")),
            "--tol" => opts.tol = parse(&mut argv, "--tol"),
            "--warmup" => opts.warmup = parse(&mut argv, "--warmup"),
            "--timeout" => opts.timeout = Duration::from_millis(parse(&mut argv, "--timeout")),
            "--ppp" => opts.ppp = Some(parse(&mut argv, "--ppp")),
            "-h" | "--help" => return Invocation::Help,
            // 文件关联双击：路径参数交给 Shell 打开，不算未知参数。
            other if other.to_ascii_lowercase().ends_with(".zmind") => {}
            other => bad_arg(format!("未知参数：{other}")),
        }
    }

    if golden {
        Invocation::Golden(opts)
    } else {
        Invocation::Interactive
    }
}

fn required<I: Iterator<Item = String>>(argv: &mut I, flag: &str) -> String {
    match argv.next() {
        Some(v) => v,
        None => bad_arg(format!("{flag} 缺少取值")),
    }
}

fn parse<T: std::str::FromStr, I: Iterator<Item = String>>(argv: &mut I, flag: &str) -> T {
    let raw = required(argv, flag);
    match raw.parse() {
        Ok(v) => v,
        Err(_) => bad_arg(format!("{flag} 取值无法解析：{raw}")),
    }
}

fn bad_arg(msg: String) -> ! {
    eprintln!("[golden] {msg}\n\n{}", usage());
    std::process::exit(ERROR as i32)
}

pub fn usage() -> &'static str {
    "\
织思 M0 壳层

  mw-shell                        交互式窗口
  mw-shell --golden               离屏渲染一帧，与基线逐像素比对
  mw-shell --golden --update      用当前渲染结果覆写基线
  mw-shell --golden --tol N       单通道容差（默认 0）
  mw-shell --golden --shown       真开窗口渲染，供人眼核对 harness

退出码：0 一致 / 1 漂移 / 2 harness 自身出错。
基线目录默认 shots/golden，文件名含显示器缩放（如 shell-ppp1.50.png）。
"
}

/// 每帧驱动一次：空跑 → 请求截图 → 等回读 → 比对 → 给出退出码。
pub struct Harness {
    opts: Options,
    slot: Arc<Mutex<Option<Arc<ColorImage>>>>,
    requested: bool,
    frames: u32,
    started: Instant,
}

impl Harness {
    pub fn new(opts: Options) -> Self {
        println!(
            "[golden] 离屏渲染 · 基线目录 {} · 容差 {} · warmup {} 帧 · 超时 {:?}",
            opts.dir.display(),
            opts.tol,
            opts.warmup,
            opts.timeout
        );
        Self {
            opts,
            slot: Arc::new(Mutex::new(None)),
            requested: false,
            frames: 0,
            started: Instant::now(),
        }
    }

    /// 输入脚本需要更多帧：把它算进 warmup，截图落在脚本结束之后。
    pub fn extend_warmup(&mut self, frames: u32) {
        self.opts.warmup = self.opts.warmup.max(frames);
    }

    /// `--shown` 是给人眼核对 harness 的，此时保留真实指针；离屏才清 hover。
    pub fn offscreen(&self) -> bool {
        !self.opts.shown
    }

    /// 把 `--ppp` 落到上下文。必须在每帧 UI 之前调用：输入脚本运行时截图状态机
    /// 不推进，若等 `tick` 才设缩放，脚本里的按键会按旧视口取景。
    pub fn prepare(&self, ctx: &egui::Context) {
        if let Some(ppp) = self.opts.ppp {
            if (ctx.pixels_per_point() - ppp).abs() > 1e-3 {
                ctx.set_pixels_per_point(ppp);
                ctx.request_repaint();
            }
        }
        // egui 的 Area 淡入按真实时间算，离屏帧跑得快慢会让遮罩/弹窗差 1 个色阶。
        // 金样只截终态：把时间型动画归零，弹窗首帧就是全不透明。
        // 文本光标同理：闪烁相位跟真实时间走，同一份脚本会随机停在「亮/灭」两态，
        // 编辑态基线双跑就 0 不了；截图里固定成常亮。
        let style = ctx.global_style();
        if style.animation_time > 0.0 || style.visuals.text_cursor.blink {
            ctx.all_styles_mut(|s| {
                s.animation_time = 0.0;
                s.visuals.text_cursor.blink = false;
            });
        }
    }

    /// 返回 `Some(code)` 表示本轮结束，主循环应当退出。
    pub fn tick(&mut self, ctx: &egui::Context) -> Option<u8> {
        self.frames += 1;
        if !self.requested {
            if self.frames > self.opts.warmup {
                self.requested = true;
                self.started = Instant::now();
                let slot = Arc::clone(&self.slot);
                ctx.request_screenshot(move |img| {
                    if let Ok(mut guard) = slot.lock() {
                        *guard = Some(img);
                    }
                });
            }
            ctx.request_repaint();
            return None;
        }

        match self.slot.lock().ok().and_then(|mut g| g.take()) {
            Some(img) => Some(self.finish(ctx, &img)),
            None => {
                if self.started.elapsed() > self.opts.timeout {
                    println!(
                        "[golden] ✗ 超时 {:?} 没拿到回读像素——窗口没被绘制？试试 --shown 观察",
                        self.opts.timeout
                    );
                    return Some(ERROR);
                }
                ctx.request_repaint();
                None
            }
        }
    }

    fn finish(&self, ctx: &egui::Context, img: &ColorImage) -> u8 {
        let [w, h] = img.size;
        if w == 0 || h == 0 {
            println!("[golden] ✗ 回读尺寸为 {w}×{h}，窗口未成型");
            return ERROR;
        }
        let ppp = ctx.pixels_per_point();
        let name = format!("shell-ppp{ppp:.2}.png");
        let path = self.opts.dir.join(&name);
        let actual: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        println!("[golden] 回读 {w}×{h} @ ppp {ppp:.2} → {}", path.display());
        crate::trace::event(&format!(
            "golden_finish frames={} now={:.3}",
            self.frames,
            ctx.input(|i| i.time)
        ));

        if self.opts.update {
            return match write_png(&path, &actual, w as u32, h as u32) {
                Ok(()) => {
                    println!("[golden] ✓ 基线已写入");
                    OK
                }
                Err(e) => {
                    println!("[golden] ✗ 写基线失败：{e}");
                    ERROR
                }
            };
        }

        let golden = match read_png(&path) {
            Ok(g) => g,
            Err(e) => {
                println!("[golden] ✗ {e}（首次请用 --golden --update 生成基线）");
                return ERROR;
            }
        };
        if golden.1 as usize != w || golden.2 as usize != h {
            println!(
                "[golden] ✗ 尺寸漂移：基线 {}×{}，当前 {w}×{h}（缩放变了？基线名里带 ppp）",
                golden.1, golden.2
            );
            return DRIFT;
        }

        let drift = Drift::measure(&actual, &golden.0, w as u32, h as u32, self.opts.tol);
        if drift.bad == 0 {
            println!("[golden] ✓ 无超容差差异（{w}×{h}，容差 {}）", self.opts.tol);
            return OK;
        }

        println!("[golden] ✗ {drift}");
        let diff = drift.heatmap(&actual, &golden.0);
        for (sub, bytes) in [("actual", &actual), ("diff", &diff)] {
            let p = self.opts.dir.join(sub).join(&name);
            match write_png(&p, bytes, w as u32, h as u32) {
                Ok(()) => println!("         {sub} → {}", p.display()),
                Err(e) => println!("         {sub} 落盘失败：{e}"),
            }
        }
        DRIFT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 金样必须与真实时间无关：入场动画归零 + 文本光标停止闪烁。
    /// 缺了后一条，同一份脚本会随机停在闪烁的亮/灭两态（CER-12 实测 126 px 飘移）。
    #[test]
    fn prepare_freezes_time_dependent_visuals() {
        let ctx = egui::Context::default();
        let harness = Harness::new(Options {
            dir: PathBuf::from("target/golden-test"),
            ..Default::default()
        });
        harness.prepare(&ctx);
        let style = ctx.global_style();
        assert_eq!(style.animation_time, 0.0, "入场动画必须归零");
        assert!(
            !style.visuals.text_cursor.blink,
            "光标闪烁必须停掉，否则编辑态基线双跑不稳定"
        );
    }
}
