//! M1b-1 出口判据：`Scene` 的几何真的落到像素上，并且「铬件恒定 / 形状随缩放」这条
//! 分界在像素上成立。跑不了 GPU 的机器（既无 Vulkan 也无 WGL）会打印原因后跳过像素项，
//! 纯 CPU 的实例断言始终执行。

use mw_core::layout::toggle_pos;
use mw_core::text::MonoMeasurer;
use mw_core::theme::{get_theme, Rgba};
use mw_core::{build_scene, Command, Document, LayoutSettings, Point, Rect, Scene, Theme};
use mw_golden::{read_png, write_png, Drift};
use mw_render::instances::{
    rgba, Frame, Ghost, GuideLine, LayerGeom, RectInst, DROP_BAD, LAYERS, L_BACKDROP, L_LINES,
    L_NODES, L_OVERLAY, SELECT_RING_GAP, SELECT_RING_W, TOGGLE_HALO, TOGGLE_R, TOGGLE_W,
};
use mw_render::{build_layers, Camera, Compositor, Image};

const VIEWPORT: [u32; 2] = [900, 600];

fn id_of(doc: &Document, text: &str) -> String {
    doc.doc_order()
        .into_iter()
        .find(|id| doc.node(id).map_or(false, |n| n.text == text))
        .unwrap_or_else(|| panic!("文档里没有「{text}」"))
}

/// 一个把四层都用上的小文档：节点、连线、折叠圆点、标签胶囊、边界、概要、关系线、备注角标。
fn fixture() -> Document {
    let mut doc = Document::new("中心主题");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "中心主题".into(),
    })
    .unwrap();
    for t in ["分支一", "分支二", "分支三"] {
        doc.apply(&Command::AddNode {
            parent: root.clone(),
            index: None,
            text: t.into(),
            style: None,
        })
        .unwrap();
    }
    let b1 = id_of(&doc, "分支一");
    for t in ["叶子甲", "叶子乙"] {
        doc.apply(&Command::AddNode {
            parent: b1.clone(),
            index: None,
            text: t.into(),
            style: None,
        })
        .unwrap();
    }
    let leaf = id_of(&doc, "叶子甲");
    doc.apply(&Command::AddLabel {
        ids: vec![leaf.clone()],
        label: "重要".into(),
    })
    .unwrap();
    doc.apply(&Command::SetNote {
        id: leaf.clone(),
        note: Some("备注文字".into()),
    })
    .unwrap();
    doc.apply(&Command::AddRelationship {
        from: b1.clone(),
        to: id_of(&doc, "分支三"),
        label: Some("关联".into()),
    })
    .unwrap();
    doc.apply(&Command::AddBoundary {
        parent: root.clone(),
        targets: vec![id_of(&doc, "分支二")],
        color: None,
    })
    .unwrap();
    doc.apply(&Command::AddSummary {
        parent: root.clone(),
        targets: vec![b1, id_of(&doc, "分支二")],
        text: "概要".into(),
    })
    .unwrap();
    doc
}

fn scene_of(doc: &Document, theme: &Theme) -> Scene {
    build_scene(
        doc,
        &doc.root_id,
        &LayoutSettings::default(),
        theme,
        &MonoMeasurer,
        None::<(&str, &str)>,
    )
}

fn frame<'a>(
    scene: &'a Scene,
    theme: &'a Theme,
    center: Point,
    zoom: f32,
    selected: u32,
) -> Frame<'a> {
    Frame {
        scene,
        theme,
        camera: Camera::new(center, zoom, 1.0),
        selected: vec![selected],
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport: None,
    }
}

#[test]
fn every_layer_gets_instances() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let layers = build_layers(&frame(
        &scene,
        &theme,
        Point::ZERO,
        1.0,
        scene.find(&doc.root_id).unwrap(),
    ));

    // 节点盒 + 标签胶囊（1 个）+ 概要主题盒（1 个）
    assert_eq!(layers[L_NODES].rects.len(), scene.nodes.len() + 2);
    assert_eq!(layers[L_LINES].rects.len(), scene.boundaries.len());
    // 连线 + 花括号 + 概要连线 + 关系线；曲线还会按屏幕容差再细分，所以只能比下界
    assert!(layers[L_LINES].segs.len() > scene.edges.len());
    assert_eq!(layers[L_OVERLAY].tris.len(), scene.relationships.len());
    assert!(!layers[L_OVERLAY].rects.is_empty(), "选中环/圆点/角标");
    assert!(
        layers[L_BACKDROP].is_empty(),
        "L0 点阵是全屏三角形，没有实例"
    );
}

/// 折叠点是「底色圆 + 描边环」两个实例，不是实心点：
/// 底色必须先画、比环大 `TOGGLE_HALO`，环本身无填充。命中半径要盖住底色圆。
#[test]
fn collapse_toggle_is_a_haloed_ring() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();
    let f = frame(
        &scene,
        &theme,
        scene.node(root).geom.rect.center(),
        1.0,
        root,
    );
    let layers = build_layers(&f);
    let overlay = &layers[L_OVERLAY];

    // 分支一有父、有孩子：只有这种节点才画折叠点（根节点没有父，不画）。
    let idx = scene.find(&id_of(&doc, "分支一")).unwrap();
    let n = scene.node(idx);
    let parent_x = scene.node(n.parent.unwrap()).geom.rect.x;
    let local = toggle_pos(n.geom.rect, scene.kind, Some(parent_x));
    let c = Point::new(n.geom.rect.x + local.x, n.geom.rect.y + local.y);

    let mut at: Vec<&RectInst> = overlay
        .rects
        .iter()
        .filter(|r| (r.center[0] - c.x).abs() < 1e-3 && (r.center[1] - c.y).abs() < 1e-3)
        .collect();
    assert_eq!(at.len(), 2, "折叠点应由底色圆与描边环两个实例组成");
    at.sort_by(|a, b| b.half[0].partial_cmp(&a.half[0]).unwrap());

    let (halo, ring) = (at[0], at[1]);
    let r = f.camera.chrome(TOGGLE_R, 4.5);
    assert!(
        (halo.half[0] - (r + TOGGLE_HALO)).abs() < 1e-4,
        "底色圆应外扩 TOGGLE_HALO"
    );
    assert_eq!(halo.params[1], 0.0, "底色圆不描边");
    assert!(halo.fill[3] > 0.9, "底色圆必须不透明才能挖掉连线");
    assert_eq!(halo.stroke[3], 0.0);
    assert!((ring.half[0] - r).abs() < 1e-4);
    assert!((ring.params[1] - f.camera.css_to_world(TOGGLE_W)).abs() < 1e-4);
    assert_eq!(ring.fill[3], 0.0, "环必须无填充，否则又退回实心点");
    assert!(ring.stroke[3] > 0.9, "环的描边应是分支色");
    // 「可见范围 <= 命中范围」由 `hit.rs` 的编译期断言钉住，这里不重复。
}

#[test]
fn instances_are_a_pure_function_of_the_frame() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let bytes = |ls: &[LayerGeom; LAYERS]| {
        ls.iter()
            .flat_map(|l| {
                l.rects.iter().flat_map(|r| bytemuck::bytes_of(r)).chain(
                    l.segs
                        .iter()
                        .flat_map(|s| bytemuck::bytes_of(s))
                        .chain(l.tris.iter().flat_map(|t| bytemuck::bytes_of(t))),
                )
            })
            .copied()
            .collect::<Vec<u8>>()
    };
    let a = build_layers(&frame(&scene, &theme, Point::ZERO, 1.25, 0));
    let b = build_layers(&frame(&scene, &theme, Point::ZERO, 1.25, 0));
    assert_eq!(bytes(&a), bytes(&b), "同样的输入必须得到逐字节相同的实例");
}

/// 视口裁剪（M1c）：视口外的盒不产实例，留下的每个矩形都与可见世界相交。
#[test]
fn culling_drops_geometry_outside_the_viewport() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();
    let center = scene.node(root).geom.rect.center();
    let viewport = Some([220.0f32, 160.0]);
    let f = Frame {
        scene: &scene,
        theme: &theme,
        camera: Camera::new(center, 1.0, 1.0),
        selected: vec![root],
        hovered: Some(root),
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport,
    };
    let culled = build_layers(&f);
    let all = build_layers(&frame(&scene, &theme, center, 1.0, root));
    assert!(
        culled[L_NODES].rects.len() < all[L_NODES].rects.len(),
        "小视口应该裁掉节点：{} vs {}",
        culled[L_NODES].rects.len(),
        all[L_NODES].rects.len()
    );
    assert!(
        culled[L_LINES].segs.len() < all[L_LINES].segs.len(),
        "视口外的连线也该被裁掉"
    );

    let view = f.view_world().unwrap();
    for r in &culled[L_NODES].rects {
        let rect = Rect::new(
            r.center[0] - r.half[0],
            r.center[1] - r.half[1],
            r.half[0] * 2.0,
            r.half[1] * 2.0,
        );
        assert!(
            view.intersects(rect),
            "裁剪后仍有矩形落在可见世界外：{rect:?} / {view:?}"
        );
    }
    // 没给视口就是全图模式（导出/离屏基线），数量必须回到完整实例数
    assert_eq!(all[L_NODES].rects.len(), scene.nodes.len() + 2);
}

/// 没有 GPU 适配器时跳过像素项：harness 要在任何机器上都能跑完其余部分。
fn compositor() -> Option<Compositor> {
    match Compositor::new() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("跳过像素断言：{e}");
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Scan {
    /// 盒右边界（描边外沿）到节点中心的物理像素距离
    fill_end: u32,
    /// 选中环实心段宽度
    ring: usize,
    /// 环起点到盒边界的物理像素距离
    gap: i32,
}

/// 从节点中心向右逐像素扫描：先出盒（形状），再遇到强调色的环（铬件）。
///
/// 调用方要保证这个节点右侧没有别的元素、且它的描边不是强调色，否则量到的
/// 是描边或标签胶囊而不是环——snow 主题的 1 号分支色恰好就等于强调色，所以
/// 选节点要避开 palette[0]。
fn scan(img: &Image, x0: u32, y: u32, fill: Rgba, bg: Rgba, accent: Rgba) -> Option<Scan> {
    let mut fill_end = None;
    let mut ring_start = None;
    let mut ring = 0usize;
    for dx in 0..260u32 {
        let p = img.pixel(x0 + dx, y);
        if fill_end.is_none() {
            // 盒内是填充色，盒外是画布底色，两者都算「还没到边界」
            if is_close(p, fill) || is_close(p, bg) {
                continue;
            }
            fill_end = Some(dx);
            continue;
        }
        if is_close(p, accent) {
            if ring_start.is_none() {
                ring_start = Some(dx);
            }
            ring += 1;
        } else if ring_start.is_some() {
            break;
        }
    }
    let (fill_end, ring_start) = (fill_end?, ring_start?);
    Some(Scan {
        fill_end,
        ring,
        gap: ring_start as i32 - fill_end as i32,
    })
}

#[inline]
fn is_close(p: [u8; 4], c: Rgba) -> bool {
    p[3] >= 200 && (0..3).all(|i| (p[i] as i32 - c.0[i] as i32).abs() <= 24)
}

/// 选中环：宽 2px、离节点 2.5px，都是 CSS 常量，任何缩放档位都一样；
/// 而盒本身的边界离中心的距离随缩放线性放大。docs/12 §3.1 那条分界就靠这一项验收。
#[test]
fn chrome_stays_constant_while_shapes_scale() {
    let mut c = match compositor() {
        Some(c) => c,
        None => return,
    };
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    // 分支三：3 号分支色（绿），右侧没有标签、备注、也没有别的节点
    let probe = scene.find(&id_of(&doc, "分支三")).unwrap();
    let node = scene.node(probe);
    let rect = node.geom.rect;
    let fill = node.style.fill;
    assert!(
        !is_close(node.style.stroke.0, theme.ui.accent),
        "探针节点的描边 {:?} 不能撞上强调色 {:?}，否则量到的是描边不是环",
        node.style.stroke.0,
        theme.ui.accent.0
    );

    let mut got: Vec<(f32, Scan)> = Vec::new();
    for zoom in [1.0f32, 3.0] {
        let f = frame(&scene, &theme, rect.center(), zoom, probe);
        let layers = build_layers(&f);
        let img = c.render(&f, &layers, VIEWPORT);
        // 相机把节点中心对准视口中心，ppp = 1
        let x0 = (VIEWPORT[0] as f32 * 0.5) as u32;
        let y = (VIEWPORT[1] as f32 * 0.5) as u32;
        let center = img.pixel(x0, y);
        assert!(
            is_close(center, fill),
            "缩放 {zoom}：节点中心应该是填充色，实测 {center:?} 期望 {:?}（文本层还没接，这里不该有别的元素）",
            fill.0
        );
        let bg = img.pixel(VIEWPORT[0] - 2, VIEWPORT[1] - 2);
        assert!(
            is_close(bg, theme.ui.canvas_bg),
            "缩放 {zoom}：画布角落应该是底色，实测 {bg:?} 期望 {:?}",
            theme.ui.canvas_bg.0
        );
        let s = scan(&img, x0, y, fill, theme.ui.canvas_bg, theme.ui.accent)
            .unwrap_or_else(|| panic!("缩放 {zoom}：没扫到盒边界或选中环"));
        println!("zoom {zoom} → {s:?}");
        got.push((zoom, s));
    }

    let (z1, s1) = got[0];
    let (z3, s3) = got[1];
    // 铬件：环宽与间隙只跟 CSS px 有关
    for (zoom, s) in &got {
        assert!(
            (1..=4).contains(&s.ring),
            "缩放 {zoom}：环的实心宽度应该 ≈{SELECT_RING_W}px，实测 {}",
            s.ring
        );
        assert!(
            (s.gap - SELECT_RING_GAP as i32).abs() <= 2,
            "缩放 {zoom}：环离盒边 {}px，应该 ≈{SELECT_RING_GAP}px",
            s.gap
        );
    }
    assert_eq!(s1.ring, s3.ring, "环宽不该随缩放变化");
    // 形状：盒右边界离中心的距离 = 半宽 × 缩放
    let expect = |zoom: f32| (rect.w * 0.5 * zoom * 1.0) as i32;
    assert!(
        (s1.fill_end as i32 - expect(z1)).abs() <= 3,
        "缩放 {z1}：盒边界 {}，期望 ≈{}",
        s1.fill_end,
        expect(z1)
    );
    assert!(
        (s3.fill_end as i32 - expect(z3)).abs() <= 3,
        "缩放 {z3}：盒边界 {}，期望 ≈{}",
        s3.fill_end,
        expect(z3)
    );
}

/// 同一帧连渲两次必须逐像素相同——golden 基线靠这一点才敢设容差 0。
/// 顺带把三档缩放的结果落到 `shots/render/`，给人眼检查留一张图。
#[test]
fn rendering_is_deterministic_and_dumps_a_picture() {
    let mut c = match compositor() {
        Some(c) => c,
        None => return,
    };
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();

    let mut shots: Vec<(f32, Image)> = Vec::new();
    for zoom in [1.0f32, 1.5, 3.0] {
        let f = frame(
            &scene,
            &theme,
            scene.node(root).geom.rect.center(),
            zoom,
            root,
        );
        let img = c.render(&f, &build_layers(&f), VIEWPORT);
        let again = c.render(&f, &build_layers(&f), VIEWPORT);
        assert_eq!(
            img.data, again.data,
            "缩放 {zoom}：两次渲染不一致，golden 基线无法用容差 0"
        );
        shots.push((zoom, img));
    }

    for (zoom, img) in shots {
        let path = shots_dir()
            .join("render")
            .join(format!("offscreen-zoom{zoom:.1}.png"));
        write_png(&path, &img.rgba(), img.width, img.height).unwrap();
        println!("{}", path.display());
    }
}

fn shots_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shots");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 画布基线：与 `mw-shell` 的 harness 同一套口径（`mw_golden`），只是捕获源从
/// `egui::Context::request_screenshot` 换成我们自己的纹理——M1b-1 的出口判据。
///
/// 基线此刻还没有文字（文本层是 M1b-2），所以 M1b-2 落地后这一张 diff 应当
/// 「只多出字形」，那正是它现在要先定下来的理由。
/// 重新生成：`MW_GOLDEN_UPDATE=1 cargo test -p mw-render -- canvas_matches_golden_baseline`。
#[test]
fn canvas_matches_golden_baseline() {
    let mut c = match compositor() {
        Some(c) => c,
        None => return,
    };
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();
    let f = frame(
        &scene,
        &theme,
        scene.node(root).geom.rect.center(),
        1.0,
        root,
    );
    let img = c.render(&f, &build_layers(&f), VIEWPORT);
    let actual = img.rgba();

    // 文件名带 ppp：跨缩放档次的像素不可能相同（与 shell-ppp1.50.png 同一约定）
    let path = shots_dir()
        .join("golden")
        .join(format!("canvas-ppp{:.2}.png", f.camera.ppp));

    if std::env::var("MW_GOLDEN_UPDATE").is_ok() {
        write_png(&path, &actual, img.width, img.height).unwrap();
        println!("[golden] ✓ 画布基线已写入 {}", path.display());
        return;
    }
    let (golden, gw, gh) = read_png(&path).expect("画布基线缺失，先用 MW_GOLDEN_UPDATE=1 生成");
    assert_eq!(
        (gw, gh),
        (img.width, img.height),
        "画布基线尺寸漂移：基线 {gw}×{gh}，当前 {}×{}",
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
            "[golden] ✗ 画布漂移：{drift}；actual/diff 已写到 {}",
            dir.display()
        );
    }
}

/// 拖拽幽灵：整棵子树按偏移多画一份（半透明），原位留淡化残影，并集轮廓进覆盖层。
#[test]
fn a_ghost_copies_the_subtree_and_fades_it_in_place() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();
    let base = build_layers(&frame(&scene, &theme, Point::ZERO, 1.0, root));

    let mut f = frame(&scene, &theme, Point::ZERO, 1.0, root);
    f.ghost = Some(Ghost {
        nodes: vec![root],
        offset: Point::new(60.0, 24.0),
        illegal: false,
    });
    let now = build_layers(&f);

    // 根节点没有标签，所以正好只多一个盒子。
    assert_eq!(
        now[L_NODES].rects.len(),
        base[L_NODES].rects.len() + 1,
        "幽灵要多画一份节点盒"
    );

    let at = |layer: &LayerGeom, p: Point| {
        layer
            .rects
            .iter()
            .find(|r| (r.center[0] - p.x).abs() < 0.01 && (r.center[1] - p.y).abs() < 0.01)
            .copied()
    };
    let c = scene.rect(root).center();
    let before = at(&base[L_NODES], c).expect("原位有根的盒子");
    let after = at(&now[L_NODES], c).expect("原位仍在（只是淡了）");
    assert!(
        after.fill[3] < before.fill[3],
        "原位要留淡化残影，整块消失会像「节点被删了」"
    );

    let copy = at(&now[L_NODES], Point::new(c.x + 60.0, c.y + 24.0)).expect("幽灵盒要跟着偏移");
    assert!((copy.fill[3] - 0.55).abs() < 0.01, "幽灵约 55% 不透明");
}

/// 多节点幽灵才套并集虚线框；单个节点套一圈只会跟落点指示抢注意力。
#[test]
fn only_a_multi_node_ghost_gets_a_union_outline() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();
    let child = scene.find(&id_of(&doc, "分支一")).unwrap();
    let base = build_layers(&frame(&scene, &theme, Point::ZERO, 1.0, root));

    let mut one = frame(&scene, &theme, Point::ZERO, 1.0, root);
    one.ghost = Some(Ghost {
        nodes: vec![child],
        offset: Point::new(20.0, 0.0),
        illegal: false,
    });
    let single = build_layers(&one);
    assert_eq!(
        single[L_OVERLAY].rects.len(),
        base[L_OVERLAY].rects.len(),
        "单节点幽灵不画并集框"
    );

    let mut two = frame(&scene, &theme, Point::ZERO, 1.0, root);
    two.ghost = Some(Ghost {
        nodes: vec![root, child],
        offset: Point::new(20.0, 0.0),
        illegal: false,
    });
    let multi = build_layers(&two);
    assert_eq!(
        multi[L_OVERLAY].rects.len(),
        base[L_OVERLAY].rects.len() + 1,
        "多节点幽灵多一个并集虚线框"
    );
}

/// 非法落点：幽灵轮廓转警示色，而不是悄悄不画（docs/06 §5）。
#[test]
fn an_illegal_ghost_turns_its_outline_red() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();
    let mut f = frame(&scene, &theme, Point::ZERO, 1.0, root);
    let child = scene.find(&id_of(&doc, "分支一")).unwrap();
    f.ghost = Some(Ghost {
        // 用两根：并集轮廓只在多节点时才画（见上一个测试）。
        nodes: vec![root, child],
        offset: Point::new(10.0, 10.0),
        illegal: true,
    });
    let now = build_layers(&f);
    let bad = rgba(DROP_BAD);
    let outline = now[L_OVERLAY]
        .rects
        .iter()
        .find(|r| r.params[2] > 0.0 && r.stroke[..3] == bad[..3])
        .expect("非法幽灵要有一条警示色虚线");
    assert!(outline.stroke[3] > 0.0);
}

/// 对齐参考线：两条线进覆盖层，线宽是屏幕空间常量（1 CSS px ÷ zoom）。
#[test]
fn guides_are_screen_constant_hairlines() {
    let doc = fixture();
    let theme = get_theme("snow");
    let scene = scene_of(&doc, &theme);
    let root = scene.find(&doc.root_id).unwrap();
    let base = build_layers(&frame(&scene, &theme, Point::ZERO, 1.0, root));

    let mut f = frame(&scene, &theme, Point::ZERO, 1.0, root);
    f.guides = vec![
        GuideLine {
            at: 100.0,
            from: 0.0,
            to: 200.0,
            vertical: true,
        },
        GuideLine {
            at: 50.0,
            from: 0.0,
            to: 200.0,
            vertical: false,
        },
    ];
    let now = build_layers(&f);
    assert_eq!(
        now[L_OVERLAY].segs.len(),
        base[L_OVERLAY].segs.len() + 2,
        "两条参考线各一段"
    );

    let added = &now[L_OVERLAY].segs[base[L_OVERLAY].segs.len()..];
    assert!((added[0].meta[0] - 1.0).abs() < 1e-5, "100% 缩放下线宽 1");
    assert_eq!(
        (added[0].a, added[0].b),
        ([100.0, 0.0], [100.0, 200.0]),
        "竖线按 x=at 画"
    );
    assert_eq!(
        (added[1].a, added[1].b),
        ([0.0, 50.0], [200.0, 50.0]),
        "横线按 y=at 画"
    );

    // 放大 2 倍时世界单位线宽减半——屏幕上还是 1px。
    let scaled = build_layers(&frame(&scene, &theme, Point::ZERO, 2.0, root));
    let _ = scaled;
    let mut g2 = frame(&scene, &theme, Point::ZERO, 2.0, root);
    g2.guides = f.guides.clone();
    let at2 = build_layers(&g2);
    let last = at2[L_OVERLAY].segs.last().unwrap();
    assert!((last.meta[0] - 0.5).abs() < 1e-5, "200% 下世界单位线宽 0.5");
}
