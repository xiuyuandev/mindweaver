//! 知识视图：卡片、标签、来源主题的关系图。纯布局，不碰 wgpu/egui。
//!
//! 节点类型：卡片（圆）、标签（方块）、主题（菱形）。标签放在内环，卡片围绕
//! 它的第一个标签散开，主题在外环；没有标签的卡片走外环。布局是 O(n) 的确定性
//! 径向散布，1000 张卡片不需要力导向迭代——`knowledge_layout_1000_cards_is_fast` 钉住。

use std::collections::BTreeMap;

/// 构建关系图用的最小卡片信息（不读正文，1000 张也不会碰 1000 个文件）。
#[derive(Clone, Debug)]
pub struct CardInput {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub source_node: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Card,
    Tag,
    Topic,
}

#[derive(Clone, Debug)]
pub struct GNode {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GEdge {
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<GNode>,
    pub edges: Vec<GEdge>,
    pub bounds: (f32, f32, f32, f32),
}

const TAG_RING: f32 = 360.0;
const CARD_RING: f32 = 560.0;
const TOPIC_RING: f32 = 760.0;

fn angle(i: usize, n: usize) -> f32 {
    std::f32::consts::TAU * (i as f32 + 0.5) / n.max(1) as f32
}

/// 确定性抖动：同一张卡片每次打开都落在同一处，不靠随机数。
fn jitter(seed: &str) -> f32 {
    let mut h: u64 = 1469598103934665603;
    for b in seed.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    ((h % 1000) as f32 / 1000.0 - 0.5) * 60.0
}

pub fn build(cards: &[CardInput]) -> Graph {
    let mut tags: BTreeMap<String, usize> = BTreeMap::new();
    let mut topics: BTreeMap<String, usize> = BTreeMap::new();
    for c in cards {
        for t in &c.tags {
            let next = tags.len();
            tags.entry(t.clone()).or_insert(next);
        }
        if let Some(t) = &c.source_node {
            let next = topics.len();
            topics.entry(t.clone()).or_insert(next);
        }
    }
    let tag_n = tags.len();
    let topic_n = topics.len();
    let mut nodes = Vec::with_capacity(tag_n + topic_n + cards.len());
    for (tag, i) in &tags {
        let a = angle(*i, tag_n);
        nodes.push(GNode {
            id: format!("tag:{tag}"),
            label: format!("#{tag}"),
            kind: Kind::Tag,
            x: a.cos() * TAG_RING,
            y: a.sin() * TAG_RING,
            r: 24.0,
        });
    }
    let tag_base = nodes.len();
    for (topic, i) in &topics {
        let a = angle(*i, topic_n) + 0.17;
        nodes.push(GNode {
            id: format!("topic:{topic}"),
            label: topic.chars().take(12).collect(),
            kind: Kind::Topic,
            x: a.cos() * TOPIC_RING,
            y: a.sin() * TOPIC_RING,
            r: 18.0,
        });
    }
    let topic_base = tag_base;
    let mut edges = Vec::new();
    for (n, c) in cards.iter().enumerate() {
        let a = c
            .tags
            .first()
            .and_then(|t| tags.get(t))
            .map(|i| angle(*i, tag_n) + jitter(&c.id) / TAG_RING)
            .unwrap_or_else(|| std::f32::consts::TAU * n as f32 / cards.len().max(1) as f32);
        let r = CARD_RING + jitter(&c.id).abs();
        let idx = nodes.len();
        nodes.push(GNode {
            id: format!("card:{}", c.id),
            label: if c.title.trim().is_empty() {
                "未命名卡片".into()
            } else {
                c.title.clone()
            },
            kind: Kind::Card,
            x: a.cos() * r,
            y: a.sin() * r,
            r: 13.0,
        });
        for t in &c.tags {
            if let Some(i) = tags.get(t) {
                edges.push(GEdge {
                    from: idx,
                    to: tag_base + i,
                });
            }
        }
        if let Some(t) = &c.source_node {
            if let Some(i) = topics.get(t) {
                edges.push(GEdge {
                    from: idx,
                    to: topic_base + i,
                });
            }
        }
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for n in &nodes {
        min_x = min_x.min(n.x - n.r);
        min_y = min_y.min(n.y - n.r);
        max_x = max_x.max(n.x + n.r);
        max_y = max_y.max(n.y + n.r);
    }
    if nodes.is_empty() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 1.0;
        max_y = 1.0;
    }
    Graph {
        nodes,
        edges,
        bounds: (min_x, min_y, max_x, max_y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards(n: usize) -> Vec<CardInput> {
        (0..n)
            .map(|i| CardInput {
                id: format!("c{i}"),
                title: format!("卡片 {i}"),
                tags: vec![format!("t{}", i % 50), "all".to_string()],
                source_node: if i % 3 == 0 {
                    Some(format!("node{}", i % 20))
                } else {
                    None
                },
            })
            .collect()
    }

    #[test]
    fn graph_links_cards_to_tags_and_topics() {
        let g = build(&cards(10));
        assert!(g.nodes.iter().any(|n| n.kind == Kind::Tag));
        assert!(g.nodes.iter().any(|n| n.kind == Kind::Topic));
        assert!(g.nodes.iter().any(|n| n.kind == Kind::Card));
        assert!(!g.edges.is_empty());
        // 每条边都必须指向存在的节点。
        for e in &g.edges {
            assert!(e.from < g.nodes.len() && e.to < g.nodes.len());
        }
    }

    #[test]
    fn knowledge_layout_1000_cards_is_fast() {
        let start = std::time::Instant::now();
        let g = build(&cards(1000));
        let took = start.elapsed();
        assert_eq!(
            g.nodes.iter().filter(|n| n.kind == Kind::Card).count(),
            1000
        );
        assert!(
            took.as_millis() < 500,
            "1000 卡片布局应 <500ms，实际 {}ms",
            took.as_millis()
        );
    }
}
