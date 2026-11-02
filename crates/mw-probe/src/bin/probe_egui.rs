//! M0 探针（egui 侧）—— 一个窗口回答 docs/13 §6 M0 的四个待定项：
//!
//! 1. **材质窗口**：Win11 Mica 能否透过 eframe/wgpu 交换链显示（`DWMWA_SYSTEMBACKDROP_TYPE`）
//! 2. **性能**：N 个节点连续平移的帧时间与 CPU 场景构建时间（含/不含文字、含/不含剔除）
//! 3. **IME**：候选窗是否跟随光标（egui-winit 0.37 已接 `set_ime_cursor_area`）
//! 4. **中文排版**：CJK 禁则折行、系统字体回退、当前 DPI 下的清晰度
//!
//! 帧时间看 stdout；材质/IME/禁则三项必须人工在窗口里看，结论记入 ADR-011。
//!
//! 运行：`cargo run -p mw-probe --bin probe_egui --release`
//! 复现测量：`bash scripts/m0-perf.sh`；截图与像素判定：`scripts/m0-capture.ps1`、`scripts/m0-chroma.ps1`。

use std::time::Instant;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

/// 合成节点标签：中文，同时压测字体回退与排版。
const LABELS: [&str; 10] = [
    "项目启动",
    "结果对比与复盘",
    "达成项（含延期）",
    "未达项：预算、人力",
    "经验与教训",
    "下一步行动",
    "用户研究/访谈纪要",
    "数据分析——留存",
    "风险管理",
    "资源分配",
];

/// 禁则语料：刻意让「，。、：；？！」落在可能被断到行首的位置。
const KINSOKU_TEXT: &str = "织思是一款仅面向 Windows 的免费开源思维导图软件，本地优先、零遥测；\
它必须把中文输入、排版与检索当作一等公民来做——候选窗跟随、禁则正确、混排不跳字，\
否则再多的功能也只会让人觉得是个廉价的网页玩具。";

enum MicaState {
    Pending,
    Applied(isize),
    Failed(String),
}

struct Probe {
    nodes: Vec<(Pos2, Vec2, usize)>,
    count: usize,
    scale: f32,
    center: Vec2,
    auto_pan: bool,
    with_text: bool,
    text_scales: bool,
    constant_chrome: bool,
    cull: bool,
    mica_visible: bool,
    ms_frame: f32,
    ms_build: f32,
    frames: u32,
    drawn_hint: u32,
    t: f32,
    frame_start: Instant,
    last_frame: Instant,
    last_report: Instant,
    mica: MicaState,
    ime_buf: String,
    ime_x: f32,
    kinsoku_w: f32,
    ppp: f32,
}

impl Probe {
    fn new(count: usize) -> Self {
        let mut nodes = Vec::with_capacity(count);
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 10_000) as f32 / 10_000.0
        };
        for _ in 0..count {
            let x = (rnd() - 0.5) * 6_000.0;
            let y = (rnd() - 0.5) * 4_000.0;
            let w = 90.0 + rnd() * 90.0;
            let h = 26.0 + rnd() * 14.0;
            let label = (rnd() * 9.999) as usize;
            nodes.push((Pos2::new(x, y), Vec2::new(w, h), label));
        }
        Self {
            nodes,
            count: count.min(5_000),
            scale: 0.5,
            center: Vec2::ZERO,
            auto_pan: true,
            with_text: true,
            text_scales: true,
            constant_chrome: true,
            cull: true,
            mica_visible: true,
            ms_frame: 0.0,
            ms_build: 0.0,
            frames: 0,
            drawn_hint: 0,
            t: 0.0,
            frame_start: Instant::now(),
            last_frame: Instant::now(),
            last_report: Instant::now(),
            mica: MicaState::Pending,
            ime_buf: String::new(),
            ime_x: 0.0,
            kinsoku_w: 240.0,
            ppp: 1.0,
        }
    }

    /// 世界 → 屏幕：铬件宽度用屏幕像素恒定，这是当前 WebView 版缺失的那一条。
    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let t0 = Instant::now();
        let (resp, painter) = ui.allocate_painter(
            ui.available_size(),
            egui::Sense::drag() | egui::Sense::hover(),
        );
        let area = resp.rect;

        if resp.dragged() {
            self.center -= resp.drag_delta() / self.scale;
        }

        let chrome = if self.constant_chrome {
            1.0
        } else {
            self.scale
        };
        let font0 = 13.0 * if self.text_scales { self.scale } else { 1.0 };
        let font0 = font0.max(1.0);
        let mut shapes: Vec<Shape> = Vec::with_capacity(self.count * 2);
        let mut drawn = 0usize;
        for (pos, size, label) in self.nodes.iter().take(self.count) {
            let min = Pos2::new(
                (pos.x - self.center.x) * self.scale + area.center().x,
                (pos.y - self.center.y) * self.scale + area.center().y,
            );
            let rect = Rect::from_min_size(min, *size * self.scale);
            if self.cull && (rect.max.x < area.min.x || rect.min.x > area.max.x) {
                continue;
            }
            drawn += 1;
            shapes.push(Shape::rect_filled(
                rect,
                6.0 * chrome,
                Color32::from_rgb(238, 240, 248),
            ));
            shapes.push(Shape::rect_stroke(
                rect,
                6.0 * chrome,
                Stroke::new(1.5 * chrome, Color32::from_rgb(90, 100, 200)),
                StrokeKind::Middle,
            ));
            if self.with_text {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    LABELS[*label],
                    FontId::proportional(font0),
                    Color32::DARK_GRAY,
                );
            }
        }
        painter.add(Shape::Vec(shapes));
        self.ms_build = t0.elapsed().as_secs_f32() * 1000.0;
        self.drawn_hint = drawn as u32;
    }

    /// 每 60 帧打印一次：`ui` 为本次 `App::ui` 的 CPU 时间，`build` 只含场景构建，`fps` 为墙钟。
    fn report(&mut self) {
        self.ms_frame = self.frame_start.elapsed().as_secs_f32() * 1000.0;
        self.frames += 1;
        if self.frames % 60 == 0 {
            let wall = self.last_report.elapsed().as_secs_f32();
            self.last_report = Instant::now();
            println!(
                "probe_egui n={:>5} drawn={:>5} scale={:.2} text={} text_scales={} cull={} \
                 chrome_const={} ui={:.2}ms build={:.2}ms fps={:.1} ppp={:.2}",
                self.count,
                self.drawn_hint,
                self.scale,
                self.with_text,
                self.text_scales,
                self.cull,
                self.constant_chrome,
                self.ms_frame,
                self.ms_build,
                60.0 / wall.max(1e-4),
                self.ppp
            );
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("节点数");
            ui.add(egui::Slider::new(&mut self.count, 1_000..=20_000).text(""));
            ui.label("缩放");
            ui.add(egui::Slider::new(&mut self.scale, 0.15..=4.0));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.with_text, "含文字");
            ui.checkbox(&mut self.text_scales, "文字随缩放");
            ui.checkbox(&mut self.constant_chrome, "铬件屏幕恒定");
            ui.checkbox(&mut self.cull, "视口剔除");
            ui.checkbox(&mut self.auto_pan, "自动平移");
            ui.checkbox(&mut self.mica_visible, "画布透明");
            ui.separator();
            match &self.mica {
                MicaState::Pending => ui.label("Mica：等待窗口句柄…"),
                MicaState::Applied(h) => {
                    ui.colored_label(Color32::DARK_GREEN, format!("Mica：hwnd={h:X} 已应用"))
                }
                MicaState::Failed(e) => ui.colored_label(Color32::DARK_RED, format!("Mica：{e}")),
            };
        });
    }

    fn chinese_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("中文探针");
        ui.label("① IME：点输入框打「xiangmu」，候选窗应贴在光标处；拖动滑杆，候选窗应随之移动。");
        ui.add(egui::Slider::new(&mut self.ime_x, 0.0..=160.0).text("输入框左偏移"));
        ui.horizontal(|ui| {
            ui.add_space(self.ime_x);
            ui.add(
                egui::TextEdit::singleline(&mut self.ime_buf)
                    .desired_width(140.0)
                    .hint_text("节点文字"),
            );
        });
        ui.separator();
        ui.label(format!(
            "② 禁则：下列中文里「，。、：；？！」不应出现在行首（当前 {}px）。",
            self.kinsoku_w as i32
        ));
        ui.add(egui::Slider::new(&mut self.kinsoku_w, 120.0..=420.0));
        ui.group(|ui| {
            ui.set_max_width(self.kinsoku_w);
            ui.label(egui::RichText::new(KINSOKU_TEXT).size(14.0));
        });
        ui.separator();
        ui.label(
            "③ 字体：system_fonts（fontique → DirectWrite 回退）。若出现豆腐块、或与记事本/资源\
             管理器字形不一致，即为回退失败；再对照放大后的边缘是否发虚（ClearType）。\
             pixels_per_point 见下。",
        );
        ui.label(format!("pixels_per_point = {:.2}", self.ppp));
    }
}

impl eframe::App for Probe {
    /// 透明清除色是 Mica 能否看见的前提：交换链像素为全透明时，DWM 背景才会透出。
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.mica_visible {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            [0.93, 0.94, 0.96, 1.0]
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.ppp = ctx.pixels_per_point();
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.frame_start = now;

        if matches!(self.mica, MicaState::Pending) {
            self.mica = apply_mica(frame);
        }

        egui::Panel::top("ctl").show(ui, |ui| self.controls(ui));
        egui::Panel::left("cn")
            .default_size(320.0)
            .show(ui, |ui| self.chinese_panel(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.draw_canvas(ui));

        if self.auto_pan {
            self.t += dt;
            self.center.x = 2_600.0 * (self.t * 0.55).sin();
            self.center.y = 1_500.0 * (self.t * 0.31).sin();
        }
        ctx.request_repaint();
        self.report();
    }
}

/// eframe 0.37 的 `Frame` 实现了 `HasWindowHandle`，拿真 HWND，不用前台窗口兜底。
fn apply_mica(frame: &mut eframe::Frame) -> MicaState {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    };

    let hwnd = match frame.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(w) => w.hwnd.get() as isize,
            other => return MicaState::Failed(format!("非 Win32 句柄：{other:?}")),
        },
        Err(e) => return MicaState::Failed(format!("取句柄失败：{e}")),
    };

    let value: i32 = DWMSBT_MAINWINDOW.0;
    unsafe {
        match DwmSetWindowAttribute(
            windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void),
            DWMWA_SYSTEMBACKDROP_TYPE,
            &value as *const i32 as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        ) {
            Ok(()) => MicaState::Applied(hwnd),
            Err(e) => MicaState::Failed(format!("DwmSetWindowAttribute：{e}")),
        }
    }
}

/// 命令行参数：让探针可脚本化，便于采一张对照表和一组帧时间，不必人工守着点复选框。
struct Args {
    nodes: usize,
    scale: f32,
    text: bool,
    cull: bool,
    chrome: bool,
    tscale: bool,
    pan: bool,
    opaque: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            nodes: 400,
            scale: 1.0,
            text: true,
            cull: true,
            chrome: true,
            tscale: true,
            pan: false,
            opaque: false,
        }
    }
}

fn parse_args() -> Args {
    let mut a = Args::default();
    for arg in std::env::args().skip(1) {
        let mut kv = arg.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim_start_matches('-');
        let val = kv.next().unwrap_or("1");
        let on = !(val == "0" || val.eq_ignore_ascii_case("false"));
        match key {
            "nodes" => a.nodes = val.parse().unwrap_or(a.nodes).clamp(1, 20_000),
            "scale" => a.scale = val.parse().unwrap_or(a.scale).clamp(0.15, 4.0),
            "text" => a.text = on,
            "cull" => a.cull = on,
            "chrome" => a.chrome = on,
            "tscale" => a.tscale = on,
            "pan" => a.pan = on,
            "opaque" => a.opaque = on,
            "help" => {
                println!(
                    "用法: probe_egui [--nodes=N] [--scale=S] [--text=0|1] [--cull=0|1] \
                          [--chrome=0|1] [--tscale=0|1] [--pan=0|1] [--opaque]"
                );
                std::process::exit(0);
            }
            other => println!("probe_egui: 忽略未知参数 {other}"),
        }
    }
    a
}

fn main() -> eframe::Result {
    let args = parse_args();
    println!(
        "probe_egui cfg nodes={} scale={:.2} text={} text_scales={} cull={} chrome_const={} pan={} clear={}（每 60 帧打印一次）",
        args.nodes,
        args.scale,
        args.text,
        args.tscale,
        args.cull,
        args.chrome,
        args.pan,
        if args.opaque { "opaque" } else { "transparent" }
    );
    let mut probe = Probe::new(args.nodes);
    probe.count = args.nodes;
    probe.scale = args.scale;
    probe.with_text = args.text;
    probe.cull = args.cull;
    probe.constant_chrome = args.chrome;
    probe.text_scales = args.tscale;
    probe.auto_pan = args.pan;
    probe.mica_visible = !args.opaque;

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_280.0, 800.0])
            .with_transparent(true)
            .with_title("织思 M0 探针 · egui 0.37 侧"),
        ..Default::default()
    };
    eframe::run_native(
        "probe_egui",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(probe))
        }),
    )
}
