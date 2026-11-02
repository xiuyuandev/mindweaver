//! Generate the deterministic structure used by the X100 reference and regression matrix.
//!
//! The small fixtures are safe to commit; the 20k performance fixture is generated on demand
//! into `target/` so a source checkout does not carry a large binary blob.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use mw_core::{
    Command, DocMeta, Document, LayoutSettings, Node, NoteCard, StyleOverride, TaskState,
};

fn main() {
    let mut out_dir = PathBuf::from("tests/fixtures/x100");
    let mut perf_path = PathBuf::from("target/x100-fixtures/perf-20k.zmind");
    let mut perf_nodes = 0usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out_dir = PathBuf::from(args.next().expect("--out needs a path")),
            "--perf" => perf_path = PathBuf::from(args.next().expect("--perf needs a path")),
            "--perf-nodes" => {
                perf_nodes = args
                    .next()
                    .expect("--perf-nodes needs a number")
                    .parse()
                    .expect("--perf-nodes must be a number")
            }
            other => panic!("unknown argument: {other}"),
        }
    }

    std::fs::create_dir_all(&out_dir).expect("create fixture directory");
    write(&tiny_cn(), &out_dir.join("tiny-cn.zmind"));
    write(&complex_style(), &out_dir.join("complex-style.zmind"));
    if perf_nodes > 0 {
        write(&performance(perf_nodes), &perf_path);
    }
}

fn write(doc: &Document, path: &Path) {
    mw_format::save(doc, path).unwrap_or_else(|e| panic!("save {}: {e}", path.display()));
    println!(
        "{} nodes={} path={}",
        doc.meta.title,
        doc.node_count(),
        path.display()
    );
}

fn add(doc: &mut Document, parent: &str, text: impl Into<String>) -> String {
    let before = doc.node(parent).expect("parent").children.clone();
    doc.apply(&Command::AddNode {
        parent: parent.to_string(),
        index: None,
        text: text.into(),
        style: None,
    })
    .expect("add node");
    doc.node(parent)
        .expect("parent after add")
        .children
        .iter()
        .find(|id| !before.contains(id))
        .expect("new child id")
        .clone()
}

fn set_theme_and_layout(doc: &mut Document) {
    doc.apply(&Command::SetTheme {
        theme: "snow".to_string(),
    })
    .expect("theme");
    let layout = LayoutSettings {
        kind: "mindmap".to_string(),
        ..Default::default()
    };
    doc.apply(&Command::SetLayout { layout }).expect("layout");
}

fn tiny_cn() -> Document {
    let mut doc = Document::new("X100 Tiny CN");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "产品大纲".to_string(),
    })
    .expect("root text");
    set_theme_and_layout(&mut doc);

    let goals = add(&mut doc, &root, "目标");
    let people = add(&mut doc, &root, "用户");
    let delivery = add(&mut doc, &root, "交付");
    let risk = add(&mut doc, &root, "风险");

    let a = add(&mut doc, &goals, "两周内完成首版");
    let b = add(&mut doc, &goals, "保留全部旧文件");
    let c = add(&mut doc, &people, "中文输入稳定");
    let d = add(&mut doc, &delivery, "导出可搜索 PDF");
    let e = add(&mut doc, &risk, "20k 节点不卡顿");

    doc.apply(&Command::SetTask {
        id: a.clone(),
        task: Some(TaskState::Done),
    })
    .expect("task");
    doc.apply(&Command::SetTask {
        id: b,
        task: Some(TaskState::Doing),
    })
    .expect("task");
    doc.apply(&Command::AddLabel {
        ids: vec![c.clone(), d.clone()],
        label: "验收".to_string(),
    })
    .expect("label");
    doc.apply(&Command::SetNote {
        id: e.clone(),
        note: Some("使用固定 20k 语料，分别测打开、平移、编辑和导出。".to_string()),
    })
    .expect("note");
    doc.apply(&Command::SetLink {
        id: c,
        link: Some("https://example.com/x100".to_string()),
    })
    .expect("link");
    doc.apply(&Command::AddRelationship {
        from: risk.clone(),
        to: delivery.clone(),
        label: Some("阻塞".to_string()),
    })
    .expect("relationship");
    doc.apply(&Command::AddBoundary {
        parent: root.clone(),
        targets: vec![people.clone(), delivery.clone()],
        color: Some("#e4635b".to_string()),
    })
    .expect("boundary");
    doc.apply(&Command::AddSummary {
        parent: root,
        targets: vec![goals, risk],
        text: "本阶段按核心体验优先".to_string(),
    })
    .expect("summary");
    doc.apply(&Command::AddFloatingTopic {
        text: "自由主题：待整理想法".to_string(),
        x: 360.0,
        y: 220.0,
    })
    .expect("floating topic");
    doc
}

fn complex_style() -> Document {
    let mut doc = Document::new("X100 Complex Style");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "知识内化".to_string(),
    })
    .expect("root");
    set_theme_and_layout(&mut doc);

    for (i, title) in ["输入", "连接", "练习", "输出", "复盘"].iter().enumerate() {
        let branch = add(&mut doc, &root, *title);
        let color = ["#4e9e6b", "#e4635b", "#d6a341", "#4a7dbd", "#8b63a8"][i];
        doc.apply(&Command::SetStyle {
            ids: vec![branch.clone()],
            style: Some(StyleOverride {
                fill: Some("#f7f3eb".to_string()),
                stroke: Some(color.to_string()),
                stroke_width: Some(1.5),
                bold: Some(true),
                shape: Some(if i % 2 == 0 { "rounded" } else { "capsule" }.to_string()),
                branch_color: Some(color.to_string()),
                ..Default::default()
            }),
        })
        .expect("branch style");
        for j in 0..6 {
            let child = add(&mut doc, &branch, format!("{title} · 子主题 {}", j + 1));
            if j % 2 == 0 {
                doc.apply(&Command::SetTask {
                    id: child.clone(),
                    task: Some(if j == 0 {
                        TaskState::Done
                    } else {
                        TaskState::Todo
                    }),
                })
                .expect("task");
            }
            if j % 3 == 0 {
                doc.apply(&Command::AddLabel {
                    ids: vec![child.clone()],
                    label: format!("{title}/示例"),
                })
                .expect("label");
            }
            for k in 0..3 {
                let leaf = add(&mut doc, &child, format!("细节 {}.{}", j + 1, k + 1));
                if k == 1 {
                    doc.apply(&Command::SetNote {
                        id: leaf,
                        note: Some("用于检查长文本、备注角标和导出。".to_string()),
                    })
                    .expect("note");
                }
            }
        }
    }

    let card = NoteCard::new(
        doc.id.clone(),
        None,
        "复杂样式说明",
        "# 验收说明\n\n- 四主题\n- 三档 DPI\n- 一次撤销",
        mw_core::now_ms(),
    );
    let card_id = card.id.clone();
    let source = doc.root_id.clone();
    doc.apply(&Command::AttachNewCard { node: source, card })
        .expect("card");
    doc.apply(&Command::SetCardTags {
        id: card_id,
        tags: vec!["x100".to_string(), "视觉".to_string()],
    })
    .expect("card tags");
    doc
}

fn performance(count: usize) -> Document {
    let count = count.max(2);
    let root = "perf-root".to_string();
    let mut nodes = HashMap::new();
    nodes.insert(
        root.clone(),
        Node::new(root.clone(), None, format!("性能根 · {count}")),
    );
    let mut queue = VecDeque::from([root.clone()]);
    let mut made = 1usize;
    // Build directly rather than executing 20k undoable commands. The fixture is a data/rendering
    // corpus, while tiny/complex fixtures exercise the command path.
    while made < count {
        let Some(parent) = queue.pop_front() else {
            break;
        };
        for _ in 0..4 {
            if made >= count {
                break;
            }
            let id = format!("perf-{made:05}");
            let text = if made.is_multiple_of(41) {
                format!("长标题节点 {made} · 用于验证中文换行、省略和文本测量性能")
            } else {
                format!("节点 {made}")
            };
            let mut node = Node::new(id.clone(), Some(parent.clone()), text);
            if made.is_multiple_of(37) {
                node.labels.push("性能".to_string());
            }
            if made.is_multiple_of(97) {
                node.task = Some(TaskState::Doing);
            }
            nodes
                .get_mut(&parent)
                .expect("parent")
                .children
                .push(id.clone());
            nodes.insert(id.clone(), node);
            queue.push_back(id);
            made += 1;
        }
    }
    let meta = DocMeta {
        title: format!("X100 Performance {count}"),
        theme: "snow".to_string(),
        ..Default::default()
    };
    Document::from_parts(Some("x100-perf".to_string()), meta, root, nodes).expect("perf fixture")
}
