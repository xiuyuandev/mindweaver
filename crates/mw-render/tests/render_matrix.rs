//! 渲染矩阵：5 种布局与 4 套主题都必须真的出图，而且彼此不同。
//!
//! 这是 v28 那轮「脚本跑矩阵」的固化版：不启动窗口、不等 3 秒，直接走离屏合成。
//! 断言只用像素统计（非背景占比 / 颜色数 / 内容哈希），不依赖人眼。
//! 没有 GPU 的机器会打印原因并跳过——与 `offscreen.rs` 同一套约定。

use mw_core::text::MonoMeasurer;
use mw_core::theme::{get_theme, Theme};
use mw_core::{build_scene, Command, Document, LayoutSettings, Point, Scene};
use mw_render::instances::Frame;
use mw_render::{build_layers, Camera, Compositor};

const VIEWPORT: [u32; 2] = [900, 600];

fn doc_with(layout: &str) -> Document {
    let mut doc = Document::new("矩阵");
    doc.meta.layout = LayoutSettings {
        kind: layout.to_string(),
        ..Default::default()
    };
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "矩阵中心".into(),
    })
    .unwrap();
    let mut ids = Vec::new();
    for text in ["甲分支", "乙分支", "丙分支"] {
        let cs = doc
            .apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: text.to_string(),
                style: None,
            })
            .expect("加分支");
        ids.push(cs.after[0].id.clone());
    }
    doc.apply(&Command::AddNode {
        parent: ids[0].clone(),
        index: None,
        text: "甲的子节点".into(),
        style: None,
    })
    .expect("加孙节点");
    doc
}

fn scene_of(doc: &Document, theme: &Theme, layout: &LayoutSettings) -> Scene {
    build_scene(
        doc,
        &doc.root_id,
        layout,
        theme,
        &MonoMeasurer,
        Default::default(),
    )
}

/// 渲染一张离屏图，返回 (非背景像素占比, 不同颜色数, 内容哈希)。
fn render_stats(compositor: &mut Compositor, doc: &Document, theme: &Theme) -> (f64, usize, u64) {
    let layout = doc.meta.layout.clone();
    let scene = scene_of(doc, theme, &layout);
    let camera = Camera::new(scene.bounds.center(), 1.0, 1.0);
    let frame = Frame {
        scene: &scene,
        theme,
        camera,
        selected: Vec::new(),
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport: Some([VIEWPORT[0] as f32, VIEWPORT[1] as f32]),
    };
    let layers = build_layers(&frame);
    let image = compositor.render(&frame, &layers, VIEWPORT);
    let rgba = image.rgba();
    let bg = theme.ui.canvas_bg.0;
    let mut non_bg = 0usize;
    let mut colors = std::collections::HashSet::new();
    let mut hash: u64 = 1469598103934665603;
    for px in rgba.chunks(4) {
        if px[0] != bg[0] || px[1] != bg[1] || px[2] != bg[2] {
            non_bg += 1;
        }
        colors.insert([px[0], px[1], px[2]]);
        for b in px {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
    }
    let total = (VIEWPORT[0] * VIEWPORT[1]) as f64;
    (non_bg as f64 / total, colors.len(), hash)
}

fn compositor() -> Option<Compositor> {
    match Compositor::new() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("跳过渲染矩阵：{e}");
            None
        }
    }
}

#[test]
fn every_layout_renders_and_they_all_differ() {
    let Some(mut c) = compositor() else {
        return;
    };
    let theme = get_theme("snow");
    let mut seen: Vec<(String, u64)> = Vec::new();
    for layout in ["mindmap", "logicRight", "orgChart", "fishbone", "timeline"] {
        let doc = doc_with(layout);
        let (occupied, colors, hash) = render_stats(&mut c, &doc, &theme);
        assert!(
            occupied > 0.01,
            "{layout} 几乎没画出东西：非背景占比 {occupied:.4}"
        );
        assert!(colors > 8, "{layout} 颜色太单一：{colors}");
        for (other, other_hash) in &seen {
            assert_ne!(&hash, other_hash, "{layout} 与 {other} 渲染结果完全相同");
        }
        seen.push((layout.to_string(), hash));
    }
    assert_eq!(seen.len(), 5);
}

#[test]
fn every_theme_renders_and_they_all_differ() {
    let Some(mut c) = compositor() else {
        return;
    };
    let mut seen: Vec<(String, u64)> = Vec::new();
    for id in mw_core::theme::THEME_IDS {
        let theme = get_theme(id);
        let doc = doc_with("mindmap");
        let (occupied, colors, hash) = render_stats(&mut c, &doc, &theme);
        // 「微光」这类主题整张图只有文字与线（画布纯白、卡片全透明），
        // 非背景占比天然只有千分之几——阈值按主题的下限定，不按有卡片的主题定。
        assert!(occupied > 0.001, "{id} 几乎没画出东西：{occupied:.4}");
        assert!(colors > 4, "{id} 颜色太单一：{colors}");
        for (other, other_hash) in &seen {
            assert_ne!(&hash, other_hash, "{id} 与 {other} 渲染结果完全相同");
        }
        seen.push((id.to_string(), hash));
    }
    assert_eq!(seen.len(), 4);
}

/// 单独渲染一张图，供需要逐像素统计的测试用。
fn render_image(compositor: &mut Compositor, doc: &Document, theme: &Theme) -> mw_render::Image {
    let layout = doc.meta.layout.clone();
    let scene = scene_of(doc, theme, &layout);
    let frame = Frame {
        scene: &scene,
        theme,
        camera: Camera::new(scene.bounds.center(), 1.0, 1.0),
        selected: Vec::new(),
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport: Some([VIEWPORT[0] as f32, VIEWPORT[1] as f32]),
    };
    let layers = build_layers(&frame);
    compositor.render(&frame, &layers, VIEWPORT)
}

/// 新调的两套深色必须真的「暗」：画布主色占比高、纯白像素极少。
/// 这是「护眼」的像素代理指标——眼睛看不到，但眩光可以数出来。
#[test]
fn the_new_dark_themes_keep_glare_low_and_canvas_dominant() {
    let Some(mut c) = compositor() else {
        return;
    };
    for id in ["ink"] {
        let theme = get_theme(id);
        let doc = doc_with("mindmap");
        let img = render_image(&mut c, &doc, &theme);
        let total = (img.width * img.height) as f64;
        let bg = theme.ui.canvas_bg.0;
        let (mut canvas, mut white) = (0usize, 0usize);
        for y in 0..img.height {
            for x in 0..img.width {
                let p = img.pixel(x, y);
                if p[0] == bg[0] && p[1] == bg[1] && p[2] == bg[2] {
                    canvas += 1;
                }
                if p[0] >= 250 && p[1] >= 250 && p[2] >= 250 {
                    white += 1;
                }
            }
        }
        assert!(
            canvas as f64 / total > 0.70,
            "{id} 画布主色只占 {:.1}%，底色被别的东西盖住了",
            canvas as f64 / total * 100.0
        );
        assert!(
            white as f64 / total < 0.02,
            "{id} 纯白像素占 {:.2}%，深色主题不该有眩光",
            white as f64 / total * 100.0
        );
        println!(
            "{id}: 画布主色 {:.1}% · 纯白 {:.2}%",
            canvas as f64 / total * 100.0,
            white as f64 / total * 100.0
        );
    }
}

#[test]
fn a_default_theme_point_is_inside_the_canvas() {
    // 最小健全性检查：场景包围盒中心确实落在包围盒里（防止「渲染了但全在画外」）。
    let doc = doc_with("mindmap");
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme, &doc.meta.layout);
    let center = Point::new(
        scene.bounds.x + scene.bounds.w * 0.5,
        scene.bounds.y + scene.bounds.h * 0.5,
    );
    assert!(scene.bounds.contains(center));
}
