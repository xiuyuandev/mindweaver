//! Scene → 实例的每帧成本（docs/08 P5 帧预算里属于我们的那一段）。
//!
//! `cargo run -p mw-render --release --example perf_layers`
//! `cargo run -p mw-render --release --example perf_layers -- --frame`
//!
//! 默认测 **CPU 半边**：`build_layers`（含 M1c 视口裁剪）与 `TextEngine::paint`，
//! 视口固定 1600×900，对照「不裁剪」的旧口径。`--frame` 追加真实全帧：离屏合成器
//! 提交 + `wait_idle`，包含 GPU 执行时间——这是 P5 ≥55fps 的验收口径。
//!
//! 帧预算怎么读：真机是 CPU 与 GPU 流水重叠，帧时间 ≈ max(CPU, GPU)；这里每帧
//! 串行等待，所以「合计」是上界。55fps 的预算是 18.18 ms。

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use mw_core::text::MonoMeasurer;
use mw_core::{
    build_scene, get_theme, DocMeta, Document, LayoutSettings, Measured, Node, Scene, TextMeasurer,
};
use mw_render::dwrite::{DWrite, DWriteMeasurer};
use mw_render::instances::Frame;
use mw_render::{build_layers, Camera, Compositor, TextEngine};

const VIEWPORT: [f32; 2] = [1600.0, 900.0];
/// P5：5,000 节点、80% 可见，≥55fps。
const P5_BUDGET_MS: f64 = 1000.0 / 55.0;

struct Noop;

impl TextMeasurer for Noop {
    fn measure(&self, text: &str, font_size: f32, _weight: u16, _max_width: f32) -> Measured {
        Measured {
            width: (text.chars().count() as f32) * font_size,
            lines: vec![text.to_string()],
            line_height: (font_size * 1.45).round(),
        }
    }
}

fn synth(count: usize) -> Document {
    const WIDTHS: [usize; 4] = [6, 8, 8, 5];
    let mut nodes: HashMap<String, Node> = HashMap::new();
    let mut root = Node::new("n0".into(), None, "中心主题");
    root.modified_at = 1;
    nodes.insert("n0".to_string(), root);

    let mut queue = VecDeque::new();
    queue.push_back(("n0".to_string(), 0usize));
    let mut next = 1usize;
    while next < count {
        let Some((parent, depth)) = queue.pop_front() else {
            break;
        };
        for _ in 0..WIDTHS[depth % WIDTHS.len()] {
            if next >= count {
                break;
            }
            let id = format!("n{next}");
            let mut n = Node::new(
                id.clone(),
                Some(parent.clone()),
                &format!("分支节点 {next}"),
            );
            n.modified_at = 1;
            if next % 40 == 0 {
                n.labels = vec!["重要".into()];
            }
            if next % 97 == 0 {
                n.icon = Some("priority-1".into());
            }
            nodes.get_mut(&parent).unwrap().children.push(id.clone());
            nodes.insert(id.clone(), n);
            queue.push_back((id, depth + 1));
            next += 1;
        }
    }
    Document::from_parts(None, DocMeta::default(), "n0".into(), nodes).expect("合成文档必须合法")
}

fn report(name: &str, runs: &[Duration], budget_ms: f64) {
    let mut ms: Vec<f64> = runs.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(|a, b| a.total_cmp(b));
    let pct = |p: f64| ms[((ms.len() as f64 - 1.0) * p) as usize];
    let (min, med, p95) = (ms[0], pct(0.5), pct(0.95));
    let verdict = if med <= budget_ms {
        "达标"
    } else {
        "超预算"
    };
    println!(
        "{name:<26}{min:6.2}  中位 {med:6.2}  p95 {p95:6.2} ms   预算 {budget_ms:.1} ms → {verdict}"
    );
}

fn measure(name: &str, runs: usize, budget_ms: f64, mut op: impl FnMut()) {
    let mut times = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        op();
        times.push(t.elapsed());
    }
    report(&format!("  {name}"), &times, budget_ms);
}

fn main() {
    let theme = get_theme("snow");
    let st = LayoutSettings {
        kind: "mindmap".into(),
        ..Default::default()
    };
    let frame_mode = std::env::args().any(|a| a == "--frame");

    text_costs(&theme, &st);

    for &n in &[500usize, 2000, 5000] {
        let doc = synth(n);
        let scene = build_scene(&doc, &doc.root_id, &st, &theme, &Noop, None);
        let center = scene.bounds.center();
        // 让整棵树落进 1600×900 的视口：缩到多小，取决于包围盒多大
        let fit = (scene.bounds.w / VIEWPORT[0])
            .max(scene.bounds.h / VIEWPORT[1])
            .max(1.0);
        let fit_zoom = 1.0 / fit;
        let z80 = zoom_for_fraction(&scene, &theme, center, 0.80);

        println!(
            "\n{n} 节点：{} 连线，包围盒 {:.0}×{:.0}，全图缩放 {fit_zoom:.4}，80% 可见缩放 {z80:.4}",
            scene.edges.len(),
            scene.bounds.w,
            scene.bounds.h
        );

        let mut text = TextEngine::new().expect("DirectWrite 必须可用");
        for (label, zoom) in [("100%", 1.0f32), ("80% 可见", z80), ("全图", fit_zoom)] {
            let culled = frame_at(&scene, &theme, center, zoom, Some(VIEWPORT));
            println!(
                "  {label} z={zoom:.4}：可见 {} / {} 节点",
                visible_nodes(&scene, &culled),
                scene.nodes.len()
            );
            dump_counts(&culled);
            measure("build_layers 不裁剪", 21, 4.0, || {
                std::hint::black_box(build_layers(&frame_at(&scene, &theme, center, zoom, None)));
            });
            measure("build_layers 裁剪", 21, 4.0, || {
                std::hint::black_box(build_layers(&culled));
            });

            let mut layers = build_layers(&culled);
            for _ in 0..3 {
                text.paint(&culled, &mut layers);
            }
            measure("build+paint 裁剪", 21, 8.0, || {
                let mut layers = build_layers(&culled);
                text.paint(&culled, &mut layers);
                std::hint::black_box(&layers);
            });
        }
    }

    if frame_mode {
        frame_cost(&theme, &st);
    }
}

/// 二分搜索「可见比例 ≈ want」的缩放（对数空间，裁剪外扩随缩放变化也一并算进去）。
fn zoom_for_fraction(
    scene: &Scene,
    theme: &mw_core::Theme,
    center: mw_core::Point,
    want: f32,
) -> f32 {
    let (mut lo, mut hi) = (0.0005f32, 4.0f32);
    for _ in 0..40 {
        let mid = (lo * hi).sqrt();
        let f = frame_at(scene, theme, center, mid, Some(VIEWPORT));
        let frac = visible_nodes(scene, &f) as f32 / scene.nodes.len() as f32;
        // 放大 → 可见更少
        if frac > want {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

fn visible_nodes(scene: &Scene, f: &Frame) -> usize {
    scene.nodes.iter().filter(|n| f.sees(n.geom.rect)).count()
}

/// 全帧：真实 DWrite 文本 + 合成器提交 + `wait_idle`。这是 P5 的验收口径。
fn frame_cost(theme: &mw_core::Theme, st: &LayoutSettings) {
    let doc = synth(5000);
    let Some(dw) = DWrite::new() else {
        println!("\n跳过全帧：DirectWrite 不可用");
        return;
    };
    let measurer = DWriteMeasurer::new(dw);
    let scene = build_scene(&doc, &doc.root_id, st, theme, &measurer, None);
    let center = scene.bounds.center();
    let fit = (scene.bounds.w / VIEWPORT[0])
        .max(scene.bounds.h / VIEWPORT[1])
        .max(1.0);
    let z80 = zoom_for_fraction(&scene, theme, center, 0.80);

    let mut compositor = match Compositor::new() {
        Ok(c) => c,
        Err(e) => {
            println!("\n跳过全帧：{e}");
            return;
        }
    };
    println!(
        "\n## 全帧：5000 节点，1600×900，{}",
        compositor.adapter_summary()
    );
    println!("   （串行等待，读数是上界；真机 CPU/GPU 流水重叠，帧时间 ≈ max(两者)）");

    let mut text = TextEngine::new().expect("DirectWrite 必须可用");
    for (label, zoom) in [("100%", 1.0f32), ("80% 可见", z80), ("全图", 1.0 / fit)] {
        let f = frame_at(&scene, theme, center, zoom, Some(VIEWPORT));
        let visible = visible_nodes(&scene, &f);
        println!(
            "  {label} z={zoom:.4}：可见 {visible} / {} 节点",
            scene.nodes.len()
        );

        // 预热：形状缓存、图集、实例缓冲都进入稳态
        for _ in 0..3 {
            let mut layers = build_layers(&f);
            let update = text.paint(&f, &mut layers);
            compositor.upload_atlas(&update);
            compositor.render_texture(&f, &layers, [1600, 900]);
            compositor.wait_idle();
        }

        let mut cpu = Vec::new();
        let mut total = Vec::new();
        for _ in 0..21 {
            let t = Instant::now();
            let mut layers = build_layers(&f);
            let update = text.paint(&f, &mut layers);
            compositor.upload_atlas(&update);
            let cpu_done = t.elapsed();
            compositor.render_texture(&f, &layers, [1600, 900]);
            compositor.wait_idle();
            cpu.push(cpu_done);
            total.push(t.elapsed());
        }
        let gpu: Vec<Duration> = total
            .iter()
            .zip(&cpu)
            .map(|(a, b)| a.saturating_sub(*b))
            .collect();
        // measure() 自带运行循环，这里已经采完样，直接三个 report
        report(&format!("  {label} CPU（build+paint+上传）"), &cpu, 8.0);
        report(&format!("  {label} GPU（提交+执行）"), &gpu, 10.0);
        report(&format!("  {label} 串行合计"), &total, P5_BUDGET_MS);
        let med = median_ms(&total);
        println!(
            "     → 串行上界 {:.1} fps；按 max(CPU,GPU) 估 {:.1} fps",
            1000.0 / med,
            1000.0 / median_ms(&cpu).max(median_ms(&gpu)),
        );
    }
}

fn median_ms(runs: &[Duration]) -> f64 {
    let mut ms: Vec<f64> = runs.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(|a, b| a.total_cmp(b));
    ms[ms.len() / 2]
}

/// 全量重排里文本度量占多少（docs/08 P8 5000 节点 ≤ 120 ms，P7 增量 ≤ 16 ms）。
///
/// 三种度量器各跑一遍，差值就是真实字体接进来后多付的钱。「冷」每次新建度量器，
/// 对应打开文档后的第一次重排；「热」复用同一个实例，对应编辑提交后的增量重排。
/// 顺带打出包围盒：近似度量把树撑成的形状会在真实度量下收敛，这个数要一直看得见。
fn text_costs(theme: &mw_core::Theme, st: &LayoutSettings) {
    let doc = synth(5000);
    println!("## 5000 节点 build_scene 的度量成本（含布局，非纯文本）");

    let noop = time_build(&doc, theme, st, |_| Box::new(Noop), 7, "Noop");
    let mono = time_build(&doc, theme, st, |_| Box::new(MonoMeasurer), 7, "Mono");
    let cold = time_build(
        &doc,
        theme,
        st,
        |_| match DWrite::new() {
            Some(dw) => Box::new(DWriteMeasurer::new(dw)),
            None => Box::new(MonoMeasurer),
        },
        7,
        "DWrite",
    );
    let warm = time_build_warm(&doc, theme, st, 21);
    println!();
    for (name, runs, budget) in [
        ("Noop 冷", &noop.0, 120.0),
        ("Mono 冷", &mono.0, 120.0),
        ("DWrite 冷", &cold.0, 120.0),
        ("DWrite 热", &warm, 16.0),
    ] {
        report(&format!("  {name}"), runs, budget);
    }
}

fn time_build(
    doc: &Document,
    theme: &mw_core::Theme,
    st: &LayoutSettings,
    mut make: impl FnMut(usize) -> Box<dyn TextMeasurer>,
    runs: usize,
    label: &str,
) -> (Vec<Duration>, mw_core::Rect) {
    let mut out = Vec::new();
    let mut bounds = mw_core::Rect::default();
    for i in 0..runs {
        let m = make(i);
        let t = Instant::now();
        let scene = build_scene(doc, &doc.root_id, st, theme, m.as_ref(), None);
        out.push(t.elapsed());
        bounds = scene.bounds;
        std::hint::black_box(&scene);
    }
    println!(
        "  {label:<7} 包围盒 {:.0}×{:.0}，最长行 {} 行",
        bounds.w,
        bounds.h,
        scene_lines(doc, theme, st, make(0).as_ref())
    );
    (out, bounds)
}

fn scene_lines(
    doc: &Document,
    theme: &mw_core::Theme,
    st: &LayoutSettings,
    m: &dyn TextMeasurer,
) -> usize {
    build_scene(doc, &doc.root_id, st, theme, m, None)
        .nodes
        .iter()
        .map(|n| n.geom.lines.len())
        .max()
        .unwrap_or(0)
}

fn time_build_warm(
    doc: &Document,
    theme: &mw_core::Theme,
    st: &LayoutSettings,
    runs: usize,
) -> Vec<Duration> {
    let Some(dw) = DWrite::new() else {
        return Vec::new();
    };
    let m = DWriteMeasurer::new(dw);
    let mut out = Vec::new();
    for _ in 0..runs {
        let t = Instant::now();
        let scene = build_scene(doc, &doc.root_id, st, theme, &m, None);
        out.push(t.elapsed());
        std::hint::black_box(scene.bounds);
    }
    out
}

fn frame_at<'a>(
    scene: &'a Scene,
    theme: &'a mw_core::Theme,
    center: mw_core::Point,
    zoom: f32,
    viewport: Option<[f32; 2]>,
) -> Frame<'a> {
    Frame {
        scene,
        theme,
        // 压测要覆盖低于交互下限（0.02）的全图档，所以直接构造；产品缩放另有 MIN_ZOOM。
        camera: Camera {
            center,
            zoom,
            ppp: 1.0,
        },
        selected: vec![],
        hovered: None,
        hover_alpha: 1.0,
        select_alpha: 1.0,
        editing: None,
        ghost: None,
        guides: Vec::new(),
        viewport,
    }
}

fn dump_counts(f: &Frame) {
    use mw_render::instances::{L_LINES, L_NODES, L_OVERLAY};
    let l = build_layers(f);
    let g = |i: usize| {
        format!(
            "面 {} 线 {} 箭 {}",
            l[i].rects.len(),
            l[i].segs.len(),
            l[i].tris.len()
        )
    };
    println!("    实例：L1 {}", g(L_LINES));
    println!("          L2 {}，L3 {}", g(L_NODES), g(L_OVERLAY));
}
