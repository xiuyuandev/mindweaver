//! 拖拽落点：三分区判定、子树禁令、多根插入索引。
//!
//! 全部是纯函数，输入 `Scene` + 一棵由它派生的 [`Tree`]，输出「该把哪个节点挂到
//! 哪个父的第几位」。壳层负责画指示条，`mw-core::Document` 负责真正改树——
//! 这里**不碰文档**，所以同一份判定既能被真实拖拽用，也能被单测逐帧喂坐标验证。

use std::collections::HashSet;

use mw_core::{Command, Point, Scene};

/// 按下后移动多少**屏幕像素**才从「点击」升格为「拖拽」（docs/06 §4）。
pub const DRAG_THRESHOLD: f32 = 4.0;
/// 落点候选盒的外扩（CSS px）。竖向比横向紧，是因为同层节点上下堆得密，
/// 松一档就会在相邻两行之间串指示条。
pub const DROP_PAD_X: f32 = 6.0;
pub const DROP_PAD_Y: f32 = 4.0;
/// 上缘 28% = 插到它前面，下缘 28% = 插到它后面，中间 = 成为它的子节点。
pub const BEFORE_BELOW: f32 = 0.28;
pub const AFTER_ABOVE: f32 = 0.72;

/// 三分区落点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// 上缘蓝条：插到目标之前（同父兄弟）。
    Before,
    /// 整框高亮：成为目标的子节点。
    Child,
    /// 下缘蓝条：插到目标之后。
    After,
}

/// 一次拖拽命中的落点。`illegal` = 命中的是被拖动的那几棵子树之一。
///
/// 旧 Svelte 版把子树直接从命中里剔掉，于是「拖到自己孩子头上」什么提示都没有，
/// 用户以为拖拽卡住了。docs/06 §4 要的是红色指示 + 禁止光标，这里照做。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropTarget {
    pub node: u32,
    pub zone: Zone,
    pub illegal: bool,
}

/// 一次移动：把 `node` 挂到 `parent` 的第 `index` 位（`None` = 追加）。
///
/// `index` 的口径与 `Document::apply(MoveNode)` 一致：**以移动发生前的兄弟数组为参照**，
/// 所以同父重排时不必自己减一。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub node: u32,
    pub parent: u32,
    pub index: Option<usize>,
}

/// 场景索引下的父子表。`build_scene` 按 DFS 前序发号，所以按索引顺序收集到的
/// 兄弟天然就是文档顺序——这是它能当落点索引参照的前提。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    children: Vec<Vec<u32>>,
    parent: Vec<Option<u32>>,
}

impl Tree {
    pub fn build(scene: &Scene) -> Self {
        let mut children = vec![Vec::new(); scene.nodes.len()];
        let mut parent = vec![None; scene.nodes.len()];
        for (idx, n) in scene.nodes.iter().enumerate() {
            parent[idx] = n.parent;
            if let Some(p) = n.parent {
                if let Some(list) = children.get_mut(p as usize) {
                    list.push(idx as u32);
                }
            }
        }
        Self { children, parent }
    }

    pub fn children(&self, parent: u32) -> &[u32] {
        self.children
            .get(parent as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn parent_of(&self, node: u32) -> Option<u32> {
        self.parent.get(node as usize).copied().flatten()
    }

    fn position(&self, parent: u32, node: u32) -> Option<usize> {
        self.children(parent).iter().position(|&c| c == node)
    }

    /// 把一次移动作用到表上，索引换算与 `Document` 的 `MoveNode` 逐条对齐：
    /// 同父时先按「含自己的数组」解释 `index`，再自己减掉移除的那一位。
    pub fn apply_move(&mut self, node: u32, parent: u32, index: Option<usize>) {
        let old = self.parent_of(node);
        let pos = old.and_then(|o| self.position(o, node));
        let idx = match (old, pos) {
            (Some(o), Some(p)) if o == parent => {
                let pre_len = self.children[o as usize].len() + 1;
                let want = index.unwrap_or(pre_len).min(pre_len);
                if want > p {
                    want - 1
                } else {
                    want
                }
            }
            _ => {
                let len = self.children(parent).len();
                index.unwrap_or(len).min(len)
            }
        };
        if let (Some(o), Some(p)) = (old, pos) {
            self.children[o as usize].remove(p);
        }
        let idx = idx.min(self.children[parent as usize].len());
        self.children[parent as usize].insert(idx, node);
        if let Some(slot) = self.parent.get_mut(node as usize) {
            *slot = Some(parent);
        }
    }
}

/// 参与拖拽的「根」：选中集合里**祖先没被一起选中**的那些。
///
/// 子树整组跟着走，所以父节点在组里时它的后代会被父的幽灵一起带走；把后代也算成
/// 一根会让它被移动两次。
pub fn drag_roots(tree: &Tree, sel: &[u32]) -> Vec<u32> {
    let set: HashSet<u32> = sel.iter().copied().collect();
    sel.iter()
        .copied()
        .filter(|&i| {
            let mut p = tree.parent_of(i);
            while let Some(a) = p {
                if set.contains(&a) {
                    return false;
                }
                p = tree.parent_of(a);
            }
            true
        })
        .collect()
}

/// 被拖动的整片子树（含根自己）——落点命中到它就判非法。
pub fn subtree(tree: &Tree, roots: &[u32]) -> HashSet<u32> {
    let mut out = HashSet::new();
    let mut stack: Vec<u32> = roots.to_vec();
    while let Some(i) = stack.pop() {
        if out.insert(i) {
            stack.extend(tree.children(i).iter().copied());
        }
    }
    out
}

/// 指针世界坐标处的落点：候选盒外扩 `DROP_PAD_*`，**面积最小者胜**
/// （父盒套着子盒时，用户指的是里面那个）。
pub fn drop_target(scene: &Scene, at: Point, banned: &HashSet<u32>) -> Option<DropTarget> {
    let mut best: Option<(u32, f32)> = None;
    for &idx in scene.draw_order.iter() {
        let r = scene.rect(idx);
        if at.x < r.x - DROP_PAD_X
            || at.x > r.right() + DROP_PAD_X
            || at.y < r.y - DROP_PAD_Y
            || at.y > r.bottom() + DROP_PAD_Y
        {
            continue;
        }
        let area = r.w.max(1.0) * r.h.max(1.0);
        if best.is_none_or(|(_, a)| area < a) {
            best = Some((idx, area));
        }
    }
    let (node, _) = best?;
    let illegal = banned.contains(&node);
    let zone = if illegal || scene.node(node).parent.is_none() {
        Zone::Child
    } else {
        let r = scene.rect(node);
        let rel = (at.y - r.y) / r.h.max(1.0);
        if rel < BEFORE_BELOW {
            Zone::Before
        } else if rel > AFTER_ABOVE {
            Zone::After
        } else {
            Zone::Child
        }
    };
    Some(DropTarget {
        node,
        zone,
        illegal,
    })
}

/// 把落点摊到每一根上，算出**逐条**的移动计划。
///
/// 逐条模拟父子表是刻意的：`Batch` 里的命令会依次作用，第二条看到的兄弟数组已经
/// 被第一条改过。旧版用 `+seq` 近似，同父多根重排时会差一位。
/// 计划最终等价于原地（比如「拖回自己原来那个位置」）时返回空，撤销栈不被脏操作污染。
pub fn plan_moves(tree: &Tree, roots: &[u32], dt: DropTarget) -> Vec<Move> {
    if dt.illegal || roots.is_empty() {
        return Vec::new();
    }
    // `illegal` 是壳层的显示判定，可能来自上一帧；这里再按当前 roots 算一遍子树，
    // 保证「绝不把节点移进自己的子树」这条不变量由纯逻辑兜底，而不是靠 UI 记得住。
    if subtree(tree, roots).contains(&dt.node) {
        return Vec::new();
    }
    let mut state = tree.clone();
    let mut out = Vec::with_capacity(roots.len());
    let mut prev: Option<(u32, usize)> = None;
    for &r in roots {
        let (slot_parent, slot_index) = slot(&state, dt);
        // 同一父下从第二根起落在前一根后面：否则多根会被反着插进去。
        let (parent, index) = match prev {
            Some((p, placed)) if p == slot_parent => (slot_parent, Some(placed + 1)),
            _ => (slot_parent, slot_index),
        };
        out.push(Move {
            node: r,
            parent,
            index,
        });
        state.apply_move(r, parent, index);
        prev = state.position(parent, r).map(|placed| (parent, placed));
    }
    if state == *tree {
        Vec::new()
    } else {
        out
    }
}

fn slot(tree: &Tree, dt: DropTarget) -> (u32, Option<usize>) {
    if dt.zone == Zone::Child {
        return (dt.node, None);
    }
    match tree.parent_of(dt.node) {
        Some(p) => match tree.position(p, dt.node) {
            Some(base) => (p, Some(base + if dt.zone == Zone::After { 1 } else { 0 })),
            // 目标不在它父的兄弟表里（自由主题根）：只能挂成子节点。
            None => (dt.node, None),
        },
        None => (dt.node, None),
    }
}

/// 移动计划 → 命令。一条单发，多条包 `Batch`（一次撤销、失败整体回滚）。
///
/// 落点父节点若是**折叠**态，同一条 `Batch` 里把它展开。不展开的话子树确实挂进去了，
/// 但屏幕上一个像素都不变——用户以为拖失败了，会再拖一次（然后又搬一遍）。
/// 展开必须和移动共用一条撤销：`Ctrl+Z` 一次要回到「旧父 + 仍折叠」。
pub fn move_commands(scene: &Scene, moves: &[Move]) -> Option<Command> {
    let mut commands: Vec<Command> = Vec::with_capacity(moves.len() + 1);
    let mut expanded: HashSet<u32> = HashSet::new();
    for m in moves {
        commands.push(Command::MoveNode {
            id: scene.ids[m.node as usize].clone(),
            new_parent: scene.ids[m.parent as usize].clone(),
            index: m.index,
        });
        // 同一个父只展开一次；多根落进同一个折叠父时不该叠几条 `SetCollapsed`。
        if expanded.insert(m.parent) && scene.node(m.parent).collapsed {
            commands.push(Command::SetCollapsed {
                id: scene.ids[m.parent as usize].clone(),
                collapsed: false,
            });
        }
    }
    match commands.len() {
        0 => None,
        1 => commands.into_iter().next(),
        _ => Some(Command::Batch { commands }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::text::MonoMeasurer;
    use mw_core::{build_scene, get_theme, Document, LayoutSettings, Point};

    /// 根「织思」下三个兄弟 A/B/C，A 下还有 A1。
    struct Fixture {
        scene: Scene,
        tree: Tree,
    }

    impl Fixture {
        fn idx(&self, text: &str) -> u32 {
            self.scene
                .nodes
                .iter()
                .position(|n| n.geom.lines.join("") == text)
                .expect("夹具里有这个节点") as u32
        }
    }

    fn fixture() -> Fixture {
        let mut doc = Document::new("织思");
        let root = doc.root_id.clone();
        doc.apply(&Command::SetText {
            id: root.clone(),
            text: "织思".into(),
        })
        .unwrap();
        let mut a = String::new();
        for t in ["A", "B", "C"] {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: t.into(),
                style: None,
            })
            .unwrap();
            if t == "A" {
                a = doc.doc_order().into_iter().last().unwrap();
            }
        }
        doc.apply(&Command::AddNode {
            parent: a,
            index: None,
            text: "A1".into(),
            style: None,
        })
        .unwrap();
        let scene = build_scene(
            &doc,
            &doc.root_id,
            &LayoutSettings::default(),
            &get_theme("snow"),
            &MonoMeasurer,
            None::<(&str, &str)>,
        );
        let tree = Tree::build(&scene);
        Fixture { scene, tree }
    }

    #[test]
    fn siblings_are_collected_in_document_order() {
        let f = fixture();
        let root = f.idx("织思");
        let names: Vec<String> = f
            .tree
            .children(root)
            .iter()
            .map(|&i| f.scene.nodes[i as usize].geom.lines.join(""))
            .collect();
        // 单测里 `children` 的顺序就是落点索引的参照系，必须先钉死。
        assert_eq!(names, ["A", "B", "C"]);
    }

    /// 拖进**折叠**的父节点：同一条 `Batch` 里必须带展开。
    ///
    /// 不带展开的话子树确实挂进去了，但屏幕上按不出来任何变化——用户会以为拖失败、
    /// 再拖一次，于是同一片子树被搬了两遍。
    #[test]
    fn dropping_into_a_collapsed_parent_expands_it_in_the_same_undo_step() {
        let mut doc = Document::new("织思");
        let root = doc.root_id.clone();
        let mut a = String::new();
        let mut b = String::new();
        for t in ["A", "B"] {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: t.into(),
                style: None,
            })
            .unwrap();
            let id = doc.doc_order().into_iter().last().unwrap();
            if t == "A" {
                a = id;
            } else {
                b = id;
            }
        }
        doc.apply(&Command::AddNode {
            parent: a.clone(),
            index: None,
            text: "A1".into(),
            style: None,
        })
        .unwrap();
        doc.apply(&Command::SetCollapsed {
            id: a.clone(),
            collapsed: true,
        })
        .unwrap();

        let settings = LayoutSettings::default();
        let theme = get_theme("snow");
        let scene = build_scene(
            &doc,
            &doc.root_id,
            &settings,
            &theme,
            &MonoMeasurer,
            None::<(&str, &str)>,
        );
        let tree = Tree::build(&scene);
        let idx = |text: &str| {
            scene
                .nodes
                .iter()
                .position(|n| n.geom.lines.join("") == text)
                .expect("夹具里有这个节点") as u32
        };
        // 折叠之后 A1 不在场景里——这正是「拖进去看不见」的机制本身。
        assert!(scene.nodes.iter().all(|n| n.geom.lines.join("") != "A1"));

        let dt = DropTarget {
            node: idx("A"),
            zone: Zone::Child,
            illegal: false,
        };
        let moves = plan_moves(&tree, &[idx("B")], dt);
        let cmd = move_commands(&scene, &moves).expect("B 应当可以移进 A");
        let Command::Batch { commands } = &cmd else {
            panic!("移动 + 展开必须是 Batch，否则撤销分两步");
        };
        assert!(commands.iter().any(|c| matches!(
            c,
            Command::SetCollapsed {
                id,
                collapsed: false
            } if id == &a
        )));

        doc.apply(&cmd).unwrap();
        assert!(
            !doc.node(&a).unwrap().collapsed,
            "落点父节点必须展开，否则屏幕上什么都看不见"
        );
        assert!(doc.node(&a).unwrap().children.contains(&b));

        // 一次撤销同时回到「旧父 + 仍折叠」。
        assert!(doc.undo().is_some());
        assert!(doc.node(&a).unwrap().collapsed);
        assert!(doc.node(&root).unwrap().children.contains(&b));
    }

    #[test]
    fn the_three_zones_split_at_28_and_72_percent() {
        let f = fixture();
        let banned = HashSet::new();
        let b = f.idx("B");
        let r = f.scene.rect(b);
        let at = |rel: f32| Point::new(r.x + r.w * 0.5, r.y + r.h * rel);
        let zone = |rel: f32| drop_target(&f.scene, at(rel), &banned).unwrap();
        assert_eq!(
            zone(0.10),
            DropTarget {
                node: b,
                zone: Zone::Before,
                illegal: false
            }
        );
        assert_eq!(zone(0.50).zone, Zone::Child);
        assert_eq!(zone(0.95).zone, Zone::After);
    }

    #[test]
    fn the_root_only_ever_accepts_children() {
        let f = fixture();
        let root = f.idx("织思");
        let r = f.scene.rect(root);
        let dt = drop_target(
            &f.scene,
            Point::new(r.x + r.w * 0.5, r.y + 1.0),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(
            (dt.node, dt.zone),
            (root, Zone::Child),
            "根没有父，插不到谁前面"
        );
    }

    #[test]
    fn dropping_onto_the_dragged_subtree_is_flagged_not_silent() {
        let f = fixture();
        let a = f.idx("A");
        let a1 = f.idx("A1");
        let banned = subtree(&f.tree, &[a]);
        assert!(banned.contains(&a1), "子树必须含后代");
        let r = f.scene.rect(a1);
        let dt = drop_target(&f.scene, r.center(), &banned).unwrap();
        assert_eq!(
            (dt.node, dt.illegal),
            (a1, true),
            "拖到自己孩子头上要给出红色指示，而不是没有落点"
        );
        assert!(
            plan_moves(&f.tree, &[a], dt).is_empty(),
            "非法落点不产生计划"
        );
    }

    #[test]
    fn the_smallest_candidate_box_wins() {
        let f = fixture();
        let a = f.idx("A");
        let a1 = f.idx("A1");
        // A1 贴在 A 里面：指针落在 A1 中心时不能算到父节点上。
        let dt = drop_target(&f.scene, f.scene.rect(a1).center(), &HashSet::new()).unwrap();
        assert_eq!(dt.node, a1);
        assert_ne!(dt.node, a);
    }

    #[test]
    fn moving_a_first_child_after_a_sibling_lands_where_the_pointer_says() {
        let f = fixture();
        let (a, b, c) = (f.idx("A"), f.idx("B"), f.idx("C"));
        let root = f.idx("织思");
        let dt = DropTarget {
            node: c,
            zone: Zone::After,
            illegal: false,
        };
        let moves = plan_moves(&f.tree, &[a], dt);
        assert_eq!(moves.len(), 1);
        // index 以「移动前」的 [A,B,C] 为参照：C 在下标 2，之后就是 3。
        assert_eq!(
            moves[0],
            Move {
                node: a,
                parent: root,
                index: Some(3)
            }
        );
        let mut state = f.tree.clone();
        state.apply_move(a, root, Some(3));
        assert_eq!(state.children(root), vec![b, c, a], "落点应当排在 C 之后");
    }

    #[test]
    fn several_roots_dropped_after_one_sibling_keep_their_order() {
        let f = fixture();
        let (a, b, c) = (f.idx("A"), f.idx("B"), f.idx("C"));
        let root = f.idx("织思");
        let dt = DropTarget {
            node: c,
            zone: Zone::After,
            illegal: false,
        };
        let moves = plan_moves(&f.tree, &[a, b], dt);
        assert_eq!(moves.len(), 2);
        let mut state = f.tree.clone();
        for m in &moves {
            state.apply_move(m.node, m.parent, m.index);
        }
        assert_eq!(
            state.children(root),
            vec![c, a, b],
            "A、B 要按拖动顺序排在 C 后"
        );
    }

    #[test]
    fn a_multi_root_drop_that_is_already_in_place_is_a_no_op() {
        let f = fixture();
        let (a, b, c) = (f.idx("A"), f.idx("B"), f.idx("C"));
        // A、B 本来就在 C 前面：逐条模拟之后树形不变，不该产生命令。
        let dt = DropTarget {
            node: c,
            zone: Zone::Before,
            illegal: false,
        };
        assert!(plan_moves(&f.tree, &[a, b], dt).is_empty());
    }

    #[test]
    fn a_drop_that_changes_nothing_produces_no_command() {
        let f = fixture();
        let c = f.idx("C");
        // C 已经是最后一个：把它再丢回「C 之后」等于没动，不该留一条撤销记录。
        let dt = DropTarget {
            node: c,
            zone: Zone::After,
            illegal: false,
        };
        assert!(plan_moves(&f.tree, &[c], dt).is_empty());
    }

    #[test]
    fn a_node_dropped_onto_itself_is_rejected_even_if_the_hint_said_otherwise() {
        let f = fixture();
        let c = f.idx("C");
        let dt = DropTarget {
            node: c,
            zone: Zone::Child,
            illegal: false,
        };
        assert!(
            plan_moves(&f.tree, &[c], dt).is_empty(),
            "壳层的 illegal 只是显示判定，纯逻辑必须自己守住这条"
        );
    }

    #[test]
    fn moving_a_node_under_a_sibling_is_a_real_change() {
        let f = fixture();
        let (a, c) = (f.idx("A"), f.idx("C"));
        let dt = DropTarget {
            node: c,
            zone: Zone::Child,
            illegal: false,
        };
        assert_eq!(
            plan_moves(&f.tree, &[a], dt),
            vec![Move {
                node: a,
                parent: c,
                index: None
            }]
        );
    }

    #[test]
    fn drag_roots_drop_descendants_that_come_along_anyway() {
        let f = fixture();
        let (a, a1, b) = (f.idx("A"), f.idx("A1"), f.idx("B"));
        assert_eq!(drag_roots(&f.tree, &[a, a1, b]), vec![a, b]);
        assert_eq!(drag_roots(&f.tree, &[a1]), vec![a1]);
    }

    #[test]
    fn one_move_is_a_bare_command_and_several_are_a_batch() {
        let f = fixture();
        let (a, b, c) = (f.idx("A"), f.idx("B"), f.idx("C"));
        let before_c = DropTarget {
            node: c,
            zone: Zone::Before,
            illegal: false,
        };
        let after_c = DropTarget {
            node: c,
            zone: Zone::After,
            illegal: false,
        };
        let single = move_commands(&f.scene, &plan_moves(&f.tree, &[a], before_c));
        // 移动前的数组是 [A,B,C]，C 在下标 2 ⇒ 插到它前面就是 2。
        assert!(matches!(
            single,
            Some(Command::MoveNode { index: Some(2), .. })
        ));
        let batch = move_commands(&f.scene, &plan_moves(&f.tree, &[a, b], after_c));
        assert!(matches!(batch, Some(Command::Batch { commands }) if commands.len() == 2));
        assert!(move_commands(&f.scene, &[]).is_none());
    }
}
