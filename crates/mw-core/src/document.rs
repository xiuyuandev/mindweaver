//! 文档：唯一权威的领域状态（文档 04 §1 铁律 1）。
//!
//! 撤销采用「受影响节点前后快照」模型：每条命令产出 `ChangeSet`，
//! 撤销 = 回放 before、重做 = 回放 after，对树结构（增删移）天然正确。

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::command::{self, Command};
use crate::model::{
    new_id, now_ms, AnnotationKind, AnnotationRef, ArrowHead, Boundary, DocMeta, Node, Position,
    Relationship, Summary,
};
use crate::note::NoteCard;
use crate::search::{MatchMode, RelationshipHit, SearchHit, SearchIndex};

pub const MAX_TEXT_LEN: usize = 2000;
pub const MAX_NOTE_LEN: usize = 10_000;
pub const MAX_LINK_LEN: usize = 2048;
pub const HISTORY_LIMIT: usize = 500;
/// 撤销栈上限的可配区间（`docs/02` §2 P0：默认 500 步，可配 100–2000）。
pub const MIN_HISTORY_LIMIT: usize = 100;
pub const MAX_HISTORY_LIMIT: usize = 2000;
/// 每个节点最多标签数（XMind 亦有限制性设计，避免节点膨胀）
pub const MAX_LABELS: usize = 5;
pub const MAX_LABEL_LEN: usize = 64;
/// 关系线标签 / 概要文字上限
pub const MAX_ANNOTATION_TEXT: usize = 200;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum CoreError {
    #[error("节点不存在：{0}")]
    NotFound(String),
    #[error("根节点不能被删除或移动")]
    RootProtected,
    #[error("文字过长（上限 {0} 字）")]
    TextTooLong(usize),
    #[error("备注过长（上限 {0} 字）")]
    NoteTooLong(usize),
    #[error("链接过长（上限 {0} 字）")]
    LinkTooLong(usize),
    #[error("无效操作：{0}")]
    Invalid(String),
    #[error("文档结构损坏：{0}")]
    Corrupt(String),
    #[error("文档只读：先另存为可写文件再编辑")]
    ReadOnly,
}

/// 跨 IPC 的增量变更：按 id 覆盖/新增 `upserted`，删除 `removed`。
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WireChange {
    pub upserted: Vec<Node>,
    pub removed: Vec<String>,
}

/// 一次命令产生的前后节点快照集合。
#[derive(Clone, Debug, Default)]
pub struct ChangeSet {
    pub label: String,
    pub before: Vec<Node>,
    pub after: Vec<Node>,
    pub before_meta: Option<DocMeta>,
    pub after_meta: Option<DocMeta>,
    /// 卡片快照：只有动到卡片的命令才填，且填的是**整份 cards**。
    pub before_cards: Vec<NoteCard>,
    pub after_cards: Vec<NoteCard>,
    /// 分组快照：只有动到分组的命令才填，填的是**整份 groups**（CM-18）。
    pub before_groups: Vec<crate::model::ConceptGroup>,
    pub after_groups: Vec<crate::model::ConceptGroup>,
    /// 主根快照：只有**换根**的命令才填（概念图删掉主概念时把根重挂到别的概念上）。
    /// 留 `None` 表示这条命令没碰 `root_id`——绝大多数命令都是这样。
    pub before_root: Option<String>,
    pub after_root: Option<String>,
}

impl ChangeSet {
    pub fn new(
        label: String,
        before: Vec<Node>,
        after: Vec<Node>,
        before_meta: Option<DocMeta>,
        after_meta: Option<DocMeta>,
    ) -> Self {
        Self {
            label,
            before,
            after,
            before_meta,
            after_meta,
            before_cards: Vec::new(),
            after_cards: Vec::new(),
            before_groups: Vec::new(),
            after_groups: Vec::new(),
            before_root: None,
            after_root: None,
        }
    }

    /// 给「换根」的命令补 `root_id` 快照——撤销/重做要把根一起还原，
    /// 否则撤销完根还指着那个已经被删掉的概念（`validate` 立刻判脏）。
    pub fn with_root(mut self, before: String, after: String) -> Self {
        self.before_root = Some(before);
        self.after_root = Some(after);
        self
    }

    /// 给「动到卡片」的命令补整份卡片快照（撤销/重做时整体换回）。
    pub fn with_cards(mut self, before: Vec<NoteCard>, after: Vec<NoteCard>) -> Self {
        self.before_cards = before;
        self.after_cards = after;
        self
    }

    /// 给「动到分组」的命令补整份分组快照（CM-18）。
    pub fn with_groups(
        mut self,
        before: Vec<crate::model::ConceptGroup>,
        after: Vec<crate::model::ConceptGroup>,
    ) -> Self {
        self.before_groups = before;
        self.after_groups = after;
        self
    }

    pub fn forward_wire(&self) -> WireChange {
        wire(&self.before, &self.after)
    }

    pub fn backward_wire(&self) -> WireChange {
        wire(&self.after, &self.before)
    }

    pub fn is_empty(&self) -> bool {
        self.before_meta == self.after_meta
            && same_nodes(&self.before, &self.after)
            && self.before_cards == self.after_cards
            && self.before_groups == self.after_groups
            && self.before_root == self.after_root
    }

    /// 这一条**新建**了这枚节点吗（`before` 里没有、`after` 里有）。
    ///
    /// 只给「上一步是不是刚把这个盒子建出来」这类判断用，不参与撤销/重做本身。
    pub fn created(&self, id: &str) -> bool {
        self.after.iter().any(|n| n.id == id) && !self.before.iter().any(|n| n.id == id)
    }
}

fn wire(old: &[Node], new: &[Node]) -> WireChange {
    let removed = old
        .iter()
        .filter(|n| !new.iter().any(|m| m.id == n.id))
        .map(|n| n.id.clone())
        .collect::<Vec<_>>();
    WireChange {
        upserted: new.to_vec(),
        removed,
    }
}

fn same_nodes(a: &[Node], b: &[Node]) -> bool {
    a.len() == b.len() && a.iter().all(|n| b.iter().any(|m| m == n))
}

/// 合并两条变更集（用于 Batch）：before 取事务起点，after 取事务终点。
fn merge(a: ChangeSet, b: ChangeSet) -> ChangeSet {
    let mut created: HashSet<String> = HashSet::new();
    let mut before_map: HashMap<String, Node> = HashMap::new();
    let mut after_map: HashMap<String, Node> = HashMap::new();

    for src in [&a, &b] {
        for n in &src.before {
            if !before_map.contains_key(&n.id) && !created.contains(&n.id) {
                before_map.insert(n.id.clone(), n.clone());
            }
        }
        for n in &src.after {
            if !src.before.iter().any(|x| x.id == n.id) {
                // 该子命令执行前还不存在 → 事务内新建
                created.insert(n.id.clone());
                before_map.remove(&n.id);
            }
            after_map.insert(n.id.clone(), n.clone());
        }
    }

    // `b` 删掉的节点不能留在事务终态：`before`/`after` 只快照**碰到的**节点，
    // 因此「在 `b.before` 里但不在 `b.after` 里」就等于「`b` 把它删了」。
    // 留着的后果是重做把已经被删掉的节点复活成幽灵。
    for n in &b.before {
        if !b.after.iter().any(|m| m.id == n.id) {
            after_map.remove(&n.id);
        }
    }

    let before: Vec<Node> = before_map.into_values().collect();
    let after: Vec<Node> = after_map.into_values().collect();
    let before_meta = a.before_meta.clone().or_else(|| b.before_meta.clone());
    let after_meta = match &b.after_meta {
        Some(m) => Some(m.clone()),
        None => a.after_meta.clone(),
    };
    let label = if a.label.is_empty() { b.label } else { a.label };
    let before_cards = if a.before_cards.is_empty() {
        b.before_cards.clone()
    } else {
        a.before_cards.clone()
    };
    let after_cards = if b.after_cards.is_empty() {
        a.after_cards.clone()
    } else {
        b.after_cards.clone()
    };
    let before_groups = if a.before_groups.is_empty() {
        b.before_groups.clone()
    } else {
        a.before_groups.clone()
    };
    let after_groups = if b.after_groups.is_empty() {
        a.after_groups.clone()
    } else {
        b.after_groups.clone()
    };
    let mut cs = ChangeSet::new(label, before, after, before_meta, after_meta)
        .with_cards(before_cards, after_cards)
        .with_groups(before_groups, after_groups);
    // 根快照与 meta 同一套「取事务起点 / 事务终点」的合并口径：漏掉它，
    // 含换根的 Batch 撤销后 `root_id` 还指着已被撤销掉的概念，`validate` 立刻判脏。
    cs.before_root = a.before_root.clone().or_else(|| b.before_root.clone());
    cs.after_root = b.after_root.clone().or_else(|| a.after_root.clone());
    cs
}

/// 反向链接：哪个节点引用了某张卡片。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlink {
    pub node_id: String,
    pub text: String,
    /// 来源是卡片正文的 `[[引用]]`（而不是节点挂载）。
    pub card: bool,
}

pub struct Document {
    pub id: String,
    pub meta: DocMeta,
    pub root_id: String,
    pub nodes: HashMap<String, Node>,
    /// 卡片笔记。与节点解耦：删节点不删卡片，卡片保留为孤儿。
    pub cards: Vec<NoteCard>,
    /// 概念分组（`docs/32` §13，CM-18）。成员是概念 id；解组只删框不删概念。
    pub groups: Vec<crate::model::ConceptGroup>,
    /// 图片/附件字节（不进 content.json，只进 .zmind 的 assets/）。
    pub assets: crate::assets::Assets,
    pub revision: u64,
    undo_stack: Vec<ChangeSet>,
    redo_stack: Vec<ChangeSet>,
    search: SearchIndex,
    /// 撤销栈上限。不进 `.zmind`、不影响 `FORMAT_VERSION`——这是用户偏好，不是文档内容。
    history_limit: usize,
    /// 只读闸（X29 G-1）。**只在内存里**：壳层按标签的协作锁每帧灌进来，
    /// 所有命令、撤销、重做在这里统一拒绝——画布/大纲/检查器不必各写一遍 if。
    /// 不进 `.zmind`，打开新文件默认 `false`。
    pub read_only: bool,
    /// 样式/结构内容的修订号（**不含文字**）。场景缓存用它判断「内容指纹能不能沿用」：
    /// 只改字时不必再遍历两万节点重算指纹（M12h）。撤销/重做一律 +1，保守但安全。
    /// 不进 `.zmind`，打开新文件从 0 开始。
    pub(crate) content_rev: u64,
}

impl Document {
    pub fn new(title: &str) -> Self {
        let root = Node::new(new_id(), None, "");
        let mut nodes = HashMap::new();
        nodes.insert(root.id.clone(), root.clone());
        Self {
            id: new_id(),
            meta: DocMeta {
                title: title.to_string(),
                ..Default::default()
            },
            root_id: root.id,
            nodes,
            cards: Vec::new(),
            groups: Vec::new(),
            assets: crate::assets::Assets::new(),
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search: SearchIndex::default(),
            history_limit: HISTORY_LIMIT,
            read_only: false,
            content_rev: 0,
        }
    }

    /// 从文件/导入器构建（会校验结构）。
    pub fn from_parts(
        id: Option<String>,
        meta: DocMeta,
        root_id: String,
        nodes: HashMap<String, Node>,
    ) -> Result<Self, CoreError> {
        Self::from_parts_full(id, meta, root_id, nodes, Vec::new(), Vec::new())
    }

    /// 带卡片的构造：卡片必须在 `validate()` 之前就位，否则 `node.cardId`
    /// 会被判成「链接的卡片缺失」。格式层与恢复点都走这里。
    pub fn from_parts_with_cards(
        id: Option<String>,
        meta: DocMeta,
        root_id: String,
        nodes: HashMap<String, Node>,
        cards: Vec<NoteCard>,
    ) -> Result<Self, CoreError> {
        Self::from_parts_full(id, meta, root_id, nodes, cards, Vec::new())
    }

    /// 全量构造（多一个分组参数）。`from_parts_with_cards` 是它的「没有分组」特例。
    pub fn from_parts_full(
        id: Option<String>,
        meta: DocMeta,
        root_id: String,
        nodes: HashMap<String, Node>,
        cards: Vec<NoteCard>,
        groups: Vec<crate::model::ConceptGroup>,
    ) -> Result<Self, CoreError> {
        let doc = Self {
            id: id.unwrap_or_else(new_id),
            meta,
            root_id,
            nodes,
            cards,
            groups,
            assets: crate::assets::Assets::new(),
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search: SearchIndex::default(),
            history_limit: HISTORY_LIMIT,
            read_only: false,
            content_rev: 0,
        };
        doc.validate()?;
        Ok(doc)
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn card(&self, id: &str) -> Option<&NoteCard> {
        self.cards.iter().find(|c| c.id == id)
    }

    /// 布局种类：`meta.layout.kind` 的枚举形态（字符串是磁盘格式，别在业务里散着比）。
    pub fn layout_kind(&self) -> crate::layout::LayoutKind {
        crate::layout::LayoutKind::from_kind(&self.meta.layout.kind)
    }

    /// 是不是概念图文档。概念图的节点是「概念」，措辞与回跳语义都跟树不同。
    pub fn is_concept_map(&self) -> bool {
        self.layout_kind().is_concept_map()
    }

    /// 概念是否被钉在画布上（整理布局时不移动）。
    pub fn is_concept_pinned(&self, id: &str) -> bool {
        self.node(id).and_then(|n| n.pinned).unwrap_or(false)
    }

    /// 概念所连接的命题总数（出度 + 入度，不重复计算自环）。
    pub fn concept_proposition_count(&self, id: &str) -> usize {
        let Some(n) = self.node(id) else {
            return 0;
        };
        let outgoing = n.relationships.len();
        let incoming = self
            .nodes
            .values()
            .filter(|other| other.id != id)
            .map(|other| other.relationships.iter().filter(|r| r.to == id).count())
            .sum::<usize>();
        outgoing + incoming
    }

    /// 文档的可读名字。`meta.title` 只有空白时给「未命名」——
    /// 卡片页的「来源」与导出包都用它，不能把空串摆到界面上。
    pub fn display_title(&self) -> &str {
        let t = self.meta.title.trim();
        if t.is_empty() {
            "未命名"
        } else {
            t
        }
    }

    /// 节点当前链接的卡片（一个节点最多一张，以 `Node.card_id` 为准）。
    pub fn card_for_node(&self, node_id: &str) -> Option<&NoteCard> {
        let id = self.nodes.get(node_id)?.card_id.as_ref()?;
        self.card(id)
    }

    /// 可以挂到 `node_id` 上的卡片：文档里还没被别的节点占用的那些，按更新时间倒序。
    /// 检查器的「关联已有」用它出候选——已经属于别人的卡片不该出现在列表里
    /// （选了也是 `SetNodeCard` 报「卡片已经链接到另一个节点」）。
    pub fn linkable_cards(&self, node_id: &str, limit: usize) -> Vec<&NoteCard> {
        let mut out: Vec<&NoteCard> = self
            .cards
            .iter()
            .filter(|c| {
                !self
                    .nodes
                    .values()
                    .any(|n| n.id != node_id && n.card_id.as_deref() == Some(c.id.as_str()))
            })
            .collect();
        out.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        out.truncate(limit);
        out
    }

    /// 引用这张卡片的节点（反链）。正常最多一个；数据损坏时也不会 panic。
    pub fn backlinks_for_card(&self, card_id: &str) -> Vec<Backlink> {
        let mut out: Vec<Backlink> = self
            .nodes
            .values()
            .filter(|n| n.card_id.as_deref() == Some(card_id))
            .map(|n| Backlink {
                node_id: n.id.clone(),
                text: n.text.clone(),
                card: false,
            })
            .collect();
        if let Some(target) = self.cards.iter().find(|c| c.id == card_id) {
            for card in &self.cards {
                if card.id == card_id {
                    continue;
                }
                if crate::references::parse(&card.body)
                    .iter()
                    .any(|t| crate::references::matches(t, &target.id, &target.title))
                {
                    out.push(Backlink {
                        node_id: card.id.clone(),
                        text: format!("卡片 · {}", card.display_title()),
                        card: true,
                    });
                }
            }
        }
        out.sort_by(|a, b| a.text.cmp(&b.text).then_with(|| a.node_id.cmp(&b.node_id)));
        out
    }

    /// 正文里的引用解析：返回（已解析目标, 断链目标）。断链用于阅读页提示。
    pub fn card_refs(&self, body: &str) -> (Vec<String>, Vec<String>) {
        let mut ok = Vec::new();
        let mut broken = Vec::new();
        for target in crate::references::parse(body) {
            let hit = self
                .cards
                .iter()
                .any(|c| crate::references::matches(&target, &c.id, &c.title));
            if hit {
                ok.push(target);
            } else {
                broken.push(target);
            }
        }
        (ok, broken)
    }

    pub fn cards(&self) -> &[NoteCard] {
        &self.cards
    }

    /// 从格式层装回卡片（会校验）。
    pub fn set_cards(&mut self, cards: Vec<NoteCard>) -> Result<(), CoreError> {
        self.cards = cards;
        self.validate()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 撤销/重做栈还剩几条；`undo` / `redo` 之后读，用来判断「一次操作到底算几步」。
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }

    /// 清空重做栈（M32）。
    ///
    /// 双击折叠圆点时，壳层会先撤掉第一击、再按「第一击之前」的状态下钻；
    /// 如果下钻本来就不改变状态（子级全展开），这条被撤掉的第一击就不该留在
    /// 重做栈里——否则 `Ctrl+Y` 会把用户已经用双击取消掉的那一下又做回来。
    pub fn clear_redo(&mut self) {
        self.redo_stack.clear();
    }

    pub fn sorted_nodes(&self) -> Vec<Node> {
        let mut v: Vec<Node> = self.nodes.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    fn check_text(text: &str) -> Result<(), CoreError> {
        if text.chars().count() > MAX_TEXT_LEN {
            return Err(CoreError::TextTooLong(MAX_TEXT_LEN));
        }
        Ok(())
    }

    /// 落点必须是有限数：`NaN`/`∞` 会污染布局、命中测试与所有导出
    /// （SVG/PDF 会把 `NaN` 原样写出，打开是一张空白图）。
    fn check_pos(x: f32, y: f32) -> Result<(), CoreError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(CoreError::Invalid("坐标必须是有限数".into()));
        }
        Ok(())
    }

    fn snapshot_ids(&self, ids: &[String]) -> Vec<Node> {
        ids.iter()
            .filter_map(|i| self.nodes.get(i).cloned())
            .collect()
    }

    /// 标签规范化校验（去空白后长度与数量）。
    fn check_label(label: &str) -> Result<String, CoreError> {
        let t = label.trim().to_string();
        if t.is_empty() {
            return Err(CoreError::Invalid("标签不能为空".into()));
        }
        if t.chars().count() > MAX_LABEL_LEN {
            return Err(CoreError::Invalid(format!(
                "标签过长（上限 {MAX_LABEL_LEN} 字）"
            )));
        }
        Ok(t)
    }

    fn check_annotation_text(text: &str) -> Result<(), CoreError> {
        if text.chars().count() > MAX_ANNOTATION_TEXT {
            return Err(CoreError::Invalid(format!(
                "文字过长（上限 {MAX_ANNOTATION_TEXT} 字）"
            )));
        }
        Ok(())
    }

    /// 规范化某节点上的边界/概要目标：剔除非直接子节点的目标，空则删除整个注解。
    /// 结构变化（删除/移动/粘贴）后调用，保证「目标 ⊆ 直接子节点」不变量。
    fn normalize_annotations_of(&mut self, id: &str) {
        let Some(children) = self.nodes.get(id).map(|n| n.children.clone()) else {
            return;
        };
        if let Some(n) = self.nodes.get_mut(id) {
            let child_set: HashSet<&str> = children.iter().map(|s| s.as_str()).collect();
            n.boundaries.retain_mut(|b| {
                b.targets.retain(|t| child_set.contains(t.as_str()));
                !b.targets.is_empty()
            });
            n.summaries.retain_mut(|s| {
                s.targets.retain(|t| child_set.contains(t.as_str()));
                !s.targets.is_empty()
            });
        }
    }

    /// 校验边界/概要的目标：必须是 `parent` 的直接子节点，去重、非空。
    fn child_targets(&self, parent: &str, targets: &[String]) -> Result<Vec<String>, CoreError> {
        let p = self
            .nodes
            .get(parent)
            .ok_or_else(|| CoreError::NotFound(parent.to_string()))?;
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for t in targets {
            if !p.children.contains(t) {
                return Err(CoreError::Invalid(
                    "边界/概要只能圈住该节点的直接子节点".into(),
                ));
            }
            if seen.insert(t.clone()) {
                out.push(t.clone());
            }
        }
        if out.is_empty() {
            return Err(CoreError::Invalid("边界/概要的目标不能为空".into()));
        }
        Ok(out)
    }

    /// 盖住这个节点的边界/概要（由它的父节点持有）。
    ///
    /// 只找**直接父级**：边界与概要的 targets 必须是父的直接子节点，再往上没有意义。
    /// 返回值按「概要在前、边界在后」排，和 XMind 里两类注解的层级一致。
    pub fn annotations_covering(&self, id: &str) -> Vec<AnnotationRef> {
        let Some(node) = self.nodes.get(id) else {
            return Vec::new();
        };
        let Some(parent) = node.parent_id.as_deref() else {
            return Vec::new();
        };
        let Some(p) = self.nodes.get(parent) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for summary in &p.summaries {
            if summary.targets.iter().any(|t| t == id) {
                let label = if summary.text.trim().is_empty() {
                    "概要".to_string()
                } else {
                    summary.text.clone()
                };
                out.push(AnnotationRef {
                    parent: parent.to_string(),
                    kind: AnnotationKind::Summary,
                    id: summary.id.clone(),
                    label,
                    owned: false,
                    color: None,
                });
            }
        }
        for boundary in &p.boundaries {
            if boundary.targets.iter().any(|t| t == id) {
                out.push(AnnotationRef {
                    parent: parent.to_string(),
                    kind: AnnotationKind::Boundary,
                    id: boundary.id.clone(),
                    label: "边界".to_string(),
                    owned: false,
                    color: boundary.color.clone(),
                });
            }
        }
        out
    }

    /// 这个节点**自己持有**的边界/概要（它盖住的是自己的子节点）。
    ///
    /// 与 `annotations_covering` 互补：选中父节点时看得到自己加的注解，
    /// 选中子节点时看得到盖住自己的注解。
    pub fn annotations_owned(&self, id: &str) -> Vec<AnnotationRef> {
        let Some(node) = self.nodes.get(id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for summary in &node.summaries {
            let label = if summary.text.trim().is_empty() {
                "概要".to_string()
            } else {
                summary.text.clone()
            };
            out.push(AnnotationRef {
                parent: id.to_string(),
                kind: AnnotationKind::Summary,
                id: summary.id.clone(),
                label,
                owned: true,
                color: None,
            });
        }
        for boundary in &node.boundaries {
            out.push(AnnotationRef {
                parent: id.to_string(),
                kind: AnnotationKind::Boundary,
                id: boundary.id.clone(),
                label: "边界".to_string(),
                owned: true,
                color: boundary.color.clone(),
            });
        }
        out
    }

    /// 自身 + 全部子孙的 id（前序）。
    pub fn subtree_ids(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![id.to_string()];
        while let Some(cur) = stack.pop() {
            if let Some(n) = self.nodes.get(&cur) {
                out.push(cur.clone());
                for c in n.children.iter().rev() {
                    stack.push(c.clone());
                }
            }
        }
        out
    }

    /// 从 `id` 往上找**折叠着的祖先**，按「近 → 远」返回；不含自身。
    ///
    /// 自己折叠不影响自己可见（只挡子孙），所以搜到折叠节点本身时不必展开它。
    /// 父链用 `seen` 挡环：`validate()` 保证没有环，但这里读的是可能被并发改坏的表，
    /// 一个环会让搜索跳转永远转下去。
    pub fn collapsed_ancestors(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut cur = self.nodes.get(id).and_then(|n| n.parent_id.as_deref());
        while let Some(pid) = cur {
            if !seen.insert(pid) {
                break;
            }
            let Some(parent) = self.nodes.get(pid) else {
                break;
            };
            if parent.collapsed {
                out.push(parent.id.clone());
            }
            cur = parent.parent_id.as_deref();
        }
        out
    }

    /// 概念图删掉主概念后，从**没被删掉**的概念里挑一个新根：取最靠左上（y、x、id）的那个。
    ///
    /// 为什么是「左上」而不是「下一个 id」：概念图没有插入顺序可言，位置就是阅读顺序；
    /// 挑左上的概念当根，重挂之后缩略图/大纲的第一行与用户眼里的「第一个」一致。
    fn surviving_concept(&self, removing: &HashSet<String>) -> Option<String> {
        self.nodes
            .values()
            .filter(|n| n.parent_id.is_none() && !removing.contains(&n.id))
            .min_by(|a, b| {
                let key = |n: &Node| {
                    let (y, x) = n.pos.as_ref().map(|p| (p.y, p.x)).unwrap_or((0.0, 0.0));
                    (y, x, n.id.clone())
                };
                key(a)
                    .partial_cmp(&key(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| n.id.clone())
    }

    /// 文档前序顺序：主根子树，随后各自由主题根（按 id 稳定排序）的子树。
    pub fn doc_order(&self) -> Vec<String> {
        let mut out = self.subtree_ids(&self.root_id);
        let mut floats: Vec<String> = self
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none() && n.id != self.root_id)
            .map(|n| n.id.clone())
            .collect();
        floats.sort();
        for id in floats {
            out.extend(self.subtree_ids(&id));
        }
        out
    }

    /// 结构不变量校验：根唯一、父子双向一致、无环、可达。
    pub fn validate(&self) -> Result<(), CoreError> {
        let root = self
            .nodes
            .get(&self.root_id)
            .ok_or_else(|| CoreError::Corrupt("根节点缺失".into()))?;
        if root.parent_id.is_some() {
            return Err(CoreError::Corrupt("根节点不应有父节点".into()));
        }
        for (key, n) in self.nodes.iter() {
            if key != &n.id {
                return Err(CoreError::Corrupt(format!("节点表键值不一致：{key}")));
            }
            let mut seen = HashSet::new();
            for c in &n.children {
                if !seen.insert(c.clone()) {
                    return Err(CoreError::Corrupt(format!("重复子节点：{c}")));
                }
                let child = self
                    .nodes
                    .get(c)
                    .ok_or_else(|| CoreError::Corrupt(format!("子节点缺失：{c}")))?;
                if child.parent_id.as_deref() != Some(key.as_str()) {
                    return Err(CoreError::Corrupt(format!("父子关系不一致：{c}")));
                }
            }
            if let Some(p) = &n.parent_id {
                if n.id == self.root_id {
                    return Err(CoreError::Corrupt("根节点不应有父节点".into()));
                }
                let parent = self
                    .nodes
                    .get(p)
                    .ok_or_else(|| CoreError::Corrupt(format!("父节点缺失：{p}")))?;
                if !parent.children.contains(&n.id) {
                    return Err(CoreError::Corrupt(format!("父节点未登记子节点：{}", n.id)));
                }
            }
            // 高级元素不变量
            if n.labels.len() > MAX_LABELS {
                return Err(CoreError::Corrupt(format!("标签数量超限：{}", n.id)));
            }
            for l in &n.labels {
                if l.trim().is_empty() || l.chars().count() > MAX_LABEL_LEN {
                    return Err(CoreError::Corrupt(format!("非法标签：{}", n.id)));
                }
            }
            for r in &n.relationships {
                if r.to == n.id {
                    return Err(CoreError::Corrupt(format!("关系线指向自身：{}", n.id)));
                }
                if !self.nodes.contains_key(&r.to) {
                    return Err(CoreError::Corrupt(format!("关系线目标缺失：{}", r.to)));
                }
                // 线宽范围与命令层同口径（`SetRelationshipWidth` 挡 `is_finite` + 0.5–6.0）。
                // 读盘路径以前不查：手工改坏的文件能把 `NaN` / 9.0 / 1e9 直接送进 Scene 几何。
                if let Some(w) = r.width {
                    if !w.is_finite() || !(0.5..=6.0).contains(&w) {
                        return Err(CoreError::Corrupt(format!("关系线宽越界：{}", n.id)));
                    }
                }
            }
            let child_set: HashSet<&str> = n.children.iter().map(|c| c.as_str()).collect();
            for b in &n.boundaries {
                if b.targets.is_empty() {
                    return Err(CoreError::Corrupt(format!("空边界：{}", n.id)));
                }
                for t in &b.targets {
                    if !child_set.contains(t.as_str()) {
                        return Err(CoreError::Corrupt(format!("边界目标不是直接子节点：{t}")));
                    }
                }
            }
            for s in &n.summaries {
                if s.targets.is_empty() {
                    return Err(CoreError::Corrupt(format!("空概要：{}", n.id)));
                }
                if s.text.chars().count() > MAX_ANNOTATION_TEXT {
                    return Err(CoreError::Corrupt(format!("概要文字过长：{}", n.id)));
                }
                for t in &s.targets {
                    if !child_set.contains(t.as_str()) {
                        return Err(CoreError::Corrupt(format!("概要目标不是直接子节点：{t}")));
                    }
                }
            }
            if let Some(p) = &n.pos {
                // 坐标必须有限：命令层（`check_pos`）已经挡掉 inf/NaN，读盘路径以前不查，
                // 一颗 `inf` 坐标会把整张图的包围盒与命中网格一起毁掉（BUG-9）。
                if !p.x.is_finite() || !p.y.is_finite() {
                    return Err(CoreError::Corrupt(format!("自由坐标不是有限数：{}", n.id)));
                }
                if n.parent_id.is_some() || n.id == self.root_id {
                    // 概念图例外：主概念也是「第一个概念」，它必须能带 `pos`（CM-04），
                    // 否则第一个概念拖不动、整理布局也只能绕开它。
                    let concept_root = n.id == self.root_id
                        && n.parent_id.is_none()
                        && self.meta.layout.kind == "conceptMap";
                    if !concept_root {
                        return Err(CoreError::Corrupt(format!(
                            "非自由主题不应带自由坐标：{}",
                            n.id
                        )));
                    }
                }
            }
        }
        // 分组不变量（CM-18）：id 唯一、成员 ≥2 且都存在、一个概念只属于一个组。
        let mut group_ids: HashSet<&str> = HashSet::new();
        let mut group_members: HashSet<&str> = HashSet::new();
        for g in &self.groups {
            if !group_ids.insert(g.id.as_str()) {
                return Err(CoreError::Corrupt(format!("分组 id 重复：{}", g.id)));
            }
            if g.members.len() < 2 {
                return Err(CoreError::Corrupt(format!("分组少于两个成员：{}", g.id)));
            }
            for m in &g.members {
                if !self.nodes.contains_key(m) {
                    return Err(CoreError::Corrupt(format!("分组引用了不存在的概念：{m}")));
                }
                if !group_members.insert(m.as_str()) {
                    return Err(CoreError::Corrupt(format!("概念同时属于多个分组：{m}")));
                }
            }
        }
        // 森林可达性（同时排除环）：主根 + 全部自由主题根（parent_id 为 None）
        let mut stack: Vec<String> = self
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none())
            .map(|n| n.id.clone())
            .collect();
        let mut seen: HashSet<String> = HashSet::new();
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur.clone()) {
                return Err(CoreError::Corrupt(format!(
                    "节点被重复引用（疑似成环）：{cur}"
                )));
            }
            if let Some(n) = self.nodes.get(&cur) {
                for c in &n.children {
                    stack.push(c.clone());
                }
            }
        }
        if seen.len() != self.nodes.len() {
            return Err(CoreError::Corrupt("存在不可达节点（疑似成环）".into()));
        }
        let mut card_ids = HashSet::new();
        let mut linked_nodes = HashSet::new();
        for c in &self.cards {
            if !card_ids.insert(c.id.clone()) {
                return Err(CoreError::Corrupt(format!("重复卡片 id：{}", c.id)));
            }
            c.validate().map_err(CoreError::Corrupt)?;
            if let Some(node) = &c.source_node {
                if !linked_nodes.insert(node.clone()) {
                    return Err(CoreError::Corrupt(format!(
                        "一个节点链接了多张卡片：{node}"
                    )));
                }
            }
        }
        let mut node_cards = HashSet::new();
        for n in self.nodes.values() {
            if let Some(cid) = &n.card_id {
                if !self.cards.iter().any(|c| &c.id == cid) {
                    return Err(CoreError::Corrupt(format!(
                        "节点链接的卡片缺失：{} -> {cid}",
                        n.id
                    )));
                }
                if !node_cards.insert(cid.clone()) {
                    return Err(CoreError::Corrupt(format!(
                        "同一张卡片被多个节点链接：{cid}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// 这条命令会不会改变「样式/结构内容」（`content_rev` 的判据）。
    ///
    /// 只放行两类**确定不影响样式表与盒子输入**的命令：`SetText` 与 `SetCollapsed`。
    /// 其余一律保守 +1——漏判的后果是「样式表复用旧值」，比多算一次指纹严重得多。
    /// `Batch` 递归：只要子命令有一条要算，就得算。
    fn changes_style_content(cmd: &Command) -> bool {
        match cmd {
            Command::SetText { .. } | Command::SetCollapsed { .. } => false,
            Command::Batch { commands } => commands.iter().any(Self::changes_style_content),
            _ => true,
        }
    }

    /// 执行命令；产生非空变更时入历史栈。
    pub fn apply(&mut self, cmd: &Command) -> Result<ChangeSet, CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let cs = self.apply_inner(cmd)?;
        if !cs.is_empty() {
            self.revision += 1;
            if Self::changes_style_content(cmd) {
                self.content_rev += 1;
            }
            self.undo_stack.push(cs.clone());
            self.trim_undo_history();
            self.redo_stack.clear();
        }
        Ok(cs)
    }

    fn apply_inner(&mut self, cmd: &Command) -> Result<ChangeSet, CoreError> {
        match cmd {
            Command::AddNode {
                parent,
                index,
                text,
                style,
            } => {
                // 概念图里没有「子节点」这回事：概念一律 `parent_id = None` + `pos`，
                // 概念之间的关系走 `relationships`（`docs/30` §5）。
                //
                // 这条命令的载荷**只有 `parent`、没有 `pos`**，所以「强行改写成独立概念」
                // 就得替用户编一个坐标（叠在父概念身上、或塞到原点），那是替用户做了
                // 一个他没法撤销的排版决定；而且同一个命令在两种文档里语义还不一样。
                // 概念图本来就有两条对的入口：`AddFloatingTopic`（自由摆放）与
                // `AddLinkedConcept`（建概念的同时连线），壳层要新建概念走那两条。
                if self.is_concept_map() {
                    return Err(CoreError::Invalid("概念图里概念不能挂成树节点".into()));
                }
                if !self.nodes.contains_key(parent) {
                    return Err(CoreError::NotFound(parent.clone()));
                }
                Self::check_text(text)?;
                let id = new_id();
                let mut node = Node::new(id.clone(), Some(parent.clone()), text.clone());
                node.style = style.clone();
                let touched = vec![parent.clone(), id.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let p = self.nodes.get_mut(parent).unwrap();
                    let idx = index.unwrap_or(p.children.len()).min(p.children.len());
                    p.children.insert(idx, id.clone());
                }
                self.nodes.insert(id, node);
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::InsertSubtree {
                parent,
                index,
                nodes,
            } => {
                // 与 `AddNode` 同一条口径：子树就是 `parent_id = Some(..)`，
                // 概念图里粘贴概念必须走 `InsertConcepts`（独立概念 + `pos`）。
                if self.is_concept_map() {
                    return Err(CoreError::Invalid(
                        "概念图里不能粘贴子树：概念请粘贴为独立概念".into(),
                    ));
                }
                if !self.nodes.contains_key(parent) {
                    return Err(CoreError::NotFound(parent.clone()));
                }
                if nodes.is_empty() {
                    return Err(CoreError::Invalid("空子树".into()));
                }
                let mut ids: HashSet<String> = HashSet::new();
                for n in nodes {
                    if !ids.insert(n.id.clone()) {
                        return Err(CoreError::Invalid(format!("重复节点 ID：{}", n.id)));
                    }
                    if self.nodes.contains_key(&n.id) {
                        return Err(CoreError::Invalid(format!("节点 ID 冲突：{}", n.id)));
                    }
                    Self::check_text(&n.text)?;
                }
                // 剪枝：粘贴的节点不带自由坐标；关系线只保留指向「文档内或本子树内」目标；
                // 边界/概要只保留仍在本节点子列表中的目标；标签去重限量。
                let external_ids: HashSet<String> = self.nodes.keys().cloned().collect();
                let mut prepared: Vec<Node> = Vec::with_capacity(nodes.len());
                for n in nodes {
                    let mut c = n.clone();
                    c.pos = None;
                    c.relationships.retain(|r| {
                        r.to != c.id && (ids.contains(&r.to) || external_ids.contains(&r.to))
                    });
                    let child_set: HashSet<String> = c.children.iter().cloned().collect();
                    c.boundaries.retain_mut(|b| {
                        b.targets.retain(|t| child_set.contains(t));
                        !b.targets.is_empty()
                    });
                    c.summaries.retain_mut(|s| {
                        s.targets.retain(|t| child_set.contains(t));
                        !s.targets.is_empty()
                    });
                    let mut seen_labels = HashSet::new();
                    c.labels.retain(|l| {
                        !l.trim().is_empty()
                            && l.chars().count() <= MAX_LABEL_LEN
                            && seen_labels.insert(l.clone())
                    });
                    c.labels.truncate(MAX_LABELS);
                    prepared.push(c);
                }
                let tops: Vec<&Node> = prepared
                    .iter()
                    .filter(|n| n.parent_id.as_deref() == Some(parent.as_str()))
                    .collect();
                if tops.is_empty() {
                    return Err(CoreError::Invalid("子树缺少挂载点（顶层节点）".into()));
                }
                for n in &prepared {
                    if n.parent_id.as_deref() == Some(parent.as_str()) {
                        continue;
                    }
                    match &n.parent_id {
                        Some(pid) if ids.contains(pid) => {}
                        _ => return Err(CoreError::Invalid("子树内部父子关系不完整".into())),
                    }
                }
                let top_ids: Vec<String> = tops.iter().map(|n| n.id.clone()).collect();
                let touched: Vec<String> = std::iter::once(parent.clone())
                    .chain(prepared.iter().map(|n| n.id.clone()))
                    .collect();
                let before = self.snapshot_ids(&touched);
                {
                    let p = self.nodes.get_mut(parent).unwrap();
                    let base = index.unwrap_or(p.children.len()).min(p.children.len());
                    for (k, tid) in top_ids.iter().enumerate() {
                        p.children.insert(base + k, tid.clone());
                    }
                }
                for n in &prepared {
                    self.nodes.insert(n.id.clone(), n.clone());
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::DeleteNodes { ids } => {
                // 概念图例外（CM-05）：`root_id` 指向的是「第一个概念」，不是树根，
                // 它必须能被删——删完把根重挂到剩下的概念上。思维导图的根依旧受保护。
                let concept_map = self.meta.layout.kind == "conceptMap";
                let old_root = self.root_id.clone();
                let mut to_delete: HashSet<String> = HashSet::new();
                for id in ids {
                    if !self.nodes.contains_key(id) {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                    if id == &self.root_id && !concept_map {
                        return Err(CoreError::RootProtected);
                    }
                    for x in self.subtree_ids(id) {
                        to_delete.insert(x);
                    }
                }
                // 一个概念都不剩的「概念图」不是文档，是空壳——按无效操作拒掉，
                // 而不是落一份 `validate` 立刻判脏的中间态。
                let new_root = if concept_map && to_delete.contains(&self.root_id) {
                    Some(
                        self.surviving_concept(&to_delete)
                            .ok_or(CoreError::RootProtected)?,
                    )
                } else {
                    None
                };
                let mut parents: Vec<String> = Vec::new();
                for id in to_delete.iter() {
                    if let Some(n) = self.nodes.get(id) {
                        if let Some(p) = &n.parent_id {
                            if !to_delete.contains(p) && !parents.contains(p) {
                                parents.push(p.clone());
                            }
                        }
                    }
                }
                // 额外受影响：指向被删节点的关系线、圈住被删节点的边界/概要
                let mut affected: Vec<String> = parents.clone();
                for (id, n) in self.nodes.iter() {
                    if to_delete.contains(id) || affected.contains(id) {
                        continue;
                    }
                    let rel_hit = n.relationships.iter().any(|r| to_delete.contains(&r.to));
                    let ann_hit = n
                        .boundaries
                        .iter()
                        .any(|b| b.targets.iter().any(|t| to_delete.contains(t)))
                        || n.summaries
                            .iter()
                            .any(|s| s.targets.iter().any(|t| to_delete.contains(t)));
                    if rel_hit || ann_hit {
                        affected.push(id.clone());
                    }
                }
                let mut touched: Vec<String> = affected.clone();
                touched.extend(to_delete.iter().cloned());
                let before = self.snapshot_ids(&touched);
                let before_groups = self.groups.clone();
                for p in &parents {
                    if let Some(pn) = self.nodes.get_mut(p) {
                        pn.children.retain(|c| !to_delete.contains(c));
                    }
                }
                for id in &affected {
                    if let Some(n) = self.nodes.get_mut(id) {
                        let len_before = n.relationships.len();
                        n.relationships.retain(|r| !to_delete.contains(&r.to));
                        if n.relationships.len() != len_before {
                            n.modified_at = now_ms();
                        }
                    }
                    self.normalize_annotations_of(id);
                }
                for id in to_delete.iter() {
                    self.nodes.remove(id);
                }
                // 分组：删掉的概念从成员里摘掉；剩不到两个成员的组整组消失——
                // 一个只圈住一颗概念的框没有意义，留着只会让用户以为还有分组。
                for g in self.groups.iter_mut() {
                    g.members.retain(|m| !to_delete.contains(m));
                }
                self.groups.retain(|g| g.members.len() >= 2);
                if let Some(nr) = &new_root {
                    self.root_id = nr.clone();
                }
                let after = self.snapshot_ids(&touched);
                let after_groups = self.groups.clone();
                let cs = ChangeSet::new(command::label(cmd).to_string(), before, after, None, None)
                    .with_groups(before_groups, after_groups);
                Ok(match new_root {
                    Some(nr) => cs.with_root(old_root, nr),
                    None => cs,
                })
            }

            Command::SetText { id, text } => {
                Self::check_text(text)?;
                let before = vec![self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?
                    .clone()];
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    if &n.text != text {
                        n.text = text.clone();
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(std::slice::from_ref(id));
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetNote { id, note } => {
                if let Some(t) = note {
                    if t.chars().count() > MAX_NOTE_LEN {
                        return Err(CoreError::NoteTooLong(MAX_NOTE_LEN));
                    }
                }
                let before = vec![self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?
                    .clone()];
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    let normalized = note.as_ref().filter(|s| !s.is_empty()).cloned();
                    if n.note != normalized {
                        n.note = normalized;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(std::slice::from_ref(id));
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetTask { id, task } => {
                let before = vec![self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?
                    .clone()];
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    if n.task != *task {
                        n.task = *task;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(std::slice::from_ref(id));
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetImage { id, image } => {
                if let Some(v) = image {
                    if v.chars().count() > 2048 {
                        return Err(CoreError::Invalid("图片地址过长".into()));
                    }
                }
                let before = vec![self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?
                    .clone()];
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    let next = image.clone().filter(|s| !s.trim().is_empty());
                    if n.image != next {
                        n.image = next;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(std::slice::from_ref(id));
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetLink { id, link } => {
                if let Some(t) = link {
                    if t.chars().count() > MAX_LINK_LEN {
                        return Err(CoreError::LinkTooLong(MAX_LINK_LEN));
                    }
                    if !valid_link(t) {
                        return Err(CoreError::Invalid(
                            "链接格式不合法（支持 http/https/file/mailto 或本地路径）".into(),
                        ));
                    }
                }
                let before = vec![self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?
                    .clone()];
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    let normalized = link.as_ref().filter(|s| !s.is_empty()).cloned();
                    if n.link != normalized {
                        n.link = normalized;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(std::slice::from_ref(id));
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetIcon { ids, icon } => {
                if let Some(k) = icon {
                    if k.chars().count() > 32 {
                        return Err(CoreError::Invalid("图标标记标识过长".into()));
                    }
                }
                for id in ids {
                    if !self.nodes.contains_key(id) {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                }
                let before = self.snapshot_ids(ids);
                for id in ids {
                    let n = self.nodes.get_mut(id).unwrap();
                    if n.icon != *icon {
                        n.icon = icon.clone();
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(ids);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetStyle { ids, style } => {
                for id in ids {
                    if !self.nodes.contains_key(id) {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                }
                let before = self.snapshot_ids(ids);
                for id in ids {
                    let n = self.nodes.get_mut(id).unwrap();
                    if n.style != *style {
                        n.style = style.clone();
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(ids);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetCollapsed { id, collapsed } => {
                let before = vec![self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?
                    .clone()];
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    if n.collapsed != *collapsed {
                        n.collapsed = *collapsed;
                    }
                }
                let after = self.snapshot_ids(std::slice::from_ref(id));
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetCollapsedAll { collapsed } => {
                let target: Vec<String> = self
                    .nodes
                    .values()
                    .filter(|n| n.id != self.root_id && !n.children.is_empty())
                    .map(|n| n.id.clone())
                    .collect();
                let before = self.snapshot_ids(&target);
                for id in &target {
                    if let Some(n) = self.nodes.get_mut(id) {
                        n.collapsed = *collapsed;
                    }
                }
                let after = self.snapshot_ids(&target);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::MoveNode {
                id,
                new_parent,
                index,
            } => {
                // 概念图里完全没有「挂成子节点」这条路。这里把文档类型判在根保护/存在性之前，
                // 是为了让错误口径与 id 无关：无论拖哪一颗概念，答案都是「不能挂成树节点」，
                // 而不是一半 `RootProtected`、一半 `Invalid`。
                //
                // 这条护栏是**数据层**的底线：壳层的大纲拖动（`OA::Move` / `OA::MoveMany`）
                // 是无条件下发 `MoveNode` 的，一旦漏进来就会把概念改成 `parent_id = Some(..)`
                // 并把 `pos` 抹成 `None`——位置当场丢失，`validate()` 之后只允许「根 + 自由概念」
                // 有 `pos`，于是整份文档静默降级成树。
                if self.is_concept_map() {
                    return Err(CoreError::Invalid("概念图里概念不能挂成树节点".into()));
                }
                if id == &self.root_id {
                    return Err(CoreError::RootProtected);
                }
                if !self.nodes.contains_key(id) {
                    return Err(CoreError::NotFound(id.clone()));
                }
                if !self.nodes.contains_key(new_parent) {
                    return Err(CoreError::NotFound(new_parent.clone()));
                }
                if id == new_parent {
                    return Err(CoreError::Invalid("不能移动到自身之下".into()));
                }
                if self.subtree_ids(id).iter().any(|x| x == new_parent) {
                    return Err(CoreError::Invalid("不能移动到自己的子树中".into()));
                }
                // 自由主题（parent_id 为 None）也可被挂回树内
                let old_parent = self.nodes[id].parent_id.clone();
                let mut touched = vec![new_parent.clone(), id.clone()];
                if let Some(op) = &old_parent {
                    if op != new_parent {
                        touched.push(op.clone());
                    }
                }
                let before = self.snapshot_ids(&touched);

                let old_pos = old_parent
                    .as_ref()
                    .and_then(|op| self.nodes[op].children.iter().position(|c| c == id));
                if let Some(pos) = old_pos {
                    self.nodes
                        .get_mut(old_parent.as_ref().unwrap())
                        .unwrap()
                        .children
                        .remove(pos);
                }
                // 索引语义：`index` 以「移动发生前的 children 数组」为参照
                // （0..=L，L 为原长度；等于 L 表示追加到末尾）。
                let idx = if old_parent.as_deref() == Some(new_parent.as_str()) {
                    let pre_len = self.nodes[new_parent].children.len() + 1;
                    let want = index.unwrap_or(pre_len).min(pre_len);
                    match old_pos {
                        Some(pos) if want > pos => want - 1,
                        _ => want,
                    }
                } else {
                    let target_len = self.nodes[new_parent].children.len();
                    index.unwrap_or(target_len).min(target_len)
                };
                self.nodes
                    .get_mut(new_parent)
                    .unwrap()
                    .children
                    .insert(idx, id.clone());
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    n.parent_id = Some(new_parent.clone());
                    n.pos = None; // 挂回树内后不再是自由主题
                    n.modified_at = now_ms();
                }
                if let Some(op) = &old_parent {
                    self.normalize_annotations_of(op);
                }
                self.normalize_annotations_of(new_parent);
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::AddRelationship { from, to, label } => {
                if from == to {
                    return Err(CoreError::Invalid("不能把节点连接到自身".into()));
                }
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                if !self.nodes.contains_key(to) {
                    return Err(CoreError::NotFound(to.clone()));
                }
                if let Some(l) = label {
                    Self::check_annotation_text(l)?;
                }
                // 概念图里**同一对概念之间允许多条边**：两条边是两条不同的命题
                // （「A 影响 B」+「A 包含 B」），这正是 CmapTools 的口径；边挂在源节点的
                // `relationships` 里、靠 `id` 区分，数据模型本来就是 M:N，不需要额外字段。
                //
                // 思维导图仍然只留一条：`scene.rs` 的 `bow_crossings` 只给「真交叉且夹角 ≥12°」
                // 的线段换弯法，同起终点的两条边既不交叉也不算交叉（端点相接），画出来**完全重合**，
                // 用户会得到一条看不见的重复线——删掉看得见那条、线还在。那是回退，不能放。
                //
                // 注意：布局期（`concept_layout.rs` 的 `kept_edges`）仍会为摆位去重，
                // 那是「同一对概念只算一条边」的几何取舍，**不是数据层的去重**。
                if !self.is_concept_map()
                    && self.nodes[from].relationships.iter().any(|r| &r.to == to)
                {
                    return Err(CoreError::Invalid("两节点之间已存在关系线".into()));
                }
                let normalized = label
                    .clone()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    n.relationships.push(Relationship {
                        id: new_id(),
                        to: to.clone(),
                        label: normalized,
                        color: None,
                        head: ArrowHead::Forward,
                        ..Default::default()
                    });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetRelationshipLabel { from, id, label } => {
                if let Some(l) = label {
                    Self::check_annotation_text(l)?;
                }
                // 概念图里「没有短语的线」不构成命题——它是「线团」的来源（`docs/30` §5）。
                // 口径：**建的时候可以没有短语**（`AddRelationship` / `AddLinkedConcept` 落
                // `label: None`，画布立刻立起短语框让用户当场打字），**改的时候不许把它删掉**。
                // 这样「拖出新线 → 立刻写短语」这条流程一点不受影响，堵住的只有「把已有短语清空」。
                if self.is_concept_map()
                    && label
                        .as_deref()
                        .map(|s| s.trim().is_empty())
                        .unwrap_or(true)
                {
                    return Err(CoreError::Invalid("概念图的连接短语必填".into()));
                }
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let rel = n
                        .relationships
                        .iter_mut()
                        .find(|r| &r.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    let normalized = label
                        .clone()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    if rel.label != normalized {
                        rel.label = normalized;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetRelationshipColor { from, id, color } => {
                if let Some(c) = color {
                    if crate::theme::Rgba::parse(c).is_none() {
                        return Err(CoreError::Invalid(format!("颜色格式无效：{c}")));
                    }
                }
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let rel = n
                        .relationships
                        .iter_mut()
                        .find(|r| &r.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    let color = color.clone();
                    if rel.color != color {
                        rel.color = color;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetRelationshipHead { from, id, head } => {
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let rel = n
                        .relationships
                        .iter_mut()
                        .find(|r| &r.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    if rel.head != *head {
                        rel.head = *head;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }
            Command::SetRelationshipDashed { from, id, dashed } => {
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let rel = n
                        .relationships
                        .iter_mut()
                        .find(|r| &r.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    if rel.dashed != *dashed {
                        rel.dashed = *dashed;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetRelationshipWidth { from, id, width } => {
                if let Some(w) = width {
                    if !w.is_finite() || !(0.5..=6.0).contains(w) {
                        return Err(CoreError::Invalid("线宽要在 0.5–6 之间".into()));
                    }
                }
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let rel = n
                        .relationships
                        .iter_mut()
                        .find(|r| &r.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    if rel.width != *width {
                        rel.width = *width;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetRelationshipNote { from, id, note } => {
                if let Some(t) = note {
                    if t.chars().count() > MAX_NOTE_LEN {
                        return Err(CoreError::NoteTooLong(MAX_NOTE_LEN));
                    }
                }
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let rel = n
                        .relationships
                        .iter_mut()
                        .find(|r| &r.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    // 与 `SetNote` 同一口径：空串归一成 `None`，别把「空注释」写进文件。
                    let normalized = note
                        .as_ref()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    if rel.note != normalized {
                        rel.note = normalized;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::CreateGroup { members, label } => {
                if !self.is_concept_map() {
                    return Err(CoreError::Invalid("分组只在概念图里有意义".into()));
                }
                let mut uniq: Vec<String> = Vec::new();
                for m in members {
                    if !self.nodes.contains_key(m) {
                        return Err(CoreError::NotFound(m.clone()));
                    }
                    if !uniq.contains(m) {
                        uniq.push(m.clone());
                    }
                }
                if uniq.len() < 2 {
                    return Err(CoreError::Invalid("分组至少要包含两个概念".into()));
                }
                // 组要能整体平移：`MoveGroup` 只会挪「自由摆放且有坐标」的概念。
                // 以前这里不校验，组里混进一颗没坐标的概念（例如还没拖过的首个概念、
                // 或手工改坏的 .zmind）时，拖动整组只动一部分，没有任何提示（BUG-8）。
                for m in &uniq {
                    let n = &self.nodes[m];
                    if n.parent_id.is_some() || n.pos.is_none() {
                        return Err(CoreError::Invalid(format!(
                            "分组只能包含已摆放的独立概念：{m}"
                        )));
                    }
                }
                let before_groups = self.groups.clone();
                // 一个概念只属于一个组：先把它从旧组里摘掉，旧组空了就整个消失。
                for g in self.groups.iter_mut() {
                    g.members.retain(|m| !uniq.contains(m));
                }
                // 摘完只剩一个成员的组直接消失：`validate` 要求成员 ≥ 2，
                // 留一个一成员的组会让文档当场判脏。
                self.groups.retain(|g| g.members.len() >= 2);
                let label = label.trim();
                self.groups.push(crate::model::ConceptGroup {
                    id: new_id(),
                    label: if label.is_empty() {
                        "分组".to_string()
                    } else {
                        label.to_string()
                    },
                    members: uniq,
                    color: None,
                });
                let after_groups = self.groups.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_groups(before_groups, after_groups))
            }

            Command::DeleteGroup { id } => {
                let Some(pos) = self.groups.iter().position(|g| &g.id == id) else {
                    return Err(CoreError::NotFound(id.clone()));
                };
                let before_groups = self.groups.clone();
                self.groups.remove(pos);
                let after_groups = self.groups.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_groups(before_groups, after_groups))
            }

            Command::SetGroupLabel { id, label } => {
                let text = label.trim();
                let text = if text.is_empty() { "分组" } else { text };
                let before_groups = self.groups.clone();
                let g = self
                    .groups
                    .iter_mut()
                    .find(|g| &g.id == id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                g.label = text.to_string();
                let after_groups = self.groups.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_groups(before_groups, after_groups))
            }

            Command::SetGroupColor { id, color } => {
                if let Some(c) = color {
                    if crate::theme::Rgba::parse(c).is_none() {
                        return Err(CoreError::Invalid(format!("颜色格式无效：{c}")));
                    }
                }
                let before_groups = self.groups.clone();
                let g = self
                    .groups
                    .iter_mut()
                    .find(|g| &g.id == id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                g.color = color.clone();
                let after_groups = self.groups.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_groups(before_groups, after_groups))
            }

            Command::MoveGroup { id, dx, dy } => {
                if !dx.is_finite() || !dy.is_finite() {
                    return Err(CoreError::Invalid("位移必须是有限数".into()));
                }
                let Some(group) = self.groups.iter().find(|g| &g.id == id).cloned() else {
                    return Err(CoreError::NotFound(id.clone()));
                };
                let touched: Vec<String> = group
                    .members
                    .iter()
                    .filter(|m| {
                        self.nodes
                            .get(*m)
                            .is_some_and(|n| n.parent_id.is_none() && n.pos.is_some())
                    })
                    .cloned()
                    .collect();
                if touched.is_empty() {
                    return Ok(ChangeSet::new(
                        command::label(cmd).to_string(),
                        Vec::new(),
                        Vec::new(),
                        None,
                        None,
                    ));
                }
                let before = self.snapshot_ids(&touched);
                for m in &touched {
                    let n = self.nodes.get_mut(m).unwrap();
                    if let Some(p) = n.pos.as_mut() {
                        p.x += dx;
                        p.y += dy;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetRelationshipTarget { from, id, to } => {
                if from == to {
                    return Err(CoreError::Invalid("不能把节点连接到自身".into()));
                }
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                if !self.nodes.contains_key(to) {
                    return Err(CoreError::NotFound(to.clone()));
                }
                // 与 `AddRelationship` 同一条口径：概念图里改端点也要允许落到
                // 「已经有另一条边连着的概念」上——否则「A 影响 B」存在之后，
                // 第二条边就再也改不到 B，等于把平行边堵回去。
                if !self.is_concept_map()
                    && self.nodes[from]
                        .relationships
                        .iter()
                        .any(|r| &r.id != id && &r.to == to)
                {
                    return Err(CoreError::Invalid("两节点之间已存在关系线".into()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let rel = n
                        .relationships
                        .iter_mut()
                        .find(|r| &r.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    if &rel.to != to {
                        rel.to = to.clone();
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::ReverseRelationship { from, id } => {
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let Some(rel) = self.nodes[from]
                    .relationships
                    .iter()
                    .find(|r| &r.id == id)
                    .cloned()
                else {
                    return Err(CoreError::NotFound(id.clone()));
                };
                let to = rel.to.clone();
                if &to == from {
                    return Err(CoreError::Invalid("不能把节点连接到自身".into()));
                }
                if !self.nodes.contains_key(&to) {
                    return Err(CoreError::NotFound(to.clone()));
                }
                // 树图仍守「一对节点一条边」：反向后就是 `to → from`，已有同向边就拒绝。
                // 概念图允许多条平行命题，不查这一条（与 `AddRelationship` 同一口径）。
                if !self.is_concept_map()
                    && self.nodes[&to].relationships.iter().any(|r| &r.to == from)
                {
                    return Err(CoreError::Invalid("两节点之间已存在关系线".into()));
                }
                let touched = vec![from.clone(), to.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    n.relationships.retain(|r| &r.id != id);
                    n.modified_at = now_ms();
                }
                {
                    let n = self.nodes.get_mut(&to).unwrap();
                    n.relationships.push(Relationship {
                        id: rel.id,
                        to: from.clone(),
                        label: rel.label,
                        color: rel.color,
                        // 箭头档位**不镜像**：`arrow` 恒指 `to` 端，两端对调之后箭头自然
                        // 落到另一颗概念上——这正是「反向」该看到的画面。若在这里把
                        // `Forward` 翻成 `Backward`，箭头会留在原地，用户点完看不出变化。
                        head: rel.head,
                        // 线型与注释跟着命题走：反的是方向，不是「这是不是一条暂定命题」。
                        dashed: rel.dashed,
                        width: rel.width,
                        note: rel.note,
                    });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::DeleteRelationship { from, id } => {
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                let touched = vec![from.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(from).unwrap();
                    let len_before = n.relationships.len();
                    n.relationships.retain(|r| &r.id != id);
                    if n.relationships.len() == len_before {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::AddBoundary {
                parent,
                targets,
                color,
            } => {
                let ts = self.child_targets(parent, targets)?;
                if let Some(c) = color {
                    if c.chars().count() > 32 {
                        return Err(CoreError::Invalid("颜色值过长".into()));
                    }
                }
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    n.boundaries.push(Boundary {
                        id: new_id(),
                        targets: ts,
                        color: color.clone(),
                    });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetBoundaryColor { parent, id, color } => {
                if let Some(hex) = color {
                    if crate::theme::Rgba::parse(hex).is_none() {
                        return Err(CoreError::Invalid("边界颜色要写成 #rrggbb".into()));
                    }
                }
                if !self.nodes.contains_key(parent) {
                    return Err(CoreError::NotFound(parent.clone()));
                }
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    let Some(b) = n.boundaries.iter_mut().find(|b| &b.id == id) else {
                        return Err(CoreError::NotFound(id.clone()));
                    };
                    b.color = color.clone();
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::DeleteBoundary { parent, id } => {
                if !self.nodes.contains_key(parent) {
                    return Err(CoreError::NotFound(parent.clone()));
                }
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    let len_before = n.boundaries.len();
                    n.boundaries.retain(|b| &b.id != id);
                    if n.boundaries.len() == len_before {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::AddSummary {
                parent,
                targets,
                text,
            } => {
                let ts = self.child_targets(parent, targets)?;
                Self::check_annotation_text(text)?;
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    n.summaries.push(Summary {
                        id: new_id(),
                        targets: ts,
                        text: text.trim().to_string(),
                    });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetBoundaryTargets {
                parent,
                id,
                targets,
            } => {
                // 与创建时同一套校验：直接子节点、去重、非空。
                let normalized = self.child_targets(parent, targets)?;
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    let b = n
                        .boundaries
                        .iter_mut()
                        .find(|b| &b.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    if b.targets != normalized {
                        b.targets = normalized;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetSummaryTargets {
                parent,
                id,
                targets,
            } => {
                // 与创建时同一套校验：必须是父节点的直接子节点、去重、非空。
                let normalized = self.child_targets(parent, targets)?;
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    let s = n
                        .summaries
                        .iter_mut()
                        .find(|s| &s.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    if s.targets != normalized {
                        s.targets = normalized;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetSummaryText { parent, id, text } => {
                Self::check_annotation_text(text)?;
                if !self.nodes.contains_key(parent) {
                    return Err(CoreError::NotFound(parent.clone()));
                }
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    let s = n
                        .summaries
                        .iter_mut()
                        .find(|s| &s.id == id)
                        .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                    let normalized = text.trim().to_string();
                    if s.text != normalized {
                        s.text = normalized;
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::DeleteSummary { parent, id } => {
                if !self.nodes.contains_key(parent) {
                    return Err(CoreError::NotFound(parent.clone()));
                }
                let touched = vec![parent.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(parent).unwrap();
                    let len_before = n.summaries.len();
                    n.summaries.retain(|s| &s.id != id);
                    if n.summaries.len() == len_before {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::AddLabel { ids, label } => {
                let label = Self::check_label(label)?;
                for id in ids {
                    if !self.nodes.contains_key(id) {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                    let n = &self.nodes[id];
                    if !n.labels.iter().any(|l| l == &label) && n.labels.len() >= MAX_LABELS {
                        return Err(CoreError::Invalid(format!(
                            "每个节点最多 {MAX_LABELS} 个标签"
                        )));
                    }
                }
                let before = self.snapshot_ids(ids);
                for id in ids {
                    let n = self.nodes.get_mut(id).unwrap();
                    if !n.labels.iter().any(|l| l == &label) {
                        n.labels.push(label.clone());
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(ids);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::RemoveLabel { ids, label } => {
                let label = label.trim().to_string();
                for id in ids {
                    if !self.nodes.contains_key(id) {
                        return Err(CoreError::NotFound(id.clone()));
                    }
                }
                let before = self.snapshot_ids(ids);
                for id in ids {
                    let n = self.nodes.get_mut(id).unwrap();
                    let len_before = n.labels.len();
                    n.labels.retain(|l| l != &label);
                    if n.labels.len() != len_before {
                        n.modified_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(ids);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::AddFloatingTopic { text, x, y } => {
                Self::check_text(text)?;
                Self::check_pos(*x, *y)?;
                let id = new_id();
                let mut node = Node::new(id.clone(), None, text.clone());
                node.pos = Some(Position { x: *x, y: *y });
                let before = Vec::new();
                self.nodes.insert(id.clone(), node);
                let after = self.snapshot_ids(std::slice::from_ref(&id));
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::InsertConcepts { nodes } => {
                if !self.is_concept_map() {
                    return Err(CoreError::Invalid("只有概念图可以插入独立概念".into()));
                }
                if nodes.is_empty() {
                    return Err(CoreError::Invalid("空概念批".into()));
                }
                let mut ids: HashSet<String> = HashSet::new();
                for n in nodes {
                    if !ids.insert(n.id.clone()) {
                        return Err(CoreError::Invalid(format!("重复概念 ID：{}", n.id)));
                    }
                    if self.nodes.contains_key(&n.id) {
                        return Err(CoreError::Invalid(format!("概念 ID 冲突：{}", n.id)));
                    }
                    if n.parent_id.is_some() || !n.children.is_empty() {
                        return Err(CoreError::Invalid("概念图不能带着父子结构粘贴".into()));
                    }
                    if n.pos
                        .as_ref()
                        .is_none_or(|p| !p.x.is_finite() || !p.y.is_finite())
                    {
                        return Err(CoreError::Invalid(format!("概念缺少落点：{}", n.id)));
                    }
                    Self::check_text(&n.text)?;
                }
                // 卡片是 1:1 绑定：副本若沿用同一张卡会立刻违反 `validate`。
                // 这里让命令自己拒绝，而不是先落盘、下次打开才发现文档脏了。
                let mut taken_cards: HashSet<String> = self
                    .nodes
                    .values()
                    .filter_map(|n| n.card_id.clone())
                    .collect();
                let mut prepared: Vec<Node> = Vec::with_capacity(nodes.len());
                for n in nodes {
                    let mut c = n.clone();
                    c.children.clear();
                    c.parent_id = None;
                    c.boundaries.clear();
                    c.summaries.clear();
                    // 钉住**不随副本走**：那是原会话里用户对手工摆位的保护意图，
                    // 复制一份带过去会让副本「整理不动」，用户看不出为什么——
                    // 副本一律未钉，要钉就在副本上重新钉一次（`SetConceptPinned`）。
                    c.pinned = None;
                    c.relationships.retain(|r| {
                        r.to != c.id && (ids.contains(&r.to) || self.nodes.contains_key(&r.to))
                    });
                    let mut seen_labels = HashSet::new();
                    c.labels.retain(|l| {
                        !l.trim().is_empty()
                            && l.chars().count() <= MAX_LABEL_LEN
                            && seen_labels.insert(l.clone())
                    });
                    c.labels.truncate(MAX_LABELS);
                    if let Some(cid) = &c.card_id {
                        if !self.cards.iter().any(|card| &card.id == cid)
                            || !taken_cards.insert(cid.clone())
                        {
                            return Err(CoreError::Invalid(format!("卡片已被其它概念关联：{cid}")));
                        }
                    }
                    prepared.push(c);
                }
                let before = Vec::new();
                let touched: Vec<String> = prepared.iter().map(|n| n.id.clone()).collect();
                for n in prepared {
                    self.nodes.insert(n.id.clone(), n);
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::AddLinkedConcept { from, text, x, y } => {
                // 全靠调用点自觉不行：今天两个入口（画布拖到空白、大纲 `Tab`）碰巧都在
                // 概念图分支里，但命令层谁都能发。思维导图里发一次就会造出
                // 「`parent_id = None` 的浮点节点」，而 `validate()` 的思维导图分支只允许
                // 「自由主题或根」有 `pos`——命令成功、文档立刻不自洽。
                if !self.is_concept_map() {
                    return Err(CoreError::Invalid("只有概念图可以新建概念并连线".into()));
                }
                Self::check_text(text)?;
                Self::check_pos(*x, *y)?;
                if !self.nodes.contains_key(from) {
                    return Err(CoreError::NotFound(from.clone()));
                }
                // 概念落点与「自由主题」同一口径：`(x, y)` 是盒子左上角。
                let id = new_id();
                let mut node = Node::new(id.clone(), None, text.clone());
                node.pos = Some(Position { x: *x, y: *y });
                let before = self.snapshot_ids(std::slice::from_ref(from));
                self.nodes.insert(id.clone(), node);
                if let Some(n) = self.nodes.get_mut(from) {
                    n.relationships.push(Relationship {
                        id: new_id(),
                        to: id.clone(),
                        label: None,
                        color: None,
                        head: ArrowHead::Forward,
                        ..Default::default()
                    });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&[from.clone(), id]);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::RetargetLinkToNewConcept {
                from,
                id,
                text,
                x,
                y,
            } => {
                // 与 `AddLinkedConcept` 同一类：也是「建一颗浮点概念 + 改边」，
                // 思维导图里同样会把文档变成不自洽的状态。
                if !self.is_concept_map() {
                    return Err(CoreError::Invalid(
                        "只有概念图可以把关系线改到新概念".into(),
                    ));
                }
                Self::check_text(text)?;
                Self::check_pos(*x, *y)?;
                let src = self
                    .nodes
                    .get(from)
                    .ok_or_else(|| CoreError::NotFound(from.clone()))?;
                let slot = src
                    .relationships
                    .iter()
                    .position(|r| &r.id == id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                let before = self.snapshot_ids(std::slice::from_ref(from));
                let new_id = new_id();
                let mut node = Node::new(new_id.clone(), None, text.clone());
                node.pos = Some(Position { x: *x, y: *y });
                self.nodes.insert(new_id.clone(), node);
                if let Some(n) = self.nodes.get_mut(from) {
                    n.relationships[slot].to = new_id.clone();
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&[from.clone(), new_id]);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::MoveConcept { id, x, y } => {
                Self::check_pos(*x, *y)?;
                let n = self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                if n.parent_id.is_some() {
                    return Err(CoreError::Invalid("概念图里只有浮动概念能自由摆放".into()));
                }
                if self.meta.layout.kind != "conceptMap" {
                    return Err(CoreError::Invalid("只有概念图可以摆放主概念".into()));
                }
                let touched = vec![id.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    n.pos = Some(Position { x: *x, y: *y });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetConceptPinned { id, pinned } => {
                // 校验与 `MoveConcept` 同款三道门：id 存在、是浮动概念（`parent_id.is_none()`）、
                // 文档是概念图。思维导图/树节点上直接 `Invalid`，不许静默吞掉——
                // 钉住只对概念图的布局算法有意义，写在树节点上只会让文件变脏。
                let n = self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                if n.parent_id.is_some() {
                    return Err(CoreError::Invalid("概念图里只有浮动概念能被钉住".into()));
                }
                if self.meta.layout.kind != "conceptMap" {
                    return Err(CoreError::Invalid("只有概念图可以钉住概念".into()));
                }
                let touched = vec![id.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    // 取消时回落 `None` 而不是 `Some(false)`：`None` = 「没写过这个键」，
                    // 序列化时 `skip_serializing_if` 直接省掉，字节回到旧版形状；
                    // `Some(false)` 会往文件里多写一个 `"pinned": false`，白弄脏 diff。
                    n.pinned = if *pinned { Some(true) } else { None };
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetNodePos { id, x, y } => {
                Self::check_pos(*x, *y)?;
                let n = self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                if n.parent_id.is_some() || id == &self.root_id {
                    return Err(CoreError::Invalid("仅自由主题可以自由摆放".into()));
                }
                let touched = vec![id.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    n.pos = Some(Position { x: *x, y: *y });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetNodeCard { node, card } => {
                if !self.nodes.contains_key(node) {
                    return Err(CoreError::NotFound(node.clone()));
                }
                if let Some(id) = card {
                    if !self.cards.iter().any(|c| &c.id == id) {
                        return Err(CoreError::NotFound(format!("卡片 {id}")));
                    }
                    if self
                        .nodes
                        .values()
                        .any(|n| &n.id != node && n.card_id.as_ref() == Some(id))
                    {
                        return Err(CoreError::Invalid("卡片已经链接到另一个节点".into()));
                    }
                }
                let before_cards = self.cards.clone();
                let touched = vec![node.clone()];
                let before = self.snapshot_ids(&touched);
                let old = self.nodes[node].card_id.clone();
                if let Some(old_id) = old.as_ref() {
                    if let Some(c) = self.cards.iter_mut().find(|c| &c.id == old_id) {
                        if c.source_node.as_deref() == Some(node.as_str()) {
                            c.source_node = None;
                            c.updated_at = now_ms();
                        }
                    }
                }
                {
                    let n = self.nodes.get_mut(node).unwrap();
                    n.card_id = card.clone();
                    n.modified_at = now_ms();
                }
                if let Some(new_id) = card {
                    if let Some(c) = self.cards.iter_mut().find(|c| &c.id == new_id) {
                        c.source_node = Some(node.clone());
                        c.updated_at = now_ms();
                    }
                }
                let after = self.snapshot_ids(&touched);
                let after_cards = self.cards.clone();
                Ok(
                    ChangeSet::new(command::label(cmd).to_string(), before, after, None, None)
                        .with_cards(before_cards, after_cards),
                )
            }

            Command::AttachNewCard { node, card } => {
                if !self.nodes.contains_key(node) {
                    return Err(CoreError::NotFound(node.clone()));
                }
                if self.nodes[node].card_id.is_some() {
                    return Err(CoreError::Invalid("该节点已经有一张卡片".into()));
                }
                card.validate().map_err(CoreError::Invalid)?;
                if self.cards.iter().any(|c| c.id == card.id) {
                    return Err(CoreError::Invalid("卡片 id 已存在".into()));
                }
                if self
                    .nodes
                    .values()
                    .any(|n| n.card_id.as_ref() == Some(&card.id))
                {
                    return Err(CoreError::Invalid("卡片已经链接到另一个节点".into()));
                }
                let before_cards = self.cards.clone();
                let touched = vec![node.clone()];
                let before = self.snapshot_ids(&touched);
                let mut card = card.clone();
                card.source_node = Some(node.clone());
                if card.source_doc.is_empty() {
                    card.source_doc = self.id.clone();
                }
                card.updated_at = now_ms();
                self.cards.push(card.clone());
                {
                    let n = self.nodes.get_mut(node).unwrap();
                    n.card_id = Some(card.id.clone());
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                let after_cards = self.cards.clone();
                Ok(
                    ChangeSet::new(command::label(cmd).to_string(), before, after, None, None)
                        .with_cards(before_cards, after_cards),
                )
            }

            Command::SetCardBody { id, body } => {
                if body.len() > crate::note::MAX_CARD_BODY {
                    return Err(CoreError::Invalid("卡片正文过长".into()));
                }
                if !self.cards.iter().any(|c| &c.id == id) {
                    return Err(CoreError::NotFound(id.clone()));
                }
                let before_cards = self.cards.clone();
                if let Some(c) = self.cards.iter_mut().find(|c| &c.id == id) {
                    if &c.body != body {
                        c.body = body.clone();
                        c.updated_at = now_ms();
                    }
                }
                let after_cards = self.cards.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_cards(before_cards, after_cards))
            }

            Command::SetCardTitle { id, title } => {
                if title.chars().count() > crate::note::MAX_CARD_TITLE {
                    return Err(CoreError::Invalid("卡片标题过长".into()));
                }
                if !self.cards.iter().any(|c| &c.id == id) {
                    return Err(CoreError::NotFound(id.clone()));
                }
                let before_cards = self.cards.clone();
                if let Some(c) = self.cards.iter_mut().find(|c| &c.id == id) {
                    if &c.title != title {
                        c.title = title.clone();
                        c.updated_at = now_ms();
                    }
                }
                let after_cards = self.cards.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_cards(before_cards, after_cards))
            }

            Command::SetCardTags { id, tags } => {
                if !self.cards.iter().any(|c| &c.id == id) {
                    return Err(CoreError::NotFound(id.clone()));
                }
                let normalized = NoteCard::normalize_tags(tags);
                let before_cards = self.cards.clone();
                if let Some(c) = self.cards.iter_mut().find(|c| &c.id == id) {
                    if c.tags != normalized {
                        c.tags = normalized;
                        c.updated_at = now_ms();
                    }
                }
                let after_cards = self.cards.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_cards(before_cards, after_cards))
            }

            Command::RenameTag { from, to } => {
                let from = from.trim();
                if from.is_empty() {
                    return Err(CoreError::Invalid("原标签不能为空".into()));
                }
                let Some(to) = NoteCard::normalize_tags(std::slice::from_ref(to))
                    .into_iter()
                    .next()
                else {
                    return Err(CoreError::Invalid("新标签不能为空或过长".into()));
                };
                let before_cards = self.cards.clone();
                for c in &mut self.cards {
                    let mut tags = c.tags.clone();
                    let mut touched = false;
                    for t in tags.iter_mut() {
                        if t.eq_ignore_ascii_case(from) {
                            *t = to.clone();
                            touched = true;
                        }
                    }
                    if touched {
                        c.tags = NoteCard::normalize_tags(&tags);
                        c.updated_at = now_ms();
                    }
                }
                let after_cards = self.cards.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                )
                .with_cards(before_cards, after_cards))
            }

            Command::DetachNode { id, x, y } => {
                if id == &self.root_id {
                    return Err(CoreError::RootProtected);
                }
                Self::check_pos(*x, *y)?;
                let node = self
                    .nodes
                    .get(id)
                    .ok_or_else(|| CoreError::NotFound(id.clone()))?;
                let old_parent = node
                    .parent_id
                    .clone()
                    .ok_or_else(|| CoreError::Invalid("该节点已是自由主题".into()))?;
                let touched = vec![old_parent.clone(), id.clone()];
                let before = self.snapshot_ids(&touched);
                {
                    let p = self.nodes.get_mut(&old_parent).unwrap();
                    p.children.retain(|c| c != id);
                }
                self.normalize_annotations_of(&old_parent);
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    n.parent_id = None;
                    n.pos = Some(Position { x: *x, y: *y });
                    n.modified_at = now_ms();
                }
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    None,
                    None,
                ))
            }

            Command::SetRoot { id } => {
                // 概念图的 root_id 只是「第一个概念」，没有中心主题的树语义；
                // 判类型放在存在性之前，让错误口径与 id 无关。
                if self.is_concept_map() {
                    return Err(CoreError::Invalid("概念图里没有中心主题".into()));
                }
                if !self.nodes.contains_key(id) {
                    return Err(CoreError::NotFound(id.clone()));
                }
                if id == &self.root_id {
                    return Err(CoreError::Invalid("它已经是根主题".into()));
                }
                let old_root = self.root_id.clone();
                let old_parent = self.nodes[id].parent_id.clone();
                let mut touched = vec![old_root.clone(), id.clone()];
                if let Some(op) = &old_parent {
                    touched.push(op.clone());
                }
                let before = self.snapshot_ids(&touched);
                // 新根从原父级摘除（自由主题没有父级，直接提根）。
                if let Some(op) = &old_parent {
                    self.nodes.get_mut(op).unwrap().children.retain(|c| c != id);
                    self.normalize_annotations_of(op);
                }
                // 旧根剩下的整棵树挂到新根之下（追加在末尾）。
                // 新根自己的子树不动；旧根这棵子树里已经不包含新根（刚摘掉），不会成环。
                self.nodes
                    .get_mut(id)
                    .unwrap()
                    .children
                    .push(old_root.clone());
                {
                    let n = self.nodes.get_mut(id).unwrap();
                    n.parent_id = None;
                    // 思维导图里只有自由主题能带坐标；当了根就不再是自由主题。
                    n.pos = None;
                    n.modified_at = now_ms();
                }
                {
                    let r = self.nodes.get_mut(&old_root).unwrap();
                    r.parent_id = Some(id.clone());
                    r.modified_at = now_ms();
                }
                self.root_id = id.clone();
                let after = self.snapshot_ids(&touched);
                Ok(
                    ChangeSet::new(command::label(cmd).to_string(), before, after, None, None)
                        .with_root(old_root, id.clone()),
                )
            }

            Command::SetLayout { layout } => {
                // 从概念图切回树布局：主概念身上的自由坐标必须交出来，
                // 否则 `validate` 会判它「非自由主题带坐标」（CM-04）。
                let drop_root_pos = self.meta.layout.kind == "conceptMap"
                    && layout.kind != "conceptMap"
                    && self
                        .nodes
                        .get(&self.root_id)
                        .is_some_and(|n| n.pos.is_some());
                let touched: Vec<String> = if drop_root_pos {
                    vec![self.root_id.clone()]
                } else {
                    Vec::new()
                };
                let before = self.snapshot_ids(&touched);
                let before_meta = self.meta.clone();
                self.meta.layout = layout.clone();
                if drop_root_pos {
                    if let Some(n) = self.nodes.get_mut(&self.root_id) {
                        n.pos = None;
                        n.modified_at = now_ms();
                    }
                }
                let after_meta = self.meta.clone();
                let after = self.snapshot_ids(&touched);
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    before,
                    after,
                    Some(before_meta),
                    Some(after_meta),
                ))
            }

            Command::SetFocusQuestion { text } => {
                // 空串/纯空白一律归一成 None：磁盘上不允许出现「有键但没内容」，
                // 否则 skip_serializing_if 就白设了，旧版还会看到一个空标题。
                let cleaned = text
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_string);
                let before_meta = self.meta.clone();
                self.meta.layout.focus_question = cleaned;
                let after_meta = self.meta.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    Some(before_meta),
                    Some(after_meta),
                ))
            }

            Command::SetTheme { theme } => {
                if theme.trim().is_empty() {
                    return Err(CoreError::Invalid("主题名不能为空".into()));
                }
                let before_meta = self.meta.clone();
                self.meta.theme = theme.clone();
                let after_meta = self.meta.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    Some(before_meta),
                    Some(after_meta),
                ))
            }

            Command::SetThemeAccent { accent } => {
                if let Some(hex) = accent {
                    if crate::theme::Rgba::parse(hex).is_none() {
                        return Err(CoreError::Invalid("强调色要写成 #rrggbb".into()));
                    }
                }
                let before_meta = self.meta.clone();
                self.meta.theme_accent = accent.clone();
                let after_meta = self.meta.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    Some(before_meta),
                    Some(after_meta),
                ))
            }

            Command::SetThemeOverrides { overrides } => {
                // 每个槽位要么是合法 #rrggbb、要么没设置；坏值直接拒绝，
                // 否则界面会「设了又没生效」，比报错难查。
                if let Some(o) = overrides {
                    for hex in [
                        &o.canvas_bg,
                        &o.central_fill,
                        &o.level1_fill,
                        &o.text,
                        &o.text_central,
                        &o.text_main,
                        &o.text_sub,
                        &o.edge_color,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if crate::theme::Rgba::parse(hex).is_none() {
                            return Err(CoreError::Invalid("颜色要写成 #rrggbb".into()));
                        }
                    }
                }
                let stored = overrides.clone().filter(|o| !o.is_empty());
                let before_meta = self.meta.clone();
                self.meta.theme_overrides = stored;
                let after_meta = self.meta.clone();
                Ok(ChangeSet::new(
                    command::label(cmd).to_string(),
                    Vec::new(),
                    Vec::new(),
                    Some(before_meta),
                    Some(after_meta),
                ))
            }

            Command::Batch { commands } => {
                // 事务语义：任一子命令失败则整体回滚。`Document` 的每个可变域都必须
                // 备份——只回滚 `nodes`/`meta` 会把改过卡片、分组与根的半截事务留在
                // 文档里（`validate` 判脏，用户还以为操作没生效）。
                let backup_nodes = self.nodes.clone();
                let backup_meta = self.meta.clone();
                let backup_cards = self.cards.clone();
                let backup_groups = self.groups.clone();
                let backup_root = self.root_id.clone();
                let mut acc: Option<ChangeSet> = None;
                for c in commands {
                    match self.apply_inner(c) {
                        Ok(cs) => {
                            acc = Some(match acc {
                                None => cs,
                                Some(a) => merge(a, cs),
                            });
                        }
                        Err(e) => {
                            self.nodes = backup_nodes;
                            self.meta = backup_meta;
                            self.cards = backup_cards;
                            self.groups = backup_groups;
                            self.root_id = backup_root;
                            return Err(e);
                        }
                    }
                }
                let mut cs = acc.unwrap_or_default();
                cs.label = command::label(cmd).to_string();
                Ok(cs)
            }
        }
    }

    /// 执行命令，并允许它**并进上一条撤销**。`merge` 看到的是上一条变更集。
    ///
    /// 只给「同一次编辑里的第二步」用：`Tab` 先建一枚空节点（一条命令），用户敲完字
    /// 提交又是一条命令。两条分开记，按一次 `Ctrl+Z` 只会清掉刚写的字、把一个空盒子
    /// 留在树上——而用户要撤的显然是「刚建的这个」。合并后栈里只留一条：
    /// 撤销回到建之前（盒子连字一起没了），重做回到写完字。
    ///
    /// 判据由调用方给（`mw-core` 不认识「编辑器里刚建的那颗」这种 UI 概念），
    /// 返回 `false` 时行为与 [`apply`](Self::apply) 完全一致。
    pub fn apply_coalesced(
        &mut self,
        cmd: &Command,
        merge: impl FnOnce(&ChangeSet) -> bool,
    ) -> Result<ChangeSet, CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let cs = self.apply_inner(cmd)?;
        if cs.is_empty() {
            return Ok(cs);
        }
        self.revision += 1;
        if Self::changes_style_content(cmd) {
            self.content_rev += 1;
        }
        let merged = match self.undo_stack.last_mut() {
            Some(prev) if merge(prev) => {
                // `before` 留上一条的（建之前）；`after` 按 **id 合并**：这一条只覆盖它
                // 碰到的那些节点，其余（最要紧的是父节点的 `children` 列表）必须留上一条的。
                // 整段覆盖会把「甲 挂到 导出 名下」这半条丢掉——撤销正常、重做却把节点
                // 塞进 `nodes` 却不挂进父节点，树上什么都看不见。
                let touched: HashSet<&str> = cs.after.iter().map(|n| n.id.as_str()).collect();
                prev.after.retain(|n| !touched.contains(n.id.as_str()));
                // 这一条删掉的节点同理不能留：`after` 里留着它，重做会把它复活。
                let removed: HashSet<&str> = cs
                    .before
                    .iter()
                    .filter(|n| !cs.after.iter().any(|m| m.id == n.id))
                    .map(|n| n.id.as_str())
                    .collect();
                prev.after.retain(|n| !removed.contains(n.id.as_str()));
                prev.after.extend(cs.after.iter().cloned());
                if cs.after_meta.is_some() {
                    prev.after_meta = cs.after_meta.clone();
                }
                if !cs.after_cards.is_empty() {
                    prev.after_cards = cs.after_cards.clone();
                }
                if !cs.after_groups.is_empty() {
                    prev.after_groups = cs.after_groups.clone();
                }
                if cs.after_root.is_some() {
                    prev.after_root = cs.after_root.clone();
                }
                true
            }
            _ => false,
        };
        if !merged {
            self.undo_stack.push(cs.clone());
            self.trim_undo_history();
        }
        self.redo_stack.clear();
        Ok(cs)
    }

    /// 撤销栈上限（本会话）。
    pub fn history_limit(&self) -> usize {
        self.history_limit
    }

    /// 调整撤销栈上限：clamp 到 [`MIN_HISTORY_LIMIT`]..=[`MAX_HISTORY_LIMIT`]，
    /// 并立即丢掉超出的最旧记录。上限是用户偏好，不写 `.zmind`。
    pub fn set_history_limit(&mut self, limit: usize) {
        self.history_limit = limit.clamp(MIN_HISTORY_LIMIT, MAX_HISTORY_LIMIT);
        self.trim_undo_history();
    }

    fn trim_undo_history(&mut self) {
        let excess = self.undo_stack.len().saturating_sub(self.history_limit);
        if excess > 0 {
            // 一次 `drain` 搬完要保留的那一段；以前是 `while … remove(0)`，每裁一条
            // 都把整条 Vec 前移一次（上限 2000 时是 O(n²) 的 memmove，20k 文档入栈有实际开销）。
            self.undo_stack.drain(..excess);
        }
    }

    /// 撤销一步；返回被撤销的变更集（调用方用 backward_wire 广播）。
    pub fn undo(&mut self) -> Option<ChangeSet> {
        if self.read_only {
            return None;
        }
        let cs = self.undo_stack.pop()?;
        self.content_rev += 1;
        self.revert(&cs);
        self.redo_stack.push(cs.clone());
        self.revision += 1;
        Some(cs)
    }

    /// 重做一步；返回变更集（调用方用 forward_wire 广播）。
    pub fn redo(&mut self) -> Option<ChangeSet> {
        if self.read_only {
            return None;
        }
        let cs = self.redo_stack.pop()?;
        self.content_rev += 1;
        self.reapply(&cs);
        self.undo_stack.push(cs.clone());
        self.trim_undo_history();
        self.revision += 1;
        Some(cs)
    }

    fn revert(&mut self, cs: &ChangeSet) {
        let before_ids: HashSet<&str> = cs.before.iter().map(|n| n.id.as_str()).collect();
        for n in &cs.after {
            if !before_ids.contains(n.id.as_str()) {
                self.nodes.remove(&n.id);
            }
        }
        for n in &cs.before {
            self.nodes.insert(n.id.clone(), n.clone());
        }
        if let Some(m) = &cs.before_meta {
            self.meta = m.clone();
        }
        if !cs.before_cards.is_empty() || !cs.after_cards.is_empty() {
            self.cards = cs.before_cards.clone();
        }
        if !cs.before_groups.is_empty() || !cs.after_groups.is_empty() {
            self.groups = cs.before_groups.clone();
        }
        if let Some(r) = &cs.before_root {
            self.root_id = r.clone();
        }
    }

    fn reapply(&mut self, cs: &ChangeSet) {
        let after_ids: HashSet<&str> = cs.after.iter().map(|n| n.id.as_str()).collect();
        for n in &cs.before {
            if !after_ids.contains(n.id.as_str()) {
                self.nodes.remove(&n.id);
            }
        }
        for n in &cs.after {
            self.nodes.insert(n.id.clone(), n.clone());
        }
        if let Some(m) = &cs.after_meta {
            self.meta = m.clone();
        }
        if !cs.before_cards.is_empty() || !cs.after_cards.is_empty() {
            self.cards = cs.after_cards.clone();
        }
        if !cs.before_groups.is_empty() || !cs.after_groups.is_empty() {
            self.groups = cs.after_groups.clone();
        }
        if let Some(r) = &cs.after_root {
            self.root_id = r.clone();
        }
    }

    /// 文本 + 拼音检索：按分数降序，同分按文档顺序（保证「上一个/下一个」可预测）。
    pub fn search(&mut self, query: &str) -> Vec<SearchHit> {
        self.search_with_mode(query, MatchMode::Pinyin)
    }

    /// 按匹配方式检索：`Text` 只做原文子串，`Pinyin` 含全拼与首字母。
    pub fn search_with_mode(&mut self, query: &str, mode: MatchMode) -> Vec<SearchHit> {
        let order = self.doc_order();
        self.search.hits(&self.nodes, &order, query, mode)
    }

    /// 概念图命题短语检索（关系线是边不是节点，单独一组）。
    pub fn search_relationships(&mut self, query: &str) -> Vec<RelationshipHit> {
        self.search_relationships_with_mode(query, MatchMode::Pinyin)
    }

    /// 按匹配方式检索命题短语。
    pub fn search_relationships_with_mode(
        &mut self,
        query: &str,
        mode: MatchMode,
    ) -> Vec<RelationshipHit> {
        let order = self.doc_order();
        self.search
            .relationship_hits(&self.nodes, &order, query, mode)
    }
}

/// 链接校验：允许 http/https/file/mailto，或本地路径/相对路径；拒绝控制字符与空 scheme。
fn valid_link(raw: &str) -> bool {
    let t = raw.trim();
    if t.is_empty() {
        return true;
    }
    if t.chars().any(|c| c.is_control()) {
        return false;
    }
    if let Some((scheme, rest)) = t.split_once("://") {
        if scheme.is_empty() || rest.trim().is_empty() {
            return false;
        }
        return matches!(
            scheme.to_ascii_lowercase().as_str(),
            "http" | "https" | "file"
        );
    }
    if let Some(rest) = t.strip_prefix("mailto:") {
        return !rest.trim().is_empty();
    }
    // 没有 `://` 但带 ASCII scheme 的（javascript:、data:…）一律拒绝；单字母盘符放行。
    if let Some((scheme, _)) = t.split_once(':') {
        if scheme.len() > 1
            && scheme.chars().all(|c| c.is_ascii_alphanumeric())
            && !matches!(scheme.to_ascii_lowercase().as_str(), "file" | "mailto")
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LayoutSettings, StyleOverride};

    fn doc() -> Document {
        Document::new("测试")
    }

    /// M12h：只改文字不动 `content_rev`，样式/结构命令与撤销/重做都要 +1。
    #[test]
    fn text_edits_do_not_bump_the_content_revision() {
        let mut d = doc();
        let root = d.root_id.clone();
        let base = d.content_rev;
        d.apply(&Command::SetText {
            id: root.clone(),
            text: "文字".into(),
        })
        .unwrap();
        assert_eq!(d.content_rev, base, "SetText 不该让内容指纹失效");

        let style = StyleOverride {
            fill: Some("#123456".into()),
            ..Default::default()
        };
        d.apply(&Command::SetStyle {
            ids: vec![root.clone()],
            style: Some(style),
        })
        .unwrap();
        assert_eq!(d.content_rev, base + 1, "改样式必须让内容指纹失效");

        d.undo();
        assert_eq!(d.content_rev, base + 2, "撤销一律保守 +1");
        d.redo();
        assert_eq!(d.content_rev, base + 3, "重做一律保守 +1");
    }

    /// X29 G-1：只读闸在**唯一的命令入口**上，不靠各面板自己记得判断。    /// X29 G-1：只读闸在**唯一的命令入口**上，不靠各面板自己记得判断。
    ///
    /// 覆盖四条路径：`apply` / `apply_coalesced` / `undo` / `redo`；
    /// 拒绝时 revision 与节点文本都不许动；解除只读后历史还能继续用。
    #[test]
    fn read_only_blocks_apply_undo_and_redo_until_cleared() {
        let mut d = doc();
        let root = d.root_id.clone();
        d.apply(&Command::SetText {
            id: root.clone(),
            text: "甲".into(),
        })
        .unwrap();
        d.apply(&Command::SetText {
            id: root.clone(),
            text: "乙".into(),
        })
        .unwrap();
        // 先造一条可重做的历史，再进只读。
        assert!(d.undo().is_some());
        assert_eq!(d.node(&root).unwrap().text, "甲");
        assert_eq!(d.redo_depth(), 1);

        d.read_only = true;
        let rev = d.revision;
        assert!(matches!(
            d.apply(&Command::SetText {
                id: root.clone(),
                text: "丙".into()
            }),
            Err(CoreError::ReadOnly)
        ));
        assert!(d
            .apply_coalesced(
                &Command::SetText {
                    id: root.clone(),
                    text: "丁".into()
                },
                |_| true
            )
            .is_err());
        assert!(d.undo().is_none());
        assert!(d.redo().is_none());
        assert_eq!(d.revision, rev, "被拒绝的命令不能推进 revision");
        assert_eq!(d.node(&root).unwrap().text, "甲");

        // 解除只读后，撤/重做仍然接着原来的历史走。
        d.read_only = false;
        assert!(!d
            .apply(&Command::SetText {
                id: root.clone(),
                text: "戊".into()
            })
            .unwrap()
            .is_empty());
        assert_eq!(d.node(&root).unwrap().text, "戊");
    }

    #[test]
    fn cards_are_validated_and_one_node_links_at_most_one() {
        let mut d = doc();
        let root = d.root_id.clone();
        d.cards.push(NoteCard::new(
            d.id.clone(),
            Some(root.clone()),
            "概念卡",
            "# 正文",
            1,
        ));
        d.nodes.get_mut(&root).unwrap().card_id = Some(d.cards[0].id.clone());
        assert!(d.validate().is_ok());
        assert_eq!(d.card_for_node(&root).unwrap().title, "概念卡");
        assert_eq!(d.cards().len(), 1);
        // 同一节点再挂一张：结构损坏
        d.cards.push(NoteCard::new(
            d.id.clone(),
            Some(root.clone()),
            "第二张",
            "",
            2,
        ));
        assert!(d.validate().is_err());
        // 孤儿卡（不挂节点）可以共存
        d.cards.retain(|c| c.title != "第二张");
        d.cards
            .push(NoteCard::new(d.id.clone(), None, "收件箱", "", 3));
        assert!(d.validate().is_ok());
    }

    /// 规范序列化：节点按文档前序、卡片按 id，避免 HashMap 顺序影响比较。
    fn canon(d: &Document) -> String {
        let mut out = format!("{:?}|{:?}", d.meta, d.assets.len());
        for id in d.doc_order() {
            if let Some(n) = d.nodes.get(&id) {
                out.push_str(&format!("{n:?}"));
            }
        }
        let mut cards = d.cards.clone();
        cards.sort_by(|a, b| a.id.cmp(&b.id));
        out.push_str(&format!("{cards:?}"));
        out
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn pick(&mut self, n: usize) -> usize {
            (self.next() as usize) % n.max(1)
        }
    }

    #[test]
    fn one_thousand_random_commands_undo_redo_byte_identically() {
        let mut d = doc();
        let mut rng = Lcg(0x5EED);
        for round in 0..1000 {
            let order = d.doc_order();
            let root = d.root_id.clone();
            let kids: Vec<String> = order.iter().filter(|id| **id != root).cloned().collect();
            let pick = if kids.is_empty() {
                root.clone()
            } else {
                kids[rng.pick(kids.len())].clone()
            };
            let cmd = match rng.pick(10) {
                0 if d.node_count() < 40 => Command::AddNode {
                    parent: pick,
                    index: None,
                    text: format!("r{round}"),
                    style: None,
                },
                1 => Command::SetText {
                    id: pick,
                    text: format!("改 {round}"),
                },
                2 => Command::SetNote {
                    id: pick,
                    note: Some(format!("备注 {round}")),
                },
                3 => Command::SetStyle {
                    ids: vec![pick],
                    style: Some(StyleOverride {
                        text_color: Some(format!("#{:06x}", (round * 7919) % 0xffffff)),
                        ..Default::default()
                    }),
                },
                4 => Command::SetCollapsed {
                    id: pick,
                    collapsed: round % 2 == 0,
                },
                5 => Command::AddLabel {
                    ids: vec![pick],
                    label: format!("标签{}", round % 7),
                },
                6 => Command::SetTask {
                    id: pick,
                    task: Some(crate::TaskState::Doing),
                },
                7 if !kids.is_empty() => Command::DeleteNodes { ids: vec![pick] },
                8 => {
                    let parent = if kids.is_empty() {
                        root.clone()
                    } else {
                        kids[rng.pick(kids.len())].clone()
                    };
                    Command::MoveNode {
                        id: pick,
                        new_parent: parent,
                        index: None,
                    }
                }
                _ => {
                    let card = NoteCard::new(
                        d.id.clone(),
                        None,
                        format!("卡 {round}"),
                        "正文",
                        round as i64,
                    );
                    Command::AttachNewCard { node: pick, card }
                }
            };
            let before = canon(&d);
            let Ok(cs) = d.apply(&cmd) else {
                continue;
            };
            // 无变化的命令不入撤销栈，也就没有 undo/redo 可测。
            if cs.is_empty() {
                assert_eq!(canon(&d), before, "空 ChangeSet 不应改文档");
                continue;
            }
            let after = canon(&d);
            d.undo().expect("已应用的命令必须能撤销");
            let got = canon(&d);
            if got != before {
                let at = got
                    .bytes()
                    .zip(before.bytes())
                    .position(|(a, b)| a != b)
                    .unwrap_or(got.len().min(before.len()));
                let lo = at.saturating_sub(80);
                let hi = (at + 80).min(got.len().max(before.len()));
                panic!(
                    "第 {round} 轮撤销后不一致：{cmd:?}\nbefore[{lo}..]: {:?}\ngot[{lo}..]: {:?}",
                    &before[lo..hi.min(before.len())],
                    &got[lo..hi.min(got.len())]
                );
            }
            d.redo().expect("撤销后必须能重做");
            assert_eq!(canon(&d), after, "第 {round} 轮重做后不一致");
            assert!(d.node_count() < 80, "节点数量失控");
        }
    }

    #[test]
    fn card_refs_resolve_and_backlink_other_cards() {
        let mut d = doc();
        let a = NoteCard::new(d.id.clone(), None, "甲卡", "正文", 1);
        let mut b = NoteCard::new(d.id.clone(), None, "乙卡", "", 2);
        b.body = format!("看 [[{}]] 和 [[不存在的]]", a.id);
        d.cards = vec![a.clone(), b.clone()];
        let back = d.backlinks_for_card(&a.id);
        assert!(back.iter().any(|x| x.card && x.node_id == b.id));
        assert!(d
            .backlinks_for_card(&a.id)
            .iter()
            .any(|x| x.text.contains("乙卡")));
        let (ok, broken) = d.card_refs(&b.body);
        assert_eq!(ok, vec![a.id.clone()]);
        assert_eq!(broken, vec!["不存在的".to_string()]);
    }

    #[test]
    fn attach_card_is_one_undo_and_link_is_one_to_one() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "概念");
        let b = add(&mut d, &root, "另一条");
        let card = NoteCard::new(d.id.clone(), None, "卡片", "# 正文", 1);
        d.apply(&Command::AttachNewCard {
            node: a.clone(),
            card: card.clone(),
        })
        .unwrap();
        let cid = d.cards[0].id.clone();
        assert_eq!(d.card_for_node(&a).unwrap().body, "# 正文");
        assert_eq!(d.cards[0].source_node.as_deref(), Some(a.as_str()));
        let back = d.backlinks_for_card(&cid);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].node_id, a);
        assert_eq!(back[0].text, "概念");

        d.undo().unwrap();
        assert!(d.cards.is_empty(), "撤销要连卡片一起收回");
        assert!(d.card_for_node(&a).is_none());
        d.redo().unwrap();
        assert_eq!(d.card_for_node(&a).unwrap().id, cid);

        // 一张卡片不能挂两个节点
        assert!(d
            .apply(&Command::SetNodeCard {
                node: b,
                card: Some(cid.clone()),
            })
            .is_err());
        // 解除链接：卡片保留，sourceNode 清空
        d.apply(&Command::SetNodeCard {
            node: a.clone(),
            card: None,
        })
        .unwrap();
        assert!(d.card_for_node(&a).is_none());
        assert_eq!(d.cards.len(), 1);
        assert!(d.cards[0].source_node.is_none());
        d.undo().unwrap();
        assert!(d.card_for_node(&a).is_some(), "解除也可撤销");
    }

    /// 新建节点 + 写名字合成**一条**撤销：一次 `Ctrl+Z` 应该把整个盒子收走。
    ///
    /// 这是 M8 的判据：`Tab` 建节点是一条命令、提交名字又是一条，分开记的话
    /// 第一次撤销只清掉文字、留一个空盒子在树上。
    #[test]
    fn coalesced_create_and_text_is_one_undo() {
        let mut d = doc();
        let root = d.root_id.clone();
        let n = add(&mut d, &root, "");
        d.apply_coalesced(
            &Command::SetText {
                id: n.clone(),
                text: "甲".into(),
            },
            |prev| prev.created(&n),
        )
        .unwrap();

        d.undo().unwrap();
        assert!(d.node(&n).is_none(), "一次撤销要把节点连字一起收回");
        assert!(!d.can_undo(), "合并后栈里只剩一条");

        d.redo().unwrap();
        assert_eq!(
            d.node(&n).map(|x| x.text.as_str()),
            Some("甲"),
            "重做连字一起回来"
        );
        // 父节点的 `children` 也必须一起回来：只把节点塞进 `nodes` 而不挂进父节点，
        // 树上什么都看不见（合并时整段覆盖 `after` 就踩过这个坑）。
        assert!(
            d.node(&root).is_some_and(|r| r.children.contains(&n)),
            "重做后要挂在父节点名下"
        );
    }

    /// 判据说不合并时，行为与 `apply` 一致：两条命令就是两步撤销。
    #[test]
    fn coalescing_is_opt_in_per_call() {
        let mut d = doc();
        let root = d.root_id.clone();
        let n = add(&mut d, &root, "");
        d.apply_coalesced(
            &Command::SetText {
                id: n.clone(),
                text: "甲".into(),
            },
            |_| false,
        )
        .unwrap();

        d.undo().unwrap();
        assert_eq!(d.node(&n).map(|x| x.text.as_str()), Some(""), "先只撤文字");
        d.undo().unwrap();
        assert!(d.node(&n).is_none(), "再撤才收走节点");
    }

    /// CM-06：卡片页「来源」要文档真名，不是「当前文档」；概念图要能被认出来。
    #[test]
    fn display_title_falls_back_and_marks_concept_maps() {
        let mut d = doc();
        assert!(!d.is_concept_map(), "默认是思维导图");
        d.meta.title = "  季度复盘  ".into();
        assert_eq!(d.display_title(), "季度复盘", "首尾空白要吃掉");
        d.meta.title = "   ".into();
        assert_eq!(d.display_title(), "未命名", "空白标题不能原样摆到界面上");

        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        assert!(d.is_concept_map());
        // 布局切回树：概念图身份必须跟着走，不能粘住。
        d.apply(&Command::SetLayout {
            layout: LayoutSettings::default(),
        })
        .unwrap();
        assert!(!d.is_concept_map());
    }

    /// CM-06：检查器「关联已有」的候选——别人占着的卡片不许出现，最近更新的排前面。
    #[test]
    fn linkable_cards_exclude_cards_taken_by_other_nodes() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        let b = add(&mut d, &root, "乙");
        let old = NoteCard::new(d.id.clone(), None, "旧卡", "", 1);
        let new = NoteCard::new(d.id.clone(), None, "新卡", "", 9);
        d.cards = vec![old.clone(), new.clone()];
        d.apply(&Command::SetNodeCard {
            node: a.clone(),
            card: Some(old.id.clone()),
        })
        .unwrap();

        let for_b: Vec<&str> = d
            .linkable_cards(&b, 8)
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(for_b, vec![new.id.as_str()], "挂着旧卡的节点占住了它");

        // 对已经挂着旧卡的节点自己：换绑也要能选回原来那张。
        let for_a: Vec<&str> = d
            .linkable_cards(&a, 8)
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert!(for_a.contains(&old.id.as_str()));
        assert!(for_a.contains(&new.id.as_str()));

        assert_eq!(d.linkable_cards(&a, 1).len(), 1, "limit 要生效");
        // `SetNodeCard` 会把卡片的 `updated_at` 刷成此刻（挂上节点算一次更新），
        // 排序断言要看干净的时间，先把被刷掉的那个抹回去。
        d.cards
            .iter_mut()
            .find(|c| c.id == old.id)
            .unwrap()
            .updated_at = 1;
        assert_eq!(
            d.linkable_cards(&a, 1)[0].id,
            new.id,
            "最近更新的排前面，列表才不用翻"
        );
    }

    /// CM-06：概念图的节点同样是反查目标——卡片能找回它的概念名。
    #[test]
    fn concept_card_backlink_names_the_concept() {
        let mut d = doc();
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = d.root_id.clone();
        d.apply(&Command::SetText {
            id: root.clone(),
            text: "用户痛点".into(),
        })
        .unwrap();
        let card = NoteCard::new(d.id.clone(), None, "痛点访谈", "", 5);
        d.apply(&Command::AttachNewCard {
            node: root.clone(),
            card,
        })
        .unwrap();
        let cid = d.cards[0].id.clone();
        let back = d.backlinks_for_card(&cid);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].text, "用户痛点");
        assert!(!back[0].card, "节点反链不是卡片引用");
        assert_eq!(d.cards[0].source_node.as_deref(), Some(root.as_str()));
    }

    #[test]
    fn card_body_and_title_are_undoable() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "概念");
        let card = NoteCard::new(d.id.clone(), None, "旧标题", "旧正文", 1);
        d.apply(&Command::AttachNewCard { node: a, card }).unwrap();
        let id = d.cards[0].id.clone();
        let rev = d.revision;
        d.apply(&Command::SetCardBody {
            id: id.clone(),
            body: "# 新正文".into(),
        })
        .unwrap();
        d.apply(&Command::SetCardTitle {
            id: id.clone(),
            title: "新标题".into(),
        })
        .unwrap();
        assert_eq!(d.cards[0].body, "# 新正文");
        assert_eq!(d.cards[0].title, "新标题");
        assert!(d.revision > rev);
        d.undo().unwrap();
        assert_eq!(d.cards[0].title, "旧标题");
        d.undo().unwrap();
        assert_eq!(d.cards[0].body, "旧正文");
        // 内容没变：不产生新历史
        let rev = d.revision;
        d.apply(&Command::SetCardBody {
            id: id.clone(),
            body: "旧正文".into(),
        })
        .unwrap();
        assert_eq!(d.revision, rev, "无变化不应入历史");
        assert!(d
            .apply(&Command::SetCardBody {
                id: "missing".into(),
                body: "x".into(),
            })
            .is_err());
    }

    #[test]
    fn layout_direction_is_saved_and_undoable() {
        let mut d = doc();
        let mut l = d.meta.layout.clone();
        l.direction = "right".into();
        d.apply(&Command::SetLayout { layout: l }).unwrap();
        assert_eq!(d.meta.layout.direction, "right");
        d.undo().unwrap();
        assert_eq!(d.meta.layout.direction, "auto");
    }

    #[test]
    fn focus_question_is_normalised_and_undoable() {
        let mut d = doc();
        assert_eq!(d.meta.layout.focus_question, None);

        // 两头空白要在入库前削掉：否则画布上那条标题会莫名缩进一截。
        d.apply(&Command::SetFocusQuestion {
            text: Some("  为什么要换掉这套 CMS？  ".into()),
        })
        .unwrap();
        assert_eq!(
            d.meta.layout.focus_question.as_deref(),
            Some("为什么要换掉这套 CMS？")
        );

        d.undo().expect("撤销");
        assert_eq!(d.meta.layout.focus_question, None, "撤销要回到「没写」");
        d.redo().expect("重做");
        assert_eq!(
            d.meta.layout.focus_question.as_deref(),
            Some("为什么要换掉这套 CMS？"),
            "重做要把那句话拿回来"
        );

        // 改成纯空白 = 清空，而不是存一个「有键、看不见」的状态。
        d.apply(&Command::SetFocusQuestion {
            text: Some("   ".into()),
        })
        .unwrap();
        assert_eq!(d.meta.layout.focus_question, None, "纯空白应归一成 None");
    }

    #[test]
    fn task_state_is_undoable_and_link_is_validated() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "任务");
        d.apply(&Command::SetTask {
            id: a.clone(),
            task: Some(crate::model::TaskState::Doing),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].task, Some(crate::model::TaskState::Doing));
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].task, None);

        assert!(d
            .apply(&Command::SetLink {
                id: a.clone(),
                link: Some("https://example.com".into()),
            })
            .is_ok());
        assert!(d
            .apply(&Command::SetLink {
                id: a.clone(),
                link: Some("javascript:alert(1)".into()),
            })
            .is_err());
        assert!(d
            .apply(&Command::SetLink {
                id: a.clone(),
                link: Some("https://".into()),
            })
            .is_err());
        assert!(d
            .apply(&Command::SetLink {
                id: a,
                link: Some("C:\\资料\\a.md".into()),
            })
            .is_ok());
    }

    #[test]
    fn card_tags_are_normalized_and_undoable() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "概念");
        let card = NoteCard::new(d.id.clone(), None, "T", "", 1);
        d.apply(&Command::AttachNewCard { node: a, card }).unwrap();
        let id = d.cards[0].id.clone();
        d.apply(&Command::SetCardTags {
            id: id.clone(),
            tags: vec![" 知识 ".into(), "知识".into(), "方法/阅读".into()],
        })
        .unwrap();
        assert_eq!(d.cards[0].tags, vec!["知识", "方法/阅读"]);
        d.undo().unwrap();
        assert!(d.cards[0].tags.is_empty());
        d.redo().unwrap();
        assert_eq!(d.cards[0].tags.len(), 2);
        d.apply(&Command::RenameTag {
            from: "知识".into(),
            to: "Knowledge".into(),
        })
        .unwrap();
        assert_eq!(d.cards[0].tags, vec!["Knowledge", "方法/阅读"]);
        d.undo().unwrap();
        assert_eq!(d.cards[0].tags[0], "知识");
    }

    #[test]
    fn deleting_a_node_keeps_its_card_as_an_orphan() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "会删掉的节点");
        let card = NoteCard::new(d.id.clone(), None, "留下的卡片", "正文", 1);
        d.apply(&Command::AttachNewCard {
            node: a.clone(),
            card,
        })
        .unwrap();
        d.apply(&Command::DeleteNodes {
            ids: vec![a.clone()],
        })
        .unwrap();
        assert!(!d.nodes.contains_key(&a));
        assert_eq!(d.cards.len(), 1, "删节点不删卡片");
        assert_eq!(d.cards[0].source_node.as_deref(), Some(a.as_str()));
        assert!(d.validate().is_ok(), "孤儿卡片不算结构损坏");
    }

    fn add(d: &mut Document, parent: &str, text: &str) -> String {
        let cs = d
            .apply(&Command::AddNode {
                parent: parent.to_string(),
                index: None,
                text: text.to_string(),
                style: None,
            })
            .unwrap();
        let before_ids: HashSet<&str> = cs.before.iter().map(|n| n.id.as_str()).collect();
        cs.after
            .iter()
            .find(|n| !before_ids.contains(n.id.as_str()))
            .expect("新增节点应出现在 after 中")
            .id
            .clone()
    }

    fn text_of(d: &Document, id: &str) -> String {
        d.node(id).unwrap().text.clone()
    }

    #[test]
    fn new_doc_has_root_and_validates() {
        let d = doc();
        assert_eq!(d.node_count(), 1);
        assert!(d.nodes.contains_key(&d.root_id));
        assert!(d.validate().is_ok());
    }

    #[test]
    fn add_undo_redo() {
        let mut d = doc();
        let root = d.root_id.clone();
        let id = add(&mut d, &root, "子节点");
        assert_eq!(d.node_count(), 2);
        assert_eq!(d.nodes[&root].children, vec![id.clone()]);
        assert_eq!(d.revision, 1);

        let cs = d.undo().unwrap();
        assert_eq!(d.node_count(), 1);
        assert!(d.nodes[&root].children.is_empty());
        let w = cs.backward_wire();
        assert!(w.removed.contains(&id));
        assert!(d.validate().is_ok());

        d.redo().unwrap();
        assert_eq!(d.node_count(), 2);
        assert_eq!(d.nodes[&root].children, vec![id.clone()]);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn delete_subtree_restores_order_and_content() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        add(&mut d, &a, "A1");
        let a2 = add(&mut d, &a, "A2");
        let b = add(&mut d, &root, "B");
        assert_eq!(d.node_count(), 5);

        d.apply(&Command::DeleteNodes {
            ids: vec![a.clone()],
        })
        .unwrap();
        assert_eq!(d.node_count(), 2);
        assert_eq!(d.nodes[&root].children, vec![b.clone()]);
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert_eq!(d.node_count(), 5);
        assert_eq!(d.nodes[&root].children, vec![a.clone(), b.clone()]);
        assert_eq!(d.nodes[&a].children.len(), 2);
        assert_eq!(text_of(&d, &a2), "A2");
        assert!(d.validate().is_ok());
    }

    #[test]
    fn move_node_between_parents_and_undo() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        let x = add(&mut d, &a, "X");

        d.apply(&Command::MoveNode {
            id: x.clone(),
            new_parent: b.clone(),
            index: None,
        })
        .unwrap();
        assert!(d.nodes[&a].children.is_empty());
        assert_eq!(d.nodes[&b].children, vec![x.clone()]);
        assert_eq!(d.nodes[&x].parent_id.as_deref(), Some(b.as_str()));
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert_eq!(d.nodes[&a].children, vec![x.clone()]);
        assert!(d.nodes[&b].children.is_empty());
        assert!(d.validate().is_ok());
    }

    /// M31「设为根主题」：新根摘出原父级、旧根整棵挂到它下面、一次撤销全还原。
    ///
    /// 钉住三件事：① `root_id` 随命令走（撤销不回来就会 `validate` 判脏）；
    /// ② 旧父级的 `children` 顺序还原（摘除再挂回不能把 B 挤到 A 前面）；
    /// ③ 不成环——旧根子树里已经不含新根。
    #[test]
    fn set_root_rewires_the_tree_and_undoes_in_one_step() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        let x = add(&mut d, &a, "X");

        d.apply(&Command::SetRoot { id: a.clone() }).unwrap();
        assert_eq!(d.root_id, a);
        assert_eq!(d.nodes[&a].parent_id, None);
        // 新根的孩子：自己的子树在前，旧根整棵树追加在末尾。
        assert_eq!(d.nodes[&a].children, vec![x.clone(), root.clone()]);
        assert_eq!(d.nodes[&root].parent_id.as_deref(), Some(a.as_str()));
        assert_eq!(d.nodes[&root].children, vec![b.clone()]);
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert_eq!(d.root_id, root);
        assert_eq!(d.nodes[&a].parent_id.as_deref(), Some(root.as_str()));
        assert_eq!(d.nodes[&root].children, vec![a.clone(), b.clone()]);
        assert_eq!(d.nodes[&a].children, vec![x.clone()]);
        assert!(d.validate().is_ok());

        d.redo().unwrap();
        assert_eq!(d.root_id, a);
        assert!(d.validate().is_ok());
    }

    /// M31：自由主题提为根要交出自由坐标（思维导图里只有自由主题能带 `pos`）。
    #[test]
    fn set_root_clears_the_free_position_of_the_promoted_topic() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        d.apply(&Command::DetachNode {
            id: a.clone(),
            x: 300.0,
            y: 120.0,
        })
        .unwrap();
        assert!(d.nodes[&a].pos.is_some());

        d.apply(&Command::SetRoot { id: a.clone() }).unwrap();
        assert_eq!(d.root_id, a);
        assert_eq!(d.nodes[&a].pos, None);
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert_eq!(d.root_id, root);
        assert!(d.nodes[&a].pos.is_some());
        assert!(d.validate().is_ok());
    }

    /// M31：从更深处提根时，祖先链整体留在旧根那一侧，撤销逐字还原。
    #[test]
    fn set_root_from_grandchild_keeps_the_ancestor_chain_under_the_old_root() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        let x = add(&mut d, &a, "X");
        let y = add(&mut d, &x, "Y");

        d.apply(&Command::SetRoot { id: y.clone() }).unwrap();
        assert_eq!(d.root_id, y);
        // 新根只剩自己的孩子（没有），旧根整棵挂上来；X 的孩子里少了 Y。
        assert_eq!(d.nodes[&y].children, vec![root.clone()]);
        assert_eq!(d.nodes[&x].children, Vec::<String>::new());
        assert_eq!(d.nodes[&root].children, vec![a.clone(), b.clone()]);
        assert_eq!(d.nodes[&a].children, vec![x.clone()]);
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert_eq!(d.root_id, root);
        assert_eq!(d.nodes[&x].children, vec![y.clone()]);
        assert!(d.validate().is_ok());
    }

    /// M31：概念图没有中心主题；根自己与不存在的 id 各有各的错误口径。
    #[test]
    fn set_root_rejects_concept_maps_the_root_itself_and_missing_ids() {
        let mut d = concept_doc(&[("甲", 0.0, 0.0), ("乙", 200.0, 0.0)]);
        let second = d
            .nodes
            .values()
            .find(|n| n.id != d.root_id)
            .unwrap()
            .id
            .clone();
        let err = d
            .apply(&Command::SetRoot { id: second })
            .expect_err("概念图不许换根");
        assert!(matches!(err, CoreError::Invalid(_)), "{err:?}");

        let mut d = doc();
        let root = d.root_id.clone();
        assert!(matches!(
            d.apply(&Command::SetRoot { id: root }),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            d.apply(&Command::SetRoot {
                id: "没有这个节点".into()
            }),
            Err(CoreError::NotFound(_))
        ));
    }

    #[test]
    fn move_within_same_parent_reorders() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        let c = add(&mut d, &root, "C");

        // index 以「移动前的数组」为参照：3 = 追加到末尾
        d.apply(&Command::MoveNode {
            id: a.clone(),
            new_parent: root.clone(),
            index: Some(3),
        })
        .unwrap();
        assert_eq!(
            d.nodes[&root].children,
            vec![b.clone(), c.clone(), a.clone()]
        );
        d.undo().unwrap();

        // 2 = 插到原位置 2 的元素（C）之前
        d.apply(&Command::MoveNode {
            id: a.clone(),
            new_parent: root.clone(),
            index: Some(2),
        })
        .unwrap();
        assert_eq!(
            d.nodes[&root].children,
            vec![b.clone(), a.clone(), c.clone()]
        );
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].children, vec![a, b, c]);
    }

    #[test]
    fn cannot_move_into_own_subtree() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let a1 = add(&mut d, &a, "A1");
        let err = d
            .apply(&Command::MoveNode {
                id: a.clone(),
                new_parent: a1.clone(),
                index: None,
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::Invalid(_)));
        assert!(d.validate().is_ok());
    }

    #[test]
    fn batch_is_single_history_entry_and_rolls_back_on_error() {
        let mut d = doc();
        let root = d.root_id.clone();
        let undo_before = d.can_undo();
        assert!(!undo_before);

        d.apply(&Command::Batch {
            commands: vec![
                Command::AddNode {
                    parent: root.clone(),
                    index: None,
                    text: "一".into(),
                    style: None,
                },
                Command::AddNode {
                    parent: root.clone(),
                    index: None,
                    text: "二".into(),
                    style: None,
                },
                Command::SetText {
                    id: root.clone(),
                    text: "根".into(),
                },
            ],
        })
        .unwrap();
        assert_eq!(d.node_count(), 3);
        assert_eq!(text_of(&d, &root), "根");

        d.undo().unwrap();
        assert_eq!(d.node_count(), 1);
        assert_eq!(text_of(&d, &root), "");
        assert!(d.validate().is_ok());

        // 失败回滚：第二条命令引用不存在的父节点
        let err = d
            .apply(&Command::Batch {
                commands: vec![
                    Command::AddNode {
                        parent: root.clone(),
                        index: None,
                        text: "会被回滚".into(),
                        style: None,
                    },
                    Command::AddNode {
                        parent: "不存在的ID".into(),
                        index: None,
                        text: "x".into(),
                        style: None,
                    },
                ],
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::NotFound(_)));
        assert_eq!(d.node_count(), 1);
        assert!(d.validate().is_ok());
    }

    /// Batch 的事务回滚必须覆盖 `cards` / `groups` / `root_id`：
    /// 只恢复 `nodes` + `meta` 会把半截事务留在文档里（`validate` 判脏）。
    #[test]
    fn failed_batch_rolls_back_cards_groups_and_root() {
        let mut d = concept_doc(&[("主", 0.0, 0.0), ("乙", 100.0, 0.0), ("丙", 200.0, 0.0)]);
        let root = d.root_id.clone();
        // 分组只圈两颗浮动概念：删主概念不会顺带把组削没，回滚缺失才看得见。
        let mut members: Vec<String> = d
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none() && n.id != root)
            .map(|n| n.id.clone())
            .collect();
        members.sort();
        let card = NoteCard::new(d.id.clone(), Some(root.clone()), "卡", "正文", 0);
        let card_id = card.id.clone();
        let err = d
            .apply(&Command::Batch {
                commands: vec![
                    Command::AttachNewCard {
                        node: root.clone(),
                        card,
                    },
                    Command::CreateGroup {
                        members,
                        label: "组".into(),
                    },
                    Command::DeleteNodes {
                        ids: vec![root.clone()],
                    },
                    Command::SetText {
                        id: "不存在的ID".into(),
                        text: "x".into(),
                    },
                ],
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::NotFound(_)));
        assert_eq!(d.root_id, root, "换根必须回滚");
        assert!(d.cards.is_empty(), "新挂的卡片必须回滚");
        assert!(
            !d.nodes
                .values()
                .any(|n| n.card_id.as_ref() == Some(&card_id)),
            "节点上的卡片链接必须回滚"
        );
        assert!(d.groups.is_empty(), "新建的分组必须回滚");
        assert!(d.validate().is_ok());
    }

    /// 含换根命令的 Batch：撤销必须把 `root_id` 一起还回来，重做再换回去。
    ///
    /// `merge` 以前只合并节点/meta/卡片/分组，漏了根快照——撤销后根还指着
    /// 那个已经被撤销掉的概念，`validate` 立刻判脏。
    #[test]
    fn batch_undo_restores_root_when_the_transaction_changes_it() {
        let mut d = concept_doc(&[("主", 0.0, 0.0), ("乙", 100.0, 0.0)]);
        let old_root = d.root_id.clone();
        d.apply(&Command::Batch {
            commands: vec![
                Command::AddFloatingTopic {
                    text: "丙".into(),
                    x: 200.0,
                    y: 0.0,
                },
                Command::DeleteNodes {
                    ids: vec![old_root.clone()],
                },
            ],
        })
        .expect("事务应成功");
        assert_ne!(d.root_id, old_root, "删主概念应换根");

        d.undo().unwrap();
        assert_eq!(d.root_id, old_root, "撤销必须把根换回原来的概念");
        assert!(d.validate().is_ok());

        d.redo().unwrap();
        assert_ne!(d.root_id, old_root, "重做要再换成新根");
        assert!(d.validate().is_ok());
    }

    /// 事务内先建后删的节点不能留在 `after`：重做会把它复活成幽灵。
    #[test]
    fn batch_redo_does_not_resurrect_a_node_created_then_deleted_inside() {
        use crate::model::Position;

        let mut d = concept_doc(&[("主", 0.0, 0.0)]);
        let before_count = d.node_count();
        let tmp = new_id();
        let mut node = Node::new(tmp.clone(), None, "临时");
        node.pos = Some(Position { x: 50.0, y: 50.0 });
        d.apply(&Command::Batch {
            commands: vec![
                Command::InsertConcepts { nodes: vec![node] },
                Command::DeleteNodes { ids: vec![tmp] },
            ],
        })
        .unwrap();
        assert_eq!(d.node_count(), before_count, "事务结束时应恢复原样");
        d.undo().unwrap();
        d.redo().unwrap();
        assert_eq!(
            d.node_count(),
            before_count,
            "重做不能把事务内删掉的节点复活"
        );
        assert!(d.validate().is_ok());
    }

    /// `apply_coalesced` 合并上一条时必须把分组快照也带过来，
    /// 否则重做会丢掉第二次编辑的分组改动。
    #[test]
    fn coalesced_apply_merges_groups() {
        let mut d = concept_doc(&[("主", 0.0, 0.0), ("乙", 100.0, 0.0), ("丙", 200.0, 0.0)]);
        let mut members: Vec<String> = d
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none())
            .map(|n| n.id.clone())
            .collect();
        members.sort();
        let gid = d
            .apply(&Command::CreateGroup {
                members,
                label: "组".into(),
            })
            .and_then(|cs| {
                cs.after_groups
                    .first()
                    .map(|g| g.id.clone())
                    .ok_or_else(|| CoreError::Invalid("没有分组".into()))
            })
            .unwrap();
        d.apply(&Command::SetGroupColor {
            id: gid.clone(),
            color: Some("#FF0000".into()),
        })
        .unwrap();
        d.apply_coalesced(
            &Command::SetGroupLabel {
                id: gid.clone(),
                label: "改名".into(),
            },
            |_| true,
        )
        .unwrap();
        d.undo().unwrap();
        d.redo().unwrap();
        let g = d.groups.iter().find(|g| g.id == gid).expect("分组还在");
        assert_eq!(g.label, "改名", "重做要保住合并进来的标签");
        assert_eq!(g.color.as_deref(), Some("#FF0000"), "颜色也要保住");
    }

    /// 落点必须是有限数：NaN/∞ 一旦进模型会污染布局与所有导出。
    #[test]
    fn position_commands_reject_non_finite_coordinates() {
        let mut d = concept_doc(&[("主", 0.0, 0.0), ("乙", 100.0, 0.0)]);
        let floating = d
            .nodes
            .values()
            .find(|n| n.parent_id.is_none() && n.id != d.root_id)
            .map(|n| n.id.clone())
            .expect("有浮动概念");
        let nan = f32::NAN;
        let inf = f32::INFINITY;
        assert!(matches!(
            d.apply(&Command::MoveConcept {
                id: floating.clone(),
                x: nan,
                y: 0.0,
            }),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            d.apply(&Command::MoveConcept {
                id: floating.clone(),
                x: 0.0,
                y: inf,
            }),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            d.apply(&Command::SetNodePos {
                id: floating.clone(),
                x: nan,
                y: nan,
            }),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            d.apply(&Command::AddFloatingTopic {
                text: "丙".into(),
                x: inf,
                y: 0.0,
            }),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            d.apply(&Command::AddLinkedConcept {
                from: floating.clone(),
                text: "丙".into(),
                x: 0.0,
                y: nan,
            }),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            d.apply(&Command::DetachNode {
                id: floating,
                x: nan,
                y: 0.0,
            }),
            Err(CoreError::Invalid(_))
        ));
        assert!(d.validate().is_ok());
    }

    /// 主题微调的每个颜色槽位都要校验：坏值不能让「设了又没生效」悄悄发生。
    #[test]
    fn theme_overrides_reject_bad_hex_in_every_color_slot() {
        use crate::model::ThemeOverrides;
        let mut d = doc();
        for overrides in [
            ThemeOverrides {
                text: Some("red".into()),
                ..Default::default()
            },
            ThemeOverrides {
                text_central: Some("#12".into()),
                ..Default::default()
            },
            ThemeOverrides {
                text_main: Some("#GGGGGG".into()),
                ..Default::default()
            },
            ThemeOverrides {
                text_sub: Some("".into()),
                ..Default::default()
            },
            ThemeOverrides {
                edge_color: Some("rgb(0,0,0)".into()),
                ..Default::default()
            },
        ] {
            assert!(
                matches!(
                    d.apply(&Command::SetThemeOverrides {
                        overrides: Some(overrides),
                    }),
                    Err(CoreError::Invalid(_))
                ),
                "坏颜色必须被拒绝"
            );
        }
        d.apply(&Command::SetThemeOverrides {
            overrides: Some(ThemeOverrides {
                text: Some("#112233".into()),
                edge_color: Some("#445566".into()),
                ..Default::default()
            }),
        })
        .expect("合法颜色应通过");
    }

    #[test]
    fn style_set_and_clear() {
        let mut d = doc();
        let root = d.root_id.clone();
        let style = StyleOverride {
            fill: Some("#E8F1FF".into()),
            bold: Some(true),
            ..Default::default()
        };
        d.apply(&Command::SetStyle {
            ids: vec![root.clone()],
            style: Some(style),
        })
        .unwrap();
        assert_eq!(
            d.nodes[&root].style.as_ref().unwrap().fill.as_deref(),
            Some("#E8F1FF")
        );
        d.apply(&Command::SetStyle {
            ids: vec![root.clone()],
            style: None,
        })
        .unwrap();
        assert!(d.nodes[&root].style.is_none());
        d.undo().unwrap();
        assert!(d.nodes[&root].style.is_some());
    }

    #[test]
    fn text_length_limit() {
        let mut d = doc();
        let root = d.root_id.clone();
        let long = "字".repeat(MAX_TEXT_LEN + 1);
        let err = d
            .apply(&Command::SetText {
                id: root.clone(),
                text: long,
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::TextTooLong(_)));
    }

    #[test]
    fn collapse_all_keeps_root_expanded() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        add(&mut d, &a, "A1");
        d.apply(&Command::SetCollapsedAll { collapsed: true })
            .unwrap();
        assert!(!d.nodes[&root].collapsed);
        assert!(d.nodes[&a].collapsed);
        d.undo().unwrap();
        assert!(!d.nodes[&a].collapsed);
    }

    #[test]
    fn collapsed_ancestors_are_listed_near_to_far() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let a1 = add(&mut d, &a, "A1");
        let leaf = add(&mut d, &a1, "叶子");
        d.apply(&Command::SetCollapsed {
            id: root.clone(),
            collapsed: true,
        })
        .unwrap();
        d.apply(&Command::SetCollapsed {
            id: a1.clone(),
            collapsed: true,
        })
        .unwrap();
        assert_eq!(d.collapsed_ancestors(&leaf), vec![a1, root]);
    }

    #[test]
    fn collapsed_ancestors_skip_expanded_and_self() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let a1 = add(&mut d, &a, "A1");
        d.apply(&Command::SetCollapsed {
            id: a.clone(),
            collapsed: true,
        })
        .unwrap();
        // 折叠的祖先只挡子孙：从 a 自己往上找只剩展开的根，结果为空。
        assert_eq!(
            d.collapsed_ancestors(&a),
            Vec::<String>::new(),
            "自己不参与"
        );
        assert_eq!(d.collapsed_ancestors(&a1), vec![a]);
    }

    #[test]
    fn collapsed_ancestors_is_empty_for_missing_node() {
        let d = doc();
        assert!(d.collapsed_ancestors("不存在的ID").is_empty());
    }

    #[test]
    fn insert_subtree_paste() {
        let mut d = doc();
        let root = d.root_id.clone();
        let target = add(&mut d, &root, "目标");

        let top = Node::new("t-1".into(), Some(target.clone()), "粘贴根");
        let mut child = Node::new("t-2".into(), Some("t-1".into()), "粘贴子");
        child.note = Some("备注".into());
        let mut top = top;
        top.children = vec!["t-2".into()];

        d.apply(&Command::InsertSubtree {
            parent: target.clone(),
            index: None,
            nodes: vec![top, child],
        })
        .unwrap();
        assert_eq!(d.nodes[&target].children, vec!["t-1".to_string()]);
        assert_eq!(d.nodes["t-2"].parent_id.as_deref(), Some("t-1"));
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert!(d.nodes[&target].children.is_empty());
        assert!(!d.nodes.contains_key("t-1"));
        assert!(d.validate().is_ok());
    }

    #[test]
    fn insert_subtree_multiple_tops() {
        let mut d = doc();
        let root = d.root_id.clone();
        let mut a = Node::new("m-1".into(), Some(root.clone()), "甲");
        let b = Node::new("m-2".into(), Some(root.clone()), "乙");
        let c = Node::new("m-3".into(), Some("m-1".into()), "甲一");
        a.children = vec!["m-3".into()];

        d.apply(&Command::InsertSubtree {
            parent: root.clone(),
            index: None,
            nodes: vec![a, b, c],
        })
        .unwrap();
        assert_eq!(d.nodes[&root].children.len(), 2);
        assert_eq!(d.nodes["m-1"].children, vec!["m-3".to_string()]);
        assert_eq!(d.node_count(), 4);
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert_eq!(d.node_count(), 1);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn insert_subtree_requires_top() {
        let mut d = doc();
        let root = d.root_id.clone();
        // 所有节点都挂在彼此之下、没有直接挂在 root 上的顶层节点
        let a = Node::new("n-1".into(), Some("n-2".into()), "甲");
        let b = Node::new("n-2".into(), Some("n-1".into()), "乙");
        let err = d
            .apply(&Command::InsertSubtree {
                parent: root.clone(),
                index: None,
                nodes: vec![a, b],
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::Invalid(_)));
        assert_eq!(d.node_count(), 1);
    }

    #[test]
    fn set_icon_and_undo() {
        let mut d = doc();
        let root = d.root_id.clone();
        d.apply(&Command::SetIcon {
            ids: vec![root.clone()],
            icon: Some("priority-1".into()),
        })
        .unwrap();
        assert_eq!(d.nodes[&root].icon.as_deref(), Some("priority-1"));
        d.apply(&Command::SetIcon {
            ids: vec![root.clone()],
            icon: None,
        })
        .unwrap();
        assert_eq!(d.nodes[&root].icon, None);
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].icon.as_deref(), Some("priority-1"));
    }

    #[test]
    fn search_text_and_pinyin() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "项目启动");
        assert!(d.search("项目").iter().any(|h| h.id == a));
        assert!(d.search("xm").iter().any(|h| h.id == a));
        assert!(d.search("xiangmu").iter().any(|h| h.id == a));
        assert!(d.search("不存在的词").is_empty());
    }

    /// 文档 07 §7 精度要求：跨音节片段不命中、多音字非主音可命中、
    /// 命中区间映射回原文字符下标、同分按文档顺序稳定排序。
    #[test]
    fn search_pinyin_precision_and_order() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "项目启动");
        let b = add(&mut d, &root, "重复任务");
        // "an" 藏在 xiang 的音节内部，不应假命中
        assert!(d.search("an").is_empty());
        // 「重」的非主音 chong 亦应命中（全拼与首字母两条路）
        assert!(d.search("chongfurenwu").iter().any(|h| h.id == b));
        assert!(d.search("cfrw").iter().any(|h| h.id == b));
        // 高亮区间：拼音命中映射回原文第 2..4 字（「启动」）
        let hit = d.search("qidong").into_iter().find(|h| h.id == a).unwrap();
        assert_eq!(hit.range, Some([2, 4]));
        // 同分（均以 x 前缀命中）时按文档顺序，而非 UUID 顺序
        let x = add(&mut d, &root, "西瓜");
        let y = add(&mut d, &root, "香蕉");
        let hits = d.search("x");
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec![a.as_str(), x.as_str(), y.as_str()]);
    }

    #[test]
    fn root_is_protected() {
        let mut d = doc();
        let root = d.root_id.clone();
        assert!(matches!(
            d.apply(&Command::DeleteNodes {
                ids: vec![root.clone()]
            })
            .unwrap_err(),
            CoreError::RootProtected
        ));
    }

    #[test]
    fn long_command_sequence_stays_valid() {
        let mut d = doc();
        let root = d.root_id.clone();
        let mut ids = vec![root.clone()];
        for i in 0..60 {
            let parent = ids[i % ids.len()].clone();
            let id = add(&mut d, &parent, &format!("节点{i}"));
            ids.push(id);
        }
        for i in 0..20 {
            let _ = i;
            let _ = d.undo();
        }
        for _ in 0..20 {
            let _ = d.redo();
        }
        assert!(d.validate().is_ok());
        assert_eq!(d.node_count(), 61);
    }

    /// CM-10 ① 的姊妹手势：「把线改指到空白处的新概念」也必须是**一步撤销**。
    #[test]
    fn retarget_link_to_new_concept_undoes_in_one_step() {
        // 同上：夹具要概念图（命令自带概念图护栏，C14）。
        let mut d = concept_doc(&[("根", 0.0, 0.0)]);
        let root = d.root_id.clone();
        d.apply(&Command::AddLinkedConcept {
            from: root.clone(),
            text: "甲".into(),
            x: 200.0,
            y: 0.0,
        })
        .expect("先有一条线");
        let a = d
            .doc_order()
            .into_iter()
            .find(|id| id != &root)
            .expect("甲");
        let rid = d.nodes[&root].relationships[0].id.clone();
        // 短语是语义，改指时**只该换目标**：短语与线色都得留着。
        d.apply(&Command::SetRelationshipLabel {
            from: root.clone(),
            id: rid.clone(),
            label: Some("支撑".into()),
        })
        .expect("打短语");
        d.apply(&Command::SetRelationshipColor {
            from: root.clone(),
            id: rid.clone(),
            color: Some("#ff8800".into()),
        })
        .expect("线色");
        d.apply(&Command::RetargetLinkToNewConcept {
            from: root.clone(),
            id: rid.clone(),
            text: "乙".into(),
            x: 400.0,
            y: 0.0,
        })
        .expect("改线到新概念");
        assert_eq!(d.nodes.len(), 3, "新概念没建出来");
        let b = d
            .doc_order()
            .into_iter()
            .find(|id| id != &root && id != &a)
            .expect("乙");
        assert_eq!(d.nodes[&root].relationships[0].to, b, "线没改指到新概念");
        // 改指只换目标：短语与线色不能顺手丢。
        let r = &d.nodes[&root].relationships[0];
        assert_eq!(r.label.as_deref(), Some("支撑"), "改指把短语弄丢了");
        assert_eq!(r.color.as_deref(), Some("#ff8800"), "改指把线色弄丢了");
        // 一次撤销：新概念与「改指」一起回去。
        d.undo().expect("可撤销");
        assert_eq!(d.nodes.len(), 2, "撤销后新概念没删");
        assert_eq!(d.nodes[&root].relationships[0].to, a, "撤销后线没还原");
    }

    /// CM-10 ⑦：端点箭头改一条撤销记录；默认是 `Forward`，也就是今天的样子。
    #[test]
    fn the_arrow_head_can_be_changed_and_undone() {
        let mut d = Document::new("根");
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        d.apply(&Command::AddRelationship {
            from: root.clone(),
            to: a.clone(),
            label: None,
        })
        .expect("加关系线");
        let rid = d.nodes[&root].relationships[0].id.clone();
        assert_eq!(
            d.nodes[&root].relationships[0].head,
            ArrowHead::Forward,
            "没设置时就是前向箭头（= 旧行为）"
        );
        d.apply(&Command::SetRelationshipHead {
            from: root.clone(),
            id: rid.clone(),
            head: ArrowHead::Both,
        })
        .expect("改方向");
        assert_eq!(d.nodes[&root].relationships[0].head, ArrowHead::Both);
        d.undo().expect("可撤销");
        assert_eq!(
            d.nodes[&root].relationships[0].head,
            ArrowHead::Forward,
            "撤销后回到前向"
        );
        d.redo().expect("可重做");
        assert_eq!(d.nodes[&root].relationships[0].head, ArrowHead::Both);
    }
    /// CM-10 ①：「概念 + 边」必须是**一步撤销**——只撤边会留下孤儿概念。
    #[test]
    fn add_linked_concept_undoes_in_one_step() {
        // 夹具必须是概念图：`AddLinkedConcept` 自己带「只有概念图可以新建概念并连线」
        // 的护栏（C14），它在思维导图里本来就不该成功。
        let mut d = concept_doc(&[("根", 0.0, 0.0)]);
        let root = d.root_id.clone();
        d.apply(&Command::AddLinkedConcept {
            from: root.clone(),
            text: "新概念".into(),
            x: 240.0,
            y: 0.0,
        })
        .expect("建概念并连线");
        assert_eq!(d.nodes.len(), 2, "多了一个概念");
        // 落点语义：`pos` 原样存下，而 `scene` 把 `pos` 当盒子**左上角**
        // （`floating_topic_honours_its_stored_position`）。两条合起来 = 松手点就是新框左上角，
        // 也就是画布上那个占位框画的位置。
        let new_id = d
            .doc_order()
            .into_iter()
            .find(|id| id != &root)
            .expect("新概念");
        let p = d.nodes[&new_id].pos.as_ref().expect("有落点");
        assert_eq!((p.x, p.y), (240.0, 0.0), "落点被改写了");
        let rels = d.nodes[&root].relationships.len();
        assert_eq!(rels, 1, "边没建上");
        // 一次撤销：概念与边一起消失。
        d.undo().expect("可撤销");
        assert_eq!(d.nodes.len(), 1, "撤销后概念没删干净");
        assert_eq!(d.nodes[&root].relationships.len(), 0, "撤销后边没删干净");
        // 重做也一样：一步回来。
        d.redo().expect("可重做");
        assert_eq!(d.nodes.len(), 2);
        assert_eq!(d.nodes[&root].relationships.len(), 1);
    }
    #[test]
    fn relationship_color_roundtrip_and_undo() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b,
            label: None,
        })
        .unwrap();
        let rid = d.nodes[&a].relationships[0].id.clone();
        d.apply(&Command::SetRelationshipColor {
            from: a.clone(),
            id: rid.clone(),
            color: Some("#ff0000".into()),
        })
        .unwrap();
        assert_eq!(
            d.nodes[&a].relationships[0].color.as_deref(),
            Some("#ff0000")
        );
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships[0].color, None);
        d.redo().unwrap();
        assert_eq!(
            d.nodes[&a].relationships[0].color.as_deref(),
            Some("#ff0000")
        );
        assert!(d
            .apply(&Command::SetRelationshipColor {
                from: a,
                id: rid,
                color: Some("not-a-color".into()),
            })
            .is_err());
    }

    #[test]
    fn relationship_target_can_be_rewired_and_undone() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        let c = add(&mut d, &root, "C");
        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b.clone(),
            label: None,
        })
        .unwrap();
        let rid = d.nodes[&a].relationships[0].id.clone();
        d.apply(&Command::SetRelationshipTarget {
            from: a.clone(),
            id: rid.clone(),
            to: c.clone(),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].relationships[0].to, c);
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships[0].to, b, "撤销要回原目标");
        // 自环 / 未知目标 / 重复目标都拒绝
        assert!(d
            .apply(&Command::SetRelationshipTarget {
                from: a.clone(),
                id: rid.clone(),
                to: a.clone(),
            })
            .is_err());
        assert!(d
            .apply(&Command::SetRelationshipTarget {
                from: a.clone(),
                id: rid.clone(),
                to: "missing".into(),
            })
            .is_err());
        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: c.clone(),
            label: None,
        })
        .unwrap();
        assert!(d
            .apply(&Command::SetRelationshipTarget {
                from: a.clone(),
                id: rid,
                to: c,
            })
            .is_err());
    }

    #[test]
    fn reverse_relationship_swaps_ends_keeps_head_and_undoes_in_one_step() {
        use crate::model::ArrowHead;
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b.clone(),
            label: Some("导致".into()),
        })
        .unwrap();
        let rid = d.nodes[&a].relationships[0].id.clone();
        // 先把箭头设成非默认档：反向之后要**原样保留**，否则「反向」会顺手改掉用户设的方向。
        d.apply(&Command::SetRelationshipHead {
            from: a.clone(),
            id: rid.clone(),
            head: ArrowHead::Both,
        })
        .unwrap();
        // 线型与注释也要跟着命题走：反的是方向，不是「这是不是暂定命题」。
        d.apply(&Command::SetRelationshipDashed {
            from: a.clone(),
            id: rid.clone(),
            dashed: true,
        })
        .unwrap();
        d.apply(&Command::SetRelationshipNote {
            from: a.clone(),
            id: rid.clone(),
            note: Some("暂定".into()),
        })
        .unwrap();

        d.apply(&Command::ReverseRelationship {
            from: a.clone(),
            id: rid.clone(),
        })
        .unwrap();
        assert!(d.nodes[&a].relationships.is_empty(), "源端不再挂着这条边");
        assert_eq!(d.nodes[&b].relationships.len(), 1);
        let r = &d.nodes[&b].relationships[0];
        assert_eq!(r.to, a);
        assert_eq!(r.label.as_deref(), Some("导致"), "短语要跟着命题走");
        assert_eq!(
            r.head,
            ArrowHead::Both,
            "箭头档位不镜像：`arrow` 恒指 `to` 端，两端对调后箭头自然落到另一颗上"
        );
        assert_eq!(r.id, rid, "边 id 不变，选中态与检查器才接得上");
        assert!(r.dashed, "虚线跟着命题走");
        assert_eq!(r.note.as_deref(), Some("暂定"), "注释跟着命题走");
        assert!(d.validate().is_ok());

        // 一条撤销把两端一起还回来
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships.len(), 1);
        assert_eq!(d.nodes[&a].relationships[0].to, b);
        assert_eq!(d.nodes[&a].relationships[0].head, ArrowHead::Both);
        assert!(d.nodes[&b].relationships.is_empty());
        assert!(d.validate().is_ok());
        d.redo().unwrap();
        assert_eq!(d.nodes[&b].relationships[0].to, a);

        // 未知边 / 未知节点都拒绝，不留半条边
        assert!(d
            .apply(&Command::ReverseRelationship {
                from: a.clone(),
                id: "nope".into()
            })
            .is_err());
        assert!(d
            .apply(&Command::ReverseRelationship {
                from: "nope".into(),
                id: rid.clone()
            })
            .is_err());
    }

    #[test]
    fn relationship_line_style_and_note_are_undoable_and_normalised() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b.clone(),
            label: Some("导致".into()),
        })
        .unwrap();
        let rid = d.nodes[&a].relationships[0].id.clone();

        d.apply(&Command::SetRelationshipDashed {
            from: a.clone(),
            id: rid.clone(),
            dashed: true,
        })
        .unwrap();
        assert!(d.nodes[&a].relationships[0].dashed);

        d.apply(&Command::SetRelationshipWidth {
            from: a.clone(),
            id: rid.clone(),
            width: Some(2.5),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].relationships[0].width, Some(2.5));

        d.apply(&Command::SetRelationshipNote {
            from: a.clone(),
            id: rid.clone(),
            note: Some("  暂定  ".into()),
        })
        .unwrap();
        assert_eq!(
            d.nodes[&a].relationships[0].note.as_deref(),
            Some("暂定"),
            "前后空白要修掉"
        );

        // 空注释归一成 `None`：别把「空串」写进文件（与 `SetNote` 同口径）。
        d.apply(&Command::SetRelationshipNote {
            from: a.clone(),
            id: rid.clone(),
            note: Some("   ".into()),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].relationships[0].note, None);

        // 越界 / NaN 的线宽拒绝，不留半条脏线。
        assert!(d
            .apply(&Command::SetRelationshipWidth {
                from: a.clone(),
                id: rid.clone(),
                width: Some(9.0),
            })
            .is_err());
        assert!(d
            .apply(&Command::SetRelationshipWidth {
                from: a.clone(),
                id: rid.clone(),
                width: Some(f32::NAN),
            })
            .is_err());

        // 四条命令各占一条撤销记录：退到线宽之前，虚线仍在。
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships[0].note.as_deref(), Some("暂定"));
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships[0].note, None);
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships[0].width, None);
        assert!(d.nodes[&a].relationships[0].dashed);
        d.undo().unwrap();
        assert!(!d.nodes[&a].relationships[0].dashed);
        assert!(d.validate().is_ok());
    }

    /// 造一张概念图：主概念 + `n` 个浮动概念，返回它们的 id。
    fn group_test_doc(n: usize) -> (Document, Vec<String>) {
        let mut d = Document::new("分组测试");
        let _ = d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        });
        let mut ids = Vec::new();
        for i in 0..n {
            let cs = d
                .apply(&Command::AddFloatingTopic {
                    text: format!("C{i}"),
                    x: 100.0 * i as f32,
                    y: 0.0,
                })
                .unwrap();
            let before: HashSet<&str> = cs.before.iter().map(|n| n.id.as_str()).collect();
            let id = cs
                .after
                .iter()
                .find(|n| !before.contains(n.id.as_str()))
                .expect("新增概念")
                .id
                .clone();
            ids.push(id);
        }
        (d, ids)
    }

    #[test]
    fn groups_wrap_members_and_ungroup_keeps_the_concepts() {
        let (mut d, ids) = group_test_doc(3);
        let nodes_before = d.nodes.len();
        d.apply(&Command::CreateGroup {
            members: vec![ids[0].clone(), ids[1].clone()],
            label: "  ".into(),
        })
        .unwrap();
        assert_eq!(d.groups.len(), 1);
        assert_eq!(d.groups[0].label, "分组", "空标签回落到「分组」");
        assert!(d.validate().is_ok());

        // 一个概念只属于一个组：把 b 再分到新组，旧组摘掉 b 之后只剩一个成员 → 整组消失。
        d.apply(&Command::CreateGroup {
            members: vec![ids[1].clone(), ids[2].clone()],
            label: "G2".into(),
        })
        .unwrap();
        assert_eq!(d.groups.len(), 1);
        assert_eq!(d.groups[0].members, vec![ids[1].clone(), ids[2].clone()]);

        // 解组：只删框，概念一个不删。
        let gid = d.groups[0].id.clone();
        d.apply(&Command::DeleteGroup { id: gid }).unwrap();
        assert!(d.groups.is_empty());
        assert_eq!(d.nodes.len(), nodes_before);

        // 一次撤销把组还回来。
        d.undo().unwrap();
        assert_eq!(d.groups.len(), 1);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn deleting_a_concept_prunes_groups_and_drops_the_ones_left_too_small() {
        let (mut d, ids) = group_test_doc(2);
        d.apply(&Command::CreateGroup {
            members: ids.clone(),
            label: "G".into(),
        })
        .unwrap();
        assert_eq!(d.groups.len(), 1);

        d.apply(&Command::DeleteNodes {
            ids: vec![ids[1].clone()],
        })
        .unwrap();
        assert!(d.groups.is_empty(), "只剩一个成员的组要整组消失");
        assert!(d.validate().is_ok());

        d.undo().unwrap();
        assert_eq!(d.groups.len(), 1, "撤销要把组一起还回来");
        assert_eq!(d.groups[0].members.len(), 2);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn moving_a_group_moves_every_member_in_one_step() {
        let (mut d, ids) = group_test_doc(2);
        d.apply(&Command::CreateGroup {
            members: ids.clone(),
            label: "G".into(),
        })
        .unwrap();
        let gid = d.groups[0].id.clone();
        let before: Vec<(f32, f32)> = ids
            .iter()
            .map(|id| {
                let p = d.nodes[id].pos.as_ref().expect("概念要有坐标");
                (p.x, p.y)
            })
            .collect();
        let depth = d.undo_depth();
        d.apply(&Command::MoveGroup {
            id: gid,
            dx: 30.0,
            dy: -20.0,
        })
        .unwrap();
        assert_eq!(d.undo_depth(), depth + 1, "整组平移只占一条撤销");
        for (id, (x, y)) in ids.iter().zip(before.iter()) {
            let p = d.nodes[id].pos.as_ref().expect("概念要有坐标");
            assert!((p.x - (x + 30.0)).abs() < 1e-3);
            assert!((p.y - (y - 20.0)).abs() < 1e-3);
        }
        d.undo().unwrap();
        for (id, (x, y)) in ids.iter().zip(before.iter()) {
            let p = d.nodes[id].pos.as_ref().expect("概念要有坐标");
            assert!((p.x - x).abs() < 1e-3);
            assert!((p.y - y).abs() < 1e-3);
        }
        assert!(d.validate().is_ok());
    }

    /// BUG-8：`CreateGroup` 不许收「挪不动」的成员。
    ///
    /// `MoveGroup` 只会平移 `parent_id.is_none() && pos.is_some()` 的概念；组里混进一颗
    /// 没坐标的成员（概念图的主概念在第一次拖动前就是这种状态）时，整组拖动只动一部分，
    /// 以前没有任何提示。现在创建时就拒掉，命令不留半截状态。
    #[test]
    fn create_group_rejects_members_it_cannot_move() {
        let mut d = doc();
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = d.root_id.clone();
        assert!(d.nodes[&root].pos.is_none(), "前提：主概念还没摆过位");
        d.apply(&Command::AddFloatingTopic {
            text: "乙".into(),
            x: 120.0,
            y: 0.0,
        })
        .unwrap();
        let float = d
            .nodes
            .values()
            .find(|n| n.parent_id.is_none() && n.id != root)
            .expect("浮动概念")
            .id
            .clone();
        let before_groups = d.groups.clone();
        let err = d
            .apply(&Command::CreateGroup {
                members: vec![root.clone(), float.clone()],
                label: "G".into(),
            })
            .unwrap_err();
        assert!(
            matches!(err, CoreError::Invalid(_)),
            "应该是 Invalid，实际 {err:?}"
        );
        assert_eq!(d.groups, before_groups, "被拒的命令不许动文档");
        assert!(d.validate().is_ok(), "被拒后文档仍是干净的");
    }

    /// BUG-9：读盘路径必须挡住 `inf` / `NaN` 坐标与越界线宽。
    ///
    /// 命令层（`check_pos`、`SetRelationshipWidth`）都查，但手工改坏的 `.zmind`
    /// 绕过命令直接进 `validate`：一颗 `inf` 坐标会把 Scene 的包围盒与命中网格一起毁掉。
    #[test]
    fn validate_rejects_non_finite_positions_and_out_of_range_widths() {
        let mut d = concept_doc(&[("甲", 0.0, 0.0), ("乙", 120.0, 0.0)]);
        let float = d
            .nodes
            .values()
            .find(|n| n.parent_id.is_none() && n.id != d.root_id)
            .expect("浮动概念")
            .id
            .clone();
        assert!(d.validate().is_ok(), "前提：正常文档校验通过");

        d.nodes.get_mut(&float).unwrap().pos = Some(Position {
            x: f32::INFINITY,
            y: 0.0,
        });
        assert!(d.validate().is_err(), "inf 坐标必须判脏");
        d.nodes.get_mut(&float).unwrap().pos = Some(Position { x: 0.0, y: 0.0 });

        let root = d.root_id.clone();
        d.apply(&Command::AddRelationship {
            from: root.clone(),
            to: float.clone(),
            label: None,
        })
        .unwrap();
        d.nodes.get_mut(&root).unwrap().relationships[0].width = Some(9.0);
        assert!(d.validate().is_err(), "线宽 9.0 越界必须判脏");
        d.nodes.get_mut(&root).unwrap().relationships[0].width = Some(f32::NAN);
        assert!(d.validate().is_err(), "NaN 线宽必须判脏");
        d.nodes.get_mut(&root).unwrap().relationships[0].width = Some(2.0);
        let n_before = d.nodes.len();
        assert!(d.validate().is_ok(), "改回合法宽度应恢复通过");
        assert_eq!(d.nodes.len(), n_before);
    }

    #[test]
    fn groups_are_concept_map_only() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        assert!(d
            .apply(&Command::CreateGroup {
                members: vec![a, b],
                label: "G".into(),
            })
            .is_err());
        assert!(d.groups.is_empty());
    }

    #[test]
    fn batching_relationship_style_is_one_undo() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        for i in 0..3 {
            let t = add(&mut d, &root, &format!("T{i}"));
            d.apply(&Command::AddRelationship {
                from: a.clone(),
                to: t,
                label: Some(format!("谓词{i}")),
            })
            .unwrap();
        }
        let ids: Vec<String> = d.nodes[&a]
            .relationships
            .iter()
            .map(|r| r.id.clone())
            .collect();
        assert_eq!(ids.len(), 3);

        let depth = d.undo_depth();
        d.apply(&Command::Batch {
            commands: ids
                .iter()
                .map(|id| Command::SetRelationshipDashed {
                    from: a.clone(),
                    id: id.clone(),
                    dashed: true,
                })
                .collect(),
        })
        .unwrap();
        assert!(d.nodes[&a].relationships.iter().all(|r| r.dashed));
        assert_eq!(d.undo_depth(), depth + 1, "批量改线型只占一条撤销");

        d.undo().unwrap();
        assert!(
            d.nodes[&a].relationships.iter().all(|r| !r.dashed),
            "一次撤销要把三条一起退回去"
        );
        assert!(d.validate().is_ok());
    }

    #[test]
    fn reverse_relationship_keeps_parallel_propositions_in_a_concept_map() {
        let mut d = doc();
        let _ = d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        });
        // 概念图的概念是浮动节点：`AddNode` 在这里一律被拒，得走 `AddFloatingTopic`。
        let add_concept = |d: &mut Document, text: &str, x: f32| -> String {
            let cs = d
                .apply(&Command::AddFloatingTopic {
                    text: text.to_string(),
                    x,
                    y: 0.0,
                })
                .unwrap();
            let before_ids: HashSet<&str> = cs.before.iter().map(|n| n.id.as_str()).collect();
            cs.after
                .iter()
                .find(|n| !before_ids.contains(n.id.as_str()))
                .expect("新增概念应出现在 after 中")
                .id
                .clone()
        };
        let a = add_concept(&mut d, "A", 0.0);
        let b = add_concept(&mut d, "B", 200.0);
        // 概念图允许 A→B 与 B→A 同时存在（两条不同命题）。
        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b.clone(),
            label: Some("导致".into()),
        })
        .unwrap();
        d.apply(&Command::AddRelationship {
            from: b.clone(),
            to: a.clone(),
            label: Some("支撑".into()),
        })
        .unwrap();
        let rid = d.nodes[&a].relationships[0].id.clone();
        // 反向后 A 下没有边、B 下有两条（一条原有、一条反过来的）。
        d.apply(&Command::ReverseRelationship {
            from: a.clone(),
            id: rid.clone(),
        })
        .unwrap();
        assert!(d.nodes[&a].relationships.is_empty());
        assert_eq!(d.nodes[&b].relationships.len(), 2);
        assert!(d.nodes[&b]
            .relationships
            .iter()
            .any(|r| r.to == a && r.label.as_deref() == Some("导致")));
    }

    #[test]
    fn relationship_add_delete_cleanup_and_undo() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b.clone(),
            label: Some(" 影响 ".into()),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].relationships.len(), 1);
        assert_eq!(d.nodes[&a].relationships[0].to, b);
        assert_eq!(d.nodes[&a].relationships[0].label.as_deref(), Some("影响"));

        // 重复添加 / 自环被拒绝
        assert!(d
            .apply(&Command::AddRelationship {
                from: a.clone(),
                to: b.clone(),
                label: None
            })
            .is_err());
        assert!(d
            .apply(&Command::AddRelationship {
                from: a.clone(),
                to: a.clone(),
                label: None
            })
            .is_err());

        // 修改标签
        let rid = d.nodes[&a].relationships[0].id.clone();
        d.apply(&Command::SetRelationshipLabel {
            from: a.clone(),
            id: rid.clone(),
            label: Some("阻塞".into()),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].relationships[0].label.as_deref(), Some("阻塞"));

        // 删除 B → A 的关系线被清理；撤销两步全部恢复
        d.apply(&Command::DeleteNodes {
            ids: vec![b.clone()],
        })
        .unwrap();
        assert!(d.nodes[&a].relationships.is_empty());
        assert!(d.validate().is_ok());
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships.len(), 1);
        assert!(d.nodes.contains_key(&b));
        assert!(d.validate().is_ok());
        // 再撤销一步：标签回到「影响」；再一步：关系线整体消失
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships[0].label.as_deref(), Some("影响"));
        d.undo().unwrap();
        assert!(d.nodes[&a].relationships.is_empty());
        assert!(d.validate().is_ok());
        // 重做两步（恢复关系线与标签）后，删除关系线命令本身可撤销
        d.redo().unwrap();
        d.redo().unwrap();
        assert_eq!(d.nodes[&a].relationships.len(), 1);
        d.apply(&Command::DeleteRelationship {
            from: a.clone(),
            id: rid,
        })
        .unwrap();
        assert!(d.nodes[&a].relationships.is_empty());
        d.undo().unwrap();
        assert_eq!(d.nodes[&a].relationships.len(), 1);
    }

    #[test]
    fn boundary_follows_structure_changes() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let b = add(&mut d, &root, "B");
        let c = add(&mut d, &root, "C");
        d.apply(&Command::AddBoundary {
            parent: root.clone(),
            targets: vec![a.clone(), b.clone()],
            color: None,
        })
        .unwrap();
        assert_eq!(d.nodes[&root].boundaries.len(), 1);
        let bid = d.nodes[&root].boundaries[0].id.clone();

        // 目标必须是直接子节点
        let a1 = add(&mut d, &a, "A1");
        assert!(d
            .apply(&Command::AddBoundary {
                parent: root.clone(),
                targets: vec![a1],
                color: None
            })
            .is_err());

        // 删除 B → 边界目标收敛为 [A]
        d.apply(&Command::DeleteNodes { ids: vec![b] }).unwrap();
        assert_eq!(d.nodes[&root].boundaries[0].targets, vec![a.clone()]);
        // 把 A 移走 → 边界目标为空 → 整个边界被移除
        d.apply(&Command::MoveNode {
            id: a.clone(),
            new_parent: c.clone(),
            index: None,
        })
        .unwrap();
        assert!(d.nodes[&root].boundaries.is_empty());
        assert!(d.validate().is_ok());

        // 撤销两步：恢复 A 的挂载与边界
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].boundaries[0].targets, vec![a.clone()]);
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].boundaries.len(), 1);
        assert!(d.validate().is_ok());

        // 显式删除边界可撤销
        let bid_now = d.nodes[&root].boundaries[0].id.clone();
        let _ = bid;
        d.apply(&Command::DeleteBoundary {
            parent: root.clone(),
            id: bid_now,
        })
        .unwrap();
        assert!(d.nodes[&root].boundaries.is_empty());
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].boundaries.len(), 1);
    }

    #[test]
    fn boundary_span_shares_the_same_validation() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        let b = add(&mut d, &root, "乙");
        d.apply(&Command::AddBoundary {
            parent: root.clone(),
            targets: vec![a.clone()],
            color: None,
        })
        .unwrap();
        let bid = d.nodes[&root].boundaries[0].id.clone();
        d.apply(&Command::SetBoundaryTargets {
            parent: root.clone(),
            id: bid.clone(),
            targets: vec![a.clone(), b.clone()],
        })
        .unwrap();
        assert_eq!(d.nodes[&root].boundaries[0].targets, vec![a, b.clone()]);
        assert!(
            d.apply(&Command::SetBoundaryTargets {
                parent: root.clone(),
                id: bid.clone(),
                targets: Vec::new(),
            })
            .is_err(),
            "空目标要拒绝"
        );
        let deep = add(&mut d, &b, "乙的叶子");
        assert!(
            d.apply(&Command::SetBoundaryTargets {
                parent: root.clone(),
                id: bid,
                targets: vec![deep],
            })
            .is_err(),
            "非直接子节点要拒绝"
        );
    }

    #[test]
    fn summary_span_can_be_widened_and_narrowed_but_never_emptied() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        let b = add(&mut d, &root, "乙");
        let c = add(&mut d, &root, "丙");
        d.apply(&Command::AddSummary {
            parent: root.clone(),
            targets: vec![a.clone()],
            text: "结论".into(),
        })
        .unwrap();
        let sid = d.nodes[&root].summaries[0].id.clone();

        d.apply(&Command::SetSummaryTargets {
            parent: root.clone(),
            id: sid.clone(),
            targets: vec![a.clone(), b.clone(), c.clone()],
        })
        .unwrap();
        assert_eq!(
            d.nodes[&root].summaries[0].targets,
            vec![a.clone(), b.clone(), c]
        );

        // 非直接子节点：拒绝
        let deep = add(&mut d, &a, "甲的叶子");
        assert!(d
            .apply(&Command::SetSummaryTargets {
                parent: root.clone(),
                id: sid.clone(),
                targets: vec![deep],
            })
            .is_err());
        // 空目标：拒绝
        assert!(d
            .apply(&Command::SetSummaryTargets {
                parent: root.clone(),
                id: sid.clone(),
                targets: Vec::new(),
            })
            .is_err());
        // 未知概要 id：拒绝
        assert!(d
            .apply(&Command::SetSummaryTargets {
                parent: root.clone(),
                id: "nope".into(),
                targets: vec![a.clone()],
            })
            .is_err());

        // 窄回去，并确认可撤销
        d.apply(&Command::SetSummaryTargets {
            parent: root.clone(),
            id: sid.clone(),
            targets: vec![a],
        })
        .unwrap();
        assert_eq!(d.nodes[&root].summaries[0].targets.len(), 1);
        d.undo().unwrap();
        assert_eq!(
            d.nodes[&root].summaries[0].targets.len(),
            3,
            "撤销回到三步之前的跨度"
        );
    }

    #[test]
    fn deleting_two_annotations_in_one_batch_is_one_undo() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        let b = add(&mut d, &root, "乙");
        d.apply(&Command::AddSummary {
            parent: root.clone(),
            targets: vec![a],
            text: "结论".into(),
        })
        .unwrap();
        d.apply(&Command::AddBoundary {
            parent: root.clone(),
            targets: vec![b],
            color: None,
        })
        .unwrap();
        let sid = d.nodes[&root].summaries[0].id.clone();
        let bid = d.nodes[&root].boundaries[0].id.clone();

        d.apply(&Command::Batch {
            commands: vec![
                Command::DeleteSummary {
                    parent: root.clone(),
                    id: sid,
                },
                Command::DeleteBoundary {
                    parent: root.clone(),
                    id: bid,
                },
            ],
        })
        .unwrap();
        assert!(d.nodes[&root].summaries.is_empty());
        assert!(d.nodes[&root].boundaries.is_empty());
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].summaries.len(), 1, "一次撤销两条都要回来");
        assert_eq!(d.nodes[&root].boundaries.len(), 1);
    }

    #[test]
    fn boundary_colour_is_settable_validated_and_undoable() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        d.apply(&Command::AddBoundary {
            parent: root.clone(),
            targets: vec![a],
            color: None,
        })
        .unwrap();
        let bid = d.nodes[&root].boundaries[0].id.clone();
        d.apply(&Command::SetBoundaryColor {
            parent: root.clone(),
            id: bid.clone(),
            color: Some("#318de1".into()),
        })
        .unwrap();
        assert_eq!(
            d.nodes[&root].boundaries[0].color.as_deref(),
            Some("#318de1")
        );
        // 坏颜色拒绝，且不改动已有值
        assert!(d
            .apply(&Command::SetBoundaryColor {
                parent: root.clone(),
                id: bid.clone(),
                color: Some("tomato".into()),
            })
            .is_err());
        assert_eq!(
            d.nodes[&root].boundaries[0].color.as_deref(),
            Some("#318de1")
        );
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].boundaries[0].color, None, "撤销回到未设置");
        // 不存在的边界 id 要报错而不是静默成功
        assert!(d
            .apply(&Command::SetBoundaryColor {
                parent: root.clone(),
                id: "nope".into(),
                color: None,
            })
            .is_err());
    }

    #[test]
    fn summary_add_set_text_delete() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        let b = add(&mut d, &root, "乙");
        d.apply(&Command::AddSummary {
            parent: root.clone(),
            targets: vec![a, b],
            text: "两者都重要".into(),
        })
        .unwrap();
        assert_eq!(d.nodes[&root].summaries.len(), 1);
        let sid = d.nodes[&root].summaries[0].id.clone();
        d.apply(&Command::SetSummaryText {
            parent: root.clone(),
            id: sid.clone(),
            text: "结论".into(),
        })
        .unwrap();
        assert_eq!(d.nodes[&root].summaries[0].text, "结论");
        d.apply(&Command::DeleteSummary {
            parent: root.clone(),
            id: sid,
        })
        .unwrap();
        assert!(d.nodes[&root].summaries.is_empty());
        d.undo().unwrap();
        assert_eq!(d.nodes[&root].summaries[0].text, "结论");
        assert!(d.validate().is_ok());
    }

    #[test]
    fn floating_topic_lifecycle() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "树内节点");
        let cs = d
            .apply(&Command::AddFloatingTopic {
                text: "便签".into(),
                x: 100.0,
                y: 50.0,
            })
            .unwrap();
        let fid = cs.after[0].id.clone();
        assert_eq!(d.nodes[&fid].parent_id, None);
        assert_eq!(d.nodes[&fid].pos.as_ref().unwrap().x, 100.0);
        assert!(d.validate().is_ok());

        // 自由主题可作为父级容纳树内节点（整体拖入）
        d.apply(&Command::MoveNode {
            id: a.clone(),
            new_parent: fid.clone(),
            index: None,
        })
        .unwrap();
        assert!(d.validate().is_ok());

        // 自由主题整体挂回主树：pos 清除
        d.apply(&Command::MoveNode {
            id: fid.clone(),
            new_parent: root.clone(),
            index: None,
        })
        .unwrap();
        assert!(d.nodes[&fid].pos.is_none());
        assert!(d.validate().is_ok());

        // 树内节点分离为自由主题（含子树）
        d.apply(&Command::DetachNode {
            id: fid.clone(),
            x: -40.0,
            y: -20.0,
        })
        .unwrap();
        assert_eq!(d.nodes[&fid].pos.as_ref().unwrap().x, -40.0);
        assert_eq!(d.nodes[&fid].children, vec![a.clone()]);
        assert!(d.validate().is_ok());

        // 根不可分离；树内节点不可直接 SetNodePos
        assert!(d
            .apply(&Command::DetachNode {
                id: root.clone(),
                x: 0.0,
                y: 0.0
            })
            .is_err());
        d.apply(&Command::SetNodePos {
            id: fid.clone(),
            x: 7.0,
            y: 8.0,
        })
        .unwrap();
        assert_eq!(d.nodes[&fid].pos.as_ref().unwrap().y, 8.0);

        // 全部撤销 → 回到只有根
        while d.can_undo() {
            d.undo().unwrap();
            assert!(d.validate().is_ok());
        }
        assert_eq!(d.node_count(), 1);
    }

    /// CM-04：概念图的主概念也能被摆放；思维导图的根不行。
    #[test]
    fn move_concept_only_works_in_concept_maps() {
        let mut d = doc();
        let root = d.root_id.clone();

        // 思维导图：根不可摆（既有不变量，`MoveConcept` 也不该开后门）
        assert!(d
            .apply(&Command::MoveConcept {
                id: root.clone(),
                x: 10.0,
                y: 20.0
            })
            .is_err());

        // 切成概念图后：根可以摆，且一条命令一次撤销
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        d.apply(&Command::MoveConcept {
            id: root.clone(),
            x: 10.0,
            y: 20.0,
        })
        .unwrap();
        let p = d.nodes[&root].pos.as_ref().unwrap().clone();
        assert_eq!((p.x, p.y), (10.0, 20.0));
        assert!(d.validate().is_ok(), "{:?}", d.validate());

        // 有父级的节点仍不可摆（概念图不靠 children 表达层级）。
        //
        // 这种节点今天**已经造不出来**了——概念图里 `AddNode` 本身就被拒
        // （见 `add_node_is_rejected_in_concept_maps`）。这条断言守的是旧文件与导入数据：
        // 「先是一棵思维导图、再切成概念图」之后，残留的父子关系不能让 `MoveConcept` 开后门。
        let mut legacy = doc();
        let legacy_root = legacy.root_id.clone();
        let legacy_child = add(&mut legacy, &legacy_root, "子");
        legacy
            .apply(&Command::SetLayout {
                layout: LayoutSettings {
                    kind: "conceptMap".into(),
                    ..Default::default()
                },
            })
            .unwrap();
        assert!(
            legacy
                .apply(&Command::MoveConcept {
                    id: legacy_child,
                    x: 1.0,
                    y: 2.0
                })
                .is_err(),
            "带父级的节点（旧数据）不许自由摆放"
        );

        d.undo().unwrap();
        d.undo().unwrap();
        assert!(d.nodes[&root].pos.is_none(), "撤销后位置要退回");
        assert!(d.validate().is_ok());
    }

    /// 概念图夹具：第一个概念当主概念（带 `pos`），其余是浮动概念。
    fn concept_doc(concepts: &[(&str, f32, f32)]) -> Document {
        let mut d = doc();
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .expect("切概念图");
        let root = d.root_id.clone();
        let (text, x, y) = concepts[0];
        d.apply(&Command::SetText {
            id: root.clone(),
            text: text.into(),
        })
        .expect("主概念改名");
        d.apply(&Command::MoveConcept { id: root, x, y })
            .expect("主概念摆位");
        for (text, x, y) in &concepts[1..] {
            d.apply(&Command::AddFloatingTopic {
                text: (*text).into(),
                x: *x,
                y: *y,
            })
            .expect("加概念");
        }
        d
    }

    /// CM-11：概念粘贴必须是独立概念，关系线只在副本之间重映射，整批一次撤销。
    ///
    /// 这条钉住三件旧病：① 粘贴造出父子关系；② 关系线指向原概念、副本里留下悬空边；
    /// ③ 两个概念变成两步撤销。
    #[test]
    fn inserting_concepts_keeps_them_floating_and_undoes_in_one_step() {
        use crate::model::{ArrowHead, Position, Relationship};

        let mut d = concept_doc(&[("主概念", 0.0, 0.0)]);
        let before_count = d.node_count();
        let before_undo = d.undo_depth();
        let a = new_id();
        let b = new_id();
        let mut na = Node::new(a.clone(), None, "甲");
        na.pos = Some(Position { x: 40.0, y: 60.0 });
        na.relationships.push(Relationship {
            id: new_id(),
            to: b.clone(),
            label: Some("导致".into()),
            color: None,
            head: ArrowHead::Forward,
            ..Default::default()
        });
        let mut nb = Node::new(b.clone(), None, "乙");
        nb.pos = Some(Position { x: 240.0, y: 60.0 });

        let cs = d
            .apply(&Command::InsertConcepts {
                nodes: vec![na, nb],
            })
            .expect("概念图应接受独立概念批");
        assert_eq!(d.node_count(), before_count + 2);
        assert_eq!(d.undo_depth(), before_undo + 1, "整批只占一步撤销");
        assert!(d.validate().is_ok(), "{:?}", d.validate());
        assert_eq!(d.nodes[&a].parent_id, None);
        assert!(d.nodes[&a].children.is_empty());
        assert_eq!(d.nodes[&a].relationships[0].to, b);
        assert_eq!(cs.after.len(), 2, "变更集要覆盖两颗新概念");

        d.undo().expect("一次撤销");
        assert_eq!(d.node_count(), before_count);
        assert!(!d.nodes.contains_key(&a));
        assert!(d.validate().is_ok());
    }

    /// 树文档不接受 `InsertConcepts`；概念图也不接受带父子结构的批次。
    #[test]
    fn insert_concepts_is_concept_map_only_and_rejects_tree_payloads() {
        let mut tree = doc();
        let mut node = Node::new(new_id(), None, "甲");
        node.pos = Some(crate::model::Position { x: 0.0, y: 0.0 });
        assert!(matches!(
            tree.apply(&Command::InsertConcepts { nodes: vec![node] }),
            Err(CoreError::Invalid(_))
        ));

        let mut cm = concept_doc(&[("主概念", 0.0, 0.0)]);
        let mut child = Node::new(new_id(), Some(cm.root_id.clone()), "子");
        child.pos = Some(crate::model::Position { x: 10.0, y: 10.0 });
        assert!(matches!(
            cm.apply(&Command::InsertConcepts { nodes: vec![child] }),
            Err(CoreError::Invalid(_))
        ));
        assert!(cm.validate().is_ok());
    }

    /// 卡片是 1:1 绑定：粘贴一个沿用旧 `card_id` 的副本必须被拒绝，
    /// 而不是先落盘、下次打开才发现文档脏了。
    #[test]
    fn inserting_a_concept_does_not_steal_an_already_bound_card() {
        let mut d = concept_doc(&[("主概念", 0.0, 0.0)]);
        let root = d.root_id.clone();
        d.cards
            .push(NoteCard::new(d.id.clone(), Some(root.clone()), "卡", "", 1));
        let card_id = d.cards[0].id.clone();
        d.nodes.get_mut(&root).unwrap().card_id = Some(card_id.clone());
        assert!(d.validate().is_ok());

        let mut copy = Node::new(new_id(), None, "副本");
        copy.pos = Some(crate::model::Position { x: 40.0, y: 40.0 });
        copy.card_id = Some(card_id);
        assert!(matches!(
            d.apply(&Command::InsertConcepts { nodes: vec![copy] }),
            Err(CoreError::Invalid(_))
        ));
        assert_eq!(d.node_count(), 1);
        assert!(d.validate().is_ok());
    }

    /// CM-05：概念图的主概念能删，删完把根重挂到「剩下里最靠左上」的概念上；
    /// 撤销要把根**一起**还回来，否则根会指着已经不存在的节点。
    #[test]
    fn deleting_the_first_concept_rehangs_the_root_and_undo_restores_it() {
        let mut d = concept_doc(&[("甲", 0.0, 0.0), ("乙", 200.0, 0.0), ("丙", 0.0, 200.0)]);
        let first = d.root_id.clone();
        assert!(d.validate().is_ok(), "{:?}", d.validate());

        let cs = d
            .apply(&Command::DeleteNodes {
                ids: vec![first.clone()],
            })
            .expect("概念图主概念可删");
        assert_eq!(cs.before_root.as_deref(), Some(first.as_str()));
        assert!(!d.nodes.contains_key(&first));
        let new_root = d.root_id.clone();
        assert_ne!(new_root, first, "删掉主概念必须换根");
        assert_eq!(d.node(&new_root).map(|n| n.text.as_str()), Some("乙"));
        assert!(d.validate().is_ok(), "{:?}", d.validate());

        d.undo().expect("撤销删除");
        assert_eq!(d.root_id, first, "撤销必须把根一起还回来");
        assert!(d.nodes.contains_key(&first));
        assert!(d.validate().is_ok(), "{:?}", d.validate());

        d.redo().expect("重做删除");
        assert_eq!(d.root_id, new_root);
        assert!(!d.nodes.contains_key(&first));
        assert!(d.validate().is_ok(), "{:?}", d.validate());
    }

    /// 最后一个概念删不得：没有「零概念概念图」这种文档，落下去 `validate` 立刻判脏。
    #[test]
    fn deleting_the_last_concept_is_rejected() {
        let mut d = concept_doc(&[("唯一", 0.0, 0.0)]);
        let root = d.root_id.clone();
        assert!(matches!(
            d.apply(&Command::DeleteNodes { ids: vec![root] }),
            Err(CoreError::RootProtected)
        ));
        assert_eq!(d.node_count(), 1);
        assert!(d.validate().is_ok());

        // 思维导图的根依旧受保护（这条没有放宽）
        let mut tree = doc();
        let troot = tree.root_id.clone();
        assert!(matches!(
            tree.apply(&Command::DeleteNodes { ids: vec![troot] }),
            Err(CoreError::RootProtected)
        ));
    }

    /// CM-05：整组微调是**一条** Batch——按 5 次方向键不该变成 5 步撤销。
    #[test]
    fn nudging_a_group_of_concepts_is_one_undo_step() {
        let mut d = concept_doc(&[("甲", 0.0, 0.0), ("乙", 200.0, 0.0)]);
        let root = d.root_id.clone();
        let other = d
            .doc_order()
            .into_iter()
            .find(|id| id != &root)
            .expect("第二个概念");
        let before: Vec<(f32, f32)> = [&root, &other]
            .iter()
            .map(|id| {
                let p = d.nodes[*id].pos.as_ref().unwrap();
                (p.x, p.y)
            })
            .collect();

        let nudge = Command::Batch {
            commands: vec![
                Command::MoveConcept {
                    id: root.clone(),
                    x: before[0].0 + 8.0,
                    y: before[0].1,
                },
                Command::MoveConcept {
                    id: other.clone(),
                    x: before[1].0 + 8.0,
                    y: before[1].1,
                },
            ],
        };
        d.apply(&nudge).expect("整组微调");
        for (i, id) in [&root, &other].iter().enumerate() {
            let p = d.nodes[*id].pos.as_ref().unwrap();
            assert_eq!(
                (p.x, p.y),
                (before[i].0 + 8.0, before[i].1),
                "两个概念都该平移 8px"
            );
        }
        assert!(d.validate().is_ok(), "{:?}", d.validate());

        d.undo().expect("一次撤销回原位");
        let after: Vec<(f32, f32)> = [&root, &other]
            .iter()
            .map(|id| {
                let p = d.nodes[*id].pos.as_ref().unwrap();
                (p.x, p.y)
            })
            .collect();
        assert_eq!(after, before, "一次撤销要把整组都送回原位");
    }

    #[test]
    fn labels_add_remove_undo_and_limits() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        d.apply(&Command::AddLabel {
            ids: vec![a.clone()],
            label: " 急 ".into(),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].labels, vec!["急".to_string()]);
        d.apply(&Command::AddLabel {
            ids: vec![a.clone()],
            label: "急".into(),
        })
        .unwrap();
        assert_eq!(d.nodes[&a].labels.len(), 1);
        for i in 0..4 {
            d.apply(&Command::AddLabel {
                ids: vec![a.clone()],
                label: format!("t{i}"),
            })
            .unwrap();
        }
        assert_eq!(d.nodes[&a].labels.len(), MAX_LABELS);
        assert!(d
            .apply(&Command::AddLabel {
                ids: vec![a.clone()],
                label: "溢出".into()
            })
            .is_err());
        assert!(d
            .apply(&Command::AddLabel {
                ids: vec![a.clone()],
                label: "   ".into()
            })
            .is_err());
        d.apply(&Command::RemoveLabel {
            ids: vec![a.clone()],
            label: "急".into(),
        })
        .unwrap();
        assert!(!d.nodes[&a].labels.iter().any(|l| l == "急"));
        d.undo().unwrap();
        assert!(d.nodes[&a].labels.iter().any(|l| l == "急"));
        assert!(d.validate().is_ok());
    }

    #[test]
    fn insert_subtree_sanitizes_pos_and_relationships() {
        let mut d = doc();
        let root = d.root_id.clone();
        let mut top = Node::new("p-1".into(), Some(root.clone()), "粘贴");
        let mut child = Node::new("p-2".into(), Some("p-1".into()), "子");
        child.pos = Some(Position { x: 1.0, y: 2.0 });
        child.relationships.push(Relationship {
            id: "r".into(),
            to: "不存在的节点".into(),
            label: None,
            color: None,
            head: ArrowHead::Forward,
            ..Default::default()
        });
        child.relationships.push(Relationship {
            id: "r2".into(),
            to: root.clone(),
            label: None,
            color: None,
            head: ArrowHead::Forward,
            ..Default::default()
        });
        child.labels = vec!["a".into(), "a".into()];
        top.children = vec!["p-2".into()];
        d.apply(&Command::InsertSubtree {
            parent: root.clone(),
            index: None,
            nodes: vec![top, child],
        })
        .unwrap();
        let c = &d.nodes["p-2"];
        assert!(c.pos.is_none());
        assert_eq!(c.relationships.len(), 1);
        assert_eq!(c.relationships[0].to, root);
        assert_eq!(c.labels, vec!["a".to_string()]);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn from_parts_rejects_dangling_annotations() {
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "A");
        let mut nodes = d.nodes.clone();
        nodes.get_mut(&root).unwrap().boundaries.push(Boundary {
            id: "b".into(),
            targets: vec![a],
            color: None,
        });
        assert!(
            Document::from_parts(None, DocMeta::default(), root.clone(), nodes.clone()).is_ok()
        );
        nodes.get_mut(&root).unwrap().boundaries[0].targets = vec!["不存在".into()];
        assert!(Document::from_parts(None, DocMeta::default(), root, nodes).is_err());
    }

    #[test]
    fn accent_override_applies_undoes_and_redoes() {
        let mut d = doc();
        d.apply(&Command::SetThemeAccent {
            accent: Some("#e5484d".into()),
        })
        .unwrap();
        assert_eq!(d.meta.theme_accent.as_deref(), Some("#e5484d"));
        d.undo().unwrap();
        assert_eq!(d.meta.theme_accent, None, "撤销该把覆盖还回未设置");
        d.redo().unwrap();
        assert_eq!(d.meta.theme_accent.as_deref(), Some("#e5484d"));
        // 回到主题默认也是一步可撤销的改动，不是「什么都不做」
        d.apply(&Command::SetThemeAccent { accent: None }).unwrap();
        assert_eq!(d.meta.theme_accent, None);
        d.undo().unwrap();
        assert_eq!(d.meta.theme_accent.as_deref(), Some("#e5484d"));
    }

    /// 注解只认直接父级：概要/边界盖住哪个子节点就报哪个。
    #[test]
    fn annotations_covering_lists_summaries_then_boundaries() {
        use crate::model::AnnotationKind;
        let mut d = doc();
        let root = d.root_id.clone();
        let a = add(&mut d, &root, "甲");
        let b = add(&mut d, &root, "乙");
        d.apply(&Command::AddSummary {
            parent: root.clone(),
            targets: vec![a.clone()],
            text: "结论".into(),
        })
        .unwrap();
        d.apply(&Command::AddBoundary {
            parent: root.clone(),
            targets: vec![a.clone(), b.clone()],
            color: None,
        })
        .unwrap();

        let covered = d.annotations_covering(&a);
        assert_eq!(covered.len(), 2);
        assert_eq!(covered[0].kind, AnnotationKind::Summary);
        assert_eq!(covered[0].label, "结论");
        assert_eq!(covered[0].parent, root);
        assert_eq!(covered[1].kind, AnnotationKind::Boundary);

        assert_eq!(d.annotations_covering(&b).len(), 1, "乙只被边界盖住");
        assert!(
            d.annotations_covering(&root).is_empty(),
            "根没有父级，不可能被注解盖住"
        );
    }

    #[test]
    fn theme_overrides_are_stored_rejected_and_undoable() {
        use crate::model::ThemeOverrides;
        let mut d = doc();
        let o = ThemeOverrides {
            canvas_bg: Some("#fad8df".into()),
            ..Default::default()
        };
        d.apply(&Command::SetThemeOverrides {
            overrides: Some(o.clone()),
        })
        .unwrap();
        assert_eq!(d.meta.theme_overrides.as_ref(), Some(&o));
        d.undo().unwrap();
        assert_eq!(d.meta.theme_overrides, None, "撤销要回到未设置");
        d.redo().unwrap();
        assert_eq!(d.meta.theme_overrides.as_ref(), Some(&o));

        // 三个槽位都空 = 没有微调，存 None 而不是空对象
        d.apply(&Command::SetThemeOverrides {
            overrides: Some(ThemeOverrides::default()),
        })
        .unwrap();
        assert_eq!(d.meta.theme_overrides, None);

        // 坏颜色直接拒绝，界面不会「设了又没生效」
        let err = d
            .apply(&Command::SetThemeOverrides {
                overrides: Some(ThemeOverrides {
                    canvas_bg: Some("tomato".into()),
                    ..Default::default()
                }),
            })
            .unwrap_err();
        assert!(err.to_string().contains("#rrggbb"), "{err}");
        assert_eq!(d.meta.theme_overrides, None);
    }

    #[test]
    fn a_non_hex_accent_is_rejected_instead_of_stored() {
        // 存进去只会让下一次 `theme_for` 静默忽略它：界面上「设了又没设」，比报错难查得多。
        let mut d = doc();
        let err = d
            .apply(&Command::SetThemeAccent {
                accent: Some("tomato".into()),
            })
            .unwrap_err();
        assert!(err.to_string().contains("#rrggbb"), "{err}");
        assert_eq!(d.meta.theme_accent, None);
        assert!(!d.can_undo(), "被拒绝的命令不该留下撤销足迹");
        d.apply(&Command::SetThemeAccent {
            accent: Some("#F5A623".into()),
        })
        .unwrap();
        assert_eq!(d.meta.theme_accent.as_deref(), Some("#F5A623"));
    }

    #[test]
    fn switching_theme_keeps_the_accent_and_clearing_it_restores_the_preset() {
        let mut d = doc();
        d.apply(&Command::SetTheme {
            theme: "ink".into(),
        })
        .unwrap();
        d.apply(&Command::SetThemeAccent {
            accent: Some("#e5484d".into()),
        })
        .unwrap();
        // 覆盖记的是色相，切主题得把它一起带过去，而不是切一下主题就丢。
        d.apply(&Command::SetTheme {
            theme: "ink".into(),
        })
        .unwrap();
        assert_eq!(d.meta.theme.as_str(), "ink");
        assert_eq!(d.meta.theme_accent.as_deref(), Some("#e5484d"));
        let themed = crate::theme_for(&d.meta);
        assert_eq!(themed.id, "ink");
        assert_ne!(
            themed.ui.accent,
            crate::get_theme("ink").ui.accent,
            "强调色覆盖必须真的落到派生主题上"
        );
        d.apply(&Command::SetThemeAccent { accent: None }).unwrap();
        assert_eq!(crate::theme_for(&d.meta), crate::get_theme("ink"));
    }

    #[test]
    fn the_accent_command_is_named_for_the_toast() {
        let cs = doc()
            .apply(&Command::SetThemeAccent {
                accent: Some("#e5484d".into()),
            })
            .unwrap();
        assert_eq!(cs.label, "更换强调色");
    }

    // ---------------- C14：概念图里不许漏出树语义 ----------------

    /// 概念图里 `MoveNode` 必须被拒，且**文档一个字节都不许动**。
    ///
    /// 这条钉住的是最脏的一条漏洞：`MoveNode` 会写 `parent_id = Some(..)` 并把 `pos`
    /// 抹成 `None`（`n.pos = None; // 挂回树内后不再是自由主题`），概念图里跑一次，
    /// 概念就静默降级成树节点、位置当场丢失，而 `validate()` 之后只允许「根 + 自由概念」
    /// 有 `pos`。壳层的大纲拖动（`OA::Move` / `OA::MoveMany`）是无条件下发的。
    #[test]
    fn move_node_is_rejected_in_concept_maps() {
        let mut d = concept_doc(&[("主概念", 0.0, 0.0), ("甲", 300.0, 40.0)]);
        let root = d.root_id.clone();
        let child = d
            .doc_order()
            .into_iter()
            .find(|id| id != &root)
            .expect("夹具里应当有第二颗概念");
        let pos_before = d.node(&child).and_then(|n| n.pos.clone());
        assert!(pos_before.is_some(), "夹具：概念必须带位置");

        let canon_before = canon(&d);
        let rev_before = d.revision;
        let undo_before = d.undo_depth();

        let err = d
            .apply(&Command::MoveNode {
                id: child.clone(),
                new_parent: root.clone(),
                index: None,
            })
            .expect_err("概念图里不许把概念挂成树节点");
        assert!(
            matches!(&err, CoreError::Invalid(m) if m == "概念图里概念不能挂成树节点"),
            "错误口径要写人话：{err:?}"
        );

        // 无副作用：字节级不变、revision 不动、撤销栈不动。
        assert_eq!(canon(&d), canon_before, "被拒的命令不能改文档");
        assert_eq!(d.revision, rev_before, "被拒的命令不能推进 revision");
        assert_eq!(d.undo_depth(), undo_before, "被拒的命令不能压撤销栈");
        let n = d.node(&child).expect("概念还在");
        assert_eq!(n.parent_id, None, "概念不能长出 parent_id");
        assert_eq!(n.pos, pos_before, "概念的位置不能被抹掉");
        assert!(d.validate().is_ok(), "文档必须仍然自洽");
    }

    /// 概念图里 `AddNode` 被拒——**这是本轮做出的取舍**：`AddNode` 只带 `parent`、不带位置，
    /// 「强行改成独立概念」就得替用户编一个坐标，所以宁可直接拒绝，让调用方走
    /// `AddFloatingTopic` / `AddLinkedConcept`。同一条命令在思维导图里照旧成功。
    #[test]
    fn add_node_is_rejected_in_concept_maps() {
        let mut cm = concept_doc(&[("主概念", 0.0, 0.0)]);
        let root = cm.root_id.clone();
        let canon_before = canon(&cm);
        let undo_before = cm.undo_depth();
        let err = cm
            .apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: "子节点".into(),
                style: None,
            })
            .expect_err("概念图里不许挂子节点");
        assert!(
            matches!(&err, CoreError::Invalid(m) if m == "概念图里概念不能挂成树节点"),
            "错误口径要写人话：{err:?}"
        );
        assert_eq!(canon(&cm), canon_before);
        assert_eq!(cm.undo_depth(), undo_before);
        assert_eq!(cm.node_count(), 1, "不能偷偷多出一颗节点");

        // 反面：思维导图的同一命令照旧成功（守卫只针对概念图）。
        let mut mm = doc();
        let root = mm.root_id.clone();
        mm.apply(&Command::AddNode {
            parent: root,
            index: None,
            text: "子节点".into(),
            style: None,
        })
        .expect("思维导图里 AddNode 是正经入口");
        assert_eq!(mm.node_count(), 2);
    }

    /// 子树粘贴也是 `parent_id = Some(..)`，概念图里一并拒；概念要粘贴得走 `InsertConcepts`。
    #[test]
    fn insert_subtree_is_rejected_in_concept_maps() {
        use crate::model::Position;

        let mut d = concept_doc(&[("主概念", 0.0, 0.0)]);
        let root = d.root_id.clone();
        let mut n = Node::new(new_id(), Some(root.clone()), "甲");
        n.pos = Some(Position { x: 10.0, y: 20.0 });
        let canon_before = canon(&d);
        let err = d
            .apply(&Command::InsertSubtree {
                parent: root,
                index: None,
                nodes: vec![n],
            })
            .expect_err("概念图里不许粘贴子树");
        assert!(
            matches!(&err, CoreError::Invalid(m) if m.contains("概念图")),
            "错误文案要讲清楚走哪去：{err:?}"
        );
        assert_eq!(canon(&d), canon_before);
        assert_eq!(d.node_count(), 1);
    }

    /// `AddLinkedConcept` 自己带概念图护栏（今天两个调用点只是碰巧在概念图分支里）。
    #[test]
    fn add_linked_concept_is_concept_map_only() {
        let mut mm = doc();
        let root = mm.root_id.clone();
        let canon_before = canon(&mm);
        let undo_before = mm.undo_depth();
        let err = mm
            .apply(&Command::AddLinkedConcept {
                from: root.clone(),
                text: "概念".into(),
                x: 40.0,
                y: 40.0,
            })
            .expect_err("思维导图里没有浮点概念");
        assert!(
            matches!(&err, CoreError::Invalid(m) if m == "只有概念图可以新建概念并连线"),
            "错误口径要写人话：{err:?}"
        );
        assert_eq!(canon(&mm), canon_before, "被拒的命令不能留下孤儿概念");
        assert_eq!(mm.undo_depth(), undo_before);
        assert_eq!(mm.node_count(), 1);

        // 姊妹手势同样只在概念图里合法。
        let rel = new_id();
        let err = mm
            .apply(&Command::RetargetLinkToNewConcept {
                from: root,
                id: rel,
                text: "概念".into(),
                x: 40.0,
                y: 40.0,
            })
            .expect_err("思维导图里同样的道理");
        assert!(matches!(&err, CoreError::Invalid(_)), "{err:?}");

        // 反面：概念图里同一条命令成功，且一次撤销就回到原样（CM-10 ①）。
        let mut cm = concept_doc(&[("主概念", 0.0, 0.0)]);
        let root = cm.root_id.clone();
        let canon_before = canon(&cm);
        cm.apply(&Command::AddLinkedConcept {
            from: root,
            text: "新概念".into(),
            x: 120.0,
            y: 60.0,
        })
        .expect("概念图里这是正经入口");
        assert_eq!(cm.node_count(), 2);
        cm.undo().unwrap();
        assert_eq!(canon(&cm), canon_before, "一步撤销要回到按之前");
    }

    /// 同一对概念之间**可以有多条边**（两条边 = 两条不同的命题），且互相独立：
    /// 删掉其中一条，另一条连短语一起还在；一次撤销只吐回一条边。
    ///
    /// 旧口径是「两节点之间已存在关系线」，那是树语义带出来的（一棵树里两个节点只有一条路），
    /// 直接把「A 影响 B」+「A 包含 B」判死。
    #[test]
    fn parallel_propositions_between_the_same_pair_are_independent() {
        let mut d = concept_doc(&[("甲", 0.0, 0.0), ("乙", 300.0, 60.0)]);
        let from = d.root_id.clone();
        let to = d
            .doc_order()
            .into_iter()
            .find(|id| id != &from)
            .expect("夹具里应当有第二颗概念");

        d.apply(&Command::AddRelationship {
            from: from.clone(),
            to: to.clone(),
            label: Some("影响".into()),
        })
        .expect("第一条命题");
        d.apply(&Command::AddRelationship {
            from: from.clone(),
            to: to.clone(),
            label: Some("包含".into()),
        })
        .expect("同一对概念之间的第二条命题必须能建起来");

        let rels = d.node(&from).unwrap().relationships.clone();
        assert_eq!(rels.len(), 2, "两条边各占一个槽位");
        let a = rels[0].id.clone();
        let b = rels[1].id.clone();
        assert_ne!(a, b, "平行边靠 id 区分，不是靠 (from, to)");
        assert_eq!(rels[0].to, rels[1].to, "两条边指向同一个目标");
        assert_eq!(rels[0].label.as_deref(), Some("影响"));
        assert_eq!(rels[1].label.as_deref(), Some("包含"));

        let undo_before = d.undo_depth();
        d.apply(&Command::DeleteRelationship {
            from: from.clone(),
            id: a.clone(),
        })
        .expect("删掉第一条");
        let left = d.node(&from).unwrap().relationships.clone();
        assert_eq!(left.len(), 1, "只该少一条");
        assert_eq!(left[0].id, b, "剩下的是另一条");
        assert_eq!(
            left[0].label.as_deref(),
            Some("包含"),
            "删一条不能把另一条的短语也改掉"
        );

        // 一次撤销只吐回一条边。
        d.undo().unwrap();
        assert_eq!(d.undo_depth(), undo_before);
        assert_eq!(d.node(&from).unwrap().relationships.len(), 2);

        // 删第二条同样只影响它自己。
        d.apply(&Command::DeleteRelationship { from, id: b })
            .unwrap();
        assert_eq!(d.node(&d.root_id).unwrap().relationships.len(), 1);
    }

    /// 自连仍然禁止：这与树语义无关，是自反命题的脏数据。
    #[test]
    fn self_link_is_still_rejected() {
        let mut cm = concept_doc(&[("甲", 0.0, 0.0)]);
        let root = cm.root_id.clone();
        let err = cm
            .apply(&Command::AddRelationship {
                from: root.clone(),
                to: root.clone(),
                label: Some("自己".into()),
            })
            .expect_err("概念不许连自己");
        assert!(
            matches!(&err, CoreError::Invalid(m) if m == "不能把节点连接到自身"),
            "{err:?}"
        );

        let mut mm = doc();
        let root = mm.root_id.clone();
        assert!(mm
            .apply(&Command::AddRelationship {
                from: root.clone(),
                to: root,
                label: None,
            })
            .is_err());
    }

    /// 概念图里**不许把已有短语清空**（没有短语的线不构成命题），
    /// 但「新建时留空」照样合法——拖出新线立刻写短语那条流程不能堵死。
    #[test]
    fn clearing_the_phrase_is_rejected_in_concept_maps() {
        let mut d = concept_doc(&[("甲", 0.0, 0.0), ("乙", 300.0, 60.0)]);
        let from = d.root_id.clone();
        let to = d
            .doc_order()
            .into_iter()
            .find(|id| id != &from)
            .expect("第二颗概念");

        // 建线时留空是合法的：画布拖出新线就是这条路径。
        d.apply(&Command::AddRelationship {
            from: from.clone(),
            to: to.clone(),
            label: None,
        })
        .expect("新建时可以不写短语");
        let id = d.node(&from).unwrap().relationships[0].id.clone();
        assert_eq!(d.node(&from).unwrap().relationships[0].label, None);

        d.apply(&Command::SetRelationshipLabel {
            from: from.clone(),
            id: id.clone(),
            label: Some("影响".into()),
        })
        .expect("补上短语");

        let canon_before = canon(&d);
        let undo_before = d.undo_depth();
        for empty in [None, Some("".to_string()), Some("   ".to_string())] {
            let err = d
                .apply(&Command::SetRelationshipLabel {
                    from: from.clone(),
                    id: id.clone(),
                    label: empty.clone(),
                })
                .expect_err("概念图里不许把短语清空");
            assert!(
                matches!(&err, CoreError::Invalid(m) if m == "概念图的连接短语必填"),
                "{err:?}"
            );
            assert_eq!(canon(&d), canon_before, "被拒的命令不能改文档");
            assert_eq!(d.undo_depth(), undo_before);
        }
        assert_eq!(
            d.node(&from).unwrap().relationships[0].label.as_deref(),
            Some("影响"),
            "短语要原样留着"
        );

        // 反面：思维导图的线本来就可以没有短语，清空照旧成功。
        let mut mm = doc();
        let root = mm.root_id.clone();
        mm.apply(&Command::AddNode {
            parent: root.clone(),
            index: None,
            text: "甲".into(),
            style: None,
        })
        .unwrap();
        let a = mm.node(&root).unwrap().children[0].clone();
        mm.apply(&Command::AddRelationship {
            from: root.clone(),
            to: a,
            label: Some("支撑".into()),
        })
        .unwrap();
        let rid = mm.node(&root).unwrap().relationships[0].id.clone();
        mm.apply(&Command::SetRelationshipLabel {
            from: root.clone(),
            id: rid,
            label: None,
        })
        .expect("思维导图清空短语是允许的");
        assert_eq!(mm.node(&root).unwrap().relationships[0].label, None);
    }

    /// 往历史栈里压 `n` 条各自独立的命令（改根节点文字），返回根 id。
    fn push_history(d: &mut Document, n: usize) -> String {
        let root = d.root_id.clone();
        for i in 0..n {
            d.apply(&Command::SetText {
                id: root.clone(),
                text: format!("v{i}"),
            })
            .unwrap();
        }
        root
    }

    #[test]
    fn history_limit_defaults_and_clamps() {
        let mut d = doc();
        assert_eq!(d.history_limit(), HISTORY_LIMIT);
        assert_eq!(HISTORY_LIMIT, 500);

        d.set_history_limit(1);
        assert_eq!(d.history_limit(), MIN_HISTORY_LIMIT);
        d.set_history_limit(usize::MAX);
        assert_eq!(d.history_limit(), MAX_HISTORY_LIMIT);
        d.set_history_limit(800);
        assert_eq!(d.history_limit(), 800);
    }

    #[test]
    fn lowering_history_limit_drops_oldest_and_undo_stops_there() {
        let mut d = doc();
        let root = push_history(&mut d, 130);
        assert_eq!(d.undo_depth(), 130);

        d.set_history_limit(100);
        assert_eq!(d.undo_depth(), 100, "超出的最旧 30 条要立刻丢掉");

        for _ in 0..100 {
            assert!(d.undo().is_some());
        }
        assert!(d.undo().is_none(), "被裁掉的历史不能撤回来");
        assert_eq!(
            d.node(&root).unwrap().text,
            "v29",
            "剩 100 步 = 第 31–130 条，全撤后应停在第 30 条之后"
        );

        // 调高上限不会凭空补回被裁掉的历史。
        d.set_history_limit(MAX_HISTORY_LIMIT);
        assert_eq!(d.undo_depth(), 0);
        assert_eq!(d.node(&root).unwrap().text, "v29");
    }

    #[test]
    fn redo_cannot_push_history_past_limit_after_lowering() {
        let mut d = doc();
        let root = push_history(&mut d, 150);
        for _ in 0..60 {
            d.undo().unwrap();
        }
        assert_eq!((d.undo_depth(), d.redo_depth()), (90, 60));

        // 上限在撤销之后调低：重做把条目压回 undo 时也不能越界。
        d.set_history_limit(100);
        for _ in 0..60 {
            d.redo().unwrap();
        }
        assert_eq!((d.undo_depth(), d.redo_depth()), (100, 0));
        assert_eq!(d.node(&root).unwrap().text, "v149", "重做仍是完整回放");

        for _ in 0..100 {
            assert!(d.undo().is_some());
        }
        assert!(d.undo().is_none());
        assert_eq!(
            d.node(&root).unwrap().text,
            "v49",
            "保留的是最近 100 条（第 51–150 条）"
        );
    }

    #[test]
    fn coalesced_apply_honors_history_limit() {
        let mut d = doc();
        d.set_history_limit(MIN_HISTORY_LIMIT);
        let root = d.root_id.clone();
        for i in 0..(MIN_HISTORY_LIMIT + 1) {
            d.apply_coalesced(
                &Command::SetText {
                    id: root.clone(),
                    text: format!("c{i}"),
                },
                |_| false,
            )
            .unwrap();
        }
        assert_eq!(d.undo_depth(), MIN_HISTORY_LIMIT);
        assert_eq!(d.node(&root).unwrap().text, "c100");

        // 合并路径本来就不加栈深，上限之下同样不越界。
        d.apply_coalesced(
            &Command::SetText {
                id: root.clone(),
                text: "merged".into(),
            },
            |_| true,
        )
        .unwrap();
        assert_eq!(d.undo_depth(), MIN_HISTORY_LIMIT);
    }

    #[test]
    fn set_concept_pinned_toggles_and_undoes_in_one_step() {
        let mut d = doc();
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = d.root_id.clone();
        assert_eq!(d.node(&root).unwrap().pinned, None);

        // 钉住
        d.apply(&Command::SetConceptPinned {
            id: root.clone(),
            pinned: true,
        })
        .unwrap();
        assert_eq!(d.node(&root).unwrap().pinned, Some(true));

        // 撤销 -> 回到 None
        d.undo().unwrap();
        assert_eq!(d.node(&root).unwrap().pinned, None);

        // 重做 -> Some(true)
        d.redo().unwrap();
        assert_eq!(d.node(&root).unwrap().pinned, Some(true));

        // 取消钉住 -> 回到 None（不留 Some(false) 脏数据）
        d.apply(&Command::SetConceptPinned {
            id: root.clone(),
            pinned: false,
        })
        .unwrap();
        assert_eq!(d.node(&root).unwrap().pinned, None);
    }

    #[test]
    fn set_concept_pinned_rejects_tree_node_and_mind_map() {
        let mut d = doc();
        let root = d.root_id.clone();
        // 思维导图下直接拒绝
        let err = d
            .apply(&Command::SetConceptPinned {
                id: root.clone(),
                pinned: true,
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::Invalid(_)));

        // 切到概念图
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();

        // 树子节点（带 parent_id）同样拒绝（概念图里只有浮动概念能被钉住）
        d.nodes
            .get_mut(&root)
            .unwrap()
            .children
            .push("child".into());
        d.nodes.insert(
            "child".into(),
            Node {
                id: "child".into(),
                text: "子".into(),
                parent_id: Some(root.clone()),
                ..Default::default()
            },
        );
        let err2 = d
            .apply(&Command::SetConceptPinned {
                id: "child".into(),
                pinned: true,
            })
            .unwrap_err();
        assert!(matches!(err2, CoreError::Invalid(_)));
    }

    #[test]
    fn concept_proposition_count_and_pinned_helpers_work() {
        let mut d = Document::new("概念测试");
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let a = d.root_id.clone();
        d.apply(&Command::AddFloatingTopic {
            text: "乙".into(),
            x: 200.0,
            y: 0.0,
        })
        .unwrap();
        d.apply(&Command::AddFloatingTopic {
            text: "丙".into(),
            x: 0.0,
            y: 200.0,
        })
        .unwrap();
        let b = d
            .nodes
            .values()
            .find(|n| n.text == "乙")
            .unwrap()
            .id
            .clone();
        let c = d
            .nodes
            .values()
            .find(|n| n.text == "丙")
            .unwrap()
            .id
            .clone();

        assert!(!d.is_concept_pinned(&a));
        assert_eq!(d.concept_proposition_count(&a), 0);

        d.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b.clone(),
            label: Some("出".into()),
        })
        .unwrap();
        d.apply(&Command::AddRelationship {
            from: c.clone(),
            to: a.clone(),
            label: Some("入".into()),
        })
        .unwrap();

        assert_eq!(d.concept_proposition_count(&a), 2);
        assert_eq!(d.concept_proposition_count(&b), 1);
        assert_eq!(d.concept_proposition_count(&c), 1);

        d.apply(&Command::SetConceptPinned {
            id: a.clone(),
            pinned: true,
        })
        .unwrap();
        assert!(d.is_concept_pinned(&a));
        assert!(!d.is_concept_pinned(&b));
    }
}
