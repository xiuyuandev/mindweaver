//! 双击折叠圆点「展开到下一级」（M32 / `docs/02` §3、`docs/06` §5）。
//!
//! 单击圆点是「折叠 ↔ 展开整棵」，双击是**逐级深入**：一次只多露一层。
//! 口径定死成两条：
//!
//! - 节点**折叠**时：先展开它，再把它的**直接子级全部折叠**——恰好露出下一层；
//!   子级此前若展开过，单击会把它们连同各自的子级一起露出来，双击只露一层。
//! - 节点**已展开**时：把**直接子级**展开（再深一层），节点本身保持展开；
//!   单击在这里是「收回整棵」，双击是「往下走」。
//!
//! 规划器只产出**真的会改状态**的 `SetCollapsed`（顺序固定：先父后子），
//! 多条合成一条 [`Command::Batch`]，所以一次双击 = 一条撤销记录。
//! 叶子、未知 id、概念图、以及「状态本来就满足」的情况一律返回 `None`——
//! 不发命令，也不在撤销栈里留一条空记录。

use crate::{Command, Document};

/// 双击 `id` 的折叠圆点要执行的命令；`None` = 这一下没有可改的状态。
///
/// 调用方（壳层）负责把它交给 `Document::apply`，再按需要重建场景。
/// 返回的 `Batch` 已经按「先父后子」排好，撤销/重做由 `ChangeSet` 整体还原。
pub fn drill_toggle_command(doc: &Document, id: &str) -> Option<Command> {
    // 概念图没有折叠语义（`docs/30` §5）：这里显式拒绝，免得调用方误用。
    if doc.is_concept_map() {
        return None;
    }
    let node = doc.node(id)?;
    if node.children.is_empty() {
        return None;
    }

    let mut commands = Vec::new();
    if node.collapsed {
        // 折叠态双击：展开自己，并把直接子级全部折起来——只露一层。
        commands.push(Command::SetCollapsed {
            id: id.to_string(),
            collapsed: false,
        });
        for child in &node.children {
            if let Some(child) = doc.node(child) {
                // 叶子没有「折叠」可言：写它的 `collapsed` 不可见，却会多一条撤销记录。
                if child.children.is_empty() {
                    continue;
                }
                if !child.collapsed {
                    commands.push(Command::SetCollapsed {
                        id: child.id.clone(),
                        collapsed: true,
                    });
                }
            }
        }
    } else {
        // 展开态双击：下一层的子级展开，自己保持展开。
        for child in &node.children {
            if let Some(child) = doc.node(child) {
                if child.children.is_empty() {
                    continue;
                }
                if child.collapsed {
                    commands.push(Command::SetCollapsed {
                        id: child.id.clone(),
                        collapsed: false,
                    });
                }
            }
        }
    }

    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(Command::Batch { commands }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        Document::new("下钻测试")
    }

    fn add(doc: &mut Document, parent: &str, text: &str) -> String {
        let cs = doc
            .apply(&Command::AddNode {
                parent: parent.to_string(),
                index: None,
                text: text.to_string(),
                style: None,
            })
            .expect("建节点");
        let before: std::collections::HashSet<&str> =
            cs.before.iter().map(|n| n.id.as_str()).collect();
        cs.after
            .iter()
            .find(|n| !before.contains(n.id.as_str()))
            .expect("新增节点应出现在 after 中")
            .id
            .clone()
    }

    fn set_collapsed(doc: &mut Document, id: &str, collapsed: bool) {
        doc.apply(&Command::SetCollapsed {
            id: id.to_string(),
            collapsed,
        })
        .expect("改折叠态");
    }

    fn collapsed(doc: &Document, id: &str) -> bool {
        doc.node(id).expect("节点存在").collapsed
    }

    /// 折叠的 A（子级 B、C 曾经展开过）双击圆点：只有 B、C 作为一层出现，
    /// B/C 各自的子级不可见——即 A 展开、B/C 被折起来。
    #[test]
    fn drilling_into_a_collapsed_node_reveals_exactly_one_level() {
        let mut doc = doc();
        let root = doc.root_id.clone();
        let a = add(&mut doc, &root, "A");
        let b = add(&mut doc, &a, "B");
        let _b1 = add(&mut doc, &b, "B1");
        let c = add(&mut doc, &a, "C");
        let _c1 = add(&mut doc, &c, "C1");
        set_collapsed(&mut doc, &a, true);
        // 前提：B、C 自身是展开的（单击会连带露出 B1/C1）。
        assert!(!collapsed(&doc, &b) && !collapsed(&doc, &c));

        let cmd = drill_toggle_command(&doc, &a).expect("折叠节点应能下钻");
        doc.apply(&cmd).expect("下钻命令应成立");

        assert!(!collapsed(&doc, &a), "A 应展开");
        assert!(collapsed(&doc, &b), "B 应被折回一层");
        assert!(collapsed(&doc, &c), "C 应被折回一层");
        assert!(doc.validate().is_ok());
    }

    /// 展开的 A 双击圆点：A 的直接子级展开（再深一层），A 仍展开。
    #[test]
    fn drilling_into_an_expanded_node_opens_only_its_direct_children() {
        let mut doc = doc();
        let root = doc.root_id.clone();
        let a = add(&mut doc, &root, "A");
        let b = add(&mut doc, &a, "B");
        let _b1 = add(&mut doc, &b, "B1");
        set_collapsed(&mut doc, &b, true);
        assert!(!collapsed(&doc, &a));

        let cmd = drill_toggle_command(&doc, &a).expect("展开节点也应能下钻");
        doc.apply(&cmd).expect("下钻命令应成立");

        assert!(!collapsed(&doc, &a), "A 保持展开");
        assert!(!collapsed(&doc, &b), "B 应被展开一层");
        assert!(doc.validate().is_ok());
    }

    /// 状态本来就满足（子级全展开 / 子级全折叠）时不出命令：
    /// 不留空撤销记录，也不改任何节点。
    #[test]
    fn drilling_is_a_noop_when_nothing_would_change() {
        let mut doc = doc();
        let root = doc.root_id.clone();
        let a = add(&mut doc, &root, "A");
        let b = add(&mut doc, &a, "B");
        let c = add(&mut doc, &a, "C");
        // 展开态 + 子级全展开 → 没有可展开的。
        assert!(drill_toggle_command(&doc, &a).is_none());
        // 折叠态 + 子级全折叠 → 只剩「展开 A」一条，仍然有意义，不是 no-op。
        set_collapsed(&mut doc, &a, true);
        set_collapsed(&mut doc, &b, true);
        set_collapsed(&mut doc, &c, true);
        assert!(matches!(
            drill_toggle_command(&doc, &a),
            Some(Command::SetCollapsed {
                collapsed: false,
                ..
            })
        ));
    }

    /// 折叠节点下已经折着的子级不重复发命令；命令顺序是先父后子。
    #[test]
    fn the_plan_skips_children_already_in_the_target_state() {
        let mut doc = doc();
        let root = doc.root_id.clone();
        let a = add(&mut doc, &root, "A");
        let b = add(&mut doc, &a, "B");
        let c = add(&mut doc, &a, "C");
        // B/C 各有子级：叶子不参与折叠，只有「有下一层的子级」才值得改状态。
        add(&mut doc, &b, "B1");
        add(&mut doc, &c, "C1");
        set_collapsed(&mut doc, &a, true);
        set_collapsed(&mut doc, &b, true); // B 已经折着，不该再发一条

        let Some(Command::Batch { commands }) = drill_toggle_command(&doc, &a) else {
            panic!("应合成一条 Batch");
        };
        assert_eq!(commands.len(), 2, "展开 A + 折叠 C，B 跳过");
        assert!(matches!(
            &commands[0],
            Command::SetCollapsed { id, collapsed: false } if id == &a
        ));
        assert!(matches!(
            &commands[1],
            Command::SetCollapsed { id, collapsed: true } if id == &c
        ));
    }

    /// 叶子不参与折叠：它们的 `collapsed` 不可见，写进去只会多一条撤销记录。
    #[test]
    fn leaves_never_get_a_collapsed_flag_written() {
        let mut doc = doc();
        let root = doc.root_id.clone();
        let a = add(&mut doc, &root, "A");
        let leaf = add(&mut doc, &a, "叶子");
        let branch = add(&mut doc, &a, "分支");
        add(&mut doc, &branch, "分支的子级");
        set_collapsed(&mut doc, &a, true);

        let cmd = drill_toggle_command(&doc, &a).expect("应能下钻");
        doc.apply(&cmd).expect("下钻命令应成立");
        assert!(!collapsed(&doc, &leaf), "叶子的折叠态不该被写");
        assert!(collapsed(&doc, &branch), "有下一层的子级才折回");
    }

    /// 一次双击只留一条撤销记录：撤销后 A 回到折叠、B/C 回到展开。
    #[test]
    fn drilling_is_one_undo() {
        let mut doc = doc();
        let root = doc.root_id.clone();
        let a = add(&mut doc, &root, "A");
        let b = add(&mut doc, &a, "B");
        let c = add(&mut doc, &a, "C");
        set_collapsed(&mut doc, &a, true);
        let depth_before = doc.undo_depth();

        let cmd = drill_toggle_command(&doc, &a).expect("应能下钻");
        doc.apply(&cmd).expect("下钻命令应成立");
        assert_eq!(doc.undo_depth(), depth_before + 1, "一次双击只加一步");

        doc.undo().expect("撤销下钻");
        assert!(collapsed(&doc, &a), "A 回到折叠");
        assert!(!collapsed(&doc, &b) && !collapsed(&doc, &c), "B/C 回到展开");
    }

    /// 叶子、未知 id、概念图都不下钻。
    #[test]
    fn leaves_unknown_ids_and_concept_maps_do_not_drill() {
        let mut doc = doc();
        let root = doc.root_id.clone();
        let leaf = add(&mut doc, &root, "叶子");
        assert!(drill_toggle_command(&doc, &leaf).is_none());
        assert!(drill_toggle_command(&doc, "不存在的节点").is_none());

        let a = add(&mut doc, &root, "A");
        add(&mut doc, &a, "A1");
        doc.meta.layout.kind = "conceptMap".to_string();
        assert!(drill_toggle_command(&doc, &a).is_none());
    }
}
