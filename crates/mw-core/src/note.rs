//! 卡片笔记（Card Note）：节点可以链接一张独立的 Markdown 卡片。
//!
//! 为什么不是"备注字段"：备注是节点的一部分，适合一句话；卡片是独立实体，适合一篇长文、
//! 一个概念、一段摘录。节点只存 `cardId`，卡片可以在笔记库里独立管理、打标签、被搜索、
//! 被其它卡片引用。两者生命周期解耦：删节点不删卡片，卡片保留为孤儿。
//!
//! 这一层只做**数据与校验**，不碰文件、不碰 UI。`.zmind` 的落盘在 `mw-format`，
//! 全局笔记库的索引在后续包（P08）。

use serde::{Deserialize, Serialize};

use crate::model::new_id;

/// 单张卡片正文上限（字节）。1MB 足够放长文，也防止畸形文件把内存打爆。
pub const MAX_CARD_BODY: usize = 1024 * 1024;
pub const MAX_CARD_TITLE: usize = 200;
pub const MAX_CARD_TAGS: usize = 32;
pub const MAX_TAG_LEN: usize = 32;

/// 一张卡片笔记。
///
/// `source_doc` 是来源文档 id（不是路径），`source_node` 是链接的节点 id。
/// 两者都可以为空：空 = 独立卡片（收件箱/每日笔记），节点删除后保留孤儿卡。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NoteCard {
    pub id: String,
    pub title: String,
    /// Markdown 正文。
    pub body: String,
    pub tags: Vec<String>,
    pub source_doc: String,
    pub source_node: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// 已归档：从「全部卡片」里收起来，只在侧栏「归档」下出现（`docs/28` §7.1）。
    ///
    /// 与 `source_node` 是两件事——挂到导图节点只说明它被整理过，归档是用户主动收起。
    /// 落盘只在真归档时写键：老文件缺键即未归档，无迁移、不升 `FORMAT_VERSION`
    /// （字段例外记录在 `docs/32` §14）。
    #[serde(skip_serializing_if = "is_false")]
    pub archived: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl NoteCard {
    /// 新建一张卡片；`tags` 会被归一化（去空白、去重、截断）。
    pub fn new(
        source_doc: impl Into<String>,
        source_node: Option<String>,
        title: impl Into<String>,
        body: impl Into<String>,
        now: i64,
    ) -> Self {
        Self {
            id: new_id(),
            title: title.into(),
            body: body.into(),
            tags: Vec::new(),
            source_doc: source_doc.into(),
            source_node,
            created_at: now,
            updated_at: now,
            archived: false,
        }
    }

    /// 标签归一化：trim、去空、按不区分大小写去重、保序、限量。
    ///
    /// 不在这里改大小写：中文没有大小写，英文标签用户可能故意写 `Rust`。
    pub fn normalize_tags(tags: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for raw in tags {
            let t = raw.trim();
            if t.is_empty() || t.chars().count() > MAX_TAG_LEN {
                continue;
            }
            let key = t.to_lowercase();
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            out.push(t.to_string());
            if out.len() >= MAX_CARD_TAGS {
                break;
            }
        }
        out
    }

    /// 读起来安全的标题；空标题不返回空串，避免列表里一片空白。
    pub fn display_title(&self) -> &str {
        let t = self.title.trim();
        if t.is_empty() {
            "未命名卡片"
        } else {
            t
        }
    }

    /// 校验。返回人类可读的原因，由上层决定是拒绝还是降级。
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("卡片 id 不能为空".into());
        }
        if self.body.len() > MAX_CARD_BODY {
            return Err(format!("卡片正文超过 {MAX_CARD_BODY} 字节"));
        }
        if self.title.chars().count() > MAX_CARD_TITLE {
            return Err(format!("卡片标题超过 {MAX_CARD_TITLE} 字"));
        }
        if self.tags.len() > MAX_CARD_TAGS {
            return Err(format!("卡片标签超过 {MAX_CARD_TAGS} 个"));
        }
        if let Some(t) = self.tags.iter().find(|t| t.chars().count() > MAX_TAG_LEN) {
            return Err(format!("标签「{t}」超过 {MAX_TAG_LEN} 字"));
        }
        Ok(())
    }

    /// 正文的纯文本摘要（列表预览用）：去掉常见 Markdown 标记，压成一行。
    pub fn excerpt(&self, max_chars: usize) -> String {
        let mut out = String::new();
        let mut in_code = false;
        for line in self.body.lines() {
            let t = line.trim();
            if t.starts_with("```") {
                in_code = !in_code;
                continue;
            }
            if in_code || t.is_empty() {
                // 空行不额外补空格，否则摘要里会出现双空格。
                continue;
            }
            let t = t.trim_start_matches('#').trim();
            let t = t.trim_start_matches('>').trim();
            let t = t.trim_start_matches(['-', '*', '+']).trim();
            // 待办框：`- [ ] 事` 去掉bullet后还剩 `[ ] 事`，方括号留在摘要里是噪声。
            let t = t
                .strip_prefix("[ ] ")
                .or_else(|| t.strip_prefix("[x] "))
                .or_else(|| t.strip_prefix("[X] "))
                .unwrap_or(t);
            let t = t.replace(['`', '*', '_', '~'], "");
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&t);
            if out.chars().count() >= max_chars {
                break;
            }
        }
        out.chars().take(max_chars).collect()
    }
}

/// 标签路径：`知识/方法/阅读` → `["知识","方法","阅读"]`。空段被丢掉。
pub fn tag_path(tag: &str) -> Vec<&str> {
    tag.split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// 父标签：`知识/方法` → `Some("知识")`；顶层返回 `None`。
pub fn tag_parent(tag: &str) -> Option<&str> {
    let t = tag.trim();
    let (parent, _) = t.rsplit_once('/')?;
    let parent = parent.trim();
    (!parent.is_empty()).then_some(parent)
}

/// 顶层标签：`知识/方法` → `"知识"`。
pub fn tag_root(tag: &str) -> &str {
    tag_path(tag).first().copied().unwrap_or("")
}

/// 标签圆点取色下标：同一路径根永远同一支色，不依赖文档、主题或启动顺序。
///
/// 只返回 0..6 的下标，**不返回颜色**——颜色是主题令牌（`ContentColors::tag_dots`），
/// 领域层不该知道屏幕长什么样。
pub fn tag_dot_index(tag: &str) -> usize {
    let root = tag_root(tag);
    let h = root
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    h as usize % TAG_DOT_COUNT
}

/// 侧栏标签圆点的色数（`docs/28` §4.3 `tag.dot`）。
pub const TAG_DOT_COUNT: usize = 6;

// ---------------------------------------------------------------------------
// 笔记库查询（CER-22 · docs/28 §7.1 / §7.11「单一查询入口」）
// ---------------------------------------------------------------------------

/// 查询要读的最小卡片面。
///
/// `NoteCard`（整张卡）与壳层索引里的 `CardMeta`（只有元数据）都实现它，
/// 于是侧栏计数、列表行与预览取自同一份判定——不再出现「收件箱 4 = 全部 4」
/// 这种各栏各自过滤造成的语义分裂。
pub trait CardRef {
    fn card_id(&self) -> &str;
    fn card_title(&self) -> &str;
    fn card_tags(&self) -> &[String];
    fn card_source_node(&self) -> Option<&str>;
    fn card_archived(&self) -> bool;
    fn card_created(&self) -> i64;
    fn card_updated(&self) -> i64;
}

impl CardRef for NoteCard {
    fn card_id(&self) -> &str {
        &self.id
    }
    fn card_title(&self) -> &str {
        &self.title
    }
    fn card_tags(&self) -> &[String] {
        &self.tags
    }
    fn card_source_node(&self) -> Option<&str> {
        self.source_node.as_deref()
    }
    fn card_archived(&self) -> bool {
        self.archived
    }
    fn card_created(&self) -> i64 {
        self.created_at
    }
    fn card_updated(&self) -> i64 {
        self.updated_at
    }
}

/// 侧栏范围（`docs/28` §7.1）。四者互斥地定义在同一份谓词上，计数与列表必然一致。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CardScope {
    /// 全部卡片 = 所有**未归档**卡片。
    #[default]
    All,
    /// 收件箱 = 未归档 ∧ 无标签 ∧ 未挂到任何导图节点（真正「还没整理」的那批）。
    Inbox,
    /// 未打标签 = 未归档 ∧ 标签为空（含已挂导图但未打标签）。
    Untagged,
    /// 归档 = 已归档卡片。
    Archived,
}

impl CardScope {
    pub fn label(self) -> &'static str {
        match self {
            CardScope::All => "全部卡片",
            CardScope::Inbox => "收件箱",
            CardScope::Untagged => "未打标签",
            CardScope::Archived => "归档",
        }
    }

    /// 侧栏四个固定项，顺序即显示顺序。
    pub const ROWS: [CardScope; 4] = [
        CardScope::All,
        CardScope::Inbox,
        CardScope::Untagged,
        CardScope::Archived,
    ];

    pub fn accepts(self, card: &impl CardRef) -> bool {
        let archived = card.card_archived();
        match self {
            CardScope::All => !archived,
            CardScope::Inbox => {
                !archived && card.card_tags().is_empty() && card.card_source_node().is_none()
            }
            CardScope::Untagged => !archived && card.card_tags().is_empty(),
            CardScope::Archived => archived,
        }
    }
}

/// 侧栏四个固定范围的计数。由 [`CardScope::accepts`] 数出来，不是另算一套。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScopeCounts {
    pub all: usize,
    pub inbox: usize,
    pub untagged: usize,
    pub archived: usize,
}

/// 数一遍全部卡片得到四个范围的计数。
pub fn scope_counts(cards: &[impl CardRef]) -> ScopeCounts {
    let mut out = ScopeCounts::default();
    for c in cards {
        out.all += CardScope::All.accepts(c) as usize;
        out.inbox += CardScope::Inbox.accepts(c) as usize;
        out.untagged += CardScope::Untagged.accepts(c) as usize;
        out.archived += CardScope::Archived.accepts(c) as usize;
    }
    out
}

/// 列表排序（`docs/28` §7.6）。搜索时由调用方改按相关度，再回落 [`CardSort::UpdatedDesc`]。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CardSort {
    #[default]
    UpdatedDesc,
    CreatedDesc,
    Title,
}

impl CardSort {
    pub fn label(self) -> &'static str {
        match self {
            CardSort::UpdatedDesc => "最近更新",
            CardSort::CreatedDesc => "最近创建",
            CardSort::Title => "按标题",
        }
    }

    /// 排序行里的三个选项，顺序即菜单顺序。
    pub const OPTIONS: [CardSort; 3] = [
        CardSort::UpdatedDesc,
        CardSort::CreatedDesc,
        CardSort::Title,
    ];

    fn cmp<T: CardRef>(self, a: &T, b: &T) -> std::cmp::Ordering {
        // 每种排序都以 id 收尾：同值时顺序必须稳定，否则滚动/选择会漂。
        match self {
            CardSort::UpdatedDesc => b.card_updated().cmp(&a.card_updated()),
            CardSort::CreatedDesc => b.card_created().cmp(&a.card_created()),
            CardSort::Title => {
                let key = |c: &T| c.card_title().trim().to_lowercase();
                key(a).cmp(&key(b))
            }
        }
        .then_with(|| a.card_id().cmp(b.card_id()))
    }
}

/// 一次笔记库查询：范围 + 标签 AND + 排序（+ 调用方给的搜索命中集）。
///
/// 侧栏计数、列表行、预览都从这里取数（`docs/28` §7.11）。搜索正文要读磁盘，
/// 归壳层的 `NoteStore`；查询只认「哪些 id 命中」，所以领域层不碰文件。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CardQuery {
    pub scope: CardScope,
    /// 标签 AND：每一项按路径匹配，含子标签（`知识` 命中 `知识/方法`）。空 = 不按标签筛。
    pub tags: Vec<String>,
    pub sort: CardSort,
}

/// 标签筛选是否命中一张卡：路径相等或它是被筛路径的子标签。大小写不敏感。
fn tag_matches(card_tag: &str, filter: &str) -> bool {
    let c = card_tag.to_lowercase();
    let f = filter.to_lowercase();
    c == f || c.starts_with(&format!("{f}/"))
}

impl CardQuery {
    pub fn new(scope: CardScope) -> Self {
        Self {
            scope,
            tags: Vec::new(),
            sort: CardSort::default(),
        }
    }

    /// 范围与标签 AND 都过才算命中。搜索命中集由调用方另判（要读正文）。
    pub fn accepts(&self, card: &impl CardRef) -> bool {
        if !self.scope.accepts(card) {
            return false;
        }
        let tags = card.card_tags();
        self.tags
            .iter()
            .all(|f| tags.iter().any(|t| tag_matches(t, f)))
    }

    /// 过滤 + 排序，一次给出列表要显示的顺序。
    pub fn filter<'a, T: CardRef>(&self, cards: &'a [T]) -> Vec<&'a T> {
        let mut out: Vec<&T> = cards.iter().filter(|c| self.accepts(*c)).collect();
        self.sort_in_place(&mut out);
        out
    }

    /// 只排序（搜索时调用方先按相关度排，再对同分项用这里的口径收尾）。
    pub fn sort_in_place<T: CardRef>(&self, rows: &mut [&T]) {
        rows.sort_by(|a, b| self.sort.cmp(*a, *b));
    }

    /// 同 [`Self::sort_in_place`]，给拥有所有权的行用。
    pub fn sort_owned<T: CardRef>(&self, rows: &mut [T]) {
        let sort = self.sort;
        rows.sort_by(|a, b| sort.cmp(a, b));
    }

    /// 裸比较器：调用方手里是下标或相关度加权的行时用它，
    /// 不必为了排序先克隆一遍卡片（一万张时这点很要紧）。
    pub fn cmp_cards<T: CardRef>(&self, a: &T, b: &T) -> std::cmp::Ordering {
        self.sort.cmp(a, b)
    }
}

// ---------------------------------------------------------------------------
// 侧栏标签树（CER-24 · docs/28 §7.3「最多两级」）
// ---------------------------------------------------------------------------

/// 侧栏可见的标签层级数。更深的层级不进侧栏，用搜索/筛选表达（§7.3 / §7.9）。
pub const TAG_TREE_DEPTH: usize = 2;

/// 标签树的一个结点。`count` 已聚合子标签，且只数**未归档**卡片（§7.1「每个标签的未归档数」）。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TagNode {
    pub name: String,
    pub path: String,
    pub count: usize,
    pub children: Vec<TagNode>,
}

/// 拍平后的一行侧栏标签。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatTag {
    pub name: String,
    pub path: String,
    pub count: usize,
    /// 0 = 顶层。侧栏最多到 `TAG_TREE_DEPTH - 1`。
    pub depth: usize,
    /// 是否画 chevron 并可展开。最后一层的结点即便有子标签也是 `false`——
    /// 侧栏不下钻，否则会无限缩进（§7.9）。
    pub expandable: bool,
    /// 圆点取色下标（0..6），同一根标签在侧栏/列表/预览同色。
    pub dot: usize,
}

/// 从卡片建标签树：按 `/` 分层，计数聚合子标签，只数未归档卡片。
pub fn tag_tree(cards: &[impl CardRef]) -> Vec<TagNode> {
    let mut roots: Vec<TagNode> = Vec::new();
    for card in cards {
        if card.card_archived() {
            continue;
        }
        let mut counted: Vec<String> = Vec::new();
        for tag in card.card_tags() {
            let parts = tag_path(tag);
            if parts.is_empty() {
                continue;
            }
            insert_tag_path(&mut roots, &parts, &mut counted, "");
        }
    }
    sort_tag_tree(&mut roots);
    roots
}

fn insert_tag_path(
    nodes: &mut Vec<TagNode>,
    parts: &[&str],
    counted: &mut Vec<String>,
    prefix: &str,
) {
    let Some(&head) = parts.first() else {
        return;
    };
    let path = if prefix.is_empty() {
        head.to_string()
    } else {
        format!("{prefix}/{head}")
    };
    let idx = match nodes.iter().position(|n| n.name == head) {
        Some(i) => i,
        None => {
            nodes.push(TagNode {
                name: head.to_string(),
                path: path.clone(),
                count: 0,
                children: Vec::new(),
            });
            nodes.len() - 1
        }
    };
    // 一张卡带 `知识` 与 `知识/方法` 时，`知识` 只能被数一次。
    if !counted.contains(&path) {
        counted.push(path.clone());
        nodes[idx].count += 1;
    }
    insert_tag_path(&mut nodes[idx].children, &parts[1..], counted, &path);
}

/// 计数多的在前，同计数按名称——稳定、不随机（§7.1）。
fn sort_tag_tree(nodes: &mut Vec<TagNode>) {
    nodes.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    for n in nodes {
        sort_tag_tree(&mut n.children);
    }
}

/// 把树拍平成侧栏行：`collapsed` 里的路径不展开子级，深度封顶 [`TAG_TREE_DEPTH`]。
pub fn flatten_tag_rows(
    nodes: &[TagNode],
    collapsed: &std::collections::HashSet<String>,
    out: &mut Vec<FlatTag>,
) {
    walk_tag_rows(nodes, collapsed, 0, out);
}

fn walk_tag_rows(
    nodes: &[TagNode],
    collapsed: &std::collections::HashSet<String>,
    depth: usize,
    out: &mut Vec<FlatTag>,
) {
    for n in nodes {
        let last_level = depth + 1 >= TAG_TREE_DEPTH;
        out.push(FlatTag {
            name: n.name.clone(),
            path: n.path.clone(),
            count: n.count,
            depth,
            expandable: !last_level && !n.children.is_empty(),
            dot: tag_dot_index(&n.path),
        });
        if !last_level && !n.children.is_empty() && !collapsed.contains(&n.path) {
            walk_tag_rows(&n.children, collapsed, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_trimmed_deduped_and_capped() {
        let tags = vec![
            " Rust ".into(),
            "rust".into(),
            "".into(),
            "知识管理".into(),
            "x".repeat(MAX_TAG_LEN + 1),
        ];
        let got = NoteCard::normalize_tags(&tags);
        assert_eq!(got, vec!["Rust", "知识管理"]);
        let many: Vec<String> = (0..100).map(|i| format!("t{i}")).collect();
        assert_eq!(NoteCard::normalize_tags(&many).len(), MAX_CARD_TAGS);
    }

    #[test]
    fn a_blank_title_still_has_a_readable_name() {
        let c = NoteCard::new("doc", None, "   ", "", 0);
        assert_eq!(c.display_title(), "未命名卡片");
    }

    #[test]
    fn excerpt_drops_markdown_noise_and_stays_single_line() {
        let c = NoteCard::new(
            "doc",
            None,
            "T",
            "# 标题\n\n> 引用\n- 列表项\n```rust\nlet x = 1;\n```\n**加粗** 正文",
            0,
        );
        let e = c.excerpt(40);
        assert!(
            !e.contains('#') && !e.contains('`') && !e.contains('\n'),
            "{e:?}"
        );
        assert!(e.contains("标题") && e.contains("加粗"), "{e:?}");
    }

    #[test]
    fn excerpt_drops_task_list_checkboxes() {
        let c = NoteCard::new("doc", None, "T", "- [ ] 待办\n- [x] 已办\n正文", 0);
        let e = c.excerpt(40);
        assert!(!e.contains('[') && !e.contains(']'), "{e:?}");
        assert_eq!(e, "待办 已办 正文");
    }

    #[test]
    fn validation_rejects_oversized_values() {
        let mut c = NoteCard::new("doc", None, "T", "", 0);
        c.body = "x".repeat(MAX_CARD_BODY + 1);
        assert!(c.validate().is_err());
        c.body.clear();
        c.title = "x".repeat(MAX_CARD_TITLE + 1);
        assert!(c.validate().is_err());
        c.title.clear();
        c.id.clear();
        assert!(c.validate().is_err());
    }

    #[test]
    fn tag_hierarchy_splits_on_slash_and_ignores_empty_segments() {
        assert_eq!(tag_path("知识/方法/阅读"), vec!["知识", "方法", "阅读"]);
        assert_eq!(tag_path(" / 方法 / "), vec!["方法"]);
        assert_eq!(tag_parent("知识/方法"), Some("知识"));
        assert_eq!(tag_parent("知识"), None);
        assert_eq!(tag_root("知识/方法"), "知识");
        assert_eq!(tag_root(""), "");
    }

    // -------------------------------------------------- CER-22 查询与范围边界

    /// 一张卡片：只填查询要读的那几项。
    fn card(id: &str, tags: &[&str], node: Option<&str>, archived: bool) -> NoteCard {
        NoteCard {
            id: id.into(),
            title: id.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            source_node: node.map(str::to_string),
            archived,
            ..Default::default()
        }
    }

    /// 出厂即未归档；键缺失时的落盘/读回行为在 `mw-format` 里钉（那才有 serde_json）。
    #[test]
    fn new_cards_start_unarchived() {
        assert!(!NoteCard::default().archived);
        assert!(!NoteCard::new("d", None, "T", "B", 0).archived);
    }

    /// 合同 §7.1 点名要修的错误语义：「收件箱 = 全部卡片」。
    /// 有标签但没挂节点的卡片属于全部、不属于收件箱。
    #[test]
    fn inbox_is_a_strict_subset_of_all() {
        let cards = vec![
            card("裸", &[], None, false),
            card("有标签", &["知识"], None, false),
            card("挂了节点", &[], Some("n1"), false),
            card("归档的裸卡", &[], None, true),
        ];
        let c = scope_counts(&cards);
        assert_eq!((c.all, c.inbox, c.untagged, c.archived), (3, 1, 2, 1));
        assert_ne!(c.all, c.inbox, "收件箱不能再等于全部");
        // 计数与谓词必须同源：逐条数一遍要对得上。
        for scope in CardScope::ROWS {
            let n = cards.iter().filter(|x| scope.accepts(*x)).count();
            let recorded = match scope {
                CardScope::All => c.all,
                CardScope::Inbox => c.inbox,
                CardScope::Untagged => c.untagged,
                CardScope::Archived => c.archived,
            };
            assert_eq!(n, recorded, "{} 的计数与谓词不一致", scope.label());
        }
    }

    /// 归档只从「归档」这一项出现；其余三项一律排除它。
    #[test]
    fn archived_cards_leave_every_scope_except_archive() {
        let a = card("a", &["知识"], Some("n1"), true);
        assert!(CardScope::Archived.accepts(&a));
        for scope in [CardScope::All, CardScope::Inbox, CardScope::Untagged] {
            assert!(!scope.accepts(&a), "{} 不该收归档卡", scope.label());
        }
        // 归档 + 无标签：仍是归档，不进未打标签。
        assert!(!CardScope::Untagged.accepts(&card("b", &[], None, true)));
    }

    /// 「未打标签」含已挂导图但没打标签的卡片（§7.1 括号里那句）。
    #[test]
    fn untagged_includes_attached_cards() {
        let attached = card("a", &[], Some("n1"), false);
        assert!(CardScope::Untagged.accepts(&attached));
        assert!(!CardScope::Inbox.accepts(&attached), "挂了节点就不在收件箱");
    }

    /// 多标签筛选是 AND，不是 OR；每个条目命中子标签。
    #[test]
    fn tag_filter_is_and_and_reaches_descendants() {
        let cards = vec![
            card("both", &["知识/方法", "项目/织思"], None, false),
            card("only-k", &["知识"], None, false),
            card("only-p", &["项目/织思"], None, false),
        ];
        let q = CardQuery {
            scope: CardScope::All,
            tags: vec!["知识".into(), "项目".into()],
            sort: CardSort::default(),
        };
        let got: Vec<&str> = q.filter(&cards).iter().map(|c| c.card_id()).collect();
        assert_eq!(got, vec!["both"], "两个标签必须同时命中");

        let single = CardQuery {
            tags: vec!["知识".into()],
            ..Default::default()
        };
        let got: Vec<&str> = single.filter(&cards).iter().map(|c| c.card_id()).collect();
        assert_eq!(got, vec!["both", "only-k"], "`知识` 要带上 `知识/方法`");
    }

    /// 排序必须以 id 收尾：同值时顺序不能随输入抖动，否则选择/滚动会漂。
    #[test]
    fn sorts_are_total_and_stable() {
        let mut cards = vec![
            NoteCard {
                id: "b".into(),
                updated_at: 5,
                ..Default::default()
            },
            NoteCard {
                id: "a".into(),
                updated_at: 5,
                ..Default::default()
            },
        ];
        for sort in CardSort::OPTIONS {
            let q = CardQuery {
                sort,
                ..Default::default()
            };
            q.sort_owned(&mut cards);
            let mut again = cards.clone();
            q.sort_owned(&mut again);
            assert_eq!(cards, again, "{sort:?} 不稳");
            let ids: Vec<&str> = cards.iter().map(|c| c.id.as_str()).collect();
            assert_eq!(ids, vec!["a", "b"], "{sort:?} 同值时应按 id 收尾");
        }
        // 标题排序不看大小写，也不受首尾空白影响。
        let mut titled = vec![
            NoteCard {
                id: "1".into(),
                title: " beta ".into(),
                ..Default::default()
            },
            NoteCard {
                id: "2".into(),
                title: "Alpha".into(),
                ..Default::default()
            },
        ];
        CardQuery {
            sort: CardSort::Title,
            ..Default::default()
        }
        .sort_owned(&mut titled);
        assert_eq!(titled[0].id, "2", "Alpha 应排在 beta 前");
    }

    /// 侧栏最多两级：第三层不进侧栏，但计数要聚合上来；归档卡不计数。
    #[test]
    fn tag_tree_caps_at_two_levels_and_counts_unarchived_only() {
        let cards = vec![
            card("a", &["知识/方法/阅读"], None, false),
            card("b", &["知识/方法"], None, false),
            card("c", &["知识"], None, false),
            card("d", &["知识"], None, true),
            card("e", &["项目"], None, false),
        ];
        let tree = tag_tree(&cards);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "知识");
        assert_eq!(tree[0].count, 3, "归档的那张不该被数进来");
        assert_eq!(tree[1].name, "项目");

        let mut rows = Vec::new();
        flatten_tag_rows(&tree, &Default::default(), &mut rows);
        let paths: Vec<(&str, usize, bool)> = rows
            .iter()
            .map(|r| (r.path.as_str(), r.depth, r.expandable))
            .collect();
        assert_eq!(
            paths,
            vec![
                ("知识", 0, true),
                ("知识/方法", 1, false),
                ("项目", 0, false)
            ],
            "第三层「阅读」不能出现在侧栏，第二层也不该再有 chevron"
        );
        assert_eq!(rows[1].count, 2, "「知识/方法」要聚合它下面的「阅读」");

        // 折叠顶层后子级消失。
        let mut collapsed = std::collections::HashSet::new();
        collapsed.insert("知识".to_string());
        let mut rows = Vec::new();
        flatten_tag_rows(&tree, &collapsed, &mut rows);
        assert_eq!(
            rows.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            vec!["知识", "项目"]
        );
    }

    /// 圆点取色下标：同根稳定、落在 0..6、不同根会分开。
    #[test]
    fn tag_dot_index_is_stable_within_six() {
        for tag in ["知识", "知识/方法", "知识/方法/阅读"] {
            let i = tag_dot_index(tag);
            assert!(i < TAG_DOT_COUNT, "{tag} → {i}");
            assert_eq!(i, tag_dot_index(tag), "同一个标签两次取色必须一致");
        }
        assert_eq!(tag_dot_index("知识/方法"), tag_dot_index("知识/别的"));
        let spread: std::collections::HashSet<usize> = (0..40)
            .map(|i| tag_dot_index(&format!("标签{i}")))
            .collect();
        assert!(spread.len() > 1, "40 个根标签不该全落同一支色");
    }
}
