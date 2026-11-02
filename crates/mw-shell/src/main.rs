//! 织思原生壳层 —— M0 骨架（docs/13 §3 顶层「壳层」，§6 M0 出口标准之二）
//!
//! 前提已在 §4.2 实测：Mica 可透过 eframe/wgpu 交换链显示。本骨架补上「合并标题栏」
//! 必须的三件事，且全部走**原生语义**而不是自造轮子：
//!
//! · **拖动** → `ViewportCommand::StartDrag` → winit `drag_window()` →
//!   `WM_NCLBUTTONDOWN(HTCAPTION)`。因此 Win11 的贴边快照布局、拖到顶部最大化、
//!   双击标题栏最大化**全部照常工作**，不需要自己实现。
//! · **缩放** → `ViewportCommand::BeginResize(dir)`。winit 在无框窗口下会清掉
//!   `WS_THICKFRAME`（`window_state.rs:436`），系统不再给缩放边框，必须自己铺 8 条热区，
//!   但热区只负责「触发」，真正的缩放行为仍由系统模态循环完成（跟手感是原生的）。
//! · **圆角** → 无框窗口不会被 DWM 自动圆角，需显式
//!   `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND`。

use std::process::ExitCode;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use app_helpers::*;
use icons::*;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use eframe::egui_wgpu;

mod app_helpers;
mod app_methods_1;
mod app_methods_2;
mod app_methods_3;
mod app_methods_4;
mod autosave;
mod canvas;
mod canvas_chrome;
mod chrome;
mod cli;
mod collection;
mod content_keys;
mod content_kit;
mod demo;
mod demo_notes;
mod dialog;
mod export;
mod export_job;
mod export_pdf;
mod export_prefs;
mod export_presets;
mod export_svg;
mod file_guard;
mod files;
mod fonts;
mod golden;
mod golden_input;
mod history;
mod history_ui;
mod icon_paint;
mod icons;
mod import_job;
mod knowledge;
mod menu;
mod note_library;
mod note_store;
mod note_view;
mod overlay;
mod panels;
mod paths;
mod perf;
mod recent;
mod review;
mod scroll_bar;
mod session;
mod settings;
mod settings_ui;
mod shortcuts;
mod start;
mod stats;
mod tab_strip;
mod theme_presets;
mod theme_studio;
mod theme_studio_ui;
mod trace;
mod ui_kit;
mod window_state;
use canvas::{clipboard_chord, format_zoom, Canvas, ZoomSrc};
use chrome::{c32, chrome_of, install_visuals, Chrome, Glyph, Tone, Type};
use golden::{Harness, Invocation};
use mw_render::Camera;
use perf::Perf;
use recent::Recent;
use session::Session;
use start::StartPage;
use stats::Stats;
use window_state::WindowState;

const APP_ID: &str = "cn.zhisi.shell";
const TITLEBAR_H: f32 = mw_core::design::component::TITLEBAR_H;
/// 标签条高度。**只在有两个及以上标签页时出现**，单标签视图与旧版逐像素一致。
const TABBAR_H: f32 = 34.0;
/// 缩发热区厚度（逻辑像素）。边取 6、角取 12，与 Win11 无框窗口的常见手感一致。
const EDGE: f32 = 6.0;
const CORNER: f32 = 12.0;

const STATUS_H: f32 = 26.0;

/// Win11 关闭按钮悬停红。
const CLOSE_HOT: Color32 = c32(mw_core::design::shell::CLOSE_HOT);

/// 状态栏里的长路径按字符截断（不能按字节切——中文会切出半个字）。
fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// 系统「显示动画」是否关闭（设置 → 辅助功能 → 视觉效果 → 动画效果）。
/// 读一次缓存；改变设置需要重启，和 Win32 大多数 UI 参数一致。
fn system_reduce_motion() -> bool {
    use std::sync::OnceLock;
    static REDUCED: OnceLock<bool> = OnceLock::new();
    *REDUCED.get_or_init(|| {
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        };
        // SPI_GETCLIENTAREAANIMATION 的 pvParam 是一个 BOOL（4 字节 int）。
        // 不用 windows crate 的 BOOL：0.62 里它落在 windows_core，路径与 Win32 模块不一致。
        let mut animations: i32 = 1;
        // SAFETY: 只传一个有效的 i32 指针；该 SPI 不写入更长的结构。
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(&mut animations as *mut _ as *mut std::ffi::c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        ok.is_ok() && animations == 0
    })
}

/// 主显示器的工作区（物理像素，已扣掉任务栏）。默认窗口在这里居中，
/// 而不是按整块显示器居中——否则底部会被任务栏盖住一截。
fn primary_work_area() -> Option<(f32, f32, f32, f32)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: SPI_GETWORKAREA 只往传入的 RECT 里写四个 i32。
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut r as *mut _ as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if ok.is_err() {
        return None;
    }
    Some((
        r.left as f32,
        r.top as f32,
        (r.right - r.left) as f32,
        (r.bottom - r.top) as f32,
    ))
}

/// 聚焦面包屑的一枚节点。`depth` 是路径下标（0 = 主根）。
struct Crumb {
    depth: usize,
    text: String,
    rect: Rect,
    current: bool,
    clickable: bool,
}

struct BreadcrumbBar {
    bar: Rect,
    crumbs: Vec<Crumb>,
}

/// 画布浮层（面包屑、缩放控件）共用的一套比例：同圆角、同高度、同阴影。
/// 它们都是「浮在画布上的玻璃条」，任何一项不齐都会显出拼凑感。
const FLOAT_RADIUS: f32 = canvas_chrome::FLOAT_RADIUS;
const FLOAT_H: f32 = mw_core::design::component::FLOATING_H;

/// 计算面包屑的固定布局。**绘制前**就要知道矩形：它压在画布上，必须提前交给
/// `Canvas::set_overlays` 做指针门禁，否则点面包屑会同时触发画布的框选/清空。
fn breadcrumb_layout(
    ui: &egui::Ui,
    central: Rect,
    path: &[(String, String)],
) -> Option<BreadcrumbBar> {
    if path.len() < 2 {
        return None;
    }
    let font = egui::FontId::proportional(Type::BODY);
    let mut items: Vec<(usize, String, bool)> = Vec::new();
    if path.len() <= 5 {
        for (i, (_, text)) in path.iter().enumerate() {
            items.push((i, shorten(text, 14), i + 1 < path.len()));
        }
    } else {
        items.push((0, shorten(&path[0].1, 14), true));
        items.push((usize::MAX, "…".to_string(), false));
        for (i, (_, text)) in path.iter().enumerate().skip(path.len() - 3) {
            items.push((i, shorten(text, 14), i + 1 < path.len()));
        }
    }
    let h = 24.0;
    let pad = 6.0;
    let gap = 13.0;
    let mut width = pad * 2.0;
    for (i, (_, text, _)) in items.iter().enumerate() {
        let text_w = ui
            .painter()
            .layout_no_wrap(text.clone(), font.clone(), Color32::BLACK)
            .size()
            .x;
        width += text_w + 12.0;
        if i + 1 < items.len() {
            width += gap;
        }
    }
    width = width.min(central.width() - 24.0);
    let bar = Rect::from_min_size(
        Pos2::new(central.left() + 12.0, central.top() + 10.0),
        Vec2::new(width.max(80.0), FLOAT_H),
    );
    let mut crumbs = Vec::new();
    let mut x = bar.left() + pad;
    let y = bar.top() + (FLOAT_H - h) * 0.5;
    for (i, (depth, text, clickable)) in items.iter().enumerate() {
        let text_w = ui
            .painter()
            .layout_no_wrap(text.clone(), font.clone(), Color32::BLACK)
            .size()
            .x;
        let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(text_w + 12.0, h));
        crumbs.push(Crumb {
            depth: *depth,
            text: text.clone(),
            rect,
            current: i + 1 == items.len(),
            clickable: *clickable,
        });
        x += text_w + 12.0;
        if i + 1 < items.len() {
            x += gap;
        }
    }
    Some(BreadcrumbBar { bar, crumbs })
}

/// 画面包屑；返回被点击的路径下标（由调用方执行 `focus_to`）。
fn paint_breadcrumbs(ui: &mut egui::Ui, bar: &BreadcrumbBar, ch: Chrome) -> Option<usize> {
    ui.painter()
        .add(canvas_chrome::float_shadow().as_shape(bar.bar, FLOAT_RADIUS));
    ui.painter().rect_filled(bar.bar, FLOAT_RADIUS, ch.panel);
    ui.painter().rect_stroke(
        bar.bar,
        FLOAT_RADIUS,
        Stroke::new(1.0, ch.border),
        egui::StrokeKind::Inside,
    );
    let font = egui::FontId::proportional(Type::BODY);
    let mut clicked = None;
    for (i, crumb) in bar.crumbs.iter().enumerate() {
        if crumb.clickable {
            let resp = ui.interact(
                crumb.rect,
                egui::Id::unique(("mw-crumb", i, crumb.depth)),
                Sense::click(),
            );
            if resp.hovered() {
                ui.painter().rect_filled(crumb.rect, 5.0, ch.hover);
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                clicked = Some(crumb.depth);
            }
        }
        ui.painter().text(
            crumb.rect.center(),
            Align2::CENTER_CENTER,
            &crumb.text,
            font.clone(),
            if crumb.current { ch.text } else { ch.dim },
        );
        if i + 1 < bar.crumbs.len() {
            let x = (crumb.rect.right() + bar.crumbs[i + 1].rect.left()) / 2.0;
            let y = crumb.rect.center().y;
            ui.painter().add(egui::Shape::line(
                vec![
                    Pos2::new(x - 1.8, y - 3.6),
                    Pos2::new(x + 1.8, y),
                    Pos2::new(x - 1.8, y + 3.6),
                ],
                Stroke::new(1.2, ch.faint),
            ));
        }
    }
    clicked
}

/// 右下角缩放控件的屏幕矩形。
fn zoom_control_rect(central: Rect) -> Rect {
    canvas_chrome::zoom_bar_layout(central).bar
}

fn zoom_control(ui: &mut egui::Ui, canvas: &mut Canvas, central: Rect, ch: Chrome) {
    let z = canvas_chrome::zoom_bar_layout(central);
    let rect = z.bar;
    ui.painter()
        .add(canvas_chrome::float_shadow().as_shape(rect, FLOAT_RADIUS));
    ui.painter().rect_filled(rect, FLOAT_RADIUS, ch.panel);
    ui.painter().rect_stroke(
        rect,
        FLOAT_RADIUS,
        Stroke::new(mw_core::design::stroke::HAIRLINE, ch.border),
        StrokeKind::Inside,
    );

    let hit = |ui: &mut egui::Ui, area: Rect, id: &str, hint: &str| -> egui::Response {
        let resp = ui.interact(area, egui::Id::unique(id), Sense::click());
        if resp.hovered() {
            ui.painter()
                .rect_filled(area, mw_core::design::radius::XS, ch.hover);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.on_hover_text(hint)
    };
    // 加减号用线画，别让字体决定粗细和对齐。
    let sign = |ui: &egui::Ui, resp: &egui::Response, area: Rect, plus: bool| {
        let c = area.center();
        let stroke = Stroke::new(1.4, if resp.hovered() { ch.text } else { ch.dim });
        ui.painter()
            .line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], stroke);
        if plus {
            ui.painter()
                .line_segment([c + Vec2::new(0.0, -5.0), c + Vec2::new(0.0, 5.0)], stroke);
        }
    };

    let minus = hit(ui, z.minus, "mw-zoom-out", "缩小 · Ctrl+-");
    sign(ui, &minus, z.minus, false);
    if minus.clicked() {
        canvas.zoom_by(1.0 / canvas_chrome::ZOOM_STEP, ZoomSrc::Button);
    }
    let pct = format_zoom(canvas.zoom());
    let reset = hit(ui, z.pct, "mw-zoom-reset", "重置为 100%");
    ui.painter().text(
        z.pct.center(),
        Align2::CENTER_CENTER,
        &pct,
        FontId::proportional(Type::SMALL),
        if reset.hovered() { ch.text } else { ch.dim },
    );
    if reset.clicked() {
        canvas.zoom_reset_action();
    }
    let plus = hit(ui, z.plus, "mw-zoom-in", "放大 · Ctrl++");
    sign(ui, &plus, z.plus, true);
    if plus.clicked() {
        canvas.zoom_by(canvas_chrome::ZOOM_STEP, ZoomSrc::Button);
    }
    ui.painter().vline(
        z.sep_x,
        (rect.top() + 6.0)..=(rect.bottom() - 6.0),
        Stroke::new(mw_core::design::stroke::HAIRLINE, ch.sep),
    );
    let fit = hit(ui, z.fit, "mw-zoom-fit", "适应画布 · Home");
    ui.painter().text(
        z.fit.center(),
        Align2::CENTER_CENTER,
        "适应",
        FontId::proportional(Type::SMALL),
        if fit.hovered() { ch.text } else { ch.dim },
    );
    if fit.clicked() {
        canvas.fit_view();
    }
}

/// 右下角缩略卡（M30）——`docs/02` §4 / `docs/06` §4 的「小地图」。
///
/// 只读 `Scene`（`Canvas::scene`）与相机（`Canvas::camera` / `view_bounds`），
/// 世界 ⇄ 卡内的换算全部交给 `mw_ui::overview`（纯几何层，与画布共用同一份 `Scene`，
/// 不另起一条布局）。绘制在 egui 覆盖层：`mw_render::Frame` 的 ABI 不动，导出无污染。
///
/// 返回这一帧之后的折叠态。指针输入不能穿透到画布：调用方必须把 `card` 提前塞进
/// `Canvas::set_overlays`（与缩放条、面包屑、工具条同一道闸门）。
fn minimap_panel(
    ui: &mut egui::Ui,
    canvas: &mut Canvas,
    central: Rect,
    ch: Chrome,
    collapsed: bool,
) -> bool {
    let Some(m) = canvas_chrome::minimap_layout(central, collapsed) else {
        return collapsed;
    };
    let painter = ui.painter();
    painter.add(canvas_chrome::float_shadow().as_shape(m.card, FLOAT_RADIUS));
    painter.rect_filled(m.card, FLOAT_RADIUS, ch.panel);
    painter.rect_stroke(
        m.card,
        FLOAT_RADIUS,
        Stroke::new(mw_core::design::stroke::HAIRLINE, ch.border),
        StrokeKind::Inside,
    );

    let view = canvas.view_bounds();
    let scene_bounds = canvas.scene().bounds;
    let inner = m.inner;
    let clipped = painter.with_clip_rect(inner);
    if let Some(map) = mw_ui::overview::Map::new(scene_bounds, [inner.width(), inner.height()], 0.0)
    {
        let scene = canvas.scene();
        // 代理块先画、关系线后画（线压块），与 `overview::proxies` 的顺序同一口径。
        // 大文档走 `blocks_for` 的 900 块封顶，小文档全量——这条规格在纯几何层钉死。
        let cap = if scene.nodes.len() > mw_ui::overview::DOWNSAMPLE_MIN_NODES {
            mw_ui::overview::MAX_BLOCKS
        } else {
            usize::MAX
        };
        for proxy in mw_ui::overview::proxies(scene, &map, cap) {
            match proxy {
                mw_ui::overview::Proxy::Block { idx, rect, .. } => {
                    let color = scene
                        .nodes
                        .get(idx as usize)
                        .map(|n| crate::chrome::c32(n.style.branch_color))
                        .unwrap_or(ch.dim);
                    let r = Rect::from_min_size(
                        Pos2::new(inner.left() + rect.x, inner.top() + rect.y),
                        Vec2::new(rect.w.max(1.6), rect.h.max(1.2)),
                    );
                    clipped.rect_filled(r, 1.0, color);
                }
                mw_ui::overview::Proxy::Link { from, mid, to } => {
                    let p = |q: mw_core::Point| Pos2::new(inner.left() + q.x, inner.top() + q.y);
                    clipped.add(egui::Shape::line(
                        vec![p(from), p(mid), p(to)],
                        Stroke::new(0.8, ch.faint),
                    ));
                }
            }
        }
        // 当前视口框：淡强调底 + 1px 强调描边（`docs/06` §4「显示全图与当前视口框」）。
        let v = map.rect_in_thumb(view);
        let vr = Rect::from_min_size(
            Pos2::new(inner.left() + v.x, inner.top() + v.y),
            Vec2::new(v.w.max(2.0), v.h.max(2.0)),
        );
        clipped.rect_filled(
            vr,
            mw_core::design::radius::XS,
            Color32::from_rgba_unmultiplied(ch.accent.r(), ch.accent.g(), ch.accent.b(), 26),
        );
        clipped.rect_stroke(
            vr,
            mw_core::design::radius::XS,
            Stroke::new(1.0, ch.accent),
            StrokeKind::Inside,
        );
    }

    // 折叠按钮：命中区 24×24（`docs/13` §4.19 的旧账），图标仍只画 12px。
    if !collapsed {
        let resp = ui.interact(
            m.collapse,
            egui::Id::unique("mw-minimap-collapse"),
            Sense::click(),
        );
        if resp.hovered() {
            ui.painter()
                .rect_filled(m.collapse, mw_core::design::radius::XS, ch.hover);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let c = m.collapse.center();
        ui.painter().line_segment(
            [c + Vec2::new(-6.0, 0.0), c + Vec2::new(6.0, 0.0)],
            Stroke::new(1.4, if resp.hovered() { ch.text } else { ch.dim }),
        );
        if resp.clicked() {
            crate::trace::event("minimap toggle collapsed=1");
            return true;
        }
    }

    // 折叠态：整块角标就是展开按钮（`minimap_layout` 的 `collapse == card`）。
    if collapsed {
        let resp = ui.interact(
            m.card,
            egui::Id::unique("mw-minimap-expand"),
            Sense::click(),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            crate::trace::event("minimap toggle collapsed=0");
            return false;
        }
        return true;
    }

    let resp = ui.interact(
        inner,
        egui::Id::unique("mw-minimap-pan"),
        Sense::click_and_drag(),
    );
    let over_collapse = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| m.collapse.contains(p));
    if resp.hovered() && !over_collapse {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        let factor = canvas_chrome::minimap_wheel_factor(dy);
        if factor != 1.0 {
            if let Some(map) =
                mw_ui::overview::Map::new(scene_bounds, [inner.width(), inner.height()], 0.0)
            {
                if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                    let at = mw_core::Point::new(p.x - inner.left(), p.y - inner.top());
                    let world = map.thumb_to_world(at);
                    // 卡片上的锚点是世界点，画布入口收的是屏幕点：先投影回画布视口
                    // （`world_to_screen` 与 `screen_to_world` 互逆），再走与滚轮同一个
                    // `zoom_step_at`——钳位与锚点补偿只剩一条式子，轨迹 `src=minimap`。
                    let screen = canvas.camera().world_to_screen(world, canvas.viewport());
                    // 轨迹只留 `zoom_anchor src=minimap` 一条：同一件事两套写法会让探针
                    // 分叉（倍率已由 `zoom` 的前后值可推，`factor=` 是冗余的）。
                    canvas.zoom_step_at(screen, factor, ZoomSrc::Minimap);
                }
            }
        }
    }
    if !over_collapse && (resp.clicked() || resp.dragged()) {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(map) =
                mw_ui::overview::Map::new(scene_bounds, [inner.width(), inner.height()], 0.0)
            {
                let at = mw_core::Point::new(p.x - inner.left(), p.y - inner.top());
                let center = mw_ui::overview::center_from_thumb(&map, scene_bounds, at, view);
                canvas.pan_to(center);
                crate::trace::event(&format!(
                    "minimap pan center=({:.1},{:.1})",
                    center.x, center.y
                ));
            }
        }
    }
    // 双击卡内的一颗节点：选中并露出它（M30b）。卡上块只有 1–2px，容差按
    // 4 css px 折成世界单位；双击空白走回「拖视口框」，不选中、不跳相机。
    if !over_collapse && resp.double_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(map) =
                mw_ui::overview::Map::new(scene_bounds, [inner.width(), inner.height()], 0.0)
            {
                let at = mw_core::Point::new(p.x - inner.left(), p.y - inner.top());
                let world = map.thumb_to_world(at);
                let pad = 4.0 / map.scale.max(1e-4);
                let hit = {
                    let scene = canvas.scene();
                    mw_ui::overview::node_at(scene, world, pad).map(str::to_string)
                };
                match hit {
                    Some(id) => {
                        canvas.select_node(&id);
                        canvas.reveal_id(&id);
                        crate::trace::event(&format!("minimap locate id={id} hit=1"));
                    }
                    None => crate::trace::event("minimap locate hit=0"),
                }
            }
        }
    }
    collapsed
}

/// 标签管理窗：列出全库标签，选一个改名。
#[derive(Default)]
struct TagManager {
    from: Option<String>,
    to: String,
}

enum LibraryDialogKind {
    Export(Vec<String>),
    Import,
}

struct LibraryDialog {
    kind: LibraryDialogKind,
    path: String,
}

struct Shell {
    backdrop: Backdrop,
    /// **当前标签页**的画布。整个窗口只有一个 `Canvas`（一份合成器 + 一个 3072² 字形图集）；
    /// 切标签只是把它里面的会话/相机/选择换掉，不重建画布。见 [`Shell::switch_tab`]。
    canvas: Option<Canvas>,
    /// 标签页状态槽。`tabs[active]` 里是**上一轮换出的陈旧会话**，权威内容在 `canvas` 里；
    /// 非活动槽的 `session` 才是权威的——读名字/脏标记时按这个规则取（[`Shell::tab_label`]）。
    tabs: Vec<Tab>,
    /// 当前标签页在 `tabs` 里的下标。
    active: usize,
    /// 关闭某个有未保存改动的标签页时的待确认下标。
    confirm_close_tab: Option<usize>,
    /// `Some` 时本进程是快照 harness：渲染若干帧后截图、比对、退出。
    harness: Option<Harness>,
    /// 金样输入脚本（仅 harness）：按帧注入键鼠/IME，验收不依赖桌面焦点。
    golden_input: Option<golden_input::GoldenInput>,
    /// 金样虚拟时钟（秒）：每帧固定 1/60，`sleep` 到期补名义时长。
    /// 注入 `RawInput::time` 后，egui 的悬停计时、滚动落定、动画都按帧数走，
    /// 与真实帧耗时无关——门里第一遍机器降载时也不会抓出第二种画面。
    golden_clock: f64,
    exit: Arc<AtomicI32>,
    /// 底部一闪而过的状态条（保存成功 / 打开失败）。`None` 时不画，golden 因此不受影响。
    toast: Option<Toast>,
    /// 样式类编辑的合并窗口（`docs/30` §4.3）：同一批 id 的连点 / 连拖在 500ms 内并成一条撤销。
    /// 记的是「上一次样式命令的手势键 + 时刻」，判据在 `panels::inspector::same_gesture`。
    style_gesture: Option<(String, std::time::Instant)>,
    /// 有未保存改动时按了关闭：先弹确认，别让人一手抖丢半小时。
    confirm_close: bool,
    /// 用户已经确认过关闭，下一次 Close 直接放行。
    close_confirmed: bool,
    /// 主窗口句柄（对话框要它当 owner，否则会弹到窗口后面去）。
    hwnd: Option<isize>,
    /// 用户想做但被「有未保存改动」拦下的事（打开/新建/导入）。
    pending: Option<PendingAction>,
    /// 最近文件（读一次，之后跟着打开/保存更新）。
    recent: Recent,
    /// 最近文件列表的异步读取句柄。`Some` = 还在读，起始页最近文件区画 3 行骨架。
    recent_load: Option<std::sync::mpsc::Receiver<Recent>>,
    /// 金样用的确定性「假装还没读完」帧数（`MW_RECENTS_PENDING_FRAMES`）；真实运行恒为 0。
    recents_pending_frames: u32,
    /// 启动时发现的恢复点：`(恢复点路径, 正本路径)`。
    recovery: Option<(std::path::PathBuf, std::path::PathBuf)>,
    /// 下一次自动保存的时刻（egui 时间）。
    next_autosave: f64,
    /// 每份文档（按恢复点槽位）上次自动保存到的修订号。
    ///
    /// 多标签必须按槽位分别记：一个 `u64` 会让「标签 A 刚写完 40」把「标签 B 还是 12」
    /// 误判成已保存，B 的恢复点就再也不会更新。
    last_autosave_rev: std::collections::HashMap<std::path::PathBuf, u64>,
    /// 自动保存的后台写线程：UI 线程只取快照、收结果，序列化与磁盘 IO 不在帧内。
    autosave: autosave::Background,
    /// 本地卡片库（P08）。没有 %APPDATA% 时为 None：功能降级，不阻塞打开文档。
    notes: Option<note_store::NoteStore>,
    /// 打开中的卡片阅读页（P12）。
    note_reader: Option<note_view::CardReader>,
    /// 卡片是从笔记库点开的：关闭时回笔记库，而不是落回开始页/画布。
    reader_from_library: bool,
    /// 后台导入任务（P14）。
    import_job: Option<import_job::ImportJob>,
    /// 导入完成后的丢失项/降级项，弹一次窗。
    import_warnings: Option<Vec<String>>,
    /// 标签管理窗（P14）。
    tag_manager: Option<TagManager>,
    /// 笔记库页（P15）。
    library: Option<note_library::NoteLibrary>,
    /// 笔记库导出/导入路径对话框（P20）。
    library_dialog: Option<LibraryDialog>,
    /// 大纲视图与画布共用同一份行缓存（只在文档变化时重建）。
    outline: panels::Outline,
    /// 大纲 `Tab` 刚建出来的概念：等它的**名字**提交后，再自动立起短语编辑框
    /// （docs/30 §5.6.1 的两步：先给概念起名，再给边填短语）。
    via_after_rename: Option<String>,
    /// 验收钩子 `MW_OUTLINE_SELECT` 钉住的行：每帧把选中与按键焦点按回它，
    /// 直到行内编辑或短语框立起来为止（`raw_input_hook` 里执行）。
    /// 只在离屏场景用；正常运行时恒为 `None`。
    outline_pin: Option<String>,
    /// 还要再钉几帧（首帧的「选中为空选根」会把焦点抢走，钉几次就够；
    /// 一直钉会把短语框/文本框的焦点每帧抢回行上）。
    outline_pin_left: u8,
    /// `MW_OUTLINE_SELECT` 只武装一次：行内编辑/短语框把 pin 清掉之后不再重新钉，
    /// 否则夹具会跟文本框抢焦点，把短语框挤没。
    outline_pin_done: bool,
    /// 全屏大纲视图（F8）：中央面板整块让给大纲，不再挤压画布。
    outline_view: bool,
    /// 大纲页那条「聚焦问题」的就地编辑态（CM-10 ⑥，`docs/32` §8）。
    /// 类型与画布那个 `FocusEdit` 是同一个，但各持一份：两处的生命周期互不相干。
    outline_question_edit: Option<canvas::FocusEdit>,
    /// 检查器（右侧属性面板，F9）。
    inspector: panels::Inspector,
    show_inspector: bool,
    /// 检查器宽度（逻辑 px），拖拽后落盘。
    inspector_w: f32,
    /// 右下角缩略卡（M30）是否显示。开合落盘；**出厂关**，既有基线因此零漂移。
    show_minimap: bool,
    /// 缩略卡是否折叠成 34×26 角标。只是本进程的视图态，不落盘：
    /// 重启后回到展开，用户不会以为「角标怎么又打不开了」。
    minimap_collapsed: bool,
    /// 面板焦点记忆（H3）：面板关掉再打开时，把键盘焦点送回原控件。
    /// 长期持有——面板一关就清空的话，重开时无从恢复。
    focus_mem: panels::FocusMemory,
    /// 首次引导（P42）：看完或跳过就落盘，不再出现。
    onboarding: bool,
    /// 冲突提示：哪份文件被外部改过了（**只记路径**，守卫本身在 `Tab::guard` 里）。
    ///
    /// SR-02：`file_lock` / `disk_print` / `read_only` 以前也挂在这里，但文档是**每标签**的，
    /// 窗口级字段必然张冠李戴（打开 A 再打开 B，切回 A 时用的是 B 的指纹）。
    /// 三项已搬进 [`TabGuard`]；这里只留这条「谁在冲突」的提示，它本来就是一次性事件。
    conflict: Option<std::path::PathBuf>,
    /// 已提交、还没收尾的自动保存修订号（SR-02 问题 7：提交与记账分开）。
    ///
    /// 以前 `submit()` 一返回就把修订号记进 `last_autosave_rev`，可后台任务可能被
    /// `invalidate`/作废掉、结果根本不回来；那一修订号此后被当成「已保存」，再不重投。
    /// 现在这张表只表示「在途」，收尾（成功/失败/被作废）就删。
    autosave_submitted: std::collections::HashMap<std::path::PathBuf, u64>,
    /// 自动保存失败退避：槽位 → 连败次数与下次可重试的墙钟时刻（SR-02 问题 6）。
    ///
    /// 用 `Instant` 而不是 egui 时间：提示条与重试都要在原生文件对话框阻塞 UI 线程时仍然正确。
    autosave_stalls: std::collections::HashMap<std::path::PathBuf, AutosaveStall>,
    /// 用户已经在「有未保存改动」里确认过丢弃：下一次替换文档放行（**一次性**）。
    ///
    /// 脏检查不能只守在按钮那一处（`request`），真正的数据通道是 `load_open`；
    /// 但确认之后必须放行，否则会自己拦住自己、无限弹窗。
    allow_replace: bool,
    /// 上一次做过收尾的 `(路径, 修订号)`：`after_saved()` 的幂等闸门（SR-02 问题 5）。
    last_finish: Option<(std::path::PathBuf, u64)>,
    /// 恢复中心列表（打开时扫描一次，操作后刷新）。
    recovery_center: Option<Vec<autosave::RecoveryPoint>>,
    /// 全局设置（外观/自动保存/导出/笔记库）。
    settings: settings::Store,
    settings_open: bool,
    /// 验收钩子：再等几帧自动打开设置中心（`MW_SETTINGS_AFTER=<n>`）。
    /// 金样要先在画布上堆够撤销历史，再让设置弹出来——`MW_SETTINGS_DEMO` 开局就开，
    /// 后续按键全被对话框吃掉（打磨轮 59 实测）。
    settings_open_after: Option<u32>,
    /// 设置里「调低撤销上限会丢历史」警告行的最近一次（dropped, depth）：只在变化时记轨迹。
    settings_warn_log: Option<(usize, usize)>,
    /// 主题编辑器（打开时存在）。
    theme_studio: Option<theme_studio::Studio>,
    /// 导出预设（命名格式/范围/倍率/背景）。
    export_presets: Vec<export_presets::Preset>,
    /// 版本历史窗口。
    history: Option<(std::path::PathBuf, Vec<history::Snapshot>)>,
    /// 壳层颜色（顶栏/面板/弹层/状态栏）。主题变化时在 `sync_chrome` 里刷一次，
    /// 绘制点只读不算——暗色主题下 chrome 不能还是白的。
    chrome: Chrome,
    /// `chrome` 与 egui visuals 当前对应的主题 id。
    chrome_theme: String,
    /// 浮层配色（搜索条 / 命令面板 / 菜单 / 帮助），与 `chrome` 同步刷新。
    tone: Tone,
    /// 命令面板（Ctrl+K）。
    palette: panels::Palette,
    /// 搜索条（Ctrl+F）。
    search: panels::SearchBar,
    /// 右键菜单。`Some` 时画布不收键盘与指针，菜单收起后立刻清空。
    context_menu: Option<panels::Menu>,
    /// 大纲行菜单（行尾 `⋯` / 行右键）：与右键菜单同一套引擎与键盘纪律。
    outline_row_menu: Option<panels::outline::RowMenu>,
    /// 标签页右键菜单：与画布菜单同一套 X16 引擎，但动作类型不同。
    tab_menu: Option<menu::Menu<TabAction>>,
    /// 正在拖拽重排的标签；`Some` 时画布锁输入并画插入线。
    tab_drag: Option<TabDrag>,
    /// 标签条上一帧画出的 (活动下标, 标签数, 标签条宽)。变化后把活动标签滚进视口；
    /// 滚轮滚动不改变它，所以不会把用户从别处抢回来。
    tab_seen: Option<(usize, usize, u32)>,
    /// 标签条上一帧的实际横向偏移，仅用于 `MW_TRACE` 记录 `tab_scroll`。
    tab_scroll_x: f32,
    /// 保存阶段：Saving 是「已按下、等下一帧写盘」，Failed 只在失败修订号上有效。
    save_phase: SavePhase,
    /// 卡片页排队中的一次保存（下一帧开头写盘，见 `pump_card_save`）。
    pending_card_save: Option<CardSave>,
    /// 当前打开的**库内**卡片在磁盘上的指纹（外部修改检测）；文档内卡片恒为 `None`。
    card_disk_print: Option<file_guard::Fingerprint>,
    /// 用户在「文件已在外部修改」里点了「覆盖」：这一笔不再比指纹。
    card_overwrite: bool,
    /// 两段式打开计划；`painted` 之前不读盘。
    open_plan: Option<OpenPlan>,
    /// 打开失败弹窗的现场；`Some` 时 overlay 顶层是 OpenFailed。
    open_failed: Option<OpenFailure>,
    /// eframe 上下文（保存需要主动请求下一帧，才能先画「保存中」再写盘）。
    ctx: egui::Context,
    /// 快捷键速查（F1）。
    help: panels::Help,
    /// 窗口级快捷键（可配置，落盘到 %APPDATA%\织思\shortcuts.json）。
    shortcuts: shortcuts::Bindings,
    /// 快捷键设置面板。
    shortcuts_dialog: shortcuts::Dialog,
    /// 「我的主题」预设（跨文档复用）。
    presets: theme_presets::Presets,
    /// 系统「减少动画」；面板开合与搜索呼吸环据此降级。
    reduce_motion: bool,
    /// 导出对话框的选项（格式 / PNG 比例 / 背景）。关掉对话框后选项留着，下次还是上次那套。
    export: ExportOpts,
    /// PNG/PDF 的后台导出任务。单飞：同一时刻只允许一个（纹理回读很重）。
    export_job: Option<export_job::Running>,
    /// `MW_EXPORT_BENCH=<out>`：首帧画布后自动跑一次 PNG 导出并打印耗时（P11 量法）。
    bench_export: Option<std::path::PathBuf>,
    /// 基准导出是否已经启动（只跑一次）。
    bench_started: bool,
    /// 已同步进 egui 样式的主题主色；只在主题真的换了时重建 Style。
    ui_accent: Option<Color32>,
    /// 首帧按显示器工作区收一次窗口尺寸。winit 的 `clamp_size_to_monitor_size`
    /// 在 150% + 小屏上只夹外框，内容区仍可能超出屏幕。
    sized_to_monitor: bool,
    /// F11 专注模式：`Some` 时记住进入前的窗口几何。见 [`Shell::toggle_focus_mode`]。
    focus_mode: Option<FocusRestore>,
    /// 阅读模式：隐藏面板与悬浮工具条，退出恢复视图。
    reading: Option<ReadingRestore>,
    /// 主动回忆：本地复习状态与当前会话。
    review_store: review::Store,
    review: Option<ReviewSession>,
    /// 知识视图（卡片/标签/主题关系图）。
    knowledge: Option<KnowledgeView>,
    /// 学习统计窗口。
    stats_open: bool,
    /// 专注模式下上一次的焦点状态：失焦撤置顶，回来再置顶。
    focus_prev_focused: bool,
    /// 窗口几何 + 面板开合的持久化。harness 下是 disabled（不读不写），
    /// 否则 golden 基线会被本机上次的窗口布局带偏。
    window_state: WindowState,
    /// 上次的窗口几何是否已经处理过（恢复成功、或判定无需恢复）。
    geometry_restored: bool,
    /// 下一次写窗口状态的时刻。按 `Instant` 防抖而不是每帧 stat/写盘。
    next_geometry_save: std::time::Instant,
    /// 恢复窗口几何后的一小段"重新取景"窗口：真实 resize 比命令行晚一两帧到，
    /// 期间每帧都让画布按当前视口重 fit，落定后的构图才不会偏向一侧。
    refit_until: Option<std::time::Instant>,
    /// 启动后还要盯着视口几帧（**不是**每帧都重取景：只有视口和上次取景时不一样才重取）。
    /// 窗口尺寸/面板宽度要晚一两帧才落定，首帧那次 `fit` 读到的是
    /// **没落定的视口**，文档会被画小一档（实测同一包围盒首帧 `zoom=1.2369`、落定后 `1.5`）。
    /// 用帧预算而不是墙钟：金样与慢机器必须同结果。
    start_refit_frames: u32,
    /// 开始页正在显示。此时画布/面板/标签条都不画，但标签页原封不动——
    /// 从菜单进开始页只是"盖住文档"，不是关闭文档。
    home: bool,
    /// 开始页可以退回当前文档（从菜单进入）。关掉最后一页进入时为 `false`，
    /// 页面上不出现"返回文档"，避免退回一张被自己刚关掉的空白页。
    home_return: bool,
    /// 开始页自身状态（欢迎条是否已关闭）。
    start: StartPage,
    /// 上次发给系统的窗口标题。`ViewportCommand::Title` 每帧发等于每帧调一次
    /// winit，这里只在文档名/开始页真的变了时同步（任务栏与 Alt+Tab 都看这个）。
    window_title: Option<String>,
    /// 启动里程碑（docs/08 P1/P3），`MW_PERF_LOG` 未设置时不写任何东西。
    perf: Perf,
    /// 第一帧真实画布是否已经渲染过（只用于打一次 `first_canvas`）。
    perf_saw_canvas: bool,
    /// 第一帧 UI 是否已经跑过（开始页没有画布，P2 用它当可交互端点）。
    perf_saw_ui: bool,
    /// 运行时帧率探针（`MW_STATS=1`），发布版默认关闭。
    stats: Stats,
    /// `MW_DIALOG_DEMO=1`：打开统一弹窗演示，给 golden 与行为脚本当靶子。
    dialog_demo: bool,
    /// `MW_CONTENT_KIT=1`：组件陈列页（`content_kit::show_gallery`），只服务
    /// CER-02 的 content-kit golden 与人工复核，不进任何交互路径。
    content_kit_demo: bool,
    /// 概念图验收钩子（CM-08）：`MW_CONCEPT_EDIT/LINK/REL_EDIT` 里的一次性动作。
    ///
    /// 为什么不在启动时直接跑：短语编辑框要落在线段中点，而那个中点得先有
    /// `last_viewport` / `last_rect`——它们在第一帧画布绘制之后才成立。启动时算，
    /// 框会掉到窗口左上角，基线拍到的就是个错的东西。
    concept_hook: Option<canvas::ConceptHook>,
    /// 钩子还要等几帧再跑。第一帧的画布矩形是「菜单栏还没占位」的那一版，
    /// 相机 fit 也是按它算的：第 1 帧就取中点，编辑框会偏到左上（实测差 215/115px）。
    /// 等布局落定再跑，坐标才是屏幕上真实的那条线的中点。
    concept_hook_after: u8,
}

/// 专注模式进入前的窗口几何（物理像素）。退出时原样放回；`maximized` 记录当时是否最大化。
#[derive(Clone, Copy)]
struct FocusRestore {
    rect: (i32, i32, i32, i32),
    maximized: bool,
}

/// 阅读模式进入前的视图：退出时把相机、选择、面板原样还回去。
struct ReadingRestore {
    camera: mw_render::Camera,
    selection: Vec<String>,
    panels: (bool, bool),
}

/// 一次复习会话：到期项 + 当前下标 + 是否已翻面。
struct ReviewSession {
    items: Vec<ReviewItem>,
    at: usize,
    revealed: bool,
}

struct ReviewItem {
    key: String,
    title: String,
    answer: String,
}

/// 知识视图的现场：图、平移、缩放。
struct KnowledgeView {
    graph: knowledge::Graph,
    pan: egui::Vec2,
    zoom: f32,
}

/// 导出对话框的选项。`open` 是对话框本体的开关，其余三项是记忆值。
#[derive(Clone, Copy)]
struct ExportOpts {
    open: bool,
    fmt: export::Format,
    /// PNG 比例（世界单位 → 像素的倍数）。
    scale: f32,
    /// PNG 背景：`true` = 用文档底色，`false` = 透明。
    opaque: bool,
    /// 导出范围：整张导图 / 选中子树（文本格式忽略这一项）。
    scope: export::Scope,
}

impl Default for ExportOpts {
    fn default() -> Self {
        // 上次选的格式/范围/倍率沿用到这一次——连着导两份不同格式是常事。
        let (fmt, scope, scale, opaque) = export_prefs::Prefs::load().restore();
        Self {
            open: false,
            fmt,
            scale,
            opaque,
            scope,
        }
    }
}

/// 一个标签页的持久状态。切标签时把当前画布的会话/相机/选择存回这里，
/// 再把目标标签的状态装回画布（[`Shell::switch_tab`]）。
///
/// **不持有画布/合成器/字形图集**：那些是全窗口共用的（一个 16MB 图集，按标签复制会爆内存）。
struct Tab {
    session: Session,
    camera: Camera,
    /// 有序选中节点 id（末位为主选）。切回来按 id 重映射到新场景索引。
    sel_ids: Vec<String>,
    /// 这一份文档自己的文件守卫（SR-02 问题 1）。
    ///
    /// 以前这三项挂在 `Shell` 上：打开 A 再打开 B，切回 A 时用的还是 B 的指纹，
    /// `Ctrl+S` 就会弹假「文件已被外部修改」。守卫必须跟着文档走，所以放进标签。
    guard: TabGuard,
}

/// 一个标签的文件守卫：协作锁 + 磁盘指纹 + 只读态。
///
/// 三者要么同时属于一份文档，要么同时为空（未命名文档），所以打成一个结构体，
/// 搬进 [`Tab`] 之后就不可能再出现「锁是 A 的、指纹是 B 的」这种撕裂状态。
#[derive(Default)]
pub(crate) struct TabGuard {
    /// 这份守卫对应的路径。路径一变（另存为 / 重新加载到别的文件）就要放掉旧锁。
    pub(crate) path: Option<std::path::PathBuf>,
    pub(crate) lock: Option<file_guard::Guard>,
    pub(crate) print: Option<file_guard::Fingerprint>,
    pub(crate) read_only: bool,
}

impl TabGuard {
    /// 按 `path` 武装守卫：换路径先放旧锁，没锁就取锁，再重取指纹、判只读。
    ///
    /// “没锁才取锁”这一句是刻意的：切回自己已经持锁的标签时不能重复取——
    /// `create_new` 语义下第二次必然失败，那会把可写的文档误判成只读。
    fn arm(&mut self, path: Option<&std::path::Path>) {
        let Some(path) = path else {
            // 未命名文档：不留陈旧锁，也不该是只读。
            *self = Self::default();
            return;
        };
        if self.path.as_deref() != Some(path) {
            // 换了文档：旧文件的 `.lock` 必须立刻放掉，否则那个文件到关窗都是只读。
            self.lock = None;
            self.path = Some(path.to_path_buf());
        }
        if self.lock.is_none() {
            self.lock = file_guard::Guard::try_acquire(path);
        }
        self.print = file_guard::fingerprint(path);
        self.read_only = self.lock.is_none();
    }

    /// 只重取指纹（刚写盘成功，锁和只读态都没变）。
    fn refresh_print(&mut self) {
        if let Some(path) = self.path.clone() {
            self.print = file_guard::fingerprint(&path);
        }
    }

    /// 放掉锁并清空守卫（重新加载 / 换成未命名文档时）。
    fn release(&mut self) {
        *self = Self::default();
    }
}
impl Tab {
    fn new(session: Session) -> Self {
        // 新标签也要武装：只改 `load_active` 不够——新标签第一次保存同样会读到空指纹，
        // 于是"没有已知指纹"被当成不冲突、或反过来弹假冲突（lead 提醒的第二点）。
        let mut guard = TabGuard::default();
        guard.arm(session.path.as_deref());
        Self {
            session,
            camera: Camera::new(mw_core::Point::new(0.0, 0.0), 1.0, 1.0),
            sel_ids: Vec::new(),
            guard,
        }
    }
}

/// 关窗脏检查的判定内核：`(标签名, 脏?)` 快照 → 需要提示的名字（保持标签顺序）。
///
/// 独立成自由函数是因为老代码只看活动标签那一份（`guard_close` 里 `canvas.session().dirty()`），
/// 后台标签的未保存改动连一句话都没有；这段判据要能脱离 GPU 直接测。
fn dirty_tab_names(tabs: &[(String, bool)]) -> Vec<String> {
    tabs.iter()
        .filter(|(_, dirty)| *dirty)
        .map(|(name, _)| name.clone())
        .collect()
}

/// 关窗时后台标签的写盘结果（活动标签另算，它走画布的三阶段保存）。
#[derive(Default, Debug, PartialEq, Eq)]
struct SaveBackground {
    /// 写成功的标签数。
    saved: usize,
    /// 写失败的标签名（含原因）。
    failed: Vec<String>,
    /// 没有路径、无法非交互落盘的标签名（未命名文档）。
    untitled: Vec<String>,
}

/// 关窗前把**后台标签**的脏文档写回各自的路径（SR-02 问题 2）。
///
/// 「保存并关闭」以前只 `action_save()` 一次，后台标签的改动就这么丢了；
/// 这里逐个写回，成功就刷新它自己的守卫指纹。活动标签不在其中：
/// 它的权威会话在画布上，走既有的三阶段保存（`action_save` + `pump_save`）。
fn save_background_dirty_tabs(tabs: &mut [Tab], active: usize) -> SaveBackground {
    let mut out = SaveBackground::default();
    for (i, tab) in tabs.iter_mut().enumerate() {
        if i == active || !tab.session.dirty() {
            continue;
        }
        let Some(path) = tab.session.path.clone() else {
            out.untitled.push(tab.session.title());
            continue;
        };
        match tab.session.save_to(&path) {
            Ok(()) => {
                tab.guard.refresh_print();
                out.saved += 1;
            }
            Err(e) => out.failed.push(format!("{}：{e}", tab.session.title())),
        }
    }
    out
}

/// 退出前这份文档要不要再补一份恢复点（SR-02 问题 3）。
///
/// 脏、且这一修订号还没有确认写进恢复点 → 补。`confirmed` 是**确认**写成功的修订号，
/// 不是「提交过」的修订号（见问题 7）。
fn needs_recovery_submit(dirty: bool, rev: u64, confirmed: Option<u64>) -> bool {
    dirty && confirmed != Some(rev)
}

/// 自动保存失败后的退避状态（每个恢复点槽位一份，SR-02 问题 6）。
#[derive(Clone, Copy, Debug)]
struct AutosaveStall {
    /// 连败次数。
    attempts: u32,
    /// 失败那一笔的修订号：修订号一变（用户又改了内容）就重新给满机会。
    rev: u64,
    /// 下次可以重试的墙钟时刻。
    retry_at: std::time::Instant,
}

/// 连败这么多次就停止自动重试，等用户手动保存或下一次编辑（问题 6）。
const AUTOSAVE_GIVE_UP: u32 = 5;

/// 自动保存的退避阶梯：15s → 30s → 60s → 120s → 240s，再往上封顶 300s。
///
/// 盘满/目录只读这类失败重试再快也没用，打盘只会更糟；但完全不retry 又会在
/// 用户腾出空间后白等，所以退避 + 上限。
fn autosave_backoff(attempts: u32) -> std::time::Duration {
    let secs = 15u64 << attempts.saturating_sub(1).min(5);
    std::time::Duration::from_secs(secs.min(300))
}

/// 这个槽位现在能不能再提交自动保存（问题 6 的判据）。
///
/// · 没失败过 → 能；
/// · 修订号变了（用户又改了）→ 能，旧失败不该挡住新内容；
/// · 否则连败满 `AUTOSAVE_GIVE_UP` 次就停手，未到点也停手。
fn autosave_may_retry(stall: Option<&AutosaveStall>, rev: u64, now: std::time::Instant) -> bool {
    match stall {
        None => true,
        Some(s) if s.rev != rev => true,
        Some(s) => s.attempts < AUTOSAVE_GIVE_UP && now >= s.retry_at,
    }
}

/// 这一次编辑该不该提交自动保存（SR-02 问题 7）。
///
/// `confirmed` = **确认**写盘成功的修订号，`submitted` = 已提交还没收尾的修订号。
/// 两者分开之后，提交只记在途表、确认才记基线：任务被作废时基线不会谎报「已保存」。
fn autosave_wants(dirty: bool, rev: u64, confirmed: Option<u64>, submitted: Option<u64>) -> bool {
    dirty && confirmed != Some(rev) && submitted != Some(rev)
}

/// 写盘成功后的统一收尾（`Shell::after_saved` 的幂等判据，SR-02 问题 5）。
///
/// `after_saved()` 现在有四个调用点（普通保存、另存为、覆盖保存、历史恢复），
/// 同一份修订号被重复收尾会让 `.history` 收进两份一模一样的快照、把 `KEEP=10`
/// 的额度吃在重复内容上。所以提交前先问一句「这一笔收尾过了吗」。
fn save_finish_is_new(
    last: Option<(&std::path::Path, u64)>,
    path: &std::path::Path,
    rev: u64,
) -> bool {
    !matches!(last, Some((p, r)) if p == path && r == rev)
}

/// 卡片草稿与已存内容是否不同（退出前补写的判据，SR-02 问题 3）。
///
/// `CardReader::is_dirty` 是视图层私有的，壳层拿不到；这里用公开的 `draft_*` 与
/// 「库里 / 当前文档里那张卡」的已存字段比，等价且不用给 `note_view.rs` 加接口。
/// 标签按集合比：顺序不同不算改动，免得每次退出都白写一遍 `updated_at`。
fn draft_differs(
    baseline: Option<&(String, String, Vec<String>)>,
    title: &str,
    body: &str,
    tags: &[String],
) -> bool {
    match baseline {
        // 还没有这张卡：草稿里只要有任何内容就算有东西要落。
        None => !title.trim().is_empty() || !body.trim().is_empty() || !tags.is_empty(),
        Some((t, b, g)) => {
            let mut saved: Vec<&String> = g.iter().collect();
            let mut draft: Vec<&String> = tags.iter().collect();
            saved.sort();
            draft.sort();
            t != title || b != body || saved != draft
        }
    }
}

///
/// 启动时装载一个标签：比正本更新的恢复点优先（SR-02 问题 4）。
///
/// 返回 `(会话, 用掉的恢复点槽位)`。恢复出来的会话是**脏**的——它还没写回正本，
/// 所以 `Ctrl+S` 会把它落到原文件上，这正是崩溃恢复该有的样子。
fn restore_tab_session(path: &std::path::Path) -> Option<(Session, Option<std::path::PathBuf>)> {
    if let Some(slot) = autosave::pending_for(path) {
        if let Ok(session) = Session::open_recovered(&slot, path) {
            return Some((session, Some(slot)));
        }
    }
    Session::open(path).ok().map(|s| (s, None))
}

/// 循环切页的下标（`Ctrl+Tab` / `Ctrl+Shift+Tab`）。`delta` = ±1，越界环绕；
/// `n == 0` 时返回 0（空窗口不该有，但别 panic）。
///
/// 单独拎出来是为了能脱离 GPU/`Shell` 直接测：这段下标运算是绕圈语义的全部，
/// 包在方法里就只能靠开窗口手工试。
fn next_tab(cur: usize, delta: isize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    (cur as isize + delta).rem_euclid(n as isize) as usize
}

/// 关掉 `idx` 之后新的活动页下标。`remaining` = 关闭**之后**还剩几个标签（调用方保证 ≥1）。
///
/// 被关的在活动位左边 → 活动位左移一格；关的就是活动位 → 顺位到同下标（末页则退一格）；
/// 关的在活动位右边 → 活动位不动。
fn active_after_close(idx: usize, active: usize, remaining: usize) -> usize {
    debug_assert!(remaining >= 1);
    if idx < active {
        active - 1
    } else if idx == active {
        idx.min(remaining - 1)
    } else {
        active
    }
}

/// 启动时是否先显示开始页。显式钩子（`MW_HOME`）优先；否则「有文件要开」
/// 或快照 harness 时进文档，其余情况落在开始页。
fn start_at_home(home_hook: Option<bool>, harness: bool, opened: bool) -> bool {
    home_hook.unwrap_or(!harness && !opened)
}

/// 会替换掉当前文档的动作。有未保存改动时先挂起，等用户确认。
/// 导入不在其中：它开新标签页，当前文档原封不动。
enum PendingAction {
    Open(std::path::PathBuf),
    New,
    /// 重新加载磁盘上的版本（外部修改后从冲突提示进来，SR-02 问题 1）。
    ///
    /// 它跟 `Open` 走的是同一条数据通道（`load_open`），但提示文案不同，
    /// 而且用户以为"只是刷新一下"，所以脏检查一个都不能省。
    Reload(std::path::PathBuf),
}

/// 顶栏菜单的动作。菜单只收集意图，落地统一走 `apply_title_action`。
enum TitleMenuAction {
    New,
    /// 新建概念图（`docs/30` §3.6）。
    NewConcept,
    NewTab,
    Open,
    Import,
    Save,
    SaveAs,
    Export,
    Recent(std::path::PathBuf),
    Find,
    Palette,
    Preferences,
    Help,
    Home,
    ToggleOutline,
    ToggleInspector,
    ToggleMinimap,
    FocusMode,
    Reading,
    SetTheme(&'static str),
    Undo,
    Redo,
    AddChild,
    AddSibling,
    DeleteNode,
    Fit,
    Library,
    DailyNote,
    CaptureCard,
    TagManager,
    Quit,
}

/// 顶栏菜单绘制前一次性取好的状态。
///
/// 菜单闭包如果直接摸 `self`，会和外层「正在改 self 的 titlebar」抢借用；
/// 把提示键、最近文件、可用性拼成一份只读快照，菜单函数就只借这份快照。
struct MenuCtx {
    k_new: String,
    k_new_concept: String,
    k_new_tab: String,
    k_open: String,
    k_save: String,
    k_save_as: String,
    k_export: String,
    k_undo: String,
    k_redo: String,
    k_find: String,
    k_palette: String,
    k_prefs: String,
    k_outline: String,
    k_inspector: String,
    k_focus: String,
    k_reading: String,
    k_help: String,
    recent: Vec<(String, std::path::PathBuf)>,
    theme: Option<&'static str>,
    /// 当前画布是概念图：主题菜单只列概念图那四套（`docs/33` §4）。
    concept_map: bool,
    can_undo: bool,
    can_redo: bool,
    can_add: bool,
    can_delete: bool,
    has_canvas: bool,
    outline: bool,
    inspector: bool,
    minimap: bool,
    focus: bool,
    reading: bool,
    notes: bool,
}

/// 自绘窗口控制图标：字符字形在不同 DPI 下会跳动，统一改成矢量线。
#[derive(Clone, Copy)]
enum WinGlyph {
    Minimize,
    Maximize,
    Restore,
    Close,
}

enum ToolItem {
    Button {
        icon: ToolIcon,
        enabled: bool,
        tip: &'static str,
    },
    Sep,
}

/// 标签条上的意图。绘制时只收集，出了绘制循环再落地（和面板动作同一套纪律）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabAction {
    Switch(usize),
    Close(usize),
    CloseOthers(usize),
    NewTab,
    CopyPath(usize),
    Reveal(usize),
    /// `(来源下标, 插入槽)`，槽位 0..=len。
    Reorder(usize, usize),
}

/// 正在拖的标签。`from` 固定，插入槽每帧按指针重算。
#[derive(Clone, Copy)]
struct TabDrag {
    from: usize,
}

/// 两段式打开：先让「正在打开」完整画一帧，下一帧开头才读盘。
///
/// `painted` 由模态自己在绘制时置位；`demo` 只服务 golden，永远不真的读盘。
struct OpenPlan {
    path: std::path::PathBuf,
    painted: bool,
    demo: bool,
}

/// 打开失败的现场。`from_recent` 决定弹窗里要不要给「从最近列表移除」。
#[derive(Clone, Debug)]
struct OpenFailure {
    path: std::path::PathBuf,
    reason: String,
    from_recent: bool,
}

/// 演示控制条上的意图。
#[derive(Clone, Copy)]
enum DemoBarAction {
    Prev,
    Next,
    Exit,
}

/// 演示控制条按钮：深色卡片上的自绘小控件（透明底 + 悬停提亮），
/// 不借用全局浅色 Button 样式——那套控件放在深色幕布上会带一块脏白底。
fn demo_button(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    enabled: bool,
    tip: &str,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 28.0), sense);
    let hot = enabled && response.hovered();
    if hot {
        ui.painter()
            .rect_filled(rect, 8.0, Color32::from_white_alpha(22));
    }
    let color = if !enabled {
        Color32::from_gray(92)
    } else if hot {
        Color32::WHITE
    } else {
        Color32::from_rgb(0xE6, 0xEA, 0xF0)
    };
    let size = if label.chars().count() > 1 {
        Type::BODY
    } else {
        Glyph::LARGE
    };
    // 翻页箭头走统一图标几何：字符「‹ ›」在不同字体里基线和粗细都漂。
    if label == "›" || label == "‹" {
        let icon = if label == "›" {
            Icon::ChevronRight
        } else {
            Icon::ChevronLeft
        };
        crate::icon_paint::paint_icon_sized(
            ui.painter(),
            rect.center(),
            Glyph::MEDIUM,
            icon,
            color,
        );
    } else {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(size),
            color,
        );
    }
    response.on_hover_text(tip)
}

struct Toast {
    text: String,
    /// 消失时刻。**用 `Instant` 而不是 egui 的 `input.time`**：原生文件对话框会阻塞
    /// UI 线程好几秒，恢复后的那一帧时间会突然前跳，按帧时间算的到期点在提示还没画出来
    /// 之前就已经过期了——「已导出 x.png」就这么一次也看不见（docs/14 教训 38）。
    until: Option<std::time::Instant>,
    bad: bool,
}

enum Backdrop {
    Pending,
    Applied { corner: Result<(), String> },
    Failed(String),
}

/// 真实宿主送来的输入事件。金样 harness 一律丢掉，只认脚本注入的那一份：
/// 离屏窗口不可见，却仍可能收到指针进出、真实键盘与输入法事件。
fn is_host_input(e: &egui::Event) -> bool {
    matches!(
        e,
        egui::Event::Copy
            | egui::Event::Cut
            | egui::Event::Paste(_)
            | egui::Event::PasteImage(_)
            | egui::Event::MiddleClickPaste { .. }
            | egui::Event::Text(_)
            | egui::Event::Key { .. }
            | egui::Event::ModifiersChanged(_)
            | egui::Event::PointerMoved(_)
            | egui::Event::MouseMoved(_)
            | egui::Event::PointerButton { .. }
            | egui::Event::PointerGone
            | egui::Event::Zoom(_)
            | egui::Event::Rotate(_)
            | egui::Event::Ime(_)
            | egui::Event::Touch { .. }
            | egui::Event::MouseWheel { .. }
            | egui::Event::WindowFocused(_)
            | egui::Event::AccessKitActionRequest(_)
    )
}

impl eframe::App for Shell {
    /// 全透明清除：Mica 由 DWM 画在窗口背后，只有 alpha=0 的像素才透得出来。
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    /// 金样脚本在 egui 处理输入之前入场：键盘、IME、指针都落在本进程内，
    /// 与桌面焦点、输入法状态无关。
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // 离屏 harness 的窗口不可见，DWM/winit 不一定给 focus 事件；而 TextEdit
        // 只在 `input.focused` 为真时画光标，焦点状态抖动会让同一份构建截出
        // 「有光标/没光标」两张图。harness 里强制有焦点，基线才是确定的。
        if self.harness.is_some() {
            raw_input.focused = true;
            // 隐藏窗口仍会收到桌面上的指针进出、真实键入与输入法组合：同一份构建
            // 因此能截出「悬停被 PointerGone 抹掉」「标题里多了个字符」两张图。
            // 金样的输入只能来自脚本，先把宿主事件全部清掉。
            raw_input.events.retain(|e| !is_host_input(e));
            // 时间也归金样管：每帧固定 1/60 秒，`sleep` 到期一次补名义时长。
            // 悬停 120ms 露握把、折叠行 500ms 展开、滚动落定都按帧数走。
            let step = match self.golden_input.as_mut() {
                Some(input) => input.advance_clock(),
                None => 1.0 / 60.0,
            };
            self.golden_clock += step;
            raw_input.time = Some(self.golden_clock);
            raw_input.predicted_dt = 1.0 / 60.0;
            // 指针动作全归脚本管的场景（hover 计时要攒时间）不能每帧清，否则
            // 握把/悬停态永远出不来；纯键盘场景仍要把桌面光标挪到画外。
            let script_owns_pointer = self
                .golden_input
                .as_ref()
                .is_some_and(|input| input.owns_pointer());
            if self.harness.as_ref().is_some_and(Harness::offscreen) && !script_owns_pointer {
                raw_input
                    .events
                    .push(egui::Event::PointerMoved(egui::pos2(-10_000.0, -10_000.0)));
                raw_input.events.push(egui::Event::PointerGone);
            }
        }
        // `MW_OUTLINE_SELECT`：首帧才查节点——`Shell::new` 里画布还没建好，
        // `shell.canvas` 是 `None`，按文字找不到行，键位永远落在首行。
        if self.outline_pin.is_none() && !self.outline_pin_done {
            if let Ok(needle) = std::env::var("MW_OUTLINE_SELECT") {
                if let Some(canvas) = self.canvas.as_ref() {
                    self.outline_pin = canvas
                        .session()
                        .doc
                        .nodes
                        .values()
                        .find(|n| n.text == needle)
                        .map(|n| n.id.clone());
                    if self.outline_pin.is_some() {
                        self.outline_pin_left = 4;
                        self.outline_pin_done = true;
                    }
                }
            }
        }
        // 验收钩子 `MW_OUTLINE_SELECT`：每帧把选中与按键焦点按回目标行。
        // 首帧的「选中为空就选根」会覆盖建窗口时那次 select，钉在这里才稳；
        // 行内编辑或短语框一立起来就撒手，否则打字会连框一起被抹掉。
        if self.outline.is_editing() || self.outline.is_via_editing() {
            self.outline.clear_pin();
        }
        if let Some(pin) = self.outline_pin.clone() {
            if self.outline_pin_left > 0
                && !self.outline.is_editing()
                && !self.outline.is_via_editing()
            {
                self.outline_pin_left -= 1;
                if let Some(canvas) = self.canvas.as_mut() {
                    // 用大纲自己的选中入口（`select_many`）：`select_node` 只动画布那份
                    // 选择，面板每帧还会按「主选中」把焦点拉回去，钉不住。
                    canvas.select_many(std::slice::from_ref(&pin), &pin);
                }
                self.outline.force_row_focus(pin);
            }
        }
        if let Some(input) = self.golden_input.as_mut() {
            input.apply(raw_input);
        }
    }

    /// 正常关窗的最后一次落盘。移动后 700ms 内就关窗的话，靠这里补上。
    fn on_exit(&mut self) {
        self.window_state.set_inspector_width(self.inspector_w);
        self.save_window_state_now();
        // 导出线程正写到一半就退出会留下 `.part`；成品路径不会被半份文件顶替。
        // 2× 5k 的导出通常 1s 上下，4s 足够收尾。
        if let Some(job) = self.export_job.as_mut() {
            job.settle(std::time::Duration::from_secs(4));
        }
        // 导入线程只读文件；取消后最多等半秒，别让关窗卡在解析上。
        if let Some(job) = self.import_job.as_mut() {
            job.cancel();
            job.settle(std::time::Duration::from_millis(500));
        }
        // 卡片页的去抖是 800ms：退出前把还压着的草稿落一次，别丢最后半句话。
        self.flush_card_draft_on_exit();
        // 每个脏标签补一份恢复点。以前这里什么都不写，崩溃保护对「改完直接关窗」
        // 完全不起作用（SR-02 问题 3）。
        self.flush_recovery_points_on_exit();
        // 分离线程不会拖住进程退出；最后一份恢复点值得等这几十毫秒。
        self.autosave.settle(std::time::Duration::from_millis(1500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let frame_start = std::time::Instant::now();
        // 验收钩子 `MW_SETTINGS_AFTER`：先让脚本在画布上堆够历史，再弹设置中心。
        if let Some(left) = self.settings_open_after.as_mut() {
            if *left == 0 {
                self.settings_open = true;
                self.settings_open_after = None;
            } else {
                *left -= 1;
            }
        }
        if !self.perf_saw_ui {
            self.perf_saw_ui = true;
            self.perf.mark("first_ui");
        }
        let ctx = ui.ctx().clone();
        // 快照的 `--ppp` 要在本帧布局之前生效：输入脚本运行时截图状态机不推进，
        // 等 `tick` 才设会让脚本按键按旧缩放取景（见 `Harness::prepare`）。
        if let Some(harness) = self.harness.as_ref() {
            harness.prepare(&ctx);
        }
        if self.hwnd.is_none() {
            self.hwnd = window_hwnd(frame);
        }
        // 恢复要排在默认尺寸之前：恢复成功会置 `sized_to_monitor`，默认居中块随即跳过。
        self.restore_window_state(&ctx);
        if !self.sized_to_monitor && self.harness.is_none() {
            if let Some(mon) = ctx.input(|i| i.viewport().monitor_size) {
                self.sized_to_monitor = true;
                // 屏幕点数是逻辑像素；150% 缩放下 1080×720 的默认窗口会顶出屏幕。
                // 工作区（物理）换算成逻辑点后居中，任务栏那一条不会被压住。
                let ppp = ctx.pixels_per_point();
                let work = primary_work_area()
                    .map(|(x, y, w, h)| (x / ppp, y / ppp, w / ppp, h / ppp))
                    .unwrap_or((0.0, 0.0, mon.x, mon.y - 48.0));
                let want = egui::vec2(
                    (work.2 - 16.0).clamp(640.0, 1080.0),
                    (work.3 - 16.0).clamp(460.0, 720.0),
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(want));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    work.0 + (work.2 - want.x) * 0.5,
                    work.1 + (work.3 - want.y) * 0.5,
                )));
            }
        }
        self.sync_accent(&ctx, ui);
        self.sync_chrome(&ctx);
        self.sync_window_title(&ctx);
        self.sync_history_limit();
        // `chrome`/`tone` 是 Copy：先取一份，后面各处闭包可变借 `self` 时不再冲突。
        let chrome = self.chrome;
        let tone = self.tone;

        if matches!(self.backdrop, Backdrop::Pending) {
            self.backdrop = apply_backdrop(frame);
        }
        self.geometry_tick();
        // 启动的头几十帧窗口尺寸/面板宽度还没落定：**视口一变就重取一次景**，落定的那一帧
        // 之后就不动了。为什么不是「每帧都重取」：窗口可能在第 2 帧也可能在第 8 帧落定，
        // 每帧重取会让结果取决于落定时刻；视口一变才重取，落定在第几帧都是同一张图。
        if self.start_refit_frames > 0 {
            self.start_refit_frames -= 1;
            if let Some(c) = self.canvas.as_mut() {
                match c.fit_viewport() {
                    // 还没取过景：先取一次（首帧视口通常就是没落定的那个）。
                    None => c.request_fit(),
                    Some(vp) if vp != c.viewport() => c.request_fit(),
                    _ => {}
                }
            }
        }
        // 最近文件是异步读的（起始页首帧画 3 行骨架）：到了就整份换上，骨架切真行。
        if let Some(rx) = self.recent_load.as_ref() {
            if let Ok(mut loaded) = rx.try_recv() {
                loaded.mark_missing();
                self.recent = loaded;
                self.recent_load = None;
                crate::trace::event(&format!("recents loaded n={}", self.recent.entries().len()));
            }
        }
        if self.recents_pending_frames > 0 {
            self.recents_pending_frames -= 1;
            if self.recents_pending_frames == 0 {
                crate::trace::event(&format!("recents loaded n={}", self.recent.entries().len()));
            }
        }
        if let Some(until) = self.refit_until {
            if std::time::Instant::now() < until {
                if let Some(c) = self.canvas.as_mut() {
                    c.request_fit();
                }
            } else {
                self.refit_until = None;
            }
        }

        // 上一帧已经把「保存中…」画出来了，这一帧开头才真正写盘：
        // UI 线程不会被同步 IO 卡住绘制，失败也能落到标题芯片而不是一闪而过的 toast。
        self.pump_save();
        self.pump_card_save();
        self.pump_open();
        self.guard_close(&ctx);
        // 弹窗优先：有模态时 Esc 先归它，页面/菜单/画布这一帧都看不到同一下 Esc。
        // 改键捕获态例外：那一下 Esc 是「取消本次改键」，由 capture 自己读。
        let overlay_now = self.overlay_state();
        let modal_now = overlay_now.top_modal();
        let capturing =
            modal_now == Some(overlay::Modal::Shortcuts) && self.shortcuts_dialog.is_capturing();
        // 演示弹窗不在浮层决策表里（它只服务 golden/脚本），但同样拥有键盘。
        dialog::route_escape(
            &ctx,
            (modal_now.is_some() && !capturing) || self.dialog_demo,
        );
        if capturing {
            self.shortcuts_dialog.capture(&ctx, &mut self.shortcuts);
        } else if modal_now.is_none() && !self.dialog_demo {
            self.window_shortcuts(&ctx);
        }
        // 弹窗在上时，背景的菜单/搜索/帮助/命令面板一律不读键盘：它们照旧画在暗幕后面，
        // 但同一帧的 ↑↓/Enter 只能归栈顶弹窗，否则会在确认框背后执行一条命令。
        let background_keys = modal_now.is_none() && !self.dialog_demo;
        // 右键菜单的 ↑↓/Enter/Esc 必须早于画布：否则画布会把 Esc 当成清空选择。
        let menu_action = background_keys
            .then(|| {
                self.context_menu
                    .as_mut()
                    .and_then(|menu| menu.handle_keys(&ctx))
            })
            .flatten();
        if self
            .context_menu
            .as_ref()
            .is_some_and(|menu| !menu.is_open())
        {
            self.context_menu = None;
        }
        if let Some(action) = menu_action {
            self.run_context_action(&ctx, action);
        }
        // 标签页右键菜单与画布菜单同一套引擎：↑↓/Enter/Esc 同样必须先于画布。
        let tab_menu_action = background_keys
            .then(|| {
                self.tab_menu
                    .as_mut()
                    .and_then(|menu| menu.handle_keys(&ctx))
            })
            .flatten();
        if self.tab_menu.as_ref().is_some_and(|menu| !menu.is_open()) {
            self.tab_menu = None;
        }
        if let Some(action) = tab_menu_action {
            self.run_tab_action(&ctx, action);
        }
        // 大纲行菜单：与右键菜单同一套 ↑↓/Enter/Esc，必须先于画布与行表。
        let row_menu_hit = background_keys
            .then(|| {
                if !self.outline_view {
                    return None;
                }
                let row = self.outline_row_menu.as_mut()?;
                row.menu
                    .handle_keys(&ctx)
                    .map(|action| (action, row.target.clone(), row.multi))
            })
            .flatten();
        if self
            .outline_row_menu
            .as_ref()
            .is_some_and(|row| !row.menu.is_open())
        {
            self.outline_row_menu = None;
        }
        if let Some((action, target, multi)) = row_menu_hit {
            self.run_outline_row_action(&ctx, action, &target, multi);
        }
        // 搜索条的 Esc / Enter / ↑↓ 必须先于画布消费，否则会漏成「加同级主题」。
        if background_keys {
            if let Some(action) = self.search.handle_keys(&ctx) {
                self.run_search(action);
            }
        }
        if background_keys && self.help.handle_keys(&ctx) {
            if self.outline_view {
                // 大纲不在画布上：关掉帮助后焦点回当前行，而不是把键盘交给隐藏的画布。
                if let Some(id) = self.canvas.as_ref().and_then(|c| c.primary_id()) {
                    self.outline.request_row_focus(id);
                }
            } else if let Some(canvas) = self.canvas.as_mut() {
                canvas.request_canvas_focus();
            }
        }
        self.handle_dropped(&ctx);
        self.autosave_tick(&ctx);
        self.import_tick(&ctx);
        // 命令面板要**先于画布**处理：它的 Esc / ↑↓ / Enter 不能被画布的快捷键吃掉。
        if let Some(action) = self
            .palette
            .show(&ctx, tone, &self.shortcuts, background_keys)
        {
            self.run_palette(action);
        }
        self.help.show(&ctx, tone);
        // 演示状态的权威在画布；这一帧先取一次，后面面板/标题栏/输入门禁都用同一个值。
        let demo = self.demo_active();
        // 标题栏菜单是 egui Memory 弹层，不进 overlay 状态，但同样独占指针：点菜单
        // 外面第一下只收起菜单，不能顺手选中/框选/缩放画布（同 ⑨ 的标签条口径）。
        // 读的是上一帧的弹层状态，而画布在本帧稍后才绘制——收起菜单的那一下正好
        // 被挡住；tooltip 不写 Memory 弹层，不受影响。
        let menu_popup = egui::Popup::is_any_open(&ctx);
        let input_blocked = demo
            || self.dialog_demo
            || overlay_now.blocks_canvas()
            || self.tab_drag.is_some()
            || menu_popup;
        if let Some(canvas) = self.canvas.as_mut() {
            // 导出对话框和命令面板/搜索条一样是**浮层**：它开着的时候画布不该再收键盘，
            // 否则回车既确认导出、又在画布上加了同级主题。
            // 演示态同样整块锁死：键盘归 `Shell::demo_shortcuts`，指针不该改选择或拖节点。
            canvas.set_input_blocked(input_blocked);
        }

        let maximized =
            ctx.input(|i| i.viewport().maximized.unwrap_or(false)) || self.focus_mode.is_some();
        // harness 必须与「谁在前台」无关：隐藏窗口的 `viewport().focused` 取决于截图那一刻
        // 系统前台归属，标题栏文字 dim 会在 1.0/0.55 之间跳，基线因此随机漂移。
        let focused = self.harness.is_some() || ctx.input(|i| i.viewport().focused.unwrap_or(true));
        // 专注模式靠置顶盖任务栏；失焦时撤掉，否则 Alt+Tab 切到别的窗口会被自己压住。
        if self.focus_mode.is_some() && focused != self.focus_prev_focused {
            Shell::set_topmost(self.hwnd, focused);
        }
        if self.focus_mode.is_some() {
            self.focus_prev_focused = focused;
        }
        let win = ui.max_rect();

        // 演示 = 幕布：标题栏、标签条、状态栏、两块面板全部收起，只留画布与控制条。
        if !demo {
            egui::Panel::top("titlebar")
                .exact_size(TITLEBAR_H)
                .frame(egui::Frame::NONE.fill(chrome.titlebar))
                .show_separator_line(false)
                .show(ui, |ui| {
                    self.titlebar(ui, &ctx, maximized, focused);
                });
        }

        // 标签条紧贴标题栏下缘，只在多标签时出现（单标签视图与旧版逐像素一致）。
        // 开始页盖住文档时不画标签条：它属于"文档视图"，不属于开始页。
        if !demo && !self.home && self.tabs.len() > 1 {
            egui::Panel::top("tabbar")
                .exact_size(TABBAR_H)
                .frame(egui::Frame::NONE.fill(chrome.panel))
                .show_separator_line(false)
                .show(ui, |ui| {
                    self.tab_bar(ui);
                    let r = ui.max_rect();
                    ui.painter()
                        .hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, chrome.sep));
                });
        } else {
            // 标签条被藏起来时清掉记忆：下次出现（含从开始页返回）重新揭示活动标签。
            self.tab_seen = None;
        }

        // 大纲视图自带 28px 页脚状态；再叠一条全局状态栏就是两条重复的底边。
        if !demo && !self.outline_view {
            egui::Panel::bottom("status")
                .exact_size(STATUS_H)
                .resizable(false)
                .frame(egui::Frame::NONE.fill(chrome.panel))
                .show(ui, |ui| {
                    self.status_bar(ui);
                    let r = ui.max_rect();
                    ui.painter()
                        .hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, chrome.sep));
                });
        }

        // H3：开合状态的判定放在绘制前，焦点恢复放在绘制后。
        // 顺序不能反：这一帧到底画出了哪些键，只有画完才知道；拿上一帧的键去
        // `request_focus`，目标已被文档变化删掉时会被 egui 的死人开关清掉（5.3.3.a 落空）。
        // 演示模式 / 开始页盖住文档时面板根本没画，这时候只同步开合状态、不请求焦点。
        let panels_visible = !demo && !self.home;
        let outline_opened = self.focus_mem.set_open(
            panels::PanelId::Outline,
            panels_visible && self.outline_view,
        );
        // 全屏大纲不画右栏：整块窗口只留一列文字，检查器与画布一起退场。
        let inspector_opened = self.focus_mem.set_open(
            panels::PanelId::Inspector,
            panels_visible && self.show_inspector && !self.outline_view,
        );

        // 大纲不再是左侧抽屉：它整块接管中央面板，画布一个像素都不会被挤。
        if !demo && !self.home && self.outline_view {
            self.focus_mem.set_open(panels::PanelId::Outline, true);
        }

        let inspector_t = self.panel_motion(&ctx, "mw-inspector-anim", self.show_inspector);
        if !demo && !self.home && !self.outline_view && (self.show_inspector || inspector_t > 0.005)
        {
            if let Some(action) = self.inspector_panel(ui, inspector_t) {
                use panels::inspector::Action as IA;
                match action {
                    IA::Apply(cmd) => {
                        // §4.3：同一批 id 的样式编辑在 500ms 内并成一条撤销——
                        // 连点 stepper 或拖滑杆时用户心里是「调了一次」，不该按十次撤销。
                        let key = panels::inspector::style_gesture_key(&cmd);
                        let merge = match (&key, self.style_gesture.as_ref()) {
                            (Some(k), Some((prev, at))) => panels::inspector::same_gesture(
                                Some(prev.as_str()),
                                at.elapsed().as_millis(),
                                k,
                            ),
                            _ => false,
                        };
                        let applied = self.canvas.as_mut().is_some_and(|canvas| {
                            let ok = if merge {
                                canvas.apply_command_coalesced(cmd, |_| true)
                            } else {
                                canvas.apply_command(cmd)
                            };
                            if ok {
                                // 控件（按钮/滑杆）会拿走焦点；改完把键盘还给画布，
                                // 否则 Delete/方向键要等用户再点一下画布才恢复。
                                canvas.request_canvas_focus();
                            }
                            ok
                        });
                        if applied {
                            // 不是样式命令（换主题、写备注…）就把窗口关掉，免得被后面的编辑粘上。
                            self.style_gesture = key.map(|k| (k, std::time::Instant::now()));
                            self.inspector.clear_error();
                        } else {
                            // §4.3：命令失败不能只在底部 toast 一闪——面板里也要留一条，
                            // 用户盯着面板改东西，眼睛不在屏幕底下。
                            self.inspector
                                .report_error("修改未生效：这条命令被文档拒绝了");
                            self.set_toast("修改未生效", true);
                        }
                    }
                    IA::OpenLink(url) => {
                        if let Err(e) = files::open_url(&url) {
                            self.set_toast(e, true);
                        } else if let Some(canvas) = self.canvas.as_mut() {
                            canvas.request_canvas_focus();
                        }
                    }
                    IA::Close => self.toggle_inspector(),
                    IA::ToggleGroup { key, open } => {
                        self.window_state.set_inspector_group(&key, open);
                        crate::trace::event(&format!("group key={key} open={}", open as u8));
                    }
                    IA::ArrangeConcepts => {
                        let arranged = self.canvas.as_mut().and_then(|canvas| {
                            let n = canvas.arrange_concepts();
                            canvas.request_canvas_focus();
                            n
                        });
                        if let Some(n) = arranged {
                            self.set_toast(format!("已整理 {n} 颗概念 · Ctrl+Z 撤销"), false);
                        }
                    }
                    IA::ArrangeSelectedConcepts => {
                        let arranged = self.canvas.as_mut().and_then(|canvas| {
                            let n = canvas.arrange_selected_concepts();
                            canvas.request_canvas_focus();
                            n
                        });
                        if let Some(n) = arranged {
                            self.set_toast(format!("已整理 {n} 颗概念 · Ctrl+Z 撤销"), false);
                        }
                    }
                    IA::ClearAnnotation => {
                        if let Some(canvas) = self.canvas.as_mut() {
                            canvas.clear_annotation_selection();
                        }
                    }
                    IA::OpenCard(id) => {
                        let card = self
                            .canvas
                            .as_ref()
                            .and_then(|c| c.session().doc.card(&id))
                            .cloned();
                        match card {
                            Some(card) => {
                                let reader = self.card_reader_for(&card);
                                self.close_light_overlays();
                                self.library = None;
                                self.reader_from_library = false;
                                self.note_reader = Some(reader);
                            }
                            None => self.set_toast("这张卡片已经不存在", true),
                        }
                    }
                    IA::NewCardForNode(node) => {
                        let applied = {
                            let Some(canvas) = self.canvas.as_mut() else {
                                return;
                            };
                            let doc_id = canvas.session().doc.id.clone();
                            let title = canvas
                                .node_text(&node)
                                .map(str::trim)
                                .filter(|t| !t.is_empty())
                                .unwrap_or("卡片笔记")
                                .to_string();
                            let card =
                                mw_core::NoteCard::new(doc_id, None, title, "", mw_core::now_ms());
                            let ok = canvas
                                .apply_command(mw_core::Command::AttachNewCard { node, card });
                            if ok {
                                canvas.request_canvas_focus();
                            }
                            ok
                        };
                        if applied {
                            self.sync_notes();
                        } else {
                            self.set_toast("新建卡片失败", true);
                        }
                    }
                    IA::DetachCard(node) => {
                        let applied = {
                            let Some(canvas) = self.canvas.as_mut() else {
                                return;
                            };
                            let ok = canvas
                                .apply_command(mw_core::Command::SetNodeCard { node, card: None });
                            if ok {
                                canvas.request_canvas_focus();
                            }
                            ok
                        };
                        if applied {
                            self.sync_notes();
                        }
                    }
                    IA::LinkCard { node, card } => {
                        let applied = {
                            let Some(canvas) = self.canvas.as_mut() else {
                                return;
                            };
                            let ok = canvas.apply_command(mw_core::Command::SetNodeCard {
                                node,
                                card: Some(card),
                            });
                            if ok {
                                canvas.request_canvas_focus();
                            }
                            ok
                        };
                        if applied {
                            self.sync_notes();
                        } else {
                            self.set_toast("这张卡片已经挂在别的主题上了", true);
                        }
                    }
                    IA::SelectNode(id) => {
                        if let Some(canvas) = self.canvas.as_mut() {
                            canvas.select_node(&id);
                        }
                    }
                    IA::SelectRelationship { from, id } => {
                        if let Some(canvas) = self.canvas.as_mut() {
                            if canvas.select_relationship(&from, &id) {
                                canvas.request_canvas_focus();
                            }
                        }
                    }
                    IA::ClearRelationship => {
                        if let Some(canvas) = self.canvas.as_mut() {
                            canvas.clear_relationship_selection();
                        }
                    }
                    IA::FocusCanvas => {
                        if let Some(canvas) = self.canvas.as_mut() {
                            canvas.request_canvas_focus();
                        }
                    }
                    IA::PickImage => self.pick_image_for_primary(),
                    IA::ClearImage => {
                        let id = self
                            .canvas
                            .as_ref()
                            .and_then(|c| c.selected_ids().last().cloned());
                        if let (Some(id), Some(canvas)) = (id, self.canvas.as_mut()) {
                            if canvas.apply_command(mw_core::Command::SetImage { id, image: None })
                            {
                                self.set_toast("已移除图片", false);
                            }
                        }
                    }
                    IA::SavePreset => {
                        let Some(canvas) = self.canvas.as_ref() else {
                            return;
                        };
                        let meta = &canvas.session().doc.meta;
                        let base = mw_core::theme::get_theme(&meta.theme);
                        let name = theme_presets::Preset::sanitize_name(&format!(
                            "{} {}",
                            base.name,
                            self.presets.len() + 1
                        ))
                        .unwrap_or_else(|| "我的主题".into());
                        self.presets.add(theme_presets::Preset {
                            name,
                            theme: meta.theme.clone(),
                            overrides: meta.theme_overrides.clone(),
                            accent: meta.theme_accent.clone(),
                        });
                        self.presets.save();
                        self.set_toast("已存为我的主题", false);
                    }
                    IA::ApplyPreset(index) => {
                        let Some(preset) = self.presets.get(index).cloned() else {
                            return;
                        };
                        // 一条 Batch：换主题 + 微调 + 强调色，撤销一次全回去。
                        let cmd = theme_presets::apply_command(&preset);
                        let applied = self.canvas.as_mut().is_some_and(|canvas| {
                            let ok = canvas.apply_command(cmd);
                            if ok {
                                canvas.request_canvas_focus();
                            }
                            ok
                        });
                        if !applied {
                            self.set_toast("预设未能套用", true);
                        }
                    }
                    IA::DeletePreset(index) => {
                        if self.presets.remove(index) {
                            self.presets.save();
                            self.set_toast("已删除我的主题", false);
                        }
                    }
                }
            }
        }

        // H3：绘制完再恢复——此刻 `known` 已是本帧真实存在的键。
        // 轨迹里补一行 `event restore <键>`：光看帧末焦点行分不清「恰好又是它」和
        // 「真的从记忆里取回来的」，探针要核对的就是这条因果链（5.3.1.a）。
        if outline_opened {
            if crate::trace::is_enabled() {
                if let Some(k) = self.focus_mem.last(panels::PanelId::Outline) {
                    crate::trace::event(&format!("restore {}", k.trace_name()));
                }
            }
            self.focus_mem
                .restore(panels::PanelId::Outline, &ctx, panels::outline::anchor());
        }
        if inspector_opened {
            if crate::trace::is_enabled() {
                if let Some(k) = self.focus_mem.last(panels::PanelId::Inspector) {
                    crate::trace::event(&format!("restore {}", k.trace_name()));
                }
            }
            self.focus_mem.restore(
                panels::PanelId::Inspector,
                &ctx,
                panels::inspector::anchor(),
            );
        }

        let mut search_action = None;
        let mut home_action = None;
        let mut context_request = None;
        let mut card_open = None;
        let mut canvas_notice = None;
        let mut toolbar_card = false;
        egui::CentralPanel::no_frame().show(ui, |ui| {
            // CER-02 组件陈列页（MW_CONTENT_KIT=1）：只服务 content-kit golden
            // 与人工复核，放在一切路由之前，与产品页面零耦合。
            if self.content_kit_demo {
                content_kit::show_gallery(ui);
                return;
            }
            if self.home {
                home_action = self.start.show(
                    ui,
                    self.recent.entries(),
                    self.recents_pending(),
                    self.home_return,
                    mw_core::now_ms(),
                    tone,
                );
                return;
            }
            // 演示：中央面板只铺画布——缩放控件、面包屑、搜索条都不该出现在幕布上。
            if demo {
                if let (Some(canvas), Some(render_state)) =
                    (self.canvas.as_mut(), frame.wgpu_render_state())
                {
                    canvas.show(ui, render_state);
                }
                return;
            }
            // 全屏大纲：画布这一帧完全不画，也不登记任何画布手势。
            if self.outline_view {
                if let Some(action) = self.outline_screen(ui, input_blocked) {
                    self.apply_outline_action(action);
                }
                return;
            }
            // 卡片库同样铺满全窗且自身不透明：不画布既省一帧绘制，
            // 也避免离屏回读时画布以底图形式叠到卡片库上。
            if self.library.is_some() {
                return;
            }
            let central = ui.max_rect();
            match (self.canvas.as_mut(), frame.wgpu_render_state()) {
                (Some(canvas), Some(render_state)) => {
                    let show_zoom = central.width() > 320.0 && central.height() > 220.0;
                    // 浮层矩形必须在画布之前登记：缩放控件、面包屑与上下文工具条上的
                    // 点击不能再穿透成画布手势。
                    let path = canvas.focus_path();
                    let crumbs = breadcrumb_layout(ui, central, &path);
                    // 工具条跟着选区出现；行内编辑、阅读模式与任何浮层打开时让位。
                    let toolbar = if !input_blocked
                        && self.reading.is_none()
                        && !canvas.is_editing()
                        && canvas.has_selection()
                    {
                        canvas_chrome::toolbar_layout(central, crumbs.as_ref().map(|b| b.bar))
                    } else {
                        None
                    };
                    let toolbar_state = toolbar.map(|_| canvas_chrome::ToolbarState {
                        undo: canvas.can_undo(),
                        redo: canvas.can_redo(),
                        add: true,
                        delete: canvas.can_delete(),
                        card: canvas.selected_count() == 1,
                    });
                    let mut gates = Vec::new();
                    if show_zoom {
                        gates.push(zoom_control_rect(central));
                    }
                    // 缩略卡（M30）的矩形必须在画布之前登记（与缩放条同一道指针闸门）。
                    let minimap = if self.show_minimap && show_zoom {
                        canvas_chrome::minimap_layout(central, self.minimap_collapsed)
                    } else {
                        None
                    };
                    if let Some(m) = &minimap {
                        gates.push(m.card);
                    }
                    if let Some(bar) = &crumbs {
                        gates.push(bar.bar);
                    }
                    if let Some(tb) = &toolbar {
                        gates.push(tb.bar);
                    }
                    canvas.set_overlays(gates);
                    canvas.show(ui, render_state);
                    // 概念图验收钩子（CM-08）：**放在 show 之后**、并**等布局落定**——
                    // 短语编辑框要落在线段中点，中点要等 `last_viewport` / `last_rect`
                    // 填好，而且第一帧那份矩形还没算上菜单栏占位（实测偏 215/115px）。
                    // 只在环境变量开着时有一次值，跑完即 none，不重复进编辑。
                    if self.concept_hook.is_some() {
                        if self.concept_hook_after > 0 {
                            self.concept_hook_after -= 1;
                            ui.ctx().request_repaint();
                        } else if let Some(hook) = self.concept_hook.take() {
                            let ran = match hook {
                                canvas::ConceptHook::Edit(text) => canvas.start_concept_edit(&text),
                                canvas::ConceptHook::Link(from, to) => {
                                    canvas.create_concept_link(&from, &to)
                                }
                                canvas::ConceptHook::Phrase(label) => {
                                    canvas.edit_concept_phrase(&label)
                                }
                                canvas::ConceptHook::LinkBlank(from, dx, dy) => {
                                    canvas.create_concept_link_blank(&from, dx, dy)
                                }
                                canvas::ConceptHook::LinkDrag(from, dx, dy) => {
                                    canvas.begin_concept_link_drag(&from, dx, dy)
                                }
                                canvas::ConceptHook::RelinkBlank(label, dx, dy) => {
                                    canvas.retarget_phrase_to_blank(&label, dx, dy)
                                }
                                canvas::ConceptHook::RelinkDrag(label, dx, dy) => {
                                    canvas.begin_relink_drag_label(&label, dx, dy)
                                }
                                canvas::ConceptHook::MultiSelect(text) => {
                                    canvas.add_two_annotations_and_select(&text)
                                }
                                canvas::ConceptHook::MultiSelectOff(text) => {
                                    canvas.add_two_annotations_and_deselect(&text)
                                }
                                canvas::ConceptHook::SpanCommit(text) => {
                                    canvas.add_span_commit_demo(&text)
                                }
                                canvas::ConceptHook::SpanDrag(text) => {
                                    canvas.add_span_drag_demo(&text)
                                }
                                canvas::ConceptHook::BoundarySelect => {
                                    canvas.add_boundary_and_select()
                                }
                                canvas::ConceptHook::SummarySelect(text) => {
                                    canvas.add_summary_and_select(&text)
                                }
                                canvas::ConceptHook::SelectRel(label) => {
                                    canvas.select_relationship_by_label(&label)
                                }
                                canvas::ConceptHook::LinkMode(text) => {
                                    canvas.begin_concept_link_mode(&text)
                                }
                                canvas::ConceptHook::LinkTwo(a, b) => {
                                    canvas.connect_concepts_by_text(&a, &b)
                                }
                                canvas::ConceptHook::Reverse(label) => {
                                    canvas.reverse_relationship_by_label(&label)
                                }
                                canvas::ConceptHook::Suggest(label, typed) => {
                                    canvas.edit_concept_phrase_typed(&label, &typed)
                                }
                                canvas::ConceptHook::RelStyle(label) => {
                                    canvas.style_relationship_by_label(&label)
                                }
                                canvas::ConceptHook::MultiRel(labels) => {
                                    let refs: Vec<&str> =
                                        labels.iter().map(|s| s.as_str()).collect();
                                    canvas.select_relationships_by_labels(&refs)
                                }
                                canvas::ConceptHook::Group(members) => {
                                    let refs: Vec<&str> =
                                        members.iter().map(|s| s.as_str()).collect();
                                    canvas.create_group_by_text(&refs)
                                }
                            };
                            crate::trace::event(&format!("concept-hook ran={ran}"));
                        }
                    }
                    if !self.perf_saw_canvas {
                        self.perf_saw_canvas = true;
                        self.perf.mark("first_canvas");
                    }
                    if let (Some(tb), Some(st)) = (&toolbar, toolbar_state) {
                        if let Some(action) = canvas_chrome::toolbar(ui, chrome, *tb, st) {
                            use canvas_chrome::ToolbarAction as A;
                            match action {
                                A::Undo => {
                                    canvas.undo();
                                }
                                A::Redo => {
                                    canvas.redo();
                                }
                                A::AddChild => {
                                    canvas.add_child();
                                }
                                A::AddSibling => {
                                    canvas.add_sibling();
                                }
                                A::Delete => {
                                    canvas.delete_selected();
                                }
                                A::Card => toolbar_card = true,
                                A::More => canvas.request_node_context_at(
                                    tb.more.left_bottom()
                                        + Vec2::new(0.0, mw_core::design::space::XS),
                                ),
                            }
                        }
                    }
                    if let Some(req) = canvas.take_context_request() {
                        context_request = Some(req);
                    }
                    card_open = canvas.take_card_open();
                    canvas_notice = canvas.take_notice();
                    if let Some(bar) = &crumbs {
                        if let Some(depth) = paint_breadcrumbs(ui, bar, chrome) {
                            if canvas.focus_to(depth) {
                                canvas.request_canvas_focus();
                            }
                        }
                    }
                    if show_zoom {
                        zoom_control(ui, canvas, central, chrome);
                    }
                    if minimap.is_some() {
                        self.minimap_collapsed =
                            minimap_panel(ui, canvas, central, chrome, self.minimap_collapsed);
                    }
                    if self.search.is_open() {
                        let revision = canvas.session().doc.revision;
                        let notes = self.notes.as_mut();
                        search_action = self.search.show(ui, central, revision, tone, |q, mode| {
                            use panels::search::{Group, HitRow};
                            let mut rows: Vec<HitRow> = canvas
                                .search_nodes(q, mode)
                                .into_iter()
                                .map(|hit| {
                                    let title = canvas
                                        .node_text(&hit.id)
                                        .filter(|t| !t.trim().is_empty())
                                        .unwrap_or("未命名主题")
                                        .to_string();
                                    let field = hit.field;
                                    // 备注/链接命中：首行展示命中处摘录，高亮已折算好；
                                    // 第二行带上节点标题，否则「这条备注属于谁」看不出来。
                                    let (text, range) = match hit.excerpt {
                                        Some(ex) => (ex.text, Some(ex.range)),
                                        None => (title.clone(), hit.range),
                                    };
                                    let ancestors = canvas.node_path(&hit.id);
                                    let path = if field == mw_core::search::FIELD_TEXT {
                                        ancestors
                                    } else if ancestors.is_empty() {
                                        title
                                    } else {
                                        format!("{title} · {ancestors}")
                                    };
                                    HitRow {
                                        id: hit.id,
                                        text,
                                        kind: hit.kind,
                                        field,
                                        range,
                                        path,
                                        group: Group::Node,
                                        rel: None,
                                    }
                                })
                                .collect();
                            // 命题命中：关系线是边不是节点，单独一组排节点之后。
                            for hit in canvas.search_relationships(q, mode) {
                                let from = canvas
                                    .node_text(&hit.from)
                                    .filter(|t| !t.trim().is_empty())
                                    .unwrap_or("未命名概念")
                                    .to_string();
                                let to = canvas
                                    .node_text(&hit.to)
                                    .filter(|t| !t.trim().is_empty())
                                    .unwrap_or("未命名概念")
                                    .to_string();
                                rows.push(HitRow {
                                    id: hit.id.clone(),
                                    text: hit.excerpt.text,
                                    kind: hit.kind,
                                    field: "relationship".into(),
                                    range: Some(hit.excerpt.range),
                                    path: format!("{from} → {to}"),
                                    group: Group::Relationship,
                                    rel: Some(panels::search::RelRow {
                                        from: hit.from,
                                        id: hit.id,
                                    }),
                                });
                            }
                            if let Some(notes) = notes {
                                for hit in notes.search(q, 4, mode) {
                                    rows.push(HitRow {
                                        id: hit.id,
                                        text: hit.title,
                                        kind: hit.kind,
                                        field: "text".into(),
                                        range: None,
                                        path: "卡片".into(),
                                        group: Group::Card,
                                        rel: None,
                                    });
                                }
                                for (tag, id) in notes.search_tags(q) {
                                    rows.push(HitRow {
                                        id,
                                        text: format!("#{tag}"),
                                        kind: "tag".into(),
                                        field: "text".into(),
                                        range: None,
                                        path: "标签".into(),
                                        group: Group::Tag,
                                        rel: None,
                                    });
                                }
                            }
                            let seen: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
                            for (id, text, path) in canvas.search_paths(q) {
                                if seen.contains(&id) {
                                    continue;
                                }
                                rows.push(HitRow {
                                    id,
                                    text,
                                    kind: "path".into(),
                                    field: "text".into(),
                                    range: None,
                                    path,
                                    group: Group::Path,
                                    rel: None,
                                });
                            }
                            rows.sort_by_key(|r| r.group);
                            if crate::trace::is_enabled() {
                                let note = rows
                                    .iter()
                                    .filter(|r| r.field == mw_core::search::FIELD_NOTE)
                                    .count();
                                let link = rows
                                    .iter()
                                    .filter(|r| r.field == mw_core::search::FIELD_LINK)
                                    .count();
                                let rel = rows.iter().filter(|r| r.rel.is_some()).count();
                                crate::trace::event(&format!(
                                    "search_hits q={q} rows={} note={note} link={link} rel={rel}",
                                    rows.len()
                                ));
                            }
                            rows
                        });
                    }
                }
                // 没有 wgpu 后端时退回 M0 占位，至少把材质/句柄状态显示出来
                _ => Shell::body(ui, &ctx, &self.backdrop, self.harness.is_none()),
            }
        });
        // 基准导出排在首帧画布之后：此时场景、字体与 GPU 后端都已就绪，
        // 量到的就是真实导出路径，而不是冷启动融合值。
        if self.perf_saw_canvas && self.bench_export.is_some() && !self.bench_started {
            if let Some(rs) = frame.wgpu_render_state() {
                self.start_bench_export(rs);
                ctx.request_repaint();
            }
        }
        self.export_tick(&ctx);
        if toolbar_card {
            self.capture_card_for_selected();
        }
        if let Some((text, bad)) = canvas_notice {
            self.set_toast(text, bad);
        }
        if let Some(id) = card_open {
            let card = self
                .canvas
                .as_ref()
                .and_then(|c| c.session().doc.card(&id).cloned())
                .or_else(|| self.notes.as_ref().and_then(|n| n.get(&id)));
            if let Some(card) = card {
                let reader = self.card_reader_for(&card);
                self.reader_from_library = false;
                self.note_reader = Some(reader);
            }
        }
        if let Some(req) = context_request {
            // 其它浮层开着时右键已经被画布挡掉；这里再守一道，避免菜单压在两层面板上。
            if !demo
                && !self.home
                && !self.palette.is_open()
                && !self.search.is_open()
                && !self.help.is_open()
                && !self.export.open
            {
                self.context_menu = self.build_context_menu(req);
            }
        }
        let menu_action = self
            .context_menu
            .as_mut()
            .and_then(|menu| menu.show(&ctx, tone));
        if self
            .context_menu
            .as_ref()
            .is_some_and(|menu| !menu.is_open())
        {
            self.context_menu = None;
        }
        if let Some(action) = menu_action {
            self.run_context_action(&ctx, action);
        }
        let tab_menu_action = self
            .tab_menu
            .as_mut()
            .and_then(|menu| menu.show(&ctx, tone));
        if self.tab_menu.as_ref().is_some_and(|menu| !menu.is_open()) {
            self.tab_menu = None;
        }
        if let Some(action) = tab_menu_action {
            self.run_tab_action(&ctx, action);
        }
        // 大纲行菜单浮层：与画布右键菜单同层，画完立刻处理点击。
        if !self.outline_view {
            self.outline_row_menu = None;
        }
        let outline_row_action = self.outline_row_menu.as_mut().and_then(|row| {
            row.menu
                .show(&ctx, tone)
                .map(|action| (action, row.target.clone(), row.multi))
        });
        if self
            .outline_row_menu
            .as_ref()
            .is_some_and(|row| !row.menu.is_open())
        {
            self.outline_row_menu = None;
        }
        if let Some((action, target, multi)) = outline_row_action {
            self.run_outline_row_action(&ctx, action, &target, multi);
        }

        if let Some(action) = search_action {
            self.run_search(action);
            ctx.request_repaint();
        }
        if let Some(action) = home_action {
            self.run_home(action);
            ctx.request_repaint();
        }

        // 热区盖在最上层；最大化时系统已禁缩放，不必再铺。
        if !maximized {
            ui.scope_builder(egui::UiBuilder::new().max_rect(win), |ui| {
                Shell::resize_edges(ui, win, &ctx);
                // 1px 描边：Mica 与桌面之间需要一条边界，否则无框窗口「糊」在背景上
                ui.painter().rect_stroke(
                    win,
                    8.0,
                    Stroke::new(1.0, chrome.edge),
                    StrokeKind::Inside,
                );
            });
        }

        // 演示幕布上只留控制条；保存提示之类的 toast 会破坏投影画面。
        if demo {
            self.demo_bar(&ctx);
            self.demo_notes(&ctx);
        } else {
            self.toast_bar(&ctx);
        }

        // 运行时探针（`MW_STATS=1`）：放在所有自身 UI 之后，本帧耗时已定型。
        if self.stats.enabled() {
            let snap = self
                .canvas
                .as_ref()
                .map(|c| c.perf_snapshot())
                .unwrap_or_default();
            let ui_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
            self.stats.tick(std::time::Instant::now(), ui_ms, snap);
            self.stats.show(&ctx, snap);
        }

        // 页面型浮层先画：它们是弹窗的背景，不参与互斥。
        let library_t0 = std::time::Instant::now();
        let library_was_open = self.library.is_some();
        self.library_ui(&ctx);
        if library_was_open {
            // 10k 库首屏预算（docs/28 §7.4）：`MW_LIBRARY_PERF=<文件>` 逐帧记耗时。
            crate::trace::library_frame_ms(library_t0.elapsed().as_secs_f32() * 1000.0);
        }
        self.review_ui(&ctx);
        self.knowledge_ui(&ctx);
        // 卡片阅读页是前景页：画布已 `set_input_blocked`，这里只负责画与收动作。
        let reader_t0 = std::time::Instant::now();
        let reader_theme = self
            .canvas
            .as_ref()
            .map(|c| c.session().theme.clone())
            .unwrap_or_else(|| mw_core::get_theme("snow"));
        let reader_action = self
            .note_reader
            .as_mut()
            .and_then(|r| r.show(&ctx, &self.chrome, &reader_theme));
        if self.note_reader.is_some() {
            // CER-20 长文性能：`MW_CARD_PERF=<文件>` 时逐帧记下画卡片页的耗时。
            crate::trace::card_frame_ms(reader_t0.elapsed().as_secs_f32() * 1000.0);
        }
        if let Some(action) = reader_action {
            match action {
                note_view::ReaderAction::Close => {
                    self.note_reader = None;
                    if std::mem::take(&mut self.reader_from_library) {
                        self.open_library();
                    }
                }
                note_view::ReaderAction::OpenCard(id) => {
                    self.note_reader = None;
                    self.open_library_card(&id);
                }
                note_view::ReaderAction::JumpToNode(id) => {
                    if !self.jump_to_source(&id) {
                        let concept = self
                            .canvas
                            .as_ref()
                            .is_some_and(|c| c.session().doc.is_concept_map());
                        self.set_toast(
                            if concept {
                                "来源概念不在当前文档里"
                            } else {
                                "来源主题不在当前文档里"
                            },
                            true,
                        );
                    }
                }
                note_view::ReaderAction::OpenLink(url) => {
                    if let Err(e) = files::open_url(&url) {
                        self.set_toast(e, true);
                    }
                }
                note_view::ReaderAction::ManageTags => {
                    self.note_reader = None;
                    self.reader_from_library = false;
                    self.tag_manager = Some(TagManager::default());
                }
                note_view::ReaderAction::Save {
                    title,
                    body,
                    tags,
                    close,
                } => {
                    // CER-20：不在这里同步写盘——排进队列，下一帧开头 `pump_card_save` 才落，
                    // 这样顶栏那一帧的「保存中…」说的就是真话（与文档 `SavePhase::Saving` 同一套）。
                    self.pending_card_save = Some(CardSave {
                        title,
                        body,
                        tags,
                        close,
                    });
                    self.ctx.request_repaint();
                }
                note_view::ReaderAction::ReloadFromDisk => {
                    if let Some(id) = self.note_reader.as_ref().map(|r| r.card_id.clone()) {
                        self.reload_card_from_disk(&id);
                    }
                }
                note_view::ReaderAction::OverwriteDisk => {
                    // 用户认了这一笔：跳过指纹检查，立刻按当前草稿写一次。
                    self.card_overwrite = true;
                    if let Some(r) = self.note_reader.as_mut() {
                        r.set_conflict(false);
                        self.pending_card_save = Some(CardSave {
                            title: r.draft_title.clone(),
                            body: r.draft_body.clone(),
                            tags: r.draft_tags.clone(),
                            close: false,
                        });
                        r.mark_saving();
                    }
                    self.ctx.request_repaint();
                }
                note_view::ReaderAction::DismissConflict => {
                    // 「取消」= 这一笔先不写：草稿还在、还是脏的，顶栏回到「编辑中」。
                    // 下一次自动保存仍会先比指纹，磁盘还被人改着就会再提这条。
                    if let Some(r) = self.note_reader.as_mut() {
                        r.set_conflict(false);
                    }
                }
            }
        }

        // 弹窗最后画，且只画栈顶：低层状态保留，顶层关掉后自然恢复。
        // OutlineInfo / LibraryRemove 在各自页面里绘制，这里不重复调用。
        let modal = self.overlay_state().top_modal();
        match modal {
            Some(overlay::Modal::ImportBusy | overlay::Modal::ImportWarnings) => {
                self.import_ui(&ctx)
            }
            Some(overlay::Modal::Opening) => self.opening_ui(&ctx),
            Some(overlay::Modal::OpenFailed) => self.open_failed_ui(&ctx),
            Some(overlay::Modal::TagManager) => self.tag_manager_ui(&ctx),
            Some(overlay::Modal::LibraryDialog) => self.library_dialog_ui(&ctx),
            Some(overlay::Modal::Export) => self.export_dialog(&ctx, frame.wgpu_render_state()),
            Some(overlay::Modal::Shortcuts) => {
                self.shortcuts_dialog.show(&ctx, tone, &mut self.shortcuts);
            }
            Some(overlay::Modal::Settings) => self.settings_ui(&ctx),
            Some(overlay::Modal::ThemeStudio) => self.theme_studio_ui(&ctx),
            Some(overlay::Modal::History) => self.history_ui(&ctx),
            Some(overlay::Modal::RecoveryCenter) => self.recovery_ui(&ctx),
            Some(overlay::Modal::RecoveryPrompt) => self.recovery_prompt(&ctx),
            Some(overlay::Modal::Conflict) => self.conflict_ui(&ctx),
            Some(overlay::Modal::CloseConfirm) => self.close_prompt(&ctx),
            Some(overlay::Modal::DiscardConfirm) => self.discard_prompt(&ctx),
            Some(overlay::Modal::TabCloseConfirm) => self.tab_close_prompt(&ctx),
            Some(overlay::Modal::Stats) => self.stats_ui(&ctx),
            Some(overlay::Modal::Onboarding) => self.onboarding_ui(&ctx),
            Some(overlay::Modal::OutlineInfo | overlay::Modal::LibraryRemove) | None => {}
        }
        if self.dialog_demo && modal.is_none() {
            self.dialog_demo = dialog::demo(&ctx, self.chrome);
        }

        // 焦点轨迹（`MW_TRACE=<文件>`）：帧末统一记下「这一帧谁握着焦点」。放在最后是因为
        // 语义名由各控件在本帧绘制时登记（`trace::name`），而焦点可能又被这一帧后面的
        // `request_focus` 改掉——只有这里读到的才是本帧的结论。
        crate::trace::observe(&ctx);

        // 输入脚本没跑完（含 sleep）之前不推进截图状态机：否则会把拖拽中间态
        // 当成终态截下来。脚本结束后再空跑 warmup，画面已静止。
        let script_running = self.golden_input.as_ref().is_some_and(|g| !g.finished());
        if script_running {
            ctx.request_repaint();
        } else if let Some(harness) = self.harness.as_mut() {
            // 快照状态机放在最后：命令在本帧输出里落地，下一帧绘制时被取走。
            // warmup 之后场景已静止，所以截到的与本轮画的就是同一张图。
            if let Some(code) = harness.tick(&ctx) {
                self.exit.store(code as i32, Ordering::Relaxed);
                // 开始页等分支可能不响应 Close；harness 是无状态进程，直接退出最稳。
                std::process::exit(code as i32);
            }
        }
    }
}

/// egui 全局选项里与本应用冲突的那几条（目前只有一条）。
///
/// **键盘缩放归画布**：egui 自带的 `zoom_with_keyboard`（默认 true）会在每帧末尾
/// `consume_shortcut` 掉 `Ctrl+=` / `Ctrl+-` / `Ctrl+0`，去改整个 UI 的缩放因子
/// （`pixels_per_point` 跟着变）。画布自己也收这三个键（`Canvas::zoom_step_at`）
/// 于是按一次 `Ctrl+=`：画布放大一格 + 整个界面放大一档，`ppp` 从 1.5 跳到 1.6 之类。
/// 用户看到的是「按了缩放，窗口比例也变了」；金样里是整屏漂，键盘缩放的基线根本拍不住。
/// 画布已有自己的缩放入口与轨迹，这条让出来，界面缩放仍可走系统 DPI。
fn install_options(ctx: &egui::Context) {
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}

fn main() -> eframe::Result<ExitCode> {
    // 无界面文本导出先于窗口初始化：脚本/CI 不需要 GPU。
    if let Some(code) = cli::run(std::env::args().skip(1)) {
        return Ok(ExitCode::from(code as u8));
    }
    // `MW_PERF_LOG` 未设置时就是一个空壳，冷启动计时从这里开始（进程入口附近）。
    let perf = Perf::from_env();
    let golden = match golden::invocation() {
        Invocation::Help => {
            print!("{}", golden::usage());
            return Ok(ExitCode::SUCCESS);
        }
        Invocation::Interactive => None,
        Invocation::Golden(opts) => Some(opts),
    };
    // 离屏：窗口自始至终不显示。合成器不给它交换链纹理，但 capture 走自有纹理，像素照样回读
    // （见 golden.rs 顶注）。少了「窗口在桌面上摆哪儿」这一整个变量，基线才谈得上可比。
    let offscreen = golden.as_ref().is_some_and(|o| !o.shown);

    let harness = golden.map(Harness::new);
    // 退出码要从被 Box 吃掉、再也取不回来的 App 里捎回进程，只能靠共享原子量。
    let exit = Arc::new(AtomicI32::new(0));
    let closure_exit = Arc::clone(&exit);

    // 验收矩阵用：`MW_WINDOW_SIZE=1024x720` 只改首屏逻辑尺寸，不写窗口状态。
    let initial_size = std::env::var("MW_WINDOW_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once(['x', 'X'])?;
            let w: f32 = w.trim().parse().ok()?;
            let h: f32 = h.trim().parse().ok()?;
            (w >= 640.0 && h >= 460.0).then_some([w, h])
        })
        .unwrap_or([1_080.0, 720.0]);
    eframe::run_native(
        APP_ID,
        eframe::NativeOptions {
            // 未指定位置时 winit 会把大窗口摆到屏幕外（1080×720@150% 在某些小屏上很常见）
            centered: true,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(initial_size)
                .with_min_inner_size([640.0, 460.0])
                // 1080×720 在 1280×800@150% 这类小屏上会超出显示器；交给 winit 夹到屏幕内
                .with_clamp_size_to_monitor_size(true)
                .with_decorations(false)
                // 不要开 with_transparent(true)：winit 在 Windows 上用
                // DwmEnableBlurBehindWindow（空区域）实现，而 wgpu 的 HWND 交换链只有
                // Opaque alpha（运行时会警告）。这层 blur-behind 反而会让 DWM 在
                // 无边框全屏后不刷新窗口位置——画面停在旧尺寸，直到真实 resize。
                .with_visible(!offscreen)
                .with_title("织思"),
            ..Default::default()
        },
        Box::new(move |cc| {
            // 先装中文字体：egui 默认字体没有 CJK，标题栏与行内编辑器都依赖它。
            fonts::install(&cc.egui_ctx);
            // 键盘缩放归画布，别让 egui 顺手把整个 UI 的 ppp 也改掉（见 `install_options`）。
            install_options(&cc.egui_ctx);
            // 第一帧就得是设计稿的颜色：egui 此时还不知道系统深浅色，`set_visuals`
            // 会把它写进没人用的那个桶（见 `install_visuals`）。
            install_visuals(&cc.egui_ctx, chrome_of(&mw_core::get_theme("snow")));
            Ok(Box::new(Shell::new(cc, harness, closure_exit, perf)))
        }),
    )?;

    Ok(ExitCode::from(exit.load(Ordering::Relaxed) as u8))
}

#[cfg(test)]
mod app_tests;

#[cfg(test)]
mod egui_options_tests {
    /// X21：键盘缩放只能动画布，不能连整个 UI 的缩放因子一起动。
    ///
    /// egui 默认开着 `zoom_with_keyboard`，它会吃掉 `Ctrl+=` / `Ctrl+-` / `Ctrl+0`
    /// 去缩放界面（`pixels_per_point` 变、画布视口也跟着变），与画布的缩放双触发。
    /// 这条断言钉的是「初始化之后它是关的」——回滚这一行，键盘缩放的证据会整屏漂。
    #[test]
    fn keyboard_zoom_belongs_to_the_canvas_only() {
        let ctx = eframe::egui::Context::default();
        assert!(
            ctx.options(|o| o.zoom_with_keyboard),
            "egui 默认必须开着：这条测试才有意义"
        );
        super::install_options(&ctx);
        assert!(
            !ctx.options(|o| o.zoom_with_keyboard),
            "`install_options` 必须把键盘 UI 缩放关掉"
        );
    }
}

/// SR-02「外壳生命周期七问」的回归测试。
///
/// 全部用纯函数或"不需要 GPU/窗口"的部件（`Tab`/`TabGuard`/`Session`/`file_guard`/
/// `autosave`），所以能在 `cargo test -p mw-shell --lib` 里直接跑。
#[cfg(test)]
mod sr02_tests {
    use super::*;
    use crate::session::Session;
    use mw_core::Command;

    /// 每个用例一个独占目录，与 `autosave`/`file_guard` 的测试约定一致
    /// （并行跑测试时用毫秒当名字会撞车，撞了就是「拒绝访问」）。
    fn temp_dir(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mw-sr02-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 造一份已经落盘、干净、带路径的会话（等价于「刚打开一份文件」）。
    fn saved_session(path: &std::path::Path) -> Session {
        let mut s = Session::sample();
        s.save_to(path).expect("落盘");
        Session::open(path).expect("重新打开")
    }

    /// 走正式通道改一个字让会话变脏（不绕过 `Command`，不直接动 `Document`）。
    fn dirty(s: &mut Session) {
        let id = s.doc.root_id.clone();
        let before = s.doc.revision;
        s.doc
            .apply(&Command::SetText {
                id,
                text: format!("改过的标题 {before}"),
            })
            .expect("改字");
        assert!(s.doc.revision > before, "改字要推进修订号");
        assert!(s.dirty(), "改完应当是脏的");
    }

    // ---- 问题 1：协作锁/指纹/只读是「每标签」的，不是窗口的 -----------------

    #[test]
    fn tab_guards_do_not_cross_wire() {
        let dir = temp_dir("cross");
        let pa = dir.join("a.zmind");
        let pb = dir.join("b.zmind");
        let ta = Tab::new(saved_session(&pa));
        let tb = Tab::new(saved_session(&pb));

        assert_eq!(ta.guard.path.as_deref(), Some(pa.as_path()));
        assert_eq!(tb.guard.path.as_deref(), Some(pb.as_path()));
        assert_eq!(ta.guard.print, file_guard::fingerprint(&pa), "a 认自己那份");
        assert_eq!(tb.guard.print, file_guard::fingerprint(&pb), "b 认自己那份");
        assert!(
            !ta.guard.read_only && !tb.guard.read_only,
            "各自拿着自己的锁"
        );

        // 外部改了 b：只有 b 的守卫该发现"磁盘变了"，a 必须岿然不动。
        // 这一条就是「打开 A → 打开 B → 切回 A，Ctrl+S 谎报文件被外部修改」的反例。
        let mut external = Session::open(&pb).expect("读 b");
        dirty(&mut external);
        std::thread::sleep(std::time::Duration::from_millis(20));
        external.save_to(&pb).expect("外部写 b");

        let now_b = file_guard::fingerprint(&pb);
        let now_a = file_guard::fingerprint(&pa);
        assert!(
            file_guard::changed_since(tb.guard.print, now_b),
            "b 的指纹应当过期"
        );
        assert!(
            !file_guard::changed_since(ta.guard.print, now_a),
            "a 的指纹不该被 b 的改动波及（串线的根源就在这里）"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tab_guard_takes_over_when_other_holder_leaves() {
        let dir = temp_dir("handover");
        let p = dir.join("d.zmind");
        let s = saved_session(&p);
        let peer = file_guard::Guard::try_acquire(&p).expect("外部先占锁");
        let mut tab = Tab::new(s);
        assert!(tab.guard.read_only, "锁在别人手里应当只读");
        assert!(tab.guard.lock.is_none());

        drop(peer);
        tab.guard.arm(Some(&p));
        assert!(!tab.guard.read_only, "对方放手后应当接管为可写");
        assert!(tab.guard.lock.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tab_guard_releases_lock_when_path_changes() {
        let dir = temp_dir("move");
        let pa = dir.join("a.zmind");
        let pb = dir.join("b.zmind");
        let mut tab = Tab::new(saved_session(&pa));
        assert!(tab.guard.lock.is_some());

        tab.guard.arm(Some(&pb));
        assert_eq!(tab.guard.path.as_deref(), Some(pb.as_path()));
        assert_eq!(tab.guard.print, file_guard::fingerprint(&pb), "指纹跟着换");
        // 旧文件的锁必须已经放掉：别人现在能取到（否则那个文件到关窗都只读）。
        assert!(
            file_guard::Guard::try_acquire(&pa).is_some(),
            "换路径后旧文件的锁没放掉"
        );

        tab.guard.arm(None);
        assert!(tab.guard.lock.is_none(), "未命名文档没有锁");
        assert!(tab.guard.print.is_none());
        assert!(!tab.guard.read_only);
        tab.guard.release();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- 问题 2：关窗脏检查必须覆盖所有标签 ------------------------------

    #[test]
    fn dirty_tab_names_covers_background_tabs() {
        // 纯函数：顺序保持、只挑脏的。关窗提示与"保存并关闭"都以它为准。
        let tabs = vec![
            ("方案 A".to_string(), false),
            ("方案 B".to_string(), true),
            ("未命名".to_string(), true),
        ];
        assert_eq!(dirty_tab_names(&tabs), vec!["方案 B", "未命名"]);
        assert!(dirty_tab_names(&[]).is_empty());
        assert!(dirty_tab_names(&[("干净".to_string(), false)]).is_empty());
    }

    #[test]
    fn save_background_dirty_tabs_writes_each_path() {
        let dir = temp_dir("bg");
        let pa = dir.join("a.zmind");
        let pb = dir.join("b.zmind");
        let mut sa = saved_session(&pa);
        let mut sb = saved_session(&pb);
        dirty(&mut sa);
        dirty(&mut sb);

        // active = 0：它的权威会话在画布上，`tabs[0]` 只是留的空白占位。
        let mut tabs = vec![Tab::new(Session::blank()), Tab::new(sa), Tab::new(sb)];
        let out = save_background_dirty_tabs(&mut tabs, 0);
        assert_eq!(out.saved, 2, "两个后台标签各自写回自己的路径");
        assert!(out.failed.is_empty(), "没有失败：{:?}", out.failed);
        assert!(out.untitled.is_empty());
        assert!(!tabs[1].session.dirty() && !tabs[2].session.dirty());

        // 真的落盘了：重新打开是干净的，且内容是我们改过的。
        let reopened = Session::open(&pa).expect("重新打开 a");
        assert!(!reopened.dirty());
        assert!(!reopened.title().is_empty(), "标题栏得有标题");

        // 未命名标签没法非交互落盘：要点名报告，不能假装存好了。
        let mut untitled = Session::blank();
        dirty(&mut untitled);
        let mut tabs2 = vec![Tab::new(Session::blank()), Tab::new(untitled)];
        let out2 = save_background_dirty_tabs(&mut tabs2, 0);
        assert_eq!(out2.saved, 0);
        assert_eq!(out2.untitled.len(), 1, "未命名要单独列出来");
        assert!(tabs2[1].session.dirty(), "没路径就还是脏的");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- 问题 4：启动时"比正本新的恢复点"必须提示 ------------------------

    #[test]
    fn restore_tab_session_prefers_newer_recovery_point() {
        let dir = temp_dir("restore-newer");
        let p = dir.join("方案.zmind");
        let mut s = saved_session(&p);
        dirty(&mut s);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let slot = autosave::write(&s).expect("写恢复点");
        assert_eq!(autosave::pending_for(&p), Some(slot.clone()), "恢复点更新");

        let (session, used) = restore_tab_session(&p).expect("装载");
        assert_eq!(used, Some(slot), "应当用上恢复点");
        assert_eq!(session.path.as_deref(), Some(p.as_path()), "路径指回正本");
        assert!(
            session.dirty(),
            "恢复出来的内容还没写回正本，必须是脏的（Ctrl+S 才写回）"
        );
        autosave::clear(&session);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_tab_session_uses_plain_file_without_recovery_point() {
        let dir = temp_dir("restore-plain");
        let p = dir.join("方案.zmind");
        let _ = saved_session(&p);
        let (session, used) = restore_tab_session(&p).expect("装载");
        assert_eq!(used, None, "没有恢复点就不该提示");
        assert!(!session.dirty(), "直接读正本，干净");
        assert!(
            restore_tab_session(&dir.join("不存在.zmind")).is_none(),
            "读不到就是读不到"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- 问题 5：四条"保存成功"的路径都要收尾，且只收一次 -----------------

    /// 在 `marker` 之后 `window` 字符内出现过 `after_saved()` 吗？
    fn has_after_saved_after(src: &str, marker: &str, window: usize) -> bool {
        let mut from = 0;
        while let Some(i) = src[from..].find(marker) {
            let start = from + i;
            let end = (start + window).min(src.len());
            if src[start..end].contains("after_saved()") {
                return true;
            }
            from = start + marker.len();
        }
        false
    }

    #[test]
    fn every_save_success_path_calls_after_saved() {
        // 普通保存、另存为、覆盖保存、历史版本恢复：四条路都得收尾
        // （恢复点作废、最近文件、`.history` 快照）。漏一条就是问题 5。
        let m2 = include_str!("app_methods_2.rs");
        assert!(
            has_after_saved_after(m2, "canvas.save()", 900),
            "普通保存（pump_save）没调 after_saved"
        );
        assert!(
            has_after_saved_after(m2, "canvas.save_to(&path)", 900),
            "另存为没调 after_saved"
        );
        let m1 = include_str!("app_methods_1.rs");
        assert!(
            has_after_saved_after(m1, "canvas.save()", 900),
            "覆盖保存（conflict_ui）没调 after_saved"
        );
        let hui = include_str!("history_ui.rs");
        assert!(
            has_after_saved_after(hui, "history::restore", 900),
            "历史版本恢复没调 after_saved"
        );
    }

    #[test]
    fn after_saved_is_idempotent_for_same_revision() {
        let p = std::path::Path::new(r"C:\docs\方案.zmind");
        // 第一次：没有历史 → 算新的。
        assert!(save_finish_is_new(None, p, 7));
        // 同一个文件同一个修订号再来一次 → 不算新的（不能重复写快照）。
        assert!(!save_finish_is_new(Some((p, 7)), p, 7));
        // 修订号涨了 → 新的。
        assert!(save_finish_is_new(Some((p, 7)), p, 8));
        // 换了文件但修订号相同 → 新的（另存为）。
        assert!(save_finish_is_new(
            Some((p, 7)),
            std::path::Path::new(r"C:\docs\另一份.zmind"),
            7
        ));
    }

    // ---- 问题 6/7：自动保存的失败要说话，记账要等确认 ---------------------

    #[test]
    fn autosave_backoff_grows_and_caps() {
        // 阶梯：15 → 30 → 60 → 120 → 240，再往上被 300 秒封顶。
        assert_eq!(autosave_backoff(1), std::time::Duration::from_secs(15));
        assert_eq!(autosave_backoff(2), std::time::Duration::from_secs(30));
        assert_eq!(autosave_backoff(3), std::time::Duration::from_secs(60));
        assert_eq!(autosave_backoff(4), std::time::Duration::from_secs(120));
        assert_eq!(autosave_backoff(5), std::time::Duration::from_secs(240));
        // 15 << 5 = 480s 被 300s 压住。
        assert_eq!(autosave_backoff(6), std::time::Duration::from_secs(300));
        assert_eq!(autosave_backoff(9), std::time::Duration::from_secs(300));
        // 单调不减，别出现"重试越久间隔越短"。
        for a in 1..AUTOSAVE_GIVE_UP {
            assert!(
                autosave_backoff(a + 1) >= autosave_backoff(a),
                "第 {a} 级退避倒挂了"
            );
        }
    }

    #[test]
    fn autosave_may_retry_resets_on_new_revision() {
        let now = std::time::Instant::now();
        assert!(autosave_may_retry(None, 3, now), "没失败过就能写");
        let stall = AutosaveStall {
            attempts: 3,
            rev: 3,
            retry_at: now + std::time::Duration::from_secs(60),
        };
        assert!(
            !autosave_may_retry(Some(&stall), 3, now),
            "退避没到点不重试"
        );
        assert!(
            autosave_may_retry(Some(&stall), 3, now + std::time::Duration::from_secs(61)),
            "到点就重试"
        );
        assert!(autosave_may_retry(Some(&stall), 4, now), "内容变了立刻重试");
        let gave_up = AutosaveStall {
            attempts: AUTOSAVE_GIVE_UP,
            rev: 3,
            retry_at: now,
        };
        assert!(
            !autosave_may_retry(Some(&gave_up), 3, now),
            "同一修订号重试太多次就放弃，别无限刷屏"
        );
    }

    #[test]
    fn autosave_wants_skips_confirmed_and_in_flight() {
        // 脏 + 没确认过 → 要写。
        assert!(autosave_wants(true, 5, None, None));
        // 干净 → 不写。
        assert!(!autosave_wants(false, 5, None, None));
        // 这个修订号已经确认落盘 → 不重写。
        assert!(!autosave_wants(true, 5, Some(5), None));
        // 已经在途 → 不重复投（问题 7 的另一半：别堆成一队）。
        assert!(!autosave_wants(true, 5, None, Some(5)));
        // 在途的是旧修订号 → 新的一版要投。
        assert!(autosave_wants(true, 6, None, Some(5)));
    }

    #[test]
    fn needs_recovery_submit_only_for_dirty_unconfirmed() {
        assert!(needs_recovery_submit(true, 9, None));
        assert!(!needs_recovery_submit(false, 9, None), "干净没什么可写的");
        assert!(
            !needs_recovery_submit(true, 9, Some(9)),
            "已确认过就不必再写"
        );
        assert!(needs_recovery_submit(true, 10, Some(9)), "新修订号要写");
    }

    #[test]
    fn draft_differs_ignores_tag_order_but_catches_content() {
        let base = (
            "标题".to_string(),
            "正文".to_string(),
            vec!["a".to_string(), "b".to_string()],
        );
        assert!(!draft_differs(
            Some(&base),
            "标题",
            "正文",
            &["b".into(), "a".into()]
        ));
        assert!(draft_differs(Some(&base), "标题2", "正文", &base.2));
        assert!(draft_differs(Some(&base), "标题", "正文2", &base.2));
        assert!(draft_differs(Some(&base), "标题", "正文", &["a".into()]));
        // 库里还没有这张卡：草稿非空就得落。
        assert!(draft_differs(None, "新卡", "", &[]));
        assert!(!draft_differs(None, "  ", "", &[]), "全空白不算有内容");
    }
}
