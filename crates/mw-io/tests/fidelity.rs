//! 导入保真样例集：10 份结构/备注/链接/标签/折叠各不相同的文档，
//! 分别走 OPML、FreeMind、Markdown 的导出→导入，比较树与字段。
//!
//! XMind 与概念图 `.cxl` 都已有导出器，两条都进矩阵：XMind 只比它承诺的字段
//! （标题/备注/标签/折叠）；`.cxl` 走「互通视图 + 保真槽」，保真槽在时逐节点/逐命题比全字段，
//! 保真槽被抹掉（外部工具另存）时只比互通视图真能表达的那几项（CM-23 §3）。

use std::collections::HashMap;

use mw_core::model::{ArrowHead, ConceptGroup, Position, Relationship, StyleOverride};
use mw_core::{Command, DocMeta, Document, LayoutSettings, Node, NoteCard};
use mw_io::cmap::{load_cxl, save_cxl};
use mw_io::export::{export_freemind, export_markdown, export_opml};
use mw_io::import::{import_freemind, import_markdown, import_opml};
use mw_io::{export_xmind, import_xmind};

fn add(d: &mut Document, parent: &str, text: &str) -> String {
    let before: std::collections::HashSet<String> = d.doc_order().into_iter().collect();
    d.apply(&Command::AddNode {
        parent: parent.to_string(),
        index: None,
        text: text.into(),
        style: None,
    })
    .unwrap();
    d.doc_order()
        .into_iter()
        .find(|id| !before.contains(id))
        .expect("新节点")
}

/// 一份样例：i 决定分支数、深度、备注/链接/标签/折叠分布。
fn sample(i: usize) -> Document {
    let mut d = Document::new(&format!("样例 {i}"));
    let root = d.root_id.clone();
    d.nodes.get_mut(&root).unwrap().text = format!("根 {i}");
    let mut kids = Vec::new();
    for n in 0..(1 + i % 3) {
        let a = add(&mut d, &root, &format!("分支 {i}-{n}"));
        if n % 2 == 0 {
            let b = add(&mut d, &a, &format!("叶 {i}-{n}"));
            if i.is_multiple_of(4) {
                add(&mut d, &b, &format!("孙 {i}-{n}"));
            }
        }
        kids.push(a);
    }
    for (n, id) in kids.iter().enumerate() {
        let node = d.nodes.get_mut(id).unwrap();
        if (i + n).is_multiple_of(3) {
            node.note = Some(format!("备注 {i}-{n}"));
        }
        if (i + n).is_multiple_of(4) {
            node.link = Some(format!("https://example.com/{i}/{n}"));
        }
        if (i + n).is_multiple_of(5) {
            node.labels = vec![format!("标签{}", i % 3)];
        }
        if (i + n).is_multiple_of(6) && !node.children.is_empty() {
            node.collapsed = true;
        }
    }
    // 一张卡片，验证卡片正文不因导出丢失（OPML/FreeMind 的备注字段）。
    let card = NoteCard::new(d.id.clone(), None, "卡片", "卡片正文", 1);
    d.cards.push(card);
    d
}

fn tree(d: &Document) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for id in d.doc_order() {
        let n = &d.nodes[&id];
        let mut depth = 0;
        let mut p = n.parent_id.clone();
        while let Some(pid) = p {
            depth += 1;
            p = d.nodes.get(&pid).and_then(|x| x.parent_id.clone());
        }
        out.push((depth, n.text.clone()));
    }
    out
}

type FieldRow = (String, Option<String>, Option<String>, Vec<String>, bool);

fn fields(d: &Document) -> Vec<FieldRow> {
    d.doc_order()
        .into_iter()
        .map(|id| {
            let n = &d.nodes[&id];
            (
                n.text.clone(),
                n.note.clone(),
                n.link.clone(),
                n.labels.clone(),
                n.collapsed,
            )
        })
        .collect()
}

#[test]
fn ten_samples_survive_opml_roundtrip() {
    for i in 0..10 {
        let src = sample(i);
        let back = import_opml(&export_opml(&src)).unwrap();
        assert_eq!(tree(&back), tree(&src), "样例 {i} 的 OPML 树不一致");
        assert_eq!(fields(&back), fields(&src), "样例 {i} 的 OPML 字段不一致");
    }
}

#[test]
fn ten_samples_survive_freemind_roundtrip() {
    for i in 0..10 {
        let src = sample(i);
        let back = import_freemind(&export_freemind(&src)).unwrap();
        assert_eq!(tree(&back), tree(&src), "样例 {i} 的 FreeMind 树不一致");
        assert_eq!(
            fields(&back),
            fields(&src),
            "样例 {i} 的 FreeMind 字段不一致"
        );
    }
}

#[test]
fn ten_samples_survive_xmind_roundtrip_for_its_supported_fields() {
    for i in 0..10 {
        let src = sample(i);
        let bytes = export_xmind(&src).unwrap();
        let back = import_xmind(&bytes).unwrap();
        assert_eq!(tree(&back), tree(&src), "样例 {i} 的 XMind 树不一致");
        // XMind 映射只承诺标题/备注/标签/折叠。
        let strip = |d: &Document| -> Vec<(String, Option<String>, Vec<String>, bool)> {
            d.doc_order()
                .into_iter()
                .map(|id| {
                    let n = &d.nodes[&id];
                    (
                        n.text.clone(),
                        n.note.clone(),
                        n.labels.clone(),
                        n.collapsed,
                    )
                })
                .collect()
        };
        assert_eq!(strip(&back), strip(&src), "样例 {i} 的 XMind 字段不一致");
    }
}

#[test]
fn ten_samples_survive_markdown_roundtrip() {
    for i in 0..10 {
        let src = sample(i);
        let back = import_markdown(&export_markdown(&src)).unwrap();
        assert_eq!(
            tree(&back),
            tree(&src),
            "样例 {i} 的 Markdown 树/层级不一致"
        );
    }
}

// ---------------------------------------------------------------------------
// 概念图 `.cxl`（CM-23）：互通视图 + 保真槽
// ---------------------------------------------------------------------------

/// 概念图样例：`i` 决定概念数、箭头档位、钉住/样式/分组的有无。
fn concept_sample(i: usize) -> Document {
    let n = 3 + i % 4; // 3–6 个概念
    let mut nodes: HashMap<String, Node> = HashMap::new();
    let mut ids: Vec<String> = Vec::new();
    for k in 0..n {
        let id = format!("c-{k}");
        let mut node = Node::new(id.clone(), None, format!("概念 {i}-{k}"));
        node.pos = Some(Position {
            x: 40.0 + (k as f32) * 180.0,
            y: 60.0 + (k as f32) * 90.0,
        });
        if k % 3 == 0 {
            node.pinned = Some(true);
        }
        if k % 4 == 1 {
            node.style = Some(StyleOverride {
                fill: Some("#ffcc00".into()),
                stroke: Some("#123456".into()),
                font_size: Some(16),
                shape: Some("ellipse".into()),
                ..Default::default()
            });
        }
        // 节点扩展字段不在 `conceptMap` 块里，靠兼容镜像 `nodes` 往返（CM-23 §7.5）。
        node.note = Some(format!("备注 {i}-{k}"));
        node.labels = vec![format!("标签{}", k % 2)];
        nodes.insert(id.clone(), node);
        ids.push(id);
    }
    // 卡片：`validate` 会拒「节点链接的卡片缺失」，所以每颗概念挂一张真卡。
    let mut cards = Vec::new();
    for (k, id) in ids.iter().enumerate() {
        let card = NoteCard::new(
            format!("doc-{i}"),
            Some(id.clone()),
            format!("卡片 {i}-{k}"),
            "卡片正文",
            1,
        );
        nodes.get_mut(id).unwrap().card_id = Some(card.id.clone());
        cards.push(card);
    }
    for k in 1..n {
        let head = match (k + i) % 4 {
            0 => ArrowHead::Off,
            1 => ArrowHead::Forward,
            2 => ArrowHead::Backward,
            _ => ArrowHead::Both,
        };
        let mut rel = Relationship {
            id: format!("p-{k}"),
            to: ids[k - 1].clone(),
            label: Some(format!("关系 {i}-{k}")),
            head,
            ..Default::default()
        };
        if k % 2 == 0 {
            rel.color = Some("#3366ff".into());
            rel.dashed = true;
            rel.width = Some(2.0);
        }
        if k % 3 == 0 {
            rel.note = Some(format!("线注释 {k}"));
        }
        nodes.get_mut(&ids[k]).unwrap().relationships.push(rel);
    }
    let groups = if i.is_multiple_of(2) {
        vec![ConceptGroup {
            id: format!("g-{i}"),
            label: format!("组 {i}"),
            members: ids[..2].to_vec(),
            color: Some("#888888".into()),
        }]
    } else {
        Vec::new()
    };
    let meta = DocMeta {
        title: format!("概念图 {i}"),
        theme: if i.is_multiple_of(2) { "snow" } else { "mist" }.to_string(),
        theme_accent: None,
        theme_overrides: None,
        layout: LayoutSettings {
            kind: "conceptMap".to_string(),
            focus_question: Some(format!("聚焦问题 {i}")),
            ..Default::default()
        },
    };
    Document::from_parts_full(
        Some(format!("doc-{i}")),
        meta,
        ids[0].clone(),
        nodes,
        cards,
        groups,
    )
    .unwrap()
}

/// 去掉保真槽，模拟「CmapTools 打开后另存」。
fn strip_fidelity(xml: &str) -> String {
    let start = xml.find("<extra-properties-list>").expect("有保真槽");
    let end =
        xml.find("</extra-properties-list>").expect("有保真槽") + "</extra-properties-list>".len();
    format!("{}{}", &xml[..start], &xml[end..])
}

#[test]
fn ten_concept_samples_survive_cxl_fidelity_roundtrip() {
    for i in 0..10 {
        let src = concept_sample(i);
        let back = load_cxl(&save_cxl(&src).unwrap()).unwrap();
        assert_eq!(back.meta.title, src.meta.title, "样例 {i} 的标题");
        assert_eq!(back.meta.theme, src.meta.theme, "样例 {i} 的主题");
        assert_eq!(
            back.meta.layout.focus_question, src.meta.layout.focus_question,
            "样例 {i} 的聚焦问题"
        );
        assert_eq!(back.root_id, src.root_id, "样例 {i} 的根");
        assert_eq!(back.groups, src.groups, "样例 {i} 的分组");
        assert_eq!(back.cards.len(), src.cards.len(), "样例 {i} 的卡片数");
        // 节点与命题逐字段比。概念图的 `conceptMap` 块只描述概念形状，节点扩展字段
        // （备注/链接/标签/卡片…）靠兼容镜像 `nodes` 按 id 并回来（CM-23 §7.5 已修）。
        let mut expected = src.nodes.clone();
        let mut got = back.nodes.clone();
        // `modified_at` 是保存/重建时打的时间戳（差 1ms 就会让比较假红），不是内容字段。
        for node in expected.values_mut().chain(got.values_mut()) {
            node.modified_at = 0;
        }
        assert_eq!(got, expected, "样例 {i} 的节点/命题字段");
    }
}

#[test]
fn ten_concept_samples_survive_cxl_degraded_parse() {
    // 保真槽被外部工具抹掉后，只剩互通视图：标题/聚焦问题/几何/命题结构/箭头应当还在。
    for i in 0..10 {
        let src = concept_sample(i);
        let stripped = strip_fidelity(&save_cxl(&src).unwrap());
        let back = load_cxl(&stripped).unwrap();
        assert_eq!(back.meta.title, src.meta.title, "样例 {i} 降级后标题");
        assert_eq!(
            back.meta.layout.focus_question, src.meta.layout.focus_question,
            "样例 {i} 降级后聚焦问题"
        );
        assert_eq!(back.nodes.len(), src.nodes.len(), "样例 {i} 降级后概念数");
        for (id, node) in &src.nodes {
            let got = back.nodes.get(id).expect("概念要读回来");
            assert_eq!(got.text, node.text, "样例 {i} 概念文本");
            assert_eq!(got.pos, node.pos, "样例 {i} 概念坐标");
            // 互通视图写了 font-size / background-color / border-color，降级必须读回
            // （CM-23 §7.7 / §3 #11-#12）。没写外观的概念保持 `None`。
            match &node.style {
                Some(src_style) => {
                    let got_style = got
                        .style
                        .as_ref()
                        .unwrap_or_else(|| panic!("样例 {i} 节点 {id} 外观该读回"));
                    assert_eq!(got_style.font_size, src_style.font_size, "样例 {i} 字号");
                    // 颜色写侧是 `r,g,b,a`、回读统一成 `#RRGGBBAA`，故只比语义不等字面。
                    assert_eq!(
                        got_style.fill.as_deref(),
                        src_style.fill.as_deref().map(|_| "#FFCC00FF"),
                        "样例 {i} 填充色"
                    );
                    assert_eq!(
                        got_style.stroke.as_deref(),
                        src_style.stroke.as_deref().map(|_| "#123456FF"),
                        "样例 {i} 边框色"
                    );
                    // `shape` 不在互通视图里，降级必丢——这是设计边界。
                    assert_eq!(got_style.shape, None, "样例 {i} 形状不在互通视图");
                }
                None => assert_eq!(got.style, None, "样例 {i} 节点 {id} 无外观"),
            }
            assert_eq!(
                got.relationships.len(),
                node.relationships.len(),
                "样例 {i} 命题数"
            );
            for (want, have) in node.relationships.iter().zip(&got.relationships) {
                assert_eq!(have.to, want.to, "样例 {i} 命题终点");
                assert_eq!(have.label, want.label, "样例 {i} 命题短语");
                assert_eq!(have.head, want.head, "样例 {i} 箭头方向");
            }
        }
    }
}
