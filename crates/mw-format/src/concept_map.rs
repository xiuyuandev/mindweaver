//! 概念图的专属数据形状（`docs/33` §3）。
//!
//! 概念图不该硬塞进树模型里——那正是「处处分岔」的根源：概念要靠
//! `parent_id = None` 这种「不是什么」来定义，命题挂在节点上而不是图上，
//! 检查器/大纲/导出都要先判断「是不是概念图」再走另一条路。
//!
//! 这里给概念图一块**自己的**顶层 `conceptMap`：概念与命题各自成表。
//!
//! - **读**：块存在时**它是权威**（`nodes` 被忽略）；缺块回落到旧形状，
//!   老文件一个字节都不用改。
//! - **写**：概念图文档写出这个块；同时把 `nodes` 一起写（**兼容镜像**），
//!   老版本的织思打开还能看见。
//! - **可逆**：`nodes → block → nodes` 必须逐字节相同（[`round_trip`]），
//!   这是 `docs/33` §3 的验收判据。

use std::collections::HashMap;

use mw_core::model::{ArrowHead, ConceptGroup, Node, Relationship, StyleOverride};

/// 概念图的顶层块。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ConceptMapBlock {
    /// 没有概念时整个键都不写：空块不该往文件里塞 `concepts: []`。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<ConceptEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub propositions: Vec<PropositionEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_question: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ConceptGroup>,
}

/// 一颗概念。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ConceptEntry {
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleOverride>,
}

/// 一条命题（**升到图层**，不再寄生在源节点的 `relationships[]` 里）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PropositionEntry {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "ArrowHead::is_forward")]
    pub head: ArrowHead,
    #[serde(default, skip_serializing_if = "is_false")]
    pub dashed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// 从运行时的「概念 = `parent_id: None` 的节点」抽出专属块。
///
/// 顺序按 `(id)` / `(from, id)` 定死——同一份文档两次写出必须逐字节相同。
/// 只有 `parent_id: None` 的节点算概念（树上的节点不算）；命题取这些概念各自的
/// `relationships`，`to` 指向的概念可能不在集合里（被删了）——原样带过去，
/// 由 `validate` 那一层去判，不在这里悄悄丢数据。
pub fn from_nodes(
    nodes: &HashMap<String, Node>,
    focus_question: Option<String>,
    groups: Vec<ConceptGroup>,
) -> ConceptMapBlock {
    let mut ids: Vec<&String> = nodes
        .iter()
        .filter(|(_, n)| n.parent_id.is_none())
        .map(|(id, _)| id)
        .collect();
    ids.sort();
    let mut concepts = Vec::with_capacity(ids.len());
    let mut propositions = Vec::new();
    for id in ids {
        let n = &nodes[id];
        concepts.push(ConceptEntry {
            id: n.id.clone(),
            text: n.text.clone(),
            x: n.pos.as_ref().map(|p| p.x),
            y: n.pos.as_ref().map(|p| p.y),
            pinned: n.pinned,
            style: n.style.clone(),
        });
        for r in &n.relationships {
            propositions.push(PropositionEntry {
                id: r.id.clone(),
                from: n.id.clone(),
                to: r.to.clone(),
                label: r.label.clone(),
                color: r.color.clone(),
                head: r.head,
                dashed: r.dashed,
                width: r.width,
                note: r.note.clone(),
            });
        }
    }
    propositions.sort_by(|a, b| (&a.from, &a.id).cmp(&(&b.from, &b.id)));
    ConceptMapBlock {
        concepts,
        propositions,
        focus_question,
        groups,
    }
}

/// 把专属块还原成运行时的节点表。
///
/// 概念一律 `parent_id: None` + `pos`（没有坐标就放原点，由整理布局去摆）；
/// 命题挂回**源概念**的 `relationships`——运行时模型没变，变的只是文件里的形状。
pub fn to_nodes(block: &ConceptMapBlock) -> HashMap<String, Node> {
    let mut out = HashMap::new();
    for c in &block.concepts {
        let mut n = Node::new(c.id.clone(), None, c.text.clone());
        if let (Some(x), Some(y)) = (c.x, c.y) {
            n.pos = Some(mw_core::model::Position { x, y });
        }
        n.pinned = c.pinned;
        n.style = c.style.clone();
        out.insert(c.id.clone(), n);
    }
    for p in &block.propositions {
        let Some(n) = out.get_mut(&p.from) else {
            // 源概念不在表里：跳过（`validate` 会报），不许半路 panic。
            continue;
        };
        n.relationships.push(Relationship {
            id: p.id.clone(),
            to: p.to.clone(),
            label: p.label.clone(),
            color: p.color.clone(),
            head: p.head,
            dashed: p.dashed,
            width: p.width,
            note: p.note.clone(),
        });
    }
    out
}

/// 往返可逆（`docs/33` §3 的验收判据）：`nodes → block → nodes` 逐字节相同。
///
/// 「逐字节」指的是**结构**相同：`HashMap` 的遍历顺序不定，所以比的是
/// 「按 id 排序后的序列化」，不是 JSON 字符串。
// `docs/33` §3 的自检钩子；目前只有单测调用（模块不对外，故非测试构建视为死代码）。
#[cfg_attr(not(test), allow(dead_code))]
pub fn round_trip(nodes: &HashMap<String, Node>) -> bool {
    let block = from_nodes(nodes, None, Vec::new());
    let back = to_nodes(&block);
    fn norm(m: &HashMap<String, Node>) -> String {
        let mut ids: Vec<&String> = m.keys().collect();
        ids.sort();
        let mut s = String::new();
        for id in ids {
            s.push_str(&serde_json::to_string(&m[id]).unwrap_or_default());
            s.push('\n');
        }
        s
    }
    norm(nodes) == norm(&back)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concept(id: &str, text: &str, x: f32, y: f32) -> Node {
        let mut n = Node::new(id.to_string(), None, text);
        n.pos = Some(mw_core::model::Position { x, y });
        n
    }

    /// `docs/33` §3：`nodes → block → nodes` 必须逐字节可逆。
    #[test]
    fn round_trip_is_byte_exact() {
        let mut nodes = HashMap::new();
        let mut a = concept("a", "概念甲", 10.0, 20.0);
        a.pinned = Some(true);
        a.relationships.push(Relationship {
            id: "r1".into(),
            to: "b".into(),
            label: Some("导致".into()),
            color: Some("#c4644e".into()),
            head: ArrowHead::Both,
            dashed: true,
            width: Some(2.5),
            note: Some("2026Q3 的数据".into()),
        });
        nodes.insert("a".into(), a);
        nodes.insert("b".into(), concept("b", "概念乙", -5.0, 40.5));
        assert!(round_trip(&nodes), "nodes → block → nodes 必须逐字节可逆");
    }

    /// 树上的节点（`parent_id: Some`）不是概念，不该进块。
    #[test]
    fn only_free_nodes_become_concepts() {
        let mut nodes = HashMap::new();
        nodes.insert("a".into(), concept("a", "概念甲", 0.0, 0.0));
        let mut child = Node::new("c".into(), Some("a".into()), "子节点");
        child.pos = Some(mw_core::model::Position { x: 1.0, y: 2.0 });
        nodes.insert("c".into(), child);
        let block = from_nodes(&nodes, None, Vec::new());
        assert_eq!(block.concepts.len(), 1);
        assert_eq!(block.concepts[0].id, "a");
        assert!(block.propositions.is_empty());
    }

    /// 块里没有的概念：命题跳过，不 panic、不半路丢别的。
    #[test]
    fn propositions_to_a_missing_concept_are_skipped() {
        let block = ConceptMapBlock {
            concepts: vec![ConceptEntry {
                id: "a".into(),
                text: "甲".into(),
                ..Default::default()
            }],
            propositions: vec![
                PropositionEntry {
                    id: "r1".into(),
                    from: "a".into(),
                    to: "b".into(),
                    ..Default::default()
                },
                PropositionEntry {
                    id: "r2".into(),
                    from: "ghost".into(),
                    to: "a".into(),
                    ..Default::default()
                },
            ],
            focus_question: None,
            groups: Vec::new(),
        };
        let nodes = to_nodes(&block);
        assert_eq!(nodes["a"].relationships.len(), 1, "只留源概念在表里的那条");
        assert_eq!(nodes["a"].relationships[0].id, "r1");
    }

    /// 序列化：没有概念时整块不写（老文档一个字节都不变）。
    #[test]
    fn empty_block_is_not_written() {
        let block = ConceptMapBlock::default();
        let json = serde_json::to_string(&block).unwrap();
        assert!(!json.contains("concepts"), "空块不该写 `concepts`：{json}");
    }

    /// `to_nodes` 还原的节点必须是 `parent_id: None` + `pos`（概念的形状）。
    #[test]
    fn concepts_come_back_as_free_nodes_with_positions() {
        let block = ConceptMapBlock {
            concepts: vec![ConceptEntry {
                id: "a".into(),
                text: "甲".into(),
                x: Some(3.0),
                y: Some(-4.5),
                pinned: Some(true),
                style: None,
            }],
            propositions: Vec::new(),
            focus_question: None,
            groups: Vec::new(),
        };
        let nodes = to_nodes(&block);
        let n = &nodes["a"];
        assert!(n.parent_id.is_none(), "概念必须是自由节点");
        assert_eq!(n.pos.as_ref().map(|p| (p.x, p.y)), Some((3.0, -4.5)));
        assert_eq!(n.pinned, Some(true));
    }
}
