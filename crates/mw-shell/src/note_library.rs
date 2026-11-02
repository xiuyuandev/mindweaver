//! 笔记库页（CER-22–30 · `docs/28` §7）：三栏自适应的卡片目录 + 阅读预览。
//!
//! 一页三面，各管一件事：侧栏是**导航**（四个范围 + 两级标签树），列表是**目录**
//! （60px 行、虚拟化、按 id 选择），预览是**阅读面**（复用卡片页那一棵 Markdown AST）。
//! 三面只有一份数据口径——计数、行序、预览全部走 [`mw_core::note::CardQuery`]，
//! 视图层不再各自过滤（§7.11「单一查询入口」；旧版「收件箱 4 = 全部 4」就是
//! 侧栏自己数一遍、列表自己滤一遍数出来的）。
//!
//! 颜色只从 [`mw_core::design::content`] 的令牌取，不从壳层 [`Chrome`] 取：
//! `chrome_of` 的亮色分支把 `panel` 写死成纯白、`panel2` 写死成 `#F2F3F5`，
//! 四套主题在这一页会塌成同一张灰白纸。仍留着 `Chrome` 的只有移出确认弹窗——
//! [`crate::dialog`] 只认 `Chrome`，那是全壳层共用的一个门，不在这里另开。
//!
//! 选择一律按卡片 id（§7.11）：排序、筛选、批量操作之后选中集不漂移。

mod list;
mod preview;
mod sidebar;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use eframe::egui::{self, Align2, Color32, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2};
use mw_core::design::content::{self, ContentMetrics, TypeStyle};
use mw_core::note::{
    flatten_tag_rows, scope_counts, tag_dot_index, tag_tree, CardQuery, CardScope, CardSort,
    FlatTag, ScopeCounts,
};
use mw_core::theme::Theme;

use crate::chrome::{Chrome, Tone};
use crate::content_kit;
use crate::icons::Icon;
use crate::menu::{self, Entry, Menu};
use crate::note_store::{CardMeta, NoteStore};

// ---------------------------------------------------------------------------
// 几何（`docs/28` §7.2 / §7.3 / §7.4）
// ---------------------------------------------------------------------------

/// ≥1200 三栏。
pub(crate) const BREAK_THREE: f32 = 1200.0;
/// ≥840 两栏；再窄就单栏。
pub(crate) const BREAK_TWO: f32 = 840.0;
/// 页头 48（§7.2）。
pub(crate) const HEADER_H: f32 = 48.0;
/// 侧栏默认宽（§7.2）。
pub(crate) const SIDEBAR_W: f32 = 232.0;
pub(crate) const SIDEBAR_MIN: f32 = 200.0;
pub(crate) const SIDEBAR_MAX: f32 = 280.0;
/// 列表默认宽（§7.2）。
pub(crate) const LIST_W: f32 = 360.0;
pub(crate) const LIST_MIN: f32 = 320.0;
pub(crate) const LIST_MAX: f32 = 440.0;
/// 三栏时预览的最小宽：不够就先收列表、再收侧栏（§7.2「预览余量 ≥480」）。
pub(crate) const PREVIEW_MIN: f32 = 480.0;
/// 侧栏内边距 8（§7.3）。
pub(crate) const SIDE_PAD: f32 = 8.0;
/// 导航行 30（§7.3）。
pub(crate) const NAV_ROW_H: f32 = 30.0;
/// 标签行 28（§7.3）。
pub(crate) const TAG_ROW_H: f32 = 28.0;
/// 列表行 60（§7.4）。
pub(crate) const CARD_ROW_H: f32 = 60.0;
/// 列表左右内边距 16（§7.4）。
pub(crate) const LIST_PAD: f32 = 16.0;
/// 排序行 28（§7.4）。
pub(crate) const SORT_ROW_H: f32 = 28.0;
/// 筛选 chip 行 28。
pub(crate) const CHIP_ROW_H: f32 = 28.0;
/// 批量条 44（§7.4）。
pub(crate) const BATCH_BAR_H: f32 = 44.0;
/// 行间分隔线左右内缩 16（§7.4）。
pub(crate) const DIVIDER_INSET: f32 = 16.0;
/// 选择模式的 16px 复选框（§7.4）。
pub(crate) const CHECK_BOX: f32 = 16.0;
pub(crate) const CHECK_GAP: f32 = 10.0;
/// 拖宽把手的命中半宽。
pub(crate) const RESIZE_HIT: f32 = 4.0;
/// 选中导航行左侧那条 2px 强调条（§7.3）。
pub(crate) const NAV_BAR_W: f32 = 2.0;
/// 搜索去抖 120ms（§7.6）。
///
/// 结算读 `ctx.input(|i| i.time)` 而不是 `Instant::now()`：金样从 M16 起走
/// 帧计数虚拟时钟，读墙钟会让「打了字但还没到 120ms」这个中间态在离屏里
/// 时而出现时而不出现，基线就变成一颗定时炸弹。
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(120);
pub(crate) const DEBOUNCE_SECS: f64 = 0.12;
/// 列表底部那条反馈停留多久。
pub(crate) const HINT_SECS: f64 = 3.0;
/// 自绘行的双击判定窗口。egui 的 `double_clicked` 在自绘行上不稳（旧版踩过），
/// 自己按时间判；同样读虚拟时钟。
pub(crate) const DOUBLE_CLICK_SECS: f64 = 0.4;
/// 命中高亮 12% 强调底（§7.6）。
pub(crate) const HIT_ALPHA: f32 = 0.12;
pub(crate) const SECTION_TAGS: &str = "标签";
/// 导航四项与「标签」分组之间的空档。§7.3 只给了行高，没给分组间距；
/// 取 12 = 侧栏内距 8 与行高 28 之间的那一档，视觉上刚好一条行的呼吸。
pub(crate) const SECTION_GAP: f32 = 12.0;
pub(crate) const NO_TAG_HINT: &str = "给卡片打标签后会出现在这里";
pub(crate) const EMPTY_PREVIEW: &str = "选择一张卡片查看内容";
pub(crate) const NO_RESULTS: &str = "没有匹配的卡片";
pub(crate) const EMPTY_LIBRARY: &str = "还没有卡片";
pub(crate) const EMPTY_LIBRARY_DESC: &str = "在导图节点上按 Ctrl+Shift+N 新建，或从文件导入";
pub(crate) const SEARCH_PLACEHOLDER: &str = "搜索标题、正文、标签…";

/// 侧栏文字。§7.3 写的 13 是 TYPO-01 重校前的数：重校后 14/21 那一档
/// （`content::NAV`）的文档注释就把「库侧栏」列为消费者，按令牌走。
pub(crate) const SIDE_TEXT: TypeStyle = content::NAV;
/// 计数与分组标题。§7.3 的 11.5/500 落到令牌最小档 12/18/600（`TAG`：
/// 「计数、分组标题、快捷键提示」）——全局字阶没有 11 也没有 500 那一档。
pub(crate) const SIDE_COUNT: TypeStyle = content::TAG;

// ---------------------------------------------------------------------------
// 栏数与焦点区
// ---------------------------------------------------------------------------

/// 当前窗宽落哪一档（§7.2）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Pane {
    /// ≥1200：侧栏 + 列表 + 预览。
    Three,
    /// 840–1199：侧栏 + 列表；预览整页打开。
    Two,
    /// <840：单栏列表；侧栏是抽屉；预览整页打开。
    One,
}

impl Pane {
    pub(crate) fn of(width: f32) -> Self {
        if width >= BREAK_THREE {
            Pane::Three
        } else if width >= BREAK_TWO {
            Pane::Two
        } else {
            Pane::One
        }
    }

    /// 侧栏是否常驻（两栏以上）。单栏时它是抽屉，盖在列表上。
    pub(crate) fn sidebar_docked(self) -> bool {
        self != Pane::One
    }

    /// 预览是否占独立一栏。否则它整页盖住内容区，页头的「返回」回列表。
    pub(crate) fn preview_docked(self) -> bool {
        self == Pane::Three
    }
}

/// `Tab` 循环的焦点区（§7.8：侧栏 → 列表 → 预览）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Zone {
    Sidebar,
    List,
    Preview,
}

impl Zone {
    /// 这一档下按顺序能停的几个区。单栏且抽屉关着时侧栏不可停。
    pub(crate) fn ring(pane: Pane, drawer: bool, preview_open: bool) -> [Option<Zone>; 3] {
        [
            (pane.sidebar_docked() || drawer).then_some(Zone::Sidebar),
            Some(Zone::List),
            (pane.preview_docked() || preview_open).then_some(Zone::Preview),
        ]
    }

    pub(crate) fn next(self, pane: Pane, drawer: bool, preview_open: bool) -> Zone {
        step(self, pane, drawer, preview_open, 1)
    }

    pub(crate) fn prev(self, pane: Pane, drawer: bool, preview_open: bool) -> Zone {
        step(self, pane, drawer, preview_open, -1)
    }
}

fn step(zone: Zone, pane: Pane, drawer: bool, preview_open: bool, dir: i32) -> Zone {
    let ring = Zone::ring(pane, drawer, preview_open);
    let live: Vec<Zone> = ring.into_iter().flatten().collect();
    let Some(at) = live.iter().position(|z| *z == zone) else {
        return live.first().copied().unwrap_or(Zone::List);
    };
    // `rem_euclid`：负方向从 0 绕回末尾，不用额外判分支。
    let n = live.len() as i32;
    live[((at as i32 + dir).rem_euclid(n)) as usize]
}

/// 三栏的矩形。`None` = 这一档没有这一栏。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Cols {
    pub sidebar: Option<Rect>,
    pub list: Option<Rect>,
    pub preview: Option<Rect>,
}

/// 分栏（§7.2）。
///
/// 三栏时预览有 480 的下限：窗宽不够就**先收列表到 320、再收侧栏到 200**，
/// 因为预览是阅读面，压到 480 以下版心（600）就折成两半，比列表窄一点更难读。
/// 收完还不够就把余量全给预览——不再往下切一档，切档由 [`Pane::of`] 单独决定，
/// 两处各判一次会出现「1199 宽却是三栏」这种缝。
pub(crate) fn columns(area: Rect, pane: Pane, sidebar_w: f32, list_w: f32) -> Cols {
    let h = area.height();
    let col = |x: f32, w: f32| Rect::from_min_size(Pos2::new(x, area.top()), Vec2::new(w, h));
    match pane {
        Pane::One => Cols {
            sidebar: None,
            list: Some(col(area.left(), area.width())),
            preview: None,
        },
        Pane::Two => {
            let sw = sidebar_w.clamp(SIDEBAR_MIN, SIDEBAR_MAX).min(area.width());
            Cols {
                sidebar: Some(col(area.left(), sw)),
                list: Some(col(area.left() + sw, (area.width() - sw).max(0.0))),
                preview: None,
            }
        }
        Pane::Three => {
            let mut sw = sidebar_w.clamp(SIDEBAR_MIN, SIDEBAR_MAX).min(area.width());
            let mut lw = list_w
                .clamp(LIST_MIN, LIST_MAX)
                .min((area.width() - sw).max(0.0));
            if area.width() - sw - lw < PREVIEW_MIN {
                lw = LIST_MIN.min((area.width() - sw).max(0.0));
            }
            if area.width() - sw - lw < PREVIEW_MIN {
                sw = SIDEBAR_MIN.min(area.width());
                lw = LIST_MIN.min((area.width() - sw).max(0.0));
            }
            let px = area.left() + sw + lw;
            Cols {
                sidebar: Some(col(area.left(), sw)),
                list: Some(col(area.left() + sw, lw)),
                preview: Some(col(px, (area.right() - px).max(0.0))),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 对外意图
// ---------------------------------------------------------------------------

pub enum LibraryAction {
    /// 关掉库页，回画布。
    Close,
    /// 打开卡片页（编辑）。
    Open(String),
    /// 从本地库移出（不删 `.zmind` 里的内嵌副本）。
    Remove(Vec<String>),
    /// 导出（空 = 当前筛选的全部）。
    Export(Vec<String>),
    /// 从 Markdown 文件夹导入。
    Import,
    /// 归档 / 取消归档（`NoteCard.archived`，见 `docs/32` §14）。
    Archive(Vec<String>),
    Unarchive(Vec<String>),
    /// 批量加标签。
    AddTag(Vec<String>, String),
    /// 把选中卡片挂到当前主选节点（旧 `Archive` 的语义，改名以免和归档撞）。
    Attach(Vec<String>),
    /// 关库、回画布并选中来源节点。
    JumpToNode(String),
    ManageTags,
    /// `Ctrl+Shift+N`：走画布那条「为选中节点建卡」的路（§7.8）。
    NewCard,
    /// 重读索引。§7.1 不许有「刷新」按钮，所以它只出现在「更多」菜单里；
    /// §7.7 错误条上的「重试」也走这一条——两者本来就是同一个动作。
    Reload,
}

/// 卡片来源的显示形态：壳层按当前文档算好，库只负责摆字。
///
/// 库面板拿不到 `Document`（它只认卡片索引），所以「概念图名 · 概念名」这种
/// 要查节点的措辞必须由外面喂进来。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CardSource {
    /// 文档名。空 = 不知道（跨文档），渲染时退化为「另一份文档」。
    pub doc_label: String,
    /// 来源节点 id：§7.6 的搜索范围含「来源节点标题」，检索要按 id 查文字。
    pub node_id: String,
    /// 节点文字（`Node.text`），空白时由壳层补占位。
    pub node_text: String,
    /// 来源是概念图：按钮文案要说「概念」。
    pub concept_map: bool,
}

impl CardSource {
    /// 来源短语 `概念图名 · 概念名`（`docs/30` §5.6）。
    /// 文档名未知时给 `None`——编一个假名字比说「另一份文档」更糟。
    pub fn phrase(&self) -> Option<String> {
        let doc = self.doc_label.trim();
        if doc.is_empty() {
            return None;
        }
        Some(format!("{doc} · {}", self.node_text.trim()))
    }
}

/// 笔记库要的「面板外状态」。打成一包是因为 `show` 的参数已经到 clippy 的上限，
/// 再拆一个就得给每个调用点加一行噪声；以后加参数也只动这里。
pub struct Extras<'a> {
    /// 内容令牌的来源。库页的颜色全从这里派生，不用壳层 `Chrome`。
    pub theme: &'a Theme,
    /// 「挂到节点」时挂到哪个节点（画布主选）。`None` = 菜单项禁用。
    pub attach_target: Option<&'a str>,
    /// 「移出」确认框在栈顶。
    pub remove_prompt_top: bool,
    /// 裸键（Esc / ↑↓ / Enter）归不归笔记库。
    pub keys_owned: bool,
    /// 卡片 id → 来源措辞（跨文档的不在表里）。
    pub sources: &'a HashMap<String, CardSource>,
    /// 会话里记着的两栏宽度（§7.2「宽度写入会话状态」）。
    pub sidebar_w: f32,
    pub list_w: f32,
    /// 索引读盘出过的错数：>0 就在列表顶挂 §7.7 的错误条。
    pub store_errors: usize,
    /// 库不可用 / 索引还没读到：画骨架而不是空态（§7.7）。
    pub loading: bool,
}

/// 预览缓存的完整卡片：列表只有元数据，正文按选中项懒加载。
#[derive(Clone, Debug, PartialEq, Eq)]
struct Preview {
    id: String,
    title: String,
    body: String,
    tags: Vec<String>,
    source_node: Option<String>,
    created_at: i64,
    updated_at: i64,
}

/// 菜单/按钮发回的意图。`Menu<T: Copy>` 要求 `Copy`，所以带 id 的动作
/// 一律读当前选中集，不在枚举里背字符串。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pick {
    ManageTags,
    Import,
    Export,
    Reload,
    Sort(CardSort),
    ArchiveSel,
    UnarchiveSel,
    AttachSel,
    RemoveSel,
    AddTagSel,
    OpenFocused,
    JumpSource,
    NewCard,
}

// ---------------------------------------------------------------------------
// 状态
// ---------------------------------------------------------------------------

pub struct NoteLibrary {
    cards: Vec<CardMeta>,
    scope: CardScope,
    /// 标签 AND（§7.6）。空 = 不按标签筛。
    tag_filter: Vec<String>,
    sort: CardSort,
    /// 折叠的标签路径（默认全展开）。
    collapsed: HashSet<String>,

    /// 当前查询给出的行序：`cards` 的下标。虚拟化按它数行。
    rows: Vec<usize>,
    counts: ScopeCounts,
    tree: Vec<FlatTag>,

    /// 搜索框里的草稿；`active_query` 是去抖后真正生效的那一份。
    draft: String,
    active_query: String,
    typed_at: Option<f64>,
    /// `None` = 没在搜；`Some` = 命中 id → 相关度（§7.6「搜索时按相关度」）。
    hits: Option<HashMap<String, i32>>,

    select_mode: bool,
    selected: HashSet<String>,
    /// 预览跟随的那张卡（§7.11「选择按 id」）。
    focused: Option<String>,
    preview: Option<Preview>,
    preview_scroll: f32,
    /// 键盘移动前的锚点，`Shift+↑↓` 从这里连选到现在。
    anchor: Option<usize>,
    /// 行游标（`rows` 的下标）。滚动与 `↑↓` 都按它走。
    cursor: usize,
    /// 游标上一次是被键盘动的：只有键盘动过才画焦点环（§7.4 / §4.6）。
    cursor_by_key: bool,
    zone: Zone,

    sidebar_w: f32,
    list_w: f32,
    /// 拖宽落手后交给壳层写回会话状态的那一对值（§7.2）。
    /// 只在松手时置一次：每帧比对回写会在一手拖动里写几十次盘。
    width_pending: Option<(f32, f32)>,
    drawer: bool,
    /// 两栏/单栏下预览整页打开（§7.2）。
    preview_page: bool,
    /// 「用户要开整页阅读面」的请求位；每帧与分档合成出 `preview_page`。
    /// 分开存是因为点行时还不知道窗口会停在哪个档：拖窗跨 1200 时
    /// 请求还在，回到两栏阅读面自己回来，不用重新点一次卡片。
    preview_wanted: bool,

    list_scroll: f32,
    list_view_h: f32,
    side_scroll: f32,

    more_menu: Option<Menu<Pick>>,
    sort_menu: Option<Menu<Pick>>,
    row_menu: Option<Menu<Pick>>,
    /// 批量加标签的行内输入（不开模态：`overlay::Modal` 是全壳层共用的栈，
    /// 为一个非危险动作往里加变体不值当，§7.4 也只要求危险动作二次确认）。
    tag_input: Option<String>,
    /// 刚打开、下一帧要抢一次焦点。放在结构体上而不是 `ctx.data`：
    /// egui 的 `IdTypeMap` 只能按类型删，按 id 删不掉，第二次打开就抢不到焦点。
    tag_input_fresh: bool,
    confirm_remove: bool,

    /// 自绘行的 egui 双击标志不稳，自己按时间判（沿用旧版口径）。
    last_click: Option<(String, f64)>,
    /// 短暂显示在列表底部的反馈（toast 会被前景层挡住）。
    hint: Option<(String, f64)>,
    /// `flash` 刚被调用、时间戳还没盖成当前帧。
    hint_dirty: bool,
    /// 这一帧的虚拟时钟（`ctx.input(|i| i.time)`）。双击判定与提示过期都读它。
    clock: f64,
    search_id: Option<Id>,
}

/// 笔记库页的裸键（Esc / ↑↓ / Enter）什么时候归笔记库。
/// 模态弹窗（含移出确认、设置、正在打开）与演示弹窗在前台时都归弹窗。
/// CER-03 起委托 [`crate::content_keys::page_owns_bare_keys`]（同源门）。
pub(crate) fn page_keys_owned(modal_top: bool, dialog_demo: bool) -> bool {
    crate::content_keys::page_owns_bare_keys(crate::content_keys::Foreground::modal(
        modal_top || dialog_demo,
    ))
}

impl NoteLibrary {
    pub fn new(cards: Vec<CardMeta>, sidebar_w: f32, list_w: f32) -> Self {
        let mut lib = Self {
            cards,
            scope: CardScope::default(),
            tag_filter: Vec::new(),
            sort: CardSort::default(),
            collapsed: HashSet::new(),
            rows: Vec::new(),
            counts: ScopeCounts::default(),
            tree: Vec::new(),
            draft: String::new(),
            active_query: String::new(),
            typed_at: None,
            hits: None,
            select_mode: false,
            selected: HashSet::new(),
            focused: None,
            preview: None,
            preview_scroll: 0.0,
            anchor: None,
            cursor: 0,
            cursor_by_key: false,
            zone: Zone::List,
            sidebar_w: sidebar_w.clamp(SIDEBAR_MIN, SIDEBAR_MAX),
            list_w: list_w.clamp(LIST_MIN, LIST_MAX),
            width_pending: None,
            drawer: false,
            preview_page: false,
            preview_wanted: false,
            list_scroll: 0.0,
            list_view_h: 0.0,
            side_scroll: 0.0,
            more_menu: None,
            sort_menu: None,
            row_menu: None,
            tag_input: None,
            tag_input_fresh: false,
            confirm_remove: false,
            last_click: None,
            hint: None,
            hint_dirty: false,
            clock: 0.0,
            search_id: None,
        };
        lib.recompute();
        lib
    }

    /// 拖宽落手后取一次「该写回会话状态的新宽度」（§7.2）。
    pub fn take_width_change(&mut self) -> Option<(f32, f32)> {
        self.width_pending.take()
    }

    /// 列表底部那条短反馈。时间戳由 `show` 每帧按虚拟时钟刷新，
    /// 所以这里只记「哪一条」，不记「什么时候」。
    pub fn flash(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        let at = self.hint.as_ref().map(|(_, t)| *t).unwrap_or(0.0);
        self.hint = Some((msg, at));
        self.hint_dirty = true;
    }

    /// 库变了（导入、批量操作、卡片页写盘）后重新取数。
    /// §7.11：不设刷新按钮，`NoteStore` 一变就重建查询结果；
    /// 列表滚动与选择按 id 保持，不按行下标。
    pub fn sync(&mut self, cards: Vec<CardMeta>) {
        self.cards = cards;
        let ids: HashSet<&str> = self.cards.iter().map(|c| c.id.as_str()).collect();
        self.selected.retain(|id| ids.contains(id.as_str()));
        if self
            .focused
            .as_ref()
            .is_some_and(|id| !ids.contains(id.as_str()))
        {
            self.focused = None;
            self.preview = None;
            self.preview_scroll = 0.0;
        }
        // 正文可能在库外被改过：下一帧按 id 重读，别拿旧快照当真相。
        self.preview = None;
        self.hits = None;
        self.active_query = self.draft.clone();
        self.last_click = None;
        if self.select_mode && self.selected.is_empty() {
            self.select_mode = false;
        }
        self.recompute();
    }

    // -- 证据夹具（`docs/28` §7.10 十场）------------------------------------
    //
    // 这五个 setter 只被 `demo_notes::seed_scene` 调用：金样要在**第一帧**就是
    // 目标状态，靠输入脚本按五六下键既慢又会在中间帧上抖。它们改的都是同一个
    // `recompute()` 入口，所以「预置出来的状态」与「用户点出来的状态」逐像素一致。

    /// 焦点落在第 `at` 行（等价于单击它）。
    pub fn demo_focus(&mut self, at: usize) {
        self.row_clicked(at, Modifiers::NONE);
        self.cursor_by_key = false;
    }

    /// §7.2：两栏/单栏「选中卡片后预览以整页阅读面打开」。
    /// 勾选语义（选择模式 / 修饰键点击）不走这里。
    pub(crate) fn request_preview_page(&mut self) {
        self.preview_wanted = self.focused.is_some();
    }

    /// 行点击的统一入口：勾选语义只动选择集，纯点击才请求整页阅读面。
    pub(crate) fn row_clicked(&mut self, at: usize, mods: Modifiers) {
        self.click_row(at, mods);
        if !self.select_mode && !mods.ctrl && !mods.command && !mods.shift {
            self.request_preview_page();
        }
    }

    fn resolve_preview_page(&mut self, pane: Pane) {
        self.preview_page = self.preview_wanted && !pane.preview_docked();
    }

    /// 预置搜索词：只写**草稿**，让第一帧的去抖结算自己去算命中集。
    ///
    /// 连 `active_query` 一起写会让 `poll_search_frame` 判成「没变」而跳过检索，
    /// 于是搜索框里有字、列表却还是全库——`no-results` 那场就拍不出空态。
    pub fn demo_query(&mut self, q: &str) {
        self.draft = q.to_string();
        self.typed_at = None;
        self.cursor = 0;
        self.list_scroll = 0.0;
    }

    /// 预置标签筛选（单选口径，等价于单击侧栏那一行）。
    pub fn demo_tag_filter(&mut self, path: &str) {
        self.set_tag_filter(path.to_string(), false);
    }

    /// 预置范围（等价于单击侧栏导航行）。
    pub fn demo_scope(&mut self, scope: CardScope) {
        self.set_scope(scope);
    }

    /// 进选择模式并勾上前 `n` 行（等价于 `Ctrl+A` 后再点掉几行）。
    pub fn demo_select(&mut self, n: usize) {
        self.select_mode = true;
        for at in 0..n.min(self.rows.len()) {
            self.select_range(at);
        }
        self.cursor = 0;
    }

    /// 当前生效的查询。侧栏计数、列表行、预览都从这一份取数（§7.11）。
    fn query(&self) -> CardQuery {
        CardQuery {
            scope: self.scope,
            tags: self.tag_filter.clone(),
            sort: self.sort,
        }
    }

    /// 重算计数、标签树与行序。
    ///
    /// 行序是 `cards` 的**下标**而不是克隆出来的卡片：一万张时每帧克隆一遍
    /// 元数据就是 §7.4「10k 首屏 ≤100ms」最容易翻车的地方。
    fn recompute(&mut self) {
        self.counts = scope_counts(&self.cards);
        let nodes = tag_tree(&self.cards);
        let mut tree = Vec::new();
        flatten_tag_rows(&nodes, &self.collapsed, &mut tree);
        self.tree = tree;

        let q = self.query();
        let cards = &self.cards;
        let hits = &self.hits;
        let mut rows: Vec<usize> = cards
            .iter()
            .enumerate()
            .filter(|(_, c)| q.accepts(*c))
            .filter(|(_, c)| hits.as_ref().is_none_or(|h| h.contains_key(&c.id)))
            .map(|(i, _)| i)
            .collect();
        if let Some(h) = hits {
            // §7.6：搜索时相关度优先，同分再落回用户选的排序。
            rows.sort_by(|a, b| {
                let sa = h.get(&cards[*a].id).copied().unwrap_or(0);
                let sb = h.get(&cards[*b].id).copied().unwrap_or(0);
                sb.cmp(&sa)
                    .then_with(|| q.cmp_cards(&cards[*a], &cards[*b]))
            });
        } else {
            rows.sort_by(|a, b| q.cmp_cards(&cards[*a], &cards[*b]));
        }
        self.rows = rows;

        let last = self.rows.len().saturating_sub(1);
        self.cursor = self.cursor.min(last);
        self.anchor = self.anchor.map(|a| a.min(last));
        // 焦点那张卡被筛掉了就把游标挪到它现在的位置；不在结果里就停在原位。
        if let Some(id) = self.focused.clone() {
            if let Some(at) = self.rows.iter().position(|i| self.cards[*i].id == id) {
                self.cursor = at;
            }
        }
    }

    fn focused_card(&self) -> Option<&CardMeta> {
        self.rows.get(self.cursor).map(|i| &self.cards[*i])
    }

    /// 当前范围有多少张（页头计数用）。过滤后的张数在列表顶那一行（§7.6）。
    pub(crate) fn scope_count(&self) -> usize {
        match self.scope {
            CardScope::All => self.counts.all,
            CardScope::Inbox => self.counts.inbox,
            CardScope::Untagged => self.counts.untagged,
            CardScope::Archived => self.counts.archived,
        }
    }

    pub(crate) fn count_for(&self, scope: CardScope) -> usize {
        match scope {
            CardScope::All => self.counts.all,
            CardScope::Inbox => self.counts.inbox,
            CardScope::Untagged => self.counts.untagged,
            CardScope::Archived => self.counts.archived,
        }
    }

    /// 换范围。选择与筛选**保留**：用户在「全部卡片」里筛了 `知识`，
    /// 点到「归档」时把筛选悄悄清掉，比留着更让人找不到北（chip 一直可见可删）。
    fn set_scope(&mut self, scope: CardScope) {
        if self.scope == scope {
            return;
        }
        self.scope = scope;
        self.cursor = 0;
        self.list_scroll = 0.0;
        self.zone = Zone::List;
        crate::trace::event(&format!("library_scope scope={scope:?}"));
        self.recompute();
    }

    fn toggle_collapse(&mut self, path: &str) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.to_string());
        }
        let nodes = tag_tree(&self.cards);
        let mut tree = Vec::new();
        flatten_tag_rows(&nodes, &self.collapsed, &mut tree);
        self.tree = tree;
    }

    /// §7.6：单击侧栏标签是**单选**（换掉整组筛选），`Ctrl+click` 才是往 AND 里加一项。
    fn set_tag_filter(&mut self, path: String, multi: bool) {
        if multi {
            if let Some(at) = self.tag_filter.iter().position(|t| *t == path) {
                self.tag_filter.remove(at);
            } else {
                self.tag_filter.push(path);
            }
        } else {
            let only = self.tag_filter.len() == 1 && self.tag_filter[0] == path;
            self.tag_filter = if only { Vec::new() } else { vec![path] };
        }
        self.tag_filter.sort();
        self.cursor = 0;
        self.list_scroll = 0.0;
        crate::trace::event(&format!(
            "library_tag_filter n={} multi={}",
            self.tag_filter.len(),
            u8::from(multi)
        ));
        self.recompute();
    }

    /// 这一帧是不是双击：自绘行的 `Response::double_clicked` 不稳（旧版踩过），
    /// 自己按虚拟时钟判。同一张卡在 400ms 内的第二次单击才算。
    pub(crate) fn is_double_click(&mut self, id: &str) -> bool {
        let prev = self.last_click.take();
        let again = prev.is_some_and(|(pid, t)| pid == id && self.clock - t <= DOUBLE_CLICK_SECS);
        if !again {
            self.last_click = Some((id.to_string(), self.clock));
        }
        again
    }

    fn has_filters(&self) -> bool {
        !self.tag_filter.is_empty() || !self.active_query.trim().is_empty()
    }

    fn clear_filters(&mut self) {
        self.tag_filter.clear();
        self.draft.clear();
        self.active_query.clear();
        self.typed_at = None;
        self.hits = None;
        self.recompute();
    }

    fn selected_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.selected.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// 导出范围：没选就是当前筛选结果的全部（§7.4 的批量条同样口径）。
    fn export_ids(&self) -> Vec<String> {
        if self.selected.is_empty() {
            self.rows
                .iter()
                .map(|i| self.cards[*i].id.clone())
                .collect()
        } else {
            self.selected_ids()
        }
    }

    /// 移出确认弹窗是否开着。壳层用它参与模态优先级仲裁。
    pub(crate) fn remove_prompt_open(&self) -> bool {
        self.confirm_remove
    }

    fn menu_open(&self) -> bool {
        self.more_menu.as_ref().is_some_and(Menu::is_open)
            || self.sort_menu.as_ref().is_some_and(Menu::is_open)
            || self.row_menu.as_ref().is_some_and(Menu::is_open)
            || self.tag_input.is_some()
    }

    // -- 选择 ---------------------------------------------------------------

    /// 单击一行：选择模式里切换勾选，否则移动焦点（§7.4）。
    fn click_row(&mut self, at: usize, mods: Modifiers) {
        let Some(id) = self.rows.get(at).map(|i| self.cards[*i].id.clone()) else {
            return;
        };
        self.cursor = at;
        self.cursor_by_key = false;
        if mods.shift {
            self.select_range(at);
            return;
        }
        if mods.ctrl || mods.command {
            // `Ctrl+click` 进入选择模式（§7.4），之后每次点击都是切换勾选。
            self.select_mode = true;
            if !self.selected.remove(&id) {
                self.selected.insert(id.clone());
            }
            self.anchor = Some(at);
            self.set_focus(Some(id));
            return;
        }
        if self.select_mode {
            // 选择模式下单击就是切换（§7.4），不退出选择模式。
            if !self.selected.remove(&id) {
                self.selected.insert(id.clone());
            }
            self.anchor = Some(at);
            self.set_focus(Some(id));
            return;
        }
        self.anchor = Some(at);
        self.set_focus(Some(id));
    }

    fn row_id(&self, at: usize) -> Option<String> {
        self.rows.get(at).map(|i| self.cards[*i].id.clone())
    }

    /// `Shift+↑↓` / `Shift+click`：从锚点连选到 `to`（§7.4 / §7.8）。
    fn select_range(&mut self, to: usize) {
        self.select_mode = true;
        let from = self.anchor.unwrap_or(to);
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        for at in lo..=hi {
            if let Some(id) = self.row_id(at) {
                self.selected.insert(id);
            }
        }
        self.cursor = to;
        self.set_focus(self.row_id(to));
    }

    fn select_all_rows(&mut self) {
        self.select_mode = true;
        for i in &self.rows {
            self.selected.insert(self.cards[*i].id.clone());
        }
        crate::trace::event(&format!("library_select_all n={}", self.rows.len()));
    }

    fn exit_select(&mut self) {
        self.select_mode = false;
        self.selected.clear();
        self.anchor = None;
    }

    /// 换焦点卡：预览跟着换，滚动位置回顶（§7.5「切换卡片重置到顶部」）。
    fn set_focus(&mut self, id: Option<String>) {
        if self.focused == id {
            return;
        }
        self.focused = id;
        self.preview = None;
        self.preview_scroll = 0.0;
    }

    /// 键盘移动游标，并把那一行滚进可视区。预览即时跟随（§7.8）。
    fn move_cursor(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let last = (self.rows.len() - 1) as i32;
        let to = ((self.cursor as i32) + delta).clamp(0, last) as usize;
        self.cursor = to;
        self.cursor_by_key = true;
        self.anchor = Some(to);
        self.set_focus(self.row_id(to));
        self.reveal_cursor();
    }

    /// 把游标行滚进可视区（最小位移，不居中——居中会让长列表跳得找不到北）。
    fn reveal_cursor(&mut self) {
        let top = self.cursor as f32 * CARD_ROW_H;
        let bottom = top + CARD_ROW_H;
        let view = self.list_view_h.max(CARD_ROW_H);
        if top < self.list_scroll {
            self.list_scroll = top;
        } else if bottom > self.list_scroll + view {
            self.list_scroll = bottom - view;
        }
    }
}

// ---------------------------------------------------------------------------
// 小工具（纯函数，单测直接钉）
// ---------------------------------------------------------------------------

pub(crate) fn c32(c: mw_core::theme::Rgba) -> Color32 {
    // alpha 必须带上：`hover`/`selected`/`nav_selected`/`hairline`/`divider`
    // 全是 4–12% 的半透明令牌，丢成不透明就会画出实心色块与黑分隔线
    // （2026-11-02 实拍：侧栏选中行整块实青、行分隔线近黑）。
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

/// 标签圆点色：同一个根标签在侧栏/列表/预览三处必须同色（§7.1）。
/// 下标由 `mw_core::note::tag_dot_index` 定，颜色取主题的六支分支色。
pub(crate) fn tag_dot(m: &ContentMetrics, tag: &str) -> Color32 {
    let dots = &m.colors.tag_dots;
    c32(dots[tag_dot_index(tag) % dots.len()])
}

/// 范围对应的图标。
///
/// 刻意不新增 `Icon` 变体：`icons.rs` 的 `ALL` 数组、`trace_name` 与
/// `icon_paint.rs` 的 `icon_prims` 是全壳层共用的四处同步点，为一个库页
/// 加三支图标不值当。语义最近的现成图标够用。
pub(crate) fn scope_icon(scope: CardScope) -> Icon {
    match scope {
        CardScope::All => Icon::Library,
        CardScope::Inbox => Icon::Note,
        CardScope::Untagged => Icon::Tag,
        CardScope::Archived => Icon::Exit,
    }
}

/// 字面命中区间（字符下标，半开）。
///
/// 只做**大小写不敏感的字面**匹配：拼音命中的那些行在原文里根本没有对应的
/// 字面串，硬凑一个高亮区间等于在用户没打的地方画一块颜色。宁可不高亮。
pub(crate) fn match_ranges(text: &str, q: &str) -> Vec<(usize, usize)> {
    let q = q.trim();
    if q.is_empty() || text.is_empty() {
        return Vec::new();
    }
    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = q.chars().collect();
    if needle.len() > hay.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()]
            .iter()
            .zip(&needle)
            .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
        {
            out.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// 单行省略：超出 `w` 就截到能放下的字符数并补 `…`。
///
/// 自绘行没有 egui 的 `TextWrapMode` 可用（我们要把三段文字放进同一行的
/// 不同色/不同字号里），所以省略自己算。用**测量**而不是字符数猜：
/// 中英混排时一个字宽能差三倍。
pub(crate) fn elide(painter: &egui::Painter, style: TypeStyle, text: &str, w: f32) -> String {
    if w <= 0.0 || text.is_empty() {
        return String::new();
    }
    let font = content_kit::font_for(style);
    let full = painter.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE);
    if full.size().x <= w {
        return text.to_owned();
    }
    let dots = painter.layout_no_wrap("…".to_owned(), font.clone(), Color32::WHITE);
    let budget = (w - dots.size().x).max(0.0);
    let mut out = String::new();
    let mut used = 0.0;
    for ch in text.chars() {
        let one = painter.layout_no_wrap(ch.to_string(), font.clone(), Color32::WHITE);
        if used + one.size().x > budget {
            break;
        }
        used += one.size().x;
        out.push(ch);
    }
    out.push('…');
    out
}

/// 在一行里画若干段文字，返回右端 x。段之间按 `gap` 分隔。
pub(crate) struct Seg {
    pub text: String,
    pub style: TypeStyle,
    pub color: Color32,
    /// 这一段底下垫的高亮（搜索命中用 §7.6 的 12% 强调底）。
    pub hit: bool,
}

pub(crate) fn draw_line(
    painter: &egui::Painter,
    mut x: f32,
    y_center: f32,
    gap: f32,
    segs: &[Seg],
    accent: Color32,
    clip: Rect,
) -> f32 {
    for s in segs {
        if s.text.is_empty() {
            continue;
        }
        if x >= clip.right() {
            break;
        }
        let font = content_kit::font_for(s.style);
        // 颜色必须**烘进 galley**：`Painter::galley` 的第三参只是 PLACEHOLDER 的
        // 兜底色，烘成白再指望它改色，整行就会以白字落在浅底上（2026-11-02 实测）。
        let galley = painter.layout_no_wrap(s.text.clone(), font, s.color);
        let w = galley.size().x;
        let h = s.style.line_height;
        if s.hit {
            let r = Rect::from_min_size(Pos2::new(x, y_center - h * 0.5), Vec2::new(w, h));
            painter.rect_filled(r, 0.0, accent);
        }
        // 裁掉超出列宽的部分：egui 的 `galley` 不认 clip_rect。
        // 0.37 的 `with_clip_rect` 返回**一支新画笔**而不是收一个闭包。
        painter
            .with_clip_rect(clip)
            .galley(Pos2::new(x, y_center - h * 0.5), galley, s.color);
        x += w + gap;
    }
    x - gap
}

// ---------------------------------------------------------------------------
// 主绘制
// ---------------------------------------------------------------------------

impl NoteLibrary {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ch: &Chrome,
        mut notes: Option<&mut NoteStore>,
        extras: Extras<'_>,
    ) -> Option<LibraryAction> {
        let Extras {
            theme,
            attach_target,
            remove_prompt_top,
            keys_owned,
            sources,
            sidebar_w,
            list_w,
            store_errors,
            loading,
        } = extras;
        let m = content::content_metrics(theme, ctx.pixels_per_point());

        // 会话里的宽度只在开页那一刻读一次：拖宽时以库内状态为准，
        // 每帧回读会把正在拖的那一手立刻夹回旧值。
        if self.sidebar_w == SIDEBAR_W && self.list_w == LIST_W {
            self.sidebar_w = sidebar_w.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
            self.list_w = list_w.clamp(LIST_MIN, LIST_MAX);
        }

        // 搜索去抖与预览懒加载都必须在绘制**之前**结算，否则这一帧画的还是上一份数据。
        let now = ctx.input(|i| i.time);
        self.clock = now;
        if self.hint_dirty {
            if let Some(h) = self.hint.as_mut() {
                h.1 = now;
            }
            self.hint_dirty = false;
        }
        self.poll_search_frame(ctx, now, &mut notes, sources);

        let screen = ctx.content_rect();
        let pane = Pane::of(screen.width());
        let bare = keys_owned && !self.menu_open();
        let (header, area) = split_header(screen);

        // 菜单先于本页收键（`menu.rs` 的调用顺序约定：`handle_keys` 早、`show` 晚）。
        let mut picked = self.poll_menus(ctx);
        if let Some(a) = self.handle_keys(ctx, bare, pane, &mut picked) {
            return Some(a);
        }
        // 整页阅读面只存在于非三栏（§7.2）：请求位与分档每帧合成一次，
        // 点行时不必知道窗口停在哪个档，拖窗跨 1200 也不用重点卡片。
        self.resolve_preview_page(pane);

        let mut sink: Vec<LibraryAction> = Vec::new();
        let mut repaint_after: Option<Duration> = None;
        let (banner, body_area) = if store_errors > 0 {
            // §7.7「只读/库损坏：页头下方行内条」——32px，不弹模态。
            let bar = Rect::from_min_size(
                Pos2::new(area.left(), area.top()),
                Vec2::new(area.width(), content_kit::BANNER_H),
            );
            let rest =
                Rect::from_min_max(Pos2::new(area.left(), bar.bottom()), area.right_bottom());
            (Some(bar), rest)
        } else {
            (None, area)
        };
        // 内容区高度变了（错误条出现/消失）就重算分栏，否则三栏会盖住那条横幅。
        let cols = columns(body_area, pane, self.sidebar_w, self.list_w);
        let full_preview = !pane.preview_docked() && self.preview_page;

        egui::Area::new(Id::unique("mw-note-library"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            // 全屏页不做淡入：Area 默认会淡入，开场那几帧整页半透明，
            // 底下的画布和菜单栏会透上来（离屏抓帧会被拍到）。
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                ui.set_clip_rect(screen);
                // §7.2：三栏之间只有 1px hairline，没有面板圆角与阴影；
                // 整页铺 `surface.page`，侧栏那一栏另铺 `surface.panel`。
                ui.painter().rect_filled(screen, 0.0, c32(m.colors.page));

                let hint_now = self
                    .hint
                    .as_ref()
                    .and_then(|(t, at)| (now - *at < HINT_SECS).then(|| t.clone()));
                if hint_now.is_some() {
                    repaint_after = Some(Duration::from_millis(120));
                }

                paint_header(ui, &m, header, self, pane, full_preview, &mut sink);
                if let Some(bar) = banner {
                    paint_store_banner(ui, &m, bar, store_errors, &mut sink);
                }
                paint_body(
                    ui,
                    &m,
                    body_area,
                    cols,
                    pane,
                    self,
                    sources,
                    hint_now.as_deref(),
                    loading,
                    full_preview,
                    &mut sink,
                );
            });

        // 拖宽把手（要在 Area 外面登记，否则命中区被列的 clip 吃掉）。
        self.poll_resize(ctx, cols, pane);

        self.poll_menus_show(ctx, theme, &mut picked);
        if let Some(a) = self.remove_dialog(ctx, ch, remove_prompt_top) {
            return Some(a);
        }
        if let Some(d) = repaint_after {
            ctx.request_repaint_after(d);
        }
        if let Some(a) = sink.into_iter().next() {
            return Some(a);
        }
        picked
            .into_iter()
            .next()
            .and_then(|p| self.apply_pick(p, attach_target))
    }

    /// 去抖结算 + 预览懒加载。拆出来是因为 `show` 已经很长，而这两件事
    /// 都要在绘制**之前**发生（否则这一帧画的还是上一份数据）。
    fn poll_search_frame(
        &mut self,
        ctx: &egui::Context,
        now: f64,
        notes: &mut Option<&mut NoteStore>,
        sources: &HashMap<String, CardSource>,
    ) {
        if self.typed_at.is_some() {
            ctx.request_repaint_after(DEBOUNCE);
        }
        let draft = self.draft.trim().to_string();
        let settled = self.typed_at.is_none_or(|t| now - t >= DEBOUNCE_SECS);
        if !settled {
            return;
        }
        let changed = draft != self.active_query;
        if !changed {
            // 预览按 id 懒加载：焦点换了或库刚 sync 过（`preview = None`）才读盘。
            if self.preview.is_none() {
                if let Some(id) = self.focused.clone() {
                    if let Some(store) = notes.as_deref() {
                        if let Some(card) = store.get(&id) {
                            self.preview = Some(Preview {
                                id: card.id.clone(),
                                title: card.display_title().to_string(),
                                body: card.body.clone(),
                                tags: card.tags.clone(),
                                source_node: card.source_node.clone(),
                                created_at: card.created_at,
                                updated_at: card.updated_at,
                            });
                        }
                    }
                }
            }
            return;
        }
        self.active_query = draft.clone();
        if draft.is_empty() {
            self.hits = None;
        } else {
            // §7.6 的搜索范围含「来源节点标题」，而节点文字只有壳层喂得进来。
            let titles: HashMap<&str, &str> = sources
                .values()
                .filter(|s| !s.node_id.is_empty())
                .map(|s| (s.node_id.as_str(), s.node_text.as_str()))
                .collect();
            let hits = match notes.as_deref_mut() {
                Some(store) => store.library_hits(&draft, mw_core::MatchMode::Pinyin, |id| {
                    titles.get(id).copied()
                }),
                None => HashMap::new(),
            };
            crate::trace::event(&format!(
                "library_search q={draft:?} hits={} mode=pinyin",
                hits.len()
            ));
            self.hits = Some(hits);
        }
        self.cursor = 0;
        self.list_scroll = 0.0;
        self.recompute();
    }

    fn apply_pick(&mut self, p: Pick, attach_target: Option<&str>) -> Option<LibraryAction> {
        match p {
            Pick::ManageTags => Some(LibraryAction::ManageTags),
            Pick::Import => Some(LibraryAction::Import),
            Pick::Export => Some(LibraryAction::Export(self.export_ids())),
            Pick::Reload => {
                // §7.1：不设「刷新」按钮，手动重载只在「更多」菜单里；
                // §7.7 错误条上的「重试」发的是同一条动作。
                crate::trace::event("library_reload");
                Some(LibraryAction::Reload)
            }
            Pick::NewCard => Some(LibraryAction::NewCard),
            Pick::Sort(s) => {
                self.sort = s;
                crate::trace::event(&format!("library_sort sort={:?}", s));
                self.recompute();
                None
            }
            Pick::OpenFocused => self.focused.clone().map(LibraryAction::Open),
            Pick::JumpSource => self.jump_source(),
            Pick::ArchiveSel => Some(LibraryAction::Archive(self.take_sel())),
            Pick::UnarchiveSel => Some(LibraryAction::Unarchive(self.take_sel())),
            Pick::AttachSel => {
                if attach_target.is_none() {
                    self.flash("先在画布上选中一个主题");
                    return None;
                }
                Some(LibraryAction::Attach(self.take_sel()))
            }
            Pick::RemoveSel => {
                if self.selected.is_empty() {
                    return None;
                }
                self.confirm_remove = true;
                crate::trace::event(&format!("library_remove_prompt n={}", self.selected.len()));
                None
            }
            Pick::AddTagSel => {
                self.tag_input = Some(String::new());
                self.tag_input_fresh = true;
                None
            }
        }
    }

    /// 批量动作发出去之后清掉选择：一次批量只形成一次确认与一条结果提示（§7.4）。
    fn take_sel(&mut self) -> Vec<String> {
        let ids = self.selected_ids();
        self.exit_select();
        self.recompute();
        ids
    }

    /// 「跳到来源节点」是否可用。读**元数据**而不是预览缓存：预览是懒加载的，
    /// 右键那一刻可能还没读盘，按预览判会让这一项时隐时现。
    fn jump_source(&self) -> Option<LibraryAction> {
        let node = self.focused_card()?.source_node.clone()?;
        Some(LibraryAction::JumpToNode(node))
    }
}

/// 页头与内容区的拆分（§7.2：页头 48）。
pub(crate) fn split_header(screen: Rect) -> (Rect, Rect) {
    let h = HEADER_H.min(screen.height());
    let header = Rect::from_min_size(screen.min, Vec2::new(screen.width(), h));
    let area = Rect::from_min_max(
        Pos2::new(screen.left(), header.bottom()),
        screen.right_bottom(),
    )
    .intersect(screen);
    (header, area)
}

// ---------------------------------------------------------------------------
// 键盘、菜单、拖宽、弹窗
// ---------------------------------------------------------------------------

impl NoteLibrary {
    /// 菜单先于本页收键。返回菜单给出的意图（`show` 末尾统一落地）。
    fn poll_menus(&mut self, ctx: &egui::Context) -> Vec<Pick> {
        let mut out = Vec::new();
        // 三个槽各写一段而不是塞进数组：`[&mut a, &mut b]` 这种数组字面量
        // 在结构体字段上会被借用检查当成整体借走 `*self`。
        if let Some(menu) = self.more_menu.as_mut() {
            if let Some(p) = menu.handle_keys(ctx) {
                out.push(p);
            }
            if !menu.is_open() {
                self.more_menu = None;
            }
        }
        if let Some(menu) = self.sort_menu.as_mut() {
            if let Some(p) = menu.handle_keys(ctx) {
                out.push(p);
            }
            if !menu.is_open() {
                self.sort_menu = None;
            }
        }
        if let Some(menu) = self.row_menu.as_mut() {
            if let Some(p) = menu.handle_keys(ctx) {
                out.push(p);
            }
            if !menu.is_open() {
                self.row_menu = None;
            }
        }
        out
    }

    fn poll_menus_show(&mut self, ctx: &egui::Context, theme: &Theme, picked: &mut Vec<Pick>) {
        let tone = Tone::of(theme);
        if let Some(menu) = self.more_menu.as_mut() {
            if let Some(p) = menu.show(ctx, tone) {
                picked.push(p);
            }
            if !menu.is_open() {
                self.more_menu = None;
            }
        }
        if let Some(menu) = self.sort_menu.as_mut() {
            if let Some(p) = menu.show(ctx, tone) {
                picked.push(p);
            }
            if !menu.is_open() {
                self.sort_menu = None;
            }
        }
        if let Some(menu) = self.row_menu.as_mut() {
            if let Some(p) = menu.show(ctx, tone) {
                picked.push(p);
            }
            if !menu.is_open() {
                self.row_menu = None;
            }
        }
    }

    /// §7.8 那张键表。`bare` 已经过 `page_keys_owned`（模态在前台时全让）。
    fn handle_keys(
        &mut self,
        ctx: &egui::Context,
        bare: bool,
        pane: Pane,
        picked: &mut Vec<Pick>,
    ) -> Option<LibraryAction> {
        if !bare {
            return None;
        }
        // 加标签那个行内输入框开着时，键盘全归它：Enter 提交、Esc 取消。
        // 不开模态是因为 `overlay::Modal` 是全壳层共用的栈，为一个非危险动作
        // 往里加变体不值当（§7.4 只要求危险动作二次确认）。
        if self.tag_input.is_some() {
            return self.handle_tag_input(ctx);
        }

        // 有文字框在焦点上时，单字母与空格归输入（§7.8「搜索框内不拦截输入」）。
        let typing = ctx.memory(|mem| mem.focused().is_some());
        // §7.8「IME 组合态不触发 Enter 打开卡片」：`page_owns_bare_keys` 刻意
        // 不看 IME（组合中候选窗归输入法，但页面其余裸键归属不变），所以这里单独挡。
        let ime = crate::content_keys::ime_composing_now(ctx);

        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::N)) {
            return Some(LibraryAction::NewCard);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F)) {
            self.focus_search(ctx);
            return None;
        }
        if !typing && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::A)) {
            self.select_all_rows();
            return None;
        }

        // Esc 阶梯：清空搜索 → 退出选择 → 返回列表 → 关库（§7.8）。
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if !self.draft.is_empty() {
                self.draft.clear();
                self.typed_at = None;
                crate::trace::event("library_esc step=clear_search");
                return None;
            }
            if typing {
                if let Some(id) = self.search_id {
                    ctx.memory_mut(|mem| mem.surrender_focus(id));
                }
                crate::trace::event("library_esc step=blur_search");
                return None;
            }
            if self.select_mode {
                self.exit_select();
                crate::trace::event("library_esc step=exit_select");
                return None;
            }
            if self.preview_page {
                self.preview_wanted = false;
                self.zone = Zone::List;
                crate::trace::event("library_esc step=back_to_list");
                return None;
            }
            if self.drawer {
                self.drawer = false;
                crate::trace::event("library_esc step=close_drawer");
                return None;
            }
            crate::trace::event("library_esc step=close");
            return Some(LibraryAction::Close);
        }

        let shift = ctx.input(|i| i.modifiers.shift);
        let up = ctx.input_mut(|i| {
            i.consume_key(Modifiers::SHIFT, Key::ArrowUp)
                || i.consume_key(Modifiers::NONE, Key::ArrowUp)
        });
        let down = ctx.input_mut(|i| {
            i.consume_key(Modifiers::SHIFT, Key::ArrowDown)
                || i.consume_key(Modifiers::NONE, Key::ArrowDown)
        });
        if up || down {
            let delta = if up { -1 } else { 1 };
            if shift {
                let last = self.rows.len().saturating_sub(1);
                let to = ((self.cursor as i32) + delta).clamp(0, last as i32) as usize;
                self.select_range(to);
                self.cursor_by_key = true;
                self.reveal_cursor();
            } else {
                self.move_cursor(delta);
            }
            return None;
        }
        if !typing {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::J)) {
                self.move_cursor(1);
                return None;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::K)) {
                self.move_cursor(-1);
                return None;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
                if self.select_mode {
                    if let Some(at) = Some(self.cursor) {
                        self.click_row(at, Modifiers::NONE);
                    }
                } else if !pane.preview_docked() {
                    // 非选择模式下 Space = 预览当前（§7.8）；三栏本来就跟着预览，
                    // 两栏/单栏则是把整页阅读面打开。
                    self.preview_wanted = self.focused.is_some();
                    self.zone = Zone::Preview;
                }
                return None;
            }
        }
        if !ime && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            if let Some(id) = self.focused.clone() {
                crate::trace::event(&format!("library_open id={id} via=enter"));
                return Some(LibraryAction::Open(id));
            }
        }

        // Tab 循环焦点区（§7.8）。文字框在焦点上时把它交出去，否则输入法的
        // 「Tab 选候选」与焦点交接会撞在一起。
        if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab)) {
            self.cycle_zone(ctx, pane, -1);
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
            self.cycle_zone(ctx, pane, 1);
            return None;
        }
        let _ = picked;
        None
    }

    fn cycle_zone(&mut self, ctx: &egui::Context, pane: Pane, dir: i32) {
        if let Some(id) = self.search_id {
            ctx.memory_mut(|mem| mem.surrender_focus(id));
        }
        self.zone = if dir > 0 {
            self.zone.next(pane, self.drawer, self.preview_page)
        } else {
            self.zone.prev(pane, self.drawer, self.preview_page)
        };
        crate::trace::event(&format!("library_zone zone={:?}", self.zone));
    }

    fn focus_search(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.search_id {
            ctx.memory_mut(|mem| mem.request_focus(id));
        }
        self.zone = Zone::List;
        crate::trace::event("library_search_focus");
    }

    /// 加标签输入框的键盘：Enter 提交、Esc 取消，其余归输入。
    fn handle_tag_input(&mut self, ctx: &egui::Context) -> Option<LibraryAction> {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.tag_input = None;
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            return self.commit_tag_input();
        }
        None
    }

    fn commit_tag_input(&mut self) -> Option<LibraryAction> {
        let raw = self.tag_input.take().unwrap_or_default();
        let tag = raw.trim().to_string();
        if tag.is_empty() {
            self.flash("标签不能为空");
            return None;
        }
        let ids = self.take_sel();
        if ids.is_empty() {
            return None;
        }
        crate::trace::event(&format!("library_add_tag n={} tag={tag:?}", ids.len()));
        Some(LibraryAction::AddTag(ids, tag))
    }

    /// 拖宽把手（§7.2「侧栏可拖宽 200–280；列表可拖宽 320–440」）。
    ///
    /// 两个把手都在**两栏之间那条 1px 线上**：侧栏右缘拖侧栏宽，列表右缘拖列表宽。
    /// 1px 的线抓不住，所以命中区左右各放宽 4px。拖动改的是库内状态，落手后由壳层
    /// 每帧比对一次写回会话（不变就不落盘）。
    fn poll_resize(&mut self, ctx: &egui::Context, cols: Cols, pane: Pane) {
        if pane == Pane::One {
            return;
        }
        let (Some(sidebar), Some(list)) = (cols.sidebar, cols.list) else {
            return;
        };
        let key = Id::unique("mw-library-resize");
        let pos = ctx.input(|i| i.pointer.interact_pos());
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        let released = ctx.input(|i| i.pointer.any_released());
        let drag = ctx.data(|d| d.get_temp::<(u8, f32, f32)>(key));

        if let Some((which, w0, x0)) = drag {
            if let Some(p) = pos {
                let w = w0 + (p.x - x0);
                if which == 0 {
                    self.sidebar_w = w.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
                } else {
                    self.list_w = w.clamp(LIST_MIN, LIST_MAX);
                }
            }
            ctx.set_cursor_icon(egui::CursorIcon::ResizeColumn);
            if released || pos.is_none() {
                ctx.data_mut(|d| d.remove_by_type::<(u8, f32, f32)>());
                self.width_pending = Some((self.sidebar_w, self.list_w));
                crate::trace::event(&format!(
                    "library_resize sidebar={:.0} list={:.0}",
                    self.sidebar_w, self.list_w
                ));
            }
            return;
        }

        let Some(p) = pos else { return };
        let on_sidebar = near(sidebar.right(), p.x) && in_y(sidebar, p.y);
        let on_list = near(list.right(), p.x) && in_y(list, p.y) && cols.preview.is_some();
        if !on_sidebar && !on_list {
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::ResizeColumn);
        if pressed {
            let which = u8::from(on_list);
            let w0 = if on_list { self.list_w } else { self.sidebar_w };
            ctx.data_mut(|d| d.insert_temp(key, (which, w0, p.x)));
        }
    }

    /// 移出确认（§7.4「危险动作二次确认」）。只在它是栈顶模态时绘制，
    /// 否则设置/历史压在上面时它会透过来抢焦点。
    fn remove_dialog(
        &mut self,
        ctx: &egui::Context,
        ch: &Chrome,
        remove_prompt_top: bool,
    ) -> Option<LibraryAction> {
        if !self.confirm_remove {
            return None;
        }
        if !remove_prompt_top {
            return None;
        }
        let n = self.selected.len();
        let actions = [
            crate::dialog::DialogAction::new("移出", crate::ui_kit::ButtonTone::Danger),
            crate::dialog::DialogAction::new("取消", crate::ui_kit::ButtonTone::Secondary),
        ];
        let spec = crate::dialog::DialogSpec::new("移出本地库？")
            .width(420.0)
            .default_action(1);
        let out = crate::dialog::show(ctx, *ch, "library-remove", spec, &actions, |ui| {
            ui.label(format!("将从本地库移除 {n} 张卡片。"));
            ui.label(
                egui::RichText::new("不会删除 .zmind 文件里的内嵌副本。")
                    .size(13.0)
                    .color(ch.dim),
            );
        });
        if out.action == Some(0) {
            self.confirm_remove = false;
            let ids = self.selected_ids();
            self.exit_select();
            crate::trace::event(&format!("library_remove n={}", ids.len()));
            return Some(LibraryAction::Remove(ids));
        }
        if out.closed {
            self.confirm_remove = false;
        }
        None
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= RESIZE_HIT
}

fn in_y(rect: Rect, y: f32) -> bool {
    rect.y_range().contains(y)
}

// ---------------------------------------------------------------------------
// 页头
// ---------------------------------------------------------------------------

/// 28×28 图标按钮（`docs/28` §5.1），手摆矩形版：页头是绝对定位的一行，
/// 走 `content_kit::page_icon_button` 就得先摆一套 `Layout`，反而更容易漂。
pub(crate) fn icon_at(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    icon: Icon,
    tip: &str,
    selected: bool,
) -> bool {
    let resp = ui
        .interact(rect, Id::unique(("mw-lib-icon", tip)), Sense::click())
        .on_hover_text(tip);
    let p = ui.painter();
    let radius = content::radius::ROW;
    if selected {
        p.rect_filled(rect, radius, c32(m.colors.selected));
    }
    if resp.hovered() {
        p.rect_filled(rect, radius, c32(m.colors.hover));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let color = if resp.hovered() {
        c32(m.colors.text_primary)
    } else {
        c32(m.colors.text_secondary)
    };
    crate::icon_paint::paint_icon_sized(p, rect.center(), content_kit::PAGE_ICON, icon, color);
    resp.clicked()
}

/// 页头 48（§7.2）：返回、标题 18/28/600、计数 12 辅助色，右侧「更多」。
/// 不放刷新、不放关闭 X、不放大文字按钮——那三样正是 §7.9 点名的。
fn paint_header(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    lib: &mut NoteLibrary,
    pane: Pane,
    full_preview: bool,
    sink: &mut Vec<LibraryAction>,
) {
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(m.hairline, c32(m.colors.divider)),
    );
    let mut x = rect.left() + LIST_PAD - 4.0;
    let btn = |x: f32| {
        Rect::from_min_size(
            Pos2::new(x, rect.center().y - content_kit::PAGE_ICON_BUTTON * 0.5),
            Vec2::splat(content_kit::PAGE_ICON_BUTTON),
        )
    };

    if pane == Pane::One && !full_preview {
        if icon_at(ui, m, btn(x), Icon::Panel, "范围与标签", lib.drawer) {
            lib.drawer = !lib.drawer;
        }
        x += content_kit::PAGE_ICON_BUTTON + 4.0;
    }
    let back_tip = if full_preview {
        "返回列表"
    } else {
        "返回画布"
    };
    if icon_at(ui, m, btn(x), Icon::Back, back_tip, false) {
        if full_preview {
            lib.preview_wanted = false;
            lib.zone = Zone::List;
        } else {
            sink.push(LibraryAction::Close);
        }
    }
    x += content_kit::PAGE_ICON_BUTTON + 8.0;

    let title_right = rect.right() - LIST_PAD - content_kit::PAGE_ICON_BUTTON - 12.0;
    // 页头标题恒为「笔记库」。整页预览时卡片名已经由预览面自己的 30px 标题
    // 画出来了（§7.5），页头再抄一遍就是同一行字出现两次。
    let title = "笔记库";
    let budget = (title_right - x - 72.0).max(60.0);
    let shown = elide(ui.painter(), content::HEADING_2, title, budget);
    let tw = text_w(ui, content::HEADING_2, &shown);
    ui.painter().text(
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        shown,
        content_kit::font_for(content::HEADING_2),
        c32(m.colors.text_primary),
    );
    x += tw + 10.0;

    if !full_preview {
        // §7.2 页头计数 = 当前范围有多少张；§7.6 列表顶那个 = 过滤后剩多少张。
        // 两个数不一样时正好说明「筛选收窄了多少」。
        let n = lib.scope_count();
        let label = format!("{n} 张");
        ui.painter().text(
            Pos2::new(x, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            content_kit::font_for(content::META),
            c32(m.colors.text_secondary),
        );
    }

    let more = btn(rect.right() - LIST_PAD - content_kit::PAGE_ICON_BUTTON + 4.0);
    if icon_at(ui, m, more, Icon::More, "更多", lib.more_menu.is_some()) {
        lib.more_menu = Some(Menu::new(
            Pos2::new(more.right() - menu::ROOT_W, more.bottom() + 4.0),
            more_entries(lib),
        ));
    }
}

fn text_w(ui: &egui::Ui, style: TypeStyle, s: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(s.to_owned(), content_kit::font_for(style), Color32::WHITE)
        .size()
        .x
}

/// 「更多」菜单（§7.1：手动重载放这里；§7.2：管理标签/导入/导出放这里）。
fn more_entries(lib: &NoteLibrary) -> Vec<Entry<Pick>> {
    let has = !lib.cards.is_empty();
    vec![
        Entry::action_with_icon("管理标签…", "", Icon::Tag, Pick::ManageTags, true),
        Entry::separator(),
        Entry::action_with_icon("导入…", "", Icon::Open, Pick::Import, true),
        Entry::action_with_icon("导出…", "", Icon::Export, Pick::Export, has),
        Entry::separator(),
        Entry::action_with_icon("重新载入", "", Icon::Fit, Pick::Reload, true),
        Entry::action_with_icon(
            "新建卡片笔记",
            "Ctrl+Shift+N",
            Icon::Note,
            Pick::NewCard,
            true,
        ),
    ]
}

/// §7.7「只读/库损坏：页头下方行内条」，32px、不弹模态、右侧一个「重试」。
fn paint_store_banner(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    errors: usize,
    sink: &mut Vec<LibraryAction>,
) {
    let msg = format!("{errors} 张卡片读不出来，列表里暂时看不到它们");
    // 横幅要按 `rect` 的宽度摆按钮，直接用 Area 的 `ui` 会量到整屏宽。
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("mw-library-banner")
            .max_rect(rect),
    );
    child.spacing_mut().item_spacing = Vec2::ZERO;
    let responses = content_kit::inline_banner(&mut child, m, &msg, &["重试"]);
    if responses.first().is_some_and(|r| r.clicked()) {
        sink.push(LibraryAction::Reload);
    }
}

/// 自绘滚动条（`content::scrollbar` 那套 6px 轨道 / 4px 滑块）。
///
/// **不做淡入淡出**：`scroll_bar::Fade` 读墙钟，而金样从 M16 起按帧计数走
/// 虚拟时钟——一条会随真实时间变浓的滚动条等于给每张基线埋一颗定时炸弹。
/// 常驻一个固定浓度，看得见、也不漂。返回拖动请求的新偏移。
pub(crate) fn scrollbar(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    viewport: Rect,
    content_h: f32,
    offset: f32,
    id: Id,
) -> Option<f32> {
    let spec = crate::scroll_bar::Spec::default();
    let bar = crate::scroll_bar::layout(viewport, content_h, offset, spec)?;
    let fill = c32a(m.colors.text_primary, 0.28);
    let out = crate::scroll_bar::paint(ui, bar, spec, fill, id, content_h, viewport.height());
    out.request
}

/// 带 alpha 的令牌转 egui 颜色（悬停幕、命中高亮这类要透底的）。
pub(crate) fn c32a(c: mw_core::theme::Rgba, alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], a)
}

/// 内容区三面（§7.2）。
///
/// 整页预览**盖住**侧栏与列表，不是挤在旁边：两栏模式要是把阅读面塞进
/// 1080-232-360=488 的窄条里，600 的版心就折成两半，正是 §7.2 点名不许的
/// 「挤成 300px 窄栏」。返回靠页头那颗「返回」，不占预览自己的高度。
#[allow(clippy::too_many_arguments)]
fn paint_body(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    area: Rect,
    cols: Cols,
    pane: Pane,
    lib: &mut NoteLibrary,
    sources: &HashMap<String, CardSource>,
    hint: Option<&str>,
    loading: bool,
    full_preview: bool,
    sink: &mut Vec<LibraryAction>,
) {
    let hair = Stroke::new(m.hairline, c32(m.colors.hairline));
    if full_preview {
        preview::paint_preview(ui, m, area, lib, sources, sink);
        return;
    }
    if let Some(rect) = cols.sidebar {
        sidebar::paint_sidebar(ui, m, rect, lib);
        ui.painter().vline(rect.right(), rect.y_range(), hair);
    }
    if let Some(rect) = cols.list {
        list::paint_list(ui, m, rect, lib, hint, loading, sink);
        if cols.preview.is_some() {
            ui.painter().vline(rect.right(), rect.y_range(), hair);
        }
    }
    if let Some(rect) = cols.preview {
        preview::paint_preview(ui, m, rect, lib, sources, sink);
        return;
    }
    // 单栏抽屉（§7.2「侧栏为抽屉」）：盖在列表上，右边留一条能点掉它的幕。
    if pane == Pane::One && lib.drawer {
        if let Some(rect) = cols.list {
            let w = lib.sidebar_w.min(area.width() * 0.85);
            let dr = Rect::from_min_size(area.min, Vec2::new(w, area.height()));
            ui.painter()
                .rect_filled(rect, 0.0, c32a(m.colors.text_primary, 0.20));
            if ui
                .interact(rect, Id::unique("mw-library-scrim"), Sense::click())
                .clicked()
            {
                lib.drawer = false;
            }
            sidebar::paint_sidebar(ui, m, dr, lib);
            ui.painter().vline(dr.right(), dr.y_range(), hair);
        }
    }
}
