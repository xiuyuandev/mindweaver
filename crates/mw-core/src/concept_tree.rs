//! 概念图的**生成树**：把命题网投影成一棵树，供大纲与文本导出共用。
//!
//! 概念图不是树（`docs/30` §5.1：多根、交叉、成环），但大纲与 Markdown/OPML 只能是层级。
//! 投影规则只有一条，写死在这里，**不允许各个消费者自己再展一遍**——否则大纲里看到的层级
//! 和导出文件里的层级会各说各话（CM-07 的起点审计就是这个坑：四处各写一遍 `children` 递归）。
//!
//! 三条口径：
//!
//! 1. **从主概念开始**：`root_id` 先出，其余概念按 `(y, x, id)` 排——位置就是阅读顺序，
//!    与 `Document::surviving_concept`（删根后挑新根）同一把尺子。
//! 2. **沿有向边前序展开**：出边按 `relationships` 的声明顺序走，**环取首次访问边**
//!    （已访问的终点不再入栈，所以 A→B→C→A 只是回到 A 的那条边被丢掉，不会死循环）。
//! 3. **没被任何边指到的概念各自成根**：概念图允许孤立概念存在，硬塞进别人的子树是编造。
//!
//! 纯函数、零 UI 依赖、不做 I/O；顺序完全确定（不碰 `HashMap` 迭代序）。

use std::collections::HashSet;

use crate::document::Document;

/// 深链兜底：与 `Document::subtree_ids` 同口径，坏数据也不会把栈撑爆。
const MAX_DEPTH: u16 = 4096;

/// 生成树里的一行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConceptRow {
    pub id: String,
    /// 层级：主概念为 0，沿边每走一步 +1。
    pub depth: u16,
    /// 在生成树里的父概念（第一次把它拉进树的那条边的起点）。
    pub parent: Option<String>,
    /// 进来那条边的链接短语。导出时它是命题的另一半，丢了层级就只剩名词。
    pub via: Option<String>,
    /// 进来那条边的 id（CM-09）。大纲要改/删这条边只能按 id 点名：
    /// 同一对概念之间允许有两句话，按 to 找会删错。根行与孤立概念是 None。
    pub via_id: Option<String>,
    /// 分支号（CM-09，docs/30 §5.4.1）：depth == 1 那个祖先在出场序列里的序号。
    /// 主概念自己（0）与每个孤立概念各占一号；跨分支连线按它取来源分支色。
    pub branch: u32,
}

/// 概念图 → 生成树（前序）。非概念图文档返回空表：调用方自己决定回落到树。
pub fn concept_rows(doc: &Document) -> Vec<ConceptRow> {
    if !doc.is_concept_map() {
        return Vec::new();
    }
    let mut visited: HashSet<String> = HashSet::new();
    let mut rows: Vec<ConceptRow> = Vec::new();
    let mut next_branch: u32 = 0;
    for root in root_order(doc) {
        if visited.contains(&root) {
            continue;
        }
        // (id, depth, parent, via, via_id, branch)：显式栈，前端先出，所以入栈要倒序。
        // branch 是 Option：None = 「出栈时现发一个号」（主概念与一级概念各占一号），
        // Some = 继承（二级以下全归它那个一级祖先）。
        let mut stack: Vec<(
            String,
            u16,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<u32>,
        )> = vec![(root, 0, None, None, None, None)];
        while let Some((id, depth, parent, via, via_id, slot)) = stack.pop() {
            // 出栈才标记：同一条边被两条路径指到时，**先到的那条**定父。
            if !visited.insert(id.clone()) {
                continue;
            }
            let Some(node) = doc.node(&id) else {
                continue;
            };
            let branch = match slot {
                Some(b) => b,
                None => {
                    let b = next_branch;
                    next_branch += 1;
                    b
                }
            };
            rows.push(ConceptRow {
                id: id.clone(),
                depth,
                parent,
                via,
                via_id,
                branch,
            });
            if depth >= MAX_DEPTH {
                continue;
            }
            for rel in node.relationships.iter().rev() {
                // 自环与已访问终点都不再入栈：这就是「环取首次访问边」。
                if rel.to == id || visited.contains(&rel.to) || doc.node(&rel.to).is_none() {
                    continue;
                }
                let label = rel
                    .label
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                // 分支号在 depth == 1 处定型：一级概念自己就是那条分支的名字，
                // 再往下走全部继承它（docs/30 §5.4.1）。
                let child_slot = if depth == 0 { None } else { Some(branch) };
                stack.push((
                    rel.to.clone(),
                    depth + 1,
                    Some(id.clone()),
                    label,
                    Some(rel.id.clone()),
                    child_slot,
                ));
            }
        }
    }
    rows
}

/// 根的出场顺序：主概念最先，其余按「左上 → 右下」排（同 y 比 x，最后用 id 兜底）。
fn root_order(doc: &Document) -> Vec<String> {
    let mut roots: Vec<String> = doc.nodes.keys().cloned().collect();
    roots.sort_by(|a, b| {
        let key = |id: &str| {
            let (y, x) = doc
                .node(id)
                .and_then(|n| n.pos.as_ref())
                .map(|p| (p.y, p.x))
                .unwrap_or((0.0, 0.0));
            (y, x, id.to_string())
        };
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if doc.node(&doc.root_id).is_some() {
        roots.retain(|id| id != &doc.root_id);
        roots.insert(0, doc.root_id.clone());
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, LayoutSettings, Node};

    /// 概念图夹具：第一个概念当主概念（带 `pos`），其余按给定坐标加浮动概念。
    fn concept_doc(concepts: &[(&str, f32, f32)]) -> (Document, Vec<String>) {
        let mut d = Document::new("概念图");
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .expect("切概念图");
        let root = d.root_id.clone();
        d.apply(&Command::SetText {
            id: root.clone(),
            text: concepts[0].0.into(),
        })
        .expect("主概念改名");
        d.apply(&Command::MoveConcept {
            id: root.clone(),
            x: concepts[0].1,
            y: concepts[0].2,
        })
        .expect("主概念摆位");
        let mut ids = vec![root];
        for (text, x, y) in &concepts[1..] {
            d.apply(&Command::AddFloatingTopic {
                text: (*text).into(),
                x: *x,
                y: *y,
            })
            .expect("加概念");
            let id = d
                .nodes
                .values()
                .find(|n| &n.text == text)
                .map(|n| n.id.clone())
                .expect("新概念");
            ids.push(id);
        }
        (d, ids)
    }

    fn link(d: &mut Document, from: &str, to: &str, label: &str) {
        d.apply(&Command::AddRelationship {
            from: from.into(),
            to: to.into(),
            label: (!label.is_empty()).then(|| label.into()),
        })
        .expect("加连接");
    }

    fn shape(rows: &[ConceptRow]) -> Vec<(String, u16, Option<String>, Option<String>)> {
        rows.iter()
            .map(|r| (r.id.clone(), r.depth, r.parent.clone(), r.via.clone()))
            .collect()
    }

    #[test]
    fn rows_follow_edges_in_declaration_order_with_depth_and_phrase() {
        let (mut d, ids) = concept_doc(&[
            ("用户痛点", 0.0, 0.0),
            ("竞品访谈", 200.0, 0.0),
            ("留存低", 400.0, 0.0),
        ]);
        link(&mut d, &ids[0], &ids[1], "因为");
        link(&mut d, &ids[1], &ids[2], "导致");

        let rows = concept_rows(&d);
        assert_eq!(
            shape(&rows),
            vec![
                (ids[0].clone(), 0, None, None),
                (ids[1].clone(), 1, Some(ids[0].clone()), Some("因为".into())),
                (ids[2].clone(), 2, Some(ids[1].clone()), Some("导致".into())),
            ]
        );
    }

    #[test]
    fn cycles_are_cut_at_the_first_visit_and_never_loop() {
        let (mut d, ids) = concept_doc(&[("甲", 0.0, 0.0), ("乙", 200.0, 0.0), ("丙", 400.0, 0.0)]);
        // A→B→C→A 回环。自环虽然被 `AddRelationship` 拦着（「不能把节点连接到自身」），
        // 手改过的存档仍可能带一条，投影侧照样要挡（下面的 `rel.to == id` 就是那道闸）。
        link(&mut d, &ids[0], &ids[1], "因为");
        link(&mut d, &ids[1], &ids[2], "导致");
        link(&mut d, &ids[2], &ids[0], "反过来");
        assert!(
            d.apply(&Command::AddRelationship {
                from: ids[2].clone(),
                to: ids[2].clone(),
                label: None,
            })
            .is_err(),
            "自环在命令层就该被拒"
        );

        let rows = concept_rows(&d);
        assert_eq!(rows.len(), 3, "每个概念只出一行");
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[0].parent, None, "回到主概念的那条边要被丢掉");
        assert_eq!(rows[2].depth, 2);
        // 确定性：连算两次必须一字不差。
        assert_eq!(shape(&rows), shape(&concept_rows(&d)));
    }

    #[test]
    fn first_visit_edge_wins_when_two_edges_point_at_one_concept() {
        let (mut d, ids) =
            concept_doc(&[("甲", 0.0, 0.0), ("乙", 200.0, 0.0), ("丙", 100.0, 200.0)]);
        // 甲同时指向丙与乙；乙也指向丙。前序里丙先被甲拉进树，乙的那条边作废。
        link(&mut d, &ids[0], &ids[2], "先到");
        link(&mut d, &ids[0], &ids[1], "另一条");
        link(&mut d, &ids[1], &ids[2], "后到");

        let rows = concept_rows(&d);
        let c = rows.iter().find(|r| r.id == ids[2]).expect("丙在树里");
        assert_eq!(c.parent.as_deref(), Some(ids[0].as_str()));
        assert_eq!(c.via.as_deref(), Some("先到"));
        assert_eq!(rows.iter().filter(|r| r.id == ids[2]).count(), 1);
    }

    #[test]
    fn unreachable_concepts_become_roots_ordered_by_position() {
        let (mut d, ids) = concept_doc(&[
            ("主概念", 0.0, 0.0),
            ("连着的", 200.0, 0.0),
            ("孤儿下", 0.0, 300.0),
            ("孤儿上", 0.0, 200.0),
        ]);
        link(&mut d, &ids[0], &ids[1], "因为");

        let rows = concept_rows(&d);
        assert_eq!(rows.len(), 4, "孤立概念不能被丢掉");
        let roots: Vec<&str> = rows
            .iter()
            .filter(|r| r.depth == 0)
            .map(|r| r.id.as_str())
            .collect();
        // 主概念先出；两个孤儿按 y 从小到大（上 → 下）。
        assert_eq!(
            roots,
            vec![ids[0].as_str(), ids[3].as_str(), ids[2].as_str()]
        );
    }

    #[test]
    fn tree_documents_have_no_concept_rows() {
        let mut d = Document::new("思维导图");
        let root = d.root_id.clone();
        let child = crate::new_id();
        d.nodes.insert(
            child.clone(),
            Node::new(child.clone(), Some(root.clone()), "子主题"),
        );
        d.nodes.get_mut(&root).unwrap().children.push(child);
        assert!(concept_rows(&d).is_empty(), "非概念图不投影");
    }

    /// CM-09：每行都要带「进来那条边」的 id（断边/改短语得按 id 点名），
    /// 以及分支号（跨分支取色用）。分支号在一级概念处定型，再往下继承。
    #[test]
    fn rows_carry_edge_id_and_branch_number() {
        let (mut d, ids) = concept_doc(&[
            ("主概念", 0.0, 0.0),
            ("甲", 240.0, 0.0),
            ("甲下", 420.0, 0.0),
            ("乙", 0.0, 240.0),
        ]);
        link(&mut d, &ids[0], &ids[1], "其一");
        link(&mut d, &ids[1], &ids[2], "其二");
        link(&mut d, &ids[0], &ids[3], "其三");

        let rows = concept_rows(&d);
        let find = |id: &str| rows.iter().find(|r| r.id == id).expect("行在树里");

        // 主概念没有进来的边：两个字段都得空着，否则会删错边。
        assert_eq!(rows[0].via_id, None);
        assert_eq!(rows[0].via, None);

        let a = find(&ids[1]);
        let a2 = find(&ids[2]);
        let b = find(&ids[3]);
        let rel = d
            .node(&ids[0])
            .and_then(|n| n.relationships.iter().find(|r| r.to == ids[1]))
            .expect("甲那条边");
        assert_eq!(a.via_id.as_deref(), Some(rel.id.as_str()), "边 id 直接可取");
        assert_eq!(a.via.as_deref(), Some("其一"));
        assert_eq!(a.parent.as_deref(), Some(ids[0].as_str()));

        // 同一条分支里后代共号；两条一级分支必须不同号；根不占分支号。
        assert_eq!(a.branch, a2.branch, "二级沿用一级的分支号");
        assert_ne!(a.branch, b.branch, "兄弟分支各占一号");
        assert_ne!(rows[0].branch, a.branch, "主概念不参与分支取色");
    }
}
