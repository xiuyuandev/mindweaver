//! 布局与场景的性能基线（对应 docs/08 的 P7/P8 预算）。
//!
//! `cargo run -p mw-core --release --example perf_scene`
//!
//! P7「增量重排」要的是只重排受影响子树，那是 M1c 的活；这里先量当前的全量重排，
//! 把上限摆在那里，M1c 做完后同一份数据再跑一遍就能看出差距。

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use mw_core::layout::{compute_layout, BoxSpec, NodeMeasure};
use mw_core::model::StyleOverride;
use mw_core::{
    build_scene, build_scene_cached, get_theme, resolve_all, Command, DocMeta, Document,
    LayoutSettings, Measured, Node, SceneCache, TextMeasurer,
};

/// 只算尺寸的假度量：性能测试要的是树的规模，不是文字测量精度。
struct Fast;

impl NodeMeasure for Fast {
    fn measure(&self, _node: &Node, _depth: u32, text: &str) -> BoxSpec {
        BoxSpec {
            w: (text.chars().count() as f32) * 12.0 + 28.0,
            h: 32.0,
            lines: std::sync::Arc::from(vec![text.to_string()]),
            font_size: 14.0,
            line_height: 20.0,
            bold: false,
            label_h: 0.0,
            image_h: 0.0,
            image_ratio: 16.0 / 9.0,
        }
    }
}

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

/// 合成一棵 `count` 个节点的树：分支宽度在 6/8/8/5 之间轮换，接近真实导图的形状。
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

/// 每颗节点都带显式样式——XMind 导入文档的常见形态（导入器把 fo:color / fill /
/// fo:font-size 全写成节点覆盖）。样式级联要为每个节点解一次覆盖，是 20k 重建里
/// 唯一跟「文档有多少样式」成正比的一段。
fn styled_synth(count: usize) -> Document {
    let mut doc = synth(count);
    for n in doc.nodes.values_mut() {
        n.style = Some(StyleOverride {
            fill: Some("#f7f7f7".into()),
            text_color: Some("#222222".into()),
            stroke: Some("#cccccc".into()),
            stroke_width: Some(1.0),
            branch_color: Some("#3366cc".into()),
            ..StyleOverride::default()
        });
    }
    doc
}

fn settings(kind: &str) -> LayoutSettings {
    LayoutSettings {
        kind: kind.to_string(),
        direction: "auto".to_string(),
        edge_shape: "curve".to_string(),
        edge_arrow: false,
        sibling_gap: 24.0,
        level_gap: 80.0,
        max_node_width: 300.0,
        focus_question: None,
    }
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
    println!("{name:<22}{min:6.1}  中位 {med:6.1}  p95 {p95:6.1} ms   预算 {budget_ms:.0} ms → {verdict}");
}

fn main() {
    let theme = get_theme("snow");
    let st = settings("mindmap");

    println!("预算：P8 全量重排 5k ≤ 120 ms，P7 重排 p95 ≤ 16 ms（docs/08）");
    for &n in &[500usize, 2000, 5000, 20_000] {
        let doc = synth(n);
        assert_eq!(doc.node_count(), n);

        // ---- P8：全量重排（样式级联 + 布局 + 高级元素）----
        let mut runs = Vec::new();
        for i in 0..12 {
            let t = Instant::now();
            let s = build_scene(&doc, &doc.root_id, &st, &theme, &Noop, None);
            runs.push(t.elapsed());
            if i == 0 {
                println!(
                    "  {n} 节点：{} 连线，包围盒 {:.0}×{:.0}",
                    s.edges.len(),
                    s.bounds.w,
                    s.bounds.h
                );
            }
        }
        let budget = if n == 20_000 {
            600.0
        } else if n == 5000 {
            120.0
        } else {
            16.0
        };
        report(&format!("{n} build_scene"), &runs, budget);

        // ---- 纯布局（不含样式级联与高级元素）：定位瓶颈 ----
        let mut runs = Vec::new();
        for _ in 0..12 {
            let t = Instant::now();
            let l = compute_layout(&doc, &doc.root_id, &st, &Fast, None);
            runs.push(t.elapsed());
            assert_eq!(l.boxes.len(), n);
        }
        report(&format!("{n} 仅布局"), &runs, budget);

        // ---- P7 代理指标：改一个字后重建（真正的增量重排在 M1c）----
        let mut d = synth(n);
        let leaf = d
            .nodes
            .values()
            .find(|x| x.children.is_empty() && x.parent_id.is_some())
            .unwrap()
            .id
            .clone();
        let mut runs = Vec::new();
        for i in 0..40 {
            let t = Instant::now();
            d.apply(&Command::SetText {
                id: leaf.clone(),
                text: format!("改过的一次 {i}"),
            })
            .unwrap();
            let _ = build_scene(&d, &d.root_id, &st, &theme, &Noop, None);
            runs.push(t.elapsed());
        }
        report(&format!("{n} 改字重建"), &runs, 16.0);

        // ---- 应用真正走的那条路：同一个 SceneCache 反复重建 ----
        // 编辑每改一次字就整棵重建一次，命中的节点不再测量；这才是「打字时每键
        // 一顿」的真实成本。缓存要在循环外预热（打开文档的那一下会付出冷成本）。
        // 分阶段日志（`MW_LAYOUT_LOG=1`）从这行往下都是「缓存路径」的数字。
        eprintln!("[perf] {n} 改字重建(缓存) 起");
        let cache = SceneCache::new();
        let mut dc = synth(n);
        let _ = build_scene_cached(&dc, &dc.root_id, &st, &theme, &Noop, None, &cache);
        let leaf = dc
            .nodes
            .values()
            .find(|x| x.children.is_empty() && x.parent_id.is_some())
            .unwrap()
            .id
            .clone();
        let mut runs = Vec::new();
        for i in 0..40 {
            let t = Instant::now();
            dc.apply(&Command::SetText {
                id: leaf.clone(),
                text: format!("改过的一次 {i}"),
            })
            .unwrap();
            let _ = build_scene_cached(&dc, &dc.root_id, &st, &theme, &Noop, None, &cache);
            runs.push(t.elapsed());
        }
        report(&format!("{n} 改字重建(缓存)"), &runs, 16.0);
        println!(
            "  （缓存条目 {}，节点 {}，命中 {} 未命中 {}）",
            cache.len(),
            n,
            cache.stats().0,
            cache.stats().1
        );

        // ---- P9（缓存路径）：撤销一步 / 撤销后再重做 ----
        // 应用里的 `Ctrl+Z` 走的就是 `build_scene_cached`；只留一代账时，撤销把内容退回上一轮
        // 之外的那一份，整份作废（20k 上白付 ~25ms）。这里量「两代账」之后的真实手感。
        let mut dc2 = synth(n);
        let cache2 = SceneCache::new();
        let leaf2 = dc2
            .nodes
            .values()
            .find(|x| x.children.is_empty() && x.parent_id.is_some())
            .unwrap()
            .id
            .clone();
        let _ = build_scene_cached(&dc2, &dc2.root_id, &st, &theme, &Noop, None, &cache2);
        let mut runs = Vec::new();
        for i in 0..40 {
            dc2.apply(&Command::SetText {
                id: leaf2.clone(),
                text: format!("撤销前 {i}"),
            })
            .unwrap();
            let _ = build_scene_cached(&dc2, &dc2.root_id, &st, &theme, &Noop, None, &cache2);
            let t = Instant::now();
            dc2.undo().unwrap();
            cache2.prefer_previous();
            let _ = build_scene_cached(&dc2, &dc2.root_id, &st, &theme, &Noop, None, &cache2);
            runs.push(t.elapsed());
        }
        report(&format!("{n} 撤销重建(缓存)"), &runs, 30.0);

        let mut runs = Vec::new();
        for _ in 0..40 {
            let t = Instant::now();
            dc2.redo().unwrap();
            cache2.prefer_previous();
            let _ = build_scene_cached(&dc2, &dc2.root_id, &st, &theme, &Noop, None, &cache2);
            runs.push(t.elapsed());
            dc2.undo().unwrap();
            cache2.prefer_previous();
            let _ = build_scene_cached(&dc2, &dc2.root_id, &st, &theme, &Noop, None, &cache2);
        }
        report(&format!("{n} 重做重建(缓存)"), &runs, 30.0);

        // ---- P9（缓存路径 · 结构改动）：加一颗节点 → 撤销 ----
        // 结构一变，当前代的账整份作废（可见节点序列变了），这条全靠**上一代**那张账。
        let mut dc3 = synth(n);
        let cache3 = SceneCache::new();
        let _ = build_scene_cached(&dc3, &dc3.root_id, &st, &theme, &Noop, None, &cache3);
        let anchor3 = dc3
            .nodes
            .values()
            .find(|x| x.children.is_empty() && x.parent_id.is_some())
            .unwrap()
            .id
            .clone();
        let mut runs = Vec::new();
        for i in 0..20 {
            dc3.apply(&Command::AddNode {
                parent: anchor3.clone(),
                index: None,
                text: format!("新增 {i}"),
                style: None,
            })
            .unwrap();
            let _ = build_scene_cached(&dc3, &dc3.root_id, &st, &theme, &Noop, None, &cache3);
            let t = Instant::now();
            dc3.undo().unwrap();
            cache3.prefer_previous();
            let _ = build_scene_cached(&dc3, &dc3.root_id, &st, &theme, &Noop, None, &cache3);
            runs.push(t.elapsed());
        }
        report(&format!("{n} 结构撤销重建(缓存)"), &runs, 30.0);

        // ---- P9：撤销/重做 + 重建（UI 提交到可见场景的代理）----
        let mut runs = Vec::new();
        loop {
            let t = Instant::now();
            if d.undo().is_none() {
                break;
            }
            let _ = build_scene(&d, &d.root_id, &st, &theme, &Noop, None);
            runs.push(t.elapsed());
        }
        report(&format!("{n} 撤销重建"), &runs, 30.0);

        let mut runs = Vec::new();
        loop {
            let t = Instant::now();
            if d.redo().is_none() {
                break;
            }
            let _ = build_scene(&d, &d.root_id, &st, &theme, &Noop, None);
            runs.push(t.elapsed());
        }
        report(&format!("{n} 重做重建"), &runs, 30.0);
    }

    // ---- 样式级联（每颗节点都带覆盖样式）：20k 重建里唯一随样式数增长的段 ----
    for &n in &[5000usize, 20_000] {
        let sdoc = styled_synth(n);
        let mut runs = Vec::new();
        for _ in 0..12 {
            let t = Instant::now();
            let s = resolve_all(&sdoc, &theme);
            runs.push(t.elapsed());
            assert_eq!(s.len(), n);
        }
        report(&format!("{n} 样式级联(全带样式)"), &runs, 16.0);
    }

    // ---- P12：拼音索引首次查询（20k 冷缓存）----
    println!("P12：20k 文档首次拼音查询预算 100 ms（缓存为空，逐节点建键）");
    for i in 0..3 {
        let mut d = synth(20000);
        let t = Instant::now();
        let hits = d.search("xing");
        let cold = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let warm_hits = d.search("xing");
        let warm = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "  20k 冷查询 #{}：{cold:.1} ms，命中 {}；二次查询 {warm:.1} ms，命中 {}",
            i + 1,
            hits.len(),
            warm_hits.len()
        );
    }
}
