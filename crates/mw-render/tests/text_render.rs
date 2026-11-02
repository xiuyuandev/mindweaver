//! M1b-2b 出口判据：字形真的落到节点文字、标签、摘要与关系线标签的基线上，
//! 并且只改变文字覆盖的像素；100/125/150/200% 四档截图用于人眼验收（C3）。
//!
//! 文本依赖 DirectWrite：没有就 panic，不跳过——这台机器是 Windows，字体缺失
//! 属于环境坏了，不是「可以接受的不适用」。GPU 不在时只有像素项按老规矩跳过。

use std::path::{Path, PathBuf};

use mw_core::text::line_height;
use mw_core::theme::{get_theme, Rgba};
use mw_core::{build_scene, Command, Document, LayoutSettings, Point, Scene, Theme};
use mw_golden::{read_png, write_png, Drift};
use mw_render::{
    build_layers, Camera, Compositor, DWrite, DWriteMeasurer, Frame, LayerGeom, TextEngine, LAYERS,
    L_LINES, L_NODES, L_OVERLAY,
};

const VIEWPORT: [u32; 2] = [900, 600];

fn id_of(doc: &Document, text: &str) -> String {
    doc.doc_order()
        .into_iter()
        .find(|id| doc.node(id).is_some_and(|n| n.text == text))
        .unwrap_or_else(|| panic!("文档里没有「{text}」"))
}

/// 覆盖节点正文（含两行）、标签胶囊、摘要与关系线标签。
fn fixture() -> Document {
    let mut doc = Document::new("中心主题");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "中心主题".into(),
    })
    .unwrap();
    for t in ["分支一", "中文Rust混排", "甲\n甲"] {
        doc.apply(&Command::AddNode {
            parent: root.clone(),
            index: None,
            text: t.into(),
            style: None,
        })
        .unwrap();
    }
    let a = id_of(&doc, "分支一");
    let b = id_of(&doc, "中文Rust混排");
    let c = id_of(&doc, "甲\n甲");
    doc.apply(&Command::AddLabel {
        ids: vec![b.clone()],
        label: "重要".into(),
    })
    .unwrap();
    doc.apply(&Command::AddSummary {
        parent: root,
        targets: vec![a.clone(), c.clone()],
        text: "小结".into(),
    })
    .unwrap();
    // 两端都在左侧：标签落在左半边空白处，不会压在根节点文字上
    doc.apply(&Command::AddRelationship {
        from: a,
        to: c,
        label: Some("关联".into()),
    })
    .unwrap();
    doc
}

fn scene_of(doc: &Document, theme: &Theme) -> Scene {
    let dw = DWrite::new().expect("DirectWrite 必须可用");
    let measure = DWriteMeasurer::new(dw);
    build_scene(
        doc,
        &doc.root_id,
        &LayoutSettings::default(),
        theme,
        &measure,
        None::<(&str, &str)>,
    )
}

fn frame<'a>(scene: &'a Scene, theme: &'a Theme, zoom: f32, ppp: f32) -> Frame<'a> {
    let root = 0;
    Frame {
        scene,
        theme,
        camera: Camera::new(scene.node(root).geom.rect.center(), zoom, ppp),
        selected: Vec::new(),
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport: None,
    }
}

fn frame_in<'a>(
    scene: &'a Scene,
    theme: &'a Theme,
    zoom: f32,
    viewport: Option<[f32; 2]>,
) -> Frame<'a> {
    let root = 0;
    Frame {
        scene,
        theme,
        camera: Camera::new(scene.node(root).geom.rect.center(), zoom, 1.0),
        selected: Vec::new(),
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport,
    }
}

fn text_count(layers: &[LayerGeom; LAYERS]) -> usize {
    layers.iter().map(|l| l.texts.len()).sum()
}

fn engine() -> TextEngine {
    TextEngine::new().expect("DirectWrite 必须可用（文本渲染的硬前提）")
}

fn compositor() -> Option<Compositor> {
    match Compositor::new() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("跳过像素断言：{e}");
            None
        }
    }
}

fn shots_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shots");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn text_lands_on_every_layer_that_owns_text() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let f = frame(&scene, &theme, 1.0, 1.0);
    let mut layers = build_layers(&f);
    let mut text = engine();
    let update = text.paint(&f, &mut layers);

    assert!(
        !layers[L_NODES].texts.is_empty(),
        "节点正文/标签/摘要必须有文字"
    );
    assert!(!layers[L_OVERLAY].texts.is_empty(), "关系线标签必须有文字");
    assert!(layers[L_LINES].texts.is_empty(), "连线层不该有文字");
    assert!(update.generation >= 1);
    assert!(!update.rects.is_empty(), "首次绘制必须产生图集脏矩形");

    for inst in layers.iter().flat_map(|l| l.texts.iter()) {
        assert!(
            inst.size[0] > 0.0 && inst.size[1] > 0.0,
            "空四边形：{inst:?}"
        );
        assert!(
            inst.uv.iter().all(|v| (0.0..=1.0).contains(v)),
            "UV 越界：{inst:?}"
        );
        assert!(
            inst.pos.iter().all(|v| v.is_finite()),
            "位置非有限数：{inst:?}"
        );
    }
}

/// 文字 LOD（M1c）：字形物理高度 < 7px 时不整形，但节点盒照画；
/// 视口外的节点连盒带字都不排。
#[test]
fn text_lod_and_viewport_culling() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let mut text = engine();

    let f = frame(&scene, &theme, 1.0, 1.0);
    let mut full = build_layers(&f);
    text.paint(&f, &mut full);
    let full_glyphs = text_count(&full);
    assert!(full_glyphs > 0);

    // 14 世界 px × 0.1 = 1.4 物理 px，低于 TEXT_MIN_PX
    let f = frame(&scene, &theme, 0.1, 1.0);
    let mut tiny = build_layers(&f);
    text.paint(&f, &mut tiny);
    assert_eq!(text_count(&tiny), 0, "缩到看不清时不该整形任何字形");
    assert!(
        tiny[L_NODES].rects.len() >= scene.nodes.len(),
        "盒子属于形状，LOD 不能把它们一起丢掉"
    );
    assert!(
        tiny[L_NODES].rects.len() < full[L_NODES].rects.len(),
        "标签胶囊随文字一起进 LOD"
    );

    // 视口只框住根节点附近：文字实例必须显著少于全图
    let f = frame_in(&scene, &theme, 1.0, Some([160.0, 110.0]));
    let mut small = build_layers(&f);
    text.paint(&f, &mut small);
    assert!(
        text_count(&small) < full_glyphs,
        "视口外的文字不应整形：{} vs {full_glyphs}",
        text_count(&small)
    );
}

/// M2a：编辑中的节点文字由 egui 覆盖层画，合成器必须跳过它的字形与强调环，
/// 但节点盒、标签等几何照旧——否则编辑器底下会透出重影或整块消失。
#[test]
fn editing_node_keeps_its_box_but_not_its_glyphs() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let mut text = engine();

    let f = frame(&scene, &theme, 1.0, 1.0);
    let mut normal = build_layers(&f);
    text.paint(&f, &mut normal);

    let root = f.scene.find(&doc.root_id).expect("根节点");
    let mut editing_frame = frame(&scene, &theme, 1.0, 1.0);
    editing_frame.editing = Some(root);
    editing_frame.selected = vec![root];
    let mut editing = build_layers(&editing_frame);
    text.paint(&editing_frame, &mut editing);

    assert_eq!(
        editing[L_NODES].rects.len(),
        normal[L_NODES].rects.len(),
        "编辑只该静音文字，不能连盒子/标签一起丢掉"
    );
    assert!(
        text_count(&editing) < text_count(&normal),
        "编辑节点的字形仍然进了合成器：{} vs {}",
        text_count(&editing),
        text_count(&normal)
    );
}

/// 两行同字：第二行必须正好低一个行高。基线系数（0.72）在两行间抵消，
/// 这条测试钉的是「行间步进来自 box.line_height，而不是 DWrite 的 17.78」。
#[test]
fn second_line_sits_one_line_height_lower() {
    let mut doc = Document::new("两行");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root,
        text: "甲\n甲".into(),
    })
    .unwrap();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let f = frame(&scene, &theme, 1.0, 1.0);
    let mut layers = build_layers(&f);
    let mut text = engine();
    text.paint(&f, &mut layers);
    let texts = &layers[L_NODES].texts;
    assert_eq!(texts.len(), 2, "两行各一个字");
    let step = texts[1].pos[1] - texts[0].pos[1];
    let want = line_height(scene.node(0).geom.font_size);
    assert!(
        (step - want).abs() < 0.01,
        "行距 {step}，期望 {want}（不能是 DWrite 的 17.78）"
    );
    assert!(
        (texts[0].pos[0] - texts[1].pos[0]).abs() < 0.01,
        "两行应左对齐"
    );
}

/// 有文字与无文字两帧的差异必须全部落在文本四边形内。
/// 这条同时验证了「字形位图没有被位移/放大到盒子之外」。
#[test]
fn text_pixels_stay_inside_the_glyph_quads() {
    let mut c = match compositor() {
        Some(c) => c,
        None => return,
    };
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let f = frame(&scene, &theme, 1.0, 1.0);
    let mut text = engine();
    let mut with = build_layers(&f);
    let update = text.paint(&f, &mut with);
    c.upload_atlas(&update);
    let a = c.render(&f, &with, VIEWPORT);

    let without = build_layers(&f);
    let b = c.render(&f, &without, VIEWPORT);

    let mut mask = vec![false; (VIEWPORT[0] * VIEWPORT[1]) as usize];
    for inst in with.iter().flat_map(|l| l.texts.iter()) {
        let p0 = f.camera.world_to_phys(
            Point::new(inst.pos[0], inst.pos[1]),
            [VIEWPORT[0] as f32, VIEWPORT[1] as f32],
        );
        let p1 = f.camera.world_to_phys(
            Point::new(inst.pos[0] + inst.size[0], inst.pos[1] + inst.size[1]),
            [VIEWPORT[0] as f32, VIEWPORT[1] as f32],
        );
        let (x0, x1) = (
            (p0.x.floor() as i32 - 1).max(0),
            (p1.x.ceil() as i32 + 1).min(VIEWPORT[0] as i32 - 1),
        );
        let (y0, y1) = (
            (p0.y.floor() as i32 - 1).max(0),
            (p1.y.ceil() as i32 + 1).min(VIEWPORT[1] as i32 - 1),
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                mask[(y as u32 * VIEWPORT[0] + x as u32) as usize] = true;
            }
        }
    }

    let mut changed = 0usize;
    let mut outside = Vec::new();
    for y in 0..VIEWPORT[1] {
        for x in 0..VIEWPORT[0] {
            if a.pixel(x, y) != b.pixel(x, y) {
                changed += 1;
                if !mask[(y * VIEWPORT[0] + x) as usize] {
                    outside.push((x, y));
                }
            }
        }
    }
    assert!(
        changed > 1000,
        "只变了 {changed} 个像素，文字可能根本没画出来"
    );
    assert!(
        outside.is_empty(),
        "{} 个像素在文本四边形之外发生变化，前几个：{:?}",
        outside.len(),
        &outside[..outside.len().min(8)]
    );
}

/// 同一场景重绘两次：形状缓存复用、图集槽位不动，实例与像素都必须逐字节一致。
#[test]
fn repaint_is_byte_stable() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let f = frame(&scene, &theme, 1.25, 1.0);
    let mut text = engine();

    let mut first = build_layers(&f);
    let u1 = text.paint(&f, &mut first);
    let mut second = build_layers(&f);
    let u2 = text.paint(&f, &mut second);
    assert!(!u2.reset, "字形都在图集里，不该重排");
    assert!(u2.rects.is_empty(), "重复绘制不该产生新槽位");

    let bytes = |ls: &[LayerGeom; LAYERS]| {
        ls.iter()
            .flat_map(|l| l.texts.iter().flat_map(bytemuck::bytes_of))
            .copied()
            .collect::<Vec<u8>>()
    };
    assert_eq!(bytes(&first), bytes(&second), "同一帧两次排版实例不一致");
    assert_eq!(u1.generation + 1, u2.generation);

    if let Some(mut c) = compositor() {
        c.upload_atlas(&u1);
        let a = c.render(&f, &first, VIEWPORT);
        c.upload_atlas(&u2);
        let b = c.render(&f, &second, VIEWPORT);
        assert_eq!(a.data, b.data, "同帧两次渲染不一致");
    }
}

/// 四档系统缩放：文字都画出来、物理尺寸随 ppp 线性增长，并落四张图供人眼检查。
/// 结构上还确认每档都生成了自己的字形位图（ppd 是图集键的一部分）。
#[test]
fn text_is_sharp_at_100_125_150_200_percent() {
    let mut c = match compositor() {
        Some(c) => c,
        None => return,
    };
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let mut text = engine();
    let mut heights: Vec<(f32, u32)> = Vec::new();

    for ppp in [1.0f32, 1.25, 1.5, 2.0] {
        let f = frame(&scene, &theme, 1.0, ppp);
        let mut layers = build_layers(&f);
        let update = text.paint(&f, &mut layers);
        c.upload_atlas(&update);

        let with = c.render(&f, &layers, VIEWPORT);
        let cleared = build_layers(&f);
        let without = c.render(&f, &cleared, VIEWPORT);

        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        let mut changed = 0usize;
        for y in 0..VIEWPORT[1] {
            for x in 0..VIEWPORT[0] {
                if with.pixel(x, y) != without.pixel(x, y) {
                    changed += 1;
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        assert!(changed > 500, "ppp {ppp}：文字像素只有 {changed}");
        let h = y1 - y0 + 1;
        println!("ppp {ppp:.2} → 文字包围盒 {x0}..{x1} × {y0}..{y1}（高 {h}）");
        heights.push((ppp, h));

        let path = shots_dir()
            .join("render")
            .join(format!("text-ppp{ppp:.2}.png"));
        write_png(&path, &with.rgba(), with.width, with.height).unwrap();
        println!("{}", path.display());
    }

    // 整团文字的物理高度应随 ppp 近似线性；绘制的是同一份世界坐标。
    let (p1, h1) = heights[0];
    for (ppp, h) in heights.iter().skip(1) {
        let ratio = *h as f32 / h1 as f32;
        let want = *ppp / p1;
        assert!(
            (ratio - want).abs() <= want * 0.25,
            "ppp {ppp}：文字高度 {h}（基准 {h1}），比例 {ratio:.2} 偏离 {want:.2} 超过 25%"
        );
    }
}

/// 画布文本 golden：几何基线（`canvas-ppp1.00.png`）保持不变，这一张单独钉字形。
/// 重新生成：`MW_GOLDEN_UPDATE=1 cargo test -p mw-render -- text_canvas_matches_golden`。
#[test]
fn text_canvas_matches_golden() {
    let mut c = match compositor() {
        Some(c) => c,
        None => return,
    };
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let f = frame(&scene, &theme, 1.0, 1.0);
    let mut text = engine();
    let mut layers = build_layers(&f);
    let update = text.paint(&f, &mut layers);
    c.upload_atlas(&update);
    let img = c.render(&f, &layers, VIEWPORT);
    let actual = img.rgba();

    let path = shots_dir()
        .join("golden")
        .join(format!("canvas-text-ppp{:.2}.png", f.camera.ppp));
    if std::env::var("MW_GOLDEN_UPDATE").is_ok() {
        write_png(&path, &actual, img.width, img.height).unwrap();
        println!("[golden] ✓ 文本基线已写入 {}", path.display());
        return;
    }
    let (golden, gw, gh) = read_png(&path).expect("文本基线缺失，先用 MW_GOLDEN_UPDATE=1 生成");
    assert_eq!(
        (gw, gh),
        (img.width, img.height),
        "文本基线尺寸漂移：基线 {gw}×{gh}，当前 {}×{}",
        img.width,
        img.height
    );
    let drift = Drift::measure(&actual, &golden, img.width, img.height, 0);
    if drift.bad > 0 {
        let dir = path.parent().unwrap();
        let name = path.file_name().unwrap();
        for (sub, bytes) in [
            ("actual", &actual),
            ("diff", &drift.heatmap(&actual, &golden)),
        ] {
            write_png(&dir.join(sub).join(name), bytes, img.width, img.height).ok();
        }
        panic!(
            "[golden] ✗ 文本画布漂移：{drift}；actual/diff 已写到 {}",
            dir.display()
        );
    }
}

/// 像素是否就是某个令牌色（允许抗锯齿边缘的偏差，对齐 `offscreen.rs::is_close`）。
fn is_token(p: [u8; 4], c: Rgba) -> bool {
    p[3] >= 200 && (0..3).all(|i| (p[i] as i32 - c.0[i] as i32).abs() <= 24)
}

/// 所有实例（含字形四边形）的世界包围盒，外扩 `pad`。判「空白像素」只能靠几何：
/// 挖空字用的就是画布底色，颜色上跟真背景分不开。
fn bounds_of(pts: impl Iterator<Item = (f32, f32)>, pad: f32) -> mw_core::Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    mw_core::Rect::new(x0 - pad, y0 - pad, x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad)
}

fn phys(f: &Frame, x: f32, y: f32) -> (u32, u32) {
    let p = f
        .camera
        .world_to_phys(Point::new(x, y), [VIEWPORT[0] as f32, VIEWPORT[1] as f32]);
    (
        p.x.round().clamp(0.0, VIEWPORT[0] as f32 - 1.0) as u32,
        p.y.round().clamp(0.0, VIEWPORT[1] as f32 - 1.0) as u32,
    )
}

/// 透明底导出（M4）：清屏清成 alpha=0，**主题令牌一律不动**。
///
/// `theme.ui.canvas_bg` 兼着挖空色——标签胶囊里的字、关系线标签的底、折叠圆点的底都拿它
/// 当橡皮擦。早先的写法是导出前把这条令牌改成 `TRANSPARENT`，于是透明 PNG 里标签胶囊成了
/// 空壳（1× 导出里的「验收」标签，docs/14 教训 45）。这条测试把两件事同时钉住：
/// 底色真的清成 (0,0,0,0)，而挖空色照旧不透明。
#[test]
fn transparent_export_clears_the_background_but_keeps_the_knockout_color() {
    let mut c = match compositor() {
        Some(c) => c,
        None => return,
    };
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let mut text = engine();

    // 「中文Rust混排」带一个「重要」标签胶囊。镜头对准胶囊本身：3× 让字形有实心笔画，
    // ppp=1 与导出那条路一致（导出用 zoom=比例、ppp=1）。
    let pill_owner = scene.find(&id_of(&doc, "中文Rust混排")).expect("标签宿主");
    let n = scene.node(pill_owner);
    let pill = n.labels.first().expect("这个节点带一个标签胶囊");
    let chip = mw_core::Rect::new(
        n.geom.rect.x + pill.rect.x,
        n.geom.rect.y + pill.rect.y,
        pill.rect.w,
        pill.rect.h,
    );
    let mut f = frame(&scene, &theme, 3.0, 1.0);
    f.camera = Camera::new(chip.center(), 3.0, 1.0);
    let mut layers = build_layers(&f);
    let update = text.paint(&f, &mut layers);
    c.upload_atlas(&update);

    let opaque = c.render(&f, &layers, VIEWPORT);
    c.set_transparent(true);
    let clear = c.render(&f, &layers, VIEWPORT);
    c.set_transparent(false);

    // 「空白」要按几何认，不能按颜色认：挖空字用的就是底色，颜色上跟背景一模一样。
    let all = bounds_of(
        layers.iter().flat_map(|l| {
            l.rects
                .iter()
                .flat_map(|r| {
                    [
                        (r.center[0] - r.half[0], r.center[1] - r.half[1]),
                        (r.center[0] + r.half[0], r.center[1] + r.half[1]),
                    ]
                })
                .chain(
                    l.segs
                        .iter()
                        .flat_map(|s| [(s.a[0], s.a[1]), (s.b[0], s.b[1])]),
                )
                .chain(l.tris.iter().flat_map(|t| {
                    [
                        (t.p[0][0], t.p[0][1]),
                        (t.p[1][0], t.p[1][1]),
                        (t.p[2][0], t.p[2][1]),
                    ]
                }))
                .chain(l.texts.iter().flat_map(|t| {
                    [
                        (t.pos[0], t.pos[1]),
                        (t.pos[0] + t.size[0], t.pos[1] + t.size[1]),
                    ]
                }))
        }),
        6.0,
    );

    // 1) 不透明那张：空白处必须是清屏写下的原值（一个半透明像素都不该有）；
    //    2) 透明那张：同一批像素必须清成 (0,0,0,0)——RGB 也要清，留着会糊出浅边。
    let mut empty = 0usize;
    for y in 0..VIEWPORT[1] {
        for x in 0..VIEWPORT[0] {
            let world = f.camera.screen_to_world(
                Point::new(x as f32 + 0.5, y as f32 + 0.5),
                [VIEWPORT[0] as f32, VIEWPORT[1] as f32],
            );
            if all.contains(world) {
                continue;
            }
            assert_eq!(
                opaque.pixel(x, y),
                theme.ui.canvas_bg.0,
                "空白处应当是清屏底色：({x},{y})"
            );
            assert_eq!(clear.pixel(x, y), [0, 0, 0, 0], "透明底没清干净：({x},{y})");
            empty += 1;
        }
    }
    assert!(empty > 1000, "只找到 {empty} 个空白像素，镜头没落在空白处");

    // 3) 胶囊**内部**（收进一圈，避开节点盒的底）：挖空字仍然是不透明的画布底色。
    //    胶囊自己盖住了节点盒，所以这里除字形以外不该有别的浅色像素。
    let inset = chip.w.min(chip.h) * 0.15;
    let (x0, y0) = phys(&f, chip.x + inset, chip.y + inset);
    let (x1, y1) = phys(&f, chip.x + chip.w - inset, chip.y + chip.h - inset);
    let mut glyph = 0usize;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if is_token(clear.pixel(x, y), theme.ui.canvas_bg) {
                glyph += 1;
            }
        }
    }
    assert!(
        glyph > 20,
        "标签胶囊「{}」的字在透明导出里没了：内部只数到 {glyph} 个挖空像素",
        pill.text
    );
}
