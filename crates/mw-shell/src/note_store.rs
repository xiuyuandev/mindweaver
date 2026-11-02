//! 本地卡片库：`%APPDATA%\织思\notes` 下每张卡片一个 Markdown 文件，外加 `index.json`。
//!
//! 为什么要有库：卡片不能只活在某一份 `.zmind` 里。用户需要跨文档浏览、按标签筛选、
//! 全文搜索、把卡片带到另一张图里。`.zmind` 内嵌卡片保证"文件带走内容"，本地库保证
//! "内容可被统一管理"；两边用同一套 Markdown 编码（`mw_format::card_markdown`）。
//!
//! 磁盘是权威，索引只是缓存：索引坏了就扫盘重建，最多多花几百毫秒，不会丢数据。
//! 写文件走「临时文件 → 旧文件备份 → 更名」，进程中途退出不会留下半张卡片。

// P08 先把库的读写/索引做完整；列表、打开、编辑入口在 P09–P15 才接 UI。
// 这段 allow 只覆盖尚未接线的公开 API，接完就删。
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use mw_core::note::CardRef;
use mw_core::search::SearchIndex;
use mw_core::{Node, NoteCard};
use serde::{Deserialize, Serialize};

const INDEX_VERSION: u32 = 1;
const INDEX_FILE: &str = "index.json";

/// 索引里的一张卡片摘要：不含正文，列表页够用，打开时再读文件。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct CardMeta {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub source_doc: String,
    pub source_node: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// 纯文本摘要（列表/搜索预览用），避免每次读整个正文。
    pub excerpt: String,
    /// 相对 `root` 的文件名，只允许单层文件名。
    pub file: String,
    /// 已归档：只在侧栏「归档」下出现（`docs/28` §7.1）。
    /// 老索引缺键即未归档，所以不升 `INDEX_VERSION`、不写迁移。
    #[serde(skip_serializing_if = "is_false")]
    pub archived: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl CardMeta {
    fn from_card(card: &NoteCard, file: String) -> Self {
        Self {
            id: card.id.clone(),
            title: card.display_title().to_string(),
            tags: card.tags.clone(),
            source_doc: card.source_doc.clone(),
            source_node: card.source_node.clone(),
            created_at: card.created_at,
            updated_at: card.updated_at,
            excerpt: card.excerpt(120),
            file,
            archived: card.archived,
        }
    }
}

/// 列表/侧栏/预览都只认 `CardQuery`，所以摘要必须能直接喂给它（`docs/28` §7.11）。
impl CardRef for CardMeta {
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

/// 一批卡片改完之后，逐 id 的结果。
///
/// 契约要求「不允许静默部分成功」（`docs/28` §7.11）：UI 要能说
/// 「已归档 8 张，2 张失败」，所以失败项连原因一起带回去。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BatchReport {
    /// 真的写盘改了的 id。
    pub changed: Vec<String>,
    /// 已经是目标状态、没动盘的 id。
    pub skipped: Vec<String>,
    /// 失败的 id 与原因。
    pub failed: Vec<(String, String)>,
    /// 卡片文件都写成功了，但索引没落盘（下次 `rebuild()` 自愈）。
    pub index_lagged: bool,
}

impl BatchReport {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.skipped.is_empty() && self.failed.is_empty()
    }
    pub fn changed_count(&self) -> usize {
        self.changed.len()
    }
    pub fn failed_count(&self) -> usize {
        self.failed.len()
    }
    /// 第一条失败原因，给 toast 用（完整清单在报告里）。
    pub fn first_error(&self) -> Option<&str> {
        self.failed.first().map(|(_, e)| e.as_str())
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct IndexFile {
    version: u32,
    cards: Vec<CardMeta>,
}

/// 一次卡片全文搜索命中。片段已围绕命中位置截好，UI 直接显示。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardHit {
    pub id: String,
    pub title: String,
    pub before: String,
    pub matched: String,
    pub after: String,
    pub score: i32,
    /// text / pinyin / initials
    pub kind: String,
}

/// 本地卡片库。
pub struct NoteStore {
    root: PathBuf,
    cards: BTreeMap<String, CardMeta>,
    /// 正文缓存（`title\nbody`），搜索时懒加载；1000 张约几 MB。
    texts: BTreeMap<String, String>,
    search: SearchIndex,
    /// 笔记库专用索引：文本里还拼了标签与来源节点标题（`docs/28` §7.6）。
    ///
    /// 与 `search` 分开是因为 `SearchIndex` 按「id → 文本快照」缓存拼音键，
    /// 两份口径不同的文本共用一个实例会每帧互相踢掉缓存。
    library_search: SearchIndex,
    /// 上次扫描/加载跳过的坏文件数量。UI 可以据此提示"有 N 张卡片读取失败"。
    errors: usize,
}

impl NoteStore {
    /// 默认库；没有 `%APPDATA%` 时返回 `None`（便携/测试环境不强制写用户目录）。
    pub fn open_default() -> Option<Self> {
        let dir = crate::paths::notes_dir()?;
        Some(Self::open_at(dir))
    }

    /// 指定目录打开。索引不存在/损坏就扫盘重建；单个坏文件跳过，不拖垮整个库。
    pub fn open_at(root: PathBuf) -> Self {
        let mut store = Self {
            root,
            cards: BTreeMap::new(),
            texts: BTreeMap::new(),
            search: SearchIndex::default(),
            library_search: SearchIndex::default(),
            errors: 0,
        };
        if !store.load_index() {
            store.rebuild();
        }
        store
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// 卡片 id（BTreeMap 顺序，稳定）。
    pub fn ids(&self) -> Vec<String> {
        self.cards.keys().cloned().collect()
    }

    /// 知识视图输入：只取元数据，不读正文文件。
    pub fn graph_input(&self) -> Vec<crate::knowledge::CardInput> {
        self.cards
            .values()
            .map(|c| crate::knowledge::CardInput {
                id: c.id.clone(),
                title: c.title.clone(),
                tags: c.tags.clone(),
                source_node: c.source_node.clone(),
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn errors(&self) -> usize {
        self.errors
    }

    /// 按更新时间倒序（同时间按 id 稳定排序）。
    pub fn list(&self) -> Vec<&CardMeta> {
        let mut out: Vec<&CardMeta> = self.cards.values().collect();
        out.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        out
    }

    pub fn meta(&self, id: &str) -> Option<&CardMeta> {
        self.cards.get(id)
    }

    /// 读整张卡片（正文来自磁盘）。坏文件返回 `None`，不 panic。
    pub fn get(&self, id: &str) -> Option<NoteCard> {
        let meta = self.cards.get(id)?;
        let path = self.root.join(meta.file.as_str());
        let text = std::fs::read_to_string(path).ok()?;
        mw_format::card_from_markdown(&text).ok()
    }

    /// 写一张卡片（新增或覆盖）并刷新索引。
    pub fn upsert(&mut self, card: &NoteCard) -> Result<(), String> {
        card.validate()?;
        self.write_card(card)?;
        let file = file_name(&card.id);
        self.cards
            .insert(card.id.clone(), CardMeta::from_card(card, file));
        self.texts.insert(card.id.clone(), card_text(card));
        self.save_index()
    }

    /// 把一张卡片挂到文档节点上：写 `source_doc/source_node`，失败不改内存索引。
    ///
    /// 与「归档」（[`NoteStore::set_archived`]）是两件事：挂节点说明它被整理进
    /// 了某张图，归档是用户把它从列表里收起来。旧名字 `archive_to_node` 撞了
    /// `docs/28` §7.1 的归档语义，故改名。
    pub fn attach_to_node(
        &mut self,
        card_id: &str,
        doc_id: &str,
        node_id: &str,
        now: i64,
    ) -> Result<NoteCard, String> {
        let mut card = self.get(card_id).ok_or_else(|| "卡片不存在".to_string())?;
        card.source_doc = doc_id.to_string();
        card.source_node = Some(node_id.to_string());
        card.updated_at = now;
        self.upsert(&card)?;
        Ok(card)
    }

    /// 摘掉来源节点（保留卡片与标签）。
    pub fn detach_from_node(&mut self, card_id: &str, now: i64) -> Result<NoteCard, String> {
        let mut card = self.get(card_id).ok_or_else(|| "卡片不存在".to_string())?;
        card.source_node = None;
        card.updated_at = now;
        self.upsert(&card)?;
        Ok(card)
    }

    pub fn metas(&self) -> Vec<CardMeta> {
        self.cards.values().cloned().collect()
    }

    /// 删除一张卡片。文件不存在也把索引项清掉。
    pub fn remove(&mut self, id: &str) -> bool {
        let Some(meta) = self.cards.remove(id) else {
            return false;
        };
        self.texts.remove(id);
        let _ = std::fs::remove_file(self.root.join(meta.file));
        let _ = self.save_index();
        true
    }

    /// 全库标签聚合：大小写无关合并，保留首次出现的写法；按使用次数降序。
    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut map: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for meta in self.cards.values() {
            for tag in &meta.tags {
                let key = tag.to_lowercase();
                let e = map.entry(key).or_insert_with(|| (tag.clone(), 0));
                e.1 += 1;
            }
        }
        let mut out: Vec<(String, usize)> = map.into_values().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out
    }

    /// 全库全文搜索：复用 `mw-core` 的文本 + 拼音/首字母索引。
    /// 标签命中：返回 (标签, 卡片 id)，按标签去重、最多 4 条。
    pub fn search_tags(&self, query: &str) -> Vec<(String, String)> {
        let q = query.trim().trim_start_matches('#').to_lowercase();
        let mut out: Vec<(String, String)> = Vec::new();
        if q.is_empty() {
            return out;
        }
        for card in self.cards.values() {
            for tag in &card.tags {
                if tag.to_lowercase().contains(&q) && out.iter().all(|(t, _)| t != tag) {
                    out.push((tag.clone(), card.id.clone()));
                }
            }
        }
        out.truncate(4);
        out
    }

    pub fn search(&mut self, query: &str, limit: usize, mode: mw_core::MatchMode) -> Vec<CardHit> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }
        self.ensure_texts();
        let mut nodes: HashMap<String, Node> = HashMap::with_capacity(self.cards.len());
        let mut order: Vec<String> = Vec::with_capacity(self.cards.len());
        for id in self.cards.keys() {
            let text = self.texts.get(id).cloned().unwrap_or_default();
            nodes.insert(
                id.clone(),
                Node {
                    id: id.clone(),
                    text,
                    ..Default::default()
                },
            );
            order.push(id.clone());
        }
        let hits = self.search.hits(&nodes, &order, q, mode);
        hits.into_iter()
            .take(limit)
            .filter_map(|h| {
                let meta = self.cards.get(&h.id)?;
                let text = self.texts.get(&h.id)?;
                let (before, matched, after) = snippet(text, h.range, 36, 64);
                Some(CardHit {
                    id: h.id,
                    title: meta.title.clone(),
                    before,
                    matched,
                    after,
                    score: h.score,
                    kind: h.kind,
                })
            })
            .collect()
    }

    fn ensure_texts(&mut self) {
        let missing: Vec<String> = self
            .cards
            .keys()
            .filter(|id| !self.texts.contains_key(*id))
            .cloned()
            .collect();
        for id in missing {
            if let Some(card) = self.get(&id) {
                self.texts.insert(id, card_text(&card));
            }
        }
    }

    /// 笔记库检索（`docs/28` §7.6）：标题、正文、标签、来源节点标题都要能命中。
    ///
    /// 返回「命中 id → 相关度」，作用域与标签过滤交给 [`mw_core::note::CardQuery`]——
    /// 检索自己不判定作用域，否则「在归档里搜」就永远搜不到东西。相关度用于
    /// §7.6「搜索时相关度优先」。`node_title` 由调用方喂（只有壳层拿得到文档）：
    /// 跨文档的卡片查不到节点标题，返回 `None` 即不参与这一路命中。
    pub(crate) fn library_hits<'n>(
        &mut self,
        query: &str,
        mode: mw_core::MatchMode,
        node_title: impl Fn(&str) -> Option<&'n str>,
    ) -> HashMap<String, i32> {
        let q = query.trim();
        if q.is_empty() {
            return HashMap::new();
        }
        self.ensure_texts();
        let mut nodes: HashMap<String, Node> = HashMap::with_capacity(self.cards.len());
        let mut order: Vec<String> = Vec::with_capacity(self.cards.len());
        for (id, meta) in &self.cards {
            let mut text = self.texts.get(id).cloned().unwrap_or_default();
            if !meta.tags.is_empty() {
                text.push('\n');
                text.push_str(&meta.tags.join(" "));
            }
            if let Some(title) = meta.source_node.as_deref().and_then(&node_title) {
                text.push('\n');
                text.push_str(title);
            }
            nodes.insert(
                id.clone(),
                Node {
                    id: id.clone(),
                    text,
                    ..Default::default()
                },
            );
            order.push(id.clone());
        }
        self.library_search
            .hits(&nodes, &order, q, mode)
            .into_iter()
            .map(|h| (h.id, h.score))
            .collect()
    }

    /// 批量改卡片：逐个「读盘 → 改 → 校验 → 写盘」，最后只落一次索引。
    ///
    /// `edit` 返回 `Ok(false)` 表示已经是目标状态（记 `skipped`），`Err` 记 `failed`。
    /// 某一张失败不影响其余：契约要的是逐 id 报告，不是整批回滚——回滚反而会把
    /// 已经写好的卡片文件留在盘上，变成"报告说没改、盘上改了"。
    fn mutate_many<F>(&mut self, ids: &[String], mut edit: F) -> BatchReport
    where
        F: FnMut(&mut NoteCard) -> Result<bool, String>,
    {
        let mut report = BatchReport::default();
        for id in ids {
            let Some(mut card) = self.get(id) else {
                report
                    .failed
                    .push((id.clone(), "卡片不存在或读不出来".to_string()));
                continue;
            };
            match edit(&mut card) {
                Ok(false) => report.skipped.push(id.clone()),
                Err(e) => report.failed.push((id.clone(), e)),
                Ok(true) => {
                    if let Err(e) = card.validate() {
                        report.failed.push((id.clone(), e));
                        continue;
                    }
                    if let Err(e) = self.write_card(&card) {
                        report.failed.push((id.clone(), e));
                        continue;
                    }
                    self.cards.insert(
                        card.id.clone(),
                        CardMeta::from_card(&card, file_name(&card.id)),
                    );
                    self.texts.insert(card.id.clone(), card_text(&card));
                    report.changed.push(id.clone());
                }
            }
        }
        if !report.changed.is_empty() && self.save_index().is_err() {
            // 卡片文件都写好了，只是索引落后：下次 `rebuild()` 自愈，但要让 UI 说一声。
            report.index_lagged = true;
        }
        report
    }

    /// 归档 / 移出归档（`docs/28` §7.4 批量条）。
    ///
    /// 归档是「收起来」不是「改内容」，所以**不碰 `updated_at`**：否则按
    /// 「最近更新」排序时，归档这个动作会把卡片顶到列表最前面。
    pub(crate) fn set_archived(&mut self, ids: &[String], archived: bool) -> BatchReport {
        self.mutate_many(ids, move |card| {
            if card.archived == archived {
                return Ok(false);
            }
            card.archived = archived;
            Ok(true)
        })
    }

    /// 批量加标签。要的标签先过 `normalize_tags`（去空、去重、限个数），
    /// 一个都不合法就整批判失败——静默丢掉用户输入的标签比报错更糟。
    pub(crate) fn add_tags(&mut self, ids: &[String], tags: &[String], now: i64) -> BatchReport {
        let wanted = NoteCard::normalize_tags(tags);
        if wanted.is_empty() {
            return BatchReport {
                failed: ids
                    .iter()
                    .map(|id| (id.clone(), "标签为空或超过长度上限".to_string()))
                    .collect(),
                ..Default::default()
            };
        }
        self.mutate_many(ids, move |card| {
            let mut merged = card.tags.clone();
            merged.extend(wanted.iter().cloned());
            let normalized = NoteCard::normalize_tags(&merged);
            if normalized == card.tags {
                return Ok(false);
            }
            card.tags = normalized;
            card.updated_at = now;
            Ok(true)
        })
    }

    /// 批量删标签。整串大小写无关比较：删「知识」不该顺手删掉「知识/方法」。
    pub(crate) fn remove_tags(&mut self, ids: &[String], tags: &[String], now: i64) -> BatchReport {
        let drop: Vec<String> = tags.iter().map(|t| t.trim().to_lowercase()).collect();
        self.mutate_many(ids, move |card| {
            let kept: Vec<String> = card
                .tags
                .iter()
                .filter(|t| !drop.contains(&t.trim().to_lowercase()))
                .cloned()
                .collect();
            if kept.len() == card.tags.len() {
                return Ok(false);
            }
            card.tags = kept;
            card.updated_at = now;
            Ok(true)
        })
    }

    /// 批量移出本地库。**只删库里的 Markdown 文件**，不碰 `.zmind` 内嵌副本；
    /// 确认框的措辞由 UI 负责说清楚（见 `note_library`）。
    pub(crate) fn remove_many(&mut self, ids: &[String]) -> BatchReport {
        let mut report = BatchReport::default();
        for id in ids {
            let Some(meta) = self.cards.remove(id) else {
                report.failed.push((id.clone(), "卡片不在库里".to_string()));
                continue;
            };
            self.texts.remove(id);
            if let Err(e) = std::fs::remove_file(self.root.join(&meta.file)) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    report.failed.push((id.clone(), e.to_string()));
                    // 索引项已经摘掉了，文件却删不掉：塞回去，别报"移出成功"。
                    self.cards.insert(id.clone(), meta);
                    continue;
                }
            }
            report.changed.push(id.clone());
        }
        if !report.changed.is_empty() && self.save_index().is_err() {
            report.index_lagged = true;
        }
        report
    }

    ///
    /// 每张卡都会重写 Markdown 文件，最后统一落一次索引；中途失败会保留已写的卡片，
    /// 但索引可能落后，下次 `rebuild()` 会自愈。
    pub fn rename_tag(&mut self, from: &str, to: &str) -> Result<usize, String> {
        let from = from.trim();
        let Some(to) = NoteCard::normalize_tags(&[to.to_string()])
            .into_iter()
            .next()
        else {
            return Err("新标签不能为空，且不能超过长度上限".into());
        };
        if from.eq_ignore_ascii_case(&to) {
            return Ok(0);
        }
        let ids: Vec<String> = self
            .cards
            .iter()
            .filter(|(_, m)| m.tags.iter().any(|t| t.eq_ignore_ascii_case(from)))
            .map(|(id, _)| id.clone())
            .collect();
        let mut changed = 0;
        for id in ids {
            let Some(mut card) = self.get(&id) else {
                continue;
            };
            let mut tags = card.tags.clone();
            for t in tags.iter_mut() {
                if t.eq_ignore_ascii_case(from) {
                    *t = to.clone();
                }
            }
            card.tags = NoteCard::normalize_tags(&tags);
            card.updated_at = mw_core::now_ms();
            self.write_card(&card)?;
            self.cards.insert(
                card.id.clone(),
                CardMeta::from_card(&card, file_name(&card.id)),
            );
            changed += 1;
        }
        if changed > 0 {
            self.save_index()?;
        }
        Ok(changed)
    }

    /// 每日笔记：标题是日期、带 `每日` 标签；同一天重复调用返回同一张卡。
    pub fn daily_note(&mut self, date: &str, now: i64) -> Result<NoteCard, String> {
        let date = date.trim();
        if date.is_empty() {
            return Err("日期为空".into());
        }
        if let Some(meta) = self
            .cards
            .values()
            .find(|m| m.title == date && m.tags.iter().any(|t| t == "每日"))
        {
            if let Some(card) = self.get(&meta.id) {
                return Ok(card);
            }
        }
        let mut card = NoteCard::new(String::new(), None, date, "", now);
        card.tags = vec!["每日".into()];
        card.created_at = now;
        card.updated_at = now;
        self.upsert(&card)?;
        Ok(card)
    }

    /// 把文档内嵌的卡片合并进库：库里没有、或文档那份更新，才覆盖库。
    ///
    /// 反过来（库里更新）不自动写回文档——文档是便携副本，库是工作副本；
    /// 打开文档时的合并策略在 P09 决定。这里只保证"不会用旧文档覆盖新库"。
    pub fn sync_document(&mut self, cards: &[NoteCard]) -> Result<usize, String> {
        let mut written = 0;
        for card in cards {
            if card.validate().is_err() {
                continue;
            }
            let newer = self
                .cards
                .get(&card.id)
                .is_none_or(|m| m.updated_at < card.updated_at);
            if !newer {
                continue;
            }
            // 归档是**库**的状态（`docs/32` §14）：文档副本不携带归档命令，也不写这个字段。
            // 文档份更新时只用它的正文/标题/标签覆盖，归档位保留库里的值——否则在库里
            // 归档过的卡片，只要还挂在节点上被改一个字，就会悄悄回到「全部卡片」（CER-32 F6）。
            let archived = self
                .cards
                .get(&card.id)
                .map(|m| m.archived)
                .unwrap_or(card.archived);
            let mut merged = card.clone();
            merged.archived = archived;
            self.write_card(&merged)?;
            let file = file_name(&card.id);
            self.cards
                .insert(card.id.clone(), CardMeta::from_card(&merged, file));
            self.texts.insert(card.id.clone(), card_text(&merged));
            written += 1;
        }
        if written > 0 {
            self.save_index()?;
        }
        Ok(written)
    }

    /// 扫盘重建索引。坏文件只计数，不中断。
    pub fn rebuild(&mut self) -> usize {
        self.cards.clear();
        self.texts.clear();
        self.errors = 0;
        if std::fs::create_dir_all(&self.root).is_err() {
            return 0;
        }
        let Ok(read) = std::fs::read_dir(&self.root) else {
            return 0;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                self.errors += 1;
                continue;
            };
            match mw_format::card_from_markdown(&text) {
                Ok(card) => {
                    self.texts.insert(card.id.clone(), card_text(&card));
                    self.cards.insert(
                        card.id.clone(),
                        CardMeta::from_card(&card, name.to_string()),
                    );
                }
                Err(_) => self.errors += 1,
            }
        }
        let _ = self.save_index();
        self.cards.len()
    }

    fn load_index(&mut self) -> bool {
        let Ok(text) = std::fs::read_to_string(self.root.join(INDEX_FILE)) else {
            return false;
        };
        let Ok(index) = serde_json::from_str::<IndexFile>(&text) else {
            return false;
        };
        if index.version != INDEX_VERSION {
            return false;
        }
        self.cards = index.cards.into_iter().map(|m| (m.id.clone(), m)).collect();
        self.texts.clear();
        true
    }

    fn save_index(&self) -> Result<(), String> {
        let index = IndexFile {
            version: INDEX_VERSION,
            cards: self.cards.values().cloned().collect(),
        };
        let json = serde_json::to_vec_pretty(&index).map_err(|e| e.to_string())?;
        write_atomic(&self.root.join(INDEX_FILE), &json).map_err(|e| e.to_string())
    }

    fn write_card(&self, card: &NoteCard) -> Result<(), String> {
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.root.join(file_name(&card.id));
        let text = mw_format::card_markdown(card);
        write_atomic(&path, text.as_bytes()).map_err(|e| e.to_string())
    }
}

/// 卡片 id → 单层文件名。只留 ASCII 字母数字/`-_`，防路径穿越。
fn card_text(card: &NoteCard) -> String {
    format!("{}\n{}", card.title, card.body)
}

/// 围绕命中区间截前后文；区间按字符下标（`mw-core` 的命中就是字符下标）。
fn snippet(
    text: &str,
    range: Option<[usize; 2]>,
    before_chars: usize,
    after_chars: usize,
) -> (String, String, String) {
    let chars: Vec<char> = text.chars().collect();
    let [s, e] = range.unwrap_or([0, 0]);
    let s = s.min(chars.len());
    let e = e.min(chars.len()).max(s);
    let start = s.saturating_sub(before_chars);
    let end = (e + after_chars).min(chars.len());
    let mut before: String = chars[start..s].iter().collect();
    let matched: String = chars[s..e].iter().collect();
    let mut after: String = chars[e..end].iter().collect();
    if start > 0 {
        before.insert(0, '…');
    }
    if end < chars.len() {
        after.push('…');
    }
    (before, matched, after)
}

fn file_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.md", if safe.is_empty() { "card" } else { &safe })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("card");
    let tmp = dir.join(format!(".{name}.tmp"));
    std::fs::write(&tmp, bytes)?;
    let bak = dir.join(format!(".{name}.bak"));
    if path.exists() {
        let _ = std::fs::remove_file(&bak);
        std::fs::rename(path, &bak)?;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::rename(&bak, path);
        return Err(e);
    }
    let _ = std::fs::remove_file(&bak);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::note::CardQuery;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-notes-{}", mw_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn card(i: usize, updated: i64) -> NoteCard {
        let mut c = NoteCard::new(
            "doc-1",
            Some(format!("node-{i}")),
            format!("卡片 {i}"),
            format!("# 标题 {i}\n\n正文"),
            updated,
        );
        c.tags = vec!["知识".into(), format!("t{}", i % 3)];
        c.updated_at = updated;
        c
    }

    #[test]
    fn cards_round_trip_and_index_is_written() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let c = card(1, 10);
        store.upsert(&c).unwrap();
        assert_eq!(store.len(), 1);
        assert!(dir.join(INDEX_FILE).exists(), "索引要落盘");
        assert!(dir.join(file_name(&c.id)).exists(), "卡片 Markdown 要落盘");
        assert_eq!(store.get(&c.id), Some(c.clone()));
        assert_eq!(store.list()[0].excerpt, "标题 1 正文");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_tag_into_existing_merges_without_losing_cards() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let mut a = card(1, 1);
        a.tags = vec!["旧".into(), "保留".into()];
        let mut b = card(2, 2);
        b.tags = vec!["新".into()];
        store.upsert(&a).unwrap();
        store.upsert(&b).unwrap();
        let n = store.rename_tag("旧", "新").unwrap();
        assert_eq!(n, 1);
        let a2 = store.get(&a.id).unwrap();
        assert!(a2.tags.contains(&"新".to_string()));
        assert!(!a2.tags.contains(&"旧".to_string()));
        assert!(a2.tags.contains(&"保留".to_string()));
        let b2 = store.get(&b.id).unwrap();
        assert_eq!(b2.tags, vec!["新".to_string()], "已有标签不能重复");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn attach_then_detach_round_trips_the_source() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let c = card(1, 10);
        store.upsert(&c).unwrap();
        let attached = store.attach_to_node(&c.id, "doc-2", "node-9", 20).unwrap();
        assert_eq!(attached.source_doc, "doc-2");
        assert_eq!(attached.source_node.as_deref(), Some("node-9"));
        let back = store.detach_from_node(&c.id, 30).unwrap();
        assert!(back.source_node.is_none());
        let reloaded = NoteStore::open_at(dir.clone());
        assert!(reloaded.get(&c.id).unwrap().source_node.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 归档只收起来，不改内容也不改 `updated_at`（否则列表会被归档动作顶到最前）。
    #[test]
    fn archiving_keeps_updated_at_and_survives_reopen() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let c = card(1, 10);
        store.upsert(&c).unwrap();
        let report = store.set_archived(std::slice::from_ref(&c.id), true);
        assert_eq!(report.changed, vec![c.id.clone()]);
        assert!(report.failed.is_empty());
        assert_eq!(store.meta(&c.id).unwrap().updated_at, 10);
        assert!(store.meta(&c.id).unwrap().archived);
        drop(store);

        let reopened = NoteStore::open_at(dir.clone());
        assert!(reopened.meta(&c.id).unwrap().archived, "索引要记住归档");
        let mut rebuilt = reopened;
        assert_eq!(rebuilt.rebuild(), 1);
        assert!(
            rebuilt.meta(&c.id).unwrap().archived,
            "扫盘也要从 front matter 读回归档"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CER-32 F6：文档份更新覆盖库时，库里的归档位不能被冲掉。
    #[test]
    fn syncing_a_newer_document_card_keeps_the_library_archived_flag() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let c = card(1, 10);
        store.upsert(&c).unwrap();
        store.set_archived(std::slice::from_ref(&c.id), true);
        assert!(store.meta(&c.id).unwrap().archived);

        // 文档副本（同一个 id、更新的 updated_at、改了正文），归档位仍是 false。
        let mut from_doc = c.clone();
        from_doc.body = "# 标题 1\n\n改过一个字".into();
        from_doc.updated_at = 20;
        assert!(!from_doc.archived);
        let written = store.sync_document(&[from_doc]).unwrap();
        assert_eq!(written, 1, "文档份更新要覆盖库");

        assert!(
            store.meta(&c.id).unwrap().archived,
            "覆盖正文不能把归档冲掉"
        );
        assert_eq!(store.get(&c.id).unwrap().body, "# 标题 1\n\n改过一个字");
        drop(store);

        let reopened = NoteStore::open_at(dir.clone());
        assert!(reopened.meta(&c.id).unwrap().archived, "落盘后仍是归档");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 归档卡离开「全部卡片」，只留在「归档」里；计数走同一个判定（`docs/28` §7.1）。
    #[test]
    fn archived_cards_only_show_up_under_archive() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let a = card(1, 1);
        let mut b = card(2, 2);
        b.tags.clear();
        b.source_node = None;
        store.upsert(&a).unwrap();
        store.upsert(&b).unwrap();
        store.set_archived(std::slice::from_ref(&a.id), true);

        let counts = mw_core::note::scope_counts(&store.metas());
        assert_eq!(
            (counts.all, counts.inbox, counts.untagged, counts.archived),
            (1, 1, 1, 1)
        );
        let all = CardQuery::new(mw_core::note::CardScope::All);
        assert_eq!(all.filter(&store.metas()).len(), 1);
        let arch = CardQuery::new(mw_core::note::CardScope::Archived);
        // 先把 `metas()` 落成局部：临时值活不过这一行，而 `got` 要借它。
        let metas = store.metas();
        let got: Vec<&str> = arch.filter(&metas).iter().map(|m| m.id.as_str()).collect();
        assert_eq!(got, vec![a.id.as_str()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 批量必须逐 id 报告：坏的 id 进 `failed`，好的照常写盘，不许静默部分成功。
    #[test]
    fn batch_reports_every_id_including_failures() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let a = card(1, 1);
        store.upsert(&a).unwrap();
        let ids = vec![a.id.clone(), "不存在的卡".to_string()];

        let report = store.add_tags(&ids, &["方法".into()], 50);
        assert_eq!(report.changed, vec![a.id.clone()]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, "不存在的卡");
        assert_eq!(report.failed_count(), 1);
        assert!(store.get(&a.id).unwrap().tags.contains(&"方法".to_string()));

        // 已经是目标状态 → skipped，不重写盘也不改时间。
        let again = store.add_tags(std::slice::from_ref(&a.id), &["方法".into()], 99);
        assert_eq!(again.skipped, vec![a.id.clone()]);
        assert!(again.changed.is_empty());
        assert_eq!(store.meta(&a.id).unwrap().updated_at, 50);

        // 非法标签整批判失败，不静默丢掉用户输入。
        let blank = store.add_tags(std::slice::from_ref(&a.id), &["   ".into()], 100);
        assert_eq!(blank.failed_count(), 1);
        assert!(blank.changed.is_empty());

        // 删「知识」不该顺手删掉「知识/方法」。
        store.add_tags(std::slice::from_ref(&a.id), &["知识/方法".into()], 60);
        let removed = store.remove_tags(std::slice::from_ref(&a.id), &["知识".into()], 70);
        assert_eq!(removed.changed, vec![a.id.clone()]);
        let tags = store.get(&a.id).unwrap().tags;
        assert!(!tags.iter().any(|t| t == "知识"));
        assert!(tags.iter().any(|t| t == "知识/方法"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 检索口径要覆盖标签与来源节点标题（`docs/28` §7.6），且支持拼音。
    #[test]
    fn library_search_covers_tags_and_source_node_titles() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let mut a = card(1, 1);
        a.title = "无关标题".into();
        a.body = "只有普通文字".into();
        a.tags = vec!["梯度下降".into()];
        let mut b = card(2, 2);
        b.title = "另一张".into();
        b.body = "正文".into();
        b.tags.clear();
        store.upsert(&a).unwrap();
        store.upsert(&b).unwrap();

        let titles = HashMap::from([("node-2".to_string(), "反向传播".to_string())]);
        let lookup = |node: &str| titles.get(node).map(|s| s.as_str());

        let by_tag = store.library_hits("梯度下降", mw_core::MatchMode::Text, lookup);
        assert_eq!(by_tag.len(), 1);
        assert!(by_tag.contains_key(&a.id));

        let by_node = store.library_hits("反向传播", mw_core::MatchMode::Text, lookup);
        assert!(by_node.contains_key(&b.id), "来源节点标题要能命中");

        let by_pinyin = store.library_hits("tidu", mw_core::MatchMode::Pinyin, lookup);
        assert!(by_pinyin.contains_key(&a.id), "标签也要走拼音");

        assert!(store
            .library_hits("", mw_core::MatchMode::Pinyin, lookup)
            .is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reopening_uses_the_index_and_rebuild_survives_bad_files() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        store.sync_document(&[card(1, 1), card(2, 2)]).unwrap();
        drop(store);

        let reopened = NoteStore::open_at(dir.clone());
        assert_eq!(reopened.len(), 2, "索引可用时不用扫盘");

        std::fs::write(dir.join("broken.md"), "这不是卡片").unwrap();
        let mut rebuilt = reopened;
        assert_eq!(rebuilt.rebuild(), 2);
        assert_eq!(rebuilt.errors(), 1, "坏文件只计数，不中断");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_library_card_is_not_overwritten_by_an_older_document_copy() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let mut newer = card(1, 100);
        newer.body = "库里的新版".into();
        store.upsert(&newer).unwrap();

        let mut older = card(1, 50);
        older.id = newer.id.clone();
        older.body = "文档里的旧版".into();
        assert_eq!(store.sync_document(&[older]).unwrap(), 0);
        assert_eq!(store.get(&newer.id).unwrap().body, "库里的新版");

        let mut newer_doc = card(1, 200);
        newer_doc.id = newer.id.clone();
        newer_doc.body = "文档里的更新版".into();
        assert_eq!(store.sync_document(&[newer_doc.clone()]).unwrap(), 1);
        assert_eq!(store.get(&newer.id).unwrap().body, "文档里的更新版");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tag_index_merges_case_and_rename_rewrites_every_card() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let mut a = card(1, 1);
        a.tags = vec!["知识".into(), "方法".into()];
        let mut b = card(2, 2);
        b.tags = vec!["知识".into(), "方法/阅读".into()];
        store.upsert(&a).unwrap();
        store.upsert(&b).unwrap();

        let tags = store.all_tags();
        assert_eq!(tags[0], ("知识".to_string(), 2));
        assert_eq!(tags.len(), 3);

        // 大小写无关：把"知识"改成"Knowledge"，两张卡都要变。
        assert_eq!(store.rename_tag("知识", "Knowledge").unwrap(), 2);
        assert_eq!(store.get(&a.id).unwrap().tags[0], "Knowledge");
        assert_eq!(store.get(&b.id).unwrap().tags[0], "Knowledge");
        assert_eq!(store.all_tags()[0], ("Knowledge".to_string(), 2));
        // 再打开索引仍一致
        drop(store);
        let reopened = NoteStore::open_at(dir.clone());
        assert_eq!(reopened.get(&a.id).unwrap().tags[0], "Knowledge");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_tag_rejects_empty_and_returns_zero_for_unknown() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        store.upsert(&card(1, 1)).unwrap();
        assert!(store.rename_tag("知识", "  ").is_err());
        assert_eq!(store.rename_tag("不存在", "新").unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn daily_note_is_idempotent_per_date() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let a = store.daily_note("2026-10-20", 100).unwrap();
        let b = store.daily_note("2026-10-20", 200).unwrap();
        assert_eq!(a.id, b.id, "同一天只建一张");
        assert_eq!(b.tags, vec!["每日"]);
        assert_eq!(b.title, "2026-10-20");
        let c = store.daily_note("2026-10-21", 300).unwrap();
        assert_ne!(a.id, c.id);
        assert_eq!(store.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_text_search_finds_body_snippet_and_pinyin() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let mut a = card(1, 10);
        a.title = "深度学习".into();
        a.body = "# 反向传播\n\n梯度下降与链式法则".into();
        store.upsert(&a).unwrap();
        let mut b = card(2, 20);
        b.title = "无关卡片".into();
        b.body = "只有普通文字".into();
        store.upsert(&b).unwrap();

        let hits = store.search("反向传播", 20, mw_core::MatchMode::Pinyin);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, a.id);
        assert!(hits[0].matched.contains("反向传播"));
        assert!(hits[0].before.contains("深度学习"));

        // 拼音/首字母检索复用 mw-core：shen / sd 都应命中标题；精确文本模式只认原文。
        assert!(store
            .search("shen", 20, mw_core::MatchMode::Pinyin)
            .iter()
            .any(|h| h.id == a.id));
        assert!(store
            .search("sd", 20, mw_core::MatchMode::Pinyin)
            .iter()
            .any(|h| h.id == a.id));
        assert!(store
            .search("shen", 20, mw_core::MatchMode::Text)
            .is_empty());
        assert_eq!(store.search("", 20, mw_core::MatchMode::Pinyin).len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_thousand_cards_is_bounded() {
        let dir = temp_dir();
        let mut store = NoteStore::open_at(dir.clone());
        let cards: Vec<NoteCard> = (0..1000)
            .map(|i| {
                let mut c = card(i, i as i64 + 1);
                c.body = format!("# 标题 {i}\n\n这是第 {i} 张卡片的正文，包含检索词 alpha。");
                c
            })
            .collect();
        store.sync_document(&cards).unwrap();
        // debug 构建的墙钟断言在整包串行跑时会吃到瞬时抖动；预热后取三次最好值，
        // 仍能挡住把复杂度从 O(n) 退化成 O(n²) 的改动。
        assert_eq!(
            store.search("alpha", 50, mw_core::MatchMode::Pinyin).len(),
            50
        );
        let mut best = std::time::Duration::MAX;
        for _ in 0..3 {
            let start = std::time::Instant::now();
            let hits = store.search("alpha", 50, mw_core::MatchMode::Pinyin);
            best = best.min(start.elapsed());
            assert_eq!(hits.len(), 50);
        }
        assert!(
            best < std::time::Duration::from_millis(100),
            "1000 张搜索最快耗时 {best:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_thousand_cards_rebuild_without_losing_order_or_content() {
        let dir = temp_dir();
        let cards: Vec<NoteCard> = (0..1000).map(|i| card(i, i as i64 + 1)).collect();
        let mut store = NoteStore::open_at(dir.clone());
        assert_eq!(store.sync_document(&cards).unwrap(), 1000);
        assert_eq!(store.len(), 1000);
        let newest = store.list()[0].id.clone();
        drop(store);

        let mut reopened = NoteStore::open_at(dir.clone());
        assert_eq!(reopened.len(), 1000, "索引应能直接读出 1000 张");
        assert_eq!(reopened.rebuild(), 1000, "删掉索引后扫盘也要恢复");
        assert_eq!(reopened.errors(), 0);
        assert_eq!(reopened.list()[0].id, newest, "重建后排序仍按更新时间");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
