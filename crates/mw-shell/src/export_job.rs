//! PNG/PDF 后台导出（P11）：UI 线程只拍 `Scene`/`Theme` 快照，排版、GPU 渲染、
//! PNG/PDF 编码与落盘全在工作线程。
//!
//! 为什么不能在画布那台合成器上做：`Compositor::render` 会按导出尺寸重建内部目标纹理，
//! 而画布的纹理正被 egui 采样。这里给工作线程一台**一次性合成器**，并让它在工作线程
//! 自建 `TextEngine`（DirectWrite 的 COM 对象与 `Rc` 缓存都不是 `Send`）。
//!
//! 单飞：同一时刻只允许一个导出任务。任务写入同目录的 `.part` 文件，成功后再原子改名，
//! 进程在导出中途退出不会留下半张 PNG 冒充成品。

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::export;
use eframe::egui_wgpu;
use mw_core::{Scene, Theme};
use mw_render::{build_layers, Camera, Compositor, Frame, TextEngine};

/// 位图导出的两种落盘格式，共用同一次离屏渲染。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Png,
    Pdf,
}

/// 一次导出任务的完整输入。构造方在 UI 线程克隆场景与主题；之后所有字段都在工作线程消费。
pub struct Request {
    pub kind: Kind,
    pub path: PathBuf,
    pub scene: Scene,
    pub theme: Theme,
    pub scale: f32,
    pub opaque: bool,
    /// 取景框覆盖：`Some` 时按这个世界矩形出图（「可视区域」导出），
    /// `None` 时用整份场景的内容包围盒。
    pub bounds: Option<mw_core::Rect>,
}

/// 成功结果。`elapsed` 只算工作线程内的排版/渲染/编码/写盘，不含线程启动与 UI 轮询。
#[derive(Clone, Debug)]
pub struct Report {
    pub eff: f32,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    /// PDF 页数；位图导出恒为 1。
    pub pages: usize,
    pub elapsed: Duration,
}

/// 在途任务。UI 每帧 [`Running::poll`]，完成前不要重复提交。
pub struct Running {
    rx: Receiver<Result<Report, String>>,
    started: Instant,
    label: String,
}

impl Running {
    /// 文件名（不含目录），用于完成提示。
    pub fn label(&self) -> &str {
        &self.label
    }

    /// 从 UI 提交到现在的墙钟时间，含线程调度与轮询间隔。
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// `None` = 还在跑；`Some` = 成功或失败（只返回一次）。
    pub fn poll(&mut self) -> Option<Result<Report, String>> {
        match self.rx.try_recv() {
            Ok(v) => Some(v),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("导出线程意外退出".into())),
        }
    }

    /// 退出前最多等 `budget`。返回 `true` 表示已收尾；超时后线程仍在跑，
    /// 但 `.part` + 原子改名保证不会留下半份成品。
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

/// 启动工作线程。`device`/`queue` 是 wgpu 的引用计数句柄，克隆后跨线程使用是安全的；
/// 一次性合成器在工作线程里创建，不碰画布状态。
pub fn start(
    device: egui_wgpu::wgpu::Device,
    queue: egui_wgpu::wgpu::Queue,
    info: egui_wgpu::wgpu::AdapterInfo,
    req: Request,
) -> Running {
    let label = req
        .path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "导出".into());
    let started = Instant::now();
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("zmind-export".into())
        .spawn(move || {
            let out = render(&device, &queue, info, req);
            let _ = tx.send(out);
        });
    if let Err(e) = spawned {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Err(format!("无法启动导出线程：{e}")));
        return Running { rx, started, label };
    }
    Running { rx, started, label }
}

/// 矢量 PDF 不需要 GPU：UI 线程只需场景与主题快照，排版在工作线程里做。
pub fn start_vector(
    path: PathBuf,
    scene: Scene,
    theme: Theme,
    bounds: Option<mw_core::Rect>,
) -> Running {
    let label = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "导出.pdf".into());
    let started = Instant::now();
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("zmind-export-pdf".into())
        .spawn(move || {
            let out = render_vector(Request {
                kind: Kind::Pdf,
                path,
                scene,
                theme,
                scale: 1.0,
                opaque: true,
                bounds,
            });
            let _ = tx.send(out);
        });
    if let Err(e) = spawned {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Err(format!("无法启动导出线程：{e}")));
        return Running { rx, started, label };
    }
    Running { rx, started, label }
}

fn render(
    device: &egui_wgpu::wgpu::Device,
    queue: &egui_wgpu::wgpu::Queue,
    info: egui_wgpu::wgpu::AdapterInfo,
    req: Request,
) -> Result<Report, String> {
    if req.kind == Kind::Pdf {
        // 矢量 PDF 不需要 GPU：排版 → 字形轮廓 → 内容流，工作线程直接完成。
        return render_vector(req);
    }
    let started = Instant::now();
    let b = req
        .bounds
        .unwrap_or_else(|| export::content_bounds(&req.scene));
    let (w, h, eff) = export::png_size(b.w, b.h, req.scale);
    let frame = Frame {
        scene: &req.scene,
        theme: &req.theme,
        camera: Camera::new(b.center(), eff, 1.0),
        selected: Vec::new(),
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport: Some([w as f32, h as f32]),
    };
    let mut layers = build_layers(&frame);
    let mut text =
        TextEngine::new().ok_or_else(|| "DirectWrite 不可用，无法排版导出文字".to_string())?;
    let update = text.paint(&frame, &mut layers);

    let mut comp = Compositor::from_device(device.clone(), queue.clone(), info);
    comp.set_transparent(!req.opaque);
    // 图集与合成器都是这台导出器新建的：代次 1 接 0，逐块上传即可。
    comp.upload_atlas(&update);
    let img = if w * h > 16_000_000 || w.max(h) > export::TILE_MAX {
        // 32MP 图分块渲染：每块单独出纹理，再按 tile 计划无缝拼回。
        let tiles = export::tile_plan(w, h, export::TILE_MAX);
        let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
        for t in &tiles {
            let tw = t.w.max(1);
            let th = t.h.max(1);
            let mut tile_frame = frame.clone();
            tile_frame.viewport = Some([tw as f32, th as f32]);
            tile_frame.camera.center = mw_core::Point::new(
                frame.camera.center.x + (t.x as f32 + tw as f32 * 0.5 - w as f32 * 0.5) / eff,
                frame.camera.center.y + (t.y as f32 + th as f32 * 0.5 - h as f32 * 0.5) / eff,
            );
            let piece = comp.render(&tile_frame, &layers, [tw, th]);
            let src = piece.rgba();
            export::blit_tile(&mut rgba, w, *t, &src, tw);
        }
        rgba
    } else {
        comp.render(&frame, &layers, [w, h]).rgba()
    };
    let mut rgba = img;
    export::unpremultiply(&mut rgba);
    drop(comp);
    drop(text);

    let tmp = part_path(&req.path);
    let written = mw_golden::write_png(&tmp, &rgba, w, h);
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    let bytes = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    std::fs::rename(&tmp, &req.path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("写入失败：{e}")
    })?;
    Ok(Report {
        eff,
        width: w,
        height: h,
        bytes,
        pages: 1,
        elapsed: started.elapsed(),
    })
}

/// 矢量 PDF：不经过合成器，页数与写盘原子语义与位图导出一致。
fn render_vector(req: Request) -> Result<Report, String> {
    let started = Instant::now();
    let enc = crate::export_pdf::encode_scene(&req.scene, &req.theme, req.bounds)?;
    let tmp = part_path(&req.path);
    if let Err(e) = std::fs::write(&tmp, &enc.bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("写入失败：{e}"));
    }
    let bytes = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    std::fs::rename(&tmp, &req.path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("写入失败：{e}")
    })?;
    Ok(Report {
        eff: 1.0,
        width: crate::export_pdf::PAGE_W as u32,
        height: crate::export_pdf::PAGE_H as u32,
        bytes,
        pages: enc.pages,
        elapsed: started.elapsed(),
    })
}

/// 同目录的临时文件。必须与成品同卷，`rename` 才是原子的。
fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_part_file_stays_beside_the_target() {
        let p = part_path(Path::new("D:/导出/方案.png"));
        assert_eq!(p, PathBuf::from("D:/导出/方案.png.part"));
        assert_eq!(p.parent(), Some(Path::new("D:/导出")));
    }
}
