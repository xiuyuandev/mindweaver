//! 搜索条（`Ctrl+F`）。
//!
//! 和命令面板的分工：命令面板找**动作**，搜索条找**主题**。检索本身在
//! `mw_core::Document::search`（文本 + 全拼 + 首字母），这里只管三件事：
//!
//! * **状态**：查询词、结果行、当前行号。面板不碰文档，结果由壳层通过
//!   `lookup` 回调喂进来（`HitRow` 是纯数据）；
//! * **键盘**：`↑↓`/`Enter` 在结果间走，`Shift+Enter` 反向，`Esc` 关闭。
//!   这些键在画布之前消费，否则会被当成「加同级主题」；
//! * **视觉**：输入行 + 结果行 + 快捷键脚注，一个卡片，和菜单/命令面板同一套
//!   圆角、描边与阴影。颜色全部来自 `Tone`：暗色主题下搜索条不能还是白的。

use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2,
};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use mw_core::MatchMode;

use crate::chrome::{Tone, Type};

/// 结果所属入口：节点 / 命题 / 卡片 / 标签 / 路径。列表按这个顺序分组。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Node,
    Relationship,
    Card,
    Tag,
    Path,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Self::Node => "节点",
            Self::Relationship => "命题",
            Self::Card => "卡片",
            Self::Tag => "标签",
            Self::Path => "路径",
        }
    }
}

/// 命题命中的身份：源节点 + 关系线 id（关系线是边不是节点）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelRow {
    pub from: String,
    pub id: String,
}

/// 一条搜索结果。文本与类型由壳层从文档里取，面板不持有 `Document`。
#[derive(Clone, Debug)]
pub struct HitRow {
    pub id: String,
    pub text: String,
    /// `text` / `pinyin` / `initials`
    pub kind: String,
    /// `text` / `note` / `link`：命中的字段。备注/链接命中在右侧多挂一枚角标。
    pub field: String,
    /// 命中的原文**字符**区间 `[start, end)`（备注/链接命中时已折算进摘录坐标系）。
    pub range: Option<[usize; 2]>,
    /// 节点路径（不含自己，空串表示顶层）。第二行小字显示，回答"这条在哪儿"。
    pub path: String,
    pub group: Group,
    /// 命题命中时的身份；节点/卡片/标签/路径行为 None。
    pub rel: Option<RelRow>,
}

/// 搜索条发出的意图。跳转/关闭都由壳层执行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Jump(String),
    /// 选中一条命题并定位（源节点 id + 关系线 id）。
    JumpRelationship {
        from: String,
        id: String,
    },
    /// 跳到卡片阅读页（卡片搜索结果）。
    JumpCard(String),
    Close,
    /// 把文档里所有字面命中的 query 换成 to（一次撤销）。
    ReplaceAll {
        query: String,
        to: String,
    },
    /// 只替换当前这条标题字面命中；文本已在面板里算好。
    ReplaceOne {
        id: String,
        text: String,
    },
}

const WIDTH: f32 = 560.0;
const HEADER_H: f32 = 48.0;
/// 替换行高度（展开时才占位）。
const REPLACE_H: f32 = 40.0;
/// 两行式结果行：标题 + 路径。
const ROW_H: f32 = 46.0;
const FOOTER_H: f32 = 30.0;
const MAX_ROWS: usize = 6;
/// 匹配方式两段开关：宽窗「精确文本」比窄窗「文本」多出的宽度。
const MODE_TEXT_W: f32 = 62.0;
const MODE_TEXT_NARROW_W: f32 = 40.0;
const MODE_PINYIN_W: f32 = 40.0;
const MODE_SEG_H: f32 = 20.0;
/// 开关右侧留给 `Esc` 关闭提示的宽度。
pub(crate) const MODE_RIGHT_RESERVE: f32 = 74.0;
/// 开关左侧至少留给「↑↓ 选择 · Enter 打开」提示的宽度。
pub(crate) const MODE_HINT_MIN: f32 = 64.0;

/// 脚注右侧「精确文本 / 拼音」两段开关的几何（纯函数，M29b）。
///
/// 宽窗 62+40、窄窗 40+40，右让 [`MODE_RIGHT_RESERVE`]；返回 `(整段, 文本段, 拼音段)`。
/// 卡片最窄 240 时左界为 `240−74−80=86`，仍大于 [`MODE_HINT_MIN`]——这条等式由单测钉住，
/// 改宽度常量或右让值先看 `mode_toggle_fits_the_footer_at_every_width`。
pub(crate) fn mode_toggle(footer: Rect, narrow: bool) -> (Rect, Rect, Rect) {
    let text_w = if narrow {
        MODE_TEXT_NARROW_W
    } else {
        MODE_TEXT_W
    };
    let right = footer.right() - MODE_RIGHT_RESERVE;
    // 左界再兜一道：卡片被夹到极窄时，宁可压缩也不压住左侧「↑↓ 选择 · Enter 打开」。
    // 240 下限时计算值 86 已大于 64，这条下限在现行宽度档位上是恒等变换。
    let left = (right - text_w - MODE_PINYIN_W).max(footer.left() + MODE_HINT_MIN);
    let toggle = Rect::from_min_max(
        Pos2::new(left, footer.center().y - MODE_SEG_H * 0.5),
        Pos2::new(right, footer.center().y + MODE_SEG_H * 0.5),
    );
    let text = Rect::from_min_size(toggle.min, Vec2::new(text_w, MODE_SEG_H));
    let pinyin = Rect::from_min_size(
        Pos2::new(toggle.min.x + text_w, toggle.min.y),
        Vec2::new(MODE_PINYIN_W, MODE_SEG_H),
    );
    (toggle, text, pinyin)
}

pub struct SearchBar {
    open: bool,
    query: String,
    rows: Vec<HitRow>,
    current: Option<usize>,
    /// 匹配方式：拼音（默认，原文 + 全拼 + 首字母）或精确文本。
    mode: MatchMode,
    /// 上次检索用的查询词、匹配方式与文档修订号；任一变化就重查。
    last_query: String,
    last_mode: MatchMode,
    last_revision: u64,
    /// 下一帧把焦点交给输入框。
    grab: bool,
    /// 替换行是否展开。
    replace_open: bool,
    /// 替换文本。
    replacement: String,
    /// 下一帧把焦点交给替换框（`Ctrl+H` 展开时）。
    grab_replace: bool,
    /// 逐个替换后要跟着走的行：被替换的 id + 它原来的下标。
    pending_advance: Option<(String, usize)>,
    /// 脚注开关布局的最近一次（卡片宽, 是否窄窗）：只在变化时记轨迹（M29b）。
    footer_log: Option<(i32, bool)>,
}

impl Default for SearchBar {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            rows: Vec::new(),
            current: None,
            mode: MatchMode::default(),
            last_query: String::new(),
            last_mode: MatchMode::default(),
            last_revision: u64::MAX,
            grab: false,
            replace_open: false,
            replacement: String::new(),
            grab_replace: false,
            pending_advance: None,
            footer_log: None,
        }
    }
}

impl SearchBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 打开（已经开着时只讨回输入焦点，不清空查询——`Ctrl+F` 连按不该丢内容）。
    pub fn open(&mut self) {
        if self.open {
            self.grab = true;
            return;
        }
        self.open = true;
        self.query.clear();
        self.rows.clear();
        self.current = None;
        self.mode = MatchMode::default();
        self.last_query.clear();
        self.last_mode = MatchMode::default();
        self.last_revision = u64::MAX;
        self.grab = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.rows.clear();
        self.current = None;
        self.mode = MatchMode::default();
        self.grab = false;
        self.grab_replace = false;
        self.pending_advance = None;
    }

    fn set_mode(&mut self, mode: MatchMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        // 换匹配方式＝换了一次检索：当前行清零，下一帧按新结果重新 `Enter` 落第一条。
        self.current = None;
    }

    fn toggle_mode(&mut self) {
        self.set_mode(self.mode.toggled());
    }

    /// 键盘必须先于画布处理：这些键漏下去就会变成「加同级主题 / 清空选择」。
    pub fn handle_keys(&mut self, ctx: &egui::Context) -> Option<Action> {
        if !self.open {
            return None;
        }
        // 输入法组合中（非空 preedit）时，Esc / Enter / 方向键归输入法：候选窗要靠
        // 方向键选词、Enter 上屏。与节点内编辑同一条纪律（组合期挂起语义键）。
        let composing = ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) if !text.is_empty()
                )
            })
        });
        if composing {
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            return Some(Action::Close);
        }
        // Ctrl+H：展开/收起替换行（与多数编辑器一致）；展开时焦点进替换框，
        // 否则紧接着敲的字会进搜索词——改前就是这样。
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::H)) {
            self.replace_open = !self.replace_open;
            if self.replace_open {
                self.grab_replace = true;
            } else {
                self.grab = true;
            }
            return None;
        }
        // Ctrl+Shift+M：切换「精确文本 / 拼音」。与 Ctrl+H 同一层：搜索条开着时先归它，
        // 输入法组合期在前面已经整体挂起。**为什么不是 Ctrl+P / Ctrl+Shift+P**：全局表里
        // `Ctrl+P` 是「导出 PDF」，而 egui 的逻辑匹配允许额外按住 Shift（`Ctrl+Shift+P`
        // 同样满足 `Ctrl+P` 的模式），`window_shortcuts` 又排在搜索条之前——两种写法都只会
        // 弹出导出框。`M` 没有全局 `Ctrl+M`，`Ctrl+Shift+M` 才真正落得到这里。
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::M)) {
            self.toggle_mode();
            return None;
        }
        if self.replace_open {
            // Ctrl+Shift+Enter 必须排在 Ctrl+Enter 前面：带 Shift 的那条先消费。
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter)) {
                let query = self.query.trim().to_string();
                if !query.is_empty() {
                    return Some(Action::ReplaceAll {
                        query,
                        to: self.replacement.clone(),
                    });
                }
                return None;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)) {
                return self.replace_current();
            }
        }
        // Shift+Enter 必须排在 Enter 前面：带 Shift 的 Enter 单独消费。
        if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Enter)) {
            return self.step(-1).map(|id| self.action_for(&id));
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
        {
            return self.step(1).map(|id| self.action_for(&id));
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            return self.step(-1).map(|id| self.action_for(&id));
        }
        None
    }

    /// 移动当前行并返回要跳转的节点 id。
    ///
    /// 第一次 `step(1)`（`Enter`）落在第一条；之后逐条前进并绕回。
    /// 从 `None` 反向走则落在最后一条——`Shift+Enter` 的直觉。
    fn step(&mut self, delta: i32) -> Option<String> {
        let n = self.rows.len();
        if n == 0 {
            return None;
        }
        let next = match self.current {
            Some(i) => (i as i32 + delta).rem_euclid(n as i32) as usize,
            None if delta < 0 => n - 1,
            None => 0,
        };
        self.current = Some(next);
        Some(self.rows[next].id.clone())
    }

    fn action_for(&self, id: &str) -> Action {
        let Some(row) = self.rows.iter().find(|r| r.id == id) else {
            return Action::Jump(id.to_string());
        };
        match (&row.group, &row.rel) {
            (Group::Relationship, Some(rel)) => Action::JumpRelationship {
                from: rel.from.clone(),
                id: rel.id.clone(),
            },
            (Group::Card, _) => Action::JumpCard(id.to_string()),
            _ => Action::Jump(id.to_string()),
        }
    }

    /// 替换当前行；没有当前行时落在第一条（等价于用户先按了一次 `Enter`，
    /// 但这里只替换、不跳转——跳转由壳层在替换成功后做）。
    fn replace_current(&mut self) -> Option<Action> {
        let i = match self.current {
            Some(i) => i,
            None if !self.rows.is_empty() => 0,
            None => return None,
        };
        let (id, text) = {
            let row = self.rows.get(i)?;
            (row.id.clone(), replace_target(row, &self.replacement)?)
        };
        self.current = Some(i);
        self.pending_advance = Some((id.clone(), i));
        Some(Action::ReplaceOne { id, text })
    }

    /// 结果刷新后给逐个替换收尾：前进到被替换那行的下一行（到底绕回）；
    /// 那行不再命中时按原下标夹取。
    fn settle_after_refresh(&mut self, query_changed: bool) {
        let Some((old_id, at)) = self.pending_advance.take() else {
            return;
        };
        if query_changed || self.rows.is_empty() {
            return;
        }
        self.current = Some(match self.rows.iter().position(|r| r.id == old_id) {
            Some(pos) => (pos + 1) % self.rows.len(),
            None => at.min(self.rows.len() - 1),
        });
    }

    /// 画面板 + 读输入框。`lookup` 只在查询词或文档修订号变化时调用。
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        central: Rect,
        revision: u64,
        tone: Tone,
        lookup: impl FnOnce(&str, MatchMode) -> Vec<HitRow>,
    ) -> Option<Action> {
        if !self.open {
            return None;
        }
        let width = WIDTH.min(central.width() - 24.0).max(240.0);
        let pos = Pos2::new(central.center().x - width * 0.5, central.top() + 12.0);
        let mut action = None;
        let mut close = false;

        egui::Area::new(Id::unique("mw-search-bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                let frame = egui::Frame::new()
                    .fill(tone.card)
                    .stroke(Stroke::new(1.0, tone.stroke))
                    .corner_radius(12)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 10],
                        blur: 30,
                        spread: 0,
                        color: tone.shadow,
                    });
                let shown = frame.show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    // —— 输入行 ——
                    let (header, _) =
                        ui.allocate_exact_size(Vec2::new(width, HEADER_H), Sense::hover());
                    paint_magnifier(
                        ui.painter(),
                        Pos2::new(header.left() + 21.0, header.center().y),
                        tone.muted,
                    );
                    let input = Rect::from_min_max(
                        Pos2::new(header.left() + 38.0, header.center().y - 12.0),
                        Pos2::new(header.right() - 136.0, header.center().y + 12.0),
                    );
                    let edit = egui::TextEdit::singleline(&mut self.query)
                        .id(Id::unique("mw-search-input"))
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::symmetric(0, 4))
                        .font(FontId::proportional(Type::HEADING))
                        .text_color(tone.ink)
                        .hint_text(
                            egui::RichText::new("搜索主题，支持拼音（如 xm）")
                                .size(Type::LABEL)
                                .color(tone.muted),
                        )
                        .desired_width(f32::INFINITY);
                    let edit_resp = ui.put(input, edit);
                    // 语义名给 `MW_TRACE`：这个输入框握着键盘时，Ctrl+Z 归它自己，不归文档。
                    // 验收要能说出「键盘在输入框里」，而不是从「文档没动」倒推。
                    crate::trace::name(ui.ctx(), edit_resp.id, "search-input");
                    if self.grab {
                        edit_resp.request_focus();
                        self.grab = false;
                    }

                    // 查询词或文档修订号变了才重查。文档没动时，敲一个字只查一次；
                    // 同一节点如果还在结果里，当前行跟着它走，而不是跳回第一条。
                    let mode_changed = self.mode != self.last_mode;
                    if self.query != self.last_query
                        || revision != self.last_revision
                        || mode_changed
                    {
                        let query_changed = self.query != self.last_query;
                        let keep = self
                            .current
                            .and_then(|i| self.rows.get(i).map(|r| r.id.clone()));
                        self.last_query.clone_from(&self.query);
                        self.last_mode = self.mode;
                        self.last_revision = revision;
                        if self.query.trim().is_empty() {
                            self.rows.clear();
                        } else {
                            self.rows = lookup(self.query.trim(), self.mode);
                        }
                        self.current =
                            keep.and_then(|id| self.rows.iter().position(|r| r.id == id));
                        // 逐个替换后当前行要跟着走：被替换的行不再命中就落到原下标，
                        // 仍命中（同一节点还有第二处）就前进一行，到底绕回。
                        self.settle_after_refresh(query_changed);
                        if mode_changed {
                            crate::trace::event(&format!(
                                "search_mode mode={} rows={}",
                                self.mode.as_str(),
                                self.rows.len()
                            ));
                        }
                    }

                    let has_query = !self.query.trim().is_empty();
                    if has_query {
                        let text = match self.current {
                            Some(i) if !self.rows.is_empty() => {
                                format!("{}/{}", i + 1, self.rows.len())
                            }
                            _ if self.rows.is_empty() => "无结果".into(),
                            _ => format!("{} 项", self.rows.len()),
                        };
                        ui.painter().text(
                            Pos2::new(header.right() - 130.0, header.center().y),
                            Align2::RIGHT_CENTER,
                            text,
                            FontId::proportional(Type::CAPTION),
                            if self.rows.is_empty() {
                                tone.muted
                            } else {
                                tone.ch.dim
                            },
                        );
                    }

                    let enabled = has_query && !self.rows.is_empty();
                    let close_btn = Rect::from_center_size(
                        Pos2::new(header.right() - 20.0, header.center().y),
                        Vec2::splat(26.0),
                    );
                    let next_btn = Rect::from_center_size(
                        Pos2::new(header.right() - 48.0, header.center().y),
                        Vec2::splat(26.0),
                    );
                    let prev_btn = Rect::from_center_size(
                        Pos2::new(header.right() - 76.0, header.center().y),
                        Vec2::splat(26.0),
                    );
                    if chevron_button(ui, prev_btn, "mw-search-prev", true, enabled, tone) {
                        self.grab = true;
                        if let Some(id) = self.step(-1) {
                            action = Some(self.action_for(&id));
                        }
                    }
                    if chevron_button(ui, next_btn, "mw-search-next", false, enabled, tone) {
                        self.grab = true;
                        if let Some(id) = self.step(1) {
                            action = Some(self.action_for(&id));
                        }
                    }
                    // 悬停时 × 变深、按下去才收起：以前只有点上的那一帧才变深，看不见反馈。
                    let x = icon_hit_opt(ui, close_btn, "mw-search-close", true, tone);
                    paint_close_x(
                        ui.painter(),
                        close_btn.center(),
                        if x.as_ref().is_some_and(|r| r.hovered()) {
                            tone.ch.text
                        } else {
                            tone.muted
                        },
                    );
                    if x.as_ref().is_some_and(|r| r.clicked()) {
                        close = true;
                    }
                    // 「替换」开关：图标按钮，展开后在标题行下方多一行输入。
                    let rep_btn = Rect::from_center_size(
                        Pos2::new(header.right() - 104.0, header.center().y),
                        Vec2::splat(26.0),
                    );
                    let rep = icon_hit_opt(ui, rep_btn, "mw-search-replace-toggle", true, tone);
                    if let Some(r) = rep.as_ref() {
                        crate::trace::name(ui.ctx(), r.id, "search-replace-toggle");
                    }
                    ui.painter().text(
                        rep_btn.center(),
                        Align2::CENTER_CENTER,
                        "⇄",
                        FontId::proportional(Type::BODY),
                        if self.replace_open {
                            tone.ch.accent
                        } else if rep.as_ref().is_some_and(|r| r.hovered()) {
                            tone.ch.text
                        } else {
                            tone.muted
                        },
                    );
                    if rep.as_ref().is_some_and(|r| r.clicked()) {
                        self.replace_open = !self.replace_open;
                        if self.replace_open {
                            self.grab_replace = true;
                        } else {
                            self.grab = true;
                        }
                    }

                    if self.replace_open {
                        // —— 替换行 ——
                        let (row, _) =
                            ui.allocate_exact_size(Vec2::new(width, REPLACE_H), Sense::hover());
                        let field = Rect::from_min_max(
                            Pos2::new(row.left() + 38.0, row.center().y - 12.0),
                            Pos2::new(row.right() - 134.0, row.center().y + 12.0),
                        );
                        let resp = ui.put(
                            field,
                            egui::TextEdit::singleline(&mut self.replacement)
                                .id(Id::unique("mw-search-replace"))
                                .frame(egui::Frame::NONE)
                                .margin(egui::Margin::symmetric(0, 4))
                                .font(FontId::proportional(Type::BODY))
                                .text_color(tone.ink)
                                .hint_text(
                                    egui::RichText::new("替换为…")
                                        .size(Type::SMALL)
                                        .color(tone.muted),
                                )
                                .desired_width(f32::INFINITY),
                        );
                        crate::trace::name(ui.ctx(), resp.id, "search-replace-input");
                        if self.grab_replace {
                            resp.request_focus();
                            self.grab_replace = false;
                        }
                        ui.painter().text(
                            Pos2::new(row.left() + 21.0, row.center().y),
                            Align2::CENTER_CENTER,
                            "⇄",
                            FontId::proportional(Type::BODY),
                            tone.muted,
                        );
                        // 两个按钮：「替换」作用于当前行，「全部替换」作用于整篇。
                        // 快捷键在 `handle_keys`（`Ctrl+Enter` / `Ctrl+Shift+Enter`），
                        // 不再看焦点在哪个输入框——同一个键换焦点就换语义最容易记错。
                        let one_enabled = has_query
                            && self
                                .current
                                .or(if self.rows.is_empty() { None } else { Some(0) })
                                .and_then(|i| self.rows.get(i))
                                .is_some_and(|r| replace_target(r, &self.replacement).is_some());
                        let btn_one = Rect::from_center_size(
                            Pos2::new(row.right() - 100.0, row.center().y),
                            Vec2::new(52.0, 26.0),
                        );
                        let hit_one = ui.interact(
                            btn_one,
                            Id::unique("mw-search-replace-one"),
                            Sense::click(),
                        );
                        crate::trace::name(ui.ctx(), hit_one.id, "search-replace-one");
                        ui.painter().rect_filled(
                            btn_one,
                            7.0,
                            if !one_enabled {
                                tone.hover
                            } else if hit_one.hovered() {
                                tone.current
                            } else {
                                tone.ch.accent_soft
                            },
                        );
                        ui.painter().text(
                            btn_one.center(),
                            Align2::CENTER_CENTER,
                            "替换",
                            FontId::proportional(Type::BODY),
                            if one_enabled {
                                tone.ch.accent
                            } else {
                                tone.ch.dim
                            },
                        );
                        if one_enabled && hit_one.clicked() {
                            if let Some(a) = self.replace_current() {
                                action = Some(a);
                            }
                            self.grab_replace = true;
                        }
                        let btn = Rect::from_center_size(
                            Pos2::new(row.right() - 50.0, row.center().y),
                            Vec2::new(72.0, 26.0),
                        );
                        let hit =
                            ui.interact(btn, Id::unique("mw-search-replace-all"), Sense::click());
                        crate::trace::name(ui.ctx(), hit.id, "search-replace-all");
                        let on = has_query;
                        ui.painter().rect_filled(
                            btn,
                            7.0,
                            if !on {
                                tone.hover
                            } else if hit.hovered() {
                                tone.current
                            } else {
                                tone.ch.accent_soft
                            },
                        );
                        ui.painter().text(
                            btn.center(),
                            Align2::CENTER_CENTER,
                            "全部替换",
                            FontId::proportional(Type::BODY),
                            if on { tone.ch.accent } else { tone.ch.dim },
                        );
                        if on && hit.clicked() {
                            action = Some(Action::ReplaceAll {
                                query: self.query.trim().to_string(),
                                to: self.replacement.clone(),
                            });
                            self.grab_replace = true;
                        }
                    }

                    if has_query {
                        // —— 分隔线 + 结果 ——
                        let (sep, _) =
                            ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
                        ui.painter().hline(
                            sep.x_range().shrink(12.0),
                            sep.center().y,
                            Stroke::new(1.0, tone.hairline),
                        );
                        ui.add_space(5.0);
                        if self.rows.is_empty() {
                            let (empty, _) =
                                ui.allocate_exact_size(Vec2::new(width, 54.0), Sense::hover());
                            ui.painter().text(
                                Pos2::new(empty.left() + 16.0, empty.center().y - 8.0),
                                Align2::LEFT_CENTER,
                                "没有找到匹配的主题",
                                FontId::proportional(Type::BODY),
                                tone.ch.dim,
                            );
                            ui.painter().text(
                                Pos2::new(empty.left() + 16.0, empty.center().y + 10.0),
                                Align2::LEFT_CENTER,
                                match self.mode {
                                    MatchMode::Pinyin => "试试全拼或首字母，如 xiangmu、xm",
                                    MatchMode::Text => {
                                        "精确文本只匹配原文；切到拼音可查全拼与首字母"
                                    }
                                },
                                FontId::proportional(Type::CAPTION),
                                tone.ch.dim,
                            );
                        } else {
                            self.rows_ui(ui, width, tone, &mut action);
                        }
                        ui.add_space(5.0);
                    }

                    // —— 脚注 ——
                    let (footer, _) =
                        ui.allocate_exact_size(Vec2::new(width, FOOTER_H), Sense::hover());
                    ui.painter().hline(
                        footer.x_range(),
                        footer.top(),
                        Stroke::new(1.0, tone.hairline),
                    );
                    // 匹配方式两段开关（`docs/02` §3「精确文本 / 拼音」）：脚注右侧、Esc 左边。
                    // 选中段 8% 强调底 + 强调色文字，与检查器作用域开关同一套语言。
                    // 窄窗只留「文本 / 拼音」两字，别把左侧提示挤没。
                    let narrow = width < 420.0;
                    let (toggle, text_rect, pinyin_rect) = mode_toggle(footer, narrow);
                    // 布局变化才记一笔：窄窗判据要能从轨迹里读，但别每帧刷。
                    let stamp = (width as i32, narrow);
                    if self.footer_log != Some(stamp) {
                        self.footer_log = Some(stamp);
                        crate::trace::event(&format!(
                            "search_footer width={width:.0} narrow={} toggle=[{:.0},{:.0}]",
                            narrow as u8,
                            toggle.left(),
                            toggle.right()
                        ));
                    }
                    ui.painter().rect_filled(toggle, 5.0, tone.hover);
                    for (on, r, label) in [
                        (
                            self.mode == MatchMode::Text,
                            text_rect,
                            if narrow { "文本" } else { "精确文本" },
                        ),
                        (self.mode == MatchMode::Pinyin, pinyin_rect, "拼音"),
                    ] {
                        if on {
                            ui.painter()
                                .rect_filled(r, 5.0, tone.ch.accent.gamma_multiply(0.08));
                        }
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            label,
                            FontId::proportional(Type::CAPTION),
                            if on { tone.ch.accent } else { tone.muted },
                        );
                    }
                    for (id, r, mode) in [
                        ("mw-search-mode-text", text_rect, MatchMode::Text),
                        ("mw-search-mode-pinyin", pinyin_rect, MatchMode::Pinyin),
                    ] {
                        let resp = ui.interact(r, Id::unique(id), Sense::click());
                        crate::trace::name(ui.ctx(), resp.id, id);
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            self.set_mode(mode);
                            self.grab = true;
                        }
                    }

                    let hint = if self.query.trim().is_empty() {
                        match self.mode {
                            MatchMode::Pinyin => "输入中文、全拼或拼音首字母",
                            MatchMode::Text => "输入原文片段，大小写不敏感",
                        }
                    } else if self.replace_open {
                        "Enter 定位 · Ctrl+Enter 替换当前 · Ctrl+Shift+Enter 全部"
                    } else {
                        "↑↓ 切换 · Enter 定位 · Shift+Enter 反向"
                    };
                    // 提示文字裁到开关左边：窄窗时宁可截断，也不压在开关下面。
                    ui.painter()
                        .with_clip_rect(Rect::from_min_max(
                            footer.min,
                            Pos2::new(toggle.left() - 8.0, footer.max.y),
                        ))
                        .text(
                            Pos2::new(footer.left() + 16.0, footer.center().y),
                            Align2::LEFT_CENTER,
                            hint,
                            FontId::proportional(Type::CAPTION),
                            tone.ch.dim,
                        );
                    ui.painter().text(
                        Pos2::new(footer.right() - 14.0, footer.center().y),
                        Align2::RIGHT_CENTER,
                        "Esc 关闭",
                        FontId::proportional(Type::CAPTION),
                        tone.ch.dim,
                    );
                });

                // 点卡片外面收起（XMind 同一行为）；点里面由各控件自己处理。
                let card = shown.response.rect.expand(2.0);
                let outside = ui.ctx().input(|i| {
                    i.pointer.any_pressed()
                        && i.pointer.press_origin().is_some_and(|p| !card.contains(p))
                });
                if outside {
                    close = true;
                }
            });

        if close {
            return Some(Action::Close);
        }
        action
    }

    /// 结果区：最多 6 行；当前行始终落在窗口里（绕回时窗口跟着滑）。
    fn rows_ui(&mut self, ui: &mut egui::Ui, width: f32, tone: Tone, action: &mut Option<Action>) {
        let n = self.rows.len();
        let start = if n <= MAX_ROWS {
            0
        } else {
            let cur = self.current.unwrap_or(0);
            cur.saturating_sub(MAX_ROWS / 2).min(n - MAX_ROWS)
        };
        let end = (start + MAX_ROWS).min(n);
        for i in start..end {
            let (id, text, kind, field, range, path, group, is_rel) = {
                let r = &self.rows[i];
                (
                    r.id.clone(),
                    r.text.clone(),
                    r.kind.clone(),
                    r.field.clone(),
                    r.range,
                    r.path.clone(),
                    r.group,
                    r.rel.is_some(),
                )
            };
            let (row, resp) = ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::click());
            let bg = Rect::from_min_max(
                Pos2::new(row.left() + 7.0, row.top() + 1.0),
                Pos2::new(row.right() - 7.0, row.bottom() - 1.0),
            );
            let chosen = self.current == Some(i);
            if chosen {
                ui.painter().rect_filled(bg, 7.0, tone.current);
            } else if resp.hovered() {
                ui.painter().rect_filled(bg, 7.0, tone.hover);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            // 替换预览只跟当前行：展开替换行时整列都变会像在闪。
            let preview = if self.replace_open && chosen {
                replace_target_fields(group, &field, &kind, &text, range, &self.replacement)
            } else {
                None
            };

            let mut badges: Vec<&'static str> = Vec::new();
            if is_rel {
                badges.push("命题");
            } else if field == mw_core::search::FIELD_NOTE {
                badges.push("备注");
            } else if field == mw_core::search::FIELD_LINK {
                badges.push("链接");
            }
            if let Some(label) = kind_label(&kind) {
                badges.push(label);
            }
            // 一枚角标占 52（文字宽），两枚之间留 8。
            let badge_w = if badges.is_empty() {
                14.0
            } else {
                badges.len() as f32 * 60.0 - 8.0
            };
            let text_x = row.left() + 16.0;
            let text_w = (row.right() - badge_w - text_x).max(40.0);
            let color = if chosen { tone.hit } else { tone.ink };
            // 两行：标题在上、路径或替换预览在下。没有第二行时标题回到垂直居中。
            let title_y = if preview.is_some() || !path.is_empty() {
                row.center().y - 7.0
            } else {
                row.center().y
            };
            paint_hit_text(
                ui,
                Pos2::new(text_x, title_y),
                &text,
                range,
                text_w,
                color,
                tone,
            );
            if let Some(next) = preview.as_ref() {
                let line = format!("→ {next}");
                ui.painter()
                    .with_clip_rect(Rect::from_min_max(
                        Pos2::new(text_x, row.top() + 20.0),
                        Pos2::new(row.right() - 14.0, row.bottom()),
                    ))
                    .text(
                        Pos2::new(text_x, row.center().y + 9.0),
                        Align2::LEFT_CENTER,
                        &line,
                        FontId::proportional(Type::SMALL),
                        tone.ch.accent,
                    );
            } else if !path.is_empty() {
                let path_line = format!("{} · {path}", if is_rel { "概念" } else { group.label() });
                ui.painter()
                    .with_clip_rect(Rect::from_min_max(
                        Pos2::new(text_x, row.top() + 20.0),
                        Pos2::new(row.right() - 14.0, row.bottom()),
                    ))
                    .text(
                        Pos2::new(text_x, row.center().y + 9.0),
                        Align2::LEFT_CENTER,
                        &path_line,
                        FontId::proportional(Type::CAPTION),
                        if chosen { tone.muted } else { tone.ch.dim },
                    );
            }
            let mut badge_x = row.right() - 14.0;
            for label in badges.iter().rev() {
                ui.painter().text(
                    Pos2::new(badge_x, row.center().y),
                    Align2::RIGHT_CENTER,
                    label,
                    FontId::proportional(Type::CAPTION),
                    tone.ch.dim,
                );
                badge_x -= 60.0;
            }

            if resp.clicked() {
                self.current = Some(i);
                self.grab = true;
                *action = Some(self.action_for(&id));
            }
        }
    }
}

/// 这条结果能不能逐个替换：只认「节点标题 + 字面命中 + 有字符区间」。
///
/// 拼音/首字母命中的区间是原文区间，但用户敲的是拼音；备注/链接命中的区间在摘录里，
/// 不是标题。两类都只给跳转，不给替换入口。
fn replace_target(row: &HitRow, replacement: &str) -> Option<String> {
    replace_target_fields(
        row.group,
        &row.field,
        &row.kind,
        &row.text,
        row.range,
        replacement,
    )
}

fn replace_target_fields(
    group: Group,
    field: &str,
    kind: &str,
    text: &str,
    range: Option<[usize; 2]>,
    replacement: &str,
) -> Option<String> {
    if group != Group::Node || field != mw_core::search::FIELD_TEXT || kind != "text" {
        return None;
    }
    mw_core::search::replace_char_range(text, range?, replacement)
}

/// 命中的字符画成暖黄底——这是「为什么这条被搜出来」最直接的答案。
fn paint_hit_text(
    ui: &egui::Ui,
    pos: Pos2,
    text: &str,
    range: Option<[usize; 2]>,
    width: f32,
    color: Color32,
    tone: Tone,
) {
    // 主题里可能有换行；结果行只有一行，先把换行压成空格（字符数不变，区间仍对得上）。
    let display: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let font = FontId::proportional(Type::BODY);
    let plain = TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: width,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        },
        ..Default::default()
    };
    let total = display.chars().count();
    match range.filter(|[s, e]| s < e && *e <= total) {
        Some([s, e]) => {
            let b0 = char_byte(&display, s);
            let b1 = char_byte(&display, e);
            if b0 > 0 {
                job.append(&display[..b0], 0.0, plain.clone());
            }
            job.append(
                &display[b0..b1],
                0.0,
                TextFormat {
                    background: tone.hit_bg,
                    color: tone.hit_ink,
                    ..plain.clone()
                },
            );
            if b1 < display.len() {
                job.append(&display[b1..], 0.0, plain);
            }
        }
        None => job.append(&display, 0.0, plain),
    }
    let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
    ui.painter().galley(
        Pos2::new(pos.x, pos.y - galley.size().y * 0.5),
        galley,
        color,
    );
}

fn char_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

fn kind_label(kind: &str) -> Option<&'static str> {
    match kind {
        "pinyin" => Some("拼音"),
        "initials" => Some("首字母"),
        _ => None,
    }
}

/// 上下箭头按钮；`up = true` 画向上的人字。
fn chevron_button(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &'static str,
    up: bool,
    enabled: bool,
    tone: Tone,
) -> bool {
    let Some(resp) = icon_hit_opt(ui, rect, id, enabled, tone) else {
        paint_chevron(ui.painter(), rect.center(), up, tone.off);
        return false;
    };
    let color = if resp.hovered() {
        tone.ch.text
    } else {
        tone.muted
    };
    paint_chevron(ui.painter(), rect.center(), up, color);
    resp.clicked()
}

/// 人字箭头走统一图标几何：不再各写一套翼宽，任何 DPI 都不会糊。
fn paint_chevron(painter: &egui::Painter, c: Pos2, up: bool, color: Color32) {
    let icon = if up {
        crate::icons::Icon::ChevronUp
    } else {
        crate::icons::Icon::ChevronDown
    };
    crate::icon_paint::paint_icon_sized(painter, c, 14.0, icon, color);
}

/// 图标热区：悬停时铺一层圆底。`enabled = false` 时不响应，交由调用方画禁用态。
fn icon_hit_opt(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &'static str,
    enabled: bool,
    tone: Tone,
) -> Option<egui::Response> {
    if !enabled {
        return None;
    }
    let resp = ui.interact(rect, Id::unique(id), Sense::CLICK);
    if resp.hovered() {
        ui.painter().circle_filled(rect.center(), 11.0, tone.chip);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    Some(resp)
}

fn paint_close_x(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.4, color);
    painter.line_segment(
        [
            Pos2::new(c.x - 4.0, c.y - 4.0),
            Pos2::new(c.x + 4.0, c.y + 4.0),
        ],
        s,
    );
    painter.line_segment(
        [
            Pos2::new(c.x + 4.0, c.y - 4.0),
            Pos2::new(c.x - 4.0, c.y + 4.0),
        ],
        s,
    );
}

fn paint_magnifier(painter: &egui::Painter, c: Pos2, color: Color32) {
    painter.circle_stroke(
        Pos2::new(c.x - 1.2, c.y - 1.2),
        4.3,
        Stroke::new(1.5, color),
    );
    painter.line_segment(
        [
            Pos2::new(c.x + 2.0, c.y + 2.0),
            Pos2::new(c.x + 5.4, c.y + 5.4),
        ],
        Stroke::new(1.7, color),
    );
}

#[cfg(test)]
mod tests {
    use super::{Action, Group, HitRow, RelRow, SearchBar};

    /// 几何按 typography-plan §4.7/§5.2：页头 48、替换行 40、结果行 46、页脚 30。
    #[test]
    fn search_geometry_matches_the_typography_plan() {
        assert_eq!(super::HEADER_H, 48.0);
        assert_eq!(super::REPLACE_H, 40.0);
        assert_eq!(super::ROW_H, 46.0);
        assert_eq!(super::FOOTER_H, 30.0);
    }

    /// M29b：脚注开关在宽/窄两套文案下都必须装得进脚注：
    /// 左侧给提示留 [`super::MODE_HINT_MIN`]、右侧给 `Esc` 留 [`super::MODE_RIGHT_RESERVE`]，
    /// 两段首尾相接、高度 20。卡片最窄 240 是 `.max(240.0)` 的下限。
    #[test]
    fn mode_toggle_fits_the_footer_at_every_width() {
        use eframe::egui::{Pos2, Rect, Vec2};
        for (w, narrow) in [(240.0, true), (326.0, true), (420.0, false), (560.0, false)] {
            let footer = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, super::FOOTER_H));
            let (toggle, text, pinyin) = super::mode_toggle(footer, narrow);
            assert!(
                toggle.left() >= footer.left() + super::MODE_HINT_MIN,
                "w={w} 压住左侧提示：{toggle:?}"
            );
            assert!(
                toggle.right() <= footer.right() - super::MODE_RIGHT_RESERVE + 1e-3,
                "w={w} 压住 Esc：{toggle:?}"
            );
            assert!(footer.contains_rect(toggle), "w={w} 出脚注：{toggle:?}");
            assert_eq!(toggle.height(), super::MODE_SEG_H);
            assert_eq!(text.right(), pinyin.left(), "两段必须首尾相接");
            assert_eq!(pinyin.width(), super::MODE_PINYIN_W);
            assert_eq!(
                text.width(),
                if narrow {
                    super::MODE_TEXT_NARROW_W
                } else {
                    super::MODE_TEXT_W
                }
            );
        }
    }

    fn row(id: &str) -> HitRow {
        HitRow {
            id: id.into(),
            text: id.into(),
            kind: "text".into(),
            field: "text".into(),
            range: None,
            path: "根".into(),
            group: Group::Node,
            rel: None,
        }
    }

    fn bar(ids: &[&str]) -> SearchBar {
        let mut b = SearchBar::new();
        b.open();
        b.rows = ids.iter().map(|id| row(id)).collect();
        b
    }

    #[test]
    fn relationship_results_jump_to_the_edge() {
        let mut b = bar(&["e1"]);
        b.rows[0].group = Group::Relationship;
        b.rows[0].rel = Some(RelRow {
            from: "a".into(),
            id: "e1".into(),
        });
        assert_eq!(
            b.action_for("e1"),
            Action::JumpRelationship {
                from: "a".into(),
                id: "e1".into()
            }
        );
    }

    #[test]
    fn relationship_group_sits_between_nodes_and_cards() {
        let mut groups = vec![
            Group::Path,
            Group::Tag,
            Group::Card,
            Group::Relationship,
            Group::Node,
        ];
        groups.sort();
        assert_eq!(
            groups,
            vec![
                Group::Node,
                Group::Relationship,
                Group::Card,
                Group::Tag,
                Group::Path
            ]
        );
        assert_eq!(Group::Relationship.label(), "命题");
    }

    #[test]
    fn card_results_jump_to_the_reader_and_groups_sort_in_order() {
        let mut b = bar(&["c"]);
        b.rows[0].group = Group::Card;
        assert_eq!(b.action_for("c"), Action::JumpCard("c".into()));
        let mut groups = vec![Group::Path, Group::Tag, Group::Card, Group::Node];
        groups.sort();
        assert_eq!(
            groups,
            vec![Group::Node, Group::Card, Group::Tag, Group::Path]
        );
        assert_eq!(Group::Card.label(), "卡片");
    }

    #[test]
    fn enter_starts_at_first_then_wraps() {
        let mut b = bar(&["a", "b"]);
        assert_eq!(b.step(1).as_deref(), Some("a"));
        assert_eq!(b.step(1).as_deref(), Some("b"));
        assert_eq!(b.step(1).as_deref(), Some("a"), "到底后应绕回第一条");
    }

    #[test]
    fn shift_enter_from_idle_goes_to_last() {
        let mut b = bar(&["a", "b", "c"]);
        assert_eq!(b.step(-1).as_deref(), Some("c"));
        assert_eq!(b.step(-1).as_deref(), Some("b"));
    }

    #[test]
    fn an_empty_result_set_cannot_step() {
        let mut b = bar(&[]);
        assert_eq!(b.step(1), None);
        assert_eq!(b.step(-1), None);
    }

    #[test]
    fn close_clears_state() {
        let mut b = bar(&["a"]);
        b.query = "x".into();
        b.close();
        assert!(!b.is_open());
        assert!(b.query.is_empty());
        assert!(b.rows.is_empty());
        assert_eq!(b.step(1), None);
    }

    #[test]
    fn search_bar_toggles_mode_and_clears_the_current_row() {
        let mut b = bar(&["a"]);
        assert_eq!(b.mode, mw_core::MatchMode::Pinyin, "默认仍是拼音");
        b.current = Some(0);
        b.toggle_mode();
        assert_eq!(b.mode, mw_core::MatchMode::Text);
        assert_eq!(b.current, None, "换匹配方式＝换一次检索，当前行清零");
        b.toggle_mode();
        assert_eq!(b.mode, mw_core::MatchMode::Pinyin);
        // 单次搜索的临时选择：关掉再开回到默认拼音。
        b.toggle_mode();
        b.close();
        b.open();
        assert_eq!(b.mode, mw_core::MatchMode::Pinyin);
    }

    #[test]
    fn reopen_while_open_keeps_the_query_for_refocus() {
        let mut b = bar(&["a"]);
        b.query = "项目".into();
        b.open();
        assert_eq!(b.query, "项目", "Ctrl+F 连按不该清掉正在输入的内容");
    }

    #[test]
    fn actions_compare_by_node_id() {
        assert_eq!(Action::Jump("n1".into()), Action::Jump("n1".to_string()));
        assert_ne!(Action::Jump("n1".into()), Action::Close);
    }

    #[test]
    fn replace_target_only_accepts_title_literal_hits() {
        let mut r = row("n1");
        r.text = "中文排版".into();
        r.range = Some([2, 4]);
        assert_eq!(
            super::replace_target(&r, "排布").as_deref(),
            Some("中文排布")
        );
        // 空替换串 = 删除命中段
        assert_eq!(super::replace_target(&r, "").as_deref(), Some("中文"));

        let mut pinyin = r.clone();
        pinyin.kind = "pinyin".into();
        assert_eq!(super::replace_target(&pinyin, "排布"), None);

        let mut note = r.clone();
        note.field = mw_core::search::FIELD_NOTE.into();
        assert_eq!(super::replace_target(&note, "排布"), None);

        let mut card = r.clone();
        card.group = Group::Card;
        assert_eq!(super::replace_target(&card, "排布"), None);

        let mut no_range = r.clone();
        no_range.range = None;
        assert_eq!(super::replace_target(&no_range, "排布"), None);
    }

    #[test]
    fn replace_current_emits_one_command_and_parks_the_next_row() {
        let mut b = bar(&["n1", "n2"]);
        b.rows[0].text = "中文排版".into();
        b.rows[0].range = Some([2, 4]);
        b.rows[1].text = "排版规范".into();
        b.rows[1].range = Some([0, 2]);
        b.replace_open = true;
        b.replacement = "排布".into();
        b.current = Some(0);

        assert_eq!(
            b.replace_current(),
            Some(Action::ReplaceOne {
                id: "n1".into(),
                text: "中文排布".into()
            })
        );
        assert_eq!(b.pending_advance, Some(("n1".to_string(), 0)));

        // 刷新后 n1 不再命中：当前行按原下标夹到剩下的第一行（n2）。
        b.rows.remove(0);
        b.current = None;
        b.settle_after_refresh(false);
        assert_eq!(b.current, Some(0));
    }

    #[test]
    fn settle_after_refresh_moves_past_the_replaced_row_and_wraps() {
        let mut b = bar(&["n1", "n2"]);
        // 被替换那行仍然命中（同一节点还有第二处）：跳到它后面一行。
        b.pending_advance = Some(("n1".to_string(), 0));
        b.settle_after_refresh(false);
        assert_eq!(b.current, Some(1));

        // 到底绕回第一行，键盘连按时不会卡在末尾。
        b.pending_advance = Some(("n2".to_string(), 1));
        b.settle_after_refresh(false);
        assert_eq!(b.current, Some(0));

        // 行的先后变了：按 id 找到的新位置算“下一行”，不信旧下标。
        b.rows.swap(0, 1);
        b.pending_advance = Some(("n1".to_string(), 0));
        b.settle_after_refresh(false);
        assert_eq!(b.current, Some(0));
    }

    #[test]
    fn replace_current_starts_at_the_first_row_and_clears_on_query_change() {
        let mut b = bar(&["a", "b"]);
        for r in &mut b.rows {
            r.text = "项目 A".into();
            r.range = Some([0, 2]);
        }
        b.replace_open = true;
        b.replacement = "Project".into();
        assert!(b.current.is_none());
        assert_eq!(
            b.replace_current(),
            Some(Action::ReplaceOne {
                id: "a".into(),
                text: "Project A".into()
            })
        );
        assert_eq!(b.current, Some(0));

        // 改查询词＝用户换了一次检索，不再按旧下标前移。
        b.pending_advance = Some(("a".to_string(), 1));
        b.settle_after_refresh(true);
        assert_eq!(b.pending_advance, None);
    }

    #[test]
    fn replace_current_refuses_pinyin_and_note_rows() {
        let mut b = bar(&["a", "b"]);
        b.rows[0].text = "项目".into();
        b.rows[0].range = Some([0, 2]);
        b.rows[0].kind = "initials".into();
        b.rows[1].text = "备注里的项目".into();
        b.rows[1].field = mw_core::search::FIELD_NOTE.into();
        b.rows[1].range = Some([4, 6]);
        b.replace_open = true;
        b.replacement = "Project".into();

        assert_eq!(b.replace_current(), None);
        assert_eq!(b.pending_advance, None);
        b.current = Some(1);
        assert_eq!(b.replace_current(), None);
    }
}
