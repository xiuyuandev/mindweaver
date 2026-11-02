//! 文本 + 拼音检索（文档 07 §7）。
//!
//! 命中规则：原文子串 / 全拼（**音节边界对齐**）/ 首字母，全部小写比较。
//! 多音字按读音展开为多个拼音变体（主音优先，上限 `MAX_VARIANTS`）。
//! 检索范围：节点标题 + 备注 + 链接（`docs/02` §4 的 P0 口径），
//! 以及概念图命题（关系线短语，身份是「源节点 + 边」）。
//! 拼音键按节点、按字段惰性计算并缓存，字段文本变化时自动重建。

use std::borrow::Cow;
use std::collections::HashMap;

use pinyin::{ToPinyin, ToPinyinMulti};
use serde::Serialize;

use crate::model::Node;

/// 匹配方式：`Pinyin`（默认）= 原文 + 全拼 + 首字母；`Text` 只做原文子串（大小写不敏感）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatchMode {
    #[default]
    Pinyin,
    Text,
}

impl MatchMode {
    /// 轨迹与 `SearchHit.kind` 口径之外的短名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pinyin => "pinyin",
            Self::Text => "text",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Pinyin => Self::Text,
            Self::Text => Self::Pinyin,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchExcerpt {
    /// 已把换行折成空格的展示文本（两端带 `…`）。
    pub text: String,
    /// 折到 `text` 坐标系的命中**字符**区间。
    pub range: [usize; 2],
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub score: i32,
    /// text / pinyin / initials
    pub kind: String,
    /// text / note / link：命中的是节点的哪个字段。
    pub field: String,
    /// 命中的**字段内**原文**字符**区间 `[start, end)`，供高亮与定位；无命中位置时为 None。
    pub range: Option<[usize; 2]>,
    /// 备注/链接命中时的展示摘录；标题命中为 None（标题本身就是要展示的文本）。
    pub excerpt: Option<SearchExcerpt>,
}

/// 概念图命题（关系线短语）的一条命中。
///
/// 身份是「源节点 id + 边 id」：关系线是边不是节点，不能塞进 [`SearchHit`] 的节点语义。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipHit {
    /// 源节点 id（短语的归属节点）。
    pub from: String,
    /// 关系线 id。
    pub id: String,
    /// 目标节点 id。
    pub to: String,
    pub score: i32,
    /// text / pinyin / initials
    pub kind: String,
    /// 短语原文内命中**字符**区间 `[start, end)`。
    pub range: [usize; 2],
    /// 命中处 ±`EXCERPT_CONTEXT` 字的展示摘录（折到摘录坐标系）。
    pub excerpt: SearchExcerpt,
}

/// `SearchHit.field` 的取值：标题。
pub const FIELD_TEXT: &str = "text";
/// `SearchHit.field` 的取值：备注。
pub const FIELD_NOTE: &str = "note";
/// `SearchHit.field` 的取值：链接。
pub const FIELD_LINK: &str = "link";
/// 命题（关系线短语）的 field 取值。
///
/// `RelationshipHit` 本身不带 field 字段，这个字面量活在两处：本文件
/// `relationship_hits` 给 `field_hit` 传的字段名，以及壳层 `main.rs` 行组装处
/// （~:2117 `field: "relationship".into()`）——后者待收口人改用本常量收口。
pub const FIELD_RELATIONSHIP: &str = "relationship";

/// 备注/链接的拼音索引字符上限：超出的部分仍能原文命中，但不建拼音键。
const FIELD_PINYIN_CAP: usize = 240;
/// 摘录命中处前后各保留的字符数。
const EXCERPT_CONTEXT: usize = 24;

/// 拼音键缓存上限。20k 文档一次查询即可全量驻留（约 8–12MB）；
/// 设成 8k 会让 20k 文档每次输入都整体清空重建成一遍。
const CACHE_LIMIT: usize = 32_000;
/// 单节点最多展开的拼音变体数（多音字组合上限，超出按主音优先截断）。
const MAX_VARIANTS: usize = 8;
/// 单字最多取几个读音。
const MAX_READINGS_PER_CHAR: usize = 3;
/// 每前进一个字符的排序扣分。
const POS_PENALTY: i32 = 6;
/// 位置扣分的封顶字符数：质量分必须始终落在 `[0, KIND_STEP)` 里，
/// 否则长文本里靠后的命中会借到下一档（甚至下一个字段）的分数带里。
const POS_CAP: usize = 3_000;

/// 跨字段的**分层基分**：标题 / 备注 / 链接三档量程互不重叠（`docs/07` §7）。
///
/// 以前基分是相邻的 1000/700/500、650/520/400、600/480/380，量程互相重叠：
/// 标题里靠后的「包含」（`1000 - 6·s`，`s ≥ 22`）会被备注「精确」
/// （`650 + 120 + 150 = 920`）反超，排序就违反了「标题 > 备注 > 链接」。
/// 现在分数按「字段带 → 档位带 → 质量」编码：高位先比层级，低位才比精确度与位置。
const FIELD_STEP: i32 = 1_000_000;
const FIELD_TEXT_RANK: i32 = 3;
const FIELD_NOTE_RANK: i32 = 2;
const FIELD_LINK_RANK: i32 = 1;
/// 命题短语单独成组（壳层按分组表达先后的那个「组」），字段带排在节点字段之下。
const FIELD_REL_RANK: i32 = 0;

/// 同字段内「原文 > 全拼 > 首字母」也是一层高位：档位带。
const KIND_STEP: i32 = 100_000;
const KIND_TEXT_RANK: i32 = 2;
const KIND_FULL_RANK: i32 = 1;
const KIND_INITIALS_RANK: i32 = 0;

/// 档位带内的质量分：起点 + 精确/前缀加成 − 位置扣分。
/// 最差 `20_000 - 6×3_000 = 2_000`，最好 `20_000 + 50_000 + 20_000 = 90_000`，
/// 两端都在 `[0, KIND_STEP)` 内。
const QUALITY_BASE: i32 = 20_000;
const EXACT_BONUS: i32 = 50_000;
const PREFIX_BONUS: i32 = 20_000;

/// 一个拼音变体的检索键：拼接后的全拼串 + 音节边界 + 首字母串。
#[derive(Clone, Default)]
struct Key {
    full: String,
    /// 各音节的 `(full 内字节起点, 对应原文字符下标)`。
    starts: Vec<(usize, usize)>,
    initials: String,
}

impl Key {
    fn with_capacity(syllables: usize) -> Self {
        Self {
            full: String::with_capacity(syllables * 6),
            starts: Vec::with_capacity(syllables),
            initials: String::with_capacity(syllables),
        }
    }

    fn push_syllable(&mut self, plain: &str, char_index: usize, initial: char) {
        self.starts.push((self.full.len(), char_index));
        self.full.push_str(plain);
        self.initials.push(initial);
    }

    /// 全拼匹配：要求起点落在音节边界（避免 `an` 命中 `xiangmu` 这类跨音节假命中），
    /// 允许最后一个音节不完整（`xiangm` 应命中「项目」）。
    fn match_full(&self, q: &str) -> Option<(usize, usize, bool, bool)> {
        let mut from = 0usize;
        while let Some(rel) = self.full[from..].find(q) {
            let byte = from + rel;
            let end = byte + q.len();
            if let Some(si) = self.starts.iter().position(|&(b, _)| b == byte) {
                let li = self
                    .starts
                    .iter()
                    .rposition(|&(b, _)| b < end)
                    .unwrap_or(si)
                    .max(si);
                let char_start = self.starts[si].1;
                let char_end = self.starts[li].1 + 1;
                let exact = si == 0 && li == self.starts.len() - 1 && q.len() == self.full.len();
                return Some((char_start, char_end, si == 0, exact));
            }
            from = byte + 1;
        }
        None
    }

    /// 首字母匹配：每个字符贡献一个字母，故天然按字符对齐。
    fn match_initials(&self, q: &str) -> Option<(usize, usize, bool, bool)> {
        let idx = self.initials.find(q)?;
        let si = idx;
        let li = idx + q.chars().count() - 1;
        let char_start = self.starts.get(si)?.1;
        let char_end = self.starts.get(li)?.1 + 1;
        let exact = si == 0 && li + 1 == self.starts.len();
        Some((char_start, char_end, si == 0, exact))
    }
}

/// 单字候选读音。固定数组：20k × 十余字，逐字 `Vec<String>` 分配正是 P12 的主成本。
struct Readings {
    items: [(&'static str, char); MAX_READINGS_PER_CHAR],
    len: usize,
}

impl Readings {
    fn new() -> Self {
        Self {
            items: [("", '\0'); MAX_READINGS_PER_CHAR],
            len: 0,
        }
    }

    fn push(&mut self, plain: &'static str, first_letter: &'static str) {
        if self.len == MAX_READINGS_PER_CHAR {
            return;
        }
        if let Some(initial) = first_letter.chars().next() {
            self.items[self.len] = (plain, initial);
            self.len += 1;
        }
    }

    fn as_slice(&self) -> &[(&'static str, char)] {
        &self.items[..self.len]
    }

    /// 同一字的多读音可能拼写相同（去重以免浪费变体名额），顺带给出稳定顺序。
    fn sort_dedup(&mut self) {
        self.items[..self.len].sort_by(|a, b| a.0.cmp(b.0));
        let mut w = 0;
        for r in 0..self.len {
            if w == 0 || self.items[r].0 != self.items[w - 1].0 {
                self.items[w] = self.items[r];
                w += 1;
            }
        }
        self.len = w;
    }
}

/// 汉字取多音字表；ASCII 字母数字在 `build_keys` 里直接处理，不走这张表。
fn char_readings(c: char) -> Readings {
    let mut out = Readings::new();
    if let Some(multi) = c.to_pinyin_multi() {
        for p in multi.into_iter().take(MAX_READINGS_PER_CHAR) {
            out.push(p.plain(), p.first_letter());
        }
    }
    if out.len == 0 {
        if let Some(p) = c.to_pinyin() {
            out.push(p.plain(), p.first_letter());
        }
    }
    out.sort_dedup();
    out
}

/// 按字符逐个展开读音组合，构造至多 `MAX_VARIANTS` 个拼音键（主音变体在前）。
///
/// 单读音字符就地追加，不克隆整份 `Key`；只有遇到多音字才分叉。
fn build_keys(text: &str) -> Vec<Key> {
    let cap = text.chars().count().min(64);
    let mut cur: Vec<Key> = vec![Key::with_capacity(cap)];
    for (ci, c) in text.chars().enumerate() {
        if c.is_ascii_alphanumeric() {
            let mut buf = [0u8; 4];
            let lower = c.to_ascii_lowercase();
            let plain: &str = lower.encode_utf8(&mut buf);
            for k in &mut cur {
                k.push_syllable(plain, ci, lower);
            }
            continue;
        }
        let readings = char_readings(c);
        let rs = readings.as_slice();
        match rs.len() {
            0 => {}
            1 => {
                for k in &mut cur {
                    k.push_syllable(rs[0].0, ci, rs[0].1);
                }
            }
            _ => {
                let mut next: Vec<Key> =
                    Vec::with_capacity((cur.len() * rs.len()).min(MAX_VARIANTS));
                'outer: for k in &cur {
                    for &(plain, initial) in rs {
                        if next.len() >= MAX_VARIANTS {
                            break 'outer;
                        }
                        let mut nk = k.clone();
                        nk.push_syllable(plain, ci, initial);
                        next.push(nk);
                    }
                }
                if next.is_empty() {
                    return Vec::new();
                }
                next.sort_by(|a, b| a.full.cmp(&b.full));
                next.dedup_by(|a, b| a.full == b.full);
                cur = next;
            }
        }
    }
    cur.retain(|k| !k.starts.is_empty());
    cur
}

/// 把 `text` 里所有（区分大小写的）`query` 换成 `to`，返回新文本与替换次数。
///
/// 只做字面子串替换：拼音命中（比如用 `xm` 搜到「项目」）没有可替换的位置，
/// 调用方按次数 0 跳过即可。空查询词直接原样返回，避免无限插入。
pub fn replace_all(text: &str, query: &str, to: &str) -> (String, usize) {
    if query.is_empty() {
        return (text.to_string(), 0);
    }
    let mut count = 0;
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(query) {
        out.push_str(&rest[..at]);
        out.push_str(to);
        count += 1;
        rest = &rest[at + query.len()..];
    }
    out.push_str(rest);
    (out, count)
}

/// 把 `text` 的**字符**区间 `[start, end)` 换成 `to`，返回新文本。
///
/// 逐个替换专用：区间来自原文命中（`SearchHit.range` 是字符下标），
/// 越过字符数或 `start > end` 返回 `None`——越界不该悄悄钳到边界。
pub fn replace_char_range(text: &str, range: [usize; 2], to: &str) -> Option<String> {
    let [start, end] = range;
    if start > end {
        return None;
    }
    let count = text.chars().count();
    if end > count {
        return None;
    }
    let byte_at = |ci: usize| {
        if ci == count {
            text.len()
        } else {
            text.char_indices().nth(ci).map_or(text.len(), |(b, _)| b)
        }
    };
    let (start, end) = (byte_at(start), byte_at(end));
    if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }
    let mut out = String::with_capacity(text.len() + to.len());
    out.push_str(&text[..start]);
    out.push_str(to);
    out.push_str(&text[end..]);
    Some(out)
}

/// 用 Ctrl+H 的替换串换掉命题命中段，返回**新短语全文**。
///
/// `hit.range` 是短语**原文**坐标系的字符区间（`RelationshipHit` 里只有它是原文口径，
/// `excerpt.range` 已折到摘录坐标系，不可混用）；`current_label` 必须是发命令瞬间
/// 该边的实时短语，不能是搜索时快照——区间在旧文本上对，在新文本上未必对。
///
/// 只认字面命中：`kind` 为 pinyin / initials 时返回 None，不给替换入口。
/// 这与壳层 `replace_target_fields`（panels/search.rs:936）「只认 text 命中」是
/// **同一口径的第二次出现**——壳层把 `Group::Relationship` 放宽进替换路径时
/// 直接调本函数即可，不必再自判 kind。
///
/// 空结果语义：本函数只产字符串、不管命令语义，替换成空/全空白也照常返回
/// （行内清空一段与删除整条短语是两件事，判空不是这里的信息量）。真正的闸门在
/// 模型层——空白短语会被 `Command::SetRelationshipLabel` 拒绝
/// （document.rs「概念图的连接短语必填」）；调用方**必须**复用 `phrase_commit`
/// 的四态判定（mw-shell canvas/canvas_methods_3.rs:767 附近），把空/全空白结果
/// 按「拒绝 + 提示」处理，不许静默发一条必然失败的命令。
pub fn replace_phrase(hit: &RelationshipHit, current_label: &str, to: &str) -> Option<String> {
    if hit.kind != "text" {
        return None;
    }
    replace_char_range(current_label, hit.range, to)
}

/// 替换后这段文本**是否还会被同一查询词命中**——Ctrl+H 的「换完还在」门禁。
///
/// 三条判定路径与 [`SearchIndex::hits`] / [`SearchIndex::relationship_hits`] 产生命中的
/// 那条完全同一口径：查询词先 `trim().to_lowercase()`（两处的第一行都是它），
/// 原文档走 [`match_text`]（大小写不敏感的字符子串），拼音/首字母走 [`Key::match_full`]
/// 与 [`Key::match_initials`]，且只在 `mode == Pinyin` 且查询词全是 ASCII 字母数字时
/// 才看房拼音档——与生产者的 `pinyin_eligible` 逐字同式。空/纯空白查询词返回
/// `false`（生产者空查询不产命中）。
///
/// 唯一一处已知口径差：拼音键的字符上限（备注/链接/短语是 `FIELD_PINYIN_CAP`，
/// 标题不限）在本函数里无从区分字段身份，故对**任意**文本一律按键判定。
/// 超长文本可能因此偏「仍命中」——后果只是多跳一行，不会绕圈，可接受。
///
/// 用途：壳层 `settle_after_refresh`（panels/search.rs:317）在「当前行替换后仍命中」时
/// 据此**跳过而非绕回**，避免换完还含查询词的行被反复停在同一处打转（M21 残账）。
pub fn still_matches(text: &str, query: &str, mode: MatchMode) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }
    if match_text(text, &q).is_some() {
        return true;
    }
    let pinyin_eligible = mode == MatchMode::Pinyin && q.chars().all(|c| c.is_ascii_alphanumeric());
    if !pinyin_eligible {
        return false;
    }
    build_keys(text)
        .iter()
        .any(|key| key.match_full(&q).is_some() || key.match_initials(&q).is_some())
}

#[derive(Default)]
pub struct SearchIndex {
    /// id -> (text 快照, 拼音键)
    cache: HashMap<String, (String, Vec<Key>)>,
    /// id -> (note 快照, 拼音键)
    note_cache: HashMap<String, (String, Vec<Key>)>,
    /// id -> (link 快照, 拼音键)
    link_cache: HashMap<String, (String, Vec<Key>)>,
    /// (源节点 id, 边 id) -> (短语快照, 拼音键)
    rel_cache: HashMap<(String, String), (String, Vec<Key>)>,
}

/// 原文小写子串匹配，返回字符区间。
fn match_text(text: &str, q_lower: &str) -> Option<(usize, usize, bool, bool)> {
    // 中文正文没有大写字母：直接借用原文，省掉每节点一次 `to_lowercase` 分配。
    let lower = if text.chars().any(char::is_uppercase) {
        Cow::Owned(text.to_lowercase())
    } else {
        Cow::Borrowed(text)
    };
    let byte = lower.find(q_lower)?;
    let char_start = lower[..byte].chars().count();
    let matched = lower[byte..byte + q_lower.len()].chars().count();
    let char_end = char_start + matched;
    let total = text.chars().count();
    Some((
        char_start,
        char_end,
        char_start == 0,
        char_start == 0 && char_end == total,
    ))
}

/// 以命中区间为中心截一段展示文本：换行折成空格，前后各留 `EXCERPT_CONTEXT`
/// 个字符，两端有截断才加 `…`，并把区间折算到摘录坐标系。
fn excerpt_around(text: &str, range: [usize; 2]) -> SearchExcerpt {
    // 一换一（`\r\n` 算两个字符）保证字符下标不漂，命中区间仍指在同一处。
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let chars: Vec<char> = flat.chars().collect();
    let n = chars.len();
    let start_hit = range[0].min(n);
    let end_hit = range[1].min(n).max(start_hit);
    let start = start_hit.saturating_sub(EXCERPT_CONTEXT);
    let end = (end_hit + EXCERPT_CONTEXT).min(n);
    let mut out = String::with_capacity(flat.len().min((end - start) * 3 + 6));
    if start > 0 {
        out.push('…');
    }
    let lead = if start > 0 { 1 } else { 0 };
    out.extend(&chars[start..end]);
    if end < n {
        out.push('…');
    }
    SearchExcerpt {
        text: out,
        range: [start_hit - start + lead, end_hit - start + lead],
    }
}

/// 取（或重建）某字段的拼音键；超过 `cap` 字符的字段返回 None（只做原文匹配）。
fn cache_keys<'a>(
    cache: &'a mut HashMap<String, (String, Vec<Key>)>,
    id: &str,
    text: &str,
    cap: usize,
) -> Option<&'a [Key]> {
    if text.chars().count() > cap {
        return None;
    }
    let entry = cache
        .entry(id.to_string())
        .or_insert_with(|| (text.to_string(), build_keys(text)));
    if entry.0 != text {
        *entry = (text.to_string(), build_keys(text));
    }
    Some(&entry.1)
}

/// 命题短语的拼音键：身份是（源节点, 边），短语变化时重建。
fn rel_cache_keys<'a>(
    cache: &'a mut HashMap<(String, String), (String, Vec<Key>)>,
    from: &str,
    id: &str,
    text: &str,
    cap: usize,
) -> Option<&'a [Key]> {
    if text.chars().count() > cap {
        return None;
    }
    let entry = cache
        .entry((from.to_string(), id.to_string()))
        .or_insert_with(|| (text.to_string(), build_keys(text)));
    if entry.0 != text {
        *entry = (text.to_string(), build_keys(text));
    }
    Some(&entry.1)
}

/// 单字段内三档（原文 / 全拼 / 首字母）取最高分。
fn field_hit(
    id: &str,
    field: &str,
    text: &str,
    q: &str,
    keys: Option<&[Key]>,
    field_rank: i32,
) -> Option<SearchHit> {
    let mut best: Option<SearchHit> = None;
    let mut offer = |cand: SearchHit| {
        if best.as_ref().is_none_or(|b| cand.score > b.score) {
            best = Some(cand);
        }
    };
    let make =
        |kind: &str, s: usize, e: usize, kind_rank: i32, prefix: bool, exact: bool| SearchHit {
            id: id.to_string(),
            score: field_rank * FIELD_STEP
                + kind_rank * KIND_STEP
                + QUALITY_BASE
                + if exact { EXACT_BONUS } else { 0 }
                + if prefix { PREFIX_BONUS } else { 0 }
                - POS_PENALTY * s.min(POS_CAP) as i32,
            kind: kind.into(),
            field: field.into(),
            range: Some([s, e]),
            excerpt: None,
        };

    if let Some((s, e, prefix, exact)) = match_text(text, q) {
        offer(make("text", s, e, KIND_TEXT_RANK, prefix, exact));
    }

    if let Some(keys) = keys {
        for key in keys {
            if let Some((s, e, prefix, exact)) = key.match_full(q) {
                offer(make("pinyin", s, e, KIND_FULL_RANK, prefix, exact));
            }
            if let Some((s, e, prefix, exact)) = key.match_initials(q) {
                offer(make("initials", s, e, KIND_INITIALS_RANK, prefix, exact));
            }
        }
    }

    best
}

impl SearchIndex {
    /// `order` 为文档前序节点 id 列表，用于同分时按文档顺序稳定排序。
    pub fn hits(
        &mut self,
        nodes: &HashMap<String, Node>,
        order: &[String],
        query: &str,
        mode: MatchMode,
    ) -> Vec<SearchHit> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        if self.cache.len() > CACHE_LIMIT
            || self.note_cache.len() > CACHE_LIMIT
            || self.link_cache.len() > CACHE_LIMIT
            || self.rel_cache.len() > CACHE_LIMIT
        {
            self.cache.clear();
            self.note_cache.clear();
            self.link_cache.clear();
            self.rel_cache.clear();
        }
        // 精确文本模式下不建也不读拼音键：首查不付建键成本，切回拼音时按需建立。
        let pinyin_eligible =
            mode == MatchMode::Pinyin && q.chars().all(|c| c.is_ascii_alphanumeric());
        let mut out: Vec<SearchHit> = Vec::new();
        for (id, node) in nodes.iter() {
            let mut best: Option<SearchHit> = None;
            let offer = |cand: SearchHit, best: &mut Option<SearchHit>| {
                if best.as_ref().is_none_or(|b| cand.score > b.score) {
                    *best = Some(cand);
                }
            };

            // 标题命中永远压过同类型的备注/链接命中：按此顺序 offer，同分也保留标题。
            let title_keys = pinyin_eligible
                .then(|| cache_keys(&mut self.cache, id, &node.text, usize::MAX))
                .flatten();
            if let Some(hit) =
                field_hit(id, FIELD_TEXT, &node.text, &q, title_keys, FIELD_TEXT_RANK)
            {
                offer(hit, &mut best);
            }

            if let Some(note) = node.note.as_deref().filter(|t| !t.trim().is_empty()) {
                let keys = pinyin_eligible
                    .then(|| cache_keys(&mut self.note_cache, id, note, FIELD_PINYIN_CAP))
                    .flatten();
                if let Some(hit) = field_hit(id, FIELD_NOTE, note, &q, keys, FIELD_NOTE_RANK) {
                    offer(hit, &mut best);
                }
            }

            if let Some(link) = node.link.as_deref().filter(|t| !t.trim().is_empty()) {
                let keys = pinyin_eligible
                    .then(|| cache_keys(&mut self.link_cache, id, link, FIELD_PINYIN_CAP))
                    .flatten();
                if let Some(hit) = field_hit(id, FIELD_LINK, link, &q, keys, FIELD_LINK_RANK) {
                    offer(hit, &mut best);
                }
            }

            if let Some(mut hit) = best {
                if hit.field != FIELD_TEXT {
                    let source = match hit.field.as_str() {
                        FIELD_NOTE => node.note.as_deref(),
                        FIELD_LINK => node.link.as_deref(),
                        _ => None,
                    };
                    hit.excerpt = source.zip(hit.range).map(|(src, r)| excerpt_around(src, r));
                }
                out.push(hit);
            }
        }

        let rank: HashMap<&str, u32> = order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i as u32))
            .collect();
        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| rank.get(a.id.as_str()).cmp(&rank.get(b.id.as_str())))
        });
        out.truncate(500);
        out
    }

    /// 概念图命题短语检索。`order` 是文档前序节点 id 列表：
    /// 按它扫节点、再按节点内 `relationships` 顺序扫边，同分由稳定排序保序。
    pub fn relationship_hits(
        &mut self,
        nodes: &HashMap<String, Node>,
        order: &[String],
        query: &str,
        mode: MatchMode,
    ) -> Vec<RelationshipHit> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        if self.rel_cache.len() > CACHE_LIMIT {
            self.rel_cache.clear();
        }
        let pinyin_eligible =
            mode == MatchMode::Pinyin && q.chars().all(|c| c.is_ascii_alphanumeric());
        let mut out: Vec<RelationshipHit> = Vec::new();
        for from in order {
            let Some(node) = nodes.get(from) else {
                continue;
            };
            for rel in &node.relationships {
                let Some(label) = rel.label.as_deref().filter(|t| !t.trim().is_empty()) else {
                    continue;
                };
                let keys = pinyin_eligible
                    .then(|| {
                        rel_cache_keys(&mut self.rel_cache, from, &rel.id, label, FIELD_PINYIN_CAP)
                    })
                    .flatten();
                // 分数与节点字段分开排：壳层用分组表达先后，不靠跨类分数压人。
                if let Some(hit) =
                    field_hit(from, FIELD_RELATIONSHIP, label, &q, keys, FIELD_REL_RANK)
                {
                    if let Some(range) = hit.range {
                        out.push(RelationshipHit {
                            from: from.clone(),
                            id: rel.id.clone(),
                            to: rel.to.clone(),
                            score: hit.score,
                            kind: hit.kind,
                            range,
                            excerpt: excerpt_around(label, range),
                        });
                    }
                }
            }
        }
        // 稳定排序：同分保持「文档前序 → 边下标」。
        out.sort_by_key(|r| std::cmp::Reverse(r.score));
        out.truncate(500);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_all_changes_every_occurrence_and_counts() {
        assert_eq!(
            replace_all("项目 A、项目 B、项目", "项目", "Project"),
            ("Project A、Project B、Project".to_string(), 3)
        );
        // 交叉匹配不会被重复消费
        assert_eq!(replace_all("aaa", "aa", "b"), ("ba".to_string(), 1));
        // 空查询等于没改
        assert_eq!(replace_all("abc", "", "x"), ("abc".to_string(), 0));
        // 没命中：次数 0，文本原样
        assert_eq!(replace_all("abc", "z", "y"), ("abc".to_string(), 0));
        // 删成空串也允许
        assert_eq!(replace_all("甲乙", "乙", ""), ("甲".to_string(), 1));
    }

    #[test]
    fn replace_char_range_replaces_a_cjk_slice() {
        // 字符下标不是字节下标：「中文排版」的 [2,4) 是「排版」。
        assert_eq!(
            replace_char_range("中文排版", [2, 4], "排布").as_deref(),
            Some("中文排布")
        );
        // 空替换串 = 删除命中段
        assert_eq!(
            replace_char_range("中文排版", [2, 4], "").as_deref(),
            Some("中文")
        );
        // 首尾区间
        assert_eq!(
            replace_char_range("中文排版", [0, 1], "英").as_deref(),
            Some("英文排版")
        );
        assert_eq!(
            replace_char_range("中文排版", [3, 4], "列").as_deref(),
            Some("中文排列")
        );
    }

    #[test]
    fn replace_char_range_rejects_invalid_ranges() {
        assert_eq!(replace_char_range("中文", [1, 0], "x"), None);
        assert_eq!(replace_char_range("中文", [0, 3], "x"), None);
        assert_eq!(replace_char_range("中文", [3, 3], "x"), None);
        // 末尾空区间是合法的追加点
        assert_eq!(
            replace_char_range("中文", [2, 2], "!").as_deref(),
            Some("中文!")
        );
    }

    #[test]
    fn pinyin_requires_syllable_boundary() {
        let keys = build_keys("项目");
        let an = keys.iter().any(|k| k.match_full("an").is_some());
        let xi = keys.iter().any(|k| k.match_full("xi").is_some());
        // "an" 是 "xiang" 的音节内部片段，不应命中；"xi" 是音节起点，应命中
        assert!(!an, "跨音节片段不应命中");
        assert!(xi);
    }

    #[test]
    fn partial_last_syllable_matches() {
        let keys = build_keys("项目");
        assert!(keys.iter().any(|k| k.match_full("xiangm").is_some()));
    }

    #[test]
    fn heteronym_alternate_reading_matches() {
        // 「重」主音 zhong，另有 chong / tong
        let keys = build_keys("重复");
        assert!(keys.iter().any(|k| k.match_full("chongfu").is_some()));
        assert!(keys.iter().any(|k| k.match_full("zhongfu").is_some()));
        // 首字母亦应覆盖两个读音
        assert!(keys.iter().any(|k| k.match_initials("cf").is_some()));
        assert!(keys.iter().any(|k| k.match_initials("zf").is_some()));
    }

    #[test]
    fn range_maps_back_to_char_offsets() {
        let keys = build_keys("项目启动");
        let (_, s, e) = keys
            .iter()
            .find_map(|k| k.match_full("qidong").map(|m| ("", m.0, m.1)))
            .unwrap();
        assert_eq!((s, e), (2, 4));
    }

    #[test]
    fn mixed_ascii_kept_in_pinyin_key() {
        let keys = build_keys("iPhone手机");
        assert!(keys.iter().any(|k| k.match_full("iphone").is_some()));
    }

    #[test]
    fn ascii_case_folds_without_losing_offsets() {
        assert_eq!(match_text("XMind 项目", "xmind"), Some((0, 5, true, false)));
        assert_eq!(match_text("分支节点 42", "42"), Some((5, 7, false, false)));
    }

    fn corpus(nodes: Vec<Node>) -> (HashMap<String, Node>, Vec<String>) {
        let order = nodes.iter().map(|n| n.id.clone()).collect();
        let map = nodes.into_iter().map(|n| (n.id.clone(), n)).collect();
        (map, order)
    }

    fn node(id: &str, text: &str) -> Node {
        Node::new(id.to_string(), None, text)
    }

    fn hit_one(nodes: Vec<Node>, query: &str) -> SearchHit {
        let (map, order) = corpus(nodes);
        let hits = SearchIndex::default().hits(&map, &order, query, MatchMode::Pinyin);
        assert_eq!(hits.len(), 1, "query={query} hits={hits:?}");
        hits.into_iter().next().unwrap()
    }

    #[test]
    fn note_field_hit_maps_range_to_note() {
        let mut n = node("a", "纯 Rust 原生");
        n.note = Some("没有 WebView，没有 HTML。".into());
        let hit = hit_one(vec![n], "html");
        assert_eq!(hit.field, FIELD_NOTE);
        assert_eq!(hit.kind, "text");
        assert_eq!(hit.range, Some([14, 18]));
        let ex = hit.excerpt.unwrap();
        let chars: Vec<char> = ex.text.chars().collect();
        assert_eq!(
            chars[ex.range[0]..ex.range[1]].iter().collect::<String>(),
            "HTML"
        );
    }

    #[test]
    fn link_field_hit() {
        let mut n = node("a", "织思");
        n.link = Some("https://wgpu.rs".into());
        let hit = hit_one(vec![n], "https");
        assert_eq!(hit.field, FIELD_LINK);
        assert_eq!(hit.kind, "text");
        assert_eq!(hit.range, Some([0, 5]));
        assert_eq!(hit.excerpt.unwrap().text, "https://wgpu.rs");
    }

    #[test]
    fn note_field_pinyin_and_initials() {
        let mut n = node("a", "纯 Rust 原生");
        n.note = Some("没有 WebView".into());
        let full = hit_one(vec![n.clone()], "meiyou");
        assert_eq!(
            (full.field.as_str(), full.kind.as_str()),
            (FIELD_NOTE, "pinyin")
        );
        assert_eq!(full.range, Some([0, 2]));
        let ini = hit_one(vec![n], "my");
        assert_eq!(
            (ini.field.as_str(), ini.kind.as_str()),
            (FIELD_NOTE, "initials")
        );
        assert_eq!(ini.range, Some([0, 2]));
    }

    #[test]
    fn title_wins_over_note_on_same_node() {
        let mut n = node("a", "项目启动");
        n.note = Some("项目备注".into());
        let hit = hit_one(vec![n], "项目");
        assert_eq!(hit.field, FIELD_TEXT);
    }

    #[test]
    fn title_hit_ranks_above_note_hit() {
        let mut a = node("a", "都是 HTML 的错");
        a.note = None;
        let mut b = node("b", "别的话题");
        b.note = Some("HTML".into());
        let (map, order) = corpus(vec![b, a]);
        let hits = SearchIndex::default().hits(&map, &order, "html", MatchMode::Pinyin);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "a");
        assert_eq!(hits[0].field, FIELD_TEXT);
        assert_eq!(hits[1].id, "b");
        assert_eq!(hits[1].field, FIELD_NOTE);
    }

    #[test]
    fn title_substring_always_beats_a_later_field_exact() {
        // 标题里「HTML」排在 30 字之后（原分数最低的「包含」），
        // 备注是精确命中（原分数最高的那种）。合同（docs/07 §7）要求标题命中
        // 整体压过备注，不管两边各自的精确度与位置；旧基分 `1000 - 6·30 = 820`
        // 会被备注精确 `650 + 120 + 150 = 920` 反超，这条会红。
        let mut a = node("a", &format!("{}HTML{}", "甲".repeat(30), "尾巴"));
        a.note = None;
        let mut b = node("b", "别的话题");
        b.note = Some("HTML".into());
        let (map, order) = corpus(vec![a, b]);
        let hits = SearchIndex::default().hits(&map, &order, "html", MatchMode::Pinyin);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "a", "标题命中必须整体压过备注命中");
        assert_eq!(hits[0].field, FIELD_TEXT);
        assert_eq!(hits[1].id, "b");
        assert_eq!(hits[1].field, FIELD_NOTE);
    }

    #[test]
    fn note_substring_always_beats_a_later_field_exact() {
        // 同一条链的下一段：备注「包含」也必须压过链接「精确」。
        let long = format!("{}HTML{}", "甲".repeat(30), "尾巴");
        let mut a = node("a", "别的话题");
        a.note = Some(long);
        let mut b = node("b", "另一个话题");
        b.link = Some("HTML".into());
        let (map, order) = corpus(vec![a, b]);
        let hits = SearchIndex::default().hits(&map, &order, "html", MatchMode::Pinyin);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "a", "备注命中必须整体压过链接命中");
        assert_eq!(hits[0].field, FIELD_NOTE);
        assert_eq!(hits[1].id, "b");
        assert_eq!(hits[1].field, FIELD_LINK);
    }

    #[test]
    fn long_note_text_still_matches_beyond_pinyin_cap() {
        let mut n = node("a", "无标题命中");
        n.note = Some(format!("{}没有", "啊".repeat(300)));
        let hit = hit_one(vec![n], "没有");
        assert_eq!(hit.field, FIELD_NOTE);
        assert_eq!(hit.range, Some([300, 302]));
        let ex = hit.excerpt.unwrap();
        assert!(ex.text.starts_with('…'));
        let chars: Vec<char> = ex.text.chars().collect();
        assert_eq!(
            chars[ex.range[0]..ex.range[1]].iter().collect::<String>(),
            "没有"
        );
        // 超过 240 字不建拼音键：全拼查不到，但仍可用原文命中。
        let mut n = node("a", "无标题命中");
        n.note = Some(format!("{}没有", "啊".repeat(300)));
        let (map, order) = corpus(vec![n]);
        assert!(SearchIndex::default()
            .hits(&map, &order, "meiyou", MatchMode::Pinyin)
            .is_empty());
    }

    #[test]
    fn note_cache_invalidates_with_field_text() {
        let mut n = node("a", "标题");
        n.note = Some("旧词".into());
        let (mut map, order) = corpus(vec![n]);
        let mut index = SearchIndex::default();
        assert_eq!(
            index.hits(&map, &order, "jiuci", MatchMode::Pinyin).len(),
            1
        );
        map.get_mut("a").unwrap().note = Some("新词".into());
        assert!(index
            .hits(&map, &order, "jiuci", MatchMode::Pinyin)
            .is_empty());
        assert_eq!(
            index.hits(&map, &order, "xinci", MatchMode::Pinyin).len(),
            1
        );
    }

    #[test]
    fn excerpt_at_start_has_no_leading_ellipsis() {
        let ex = excerpt_around("HTML 是标记语言", [0, 4]);
        assert!(!ex.text.starts_with('…'));
        assert_eq!(ex.range, [0, 4]);
        assert_eq!(ex.text.chars().take(4).collect::<String>(), "HTML");
    }

    #[test]
    fn excerpt_middle_folds_range_and_adds_ellipses() {
        let text = format!("{}HTML{}", "a".repeat(30), "b".repeat(30));
        let ex = excerpt_around(&text, [30, 34]);
        assert!(ex.text.starts_with('…'));
        assert!(ex.text.ends_with('…'));
        assert_eq!(ex.range, [25, 29]);
        let chars: Vec<char> = ex.text.chars().collect();
        assert_eq!(
            chars[ex.range[0]..ex.range[1]].iter().collect::<String>(),
            "HTML"
        );
    }

    #[test]
    fn excerpt_flattens_newlines_and_keeps_range_on_hit() {
        let mut n = node("a", "标题");
        n.note = Some("第一行\n第二行 HTML".into());
        let hit = hit_one(vec![n], "html");
        let ex = hit.excerpt.unwrap();
        assert_eq!(ex.text, "第一行 第二行 HTML");
        assert_eq!(ex.range, [8, 12]);
        assert_eq!(hit.range, Some([8, 12]));
    }

    #[test]
    fn blank_note_never_hits() {
        let mut n = node("a", "标题");
        n.note = Some("  \n ".into());
        let (map, order) = corpus(vec![n]);
        assert!(SearchIndex::default()
            .hits(&map, &order, "html", MatchMode::Pinyin)
            .is_empty());
    }

    fn rel(id: &str, to: &str, label: Option<&str>) -> crate::model::Relationship {
        crate::model::Relationship {
            id: id.into(),
            to: to.into(),
            label: label.map(Into::into),
            color: None,
            head: Default::default(),
            ..Default::default()
        }
    }

    fn with_rels(id: &str, rels: Vec<crate::model::Relationship>) -> Node {
        let mut n = node(id, "概念");
        n.relationships = rels;
        n
    }

    fn rel_hit_one(nodes: Vec<Node>, query: &str) -> RelationshipHit {
        let (map, order) = corpus(nodes);
        let hits = SearchIndex::default().relationship_hits(&map, &order, query, MatchMode::Pinyin);
        assert_eq!(hits.len(), 1, "query={query} hits={hits:?}");
        hits.into_iter().next().unwrap()
    }

    #[test]
    fn relationship_text_hit_reports_both_ends_and_range() {
        let hit = rel_hit_one(
            vec![with_rels("a", vec![rel("e1", "b", Some("解决"))])],
            "解决",
        );
        assert_eq!(
            (hit.from.as_str(), hit.id.as_str(), hit.to.as_str()),
            ("a", "e1", "b")
        );
        assert_eq!(hit.kind, "text");
        assert_eq!(hit.range, [0, 2]);
        assert_eq!(hit.excerpt.text, "解决");
        assert_eq!(hit.excerpt.range, [0, 2]);
    }

    #[test]
    fn relationship_pinyin_hit() {
        let hit = rel_hit_one(
            vec![with_rels("a", vec![rel("e1", "b", Some("解决"))])],
            "jiejue",
        );
        assert_eq!((hit.kind.as_str(), hit.range), ("pinyin", [0, 2]));
    }

    #[test]
    fn relationship_initials_hit() {
        let hit = rel_hit_one(
            vec![with_rels("a", vec![rel("e1", "b", Some("解决"))])],
            "jj",
        );
        assert_eq!((hit.kind.as_str(), hit.range), ("initials", [0, 2]));
    }

    #[test]
    fn blank_relationship_label_never_hits() {
        let n = with_rels(
            "a",
            vec![
                rel("e1", "b", None),
                rel("e2", "b", Some("  \n ")),
                rel("e3", "b", Some("解决")),
            ],
        );
        let hit = rel_hit_one(vec![n], "解决");
        assert_eq!(hit.id, "e3");
    }

    #[test]
    fn relationship_cache_invalidates_with_phrase() {
        let (mut map, order) = corpus(vec![with_rels("a", vec![rel("e1", "b", Some("解决"))])]);
        let mut index = SearchIndex::default();
        assert_eq!(
            index
                .relationship_hits(&map, &order, "jiejue", MatchMode::Pinyin)
                .len(),
            1
        );
        map.get_mut("a").unwrap().relationships[0].label = Some("支撑".into());
        assert!(index
            .relationship_hits(&map, &order, "jiejue", MatchMode::Pinyin)
            .is_empty());
        assert_eq!(
            index
                .relationship_hits(&map, &order, "zhicheng", MatchMode::Pinyin)
                .len(),
            1
        );
    }

    #[test]
    fn long_phrase_still_matches_beyond_pinyin_cap() {
        let label = format!("{}没有", "啊".repeat(300));
        let hit = rel_hit_one(
            vec![with_rels("a", vec![rel("e1", "b", Some(&label))])],
            "没有",
        );
        assert_eq!(hit.range, [300, 302]);
        assert!(hit.excerpt.text.starts_with('…'));
        // 超过 240 字不建拼音键：全拼查不到，原文仍命中。
        let (map, order) = corpus(vec![with_rels("a", vec![rel("e1", "b", Some(&label))])]);
        assert!(SearchIndex::default()
            .relationship_hits(&map, &order, "meiyou", MatchMode::Pinyin)
            .is_empty());
    }

    #[test]
    fn relationship_ties_follow_doc_order_then_edge_index() {
        let b = with_rels(
            "b",
            vec![rel("e2", "c", Some("支撑")), rel("e1", "c", Some("支撑"))],
        );
        let a = with_rels("a", vec![rel("e3", "c", Some("支撑"))]);
        let (map, order) = corpus(vec![b, a]);
        let hits =
            SearchIndex::default().relationship_hits(&map, &order, "支撑", MatchMode::Pinyin);
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["e2", "e1", "e3"]);
    }

    #[test]
    fn relationship_excerpt_folds_middle_range() {
        let label = format!("{}支撑{}", "a".repeat(30), "b".repeat(30));
        let hit = rel_hit_one(
            vec![with_rels("a", vec![rel("e1", "b", Some(&label))])],
            "支撑",
        );
        assert!(hit.excerpt.text.starts_with('…'));
        assert!(hit.excerpt.text.ends_with('…'));
        assert_eq!(hit.excerpt.range, [25, 27]);
        let chars: Vec<char> = hit.excerpt.text.chars().collect();
        assert_eq!(
            chars[hit.excerpt.range[0]..hit.excerpt.range[1]]
                .iter()
                .collect::<String>(),
            "支撑"
        );
    }

    #[test]
    fn relationship_excerpt_flattens_newlines() {
        let hit = rel_hit_one(
            vec![with_rels(
                "a",
                vec![rel("e1", "b", Some("第一行\n第二行 支撑"))],
            )],
            "支撑",
        );
        assert_eq!(hit.excerpt.text, "第一行 第二行 支撑");
        assert_eq!(hit.excerpt.range, [8, 10]);
        assert_eq!(hit.range, [8, 10]);
    }

    #[test]
    fn text_mode_ignores_pinyin_and_initials() {
        let (map, order) = corpus(vec![node("a", "项目启动")]);
        let mut index = SearchIndex::default();
        assert!(index.hits(&map, &order, "xm", MatchMode::Text).is_empty());
        assert!(index
            .hits(&map, &order, "xiangmu", MatchMode::Text)
            .is_empty());
        assert_eq!(index.hits(&map, &order, "项目", MatchMode::Text).len(), 1);
        // 切回拼音，同一索引仍按全拼/首字母命中。
        assert_eq!(index.hits(&map, &order, "xm", MatchMode::Pinyin).len(), 1);
    }

    #[test]
    fn text_mode_still_matches_notes_links_and_relationships() {
        let mut n = node("a", "标题");
        n.note = Some("备注里的 HTML".into());
        n.relationships = vec![rel("e1", "b", Some("解决"))];
        let (map, order) = corpus(vec![n]);
        let mut index = SearchIndex::default();
        let note = index.hits(&map, &order, "html", MatchMode::Text);
        assert_eq!(note.len(), 1);
        assert_eq!(note[0].field, FIELD_NOTE);
        let rel = index.relationship_hits(&map, &order, "解决", MatchMode::Text);
        assert_eq!(rel.len(), 1);
        assert_eq!(rel[0].kind, "text");
    }

    #[test]
    fn text_mode_does_not_build_pinyin_keys() {
        let (map, order) = corpus(vec![node("a", "项目")]);
        let mut index = SearchIndex::default();
        assert!(index
            .hits(&map, &order, "xiangmu", MatchMode::Text)
            .is_empty());
        assert!(index.cache.is_empty(), "Text 模式不该建拼音键");
        assert_eq!(
            index.hits(&map, &order, "xiangmu", MatchMode::Pinyin).len(),
            1
        );
        assert!(!index.cache.is_empty(), "切回拼音后按键需重建");
    }

    #[test]
    fn match_mode_defaults_to_pinyin_and_text_is_case_insensitive() {
        assert_eq!(MatchMode::default(), MatchMode::Pinyin);
        assert_eq!(MatchMode::Pinyin.toggled(), MatchMode::Text);
        assert_eq!(MatchMode::Text.toggled(), MatchMode::Pinyin);
        let (map, order) = corpus(vec![node("a", "HTML 排版")]);
        let mut index = SearchIndex::default();
        assert_eq!(index.hits(&map, &order, "HTML", MatchMode::Text).len(), 1);
        assert_eq!(index.hits(&map, &order, "html", MatchMode::Text).len(), 1);
    }

    fn rel_hit(label: &str, query: &str) -> RelationshipHit {
        rel_hit_one(
            vec![with_rels("a", vec![rel("e1", "b", Some(label))])],
            query,
        )
    }

    #[test]
    fn replace_phrase_replaces_a_text_hit_slice() {
        // CJK 短语、区间在中间：「支撑理论」的 [2,4) 是「理论」。
        let hit = rel_hit("支撑理论", "理论");
        assert_eq!(hit.kind, "text");
        assert_eq!(hit.range, [2, 4]);
        assert_eq!(
            replace_phrase(&hit, "支撑理论", "依据").as_deref(),
            Some("支撑依据")
        );
        // 区间在首。
        let hit = rel_hit("支撑理论", "支撑");
        assert_eq!(hit.range, [0, 2]);
        assert_eq!(
            replace_phrase(&hit, "支撑理论", "反驳").as_deref(),
            Some("反驳理论")
        );
        // 区间在尾。
        let hit = rel_hit("A项目B", "项目");
        assert_eq!(hit.range, [1, 3]);
        assert_eq!(
            replace_phrase(&hit, "A项目B", "任务").as_deref(),
            Some("A任务B")
        );
    }

    #[test]
    fn replace_phrase_refuses_pinyin_and_initials_hits() {
        // 与壳层「只认字面命中」同一口径：拼音/首字母命中的是汉字区间，
        // 但用户敲的是字母——没有字面段可换，不给替换入口。
        let hit = rel_hit("支撑理论", "zhicheng");
        assert_eq!(hit.kind, "pinyin");
        assert_eq!(replace_phrase(&hit, "支撑理论", "反驳"), None);
        let hit = rel_hit("支撑理论", "zcll");
        assert_eq!(hit.kind, "initials");
        assert_eq!(replace_phrase(&hit, "支撑理论", "反驳"), None);
    }

    #[test]
    fn replace_phrase_rejects_out_of_range_slices() {
        // range 越界（如 stale 快照对不上实时文本）走 replace_char_range 的 None。
        let mut hit = rel_hit("解决", "解决");
        hit.range = [1, 4];
        assert_eq!(replace_phrase(&hit, "解决", "答案"), None);
        let mut hit = rel_hit("解决", "解决");
        hit.range = [2, 1];
        assert_eq!(replace_phrase(&hit, "解决", "答案"), None);
    }

    #[test]
    fn replace_phrase_returns_blank_results_untouched() {
        // 整段换成空串照常返回 Some("")：判空不是本函数的信息量，
        // 空短语的闸门在模型层（SetRelationshipLabel 拒空白）与调用方的 phrase_commit。
        let hit = rel_hit("解决", "解决");
        assert_eq!(replace_phrase(&hit, "解决", "").as_deref(), Some(""));
        let hit = rel_hit("解决之道", "解决");
        assert_eq!(
            replace_phrase(&hit, "解决之道", " ").as_deref(),
            Some(" 之道")
        );
    }

    #[test]
    fn replace_phrase_middle_of_phrase_matches_hand_computation() {
        // 「支撑理论」按字符拆：支(0) 撑(1) 理(2) 论(3)——4 字 12 字节，
        // 区间是字符下标而非字节下标，字节口径会把 [1,3) 错算进「撑」的第 2、3 字节。
        let hit = rel_hit("支撑理论", "撑理");
        assert_eq!(hit.range, [1, 3]);
        assert_eq!(
            replace_phrase(&hit, "支撑理论", "×").as_deref(),
            Some("支×论")
        );
    }

    #[test]
    fn still_matches_text_mode_follows_match_text() {
        // 替换后仍含查询词 → true；不含 → false。
        assert!(still_matches("abc 还有 abc", "abc", MatchMode::Text));
        assert!(!still_matches("abd 已换干净", "abc", MatchMode::Text));
        // 真实一轮：整串换掉后不再命中。
        let (after, n) = replace_all("abc 还有 abc", "abc", "abd");
        assert_eq!(n, 2);
        assert!(!still_matches(&after, "abc", MatchMode::Text));
        // 大小写口径与 match_text 一致：查询词小写化后比对。
        assert!(still_matches("HTML 排版", "html", MatchMode::Text));
        // Text 模式不看拼音：字面不含就不再命中，即便拼音对得上。
        assert!(!still_matches("项目计划", "xiangmu", MatchMode::Text));
        assert!(!still_matches("项目计划", "xm", MatchMode::Text));
    }

    #[test]
    fn still_matches_pinyin_modes() {
        // 全拼 / 首字母命中同样成立——与产生命中的那条同一口径。
        assert!(still_matches("项目计划", "xiangmu", MatchMode::Pinyin));
        assert!(still_matches("项目计划", "xm", MatchMode::Pinyin));
        // 换掉「项目」后既无字面也无拼音可对上。
        let (after, n) = replace_all("项目计划", "项目", "任务");
        assert_eq!((after.as_str(), n), ("任务计划", 1));
        assert!(!still_matches(&after, "项目", MatchMode::Pinyin));
        assert!(!still_matches(&after, "xiangmu", MatchMode::Pinyin));
        // 跨音节片段仍不算命中（音节边界口径与生产一致）：an 不是「项目」的拼音起点。
        assert!(!still_matches("项目计划", "an", MatchMode::Pinyin));
    }

    #[test]
    fn still_matches_empty_query_never_hits() {
        // 生产者对空/纯空白查询不产命中，这里同口径返回 false。
        assert!(!still_matches("anything", "", MatchMode::Pinyin));
        assert!(!still_matches("anything", "   ", MatchMode::Pinyin));
        assert!(!still_matches("", "", MatchMode::Text));
    }

    #[test]
    fn relationship_field_constant_is_the_row_assembly_literal() {
        // 命题 field 字面量收敛到常量：与壳层 main.rs 行组装处写入 HitRow.field 的
        // 字符串必须保持同值（待收口人让壳层改用本常量）。
        assert_eq!(FIELD_RELATIONSHIP, "relationship");
        let hit = rel_hit("解决", "解决");
        assert_eq!(hit.kind, "text");
    }
}
