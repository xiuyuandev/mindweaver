//! 卡片笔记页：**单表面**（Typora 式）编辑器 + Markdown 实时排版。
//!
//! 没有「阅读 / 编辑」两套模式：标题、标签、正文打开就可写，正文是一个多行
//! `TextEdit`，用自定义 layouter 把 Markdown 标记按块级/行内样式排出来。标记本身
//! 仍留在文本里（删掉才不符合纯文本存储），但只在光标所在行显示，其余行像成稿一样
//! 干净；列表符号、任务框、分隔线用绘制层补上，不往文本里插字符，光标映射不受影响。
//!
//! 版式：正文一栏最大 640px（版心 720，`content::CARD`）、行高 28；窗口够宽时不出侧栏。

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, pos2, vec2, Align, Color32, FontFamily, FontId, Id, Key, Modifiers, Rect, RichText,
    Sense, Stroke, StrokeKind,
};
use mw_core::design::content;
use mw_core::{Backlink, NoteCard};

use crate::chrome::{Chrome, Type};
use crate::fonts;

const BODY_PX: f32 = content::READING_BODY.size;
const BODY_LINE: f32 = content::READING_BODY.line_height;
const CODE_PX: f32 = content::CODE.size;
const CODE_LINE: f32 = content::CODE.line_height;
/// 代码块右上角语言标签的字号（合同 §6.2「语言标签 12」）。
const CODE_LANG_PX: f32 = 12.0;
/// 卡片页顶栏高度：30/42 标题与 13/19 元信息需要呼吸（`docs/28` §6.2）。
const TOP_BAR_H: f32 = 44.0;

/// 卡片页能发出的动作。壳层负责落地。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderAction {
    Close,
    JumpToNode(String),
    /// 跳到另一张卡片（`[[引用]]` 反链）。
    OpenCard(String),
    OpenLink(String),
    /// 打开全局标签管理（重命名）。
    ManageTags,
    /// 提交标题/正文/标签；`close` = 保存后关闭卡片页。
    Save {
        title: String,
        body: String,
        tags: Vec<String>,
        close: bool,
    },
    /// 外部修改提示条：丢掉本页草稿、按磁盘上的内容重开（`docs/28` §6.7）。
    ReloadFromDisk,
    /// 外部修改提示条：用本页草稿覆盖磁盘。
    OverwriteDisk,
    /// 外部修改提示条：先不管，接着写（下次要保存时会再提）。
    DismissConflict,
}

/// 卡片来源：它挂在哪个主题上，以及源节点是否还在。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLink {
    pub node_id: String,
    pub text: String,
    pub missing: bool,
    pub doc_label: String,
    /// 源文档是概念图。措辞与回跳按钮都跟着它变（「概念」不是「主题」）。
    pub concept_map: bool,
}

/// 目录项：标题级别、纯文字、在正文里的字符下标（点击定位用）。
///
/// 只在测试与「以后可能的目录」里用：UI 侧常驻右目录已按 `docs/29` CER-13 移除，
/// 所以 bin 构建下这一族没有调用者。留着是因为解析本身有测试覆盖，
/// 删掉会连带丢掉「代码围栏不算标题」这类回归网。
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub char_index: usize,
    pub level: u8,
    pub text: String,
}

pub struct CardReader {
    pub card_id: String,
    /// 上次保存后的显示标题（空标题取正文首行）。
    pub title: String,
    pub tags: Vec<String>,
    source: Option<SourceLink>,
    backlinks: Vec<Backlink>,
    broken_refs: Vec<String>,
    /// 目前只写不读（卡片页自 CER-13 起不再显示创建时间），留着给排序/导出。
    #[allow(dead_code)]
    created_at: i64,
    updated_at: i64,
    /// 上次保存的内容，用来判断脏与撤销基线。
    saved_title: String,
    saved_body: String,
    saved_tags: Vec<String>,
    pub draft_title: String,
    pub draft_body: String,
    pub draft_tags: Vec<String>,
    available_tags: Vec<String>,
    tag_input: String,
    /// 脏从哪一刻开始算（停手满 `AUTOSAVE` 就自动落盘）。每次改动都往后推。
    dirty_since: Option<std::time::Instant>,
    /// 壳层已经把这一笔排进写盘队列（`保存中…` 的那一帧起）。
    saving: bool,
    /// 上一次自动/手动保存失败的原因；草稿不丢，`Ctrl+S` 可重试（`docs/28` §6.7）。
    save_error: Option<String>,
    /// 外部修改：打开或上次写盘之后，磁盘上的这张卡被别处改过（壳层比指纹后设置）。
    conflict: bool,
    /// 只读：写不进去（权限不足 / 被另一个实例占着）。正文不可编辑，顶栏挂「只读」。
    read_only: bool,
    /// 打开卡片时把光标放进正文，只做一次。
    focus_body: bool,
    /// 正文是否有焦点；失焦时所有标记都收起，像读成稿。
    body_focused: bool,
    /// 光标所在行（0 起）。只在编辑中的那一行显示 Markdown 标记。
    /// 光标所在行与**行内字节列**（片段级标记揭示要用列）。
    active: Option<(usize, usize)>,
    /// 「带外改动」的一步撤销槽：（改动前文本，改动前光标，改动后文本）。
    /// 只有回车接管与列表粘贴会写它；之后正文一旦变化就作废。
    auto_undo: Option<(String, usize, String)>,
    /// 目录点击后下一帧把正文滚到该字符。
    scroll_to: Option<usize>,
    /// 斜杠菜单当前高亮项（按过滤后的列表下标）。
    slash_sel: usize,
    /// 在哪个 `/`（字符下标）上按过 Esc：同一处不再弹菜单。
    slash_off_at: Option<usize>,
    /// 下一帧把光标放到这个字符（QA 夹具用）。
    caret_to: Option<usize>,
    /// 专注模式（CER-19）：非当前块淡到 35%，当前块不动。
    focus_on: bool,
    /// 专注的淡入淡出进度 0..1（1 = 完全进入）。减少动画时直接置 1。
    focus_t: f32,
    /// 打字机模式（CER-19）：键盘输入后把光标锚到正文可视区 42%。
    typewriter: bool,
    /// 上一次写进过程证据的滚动偏移，只在它真的变了时才多记一行。
    tw_last_off: f32,
    /// 打字机修正窗口还剩几帧：键盘输入开窗，落位或超时即关。
    tw_settle: u8,
    /// 正文滚动区的 id（取自上一帧的 `ScrollAreaOutput`）：用来读这一帧布局用的偏移。
    tw_scroll_id: Option<egui::Id>,
    /// 系统/设置里的「减少动画」。
    reduce_motion: bool,
    /// 卡片页自己的滚动条淡入淡出（CE-01）。
    scroll_fade: crate::scroll_bar::Fade,
    /// 上一帧的滚动偏移：变了就说明用户刚滚过，滚动条要亮起来。
    scroll_seen: f32,
    /// 上一帧指针是否停在滚动条上（含正在拖）。
    scroll_hot: bool,
    /// 拖动时算出的偏移，下一帧交给 `ScrollArea` 落地。
    scroll_req: Option<f32>,
    /// 上一帧到底有没有画出滚动条：只在这一格翻转时记轨迹，不然每帧一条。
    scroll_bar_on: bool,
    /// 字数缓存：上次算字数时的正文快照与结果。
    wc_body: String,
    wc_value: usize,
    /// 正文开头那行 `# 标题` 的形态：打开时摘掉、保存时按原样拼回。
    header: TitleHeader,
}

impl CardReader {
    pub fn new(card: &NoteCard) -> Self {
        // `docs/28` §6.10「禁止顶栏标题与正文 H1 重复」：正文第一行就是
        // `# 标题` 时，那一行**就是**标题，该由标题框显示，不该在下面再画一遍。
        // 打开时把它摘掉（正文只留真正的内容），保存时按原样拼回去——字节不变。
        let (header, body) = split_title_header(&card.title, &card.body);
        let body = body.to_string();
        Self {
            card_id: card.id.clone(),
            title: card.display_title().to_string(),
            tags: card.tags.clone(),
            source: None,
            backlinks: Vec::new(),
            broken_refs: Vec::new(),
            created_at: card.created_at,
            updated_at: card.updated_at,
            saved_title: card.title.clone(),
            saved_body: body.clone(),
            saved_tags: card.tags.clone(),
            draft_title: card.title.clone(),
            draft_body: body,
            draft_tags: card.tags.clone(),
            available_tags: Vec::new(),
            tag_input: String::new(),
            dirty_since: None,
            saving: false,
            save_error: None,
            conflict: false,
            read_only: false,
            focus_body: true,
            body_focused: true,
            active: Some((0, 0)),
            auto_undo: None,
            scroll_to: None,
            slash_sel: 0,
            slash_off_at: None,
            caret_to: None,
            focus_on: false,
            focus_t: 0.0,
            typewriter: false,
            tw_last_off: 0.0,
            tw_settle: 0,
            tw_scroll_id: None,
            reduce_motion: false,
            scroll_fade: crate::scroll_bar::Fade::default(),
            scroll_seen: 0.0,
            scroll_hot: false,
            scroll_req: None,
            scroll_bar_on: false,
            // 空快照 = 第一次 `word_count()` 必定重算；正文为空时结果本来就是 0。
            wc_body: String::new(),
            wc_value: 0,
            header,
        }
    }

    /// 系统/设置里的「减少动画」：专注淡出与打字机滚动都直接落位。
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.reduce_motion = reduce;
    }

    /// 库里的全部标签，用于输入补全。
    pub fn set_available_tags(&mut self, tags: Vec<String>) {
        self.available_tags = tags;
    }

    /// 壳层查完文档后告诉卡片页：源节点还在不在、谁在引用这张卡。
    pub fn set_context(&mut self, source: Option<SourceLink>, backlinks: Vec<Backlink>) {
        self.source = source;
        self.backlinks = backlinks;
    }

    /// 断链目标（只提示，不阻止编辑）。
    pub fn set_broken_refs(&mut self, broken: Vec<String>) {
        self.broken_refs = broken;
    }

    fn add_tag(&mut self, tag: &str) {
        let mut tags = self.draft_tags.clone();
        // 用户习惯连 `#` 一起打：`#知识/方法` 存成 `知识/方法`，否则标签会显示成 `##知识/方法`。
        let tag = tag.trim().trim_start_matches('#').trim();
        tags.push(tag.to_string());
        self.draft_tags = NoteCard::normalize_tags(&tags);
        self.tag_input.clear();
        self.dirty_since = Some(std::time::Instant::now());
    }

    /// 保存成功后由壳层调用：把草稿变成新的「已保存」。
    pub fn mark_saved(&mut self) {
        self.saved_title = self.draft_title.clone();
        self.saved_body = self.draft_body.clone();
        self.saved_tags = self.draft_tags.clone();
        self.tags = self.draft_tags.clone();
        self.title = derive_title(&self.draft_title, &self.draft_body);
        self.updated_at = card_now_ms();
        self.dirty_since = None;
        self.saving = false;
        self.save_error = None;
    }

    /// 外部修改 / 只读由壳层判定后设进来（视图层不做 IO）。
    pub fn set_conflict(&mut self, on: bool) {
        self.conflict = on;
    }

    /// 只读态查询：壳层写盘前要据此改口（提示另存为，而不是硬写）。
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        if on {
            // 只读就别再自动保存了。
            self.dirty_since = None;
        }
    }

    /// 这一笔被顶掉了（外部修改）：退出「保存中…」，草稿照旧留着。
    pub fn clear_saving(&mut self) {
        self.saving = false;
    }

    /// 壳层把这一笔排进写盘队列时调用：顶栏改成「保存中…」。
    pub fn mark_saving(&mut self) {
        self.saving = true;
        self.save_error = None;
    }

    /// 写盘失败：草稿一个字节都不动，顶栏挂「保存失败」，`Ctrl+S` 可重试。
    pub fn mark_save_failed(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.saving = false;
        // 失败后不能每帧重排队（会打满磁盘）；等用户下一次输入或 Ctrl+S 再试。
        self.dirty_since = None;
        crate::trace::event(&format!("card_save_fail {message}"));
        self.save_error = Some(message);
    }

    fn is_dirty(&self) -> bool {
        self.draft_title != self.saved_title
            || self.draft_body != self.saved_body
            || self.draft_tags != self.saved_tags
    }

    fn store_caret(&self, ctx: &egui::Context, body_id: Id, caret: usize) {
        let mut st = egui::text_edit::TextEditState::load(ctx, body_id).unwrap_or_default();
        st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(caret),
        )));
        st.store(ctx, body_id);
    }

    /// QA：实拍斜杠菜单——在第一个二级标题前插一行 `/`，光标落在斜杠后。
    pub fn arm_slash_demo(&mut self) {
        let at = self
            .draft_body
            .find("\n## ")
            .unwrap_or(self.draft_body.len());
        if at < self.draft_body.len() {
            self.draft_body.insert_str(at, "\n\n/");
        } else {
            self.draft_body.push_str("\n\n/");
        }
        let caret = self.draft_body[..at].chars().count() + 3;
        self.focus_body = true;
        self.body_focused = true;
        self.caret_to = Some(caret);
        self.scroll_to = Some(caret);
    }

    /// QA：实拍专注模式——光标落在第二段，其余块淡到 35%（进度直接给满，动画另有单测）。
    pub fn arm_focus_demo(&mut self) {
        let at = self.draft_body.find("\n\n## ").map(|i| i + 2).unwrap_or(0);
        self.focus_on = true;
        self.focus_t = 1.0;
        self.focus_body = true;
        self.body_focused = true;
        self.caret_to = Some(self.draft_body[..at].chars().count());
        self.active = Some((
            line_index_at(&self.draft_body, self.draft_body[..at].chars().count()),
            0,
        ));
    }

    /// 对照夹具：光标、标记揭示都跟 `arm_focus_demo` 一样，唯独不开专注。
    /// 用来拍「只有淡不淡之差」的 A/B 两张图。
    pub fn clear_focus_demo(&mut self) {
        self.focus_on = false;
        self.focus_t = 0.0;
    }

    /// QA：实拍打字机模式——光标先落在正文中段，剩下的交给输入脚本按键触发锚定。
    pub fn arm_typewriter_demo(&mut self) {
        self.typewriter = true;
        self.focus_body = true;
        self.body_focused = true;
        let at = self.draft_body.chars().count() / 2;
        self.caret_to = Some(at);
    }

    #[allow(dead_code)]
    pub fn headings(&self) -> Vec<Heading> {
        headings(&self.draft_body)
    }

    /// 字数（中文按字、英文按词）。
    ///
    /// 按正文快照缓存：顶栏和元信息行每帧各要一次，而每次都要把整篇 Markdown
    /// 解析成一棵 AST 再抽纯文本——20k 的卡片就是每帧白解析两遍。
    /// 比较一份字符串（`memcmp`）比解析便宜一个数量级。
    pub fn word_count(&mut self) -> usize {
        if self.wc_body != self.draft_body {
            self.wc_body.clear();
            self.wc_body.push_str(&self.draft_body);
            self.wc_value = reading_count(&mw_markdown::parse(&self.draft_body).plain_text());
        }
        self.wc_value
    }

    /// 全屏卡片页。返回 `Some` 表示这一帧用户做了动作。
    ///
    /// `theme` 是内容表面的颜色来源（`docs/28` §4.3「全部通过语义 token」）：
    /// 卡片页与笔记库预览、大纲共用 `content_metrics` 同一条派生链，
    /// 不再从壳层 `Chrome` 另开一套（CER-31 F2）。
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ch: &Chrome,
        theme: &mw_core::Theme,
    ) -> Option<ReaderAction> {
        use std::time::Instant;
        let now = Instant::now();
        let before = (
            self.draft_title.clone(),
            self.draft_body.clone(),
            self.draft_tags.clone(),
        );

        let screen = ctx.content_rect();
        let colors = content::content_metrics(theme, ctx.pixels_per_point()).colors;
        let body_id = Id::unique("mw-card-body");

        // 斜杠菜单：用上一帧的光标位置判断菜单是否开着，键盘要在 TextEdit 之前截走，
        // 否则 Enter 会先落成换行、方向键会把光标挪走。
        let pre_caret = egui::text_edit::TextEditState::load(ctx, body_id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| r.primary.index.0.min(self.draft_body.chars().count()));
        let body_had_focus = ctx.memory(|m| m.had_focus_last_frame(body_id));
        let slash_ctx = pre_caret.and_then(|c| slash_context(&self.draft_body, c));
        let slash_live = body_had_focus
            && slash_ctx
                .as_ref()
                .is_some_and(|c| self.slash_off_at != Some(c.start_char));
        let mut slash_pick: Option<usize> = None;
        // IME 优先：组合态里 Enter 是「上屏」，方向键是「在候选里挑」——
        // 这时候把键截走会让输入法打架（CER-18 合同「IME 不拦截」）。
        let ime_active = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
        if let Some(sc) = slash_ctx.as_ref().filter(|_| slash_live && !ime_active) {
            let options = slash_matches(&sc.query);
            if !options.is_empty() {
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                    self.slash_sel += 1;
                }
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                    self.slash_sel += options.len() - 1;
                }
                self.slash_sel %= options.len();
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                    || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab))
                {
                    slash_pick = Some(options[self.slash_sel]);
                }
            }
        }
        let slash_esc =
            slash_live && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if slash_esc {
            self.slash_off_at = slash_ctx.as_ref().map(|c| c.start_char);
        }
        if let (Some(sc), Some(pick), Some(caret)) = (slash_ctx.as_ref(), slash_pick, pre_caret) {
            let (body, new_caret) =
                slash_apply_text(&self.draft_body, sc, caret, &SLASH_ITEMS[pick]);
            self.draft_body = body;
            self.dirty_since = Some(now);
            self.slash_off_at = None;
            self.slash_sel = 0;
            self.store_caret(ctx, body_id, new_caret);
        } else if slash_ctx.is_none() {
            self.slash_off_at = None;
        }
        // QA 夹具：光标落到指定字符（该行 `/` 会让菜单自动出现）。
        if let Some(caret) = self.caret_to.take() {
            let caret = caret.min(self.draft_body.chars().count());
            self.store_caret(ctx, body_id, caret);
        }

        // CER-19 键位：`Alt+Shift+Z` 打字机、`Alt+Z` 专注。两者都在 Esc 之前判，
        // 因为 Esc 的层级是「关菜单 → 退专注 → 关页面」。
        let alt_shift_z =
            ctx.input_mut(|i| i.consume_key(Modifiers::ALT | Modifiers::SHIFT, Key::Z));
        let alt_z = !alt_shift_z && ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::Z));
        if alt_shift_z {
            self.typewriter = !self.typewriter;
            self.tw_last_off = -1.0e9;
            self.tw_settle = 0;
            self.tw_scroll_id = None;
        }
        if alt_z {
            self.focus_on = !self.focus_on;
        }
        // 120ms 淡入淡出；减少动画时一步到位。
        let focus_target = if self.focus_on { 1.0 } else { 0.0 };
        if self.reduce_motion {
            self.focus_t = focus_target;
        } else if self.focus_t != focus_target {
            let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
            let step = dt / FOCUS_SECS;
            self.focus_t = if self.focus_t < focus_target {
                (self.focus_t + step).min(focus_target)
            } else {
                (self.focus_t - step).max(focus_target)
            };
        }
        let esc = if slash_esc {
            false
        } else if self.focus_on && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            // Esc 第一层：退专注（`docs/28` §6.4）；这一下不再关页面。
            self.focus_on = false;
            false
        } else {
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        };
        // `Ctrl+Enter` **故意不绑**（`docs/28` §6.8「不绑定提交；正文中不打断写作」）。
        // 以前它等于「保存并关闭」：在正文里顺手按一下就整页关掉，是那种
        // 「按错了才发现」的坑。现在让它落到 `TextEdit`，按下去就是一个换行。
        let ctrl_s = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S));
        // Esc 不再「取消编辑」：单表面里没有可回退的第二态，脏内容先落盘再走。
        // 斜杠菜单开着时 Esc 只关菜单，不退出卡片。
        if esc {
            return Some(if self.is_dirty() {
                self.save_action(true)
            } else {
                ReaderAction::Close
            });
        }

        let mut action = None;

        let words = self.word_count();

        egui::Area::new(Id::unique("mw-card-reader"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                ui.set_width(screen.width());
                ui.painter()
                    .rect_filled(screen, 0.0, crate::chrome::c32(colors.page));
                ui.add_space(12.0);

                // ── 顶栏 44px：返回 / 来源面包屑 / 字数 / 保存态 / 更多（CER-13）──
                // 标题不在这儿出现第二份：正文第一行就是 H1。
                ui.horizontal(|ui| {
                    ui.set_height(TOP_BAR_H);
                    ui.add_space(22.0);
                    if ui
                        .button("← 返回")
                        .on_hover_text("保存并关闭（Esc）")
                        .clicked()
                    {
                        action = Some(if self.is_dirty() {
                            self.save_action(true)
                        } else {
                            ReaderAction::Close
                        });
                    }
                    ui.add_space(10.0);
                    // 面包屑：卡片从哪来。缺源时给灰字，不给假按钮。
                    match &self.source {
                        None => {
                            ui.label(
                                RichText::new("未关联节点")
                                    .size(Type::SMALL)
                                    .color(ch.dim),
                            );
                        }
                        Some(src) if src.missing => {
                            ui.label(
                                RichText::new("源节点已删除")
                                    .size(Type::SMALL)
                                    .color(ch.dim),
                            );
                        }
                        Some(src) => {
                            // 面包屑就是「来源」：`大纲 › 用户痛点`，点它回节点。
                            // 合同顶栏只有这一个来源出口，不再另放一个「回到节点」按钮。
                            let kind = if src.concept_map {
                                "概念图"
                            } else {
                                "大纲"
                            };
                            let resp = ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(format!("{kind} › {}", src.text))
                                            .size(Type::SMALL)
                                            .color(ch.dim),
                                    )
                                    .sense(Sense::click()),
                                )
                                .on_hover_text(if src.concept_map {
                                    "回到概念图并选中这个概念"
                                } else {
                                    "回到导图并选中这个主题"
                                });
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                action = Some(ReaderAction::JumpToNode(src.node_id.clone()));
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(20.0);
                        // 顶栏只留一个出口：动作收进「更多」，不做两套按钮。
                        ui.menu_button("更多", |ui| {
                            if ui.button("标签管理").clicked() {
                                action = Some(ReaderAction::ManageTags);
                            }
                            ui.separator();
                            if ui.button("保存并关闭").clicked() {
                                action = Some(if self.is_dirty() {
                                    self.save_action(true)
                                } else {
                                    ReaderAction::Close
                                });
                            }
                        });
                        // 保存态（`docs/28` §6.7）：`保存中…` → `已保存`，13/18 辅助色 + 6px 点；
                        // 失败时挂 `保存失败`（草稿还在，`Ctrl+S` 重试），不做彩色胶囊。
                        let (label, tone) = if self.read_only {
                            ("只读", ch.dim)
                        } else if self.saving {
                            ("保存中…", ch.dim)
                        } else if self.save_error.is_some() {
                            ("保存失败", ch.warn)
                        } else if self.is_dirty() {
                            ("编辑中 · 稍后自动保存", ch.warn)
                        } else {
                            ("已保存", ch.dim)
                        };
                        ui.label(RichText::new(label).size(Type::SMALL).color(tone));
                        // 合同 §6.7：写盘失败要留一条「重试」的路，不能让用户只能
                        // 干看着「保存失败」——草稿还在，按一下就该再试一次。
                        if let Some(err) = self.save_error.clone() {
                            if ui
                                .button(RichText::new("重试").size(Type::SMALL))
                                .on_hover_text(format!("{err}（Ctrl+S 也可以）"))
                                .clicked()
                            {
                                self.mark_saving();
                                action = Some(self.save_action(false));
                            }
                        }
                        let (dot, _) =
                            ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 3.0, tone);
                        ui.label(
                            RichText::new(format!("{words} 字"))
                                .size(Type::SMALL)
                                .color(ch.dim),
                        );
                    });
                });

                // 外部修改（`docs/28` §6.7）：32px 行内条，绝不拿模态遮住正文。
                if self.conflict {
                    ui.add_space(4.0);
                    egui::Frame::NONE
                        .fill(ch.warn.gamma_multiply(0.10))
                        .stroke(Stroke::new(1.0, ch.warn.gamma_multiply(0.35)))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(16, 6))
                        .show(ui, |ui| {
                            ui.set_height(20.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .button(RichText::new("重新加载").size(Type::SMALL))
                                    .on_hover_text("丢掉这一页的草稿，按磁盘上的内容重开")
                                    .clicked()
                                {
                                    action = Some(ReaderAction::ReloadFromDisk);
                                }
                                if ui
                                    .button(RichText::new("覆盖").size(Type::SMALL))
                                    .on_hover_text("用本页草稿覆盖磁盘上的新内容")
                                    .clicked()
                                {
                                    action = Some(ReaderAction::OverwriteDisk);
                                }
                                if ui
                                    .button(RichText::new("取消").size(Type::SMALL))
                                    .clicked()
                                {
                                    action = Some(ReaderAction::DismissConflict);
                                }
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("文件已在外部修改")
                                        .size(Type::SMALL)
                                        .color(ch.warn),
                                );
                            });
                        });
                }

                ui.add_space(8.0);

                // ── 正文：唯一的一份文字，编辑即排版 ──
                // 配色与笔记库预览同源（CER-31 F2）：`content_metrics` 的语义色，
                // 代码块底与预览一致用 6% 正文色。
                let palette = MdPalette::from_content(
                    crate::chrome::c32(colors.text_primary),
                    crate::chrome::c32(colors.text_secondary),
                    crate::chrome::c32(colors.text_faint),
                    crate::chrome::c32(colors.accent),
                    crate::note_library::c32a(colors.text_primary, 0.06),
                    crate::chrome::c32(colors.page),
                );
                let total = screen.width();
                // 版心走合同令牌 `content::CARD`（720 / 正文 640 / 窄窗左右 24），
                // 不再自写「窗口 ≥1040 → 720」那套（CER-31 F1/F4）。
                let col = content::CARD.column(total);
                let article = col.text;
                let left = (col.side + col.inset).max(24.0);
                // 光标行取**本帧**的 `TextEditState`，而不是 `self.active`（上一帧末
                // 由 `output.cursor_range` 写回的那个）。带外改动——`caret_to`、
                // 斜杠插入、列表续写、行首退格——都在这一帧之前就把新光标存好了，
                // 用旧值会让标记揭示慢一拍：换行之后露标记的还是上一行。
                let caret_now = egui::text_edit::TextEditState::load(ctx, body_id)
                    .and_then(|s| s.cursor.char_range())
                    .map(|r| usize::from(r.primary.index).min(self.draft_body.chars().count()));
                let active = if self.body_focused {
                    caret_now
                        .map(|c| {
                            (
                                line_index_at(&self.draft_body, c),
                                col_in_line(&self.draft_body, c),
                            )
                        })
                        // 第一帧还没有 `TextEditState` 时退回上一帧算出的落点。
                        .or(self.active)
                } else {
                    None
                };
                // 专注模式：只淡「当前块以外」的行。区间按空行切块算，光标不在正文里时
                // 沿用最后一次的落点，免得一失焦整页忽然全亮。
                let focus_dim = if self.focus_on && self.focus_t > 0.001 {
                    let lines: Vec<&str> = self.draft_body.split_inclusive('\n').collect();
                    let line = active.map(|(l, _)| l).unwrap_or(0);
                    let (first, last) = block_lines(&lines, line);
                    Some((first, last, focus_alpha(self.focus_t)))
                } else {
                    None
                };
                // 滚动区这一帧的几何：内容高、视口、偏移。滚动条要贴**窗口**右缘，
                // 而不是版心右缘，所以得等 `horizontal_top` 收口之后在外面画。
                let mut scroll_metrics: Option<(f32, Rect, f32)> = None;
                ui.horizontal_top(|ui| {
                    ui.add_space(left);
                    // 目录在滚动区外：正文滚到哪，目录都留在原地。
                    ui.vertical(|ui| {
                        ui.set_width(article + 10.0);
                        // 滚动条自己画（`crate::scroll_bar`）：egui 0.37 的默认是
                        // `ScrollStyle::floating()`——休眠态完全不透明？不，是
                        // `dormant_handle_opacity = 0.0`，即**看不见**；滚动时才浮出来，
                        // 还盖在文字上，横向位置跟着 `ScrollArea` 的宽度走（版心右缘），
                        // 不是窗口右缘。`docs/28` §4.5 要的是常驻细条 + 悬停淡入，
                        // 所以这里关掉内置的，换成贴窗口右缘的一条。
                        let mut scroller = egui::ScrollArea::vertical()
                            .id_salt("mw-card-surface-scroll")
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .auto_shrink([false, false])
                            .max_height(ui.available_height());
                        if let Some(y) = self.scroll_req.take() {
                            // 拖动算出来的目标：只在这一帧生效，之后交还给滚轮。
                            scroller = scroller.vertical_scroll_offset(y);
                        }
                        let scroll_out = scroller.show(ui, |ui| {
                            // 滚动区换算 `Align::Min` 锚点时要再减一次**内容 ui** 的 item_spacing
                            // （scroll_area.rs 里 `spacing *= remap(0.0..=1.0, -1.0..=1.0)`）。
                            // 里面那层把间距改成了 0，这一层还是默认 8，所以得在改之前量。
                            let content_gap = ui.spacing().item_spacing.y;
                            ui.vertical(|ui| {
                                ui.set_max_width(article);
                                // 纵向间距全部显式给：默认 `item_spacing.y = 8` 会悄悄
                                // 加在每个 `add_space` 旁边，合同里的 8/12/40 就全偏了。
                                ui.spacing_mut().item_spacing.y = 0.0;
                                // 标题就是正文第一行：顶栏不再有第二份标题。
                                ui.add_space(48.0);
                                let title_out = ui.add(
                                    egui::TextEdit::singleline(&mut self.draft_title)
                                        .id(Id::unique("mw-card-title"))
                                        .hint_text("无标题（取正文首行）")
                                        .frame(egui::Frame::NONE)
                                        .desired_width(article)
                                        .font(FontId::new(
                                            content::PAGE_TITLE.size,
                                            FontFamily::Name(fonts::BOLD.into()),
                                        ))
                                        .text_color(palette.text),
                                );
                                // 合同 §6.8「标题行 Enter 转到正文」。
                                // 单行 `TextEdit` 自己不会把 Enter 当回事——不接这一下，
                                // 在标题里敲回车就像敲到空气里。这里把焦点交给正文，
                                // 光标停在它上次的位置（新卡片就是开头）。
                                if title_out.has_focus()
                                    && ui.input(|i| {
                                        i.key_pressed(Key::Enter) && !i.modifiers.shift
                                    })
                                {
                                    ui.input_mut(|i| {
                                        i.consume_key(egui::Modifiers::NONE, Key::Enter);
                                    });
                                    self.focus_body = true;
                                    crate::trace::event("card_focus_from_title");
                                }
                                ui.add_space(8.0);
                                self.meta_and_tags(ui, ch, &mut action, now);
                                ui.add_space(40.0);
                                let mut layouter =
                                    |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                                        let mut job = markdown_job_focus(
                                            buf.as_str(),
                                            palette,
                                            wrap_width,
                                            active,
                                            focus_dim,
                                        );
                                        job.wrap.max_width = wrap_width;
                                        ui.fonts_mut(|f| f.layout_job(job))
                                    };
                                // 带外改动只留一步撤销：正文一旦被别处改过就作废，
                                // 免得 Ctrl+Z 把几帧前的旧文本硬塞回来。
                                if let Some((_, _, after)) = &self.auto_undo {
                                    if *after != self.draft_body {
                                        self.auto_undo = None;
                                    }
                                }
                                if self.body_focused
                                    && !ui.input(|i| {
                                        i.events.iter().any(|e| matches!(e, egui::Event::Ime(_)))
                                    })
                                    && ui.input(|i| {
                                        i.key_pressed(egui::Key::Z)
                                            && i.modifiers.command
                                            && !i.modifiers.alt
                                    })
                                {
                                    if let Some((before_text, before_at, after_text)) =
                                        self.auto_undo.clone()
                                    {
                                        if after_text == self.draft_body {
                                            ui.input_mut(|i| {
                                                i.consume_key(
                                                    egui::Modifiers::COMMAND,
                                                    egui::Key::Z,
                                                );
                                            });
                                            self.draft_body = before_text;
                                            self.dirty_since = Some(now);
                                            self.auto_undo = None;
                                            let mut st = egui::text_edit::TextEditState::load(
                                                ui.ctx(),
                                                body_id,
                                            )
                                            .unwrap_or_default();
                                            st.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::one(
                                                    egui::text::CCursor::new(before_at),
                                                ),
                                            ));
                                            st.store(ui.ctx(), body_id);
                                        }
                                    }
                                }
                                // §6.8：行尾回车由编辑器接管——空列表项退出、有内容续写前缀。
                                // 顺序讲究：先**探测**再**消费**，因为不接管时必须把 Enter 留给
                                // TextEdit，否则普通段落按回车会没反应；IME 组合态一律不碰。
                                if self.body_focused
                                    && !ui.input(|i| {
                                        i.events.iter().any(|e| matches!(e, egui::Event::Ime(_)))
                                    })
                                    && ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter)
                                            && !i.modifiers.alt
                                            && !i.modifiers.ctrl
                                            && !i.modifiers.command
                                    })
                                {
                                    let caret_char =
                                        egui::text_edit::TextEditState::load(ui.ctx(), body_id)
                                            .and_then(|st| st.cursor.char_range())
                                            .map(|r| usize::from(r.primary.index));
                                    let edit =
                                        caret_char.and_then(|at| enter_edit(&self.draft_body, at));
                                    if let Some((text, at)) = edit {
                                        ui.input_mut(|i| {
                                            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                                        });
                                        // 带外改动也得给 TextEdit 的 undoer 留一枚还原点，
                                        // 否则 Ctrl+Z 会跳过这一下、退回到更早的陈旧文本（CER-17）。
                                        let before = self.draft_body.clone();
                                        let before_at = caret_char.unwrap_or(0);
                                        self.draft_body = text;
                                        self.dirty_since = Some(now);
                                        self.auto_undo = store_after_edit(
                                            ui.ctx(),
                                            body_id,
                                            at,
                                            &before,
                                            before_at,
                                            &self.draft_body,
                                        );
                                    }
                                }
                                // CE-01 D4 / D4b：Tab 与「行首退格」归编辑器管。
                                // 两条都在 `TextEdit` 之前截：它收到 Tab 会插一个裸
                                // `\t`（排版器不认，只留影子字符），收到 Backspace 只会
                                // 一格一格退（退到 `-` 还留个孤零零的符号）。
                                if self.body_focused && !ime_active {
                                    let shift = ui.input(|i| i.modifiers.shift);
                                    let tab = ui.input(|i| {
                                        i.key_pressed(egui::Key::Tab)
                                            && !i.modifiers.ctrl
                                            && !i.modifiers.alt
                                            && !i.modifiers.command
                                    });
                                    let back = ui.input(|i| {
                                        i.key_pressed(egui::Key::Backspace)
                                            && !i.modifiers.ctrl
                                            && !i.modifiers.alt
                                            && !i.modifiers.command
                                    });
                                    // 读整条光标范围：缩进要能带**
                                    // 整段选区**一起动，所以不能只看主端。
                                    let sel = egui::text_edit::TextEditState::load(ui.ctx(), body_id)
                                        .and_then(|st| st.cursor.char_range())
                                        .map(|r| {
                                            (
                                                usize::from(r.primary.index),
                                                usize::from(r.secondary.index),
                                            )
                                        });
                                    let edit = if tab {
                                        sel.and_then(|(p, s)| {
                                            indent_edit(&self.draft_body, p, s, shift)
                                        })
                                    } else if back {
                                        sel.map(|(p, _)| p)
                                            .and_then(|at| backspace_marker(&self.draft_body, at))
                                            .map(|(t, a)| (t, a, a))
                                    } else {
                                        None
                                    };
                                    if let Some((text, at, to)) = edit {
                                        ui.input_mut(|i| {
                                            if tab {
                                                i.consume_key(
                                                    if shift {
                                                        egui::Modifiers::SHIFT
                                                    } else {
                                                        egui::Modifiers::NONE
                                                    },
                                                    egui::Key::Tab,
                                                );
                                            } else {
                                                i.consume_key(
                                                    egui::Modifiers::NONE,
                                                    egui::Key::Backspace,
                                                );
                                            }
                                        });
                                        crate::trace::event(&format!(
                                            "card_edit op={} from={at} to={to}",
                                            if tab { "indent" } else { "outdent" },
                                        ));
                                        let before = self.draft_body.clone();
                                        let before_at = at;
                                        self.draft_body = text;
                                        self.dirty_since = Some(now);
                                        self.auto_undo = store_after_edit_range(
                                            ui.ctx(),
                                            body_id,
                                            at,
                                            to,
                                            &before,
                                            before_at,
                                            &self.draft_body,
                                        );
                                    }
                                }
                                // `docs/28` §6.5 的第二个触发方式：`Ctrl+/` 叫出斜杠菜单。
                                // 这一下必须**在 `TextEdit` 之前**把字符落进缓冲区——
                                // 菜单靠「行首 `/查询`」判定，晚一帧就看不见了。
                                if !self.read_only
                                    && !ime_active
                                    && ui.input(|i| {
                                        i.modifiers.command
                                            && !i.modifiers.alt
                                            && i.key_pressed(egui::Key::Slash)
                                    })
                                {
                                    ui.input_mut(|i| {
                                        i.consume_key(
                                            egui::Modifiers::COMMAND,
                                            egui::Key::Slash,
                                        );
                                    });
                                    let at0 = egui::text_edit::TextEditState::load(ui.ctx(), body_id)
                                        .and_then(|st| st.cursor.char_range())
                                        .map(|r| usize::from(r.primary.index))
                                        .unwrap_or(0);
                                    let (text, at) = slash_invoke_edit(&self.draft_body, at0);
                                    let before = self.draft_body.clone();
                                    self.draft_body = text;
                                    self.dirty_since = Some(now);
                                    self.focus_body = true;
                                    self.slash_off_at = None;
                                    self.slash_sel = 0;
                                    self.auto_undo = store_after_edit(
                                        ui.ctx(),
                                        body_id,
                                        at,
                                        &before,
                                        at0,
                                        &self.draft_body,
                                    );
                                    crate::trace::event(&format!(
                                        "card_slash_invoke caret={at0} at={at}"
                                    ));
                                }
                                // §CER-17：多行纯文本粘进列表项——后几行要补同型前缀。同样是先探测、
                                // 算出结果再把这个 Paste 事件摘掉自己插；不愿意接管（单行、段落里）就原样留给 TextEdit。
                                if self.body_focused {
                                    let clip = ui.input(|i| {
                                        i.events.iter().find_map(|e| match e {
                                            egui::Event::Paste(s) => Some(s.clone()),
                                            _ => None,
                                        })
                                    });
                                    if let Some(clip) = clip {
                                        let caret_char =
                                            egui::text_edit::TextEditState::load(ui.ctx(), body_id)
                                                .and_then(|st| st.cursor.char_range())
                                                .map(|r| usize::from(r.primary.index));
                                        let edit = caret_char
                                            .and_then(|at| paste_list(&self.draft_body, at, &clip));
                                        if let Some((text, at)) = edit {
                                            ui.input_mut(|i| {
                                                i.events.retain(|e| {
                                                    !matches!(e, egui::Event::Paste(_))
                                                });
                                            });
                                            let before = self.draft_body.clone();
                                            let before_at = caret_char.unwrap_or(0);
                                            self.draft_body = text;
                                            self.dirty_since = Some(now);
                                            // 验收用：粘进来的那一段和落盘后的正文各留一行。
                                            crate::trace::event(&format!(
                                                "card_paste at={before_at} structured={} clip={:?} body={:?}",
                                                clip_has_markdown_structure(&clip),
                                                ellipsis(&clip, 48),
                                                ellipsis(&self.draft_body, 96)
                                            ));
                                            self.auto_undo = store_after_edit(
                                                ui.ctx(),
                                                body_id,
                                                at,
                                                &before,
                                                before_at,
                                                &self.draft_body,
                                            );
                                        }
                                    }
                                }
                                // CER-16：这一帧 TextEdit 之前的正文与光标。
                                // 只在正文有焦点时拍——`marker_undo_step` 要判断「是不是刚敲完块标记」，
                                // 而那一定是「刚好插进一格空格」。
                                let body_before = self.body_focused.then(|| self.draft_body.clone());
                                let caret_before = self.body_focused.then(|| {
                                    egui::text_edit::TextEditState::load(ui.ctx(), body_id)
                                        .and_then(|st| st.cursor.char_range())
                                        .map(|r| usize::from(r.primary.index))
                                        .unwrap_or(0)
                                });
                                let output = egui::TextEdit::multiline(&mut self.draft_body)
                                    .id(body_id)
                                    // 只读时连光标都别进去：`interactive(false)` 之后
                                    // `body_focused` 恒为 false，回车接管 / 粘贴 / 勾选那几条都会自己跳过。
                                    .interactive(!self.read_only)
                                    .frame(egui::Frame::NONE)
                                    .margin(egui::Margin::ZERO)
                                    .desired_width(article)
                                    // 只给一个「够点到」的最小高度：写满 24 行空位会把页尾反链
                                    // 顶到窗口外面（CER-14 实测：反链分隔线落在 y718，正文看不见）。
                                    .desired_rows(6)
                                    .lock_focus(true)
                                    .cursor_at_end(false)
                                    .hint_text(
                                        "开始写…支持 # 标题、**粗体**、`行内代码`、- 列表、> 引用",
                                    )
                                    .layouter(&mut layouter)
                                    .show(ui);

                                // CER-16：「即时转换」的配套——`# ` / `> ` / `- ` / `1. ` 敲下的那一格空格
                                // 让本行成了块标记，就记一步「标记整体撤销」。`Ctrl+Z` 一次退回标记之前，
                                // 而不是像 egui 那样先退空格、留下一个光秃秃的 `#`。
                                // 只在这一格是**空格**且整行恰好只剩标记时成立（见 `marker_undo_step`）。
                                if let (Some(before), Some(at)) = (&body_before, caret_before) {
                                    if let Some((text, at)) =
                                        marker_undo_step(before, at, &self.draft_body)
                                    {
                                        self.auto_undo =
                                            Some((text, at, self.draft_body.clone()));
                                    }
                                    // 验收用：`MW_TRACE` 开着时把每次正文变化记一行，
                                    // 这样「敲下 `## ` 之后缓冲区里到底有什么」不用靠人眼看像素。
                                    if before != &self.draft_body {
                                        crate::trace::event(&format!(
                                            "card_body {:?}",
                                            self.draft_body
                                        ));
                                    }
                                }

                                // 打字机模式：把光标钉在视口 42% 处。
                                //
                                // 只在「这一帧有键盘输入」时开修正窗口（合同 §6.4：鼠标点击不强制居中，
                                // 窗口关掉就撒手，滚轮归用户）。窗口内每帧算同一个**绝对目标**并发出去；
                                // 目标只由「光标内容坐标 − 42% 视口高」决定，与当前偏移无关，重复发是幂等的。
                                // 窗口要盖住 180ms 缓动，因为光标还在视口外时 `TextEdit` 每帧都会排一个
                                // 「拉进视口」的请求，必须由我们逐帧盖回来（`scroll_to_rect` 是覆盖式，
                                // `scroll_with_delta` 则会与它同帧相加，实测多滚 181px）。
                                let typed_now = self.typewriter
                                    && self.body_focused
                                    && ui.input(|i| {
                                        i.events.iter().any(|e| {
                                            matches!(
                                                e,
                                                egui::Event::Text(_)
                                                    | egui::Event::Key { pressed: true, .. }
                                            )
                                        })
                                    });
                                if typed_now {
                                    self.tw_settle = TW_SETTLE_FRAMES;
                                }
                                if self.typewriter && self.body_focused {
                                    if let Some(range) = output.cursor_range {
                                        let caret = output
                                            .galley
                                            .pos_from_cursor(range.primary)
                                            .translate(output.galley_pos.to_vec2())
                                            .center()
                                            .y;
                                        // 视口用 `ui.clip_rect()`：正文 ui 被裁在看不见的滚动视口里，正好是我们要的框。
                                        let clip = ui.clip_rect();
                                        let h = clip.height();
                                        let want = clip.top() + h * TYPEWRITER_AT;
                                        let dy = caret - want;
                                        // 布局用的滚动偏移：滚动区每帧开头把上一帧的状态读出来再布局，
                                        // 所以此刻 `ctx.data` 里存的就是这一帧画面用的那个值。
                                        let off = self
                                            .tw_scroll_id
                                            .and_then(|id| {
                                                egui::containers::scroll_area::State::load(
                                                    ui.ctx(),
                                                    id,
                                                )
                                            })
                                            .map(|s| s.clamped_offset().y)
                                            .unwrap_or(0.0);
                                        let correcting = self.tw_settle > 0 && h > 0.0 && dy.abs() > 1.0;
                                        if self.tw_settle > 0 {
                                            self.tw_settle -= 1;
                                        }
                                        let target = typewriter_target_offset(caret, off, clip.top(), h);
                                        if correcting {
                                            // 给**绝对目标**，不给相对增量。egui 把 `scroll_to_rect` 换算成
                                            // `offset = anchor − 内容顶`，而 `scroll_with_delta` 是加到
                                            // `TextEdit` 自己排的「把光标拉进视口」请求上的——两者同帧相加，
                                            // 实测多滚 181px（42% 被推到 17%），且谁先谁后取决于帧时序，
                                            // 同一份脚本两趟能拍到两种画面。绝对目标则直接覆盖那个请求，
                                            // 又与当前偏移无关，带 180ms 缓动也不会越叠越远。
                                            let anchor = egui::Rect::from_min_size(
                                                egui::pos2(
                                                    clip.center().x,
                                                    target + ui.min_rect().min.y + content_gap,
                                                ),
                                                egui::vec2(1.0, 1.0),
                                            );
                                            let anim = if self.reduce_motion {
                                                egui::style::ScrollAnimation::none()
                                            } else {
                                                egui::style::ScrollAnimation::duration(TYPEWRITER_SECS)
                                            };
                                            ui.scroll_to_rect_animation(anchor, Some(egui::Align::Min), anim);
                                        }
                                        if let Some(path) = tw_probe_path() {
                                            let ratio = if h > 0.0 { (caret - clip.top()) / h } else { 0.0 };
                                            let line = format!(
                                                "{} caret_y={caret:.1} view_top={:.1} view_h={h:.1} want={want:.1} dy={dy:.1} off={off:.1} target={target:.1} ratio={ratio:.3} settle={} reduce={}\n",
                                                if correcting { "[tw]" } else { "[tw-idle]" },
                                                clip.top(),
                                                self.tw_settle,
                                                self.reduce_motion,
                                            );
                                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                                .create(true)
                                                .append(true)
                                                .open(path)
                                            {
                                                use std::io::Write as _;
                                                let _ = f.write_all(line.as_bytes());
                                            }
                                        }
                                    }
                                }

                                if let Some((at, checked)) = paint_block_decorations(
                                    ui,
                                    &output,
                                    &self.draft_body,
                                    active.map(|(l, _)| l),
                                    palette,
                                    focus_dim,
                                ) {
                                    // 只读时画还是要画（成稿该有的样子），但不许点着改。
                                    let editable = !self.read_only;
                                    if editable {
                                        if let Some((text, line_end)) =
                                            task_flip(&self.draft_body, at)
                                        {
                                        crate::trace::event(&format!(
                                            "card_task_toggle at={at} checked={checked} line_end={line_end}"
                                        ));
                                        self.draft_body = text;
                                        self.dirty_since = Some(now);
                                        // 光标别停在方框上：挪到行尾，继续写就是下一句。
                                        if let Some(mut state) =
                                            egui::text_edit::TextEditState::load(ctx, body_id)
                                        {
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::one(
                                                    egui::text::CCursor::new(line_end),
                                                ),
                                            ));
                                            state.store(ctx, body_id);
                                        }
                                        ctx.memory_mut(|m| m.request_focus(body_id));
                                        }
                                    }
                                }
                                // ── 斜杠菜单：光标停在行首 `/查询` 上时浮出 ──
                                let caret_now = output.cursor_range.map(|r| {
                                    r.primary.index.0.min(self.draft_body.chars().count())
                                });
                                if let Some(sc) = caret_now
                                    .filter(|_| output.response.has_focus())
                                    .and_then(|c| slash_context(&self.draft_body, c))
                                {
                                    if self.slash_off_at != Some(sc.start_char) {
                                        if let Some(pick) = slash_menu(
                                            ctx,
                                            ch,
                                            screen,
                                            &output,
                                            caret_now.unwrap_or(0),
                                            &sc.query,
                                            &mut self.slash_sel,
                                        ) {
                                            let (body, new_caret) = slash_apply_text(
                                                &self.draft_body,
                                                &sc,
                                                caret_now.unwrap_or(0),
                                                &SLASH_ITEMS[pick],
                                            );
                                            self.draft_body = body;
                                            self.dirty_since = Some(now);
                                            self.slash_off_at = None;
                                            self.slash_sel = 0;
                                            self.store_caret(ctx, body_id, new_caret);
                                            ctx.memory_mut(|m| m.request_focus(body_id));
                                        }
                                    }
                                } else if self.slash_off_at.is_some() {
                                    self.slash_off_at = None;
                                }
                                if self.focus_body {
                                    output.response.request_focus();
                                    self.focus_body = false;
                                    self.body_focused = true;
                                } else if output.response.has_focus() {
                                    self.body_focused = true;
                                    if let Some(range) = output.cursor_range {
                                        let idx = range
                                            .primary
                                            .index
                                            .0
                                            .min(self.draft_body.chars().count());
                                        let line = line_index_at(&self.draft_body, idx);
                                        self.active =
                                            Some((line, col_in_line(&self.draft_body, idx)));
                                    }
                                } else {
                                    self.body_focused = false;
                                }
                                // Ctrl+点击正文里的链接：交给系统浏览器打开。
                                if output.response.clicked() && ui.input(|i| i.modifiers.command) {
                                    if let Some(range) = output.cursor_range {
                                        if let Some(url) =
                                            link_at(&self.draft_body, range.primary.index.0)
                                        {
                                            action = Some(ReaderAction::OpenLink(url));
                                        }
                                    }
                                }
                                if let Some(idx) = self.scroll_to.take() {
                                    let rect = output
                                        .galley
                                        .pos_from_cursor(egui::text::CCursor::new(idx))
                                        .translate(output.galley_pos.to_vec2());
                                    ui.scroll_to_rect(rect, Some(Align::TOP));
                                }
                                self.backlink_footer(ui, ch, &mut action);
                                ui.add_space(96.0);
                            });
                        });
                        // 记下滚动区 id：下一帧用它读「这一帧布局用的偏移」。
                        self.tw_scroll_id = Some(scroll_out.id);
                        scroll_metrics = Some((
                            scroll_out.content_size.y,
                            scroll_out.inner_rect,
                            scroll_out.state.clamped_offset().y,
                        ));
                        // 过程证据：`MW_TW_PROBE=<文件>` 时把「每次落定后的偏移」追加进文件，验收脚本据此核对
                        // 「光标停在视口 42%」而不是靠肉眼数像素。只在偏移真的变了的时候记一行。
                        // 对照组（不开打字机）也记：两组偏移相减就是 A/B 图里正文整体挪了多少。
                        if self.typewriter || tw_probe_path().is_some() {
                            let off = scroll_out.state.clamped_offset().y;
                            if (off - self.tw_last_off).abs() > 0.5 {
                                self.tw_last_off = off;
                                if let Some(path) = tw_probe_path() {
                                    let top = scroll_out.inner_rect.top();
                                    let h = scroll_out.inner_rect.height();
                                    let max_off = (scroll_out.content_size.y - h).max(0.0);
                                    let line = format!(
                                        "[tw-apply] off={off:.1} view_top={top:.1} view_h={h:.1} max={max_off:.1} content={:.1}\n",
                                        scroll_out.content_size.y
                                    );
                                    if let Ok(mut f) = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                    {
                                        use std::io::Write as _;
                                        let _ = f.write_all(line.as_bytes());
                                    }
                                }
                            }
                        }
                    });
                });
                // 滚动条画在版心之外、窗口右缘：`horizontal_top` 收口后
                // 再落笔，`ui` 的裁剪框还是整页，盖不住也裁不掉。
                if let Some((content_h, viewport, offset)) = scroll_metrics {
                    self.draw_scroll_bar(ui, ch, screen, content_h, viewport, offset);
                }
            });

        if let Some(a) = action {
            return Some(a);
        }
        if ctrl_s {
            return Some(self.save_action(false));
        }

        // 草稿变化 → 重置自动保存计时；停手 800ms 没再输入就落盘（`docs/28` §6.7）。
        let changed = self.draft_title != before.0
            || self.draft_body != before.1
            || self.draft_tags != before.2;
        if changed {
            self.dirty_since = Some(now);
            // 刚打了一个字就把上一次失败的提示收掉：草稿还在，重试会自动来。
            self.save_error = None;
        }
        if let (true, Some(delay)) = (self.is_dirty() && !self.read_only, autosave_delay()) {
            let idle_from = self.dirty_since;
            if autosave_due(true, self.saving, idle_from, now, delay) {
                // 先摆出「保存中…」：真正写盘在壳层的下一帧（与文档 `SavePhase::Saving` 同款），
                // 这样顶栏那一帧说的就是真话。
                self.mark_saving();
                crate::trace::event("card_autosave");
                return Some(self.save_action(false));
            }
        }
        None
    }

    /// 卡片页自己的滚动条（CE-01）：贴**窗口**右缘、休眠透明、悬停或滚动
    /// 淡入 120ms、放手 300ms 后淡出，滑块可拖。口径见 `crate::scroll_bar`。
    ///
    /// 为什么不用 `ScrollArea` 自带的那条：它是浮动式，休眠态 `dormant_handle_opacity
    /// = 0.0`（看不见），滚动时才浮出来且盖在文字上；横向位置跟着 `ScrollArea`
    /// 的宽度走——卡片页的 `ScrollArea` 只有版心那么宽，条就落在版心右缘，
    /// 离窗口右缘还差一大截（`docs/28` §4.5）。
    fn draw_scroll_bar(
        &mut self,
        ui: &mut egui::Ui,
        ch: &Chrome,
        screen: Rect,
        content_h: f32,
        viewport: Rect,
        offset: f32,
    ) {
        let spec = crate::scroll_bar::Spec::default();
        // 横向取整页、纵向取滚动视口：条的上下端点跟着正文区，横坐标贴窗口右缘。
        let area = Rect::from_min_max(
            pos2(screen.left(), viewport.top()),
            pos2(screen.right(), viewport.bottom()),
        );
        let Some(bar) = crate::scroll_bar::layout(area, content_h, offset, spec) else {
            // 不满一屏：条整条收掉，别在右侧留一段空轨道。
            if self.scroll_bar_on {
                self.scroll_bar_on = false;
                crate::trace::event(&format!(
                    "card_scroll off content={content_h:.1} view={:.1}",
                    viewport.height()
                ));
            }
            self.scroll_fade = crate::scroll_bar::Fade::default();
            self.scroll_seen = offset;
            self.scroll_hot = false;
            return;
        };
        if !self.scroll_bar_on {
            self.scroll_bar_on = true;
            crate::trace::event(&format!(
                "card_scroll on content={content_h:.1} view={:.1} top={:.1} bottom={:.1} x=[{:.1},{:.1}]",
                viewport.height(),
                viewport.top(),
                viewport.bottom(),
                bar.track.left(),
                bar.track.right()
            ));
        }
        // 滚轮/键盘滚过也算「热」——不然滚完那一下什么都没出现，用户不知道自己在哪。
        let moved = (offset - self.scroll_seen).abs() > 0.5;
        self.scroll_seen = offset;
        let now = ui.input(|i| i.time);
        // 与专注淡出同一口径：`stable_dt` 在离屏第一帧可能是 0，夹一个下限，
        // 否则金样里那条滚动条永远淡不出来（每帧步进 0）。
        let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05);
        let hot = moved || self.scroll_hot;
        let fade = self.scroll_fade.advance(now, hot, dt);
        // 终值透明度对齐开始页那套（`start.rs` 的 IDLE 130 / HOT 200 / DRAG 235），
        // 淡入淡出只调「到没到」，不另立一套深浅。
        let peak = if self.scroll_hot { 200.0 } else { 130.0 } / 255.0;
        let alpha = fade * peak;
        let was_hot = self.scroll_hot;
        let out = crate::scroll_bar::paint(
            ui,
            bar,
            spec,
            // alpha 折算成颜色：绘制层只认「这一帧的墨有多浓」。
            egui::Color32::from_rgba_unmultiplied(
                ch.dim.r(),
                ch.dim.g(),
                ch.dim.b(),
                (255.0 * alpha.clamp(0.0, 1.0)).round() as u8,
            ),
            Id::unique("mw-card-scroll-bar"),
            content_h,
            viewport.height(),
        );
        if out.hot != was_hot || out.request.is_some() {
            crate::trace::event(&format!(
                "card_scroll hot={} off={offset:.1} alpha={alpha:.2} req={}",
                out.hot,
                out.request.is_some()
            ));
        }
        self.scroll_hot = out.hot;
        if let Some(y) = out.request {
            // 下一帧交给 `ScrollArea`：`State.offset` 是私有的，只能从构造参数回灌。
            self.scroll_req = Some(y);
        }
    }

    /// H1 下方的元信息行与标签行（CER-14，`docs/28` §6.1）。
    ///
    /// 顺序是合同定死的：H1（30/42）→ 8 → 元信息（13/18）→ 12 → 标签（13/20）→ 40 → 正文。
    /// 标签是**纯文字** `#知识/方法`：平时没有底色、没有边框、没有彩色 chip，
    /// hover 才出现下划线与删除 ×。早先那版是彩色 chip，与合同直接冲突。
    fn meta_and_tags(
        &mut self,
        ui: &mut egui::Ui,
        ch: &Chrome,
        action: &mut Option<ReaderAction>,
        now: std::time::Instant,
    ) {
        // 行高按合同来：元信息 13/18、标签 13/20。控件默认交互高度是 30，
        // 不压下去的话这两行会各撑到 30，H1 到正文的间距就全错了。
        ui.spacing_mut().interact_size.y = 18.0;
        let meta = Type::SMALL;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(
                RichText::new(format!("更新于 {}", format_date(self.updated_at)))
                    .size(meta)
                    .color(ch.dim),
            );
            if let Some(src) = &self.source {
                ui.label(RichText::new("  ·  来源：").size(meta).color(ch.dim));
                let label = format!("{} › {}", src.doc_label, src.text);
                if src.missing {
                    // 源节点没了就不要给假按钮：写成事实，卡片本身还在。
                    ui.label(
                        RichText::new(format!("{label}（源节点已删除）"))
                            .size(meta)
                            .color(ch.dim),
                    );
                } else if ui
                    .link(RichText::new(label).size(meta).color(ch.dim))
                    .on_hover_text(if src.concept_map {
                        "回到概念图"
                    } else {
                        "回到导图节点"
                    })
                    .clicked()
                {
                    *action = Some(ReaderAction::JumpToNode(src.node_id.clone()));
                }
            }
            ui.label(
                RichText::new(format!("  ·  {} 字", self.word_count()))
                    .size(meta)
                    .color(ch.dim),
            );
        });

        // 没有标签时整行都不占位：留一个 20px 空行会让正文多下移 32px。
        // 给无标签的卡片加标签走顶栏「更多 → 标签管理」（偏差见 CER-14 报告 §4）。
        if self.draft_tags.is_empty() && self.tag_input.is_empty() {
            return;
        }
        ui.spacing_mut().interact_size.y = 20.0;
        ui.add_space(12.0);
        let mut remove: Option<usize> = None;
        let mut new_tag: Option<String> = None;
        let tags = self.draft_tags.clone();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let row_start = ui.cursor().min;
            for (i, tag) in tags.iter().enumerate() {
                let resp = ui.add(
                    egui::Label::new(
                        RichText::new(format!("#{tag}"))
                            .size(Type::SMALL)
                            .color(ch.dim),
                    )
                    .sense(Sense::hover()),
                );
                // 删除 × 有**自己的命中区**，但不占版面：命中框落在两个标签之间的
                // 空隙里（右侧 12px），所以标签间距仍是合同里的 8，不会因为多了个
                // 按钮就松掉。以前整枚标签都是删除按钮——手一滑点上去标签就没了，
                // 而且没有任何撤销。
                let x_rect = Rect::from_min_size(
                    pos2(resp.rect.right() + 1.0, resp.rect.center().y - 8.0),
                    vec2(12.0, 16.0),
                );
                let x_resp = ui.interact(
                    x_rect,
                    ui.make_persistent_id(("mw-card-tag-x", i)),
                    Sense::click(),
                );
                if resp.hovered() || x_resp.hovered() {
                    let r = resp.rect;
                    ui.painter().line_segment(
                        [r.left_bottom(), r.right_bottom()],
                        Stroke::new(1.0, ch.faint),
                    );
                    crate::ui_kit::paint_x(
                        ui.painter(),
                        x_rect.center(),
                        2.4,
                        Stroke::new(1.1, if x_resp.hovered() { ch.warn } else { ch.faint }),
                    );
                }
                if x_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if x_resp.clicked() {
                    crate::trace::event(&format!("card_tag_remove {tag:?}"));
                    remove = Some(i);
                }
            }
            // 「+ 标签」先占位再决定画不画：行高、行宽不随指针变化，
            // golden 才不会因为鼠标停在哪而漂；而一行纯文字也不该常挂一条灰提示。
            let slot = ui.allocate_exact_size(vec2(96.0, 20.0), Sense::hover()).0;
            let tag_input_id = Id::unique("mw-card-tag-input");
            let focused = ui.ctx().memory(|m| m.has_focus(tag_input_id));
            // `docs/28` §6.6：输入框空着按退格，删掉最后一个标签。
            // 必须在 `TextEdit` 之前截：它对「空输入按退格」是**什么都不做**，
            // 而这里要的是「退回上一个标签」——连着按就把标签一个个退掉。
            if focused && self.tag_input.is_empty() && ui.input(|i| i.key_pressed(Key::Backspace)) {
                ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace));
                if let Some(t) = self.draft_tags.pop() {
                    self.dirty_since = Some(now);
                    crate::trace::event(&format!("card_tag_pop {t:?}"));
                }
            }
            if ui.rect_contains_pointer(Rect::from_min_max(row_start, slot.max))
                || focused
                || !self.tag_input.is_empty()
            {
                let resp = ui.put(
                    slot,
                    egui::TextEdit::singleline(&mut self.tag_input)
                        .id(tag_input_id)
                        .hint_text("+ 标签")
                        .frame(egui::Frame::NONE)
                        .desired_width(96.0),
                );
                if (resp.has_focus() || resp.lost_focus())
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && !self.tag_input.trim().is_empty()
                {
                    new_tag = Some(self.tag_input.clone());
                }
            }
        });
        if let Some(i) = remove {
            self.draft_tags.remove(i);
            self.dirty_since = Some(now);
        }
        if let Some(t) = new_tag {
            self.add_tag(&t);
            // 单行输入框按回车会主动交还焦点；连着加标签时把焦点还给它。
            ui.ctx()
                .memory_mut(|m| m.request_focus(Id::unique("mw-card-tag-input")));
            self.dirty_since = Some(now);
        }
    }

    /// 页尾反链（`docs/28` §6.2：32 留白 + 1px 分隔线 + 「反向链接」13/20/600 + 条目 13/20）。
    ///
    /// 断链（`[[引用]]` 指向不存在的卡片）单独一行、用辅助色，不给点——
    /// 点不出东西的链接比不显示更糟。
    fn backlink_footer(
        &mut self,
        ui: &mut egui::Ui,
        ch: &Chrome,
        action: &mut Option<ReaderAction>,
    ) {
        if self.backlinks.is_empty() && self.broken_refs.is_empty() {
            return;
        }
        ui.add_space(32.0);
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0.0, ch.sep);
        ui.add_space(12.0);
        if !self.backlinks.is_empty() {
            ui.label(
                RichText::new("反向链接")
                    .size(Type::SMALL)
                    .line_height(Some(20.0))
                    .family(FontFamily::Name(crate::fonts::BOLD.into()))
                    .color(ch.dim),
            );
            ui.add_space(4.0);
            for b in self.backlinks.clone() {
                let resp = ui
                    .add(
                        egui::Label::new(
                            RichText::new(&b.text)
                                .size(Type::SMALL)
                                .line_height(Some(20.0))
                                .color(ch.accent),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_text("跳到这条反链的来源");
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    *action = Some(if b.card {
                        ReaderAction::OpenCard(b.node_id.clone())
                    } else {
                        ReaderAction::JumpToNode(b.node_id.clone())
                    });
                }
            }
        }
        if !self.broken_refs.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("断链 {}", self.broken_refs.join("、")))
                    .size(Type::BODY)
                    .color(ch.faint),
            );
        }
    }

    /// 把「标题框 + 正文」拼回磁盘上的那份 Markdown。
    ///
    /// 打开时摘掉过 `# 标题` 这一行，这里按原来的分隔形态拼回去——
    /// 用户没动过的卡片，保存出去必须**逐字节**等于读进来的那份。
    fn compose_body(&self) -> String {
        let t = self.draft_title.trim();
        match self.header {
            TitleHeader::Absent => self.draft_body.clone(),
            // 标题被清空：那一行就真的不该存在了。
            TitleHeader::Only if t.is_empty() => String::new(),
            TitleHeader::Only => {
                if self.draft_body.is_empty() {
                    format!("# {t}")
                } else {
                    format!("# {t}\n{}", self.draft_body)
                }
            }
            TitleHeader::Split { .. } if t.is_empty() => self.draft_body.clone(),
            TitleHeader::Split { blank, crlf } => {
                let nl = if crlf { "\r\n" } else { "\n" };
                let sep = if blank {
                    format!("{nl}{nl}")
                } else {
                    nl.to_string()
                };
                format!("# {t}{sep}{}", self.draft_body)
            }
        }
    }

    fn save_action(&self, close: bool) -> ReaderAction {
        ReaderAction::Save {
            title: self.draft_title.clone(),
            body: self.compose_body(),
            tags: self.draft_tags.clone(),
            close,
        }
    }
}

/// 正文开头那行 `# 标题` 的形态（CE-01 D15）。
///
/// `docs/28` §6.10 禁止标题重复：正文第一行就是标题时，它该由标题框显示，
/// 而不是在下面再画一次。打开时摘掉、保存时按原样拼回去，**一个字节都不改**。
/// 分这么细是因为拼接要把分隔空行、甚至「整篇只有一行标题」也还原。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TitleHeader {
    /// 正文没有单独的一级标题行（或者那行不是标题，比如 `# 标题党`）。
    Absent,
    /// 整篇就只有 `# 标题`，后面什么都没有。
    Only,
    /// `# 标题` + 换行 + 正文；`blank` = 后面还空了一行。
    Split { blank: bool, crlf: bool },
}

/// 把正文开头的 `# 标题` 摘出来，返回（形态, 剩下的正文）。
///
/// 只认**完全一致**的标题：`# 标题党` 不算，`#标题`（没有空格）也不算。
fn split_title_header<'a>(title: &str, body: &'a str) -> (TitleHeader, &'a str) {
    let t = title.trim();
    if t.is_empty() {
        return (TitleHeader::Absent, body);
    }
    let head = format!("# {t}");
    let Some(rest) = body.strip_prefix(&head) else {
        return (TitleHeader::Absent, body);
    };
    if rest.is_empty() {
        return (TitleHeader::Only, "");
    }
    // 换行可能是 LF 也可能是 CRLF：摘的时候一起带走，拼的时候再还原。
    let (crlf, after) = if let Some(r) = rest.strip_prefix("\r\n") {
        (true, r)
    } else if let Some(r) = rest.strip_prefix('\n') {
        (false, r)
    } else {
        // `# 标题` 后面直接跟字（`# 标题党`）——那不是标题行，别动它。
        return (TitleHeader::Absent, body);
    };
    let nl = if crlf { "\r\n" } else { "\n" };
    let blank = after.starts_with(nl);
    let after = if blank { &after[nl.len()..] } else { after };
    (TitleHeader::Split { blank, crlf }, after)
}

/// 标题留空时取正文第一行非空内容，去掉 `#` 前缀；再不行就叫「未命名卡片」。
fn derive_title(title: &str, body: &str) -> String {
    if !title.trim().is_empty() {
        return title.trim().to_string();
    }
    body.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "未命名卡片".into())
}

/// 标签颜色：同一路径根永远同色，不依赖文档/主题，重启也稳定。
pub fn tag_color(tag: &str) -> Color32 {
    const PALETTE: [Color32; 8] = [
        crate::chrome::c32(mw_core::design::shell::tag::BLUE),
        crate::chrome::c32(mw_core::design::shell::tag::GREEN),
        crate::chrome::c32(mw_core::design::shell::tag::YELLOW),
        crate::chrome::c32(mw_core::design::shell::tag::RED),
        crate::chrome::c32(mw_core::design::shell::tag::PURPLE),
        crate::chrome::c32(mw_core::design::shell::tag::CYAN),
        crate::chrome::c32(mw_core::design::shell::tag::ORANGE),
        crate::chrome::c32(mw_core::design::shell::tag::TEAL),
    ];
    let root = mw_core::note::tag_root(tag);
    let h = root
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    PALETTE[h as usize % PALETTE.len()]
}

/// 从正文原始行提取目录：跳过代码围栏里的「#」，并给出字符下标供跳转。
#[allow(dead_code)]
fn headings(body: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut in_code = false;
    let mut chars = 0usize;
    for line in body.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = content.trim_start().trim_end_matches('\r');
        if trimmed.starts_with("```") {
            in_code = !in_code;
            chars += line.chars().count();
            continue;
        }
        if in_code {
            chars += line.chars().count();
            continue;
        }
        let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
        let after = &trimmed[hashes..];
        if (1..=6).contains(&hashes) && (after.is_empty() || after.starts_with(' ')) {
            let text = after.trim().to_string();
            if !text.is_empty() {
                let lead = content.len() - trimmed.len();
                out.push(Heading {
                    char_index: chars + content[..lead].chars().count(),
                    level: hashes as u8,
                    text,
                });
            }
        }
        chars += line.chars().count();
    }
    out
}

/// 字数：中文（含日韩）按字、英文按词，标点与空白不算。
pub fn reading_count(text: &str) -> usize {
    fn is_cjk(ch: char) -> bool {
        matches!(
            ch as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0x3040..=0x30FF
                | 0xAC00..=0xD7AF
        )
    }
    let mut count = 0;
    let mut in_word = false;
    for ch in text.chars() {
        if is_cjk(ch) {
            count += 1;
            in_word = false;
        } else if ch.is_alphanumeric() {
            if !in_word {
                count += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    count
}

/// 自动保存后的「更新于」日期。金样用墙钟会在 UTC 跨日（本地 08:00）翻页，
/// 让 `card-autosave` 每天漂一格数字；`MW_CARD_NOW_MS` 把这一刻钉在演示时间戳上。
fn card_now_ms() -> i64 {
    std::env::var("MW_CARD_NOW_MS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or_else(mw_core::now_ms)
}

/// `2026-10-20`（UTC）。只用于卡片元信息，不做时区换算。
pub fn format_date(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    // Howard Hinnant 的 days → civil date 算法（公历）。
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

// ───────────────────────── 斜杠菜单 ─────────────────────────

/// 斜杠菜单的一项：插入的 Markdown 前缀与光标要回退的字符数。
struct SlashItem {
    label: &'static str,
    hint: &'static str,
    keys: &'static [&'static str],
    insert: &'static str,
    caret_back: usize,
}

/// 浮层几何（`docs/28` §5、`docs/29` CER-18）：280 宽、行高 30、圆角 10、**最多 8 行**。
/// 专注模式淡入淡出时长（秒）：合同 120ms。
const FOCUS_SECS: f32 = 0.120;
/// 专注模式下非当前块的最终不透明度：合同 35%。
const FOCUS_DIM: f32 = 0.35;

/// 打字机模式把光标锚在正文可视区的这个高度比例：合同 42%。
const TYPEWRITER_AT: f32 = 0.42;
/// 打字机滚动时长（秒）：合同 180ms。
const TYPEWRITER_SECS: f32 = 0.180;
/// 打字机修正窗口的帧数：键盘输入后最多纠正这么多帧。
///
/// 要盖住 180ms 的缓动（约 11 帧，留余量到 16）。窗口内每帧都请求同一个**绝对目标**
/// ——目标只由「光标内容坐标 − 42% 视口高」算出，与当前偏移无关，重复请求是幂等的；
/// 而光标还在视口外时，`TextEdit` 每帧都会排一个「拉进视口」的请求把它盖掉，
/// 必须由我们逐帧盖回来。落位后 `dy` 归零，窗口内不再发请求。
const TW_SETTLE_FRAMES: u8 = 16;

const SLASH_W: f32 = 280.0;
const SLASH_ROW_H: f32 = 30.0;
const SLASH_RADIUS: u8 = 10;
const SLASH_PAD: f32 = 6.0;
const SLASH_MAX_ROWS: usize = 8;

/// 自动保存去抖：停手这么久没有新输入才落盘（`docs/28` §6.7「去抖 800ms」）。
const AUTOSAVE: std::time::Duration = std::time::Duration::from_millis(800);

/// 这一帧该不该自动保存：脏、没在写、且停手满 `delay`。
///
/// 抽成纯函数是为了能在 `cargo test` 里钉住「一直打字不落盘」「停手才落盘」
/// 「写盘途中不重复排队」三条——它们都是「丢稿 / 打满磁盘」这类事故的边界。
fn autosave_due(
    dirty: bool,
    saving: bool,
    idle_from: Option<std::time::Instant>,
    now: std::time::Instant,
    delay: std::time::Duration,
) -> bool {
    if !dirty || saving {
        return false;
    }
    idle_from.is_some_and(|t| now.saturating_duration_since(t) >= delay)
}

/// 自动保存的去抖时长；`None` = 这一轮不自动保存。
///
/// 金样（有 `MW_GOLDEN_INPUT` 脚本）默认关掉：截图和真实时钟赛跑，同一份构建会拍出
/// 「有时已保存、有时还在编辑」两张图。要拍这一格，脚本显式给 `MW_CARD_AUTOSAVE=<ms>`，
/// 配一段 `sleep` 把时间推过去即可（短阈值 + 长睡眠，判定离边界很远）。
fn autosave_delay() -> Option<std::time::Duration> {
    static DELAY: std::sync::OnceLock<Option<std::time::Duration>> = std::sync::OnceLock::new();
    *DELAY.get_or_init(|| {
        if let Some(ms) = std::env::var("MW_CARD_AUTOSAVE")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
        {
            return Some(std::time::Duration::from_millis(ms));
        }
        if std::env::var_os("MW_GOLDEN_INPUT").is_some() {
            return None;
        }
        Some(AUTOSAVE)
    })
}

/// 可见行数：超过 8 条就滚动，菜单不再往高里长。
fn slash_visible_rows(total: usize) -> usize {
    total.min(SLASH_MAX_ROWS)
}

const SLASH_ITEMS: [SlashItem; 10] = [
    SlashItem {
        label: "标题 1",
        hint: "#",
        keys: &["h1", "heading", "biaoti", "标题"],
        insert: "# ",
        caret_back: 0,
    },
    SlashItem {
        label: "标题 2",
        hint: "##",
        keys: &["h2", "heading", "biaoti", "标题"],
        insert: "## ",
        caret_back: 0,
    },
    SlashItem {
        label: "标题 3",
        hint: "###",
        keys: &["h3", "heading", "biaoti", "标题"],
        insert: "### ",
        caret_back: 0,
    },
    SlashItem {
        label: "无序列表",
        hint: "-",
        keys: &["ul", "list", "liebiao", "列表"],
        insert: "- ",
        caret_back: 0,
    },
    SlashItem {
        label: "有序列表",
        hint: "1.",
        keys: &["ol", "ordered", "youxu", "有序", "编号"],
        insert: "1. ",
        caret_back: 0,
    },
    SlashItem {
        label: "待办",
        hint: "[ ]",
        keys: &["todo", "task", "renwu", "待办", "任务"],
        insert: "- [ ] ",
        caret_back: 0,
    },
    SlashItem {
        label: "引用",
        hint: ">",
        keys: &["quote", "blockquote", "yinyong", "引用"],
        insert: "> ",
        caret_back: 0,
    },
    SlashItem {
        label: "代码块",
        hint: "```",
        keys: &["code", "codeblock", "daima", "代码"],
        insert: "```\n\n```",
        // 光标落在围栏中间的空行，直接开始写。
        caret_back: 4,
    },
    SlashItem {
        label: "分割线",
        hint: "---",
        keys: &["hr", "divider", "fenge", "分割线"],
        insert: "---\n",
        caret_back: 0,
    },
    SlashItem {
        label: "图片",
        hint: "![]()",
        keys: &["img", "image", "tupian", "图片"],
        insert: "![图片]()",
        // 光标落在 `![` 之后、`]` 之前，先改说明文字，再进括号填路径。
        caret_back: 3,
    },
];
/// 行首的 `/查询` 上下文：`start_char` 是斜杠的字符下标。
struct SlashCtx {
    start_char: usize,
    query: String,
}

/// 光标前一行是否正好是 `/` 或 `/查询`（查询里不能有空白，粘上空格就关闭菜单）。
/// 行首允许缩进；`abc/` 这种正文里的斜杠不算。
fn slash_context(text: &str, caret: usize) -> Option<SlashCtx> {
    let byte = char_to_byte(text, caret);
    let line_start = text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &text[line_start..byte];
    let trimmed = line.trim_start();
    let query = trimmed.strip_prefix('/')?;
    if query.chars().any(|c| c.is_whitespace() || c == '/') {
        return None;
    }
    let lead = line.len() - trimmed.len();
    let start_char = text[..line_start].chars().count() + line[..lead].chars().count();
    Some(SlashCtx {
        start_char,
        query: query.to_string(),
    })
}

/// 过滤菜单项。空查询列出全部；中文标签与英文别名都认。
fn slash_matches(query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    SLASH_ITEMS
        .iter()
        .enumerate()
        .filter(|(_, it)| {
            q.is_empty()
                || it.label.to_lowercase().contains(&q)
                || it.keys.iter().any(|k| k.to_lowercase().starts_with(&q))
        })
        .map(|(i, _)| i)
        .collect()
}

/// 把 `/查询` 换成菜单项的前缀，返回新正文与新光标（字符下标）。
fn slash_apply_text(body: &str, ctx: &SlashCtx, caret: usize, item: &SlashItem) -> (String, usize) {
    let start_byte = char_to_byte(body, ctx.start_char);
    let end_byte = char_to_byte(body, caret);
    let mut out = String::with_capacity(body.len() + item.insert.len());
    out.push_str(&body[..start_byte]);
    out.push_str(item.insert);
    out.push_str(&body[end_byte..]);
    let inserted = item.insert.chars().count();
    let caret = ctx.start_char + inserted - item.caret_back.min(inserted);
    (out, caret)
}

/// 在光标下方浮出斜杠菜单；返回被点中的项（键盘选择在 TextEdit 之前处理）。
#[allow(clippy::too_many_arguments)]
fn slash_menu(
    ctx: &egui::Context,
    ch: &Chrome,
    screen: Rect,
    output: &egui::text_edit::TextEditOutput,
    caret: usize,
    query: &str,
    sel: &mut usize,
) -> Option<usize> {
    let options = slash_matches(query);
    if options.is_empty() {
        return None;
    }
    *sel = (*sel).min(options.len() - 1);
    let cursor = output
        .galley
        .pos_from_cursor(egui::text::CCursor::new(caret))
        .translate(output.galley_pos.to_vec2());
    let inner_w = SLASH_W - SLASH_PAD * 2.0;
    let rows = slash_visible_rows(options.len());
    // 只算**看得见**的那几行：超过 8 条就滚动，菜单不会长到屏幕外。
    let h = rows as f32 * SLASH_ROW_H + SLASH_PAD * 2.0;
    let mut pos = pos2(cursor.left(), cursor.bottom() + 6.0);
    if pos.y + h > screen.bottom() - 8.0 {
        pos.y = (cursor.top() - h - 6.0).max(screen.top() + 8.0);
    }
    let x_max = screen.right() - SLASH_W - 12.0;
    if pos.x > x_max {
        pos.x = x_max;
    }
    let mut picked = None;
    egui::Area::new(Id::unique("mw-card-slash-menu"))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            // 与标题菜单 / 命令面板共用同一档投影（`POPOVER` 令牌），只按合同覆写底板与圆角。
            crate::ui_kit::menu_frame(*ch)
                .corner_radius(SLASH_RADIUS)
                .fill(ch.panel2)
                .show(ui, |ui| {
                    ui.set_width(inner_w);
                    // 行距必须归零：egui 默认 item_spacing.y = 8 会叠在 30 的行高外面，
                    // 实测行距变成 38 CSS，「最多 8 行」就只剩 6 行半（`docs/29` 要的是行高 30 的节距）。
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // 自己算滚动位置，不用 `scroll_to_me`：后者带动画，会给 golden 拍出
                    // 两张彼此不一致的帧（实测同一场景两遍差 9915 / 8269 px）。
                    let keep_in_view =
                        (*sel).saturating_sub(rows.saturating_sub(1)) as f32 * SLASH_ROW_H;
                    egui::ScrollArea::vertical()
                        .max_height(rows as f32 * SLASH_ROW_H)
                        .auto_shrink([false, true])
                        .vertical_scroll_offset(keep_in_view)
                        .show(ui, |ui| {
                            for (i, &idx) in options.iter().enumerate() {
                                let it = &SLASH_ITEMS[idx];
                                let active = i == *sel;
                                let resp =
                                    ui.add(
                                        egui::Button::new(
                                            RichText::new(it.label)
                                                .size(Type::BODY)
                                                .color(if active { ch.text } else { ch.dim }),
                                        )
                                        .frame(false)
                                        .fill(if active {
                                            ch.accent_soft
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .corner_radius(5.0)
                                        .min_size(vec2(inner_w, SLASH_ROW_H)),
                                    );
                                // 提示固定在右列，不靠数空格对齐：中英混排时空格宽度不一样。
                                ui.painter().text(
                                    pos2(resp.rect.right() - 10.0, resp.rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    it.hint,
                                    FontId::proportional(Type::CAPTION),
                                    if active { ch.dim } else { ch.faint },
                                );

                                if resp.hovered() {
                                    *sel = i;
                                }
                                if resp.clicked() {
                                    picked = Some(idx);
                                }
                            }
                        });
                });
        });
    picked
}

// ───────────────────────── Markdown 实时排版 ─────────────────────────

#[derive(Clone, Copy)]
pub(crate) struct MdPalette {
    text: Color32,
    dim: Color32,
    faint: Color32,
    accent: Color32,
    code_bg: Color32,
    /// 页面底色：绘制层盖住标记时用它，和背景严丝合缝。
    bg: Color32,
}

impl MdPalette {
    /// 内容表面版配色：笔记库预览与卡片页共用同一套排版（同一棵 AST、同一份
    /// `markdown_job`），但颜色跟 `design::content` 令牌走，不跟壳层 chrome 走
    /// ——`docs/28` §7.5「复用 §4.1 的阅读正文，不另立一套」。
    pub(crate) fn from_content(
        text: Color32,
        dim: Color32,
        faint: Color32,
        accent: Color32,
        code_bg: Color32,
        bg: Color32,
    ) -> Self {
        Self {
            text,
            dim,
            faint,
            accent,
            code_bg,
            bg,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct InlineFlags {
    bold: bool,
    italic: bool,
    strike: bool,
}

#[derive(Clone, Copy)]
struct LineStyle {
    palette: MdPalette,
    size: f32,
    line_h: f32,
    /// 整行加粗（标题）。
    bold: bool,
    color: Color32,
}

impl LineStyle {
    fn body(palette: MdPalette) -> Self {
        Self {
            palette,
            size: BODY_PX,
            line_h: BODY_LINE,
            bold: false,
            color: palette.text,
        }
    }

    fn heading(palette: MdPalette, level: u8) -> Self {
        // 字号/行高照 `docs/28` §6.2 那张表：H1 30/42、H2 22/33、H3 18/28、H4 16/26。
        let style = match level {
            1 => content::PAGE_TITLE,
            2 => content::HEADING_1,
            3 => content::HEADING_2,
            4 => content::HEADING_3,
            _ => content::READING_BODY,
        };
        let (size, line_h) = (style.size, style.line_height);
        Self {
            palette,
            size,
            line_h,
            bold: true,
            color: palette.text,
        }
    }

    fn quote(palette: MdPalette) -> Self {
        Self {
            palette,
            size: BODY_PX,
            line_h: BODY_LINE,
            bold: false,
            color: palette.dim,
        }
    }

    fn plain(&self, f: InlineFlags) -> TextFormat {
        let bold = f.bold || self.bold;
        TextFormat {
            font_id: FontId::new(
                self.size,
                if bold {
                    FontFamily::Name(fonts::BOLD.into())
                } else {
                    FontFamily::Proportional
                },
            ),
            line_height: Some(self.line_h),
            color: self.color,
            italics: f.italic,
            strikethrough: if f.strike {
                Stroke::new(1.0, self.palette.dim)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        }
    }

    fn marker(&self) -> TextFormat {
        TextFormat {
            font_id: FontId::new(self.size, FontFamily::Proportional),
            line_height: Some(self.line_h),
            color: self.palette.faint,
            ..Default::default()
        }
    }

    /// 非光标行的标记：字仍占一个字符位置，但缩到 0.1px 并透明，
    /// 版面像成稿，光标映射又不用改一个字节。
    fn hidden(&self) -> TextFormat {
        TextFormat {
            font_id: FontId::new(0.1, FontFamily::Proportional),
            line_height: Some(self.line_h),
            color: Color32::TRANSPARENT,
            ..Default::default()
        }
    }

    /// 列表 / 任务 / 引用标记：**一律**用「小号透明字 + 字距」把正文推进到
    /// 合同要求的缩进（列表 24、引用 16）。
    ///
    /// 以前这里按 `hide` 分两种：光标不在行内时用 4px 透明字 + 字距凑满 24，
    /// 光标进到行内就换成正常字号的可见标记——而可见标记的自然宽度远小于 24
    /// （`- ` 约 9px、`1. ` 约 17px），于是**点进列表行，正文就横着跳一下**。
    /// 绘制层本来就会先拿页面底色把这 24px 盖掉、再补圆点/方框/编号，
    /// 所以「在光标行露标记」在这条路上从来就没生效过——留着只是让文字乱跳。
    /// 现在只有一种形态：缩进恒定，看得见的那颗标记由绘制层负责。
    fn marker_indent(&self, chars: usize, target: f32) -> TextFormat {
        // 4px 字的单字宽实测约 1.1px，而字距只在字符之间生效（n 个字只有 n-1 段），
        // 所以解 n*g + (n-1)*s = target。
        let size = 4.0f32;
        let g = size * 0.275;
        let n = chars as f32;
        let spacing = if chars > 1 {
            ((target - n * g) / (n - 1.0)).max(0.0)
        } else {
            0.0
        };
        TextFormat {
            font_id: FontId::new(size, FontFamily::Proportional),
            line_height: Some(self.line_h),
            color: Color32::TRANSPARENT,
            extra_letter_spacing: spacing,
            ..Default::default()
        }
    }

    fn code(&self) -> TextFormat {
        TextFormat {
            // 合同：行内码字号 0.92em，底色 60%、内距 3。
            font_id: FontId::monospace(self.size * 0.92),
            // 行内代码不能带显式行高：epaint 会按行高画底色，却按字体排字，
            // 底色就滑到基线下面去了。让底色贴着字形走。
            line_height: None,
            color: self.palette.text,
            background: self.palette.code_bg,
            expand_bg: 3.0,
            ..Default::default()
        }
    }

    /// 围栏代码块的正文：等宽、**不带**逐行底色——底色由整块容器统一铺，
    /// 逐行再铺一层会在行与行之间露出条带。
    fn code_line(&self) -> TextFormat {
        TextFormat {
            font_id: FontId::monospace(CODE_PX),
            line_height: Some(CODE_LINE),
            color: self.palette.text,
            ..Default::default()
        }
    }

    fn link(&self, f: InlineFlags) -> TextFormat {
        let mut fmt = self.plain(f);
        fmt.color = self.palette.accent;
        fmt.underline = Stroke::new(1.0, self.palette.accent);
        fmt
    }
}

/// 标记显示策略：`ShowAll` 用于测试/无光标场景；`OnlyLine(Some(n))` 只显示第 n 行，
/// `OnlyLine(None)`（正文失焦）全部收起。
#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkerMode {
    #[cfg(test)]
    ShowAll,
    /// `Some((行号, 行内字节列))`：只显示该行的块标记，且行内只显示光标所在片段的标记。
    OnlyLine(Option<(usize, usize)>),
}

impl MarkerMode {
    fn hidden_for(self, line: usize) -> bool {
        match self {
            #[cfg(test)]
            MarkerMode::ShowAll => false,
            MarkerMode::OnlyLine(active) => active.map(|(l, _)| l) != Some(line),
        }
    }

    /// 光标落在这一行时返回它在本行内的字节列；`ShowAll` 返回 `None`（不筛片段）。
    fn caret_in(self, line: usize) -> Option<usize> {
        match self {
            #[cfg(test)]
            MarkerMode::ShowAll => None,
            MarkerMode::OnlyLine(Some((l, col))) if l == line => Some(col),
            MarkerMode::OnlyLine(_) => None,
        }
    }

    /// 光标所在的源行号（未夹取）。
    fn active_line(self) -> Option<usize> {
        match self {
            #[cfg(test)]
            MarkerMode::ShowAll => None,
            MarkerMode::OnlyLine(active) => active.map(|(l, _)| l),
        }
    }
}

/// 把整段 Markdown 编成一个 `LayoutJob`。**输出文本必须与输入逐字节一致**：
/// `TextEdit` 的光标映射依赖这个不变量，样式只能挂在同一串字符上。
#[cfg(test)]
pub(crate) fn markdown_job(text: &str, palette: MdPalette, wrap_width: f32) -> LayoutJob {
    build_job(text, palette, wrap_width, MarkerMode::ShowAll, None)
}

/// 只读阅读面的 `LayoutJob`：标记全部藏起来（`OnlyLine(None)` = 没有光标所在行）。
///
/// 笔记库预览（`docs/28` §7.5）复用卡片页这一棵 AST 与同一套排版，但预览不是
/// 编辑面——`**粗体**` 在预览里必须直接显示成粗体。刻意不复用 `markdown_job`：
/// 那份是 `ShowAll`（编辑器没有活动行时的口径），会把标记露出来。
pub(crate) fn reading_job(text: &str, palette: MdPalette, wrap_width: f32) -> LayoutJob {
    build_job(text, palette, wrap_width, MarkerMode::OnlyLine(None), None)
}

fn markdown_job_focus(
    text: &str,
    palette: MdPalette,
    wrap_width: f32,
    active: Option<(usize, usize)>,
    dim: Option<(usize, usize, f32)>,
) -> LayoutJob {
    build_job(text, palette, wrap_width, MarkerMode::OnlyLine(active), dim)
}

/// `dim` 是专注模式的淡出区间 `(首行, 末行, 系数)`：区间外的行把文字 alpha 乘上系数。
/// 不做任何遮罩矩形——合同要的是「文字自己淡下去」，不是盖一层。
fn build_job(
    text: &str,
    palette: MdPalette,
    wrap_width: f32,
    mode: MarkerMode,
    dim: Option<(usize, usize, f32)>,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    // 行尾空格要留着：egui 自己的默认 layouter 也设这一条（`builder.rs` 里写明
    // 「text editors 要保留行尾空白」）。不设的话排版层会把「刚敲的那个空格」
    // 当成对齐用的空白裁掉，光标在行尾一动不动，看起来像卡死。
    job.keep_trailing_whitespace = true;
    let mut in_code = false;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    // 光标所在源行夹到真实行数内。
    //
    // `line_index_at` 数的是 `\n` 的个数，而这里遍历的是 `split_inclusive('\n')`：
    // 正文以 `\n` 结尾、光标又停在最末尾时，前者会数出「第 n 行」，而后者只有 n 条
    // ——那个多出来的「幻影行」正是末尾换行符撑出来的行盒，它的高度由**最后一条
    // 源行**决定。不夹取的话，那一帧没有任何一行匹配得上，整页标记全收起，
    // 连带下面的空行行高修正也落不到实处。
    let caret_line = mode
        .active_line()
        .map(|l| l.min(lines.len().saturating_sub(1)));
    // 空行是块间距的唯一落点：Markdown 的块间空行是真实字符，删不得（光标映射），
    // 只能把它们的行高改成目标间距。`fence` 预扫一遍，代码块里的空行不算块间距。
    let mut fence_before = Vec::with_capacity(lines.len());
    {
        let mut open = false;
        for raw in &lines {
            fence_before.push(open);
            if raw.trim_start().starts_with("```") {
                open = !open;
            }
        }
    }
    // 连续空行只让第一个承担间距，后面的压到 0，否则两段之间会叠出双倍空白。
    let mut gap_taken = false;
    for (line, raw) in lines.iter().enumerate() {
        let (content, trailing) = match raw.strip_suffix('\n') {
            Some(c) => (c, "\n"),
            None => (*raw, ""),
        };
        let blank = content.trim().is_empty() && !fence_before[line];
        let gap = if blank && !gap_taken {
            gap_taken = true;
            Some(spacing_before(&lines, line, fence_before[line]))
        } else {
            if !blank {
                gap_taken = false;
            }
            blank.then_some(0.0)
        };
        // 光标所在的空行必须是**一整行字高**。
        //
        // 空行在这里是用行高冒充块间距的（省字符、不改字节）。对「不在光标上的
        // 空行」这没问题：连续空行压到 0 正好折叠成一枚间距。但光标偏偏会停在
        // 空行上——写完一段按两下回车就是这样——那一行的行高若是 12 甚至 0，
        // 光标就成了一条零高度的线，屏幕上看不见（CE-01 实测：
        // `"abc\n\n"` 的光标矩形 `(0,28)-(0,28)`）。所以光标行至少给足正文行高。
        let gap = match gap {
            Some(g) if caret_line == Some(line) => Some(g.max(BODY_LINE)),
            other => other,
        };
        // 相邻两个列表项之间留 4px（合同 §6.2「列表项间 4」）：没有空行可用，
        // 只能把这 4px 记在上一条自己的行高里。
        let item_tail = {
            let this_item = text_line_kind(content);
            let next_item = lines
                .iter()
                .skip(line + 1)
                .map(|l| l.trim_start().trim_end_matches(['\r', '\n']))
                .find(|l| !l.trim().is_empty())
                .map(text_line_kind)
                .unwrap_or(false);
            if this_item && next_item {
                4.0
            } else {
                0.0
            }
        };
        let sec0 = job.sections.len();
        push_line(
            &mut job,
            content,
            trailing,
            &mut in_code,
            palette,
            mode.hidden_for(line),
            mode.caret_in(line),
            gap,
            item_tail,
        );
        if let Some((first, last, alpha)) = dim {
            if line < first || line > last {
                for sec in &mut job.sections[sec0..] {
                    sec.format.color = dim_color(sec.format.color, alpha);
                }
            }
        }
    }
    job
}

/// 专注淡出的最终系数：`t=0` 全亮，`t=1` 淡到 35%。
fn focus_alpha(t: f32) -> f32 {
    1.0 - (1.0 - FOCUS_DIM) * t.clamp(0.0, 1.0)
}

/// 按系数压一个颜色的不透明度（RGB 不动，只动 alpha）。
fn dim_color(c: Color32, alpha: f32) -> Color32 {
    let a = (c.a() as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// 某个物理行落在哪个 Markdown 块的**行区间**（含首含尾）。
///
/// 块按空行切分：连续非空行算一块（段落、列表、引用都算），空行不属于任何块。
fn block_lines(lines: &[&str], line: usize) -> (usize, usize) {
    if lines.is_empty() {
        return (0, 0);
    }
    let blank = |i: usize| lines.get(i).is_none_or(|l| l.trim().is_empty());
    let line = line.min(lines.len() - 1);
    // 光标落在空行上：空行不属于任何一段，就让它自己当一块，别把上下两段连起来。
    if blank(line) {
        return (line, line);
    }
    let mut first = line;
    while first > 0 && !blank(first - 1) {
        first -= 1;
    }
    let mut last = line;
    while last + 1 < lines.len() && !blank(last + 1) {
        last += 1;
    }
    (first, last)
}

/// 打字机模式的目标滚动偏移（绝对值）：把光标放到视口 42% 处时，偏移应当是多少。
///
/// `caret_y` 与 `offset` 必须来自同一帧：光标屏幕 y + 当前偏移 − 视口顶 = 光标的内容坐标，
/// 与滚动位置无关；再减 42% 视口高就是它该在的偏移。给绝对值的另一个好处是
/// egui 的换算已经是 `offset = anchor − 内容顶`，把 `target + 内容顶` 当锚点传进去，
/// 得到的就是这个 target，不多不少（相对增量则会被 `TextEdit` 的请求叠加）。
fn typewriter_target_offset(caret_y: f32, offset: f32, view_top: f32, view_h: f32) -> f32 {
    caret_y + offset - view_top - view_h * TYPEWRITER_AT
}

/// 过程证据文件（`MW_TW_PROBE`）：只在验收时设，正常使用零文件 I/O。
fn tw_probe_path() -> Option<&'static std::path::PathBuf> {
    static P: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    P.get_or_init(|| std::env::var_os("MW_TW_PROBE").map(std::path::PathBuf::from))
        .as_ref()
}

/// 是否是列表项行（无序或有序）。
fn text_line_kind(t: &str) -> bool {
    let t = t.trim_start();
    is_bullet_line(t) || is_ordered_line(t)
}

/// 分隔线行：整行只有 - * _ 和空格，且至少三个标记符。
fn is_rule_line(t: &str) -> bool {
    t.chars().filter(|c| matches!(c, '-' | '*' | '_')).count() >= 3
        && t.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '))
}

/// 无序列表行：`- ` / `* ` / `+ `。
fn is_bullet_line(t: &str) -> bool {
    let mut c = t.chars();
    matches!(c.next(), Some('-') | Some('*') | Some('+')) && c.next() == Some(' ')
}

/// 有序列表行：1–3 位数字 + `.`/`)` + 空格。
fn is_ordered_line(t: &str) -> bool {
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return false;
    }
    let tail: String = t.chars().skip(digits).take(2).collect();
    tail == ". " || tail == ") "
}

/// 下一个非空块需要多少上边距（`docs/28` §6.2 那张表）。
fn spacing_before(lines: &[&str], blank_at: usize, in_code: bool) -> f32 {
    if in_code {
        return 0.0;
    }
    // 上一个块的「下边距」：分隔线 32、列表 8。与下一个块的「上边距」取大值，
    // 就是边距折叠——否则「列表下 8」会把紧随其后的「标题上 40」压掉。
    let prev_gap: f32 = lines[..blank_at]
        .iter()
        .rev()
        .map(|l| l.trim_start().trim_end_matches(['\r', '\n']))
        .find(|l| !l.trim().is_empty())
        .map(|prev| {
            if is_rule_line(prev) {
                32.0
            } else if is_bullet_line(prev) || is_ordered_line(prev) {
                8.0
            } else {
                // 标题的下边距按级别走：H2 下 12、H3/H4 下 8（合同 §6.2）。
                let hashes = prev.bytes().take_while(|b| *b == b'#').count();
                match hashes {
                    2 => 12.0,
                    3 | 4 => 8.0,
                    _ => 0.0,
                }
            }
        })
        .unwrap_or(0.0);
    let next_gap: f32 = lines
        .iter()
        .skip(blank_at + 1)
        .map(|l| l.trim_start().trim_end_matches(['\r', '\n']))
        .find(|l| !l.trim().is_empty())
        .map(|t| {
            if t.starts_with("```") {
                12.0
            } else if is_rule_line(t) {
                32.0
            } else {
                let hashes = t.bytes().take_while(|b| *b == b'#').count();
                if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
                    match hashes {
                        2 => 40.0,
                        3 => 28.0,
                        4 => 24.0,
                        _ => 40.0,
                    }
                } else if t.starts_with('>') {
                    12.0
                } else if is_bullet_line(t) || is_ordered_line(t) {
                    8.0
                } else {
                    12.0
                }
            }
        })
        .unwrap_or(0.0);
    prev_gap.max(next_gap)
}

// 一行 Markdown 的七个输入（正文、行尾、围栏状态、配色、显隐、空行间距、项间间距）
// 都是逐行变化的一等参数，收成结构体只会把调用点搬远，不影响可读性。
#[allow(clippy::too_many_arguments)]
fn push_line(
    job: &mut LayoutJob,
    content: &str,
    trailing: &str,
    in_code: &mut bool,
    palette: MdPalette,
    hide: bool,
    // 光标在本行内的字节列（`None` = 不筛选，所有行内标记照常显示）。
    caret: Option<usize>,
    gap: Option<f32>,
    item_tail: f32,
) {
    let mut style = LineStyle::body(palette);
    style.line_h += item_tail;
    let trimmed = content.trim_start();
    let lead_len = content.len() - trimmed.len();
    // 空行就是「块间距」本身：行高改成目标值，一个字符都不增删。
    if let (Some(gap), true) = (gap, trimmed.trim().is_empty()) {
        let mut fmt = style.plain(InlineFlags::default());
        fmt.line_height = Some(gap);
        job.append(trailing, 0.0, fmt);
        return;
    }

    // 粘贴进来的文本常带 CRLF：行尾 \r 要当行分隔符看待，否则 `---`、标题、列表这些
    // 「整行形态」判定会全部落空（实测：CRLF 正文里分隔线退化成三个短横、间距也错）。
    let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);

    if trimmed.starts_with("```") {
        *in_code = !*in_code;
        let fmt = if hide { style.hidden() } else { style.marker() };
        job.append(content, 0.0, fmt.clone());
        if !trailing.is_empty() {
            job.append(trailing, 0.0, fmt);
        }
        return;
    }
    if *in_code {
        let code = style.code_line();
        job.append(content, 0.0, code.clone());
        if !trailing.is_empty() {
            job.append(trailing, 0.0, code);
        }
        return;
    }

    // 标题：1–6 个 #，后面必须是空格或行尾。
    let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
    let after_hash = &trimmed[hashes..];
    if (1..=6).contains(&hashes) && (after_hash.is_empty() || after_hash.starts_with(' ')) {
        let hs = LineStyle::heading(palette, hashes as u8);
        let spaces = after_hash.len() - after_hash.trim_start_matches(' ').len();
        let mark = if hide { hs.hidden() } else { hs.marker() };
        job.append(&content[..lead_len], 0.0, mark.clone());
        job.append(&trimmed[..hashes + spaces], 0.0, mark);
        push_inline(
            job,
            &after_hash[spaces..],
            &hs,
            hide,
            caret.map(|c| c.saturating_sub(lead_len + hashes + spaces)),
        );
        if !trailing.is_empty() {
            job.append(trailing, 0.0, hs.hidden());
        }
        return;
    }

    // 引用：> 或 > 空格。
    if let Some(rest) = trimmed.strip_prefix('>') {
        let space = usize::from(rest.starts_with(' '));
        let qs = LineStyle::quote(palette);
        job.append(&content[..lead_len], 0.0, style.marker());
        job.append(
            &trimmed[..1 + space],
            0.0,
            qs.marker_indent(1 + space, 16.0),
        );
        push_inline(
            job,
            &rest[space..],
            &qs,
            hide,
            caret.map(|c| c.saturating_sub(lead_len + 1 + space)),
        );
        if !trailing.is_empty() {
            job.append(trailing, 0.0, qs.hidden());
        }
        return;
    }

    // 任务项 / 无序列表。标记先照常排（给正文留出缩进），绘制层再盖上一个
    // 圆点或复选框；这样换行缩进和光标位置都不变。
    let bullet = ["- ", "* ", "+ "]
        .iter()
        .find(|p| trimmed.starts_with(*p))
        .map(|p| p.len());
    if let Some(plen) = bullet {
        let mut marker_len = plen;
        let rest = &trimmed[plen..];
        if rest.starts_with("[ ] ") || rest.starts_with("[x] ") || rest.starts_with("[X] ") {
            marker_len += 4;
        }
        // 任务完成态：文字走 `text.faint`（合同：不划删除线）。
        let done = task_state(trimmed) == Some(true);
        let mut rest_style = style;
        if done {
            rest_style.color = palette.faint;
        }
        job.append(&content[..lead_len], 0.0, style.marker());
        job.append(
            &trimmed[..marker_len],
            0.0,
            style.marker_indent(marker_len, 24.0),
        );
        push_inline(
            job,
            &trimmed[marker_len..],
            &rest_style,
            hide,
            caret.map(|c| c.saturating_sub(lead_len + marker_len)),
        );
        if !trailing.is_empty() {
            job.append(trailing, 0.0, style.hidden());
        }
        return;
    }

    // 有序列表：数字 + . / ) + 空格。编号始终显示，跟 Typora 一样——
    // 但**不在这条文字流水线上显示**：这里只留占位（透明、字距凑满 24），
    // 真正的「1.」由绘制层按 20px 列右对齐画。原因见 `marker_indent`：
    // 用它自带的可见标记会让缩进随光标跳。
    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        let tail = &trimmed[digits..];
        if (tail.starts_with(". ") || tail.starts_with(") ")) && digits + 2 <= trimmed.len() {
            job.append(&content[..lead_len], 0.0, style.hidden());
            job.append(
                &trimmed[..digits + 2],
                0.0,
                style.marker_indent(digits + 2, 24.0),
            );
            push_inline(
                job,
                &trimmed[digits + 2..],
                &style,
                hide,
                caret.map(|c| c.saturating_sub(lead_len + digits + 2)),
            );
            if !trailing.is_empty() {
                job.append(trailing, 0.0, style.hidden());
            }
            return;
        }
    }

    // 分隔线：整行只有 --- / *** / ___（至少三个）。绘制层会在整行位置画一条线。
    let rule_marks = trimmed
        .chars()
        .filter(|c| matches!(c, '-' | '*' | '_'))
        .count();
    if rule_marks >= 3 && trimmed.chars().all(|c| matches!(c, '-' | '*' | '_' | ' ')) {
        job.append(content, 0.0, style.marker());
        if !trailing.is_empty() {
            job.append(trailing, 0.0, style.hidden());
        }
        return;
    }

    push_inline(job, content, &style, hide, caret);
    if !trailing.is_empty() {
        job.append(trailing, 0.0, style.hidden());
    }
}

fn push_inline(job: &mut LayoutJob, s: &str, st: &LineStyle, hide: bool, caret: Option<usize>) {
    if s.is_empty() {
        return;
    }
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let mut start = 0usize;
    let mut flags = InlineFlags::default();
    // 片段级揭示（docs/28 §6.3）：光标落在 `**粗体**` / `` `码` `` / `[文字](链接)` 这类
    // 片段内部时，只露出**这一个**片段的标记，其余片段的标记照旧收起——比「活动行整行
    // 露出」更贴合同事，也不用把页面拆成多个控件（光标映射一个字节都没变）。
    let spans = inline_spans(s, caret);
    let mark = |st: &LineStyle, at: usize| {
        let show = !hide && spans.iter().any(|(a, b)| *a <= at && at < *b);
        if show {
            st.marker()
        } else {
            st.hidden()
        }
    };

    let flush = |job: &mut LayoutJob, text: &str, flags: InlineFlags| {
        if !text.is_empty() {
            job.append(text, 0.0, st.plain(flags));
        }
    };

    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() && bytes[i + 1].is_ascii_punctuation() => {
                i += 2; // 转义符与被转义字符都留在原文里，只是不参与样式切换。
            }
            b'`' => {
                if let Some(close) = s[i + 1..].find('`').map(|p| i + 1 + p) {
                    flush(job, &s[start..i], flags);
                    job.append("`", 0.0, mark(st, i));
                    job.append(&s[i + 1..close], 0.0, st.code());
                    job.append("`", 0.0, mark(st, close));
                    i = close + 1;
                    start = i;
                } else {
                    i += 1;
                }
            }
            b'*' | b'_' => {
                let double = i + 1 < bytes.len() && bytes[i + 1] == bytes[i];
                let len = if double { 2 } else { 1 };
                flush(job, &s[start..i], flags);
                job.append(&s[i..i + len], 0.0, mark(st, i));
                if double {
                    flags.bold = !flags.bold;
                } else {
                    flags.italic = !flags.italic;
                }
                i += len;
                start = i;
            }
            b'~' if i + 1 < bytes.len() && bytes[i + 1] == b'~' => {
                flush(job, &s[start..i], flags);
                job.append("~~", 0.0, mark(st, i));
                flags.strike = !flags.strike;
                i += 2;
                start = i;
            }
            b'!' if i + 1 < bytes.len() && bytes[i + 1] == b'[' => {
                if let Some((consumed, _url)) = parse_link(&s[i + 1..]) {
                    let close = s[i + 1..].find("](").unwrap();
                    flush(job, &s[start..i], flags);
                    let m = mark(st, i);
                    job.append("![", 0.0, m.clone());
                    job.append(&s[i + 2..i + 1 + close], 0.0, st.link(flags));
                    job.append("](", 0.0, m.clone());
                    job.append(&s[i + 1 + close + 2..i + 1 + consumed - 1], 0.0, m.clone());
                    job.append(")", 0.0, m);
                    i += 1 + consumed;
                    start = i;
                } else {
                    i += 1;
                }
            }
            b'[' => {
                let wiki = if s[i..].starts_with("[[") {
                    s[i..].find("]]")
                } else {
                    None
                };
                if let Some(end) = wiki {
                    flush(job, &s[start..i], flags);
                    let m = mark(st, i);
                    job.append("[[", 0.0, m.clone());
                    job.append(&s[i + 2..i + end], 0.0, st.link(flags));
                    job.append("]]", 0.0, m);
                    i += end + 2;
                    start = i;
                } else if let Some((consumed, _url)) = parse_link(&s[i..]) {
                    let close = s[i..].find("](").unwrap();
                    flush(job, &s[start..i], flags);
                    let m = mark(st, i);
                    job.append("[", 0.0, m.clone());
                    job.append(&s[i + 1..i + close], 0.0, st.link(flags));
                    job.append("](", 0.0, m.clone());
                    job.append(&s[i + close + 2..i + consumed - 1], 0.0, m.clone());
                    job.append(")", 0.0, m);
                    i += consumed;
                    start = i;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    flush(job, &s[start..], flags);
}

/// 行内片段的字节跨度（`**粗体**`、`` `码` ``、`~~删除~~`、`[文字](链接)`、`[[双链]]`、`![图](url)`）。
/// 只用于「光标落在哪一段」的判定：返回的跨度里只要包含光标列，就认为光标在这个片段内。
/// 解析规则故意比样式那一遍宽松：判错只影响标记显不显示，不会改任何一个字节。
fn inline_spans(s: &str, caret: Option<usize>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    let Some(caret) = caret else {
        return out;
    };
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() && bytes[i + 1].is_ascii_punctuation() => i += 2,
            b'`' => {
                if let Some(close) = s[i + 1..].find('`').map(|p| i + 1 + p) {
                    out.push((i, close + 1));
                    i = close + 1;
                } else {
                    i += 1;
                }
            }
            b'*' | b'_' => {
                let dbl = i + 1 < bytes.len() && bytes[i + 1] == bytes[i];
                let len = if dbl { 2 } else { 1 };
                let mark = bytes[i];
                let mut j = i + len;
                let mut found = None;
                while j < bytes.len() {
                    if bytes[j] == mark {
                        let here_dbl = j + 1 < bytes.len() && bytes[j + 1] == mark;
                        if here_dbl == dbl {
                            found = Some(j);
                            break;
                        }
                        j += 2;
                        continue;
                    }
                    j += 1;
                }
                match found {
                    Some(e) => {
                        out.push((i, e + len));
                        i = e + len;
                    }
                    None => i += len,
                }
            }
            b'~' if i + 1 < bytes.len() && bytes[i + 1] == b'~' => {
                match s[i + 2..].find("~~").map(|p| i + 2 + p) {
                    Some(e) => {
                        out.push((i, e + 2));
                        i = e + 2;
                    }
                    None => i += 2,
                }
            }
            // 图片 `![文字](url)` 的片段从 `!` 开始：若从 `[` 起算，`![` 里的 `!` 落在跨度外，
            // 光标停在图片说明里时那个 `!` 永远不会露出来。
            b'!' if i + 1 < bytes.len() && bytes[i + 1] == b'[' => {
                if let Some((consumed, _)) = parse_link(&s[i + 1..]) {
                    out.push((i, i + consumed + 1));
                    i += consumed + 1;
                } else {
                    i += 1;
                }
            }
            b'[' => {
                if s[i..].starts_with("[[") {
                    match s[i..].find("]]").map(|p| i + p) {
                        Some(e) => {
                            out.push((i, e + 2));
                            i = e + 2;
                        }
                        None => i += 1,
                    }
                } else if let Some((consumed, _)) = parse_link(&s[i..]) {
                    out.push((i, i + consumed));
                    i += consumed;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    out.retain(|(a, b)| *a <= caret && caret < *b);
    out
}
/// 解析 `[文字](url)`，返回（整个结构字节数，url）。
fn parse_link(s: &str) -> Option<(usize, String)> {
    let open = s.find("](")?;
    let rest = &s[open + 2..];
    let close = rest.find(')')?;
    Some((open + 2 + close + 1, rest[..close].trim().to_string()))
}

/// 光标在本行内的**字节列**：行内片段跨度是字节，光标是字符索引，判断「光标落在
/// 哪个片段」时必须换算，否则中文正文里会差出一个片段的长度。
fn col_in_line(text: &str, char_idx: usize) -> usize {
    let mut bytes = 0usize;
    for (n, ch) in text.chars().enumerate() {
        if n == char_idx {
            break;
        }
        if ch == '\n' {
            bytes = 0;
        } else {
            bytes += ch.len_utf8();
        }
    }
    bytes
}

fn line_index_at(text: &str, char_idx: usize) -> usize {
    text.chars().take(char_idx).filter(|&c| c == '\n').count()
}

/// 无序列表标记的字符数（含任务框）；有序列表返回 None（编号保留）。
fn list_marker_len(trimmed: &str) -> Option<usize> {
    for p in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(p) {
            let extra = usize::from(
                rest.starts_with("[ ] ") || rest.starts_with("[x] ") || rest.starts_with("[X] "),
            ) * 4;
            return Some(p.len() + extra);
        }
    }
    None
}

/// 有序列表的编号文本（含点号，不含后面的空格）与它占的字符数。
///
/// 与 `push_line` 里那条分支同口径：1–3 位数字 + `.`/`)` + 空格。
fn ordered_marker(trimmed: &str) -> Option<(&str, usize)> {
    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let tail = &trimmed[digits..];
    if (tail.starts_with(". ") || tail.starts_with(") ")) && digits + 2 <= trimmed.len() {
        Some((&trimmed[..digits + 1], digits + 2))
    } else {
        None
    }
}

/// 代码块的内容行范围与语言名：`(首行, 末行, 语言)`，行号按 `split_inclusive('\n')` 计。
/// 围栏行本身不在范围内——容器只包内容，围栏的空白留给上下留白。
fn code_blocks(body: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut open: Option<(usize, String)> = None;
    for (i, raw) in body.split_inclusive('\n').enumerate() {
        let t = raw.trim_start().trim_end_matches(['\r', '\n']);
        if let Some(rest) = t.strip_prefix("```") {
            match open.take() {
                None => open = Some((i + 1, rest.trim().to_string())),
                Some((first, lang)) => {
                    if i > first {
                        out.push((first, i - 1, lang));
                    }
                }
            }
        }
    }
    out
}

/// 任务框状态：Some(true)=已勾选，Some(false)=未勾选。
fn task_state(trimmed: &str) -> Option<bool> {
    for p in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(p) {
            if rest.starts_with("[ ] ") {
                return Some(false);
            }
            if rest.starts_with("[x] ") || rest.starts_with("[X] ") {
                return Some(true);
            }
        }
    }
    None
}

/// 点一下任务框：把 `[ ]` ⇄ `[x]` 翻过去，返回 `(新正文, 行尾字符下标)`。
///
/// `at` 是方框里那一格（`[ ]` 的空格或 `[x]` 的字母）在**字符**上的下标，
/// 由 `paint_block_decorations` 的命中矩形算出。这里再核一遍整行确实是任务项，
/// 免得误点（坐标算错、行被改过）把正文里别的字符换掉。光标交给行尾，
/// 这样接着打字就是继续这一条，不会挤进方框里。
fn task_flip(text: &str, at: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let cur = *chars.get(at)?;
    let line_start = chars[..at]
        .iter()
        .rposition(|c| *c == '\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line_end = chars[at..]
        .iter()
        .position(|c| *c == '\n')
        .map(|i| at + i)
        .unwrap_or(chars.len());
    let line: String = chars[line_start..line_end].iter().collect();
    let trimmed = line.trim_start().trim_end_matches('\r');
    let lead_chars = line.chars().count() - trimmed.chars().count();
    if task_state(trimmed).is_none() || at != line_start + lead_chars + 3 {
        return None;
    }
    let flipped = match cur {
        ' ' => 'x',
        'x' | 'X' => ' ',
        _ => return None,
    };
    let mut out = chars;
    out[at] = flipped;
    Some((out.into_iter().collect(), line_end))
}
/// 只画在 TextEdit 的绘制层上，正文缓冲一个字符都不动。
fn paint_block_decorations(
    ui: &egui::Ui,
    out: &egui::text_edit::TextEditOutput,
    body: &str,
    active_line: Option<usize>,
    palette: MdPalette,
    dim: Option<(usize, usize, f32)>,
) -> Option<(usize, bool)> {
    let origin = out.galley_pos;
    let line_left = out.response.rect.left();
    let line_right = out.response.rect.right();
    let clicked_at = if ui.input(|i| i.pointer.primary_clicked()) {
        ui.input(|i| i.pointer.interact_pos())
    } else {
        None
    };
    let mut hit: Option<(usize, bool)> = None;
    let mut chars = 0usize;
    for (line, raw) in body.split_inclusive('\n').enumerate() {
        let content = raw.strip_suffix('\n').unwrap_or(raw);
        let hidden = active_line != Some(line);
        // 专注模式：块外的装饰跟着文字一起淡，别出现「字淡了、竖条还全亮」。
        let mut line_painter = ui.painter().clone();
        if let Some((first, last, alpha)) = dim {
            if line < first || line > last {
                line_painter.set_opacity(alpha);
            }
        }
        let painter = &line_painter;
        // 行尾 \r 不进判定：CRLF 正文里任务框和分隔线否则会整片消失。
        let trimmed = content.trim_start().trim_end_matches('\r');
        let lead = content.len() - trimmed.len();
        let lead_chars = content[..lead].chars().count();
        let caret = |n: usize| {
            out.galley
                .pos_from_cursor(egui::text::CCursor::new(chars + lead_chars + n))
                .translate(origin.to_vec2())
        };
        // 列表符号与任务框不管是不是光标行都补画：勾选框要一直看得见、点得着。
        if let Some(marker_len) = list_marker_len(trimmed) {
            let start = caret(0);
            let end = caret(marker_len);
            let row = Rect::from_min_max(
                pos2(start.left(), start.top()),
                pos2(end.left(), start.bottom()),
            );
            painter.rect_filled(row, 0.0, palette.bg);
            let cy = start.center().y;
            if let Some(checked) = task_state(trimmed) {
                let box_rect =
                    Rect::from_center_size(pos2(start.left() + 8.0, cy), vec2(16.0, 16.0));
                if checked {
                    painter.rect_filled(box_rect, 3.0, palette.accent);
                    let check = Stroke::new(1.7, Color32::WHITE);
                    painter.line_segment(
                        [
                            pos2(box_rect.left() + 3.0, box_rect.center().y),
                            pos2(box_rect.center().x, box_rect.bottom() - 3.5),
                        ],
                        check,
                    );
                    painter.line_segment(
                        [
                            pos2(box_rect.center().x, box_rect.bottom() - 3.5),
                            pos2(box_rect.right() - 3.0, box_rect.top() + 3.5),
                        ],
                        check,
                    );
                } else {
                    painter.rect_filled(box_rect, 3.0, palette.bg);
                    painter.rect_stroke(
                        box_rect,
                        3.0,
                        Stroke::new(1.2, palette.faint),
                        StrokeKind::Inside,
                    );
                }
                if let Some(pos) = clicked_at {
                    if box_rect.expand(4.0).contains(pos) {
                        crate::trace::event(&format!(
                            "task_box_click line={line} checked={checked} at={} y={:.1}",
                            chars + lead_chars + marker_len - 3,
                            box_rect.center().y
                        ));
                        hit = Some((chars + lead_chars + marker_len - 3, checked));
                    }
                }
            } else {
                painter.circle_filled(pos2(start.left() + 5.5, cy), 2.5, palette.dim);
            }
        } else if let Some((num, _)) = ordered_marker(trimmed) {
            // 编号右对齐到 20px 列（`docs/28` §6.2）。
            //
            // 文字层里那三个字符是**透明占位**（只为了让正文从 24 开始），
            // 真正的「1.」画在这里。改前它走 `marker_indent` 的隐藏分支、
            // 却被改回 `dim` 色——4px 的深色字，屏幕上只剩一个像逗号的墨点。
            let start = caret(0);
            painter.text(
                pos2(start.left() + 20.0, start.center().y),
                egui::Align2::RIGHT_CENTER,
                num,
                FontId::proportional(BODY_PX),
                palette.dim,
            );
        } else if trimmed.starts_with('>') {
            // 合同：左侧 2px 强调色 30%，正文内距 16（文字已被字距推到 16）。
            let start = caret(0);
            let bar = Rect::from_min_max(
                pos2(start.left() - 2.0, start.top()),
                pos2(start.left(), start.bottom()),
            );
            let accent30 = Color32::from_rgba_unmultiplied(
                palette.accent.r(),
                palette.accent.g(),
                palette.accent.b(),
                77,
            );
            painter.rect_filled(bar, 0.0, accent30);
        } else if hidden
            && trimmed
                .chars()
                .filter(|c| matches!(c, '-' | '*' | '_'))
                .count()
                >= 3
            && trimmed.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '))
        {
            let start = caret(0);
            let row = Rect::from_min_max(
                pos2(line_left, start.top()),
                pos2(line_right, start.bottom()),
            );
            painter.rect_filled(row, 0.0, palette.bg);
            let y = start.center().y;
            painter.line_segment(
                [pos2(line_left, y), pos2(line_right, y)],
                Stroke::new(1.0, palette.faint),
            );
        }
        chars += raw.chars().count();
    }
    // 代码块：跨行一个圆角容器（合同 圆角 10 / 内距 16 / 底色 60%）+ 右上角语言标签。
    // 行→行盒是 1:1 的前提：折行会让行号对不上，那就干脆不画，宁可少一层装饰。

    // 行 → 行盒的映射：行盒按 `ends_with_newline` 累计，一段源码行即使折行也能对上。
    // （此前用「行盒数 == 源码行数」当守卫，末尾换行会多一个空行盒，容器永远画不出来。）
    let mut line_row: Vec<usize> = vec![0];
    for (i, r) in out.galley.rows.iter().enumerate() {
        if r.ends_with_newline {
            line_row.push(i + 1);
        }
    }
    for (first, last, lang) in code_blocks(body) {
        let (Some(&top_idx), Some(&after_idx)) = (line_row.get(first), line_row.get(last + 1))
        else {
            continue;
        };
        let Some(top_row) = out.galley.rows.get(top_idx) else {
            continue;
        };
        let Some(bot_row) = out.galley.rows.get(after_idx.saturating_sub(1)) else {
            continue;
        };
        let top = top_row.rect().top() + origin.y - 16.0;
        let bottom = bot_row.rect().bottom() + origin.y + 16.0;
        let box_rect =
            Rect::from_min_max(pos2(line_left - 16.0, top), pos2(line_right - 16.0, bottom));

        let mut code_painter = ui.painter().clone();
        if let Some((afirst, alast, alpha)) = dim {
            if last < afirst || first > alast {
                code_painter.set_opacity(alpha);
            }
        }
        let painter = &code_painter;
        painter.rect_filled(box_rect, 10.0, palette.code_bg.gamma_multiply(0.6));
        if !lang.is_empty() {
            painter.text(
                pos2(box_rect.right() - 8.0, box_rect.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                &lang,
                FontId::proportional(CODE_LANG_PX),
                palette.dim,
            );
        }
    }
    hit
}

/// 列表行前缀的「续写形态」：空项回车要删它，非空项回车要复制它。
fn list_prefix(line: &str) -> Option<(usize, &'static str, Option<u64>)> {
    for (p, next) in [
        ("- [ ] ", "- [ ] "),
        ("- [x] ", "- [ ] "),
        ("- [X] ", "- [ ] "),
    ] {
        if line.starts_with(p) {
            return Some((p.chars().count(), next, None));
        }
    }
    for p in ["- ", "* ", "+ "] {
        if line.starts_with(p) {
            // 三种无序记号都归一成 "- "：续写时不要换个符号。
            return Some((2, "- ", None));
        }
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        let n: u64 = line[..digits].parse().ok()?;
        return Some((digits + 2, "", Some(n)));
    }
    None
}

/// 正文里 `Tab` / `Shift+Tab` 的缩进动作（CE-01 D4 / D11）。
///
/// - **有选区**：选区覆盖到的每一行一起缩进（`Shift+Tab` 一起退），选区保持不变。
/// - 单光标、列表行（无序 / 任务 / 有序）：整行缩进两格；
/// - 单光标、其余行：在光标处补四个空格当软缩进；`Shift+Tab` 只从行首退格。
///
/// 一律换成空格而不是制表符：本项目的 Markdown 排版器不认 `\t`——它既缩进
/// 不了也看不见，只会在缓冲区里留一个影子字符，之后 `trim_start` /
/// `lead_len` 全按它算，光标和文字就对不上了。返回 `None` = 这一下不接管
/// （退到行首没得退），把键留给上层。
///
/// 返回 `(新正文, 新 from, 新 to)`：三格是为了让**多行选区缩进之后选区还在**。
/// 以前只认一个光标位，选中三行按 `Tab` 会只缩进一行、还把选区打散。
fn indent_edit(text: &str, a: usize, b: usize, outdent: bool) -> Option<(String, usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    if lo > chars.len() {
        return None;
    }
    let hi = hi.min(chars.len());
    let line_start_of = |idx: usize| {
        chars[..idx]
            .iter()
            .rposition(|c| *c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0)
    };
    let line_end_of = |idx: usize| {
        chars[idx..]
            .iter()
            .position(|c| *c == '\n')
            .map(|i| idx + i)
            .unwrap_or(chars.len())
    };

    // ── 单光标：保持原来的两种形态 ──
    if lo == hi {
        let line_start = line_start_of(lo);
        let line_end = line_end_of(lo);
        let line: String = chars[line_start..line_end].iter().collect();
        let is_item = list_prefix(line.trim_start()).is_some();
        if outdent {
            let lead = chars[line_start..line_end]
                .iter()
                .take_while(|c| **c == ' ')
                .count();
            let step = if is_item { 2 } else { 4 };
            let take = lead.min(step);
            if take == 0 {
                return None;
            }
            let mut out = chars[..line_start].to_vec();
            out.extend_from_slice(&chars[line_start + take..]);
            let at = lo.saturating_sub(take).max(line_start);
            return Some((out.into_iter().collect(), at, at));
        }
        if is_item {
            let mut out = chars[..line_start].to_vec();
            out.push(' ');
            out.push(' ');
            out.extend_from_slice(&chars[line_start..]);
            return Some((out.into_iter().collect(), lo + 2, lo + 2));
        }
        let mut out = chars;
        for _ in 0..4 {
            out.insert(lo, ' ');
        }
        return Some((out.into_iter().collect(), lo + 4, lo + 4));
    }

    // ── 多行选区：逐行来。整段用同一个步长，免得一次 Tab 出来三种缩进。 ──
    let first_start = line_start_of(lo);
    let first_line: String = chars[first_start..line_end_of(first_start)]
        .iter()
        .collect();
    let step = if list_prefix(first_line.trim_start()).is_some() {
        2
    } else {
        4
    };
    // 被选中的行首：从 lo 所在行起，到 hi 之前的所有行。
    // `hi` 正好停在下一行行首（选区以换行结尾）时，那一行不算被选中。
    let mut starts = vec![first_start];
    let mut i = first_start;
    while i < hi {
        if chars[i] == '\n' && i + 1 < hi {
            starts.push(i + 1);
        }
        i += 1;
    }

    let mut out: Vec<char> = Vec::with_capacity(chars.len() + step * starts.len());
    let mut prev = 0usize;
    let mut new_lo = lo as isize;
    let mut new_hi = hi as isize;
    let mut changed = false;
    for &s in &starts {
        out.extend_from_slice(&chars[prev..s]);
        if outdent {
            let mut n = 0usize;
            while n < step && s + n < chars.len() && chars[s + n] == ' ' {
                n += 1;
            }
            if n > 0 {
                changed = true;
            }
            prev = s + n;
            if lo >= s {
                new_lo = (new_lo - n as isize).max(s as isize);
            }
            if hi >= s {
                new_hi = (new_hi - n as isize).max(s as isize);
            }
        } else {
            out.extend(std::iter::repeat_n(' ', step));
            changed = true;
            prev = s;
            // 严格大于：锚点正好压在行首时**不动**，让选区把新缩进一起圈进来
            // （编辑器通行手感——选两行按 Tab，两行的缩进都在选区里）。
            if lo > s {
                new_lo += step as isize;
            }
            if hi > s {
                new_hi += step as isize;
            }
        }
    }
    out.extend_from_slice(&chars[prev..]);
    if !changed {
        return None;
    }
    let n = out.len() as isize;
    Some((
        out.into_iter().collect(),
        new_lo.clamp(0, n) as usize,
        new_hi.clamp(0, n) as usize,
    ))
}

/// `Ctrl+/` 把斜杠菜单叫出来（`docs/28` §6.5 列的第二个触发方式）。
///
/// 行首空着就直接补一个 `/`；行里已经有字，就另起一行再补——
/// 菜单本身靠「行首 `/查询`」判定，所以这两个字符必须真的落进缓冲区。
fn slash_invoke_edit(text: &str, caret: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let caret = caret.min(chars.len());
    let line_start = chars[..caret]
        .iter()
        .rposition(|c| *c == '\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let blank_before = chars[line_start..caret].iter().all(|c| c.is_whitespace());
    let insert: &[char] = if blank_before { &['/'] } else { &['\n', '/'] };
    let mut out = chars[..caret].to_vec();
    out.extend_from_slice(insert);
    out.extend_from_slice(&chars[caret..]);
    (out.into_iter().collect(), caret + insert.len())
}

/// 行首退格：列表项退化成普通段落（CE-01）。
///
/// 光标正好停在标记之后（`- ` / `1. ` / `- [ ] ` 的末尾）时按 Backspace，
/// 整枚标记一起删掉，光标回到行首。判断放在这里而不是靠 `TextEdit`：
/// 它只会一格一格退，退到 `-` 还剩一个孤零零的符号，看起来像没反应。
fn backspace_marker(text: &str, caret: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    if caret == 0 || caret > chars.len() {
        return None;
    }
    let line_start = chars[..caret]
        .iter()
        .rposition(|c| *c == '\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line_end = chars[caret..]
        .iter()
        .position(|c| *c == '\n')
        .map(|i| caret + i)
        .unwrap_or(chars.len());
    let line: String = chars[line_start..line_end].iter().collect();
    let lead = line.chars().take_while(|c| *c == ' ').count();
    let trimmed: String = line.chars().skip(lead).collect();
    let (marker, _, _) = list_prefix(&trimmed)?;
    if caret != line_start + lead + marker {
        return None;
    }
    let mut out = chars[..line_start + lead].to_vec();
    out.extend_from_slice(&chars[line_start + lead + marker..]);
    let at = line_start + lead;
    Some((out.into_iter().collect(), at))
}

/// `Enter` 落在行尾时由编辑器接管的那部分行为（`docs/28` §6.8）：
/// 空列表项回车**退出列表**；有内容的列表项回车**续写同型前缀**（有序列表序号 +1）。
/// 返回 `None` 表示「不接管」，交给 `TextEdit` 自己插换行（标题、段落都走这条）。
fn enter_edit(text: &str, caret: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    if caret > chars.len() {
        return None;
    }
    // 当前行范围，行尾不含换行符。
    let mut start = caret;
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }
    let mut end = caret;
    while end < chars.len() && chars[end] != '\n' {
        end += 1;
    }
    if caret != end {
        return None;
    }
    let mut line: String = chars[start..end].iter().collect();
    if line.ends_with('\r') {
        line.pop();
    }
    // 前导空格要先摘掉再看前缀：`list_prefix` 只认「行首」，
    // 而缩进过的列表项（Tab 进来的一层）行首是空格——不摘就等于
    // 「缩进一次，这条列表从此不再续写」。
    let lead = line.chars().take_while(|c| *c == ' ').count();
    let trimmed: String = line.chars().skip(lead).collect();
    let (prefix, next, ordinal) = list_prefix(&trimmed)?;
    let body: String = trimmed.chars().skip(prefix).collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + 8);
    if body.trim().is_empty() {
        // 空列表项：只删前缀，**缩进留着**——用户可能只是想退回上一层列表，
        // 不该顺手把他缩进好的位置也抹掉。光标回到这行的内容起点。
        out.extend_from_slice(&chars[..start + lead]);
        out.extend_from_slice(&chars[start + lead + prefix..]);
        return Some((out.into_iter().collect(), start + lead));
    }
    let pad: String = " ".repeat(lead);
    let insert: Vec<char> = match ordinal {
        Some(n) => format!("\n{pad}{}. ", n + 1).chars().collect(),
        None => format!("\n{pad}{next}").chars().collect(),
    };
    out.extend_from_slice(&chars[..caret]);
    out.extend_from_slice(&insert);
    out.extend_from_slice(&chars[caret..]);
    Some((out.into_iter().collect(), caret + insert.len()))
}

/// 块标记（`docs/28` §6.3「即时转换」）：行首敲下这一串，本行就变成标题 / 引用 / 列表。
///
/// 只认**整行只剩标记**的情况——`## 标题` 不是「刚转换」，那是已经在用的标题行。
/// 返回标记的字符数（不含行尾换行）。
fn block_marker_len(line: &str) -> Option<usize> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    for p in ["# ", "## ", "### ", "#### ", "> ", "- ", "* ", "+ "] {
        if line == p {
            return Some(p.chars().count());
        }
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=3).contains(&digits) && line[digits..] == *". " {
        return Some(digits + 2);
    }
    None
}

/// 光标正因为「刚敲完一枚块标记」而停在该行行尾时，返回「把这枚标记整段去掉」的文本与光标。
///
/// 这是「`Ctrl+Z` 一次撤销转换」的实现依据：缓冲区里 `# ` 是两个字符，
/// 让 egui 自己撤销会先退空格、留下一个光秃秃的 `#`，看起来像没撤销。
fn marker_undo_edit(text: &str, caret: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    if caret > chars.len() {
        return None;
    }
    let mut start = caret;
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }
    if caret < chars.len() && chars[caret] != '\n' {
        return None;
    }
    let line: String = chars[start..caret].iter().collect();
    block_marker_len(&line)?;
    let mut out = chars[..start].to_vec();
    out.extend_from_slice(&chars[caret..]);
    Some((out.into_iter().collect(), start))
}

/// 本帧是不是「一格空格刚好补完块标记」。`before`/`caret_before` 是这一帧交给
/// `TextEdit` 之前的正文与光标字符位置。返回要并进 `auto_undo` 的（旧文本, 旧光标）。
fn marker_undo_step(before: &str, caret_before: usize, after: &str) -> Option<(String, usize)> {
    let b: Vec<char> = before.chars().collect();
    let a: Vec<char> = after.chars().collect();
    if a.len() != b.len() + 1 || caret_before > b.len() {
        return None;
    }
    if a.get(caret_before) != Some(&' ') {
        return None;
    }
    if a[..caret_before] != b[..caret_before] || a[caret_before + 1..] != b[caret_before..] {
        return None;
    }
    marker_undo_edit(after, caret_before + 1)
}

/// 多行**纯文本**粘进列表项时，给后几行补同型前缀（`docs/29` CER-17「粘贴纯文本按当前块处理」）。
/// 单行粘贴、段落里粘贴一律返回 `None`——那些交给 `TextEdit` 自己插，别抢。
///
/// 粘进来的是**成稿 Markdown**（行首自带 `#` / `-` / `> ` / 围栏…）时走另一条路：
/// 整段原样落成自己的块，不挂当前列表的前缀（`docs/28` §6.3「粘贴 Markdown 文本时保留结构」）。
/// 补前缀会把结构弄坏——`- 甲` 变成 `- - 甲`、`## 乙` 变成 `- ## 乙`。
fn paste_list(text: &str, caret: usize, clip: &str) -> Option<(String, usize)> {
    let clip = clip.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = clip.split('\n').collect();
    if lines.len() < 2 {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    if caret > chars.len() {
        return None;
    }
    let mut start = caret;
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }
    let mut end = start;
    while end < chars.len() && chars[end] != '\n' {
        end += 1;
    }
    let mut line: String = chars[start..end].iter().collect();
    if line.ends_with('\r') {
        line.pop();
    }
    // 与 `enter_edit` 同一个道理：缩进过的列表项行首是空格，
    // 直接拿整行去匹配 `list_prefix` 会认不出来，多行粘贴就退化成纯文本。
    let lead = line.chars().take_while(|c| *c == ' ').count();
    let trimmed: String = line.chars().skip(lead).collect();
    let (_, next, ordinal) = list_prefix(&trimmed)?;
    if clip_has_markdown_structure(&clip) {
        return Some(splice_structured(&chars, caret, start, end, &clip));
    }
    let pad: String = " ".repeat(lead);
    let mut insert = String::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            insert.push('\n');
            if l.trim().is_empty() {
                continue;
            }
            match ordinal {
                Some(n) => insert.push_str(&format!("{pad}{}. ", n + i as u64)),
                None => {
                    insert.push_str(&pad);
                    insert.push_str(next);
                }
            }
        }
        insert.push_str(l);
    }
    let n = insert.chars().count();
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + n);
    out.extend_from_slice(&chars[..caret]);
    out.extend(insert.chars());
    out.extend_from_slice(&chars[caret..]);
    Some((out.into_iter().collect(), caret + n))
}

/// 结构化粘贴：现有这一行留在原地，粘进来的 Markdown 自成整块插在光标处。
/// 光标前后只要还有字，就各补一个换行把块隔开——否则 `- 甲` 会和 `## 标题` 粘成一行。
fn splice_structured(
    chars: &[char],
    caret: usize,
    line_start: usize,
    line_end: usize,
    clip: &str,
) -> (String, usize) {
    let mut insert = String::new();
    if caret > line_start {
        insert.push('\n');
    }
    insert.push_str(clip);
    // 行尾只剩 CRLF 的 `\r` 时，后面的换行本来就由 `\n` 提供。
    let rest_is_cr = chars[caret..line_end].iter().all(|c| *c == '\r');
    if caret < line_end && !rest_is_cr {
        insert.push('\n');
    }
    let n = insert.chars().count();
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + n);
    out.extend_from_slice(&chars[..caret]);
    out.extend(insert.chars());
    out.extend_from_slice(&chars[caret..]);
    (out.into_iter().collect(), caret + n)
}

/// 粘进来的这一段是不是成稿 Markdown：任意一行行首带块标记就算。
fn clip_has_markdown_structure(clip: &str) -> bool {
    clip.lines()
        .any(|l| !l.trim().is_empty() && starts_with_block_marker(l))
}

/// trace 里别把整段正文都倒出来：超过 `max` 个字符就截断加省略号。
fn ellipsis(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

/// 行首块标记（判定「已经是成稿」用，和 `block_marker_len` 的「整行就是一枚标记」不同）。
fn starts_with_block_marker(line: &str) -> bool {
    let t = line.trim_start();
    let hashes = t.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') && t.len() > hashes + 1 {
        return true;
    }
    if t.starts_with("> ") || t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    let digits = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    if (1..=3).contains(&digits) && t[digits..].starts_with(". ") && t.len() > digits + 2 {
        return true;
    }
    t.starts_with("```") || t.starts_with("~~~") || t.starts_with('|') || t.trim_end() == "---"
}
/// 带外改完正文后统一收尾：写回光标，并把「改动前」的文本与光标记进 `auto_undo`。
///
/// 为什么不喂 `TextEdit` 自己的 undoer：`add_undo` 只放还原点，egui 下一帧还会按自己的
/// 规则（`stable_time` / `flux`）再补一个，`Ctrl+Z` 于是退过头（实测退到「粘过、但标记还
/// 隐藏」的中间态，正文整体右移 13px）。这里改成自己管**一步**撤销，行为可测。
fn store_after_edit(
    ctx: &egui::Context,
    body_id: egui::Id,
    caret: usize,
    before_text: &str,
    before_caret: usize,
    after_text: &str,
) -> Option<(String, usize, String)> {
    let mut st = egui::text_edit::TextEditState::load(ctx, body_id).unwrap_or_default();
    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
        egui::text::CCursor::new(caret),
    )));
    st.store(ctx, body_id);
    Some((before_text.to_owned(), before_caret, after_text.to_owned()))
}

/// 同 [`store_after_edit`]，但落的是**一整条选区**。
///
/// 多行缩进之后选区必须还在（Typora / 任何编辑器都这样），所以这里存的是
/// `CCursorRange`：`secondary` 是锚点、`primary` 是活动端。只存一个光标位
/// 会让「选中三行按 Tab」变成「缩进三行、选区消失」。
fn store_after_edit_range(
    ctx: &egui::Context,
    body_id: egui::Id,
    from: usize,
    to: usize,
    before_text: &str,
    before_caret: usize,
    after_text: &str,
) -> Option<(String, usize, String)> {
    let mut st = egui::text_edit::TextEditState::load(ctx, body_id).unwrap_or_default();
    st.cursor.set_char_range(Some(egui::text::CCursorRange {
        primary: egui::text::CCursor::new(to),
        secondary: egui::text::CCursor::new(from),
        h_pos: None,
    }));
    st.store(ctx, body_id);
    Some((before_text.to_owned(), before_caret, after_text.to_owned()))
}
fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// 光标字符位置落在哪个 `[文字](url)` 里；用于 Ctrl+点击打开链接。
fn link_at(text: &str, char_idx: usize) -> Option<String> {
    let byte = char_to_byte(text, char_idx);
    let mut search = 0usize;
    while let Some(rel) = text[search..].find('[') {
        let open = search + rel;
        if let Some((consumed, url)) = parse_link(&text[open..]) {
            let end = open + consumed;
            if (open..end).contains(&byte) && !url.is_empty() {
                return Some(url);
            }
            search = end;
        } else {
            search = open + 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> MdPalette {
        use crate::chrome::c32;
        use mw_core::design::shell;
        MdPalette {
            text: c32(shell::markdown::TEXT),
            dim: c32(shell::markdown::DIM),
            faint: c32(shell::markdown::FAINT),
            accent: c32(shell::markdown::ACCENT),
            code_bg: c32(shell::markdown::CODE_BG),
            bg: Color32::WHITE,
        }
    }

    fn job(text: &str) -> LayoutJob {
        markdown_job(text, palette(), 760.0)
    }

    fn text_of<'a>(j: &'a LayoutJob, s: &egui::text::LayoutSection) -> &'a str {
        &j.text[s.byte_range.start.0..s.byte_range.end.0]
    }

    #[test]
    fn markdown_job_preserves_every_byte() {
        let src = "# 标题\n\n正文 **粗体** 与 `code`，还有 [链接](https://example.com)。\n\n- [x] 完成\n> 引用\n\n```rs\nlet x = 1;\n```\n";
        assert_eq!(job(src).text, src);
        // 没写样式的地方（空串）也不能多出字符。
        assert_eq!(job("").text, "");
        assert_eq!(job("纯文本没有标记").text, "纯文本没有标记");
    }

    #[test]
    fn heading_uses_large_bold_font_and_dim_marker() {
        let j = job("# 标题\n正文");
        let heading = j
            .sections
            .iter()
            .find(|s| text_of(&j, s) == "标题")
            .expect("标题段");
        assert!(heading.format.font_id.size > BODY_PX + 4.0);
        assert_eq!(
            heading.format.font_id.family,
            FontFamily::Name(fonts::BOLD.into())
        );
        let hash = j
            .sections
            .iter()
            .find(|s| text_of(&j, s).starts_with('#'))
            .expect("井号段");
        assert_eq!(hash.format.color, palette().faint);
    }

    #[test]
    fn card_type_scale_matches_typography_plan() {
        let p = palette();
        assert_eq!(BODY_PX, 16.0);
        assert_eq!(BODY_LINE, 28.0);
        assert_eq!(LineStyle::heading(p, 1).size, 30.0);
        assert_eq!(LineStyle::heading(p, 1).line_h, 42.0);
        assert_eq!(LineStyle::heading(p, 2).size, 22.0);
        assert_eq!(LineStyle::heading(p, 2).line_h, 33.0);
        assert_eq!(CODE_PX, 14.0);
        assert_eq!(CODE_LINE, 22.0);
        assert_eq!(CODE_LANG_PX, 12.0);
        assert_eq!(TOP_BAR_H, 44.0);
    }

    /// 供片段级揭示的断言用：把 job 里每个「标记字符段」连同它是否可见（不透明）取出来。
    fn marker_runs(job: &egui::text::LayoutJob, src: &str) -> Vec<(String, bool)> {
        const TOKENS: [&str; 9] = ["**", "*", "_", "~~", "`", "[[", "]]", "![", "]("];
        job.sections
            .iter()
            .filter_map(|sec| {
                let text = src.get(sec.byte_range.start.0..sec.byte_range.end.0)?;
                if TOKENS.contains(&text) {
                    Some((text.to_string(), sec.format.color != Color32::TRANSPARENT))
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn only_the_fragment_under_the_caret_reveals_its_markers() {
        let src = "前 **粗** 后 `码` 尾";
        // 光标落在「粗」上：只有 `**` 露出来，反引号仍旧收起。
        let caret = src.find('粗').unwrap();
        let j = markdown_job_focus(src, palette(), 760.0, Some((0, caret)), None);
        let runs = marker_runs(&j, src);
        assert_eq!(
            runs.iter().filter(|(t, on)| t == "**" && *on).count(),
            2,
            "粗体两侧的 ** 都要露出来：{runs:?}"
        );
        assert_eq!(
            runs.iter().filter(|(t, on)| t == "`" && *on).count(),
            0,
            "不在光标片段里的反引号必须收起：{runs:?}"
        );
    }

    #[test]
    fn caret_in_plain_text_hides_every_inline_marker() {
        let src = "前 **粗** 后 `码` 尾";
        let j = markdown_job_focus(src, palette(), 760.0, Some((0, 0)), None);
        let runs = marker_runs(&j, src);
        assert!(!runs.is_empty(), "标记段本身要存在（只是透明）");
        assert!(
            runs.iter().all(|(_, on)| !*on),
            "光标在纯文字里时行内标记全收起：{runs:?}"
        );
    }

    #[test]
    fn caret_inside_code_span_reveals_only_backticks() {
        let src = "前 **粗** 后 `码` 尾";
        let caret = src.find('码').unwrap();
        let j = markdown_job_focus(src, palette(), 760.0, Some((0, caret)), None);
        let runs = marker_runs(&j, src);
        assert_eq!(
            runs.iter().filter(|(t, on)| t == "`" && *on).count(),
            2,
            "{runs:?}"
        );
        assert_eq!(
            runs.iter().filter(|(t, on)| t == "**" && *on).count(),
            0,
            "{runs:?}"
        );
    }

    #[test]
    fn caret_inside_strike_reveals_only_tildes() {
        let src = "前 ~~删~~ 后 **粗** 尾";
        let caret = src.find("删").unwrap();
        let j = markdown_job_focus(src, palette(), 760.0, Some((0, caret)), None);
        let runs = marker_runs(&j, src);
        let on = |tok: &str| runs.iter().filter(|(t, on)| t == tok && *on).count();
        assert_eq!(on("~~"), 2, "删除线的一对 ~~ 都要露：{runs:?}");
        assert_eq!(on("**"), 0, "别的片段的 ** 仍收起：{runs:?}");
    }

    #[test]
    fn caret_inside_image_reveals_bang_and_paren() {
        let src = "图 ![示意](a.png) 与 `码`";
        let caret = src.find("示意").unwrap();
        let j = markdown_job_focus(src, palette(), 760.0, Some((0, caret)), None);
        let runs = marker_runs(&j, src);
        let on = |tok: &str| runs.iter().filter(|(t, on)| t == tok && *on).count();
        assert_eq!(on("!["), 1, "图片的 ![ 要露：{runs:?}");
        assert_eq!(on("`"), 0, "同一行的行内码标记仍收起：{runs:?}");
        // `](` 与 URL 用同一格式 append，会被 egui 合并成一个 section，所以查「含 url 的 section 可见」。
        let url_visible = j.sections.iter().any(|sec| {
            let t = src
                .get(sec.byte_range.start.0..sec.byte_range.end.0)
                .unwrap_or("");
            t.contains("a.png") && sec.format.color != Color32::TRANSPARENT
        });
        assert!(url_visible, "图片的 ](url) 要露出来：{runs:?}");
    }
    #[test]
    fn store_after_edit_writes_caret_and_keeps_one_step_undo() {
        let ctx = egui::Context::default();
        let id = egui::Id::unique("mw-test-body");
        let slot = store_after_edit(&ctx, id, 2, "粘之前", 1, "粘之后");
        assert_eq!(slot, Some(("粘之前".to_owned(), 1, "粘之后".to_owned())));
        let st = egui::text_edit::TextEditState::load(&ctx, id).expect("状态要写进去");
        let at = st.cursor.char_range().map(|r| usize::from(r.primary.index));
        assert_eq!(at, Some(2), "光标要落在改动后的位置");
    }
    #[test]
    fn paste_multiline_into_bullet_prefixes_each_line() {
        let (out, at) = paste_list("- 甲", 3, "乙\n丙").expect("接管");
        assert_eq!(out, "- 甲乙\n- 丙");
        assert_eq!(at, out.chars().count());
    }

    #[test]
    fn paste_multiline_into_paragraph_is_left_alone() {
        assert!(paste_list("甲", 1, "乙\n丙").is_none());
    }

    #[test]
    fn paste_single_line_is_left_alone() {
        assert!(paste_list("- 甲", 4, "乙").is_none());
    }

    #[test]
    fn paste_multiline_into_ordered_item_numbers_the_lines() {
        let (out, _) = paste_list("3. 三", 4, "四\n五").expect("接管");
        assert_eq!(out, "3. 三四\n4. 五");
    }

    #[test]
    fn paste_multiline_into_task_item_stays_unchecked() {
        let (out, _) = paste_list("- [x] 甲", 7, "乙\n丙").expect("接管");
        assert_eq!(out, "- [x] 甲乙\n- [ ] 丙");
    }

    #[test]
    fn paste_multiline_keeps_blank_lines_blank() {
        let (out, _) = paste_list("- 甲", 3, "乙\n\n丙").expect("接管");
        assert_eq!(out, "- 甲乙\n\n- 丙");
    }

    #[test]
    fn pasting_markdown_keeps_its_own_structure() {
        // 行尾粘成稿：原行不动，粘进来的整段自成块，一个前缀都不补。
        let (out, at) = paste_list("- 甲", 3, "## 小节\n- 乙\n- 丙").expect("接管");
        assert_eq!(out, "- 甲\n## 小节\n- 乙\n- 丙");
        assert_eq!(at, out.chars().count(), "光标落在粘进来的段尾");
        // 行首粘：原行往后让一行，别和 `# 标题` 粘成一行。
        let (out, _) = paste_list("- 甲", 0, "# 标题\n正文").expect("接管");
        assert_eq!(out, "# 标题\n正文\n- 甲");
        // 行中粘：前后各断开一次，原来的后半句仍是自己的一行。
        let (out, _) = paste_list("- 甲乙", 3, "> 引用\n引用二").expect("接管");
        assert_eq!(out, "- 甲\n> 引用\n引用二\n乙");
    }

    #[test]
    fn structured_clip_is_recognized_by_its_block_markers() {
        for clip in [
            "# 标题\n正文",
            "- 甲\n- 乙",
            "1. 甲\n2. 乙",
            "> 引用\n引用二",
            "```rust\nlet a = 1;\n```",
            "| 甲 | 乙 |",
            "  - 缩进项\n- 项",
            "---",
        ] {
            assert!(
                super::clip_has_markdown_structure(clip),
                "{clip:?} 该算成稿"
            );
        }
        for clip in [
            "正文\n正文",
            "#标题没有空格\n正文",
            "####### 七级不算标题\n正文",
            "1.甲\n2.乙",
            "-甲\n-乙",
        ] {
            assert!(
                !super::clip_has_markdown_structure(clip),
                "{clip:?} 是纯文本，应当补前缀"
            );
        }
    }
    #[test]
    fn enter_on_empty_bullet_exits_the_list() {
        // 光标在 "- " 之后按回车：前缀消失，这一行退化成空段落，光标回到行首。
        let (text, at) = enter_edit("- ", 2).expect("空列表项要接管");
        assert_eq!(text, "");
        assert_eq!(at, 0);
    }

    #[test]
    fn every_block_marker_is_recognized_at_line_start() {
        for m in [
            "# ", "## ", "### ", "#### ", "> ", "- ", "* ", "+ ", "1. ", "12. ", "999. ",
        ] {
            let (text, at) = marker_undo_edit(m, m.chars().count())
                .unwrap_or_else(|| panic!("{m:?} 应当被认成块标记"));
            assert_eq!(text, "", "{m:?} 去掉之后该是空行");
            assert_eq!(at, 0, "{m:?} 的光标该回到行首");
        }
    }

    #[test]
    fn marker_undo_ignores_incomplete_markers_and_used_blocks() {
        // 没敲空格、`#` 后面已有字、五个 `#`、`1)` 都不是「刚转换」。
        for s in ["#", "##", "#x ", "# 标题", "##### ", "1)", "abc"] {
            assert!(
                marker_undo_edit(s, s.chars().count()).is_none(),
                "{s:?} 不该被当成刚完成的块标记"
            );
        }
    }

    #[test]
    fn marker_undo_only_strips_the_marker_line() {
        let (text, at) = marker_undo_edit("正文\n## ", 6).expect("第二行是标记");
        assert_eq!(text, "正文\n");
        assert_eq!(at, 3);
        // 光标不在行尾（后面还有字）→ 不是「刚敲完」，不动。
        assert!(marker_undo_edit("## 标题", 2).is_none());
    }

    #[test]
    fn space_completing_a_marker_becomes_one_undo_step() {
        // "#" + 空格：一次取消的旧文本是空行，而不是 "#"。
        let (text, at) = marker_undo_step("#", 1, "# ").expect("空格补完了标题");
        assert_eq!(text, "");
        assert_eq!(at, 0);
        // "1." + 空格：整枚 "1. " 一起去掉。
        let (text, _) = marker_undo_step("前\n1.", 4, "前\n1. ").expect("空格补完了有序项");
        assert_eq!(text, "前\n");
        // 普通打字里的空格不是转换。
        assert!(marker_undo_step("正文", 2, "正文 ").is_none());
        // 一次插两个字符（粘贴/输入法上屏）不算「敲下那一格」。
        assert!(marker_undo_step("#", 1, "# 标题").is_none());
    }

    #[test]
    fn enter_on_empty_ordered_item_exits_the_list() {
        let (text, at) = enter_edit("前一段\n1. ", 7).expect("空有序项要接管");
        assert_eq!(text, "前一段\n");
        assert_eq!(at, 4);
    }

    #[test]
    fn enter_on_empty_task_item_exits_the_list() {
        let (text, _) = enter_edit("- [ ] ", 6).expect("空任务项要接管");
        assert_eq!(text, "");
    }

    #[test]
    fn enter_on_bullet_with_text_continues_the_list() {
        let (text, at) = enter_edit("- 第一项", 5).expect("有内容的列表项要接管");
        assert_eq!(text, "- 第一项\n- ");
        assert_eq!(at, 8, "光标落在新前缀之后");
    }

    #[test]
    fn star_bullet_normalizes_to_dash_on_continue() {
        // 续写时统一成 "- "，避免同一份列表里符号乱换。
        let (text, _) = enter_edit("* 甲", 3).expect("星号列表项要接管");
        assert_eq!(text, "* 甲\n- ");
    }

    #[test]
    fn ordered_item_continues_with_next_number() {
        let (text, at) = enter_edit("9. 第九条", 6).expect("有序项要接管");
        assert_eq!(text, "9. 第九条\n10. ");
        assert_eq!(at, 11, "光标落在 \"10. \" 之后");
    }

    #[test]
    fn task_item_continues_as_an_unchecked_task() {
        let (text, _) = enter_edit("- [x] 做完了", 9).expect("任务项要接管");
        assert_eq!(text, "- [x] 做完了\n- [ ] ");
    }

    #[test]
    fn clicking_a_task_box_flips_only_the_box() {
        // `- [ ] 待办一` 的方框字符下标是 3（行首 `- ` 两个字符 + `[` 一个）。
        let body = "第一段\n\n- [ ] 待办一\n- [x] 待办二\n";
        let at = body.find("待办一").unwrap();
        let at = body[..at].chars().count() - 3; // 「待」往前三格（`] ` + 方框那一格）
        let (text, caret) = super::task_flip(body, at).expect("任务行要点得动");
        assert_eq!(text, "第一段\n\n- [x] 待办一\n- [x] 待办二\n");
        assert_eq!(
            text.chars().nth(caret),
            Some('\n'),
            "光标落在行尾，接着打字是继续这一条"
        );

        // 再点一次回到未勾选；大写 X 也算勾上。
        let (back, _) = super::task_flip(&text, at).unwrap();
        assert_eq!(back, body);
        let (upper, _) = super::task_flip("- [X] 大写", 3).unwrap();
        assert_eq!(upper, "- [ ] 大写");
    }

    #[test]
    fn task_flip_rejects_anything_that_is_not_the_box() {
        // 点到别的字符（列表圆点、正文）一律不动正文。
        assert!(
            super::task_flip("- [ ] 待办一", 0).is_none(),
            "列表符号不是方框"
        );
        assert!(
            super::task_flip("- [ ] 待办一", 6).is_none(),
            "正文不是方框"
        );
        assert!(
            super::task_flip("普通一句", 1).is_none(),
            "非任务行没有方框"
        );
        // 缩进任务行：方框跟着缩进走，命中下标也要跟着。
        let indented = "    - [ ] 缩进项";
        assert!(super::task_flip(indented, 3).is_none());
        assert_eq!(super::task_flip(indented, 7).unwrap().0, "    - [x] 缩进项");
    }

    #[test]
    fn enter_in_the_middle_of_a_line_is_not_taken_over() {
        assert!(enter_edit("- 第一项", 2).is_none(), "行中回车就是普通换行");
    }

    #[test]
    fn enter_on_a_heading_or_paragraph_is_not_taken_over() {
        assert!(
            enter_edit("## 小标题", 6).is_none(),
            "标题行不接管（新行自然不带 ##）"
        );
        assert!(enter_edit("正文一句", 4).is_none(), "段落行不接管");
    }
    #[test]
    fn col_in_line_counts_bytes_not_chars() {
        let text = "中文\n**粗**";
        // 第二行首字符（`*`）的字节列是 0，`粗` 的字节列是 2。
        assert_eq!(super::col_in_line(text, 3), 0);
        assert_eq!(super::col_in_line(text, 5), 2);
        assert_eq!(super::col_in_line(text, 0), 0);
    }
    #[test]
    fn markers_collapse_off_the_active_line_but_text_survives() {
        let src = "# 标题\n正文 **粗体** 与 `码` 和 [链接](https://e.com)\n";
        // 光标放进「粗体」这个片段里：只有它的 `**` 该露出（§6.3 片段级揭示）。
        let line_start = src.find("正文").unwrap();
        let caret = src.find("粗体").unwrap() - line_start;
        let j = markdown_job_focus(src, palette(), 760.0, Some((1, caret)), None);
        assert_eq!(j.text, src, "收起标记不能改动正文一个字节");
        let hash = j
            .sections
            .iter()
            .find(|s| text_of(&j, s).starts_with('#'))
            .expect("井号仍在文本里");
        assert!(
            hash.format.font_id.size < 1.0,
            "非光标行的标记应缩到不可见，实际 {}",
            hash.format.font_id.size
        );
        let star = j
            .sections
            .iter()
            .find(|s| text_of(&j, s) == "**")
            .expect("光标行的 ** 仍显示");
        assert_eq!(star.format.color, palette().faint);
        let bold = j
            .sections
            .iter()
            .find(|s| text_of(&j, s) == "粗体")
            .expect("粗体段");
        assert_eq!(
            bold.format.font_id.family,
            FontFamily::Name(fonts::BOLD.into())
        );
        // 换到标题行：链接所在的这一行变成非光标行，URL 与括号一起收起。
        let j2 = markdown_job_focus(src, palette(), 760.0, Some((0, 0)), None);
        let url = j2
            .sections
            .iter()
            .find(|s| text_of(&j2, s).contains("https"))
            .expect("URL 仍是文本的一部分");
        assert!(url.format.font_id.size < 1.0, "非光标行收起 URL");
        let link = j2
            .sections
            .iter()
            .find(|s| text_of(&j2, s) == "链接")
            .expect("链接文字保留");
        assert_eq!(link.format.color, palette().accent);
    }

    #[test]
    fn unfocused_card_hides_all_markers() {
        let src = "# 标题\n- 项目\n> 引用\n";
        let j = markdown_job_focus(src, palette(), 760.0, None, None);
        assert_eq!(j.text, src);
        for s in &j.sections {
            let t = text_of(&j, s);
            if t.starts_with('#') {
                assert!(
                    s.format.font_id.size < 1.0,
                    "失焦后标题标记应全部收起：{t:?}"
                );
            } else if is_bullet_line(t) {
                // 列表 / 引用标记不再缩到 0.1：它们要用字距把正文推到 16 / 24 的缩进，
                // 所以留 4px 隐形字，靠「透明」而不是「极小」来隐身。
                assert_eq!(s.format.color, Color32::TRANSPARENT, "标记必须隐形：{t:?}");
                assert!(s.format.font_id.size <= 4.0, "标记字要保持小：{t:?}");
                assert!(
                    s.format.extra_letter_spacing > 0.0,
                    "标记要用字距撑出缩进：{t:?}"
                );
            }
        }
    }

    #[test]
    fn line_index_at_counts_newlines() {
        assert_eq!(line_index_at("abc", 0), 0);
        assert_eq!(line_index_at("abc\ndef", 4), 1);
        assert_eq!(line_index_at("a\nb\nc", 4), 2);
    }

    #[test]
    fn inline_code_and_bold_get_distinct_formats() {
        let j = job("**粗** 和 `码`");
        let bold = j
            .sections
            .iter()
            .find(|s| text_of(&j, s) == "粗")
            .expect("粗体段");
        assert_eq!(
            bold.format.font_id.family,
            FontFamily::Name(fonts::BOLD.into())
        );
        let code = j
            .sections
            .iter()
            .find(|s| text_of(&j, s) == "码")
            .expect("代码段");
        assert_eq!(code.format.font_id.family, FontFamily::Monospace);
        assert_eq!(code.format.background, palette().code_bg);
    }

    #[test]
    fn code_lines_are_monospace_but_markers_are_not() {
        let j = job("```\nlet x = 1;\n```");
        let code = j
            .sections
            .iter()
            .find(|s| text_of(&j, s).trim_end_matches('\n') == "let x = 1;")
            .expect("代码行");
        assert_eq!(code.format.font_id.family, FontFamily::Monospace);
        assert_eq!(code.format.line_height, Some(CODE_LINE));
    }

    #[test]
    fn headings_skip_code_fences_and_keep_char_offsets() {
        let body = "# 一\n\n```\n# 不是标题\n```\n\n## 二\n";
        let hs = headings(body);
        assert_eq!(hs.len(), 2);
        assert_eq!(hs[0].text, "一");
        assert_eq!(hs[0].char_index, 0);
        assert_eq!(hs[1].level, 2);
        assert_eq!(hs[1].text, "二");
        let idx = body.chars().take(hs[1].char_index).collect::<String>();
        assert!(idx.ends_with("```\n\n"), "下标应落在「## 二」行首");
    }

    #[test]
    fn reading_count_mixes_cjk_and_latin_words() {
        assert_eq!(reading_count("你好 世界"), 4);
        assert_eq!(reading_count("hello world"), 2);
        assert_eq!(reading_count("你好，world!"), 3);
        assert_eq!(reading_count("   \n"), 0);
    }

    #[test]
    fn dates_are_stable_utc_dates() {
        assert_eq!(format_date(0), "1970-01-01");
        // 2026-10-20 00:00:00 UTC
        assert_eq!(format_date(1_792_454_400_000), "2026-10-20");
    }

    /// CE-01 D13：字数按正文快照缓存，正文一变就必须重算。
    #[test]
    fn word_count_cache_tracks_the_body() {
        let mut r = CardReader::new(&NoteCard::new("doc", None, "t", "甲乙", 0));
        assert_eq!(r.word_count(), 2);
        // 同一份正文再问一次：结果不变（走缓存）。
        assert_eq!(r.word_count(), 2);
        r.draft_body = "甲乙丙".into();
        assert_eq!(r.word_count(), 3, "正文一变就要重算");
        // 缩到空：清零，不是留着上一次的数。
        r.draft_body.clear();
        assert_eq!(r.word_count(), 0);
    }

    #[test]
    fn card_reader_parses_draft_and_metadata() {
        let mut card = NoteCard::new("doc", Some("n1".into()), "标题", "# 一\n\n正文", 0);
        card.tags = vec!["知识".into()];
        let mut r = CardReader::new(&card);
        assert_eq!(r.title, "标题");
        assert_eq!(r.tags, vec!["知识"]);
        assert_eq!(r.headings().len(), 1);
        assert!(r.word_count() > 0);
        assert!(!r.is_dirty());
    }

    #[test]
    fn saving_takes_a_new_baseline_and_derives_blank_title() {
        let card = NoteCard::new("doc", None, "", "旧正文", 0);
        let mut r = CardReader::new(&card);
        assert!(r.draft_title.is_empty());
        r.draft_body = "## 新正文\n\n内容".into();
        assert!(r.is_dirty());
        r.mark_saved();
        assert!(!r.is_dirty());
        assert_eq!(r.title, "新正文");
        assert_eq!(r.headings()[0].text, "新正文");
        // 再改一笔，脏状态必须回来。
        r.draft_body.push('!');
        assert!(r.is_dirty());
    }

    #[test]
    fn autosave_waits_for_the_debounce_and_never_overlaps_a_write() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let idle = Some(t0);
        // 刚脏：不到 800ms 一律不落盘。
        assert!(!super::autosave_due(
            true,
            false,
            idle,
            t0 + Duration::from_millis(799),
            Duration::from_millis(800)
        ));
        // 停手满了才落。
        assert!(super::autosave_due(
            true,
            false,
            idle,
            t0 + Duration::from_millis(800),
            Duration::from_millis(800)
        ));
        // 正在写盘：不许重排队（否则失败路径会每帧重试、打满磁盘）。
        assert!(!super::autosave_due(
            true,
            true,
            idle,
            t0 + Duration::from_secs(5),
            Duration::from_millis(800)
        ));
        // 不脏就不写。
        assert!(!super::autosave_due(
            false,
            false,
            idle,
            t0 + Duration::from_secs(5),
            Duration::from_millis(800)
        ));
        // 没在计时（刚失败过、还没新输入）也不写。
        assert!(!super::autosave_due(
            true,
            false,
            None,
            t0 + Duration::from_secs(5),
            Duration::from_millis(800)
        ));
    }

    #[test]
    fn a_failed_save_keeps_the_draft_and_stops_the_retry_loop() {
        use std::time::Instant;
        let card = NoteCard::new("doc", None, "T", "B", 0);
        let mut r = CardReader::new(&card);
        r.draft_body = "改过的正文".into();
        r.dirty_since = Some(Instant::now());
        r.mark_saving();
        assert!(r.saving);
        r.mark_save_failed("磁盘已满");
        assert!(!r.saving, "失败后不再显示保存中");
        assert_eq!(r.save_error.as_deref(), Some("磁盘已满"));
        assert_eq!(r.draft_body, "改过的正文", "草稿一个字节都不能丢");
        assert!(r.is_dirty());
        assert!(
            r.dirty_since.is_none(),
            "失败后要等下一次输入或 Ctrl+S，不能每帧重排队"
        );
        // 再存成功：错误与脏状态一起清掉。
        r.mark_saving();
        r.mark_saved();
        assert!(r.save_error.is_none());
        assert!(!r.saving);
        assert!(!r.is_dirty());
    }

    /// 取值小工具：只关心保存动作里的（标题, 正文）。
    fn saved(a: ReaderAction) -> (String, String) {
        match a {
            ReaderAction::Save { title, body, .. } => (title, body),
            other => panic!("不是保存动作：{other:?}"),
        }
    }

    /// CE-01 D15：正文开头的 `# 标题` 就是标题，不再画第二遍；
    /// 保存时按原样拼回去，**逐字节**还原（含 CRLF 与「整篇只有标题」）。
    #[test]
    fn a_leading_h1_is_the_title_and_round_trips_byte_for_byte() {
        for body in [
            "# 标题\n\n正文",
            "# 标题\n正文",
            "# 标题",
            "# 标题\r\n\r\n正文",
            "# 标题\n",
        ] {
            let card = NoteCard::new("doc", None, "标题", body, 0);
            let r = CardReader::new(&card);
            assert!(
                !r.draft_body.starts_with("# 标题"),
                "{body:?} 的标题行该被摘掉，实际 {:?}",
                r.draft_body
            );
            assert!(!r.is_dirty(), "{body:?} 刚打开不该是脏的");
            let (t, b) = saved(r.save_action(false));
            assert_eq!(t, "标题");
            assert_eq!(b, body, "{body:?} 保存出去必须逐字节还原");
        }
    }

    /// 改标题就是把那行 H1 改掉；清空标题就是那一行不该再存在。
    #[test]
    fn renaming_or_clearing_the_title_rewrites_the_leading_h1() {
        let mut r = CardReader::new(&NoteCard::new("doc", None, "旧名", "# 旧名\n\n正文", 0));
        assert_eq!(r.draft_body, "正文");
        r.draft_title = "新名".into();
        assert_eq!(saved(r.save_action(false)).1, "# 新名\n\n正文");
        r.draft_title.clear();
        assert_eq!(saved(r.save_action(false)).1, "正文");
    }

    /// 只是「以标题开头」的另一行（`# 标题党`）不许动。
    #[test]
    fn a_heading_that_merely_starts_with_the_title_is_left_alone() {
        let r = CardReader::new(&NoteCard::new("doc", None, "标题", "# 标题党\n\n正文", 0));
        assert_eq!(r.draft_body, "# 标题党\n\n正文");
        assert_eq!(saved(r.save_action(false)).1, "# 标题党\n\n正文");
        // 没有空格的一级标题也不算（Markdown 不认）。
        let r = CardReader::new(&NoteCard::new("doc", None, "标题", "#标题\n正文", 0));
        assert_eq!(r.draft_body, "#标题\n正文");
    }

    #[test]
    fn save_action_carries_close_flag() {
        let card = NoteCard::new("doc", None, "T", "B", 0);
        let mut r = CardReader::new(&card);
        r.draft_title = "T2".into();
        r.draft_body = "B2".into();
        r.draft_tags = vec!["标签".into()];
        assert_eq!(
            r.save_action(true),
            ReaderAction::Save {
                title: "T2".into(),
                body: "B2".into(),
                tags: vec!["标签".into()],
                close: true
            }
        );
    }

    /// CE-01 D1：光标落在空行上时，那一行必须是**一整行字高**。
    ///
    /// 改前实测：`"abc\n\n"` 的光标矩形是 `(0,28)-(0,28)`——高度 0，屏幕上
    /// 根本找不到光标。这正是「写完一段、回车空一行、再回车」之后的样子。
    #[test]
    fn blank_line_keeps_a_full_row_for_the_caret() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        for body in ["abc\n\n", "abc\n\n\n", "abc\n\n\n\n"] {
            // 光标停在正文最末尾：先按 `line_index_at` 数行（会数到幻影行），
            // 与壳层每帧写 `active` 的口径一致。
            let end = body.chars().count();
            let line = line_index_at(body, end);
            let g = ctx.fonts_mut(|f| {
                f.layout_job(markdown_job_focus(
                    body,
                    palette(),
                    760.0,
                    Some((line, 0)),
                    None,
                ))
            });
            let caret = g.pos_from_cursor(egui::text::CCursor::new(end));
            assert!(
                caret.height() >= BODY_LINE - 0.5,
                "{body:?} 的光标高 {:.1}，至少要 {BODY_LINE}（改前是 0）",
                caret.height()
            );
        }
    }

    /// CE-01 D1 的反面：**不在光标上的**连续空行仍要折叠成一份间距。
    ///
    /// 「光标行给足行高」很容易写成「所有空行都给足行高」——那会把
    /// `docs/28` §6.2 的段间 12 变成 28，整篇节奏全垮。这条钉住另一半。
    #[test]
    fn blank_lines_without_the_caret_still_collapse() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        let body = "甲\n\n\n乙";
        let g = ctx.fonts_mut(|f| {
            f.layout_job(markdown_job_focus(
                body,
                palette(),
                760.0,
                Some((0, 0)),
                None,
            ))
        });
        // row0=甲、row1=第一个空行（拿间距 12）、row2=第二个空行（压到 0）、row3=乙
        let rows: Vec<f32> = g.rows.iter().map(|r| r.rect().height()).collect();
        assert!(rows.len() >= 4, "该有四行，实测 {rows:?}（body={body:?}）");
        assert_eq!(rows[1], 12.0, "第一个空行承担段间 12：{rows:?}");
        assert_eq!(rows[2], 0.0, "第二个空行要折叠成 0：{rows:?}");
        assert!(rows[3] >= BODY_LINE - 0.5, "正文照常 28：{rows:?}");
    }

    /// CE-01 D1 的另一半：`active` 指向的幻影行不能把整页标记都按灭。
    ///
    /// `line_index_at` 数 `\n`，`build_job` 遍历 `split_inclusive('\n')`；
    /// 正文以换行结尾时光标停在末尾会多出一行。夹取之后，最后一条源行必须是
    /// 光标行——否则那一帧没有任何一行揭标记，正文突然全变成「成稿」。
    #[test]
    fn caret_line_never_points_past_the_last_source_line() {
        let body = "abc\n\n";
        let end = body.chars().count();
        assert_eq!(line_index_at(body, end), 2, "数换行会得到第 2 行（不存在）");
        let last = body.split_inclusive('\n').count() - 1;
        assert_eq!(last, 1, "split_inclusive 只有 0/1 两行");
        // 夹取后的行必须还能揭出标记：拿一条带块标记的正文来试。
        let body = "## 标题\n";
        let end = body.chars().count();
        let clamped = line_index_at(body, end).min(body.split_inclusive('\n').count() - 1);
        assert_eq!(clamped, 0, "末尾换行时该落回唯一那条源行");
        let j = markdown_job_focus(body, palette(), 760.0, Some((clamped, 0)), None);
        let hash = j
            .sections
            .iter()
            .find(|s| text_of(&j, s).starts_with("##"))
            .expect("井号段");
        assert_ne!(
            hash.format.color,
            Color32::TRANSPARENT,
            "光标行（哪怕是末尾幻影行）的块标记要露出来"
        );
    }

    /// CE-01 D12：缩进过的列表项**照样**续写、照样带缩进粘贴。
    ///
    /// 这一条是给 D4 补的后账：`Tab` 能把列表推进去一层，但 `list_prefix`
    /// 只认「行首」，行首变成空格之后续写就断了——缩进一次，这条列表
    /// 从此回车只换行、多行粘贴退化成纯文本。
    #[test]
    fn indented_list_items_still_continue_and_paste() {
        assert_eq!(enter_edit("  - 甲", 5), Some(("  - 甲\n  - ".into(), 10)));
        assert_eq!(
            enter_edit("  1. 甲", 6),
            Some(("  1. 甲\n  2. ".into(), 12))
        );
        // 空项：只删标记，缩进留着（用户可能只是想退回上一层）。
        assert_eq!(enter_edit("  - ", 4), Some(("  ".into(), 2)));
        assert_eq!(
            paste_list("  - 甲", 5, "乙\n丙").map(|(t, _)| t),
            Some("  - 甲乙\n  - 丙".to_string())
        );
    }

    /// CE-01 D5：自定义 layouter 必须保留行尾空白，否则行尾敲空格光标不动。
    #[test]
    fn layout_job_keeps_trailing_whitespace() {
        assert!(job("甲 乙").keep_trailing_whitespace);
        assert!(
            markdown_job_focus("甲 ", palette(), 760.0, Some((0, 2)), None)
                .keep_trailing_whitespace
        );
    }

    /// CE-01 D3：有序列表的编号由绘制层负责，文字层只留透明占位。
    #[test]
    fn ordered_marker_is_reported_for_the_paint_layer() {
        assert_eq!(ordered_marker("1. 甲"), Some(("1.", 3)));
        assert_eq!(ordered_marker("12) 乙"), Some(("12)", 4)));
        assert_eq!(ordered_marker("999. 丙"), Some(("999.", 5)));
        assert_eq!(ordered_marker("1234. 丁"), None, "超过三位数字不算列表");
        assert_eq!(ordered_marker("1.甲"), None, "点号后面必须有空格");
        assert_eq!(ordered_marker("- 甲"), None);
        assert_eq!(ordered_marker("正文"), None);
    }

    /// CE-01 D3 的同源问题：列表正文的起点不能因为光标进退而横跳。
    ///
    /// 改前 `marker_indent` 在光标行返回可见标记（自然宽 9/17px），其余行返回
    /// 4px 透明占位（凑满 24）——点进一行列表，那一行正文就左移十几个像素。
    #[test]
    fn list_indent_does_not_move_with_the_caret() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        for line in ["- 甲", "1. 甲", "- [ ] 甲", "> 甲"] {
            let marker_len = list_marker_len(line)
                .or_else(|| ordered_marker(line).map(|(_, n)| n))
                .unwrap_or(2);
            let mut xs = Vec::new();
            for active in [None, Some((0usize, 0usize))] {
                let j = markdown_job_focus(line, palette(), 760.0, active, None);
                let g = ctx.fonts_mut(|f| f.layout_job(j));
                xs.push(
                    g.pos_from_cursor(egui::text::CCursor::new(marker_len))
                        .left(),
                );
            }
            assert!(
                (xs[0] - xs[1]).abs() < 0.6,
                "{line}: 光标进出行时正文横跳 {xs:?}"
            );
            let want = if line.starts_with('>') { 16.0 } else { 24.0 };
            assert!(
                (xs[0] - want).abs() < 1.5,
                "{line}: 正文起点该是 {want}，实测 {:.1}",
                xs[0]
            );
        }
    }

    /// CE-01 D4：Tab 一律换成空格，绝不往缓冲区里写裸 `\t`。
    #[test]
    fn tab_indents_with_spaces_and_never_writes_a_tab() {
        // 列表行：整行缩进两格，光标跟着走两格。
        assert_eq!(
            indent_edit("- 甲", 2, 2, false),
            Some(("  - 甲".into(), 4, 4))
        );
        assert_eq!(
            indent_edit("1. 甲", 3, 3, false),
            Some(("  1. 甲".into(), 5, 5))
        );
        assert_eq!(
            indent_edit("- [ ] 甲", 6, 6, false),
            Some(("  - [ ] 甲".into(), 8, 8))
        );
        // 段落：光标处补四个空格当软缩进。
        assert_eq!(
            indent_edit("甲乙", 1, 1, false),
            Some(("甲    乙".into(), 5, 5))
        );
        for line in ["- 甲", "1. 甲", "甲乙", "- [ ] 甲", "> 甲"] {
            let out = indent_edit(line, 0, 0, false).expect("该接管").0;
            assert!(!out.contains('\t'), "写进了裸制表符：{out:?}");
        }
    }

    /// CE-01 D11：选中多行按 `Tab`，**每一行**都要缩进，而且选区还在。
    ///
    /// 改前只认一个光标位：选中三行按 Tab 只缩进一行，还把选区打散。
    #[test]
    fn tab_indents_every_selected_line_and_keeps_the_selection() {
        let src = "- 甲\n- 乙\n- 丙";
        // 选中第 1–2 行的中间一段（不跨到第 3 行）。
        let (text, from, to) = indent_edit(src, 0, 7, false).expect("该接管");
        assert_eq!(text, "  - 甲\n  - 乙\n- 丙");
        assert_eq!((from, to), (0, 11), "选区把两处缩进都圈进来，仍在第 1–2 行");
        // 整段（含末尾换行）选中：只缩进覆盖到的行。
        let (text, _, _) = indent_edit("- 甲\n- 乙\n", 0, 8, false).unwrap();
        assert_eq!(text, "  - 甲\n  - 乙\n");
        // 段落多行：一次四格。
        let (text, _, _) = indent_edit("甲\n乙", 0, 3, false).unwrap();
        assert_eq!(text, "    甲\n    乙");
        // 反缩进整段：每行退两格（列表行）。
        let (text, from, to) = indent_edit("  - 甲\n  - 乙", 0, 11, true).unwrap();
        assert_eq!(text, "- 甲\n- 乙");
        assert_eq!(to - from, 11 - 4, "选区长度跟着缩短");
        // 一行都没得退：不接管，把键留给上层。
        assert_eq!(indent_edit("- 甲\n- 乙", 0, 7, true), None);
    }

    /// CE-01 D4b：`Shift+Tab` 只退行首的空格，正文中间的字符一个不碰。
    #[test]
    fn shift_tab_outdents_only_the_leading_spaces() {
        assert_eq!(
            indent_edit("  - 甲", 4, 4, true),
            Some(("- 甲".into(), 2, 2))
        );
        assert_eq!(indent_edit("    甲", 5, 5, true), Some(("甲".into(), 1, 1)));
        // 列表一次退两格，段落一次退四格。
        assert_eq!(
            indent_edit("      - 甲", 8, 8, true),
            Some(("    - 甲".into(), 6, 6))
        );
        // 没有前导空格：不接管，把键留给上层。
        assert_eq!(indent_edit("- 甲", 2, 2, true), None);
    }

    /// CE-01：行首退格把列表项退回普通段落，而不是一格一格退成残骸。
    #[test]
    fn backspace_at_the_marker_end_leaves_the_list() {
        assert_eq!(backspace_marker("- 甲", 2), Some(("甲".into(), 0)));
        assert_eq!(backspace_marker("1. 甲", 3), Some(("甲".into(), 0)));
        assert_eq!(backspace_marker("- [ ] 甲", 6), Some(("甲".into(), 0)));
        // 缩进过的列表：只删标记，前导空格留着。
        assert_eq!(backspace_marker("  - 甲", 4), Some(("  甲".into(), 2)));
        // 光标不在标记末尾、行首按退格：都交给 `TextEdit`。
        assert_eq!(backspace_marker("- 甲", 3), None);
        assert_eq!(backspace_marker("- 甲", 0), None);
        assert_eq!(backspace_marker("正文", 1), None);
    }

    /// CE-01 D5 的守门人：开 `keep_trailing_whitespace` 只影响行尾空白的对齐，
    /// **不许改变行盒几何**。它若改了高度，整页会随之一像素一像素地往下挪。
    #[test]
    fn trailing_whitespace_flag_never_moves_the_rows() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        for body in [
            "第一段。\n\n第二段，写点字。\n",
            "# 标题\n\n正文\n",
            "- 甲\n- 乙\n",
            "1. 甲\n2. 乙\n",
        ] {
            let rows = |keep: bool| {
                let mut j = markdown_job_focus(body, palette(), 760.0, Some((0, 0)), None);
                j.keep_trailing_whitespace = keep;
                let g = ctx.fonts_mut(|f| f.layout_job(j));
                g.rows
                    .iter()
                    .map(|r| (r.rect().top(), r.rect().bottom()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                rows(false),
                rows(true),
                "{body:?} 的行盒在开/关行尾空白后必须一致"
            );
        }
    }

    /// `docs/28` §6.5：`Ctrl+/` 也要能叫出斜杠菜单。
    #[test]
    fn ctrl_slash_starts_the_menu_on_a_blank_line_or_a_new_one() {
        // 行首空着：直接补一个 `/`。
        assert_eq!(slash_invoke_edit("正文\n", 3), ("正文\n/".to_string(), 4));
        // 光标前只有空格也算空行。
        assert_eq!(slash_invoke_edit("  ", 2), ("  /".to_string(), 3));
        // 行里已经有字：另起一行再补，否则菜单判定不到「行首」。
        assert_eq!(slash_invoke_edit("甲乙", 1), ("甲\n/乙".to_string(), 3));
        // 光标在文末且正文为空。
        assert_eq!(slash_invoke_edit("", 0), ("/".to_string(), 1));
    }

    #[test]
    fn hidden_list_marker_indents_the_text_by_24px() {
        let ctx = egui::Context::default();
        // 字体要在第一帧之后才可排版：离屏 Context 必须先开一帧。
        ctx.begin_pass(egui::RawInput::default());
        let j = markdown_job_focus("- 列表", palette(), 760.0, None, None);
        let galley = ctx.fonts_mut(|f| f.layout_job(j));
        let first = galley.pos_from_cursor(egui::text::CCursor::new(2)).left();
        // 标记两字符（`- `）的落点就是正文起点：合同要求 24。
        assert!(
            (first - 24.0).abs() < 1.5,
            "列表正文起点应为 24px，实测 {first:.1}"
        );
    }

    #[test]
    fn block_gaps_follow_the_spec_table() {
        let g = |next: &str| {
            let lines = vec!["", next, ""];
            spacing_before(&lines, 0, false)
        };
        assert_eq!(g("正文"), 12.0);
        assert_eq!(g("## 二级"), 40.0);
        assert_eq!(g("### 三级"), 28.0);
        assert_eq!(g("#### 四级"), 24.0);
        assert_eq!(g("- 列表"), 8.0);
        assert_eq!(g("1. 有序"), 8.0);
        assert_eq!(g("> 引用"), 12.0);
        assert_eq!(g("---"), 32.0);
        assert_eq!(g("```rs"), 12.0);
        // 代码块里的空行不算块间距，压到 0。
        assert_eq!(spacing_before(&["", "正文", ""], 0, true), 0.0);
    }

    #[test]
    fn adding_tags_strips_hash_and_dedupes_case_insensitively() {
        let mut r = CardReader::new(&NoteCard::new("doc", None, "标题", "正文", 0));
        r.add_tag("#知识/方法");
        r.add_tag("  知识/方法  ");
        r.add_tag("整理术");
        assert_eq!(
            r.draft_tags,
            vec!["知识/方法".to_string(), "整理术".to_string()]
        );
        // 加完就清空输入框，否则回车会重复提交同一串。
        assert!(r.tag_input.is_empty());
    }

    #[test]
    fn removing_a_tag_leaves_the_others_in_order() {
        let mut r = CardReader::new(&NoteCard::new("doc", None, "标题", "正文", 0));
        r.add_tag("甲");
        r.add_tag("乙");
        r.add_tag("丙");
        r.draft_tags.remove(1);
        assert_eq!(r.draft_tags, vec!["甲".to_string(), "丙".to_string()]);
    }

    #[test]
    fn tag_colors_are_stable_and_rooted() {
        assert_eq!(tag_color("知识"), tag_color("知识/方法"));
        assert_eq!(tag_color("知识"), tag_color("知识"));
        assert_ne!(tag_color("知识"), tag_color("方法"));
    }

    /// 斜杠菜单（S11）：只认行首 `/查询`，空白 / 正文斜杠 / 第二根斜杠都不算。
    #[test]
    fn slash_context_only_fires_at_the_start_of_a_line() {
        let text = "开头\n  /";
        let sc = super::slash_context(text, text.chars().count()).unwrap();
        assert_eq!(sc.start_char, 5, "开头\\n 是两个字符 + 两个空格");
        assert_eq!(sc.query, "");

        let text = "第一行\n/代码";
        let sc = super::slash_context(text, text.chars().count()).unwrap();
        assert_eq!(sc.query, "代码");
        assert_eq!(sc.start_char, 4);

        assert!(
            super::slash_context("abc/", 4).is_none(),
            "正文里的斜杠不弹菜单"
        );
        assert!(super::slash_context("/a b", 4).is_none(), "空白关闭菜单");
        assert!(
            super::slash_context("/a/b", 4).is_none(),
            "第二根斜杠关闭菜单"
        );
        assert!(super::slash_context("/", 0).is_none(), "光标在斜杠前不算");
    }

    /// 过滤：中文标签、英文别名、空查询全量。
    #[test]
    fn slash_matches_labels_and_aliases() {
        assert_eq!(super::slash_matches("").len(), super::SLASH_ITEMS.len());
        let code = super::slash_matches("代码");
        assert_eq!(code.len(), 1);
        assert_eq!(super::SLASH_ITEMS[code[0]].label, "代码块");
        assert!(super::slash_matches("h2")
            .iter()
            .any(|&i| super::SLASH_ITEMS[i].insert == "## "));
        assert_eq!(
            super::SLASH_ITEMS[super::slash_matches("todo")[0]].label,
            "待办"
        );
        assert!(super::slash_matches("没有这个块").is_empty());
    }

    /// 按 label 取菜单项——下标会随条目增删漂，测试不该钉死数字。
    fn item(label: &str) -> &'static super::SlashItem {
        super::SLASH_ITEMS
            .iter()
            .find(|it| it.label == label)
            .unwrap_or_else(|| panic!("菜单里没有「{label}」"))
    }

    /// 合同几何：280 宽、行高 30、圆角 10、最多 8 行。
    #[test]
    fn slash_menu_geometry_matches_the_contract() {
        assert_eq!(super::SLASH_W, 280.0);
        assert_eq!(super::SLASH_ROW_H, 30.0);
        assert_eq!(super::SLASH_RADIUS, 10);
        assert_eq!(super::SLASH_MAX_ROWS, 8);
        assert_eq!(super::slash_visible_rows(3), 3);
        assert_eq!(super::slash_visible_rows(8), 8);
        assert_eq!(super::slash_visible_rows(10), 8, "超过 8 条要滚，不许长高");
    }

    /// 条目齐不齐 + 有序 / 图片两项的插入结果与光标落点。
    #[test]
    fn slash_items_cover_the_contract_list() {
        for label in [
            "标题 1",
            "标题 2",
            "标题 3",
            "无序列表",
            "有序列表",
            "待办",
            "引用",
            "代码块",
            "分割线",
            "图片",
        ] {
            item(label);
        }
        assert_eq!(super::SLASH_ITEMS.len(), 10);

        assert_eq!(item("有序列表").insert, "1. ");
        assert!(item("有序列表").keys.contains(&"ol"));

        let img = item("图片");
        assert_eq!(img.insert, "![图片]()");
        // 光标落在 `![` 之后、`]` 之前：先改说明文字，再进括号填路径。
        let body = "/图片";
        let caret = body.chars().count();
        let sc = super::slash_context(body, caret).unwrap();
        let (out, new_caret) = super::slash_apply_text(body, &sc, caret, img);
        assert_eq!(out, "![图片]()");
        assert_eq!(new_caret, "![图片".chars().count());
    }
    /// 替换：只吃掉 `/查询`，前后文原样保留；代码块光标落在围栏中间。
    #[test]
    fn slash_apply_replaces_only_the_query_and_places_the_caret() {
        let body = "前言\n/代码\n后记";
        let caret = "前言\n/代码".chars().count();
        let sc = super::slash_context(body, caret).unwrap();
        let (out, new_caret) = super::slash_apply_text(body, &sc, caret, item("代码块"));
        assert_eq!(out, "前言\n```\n\n```\n后记");
        assert_eq!(new_caret, "前言\n```\n".chars().count());

        let body = "/待办";
        let caret = body.chars().count();
        let sc = super::slash_context(body, caret).unwrap();
        let (out, new_caret) = super::slash_apply_text(body, &sc, caret, item("待办"));
        assert_eq!(out, "- [ ] ");
        assert_eq!(new_caret, 6);
    }

    // ── CER-19 专注 / 打字机 ──────────────────────────────────────────

    #[test]
    fn focus_alpha_goes_from_full_to_thirty_five_percent() {
        assert_eq!(super::focus_alpha(0.0), 1.0);
        assert!((super::focus_alpha(1.0) - 0.35).abs() < 1e-6);
        // 中途是线性插值：一半进度 = (1 + 0.35) / 2。
        assert!((super::focus_alpha(0.5) - 0.675).abs() < 1e-6);
        // 越界进度夹住，不会把文字淡没或提亮。
        assert_eq!(super::focus_alpha(-1.0), 1.0);
        assert!((super::focus_alpha(3.0) - 0.35).abs() < 1e-6);
    }

    #[test]
    fn block_lines_expands_to_blank_line_boundaries() {
        let body = "第一段\n还是第一段\n\n第二段\n\n- 项一\n- 项二";
        let lines: Vec<&str> = body.split_inclusive('\n').collect();
        assert_eq!(super::block_lines(&lines, 0), (0, 1));
        assert_eq!(super::block_lines(&lines, 1), (0, 1));
        // 光标在空行上：块就是它自己，不会把上下两段连成一块。
        assert_eq!(super::block_lines(&lines, 2), (2, 2));
        assert_eq!(super::block_lines(&lines, 3), (3, 3));
        // 连续两个列表项算一块（中间没有空行）。
        assert_eq!(super::block_lines(&lines, 5), (5, 6));
        assert_eq!(super::block_lines(&lines, 99), (5, 6));
        assert_eq!(super::block_lines(&[], 0), (0, 0));
    }

    #[test]
    fn dim_only_touches_lines_outside_the_active_block() {
        let src = "# 标题\n\n正文一段。\n\n正文二段。";
        let dimmed = build_job(
            src,
            palette(),
            760.0,
            MarkerMode::ShowAll,
            Some((2, 2, 0.35)),
        );
        let plain = build_job(src, palette(), 760.0, MarkerMode::ShowAll, None);
        assert_eq!(dimmed.text, plain.text);
        let mut saw_full = false;
        let mut saw_dim = false;
        for sec in &dimmed.sections {
            let text = &dimmed.text[sec.byte_range.start.0..sec.byte_range.end.0];
            if text.trim().is_empty() {
                continue;
            }
            if text.starts_with("正文一段") {
                assert_eq!(sec.format.color.a(), 255, "当前块不该被淡：{text:?}");
                saw_full = true;
            } else {
                assert_eq!(sec.format.color.a(), 89, "块外应淡到 35%：{text:?}");
                saw_dim = true;
            }
        }
        assert!(saw_full && saw_dim, "两段都要出现在排版里");
    }

    #[test]
    fn typewriter_target_lands_the_caret_at_forty_two_percent() {
        // 视口 100..700（高 600）⇒ 42% 的屏幕 y = 100 + 252 = 352。
        let (top, h) = (100.0, 600.0);
        let want = top + h * super::TYPEWRITER_AT;
        assert!((want - 352.0).abs() < 0.01, "42% 应落在 352：{want}");
        // 光标在 500、当前没滚 → 目标偏移 148；滚到 148 后光标正好回到 352。
        let t = super::typewriter_target_offset(500.0, 0.0, top, h);
        assert!((t - 148.0).abs() < 0.01, "实测 {t}");
        assert!((500.0 - t - want).abs() < 0.01, "滚完应落在 42%");
        // 同一处内容、已经滚到 40：光标 460，目标仍是 148 —— 与当前偏移无关。
        let t2 = super::typewriter_target_offset(460.0, 40.0, top, h);
        assert!((t2 - t).abs() < 0.01, "目标不该随当前偏移变：{t2} vs {t}");
        // 已经停在目标上：再算一次还是它（不动点），不会来回摆。
        let stable = super::typewriter_target_offset(want, t, top, h);
        assert!((stable - t).abs() < 0.01, "不动点要自洽：{stable}");
    }

    #[test]
    fn focus_demo_keeps_the_caret_inside_the_block_it_leaves_bright() {
        let card = NoteCard::new("doc", None, "标题", "# 标题\n\n正文一段。\n\n正文二段。", 0);
        let mut r = CardReader::new(&card);
        r.arm_focus_demo();
        assert!(r.focus_on);
        assert_eq!(r.focus_t, 1.0);
        let lines: Vec<&str> = r.draft_body.split_inclusive('\n').collect();
        let line = r.active.map(|(l, _)| l).unwrap_or(0);
        assert_eq!(super::block_lines(&lines, line), (0, 0));
    }

    #[test]
    fn typewriter_demo_moves_the_caret_into_the_middle_of_the_body() {
        let body = "一\n\n二\n\n三\n\n四";
        let card = NoteCard::new("doc", None, "标题", body, 0);
        let mut r = CardReader::new(&card);
        r.arm_typewriter_demo();
        assert!(r.typewriter);
        assert_eq!(r.caret_to, Some(body.chars().count() / 2));
    }

    /// 跑一整帧卡片页，返回这一帧的动作（`begin_pass` / `end_pass`，同 `menu.rs` 的做法）。
    fn card_frame(
        ctx: &egui::Context,
        r: &mut CardReader,
        ch: &Chrome,
        mut input: egui::RawInput,
    ) -> Option<ReaderAction> {
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 1000.0),
        ));
        ctx.begin_pass(input);
        let a = r.show(ctx, ch, &mw_core::get_theme("snow"));
        let mut out = ctx.end_pass();
        out.textures_delta.clear();
        a
    }

    fn esc_frame(modifiers: egui::Modifiers) -> egui::RawInput {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
        input
    }

    #[test]
    fn esc_leaves_focus_first_and_the_page_second() {
        let ctx = egui::Context::default();
        let ch = crate::chrome::chrome_of(&mw_core::get_theme("snow"));
        let card = NoteCard::new("doc", None, "标题", "# 一\n\n正文一段。\n\n正文二段。", 0);
        let mut r = CardReader::new(&card);
        r.arm_focus_demo();
        // 第 1 帧：什么都没按，专注必须留着（曾经的 bug：这一分支不看 Esc，每帧自关）。
        let a1 = card_frame(&ctx, &mut r, &ch, egui::RawInput::default());
        assert!(r.focus_on, "没人按 Esc，专注不该自己关掉");
        assert!((r.focus_t - 1.0).abs() < 1e-6);
        assert!(a1.is_none());
        // 第 2 帧：Esc 第一层只退专注，不关页面。
        let a2 = card_frame(&ctx, &mut r, &ch, esc_frame(egui::Modifiers::NONE));
        assert!(!r.focus_on, "Esc 应退出专注");
        // 这一帧里 ramp 先跑（那时专注还开着）、Esc 后置，所以淡出从**下一帧**才开始。
        assert_eq!(r.focus_t, 1.0, "退专注的同一帧不该已经淡了");
        assert!(a2.is_none(), "退专注的那一下不该关页面");
        // 下一帧：淡出按 `stable_dt / 120ms` 一小格一小格往下走，不是跳变。
        let _ = card_frame(&ctx, &mut r, &ch, egui::RawInput::default());
        assert!(
            r.focus_t < 1.0 && r.focus_t > 0.0,
            "淡出该开始但没走完，实测 {}",
            r.focus_t
        );
        // 减少动画：同一件事一步到位，不留半透明。
        r.set_reduce_motion(true);
        let _ = card_frame(&ctx, &mut r, &ch, egui::RawInput::default());
        assert_eq!(r.focus_t, 0.0, "减少动画时淡出必须一步落位");
        // 第 3 帧：再按一次才轮到关页面。
        let a3 = card_frame(&ctx, &mut r, &ch, esc_frame(egui::Modifiers::NONE));
        assert!(
            matches!(a3, Some(ReaderAction::Close)),
            "第二下 Esc 该关页面"
        );
    }
}
