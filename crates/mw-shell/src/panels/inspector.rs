//! 检查器面板（M3-c-b，右，`F9`）。
//!
//! 与大纲的分工：大纲回答"文档长什么样"，检查器回答"选中的东西是什么、能改成什么"。
//! 三条纪律：
//! 1. **只读会话、只发命令**：改样式的入口是 `SetStyle/SetNote/SetLink/SetIcon/AddLabel`，
//!    撤销栈因此不会断；
//! 2. **缓存只在 `doc.revision` 或选择集变化时重建**：5k 文档里每帧调 `resolve_all`
//!    是不可接受的，选中又通常只有一两个节点；
//! 3. **不复制整份文档**：缓存只存选中项的字段（id/文本/覆盖/解析后样式），
//!    和 `Outline` 的行缓存同一套思路。
//!
//! 多选时控件显示"混合"态：改动按每个节点**各自的覆盖**合并后批量下发，
//! 不会把甲节点的字号覆盖带到乙节点身上。

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2,
};
use mw_core::design;
use mw_core::icons::ICONS;
use mw_core::model::{LayoutSettings, StyleOverride, TaskState};
use mw_core::theme::{get_theme, resolve_all, Appearance, ResolvedStyle, Rgba, Shape, Theme};
use mw_core::{Command, Document};

use crate::chrome::{chrome_of, Chrome, Glyph, Type};

use super::focus::{self, FocusKey, FocusMemory, PanelId};

/// 样式页四排色板的稳定身份前缀；每个色点的键是「前缀 + 序号」。
const ROW_FILL: &str = "inspector/style/fill/swatch";
const ROW_BRANCH: &str = "inspector/style/branch/swatch";
const ROW_TEXT: &str = "inspector/style/text/swatch";
const ROW_STROKE: &str = "inspector/style/stroke/swatch";

/// 面板内的一次性配色：`Chrome` 加两支"选中态"派生色。
///
/// 亮色下选中要把强调色**压暗**才压得住白底，暗色下正相反——原来写死的
/// `gamma_multiply(0.55)` 到墨夜上就是一团看不清的黑，所以按外观分流。
#[derive(Clone, Copy)]
pub(crate) struct Ink {
    ch: Chrome,
    /// 选中态的文字色。
    on: Color32,
    /// 选中态的描边。
    ring: Color32,
    /// 色块内圈的分隔线：亮底用黑、暗底用白。
    well: Color32,
}

impl Ink {
    fn of(theme: &Theme) -> Self {
        let ch = chrome_of(theme);
        let dark = matches!(theme.appearance, Appearance::Dark);
        let a = ch.accent;
        Self {
            ch,
            on: a.gamma_multiply(if dark { 1.35 } else { 0.55 }),
            ring: a.gamma_multiply(if dark { 1.12 } else { 0.6 }),
            well: if dark {
                Color32::from_white_alpha(mw_core::design::shell::inspector::RING_WHITE_ALPHA)
            } else {
                Color32::from_black_alpha(mw_core::design::shell::inspector::RING_BLACK_ALPHA)
            },
        }
    }
}

/// 检查器默认宽（`docs/30` §4.1：288，原 420/260 都太偏），以及可拖区间。
pub const PANEL_W: f32 = 288.0;
pub const PANEL_W_MIN: f32 = 240.0;
pub const PANEL_W_MAX: f32 = 360.0;

/// 面板宽夹取：窗口太窄时宁可让画布挤一点，也不把面板压到读不出字。
pub fn clamp_width(w: f32) -> f32 {
    w.clamp(PANEL_W_MIN, PANEL_W_MAX)
}

/// 组内左右内距（§4.1「组内：上下 8、左右 12」）。面板自己的行比大纲紧 4px。
pub const GUTTER: f32 = 12.0;

/// §4.1/§4.2 的行高与字阶。写死成常量是为了既能被绘制代码用，也能被单测钉住。
mod geo {
    /// 头行：「检查器」13/19 + 关闭按钮 24。
    pub const HEAD_H: f32 = 40.0;
    /// 对象行：标题 14/20 + 副行 12/18。
    pub const OBJ_H: f32 = 44.0;
    /// 分段控件（样式/内容/标记）：高 30、底部强调条 1.5。
    pub const SEG_H: f32 = 30.0;
    pub const SEG_BAR: f32 = 1.5;
    /// 分组标题行：chevron 12 + 名称 + 摘要。
    pub const GROUP_H: f32 = 28.0;
    /// 组间分隔带的宽度（§4.1「组间 8px hairline」）：这是那条线的**带宽**，不是行距。
    pub const GROUP_GAP: f32 = 8.0;
    /// 组内行间距（§4.1「行间距 6」）。与 [`GROUP_GAP`] 是两件事：
    /// 一个是组与组之间那条线占多宽，一个是同一组里相邻两行之间空多少。
    pub const ROW_GAP: f32 = 6.0;
    pub const CLOSE_BTN: f32 = 24.0;

    pub const HEAD_TEXT: f32 = 13.0;
    pub const OBJ_TITLE: f32 = 14.0;
    pub const OBJ_META: f32 = 12.0;
    pub const SEG_TEXT: f32 = 13.0;
    /// 组内控件行高（§4.1「组内控件行高 28」）：标签行、开关行都按这一条排。
    pub const ROW_H: f32 = 28.0;
    /// 开关左标签、标签行文字：13/19（§4.1 / §4.2）。
    pub const ROW_TEXT: f32 = 13.0;
    pub const GROUP_TEXT: f32 = 13.0;
    pub const GROUP_SUMMARY: f32 = 12.0;
    /// 分组标题右侧的作用域开关（§4.2「作用域」）：两段文字，12/18，选中 8% 底 + 强调色文字。
    pub const SCOPE_W: f32 = 96.0;
    pub const SCOPE_H: f32 = 20.0;
    /// 面板内的错误条（§4.3「命令失败：面板内 28px 错误条 + toast」）。
    pub const ERROR_H: f32 = 28.0;
}

// 验收轨迹（`MW_TRACE`）要报的几行高度：布局里才知道的数，测试与证据文件都读这一份。
pub use geo::{GROUP_H, HEAD_H, OBJ_H, SEG_BAR, SEG_H};

/// 面板重开、且目标键与其语义近邻都不在时的兜底锚点。
///
/// 取「分支线行的第一格」而不是面板首项：它既是样式页最常用的一档，也不会踩到
/// 5.3.9「不得把焦点重置到第一项」那条禁止项。
pub fn anchor() -> FocusKey {
    FocusKey::indexed(ROW_BRANCH, 0)
}

/// 强调色色条的取样点：每 30° 一格，绕一圈。取整圈而不是几条"品牌色"，
/// 因为覆盖只借色相——用户心里想的是"我要蓝一点"，不是一个具体的 hex。
const ACCENT_HUES: [f32; 12] = [
    0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0, 210.0, 240.0, 270.0, 300.0, 330.0,
];

/// 色相环上的最短距离（0–180），用来挑出「离当前生效色相最近的那一格」。
fn hue_gap(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Style,
    Note,
    Mark,
    /// 概念图专属（`docs/33` §6）：概念图里只走这一条路，
    /// 思维导图那三页（样式/内容/标记）带着树语义，不进概念图。
    Concept,
}

/// 面板发起的动作。壳层负责落地——**面板自己从不改文档**。
///
/// `Debug` 是给测试用的：断言「这一帧发了哪条命令」时要有能读的失败信息
/// （`panic!("…{other:?}")`）。变体里只有 `Command` / `String` / `usize` / `bool`，都能派生。
#[derive(Debug)]
pub enum Action {
    Apply(Command),
    OpenLink(String),
    Close,
    /// 把当前「基础主题 + 微调 + 强调色」存成命名预设。
    SavePreset,
    /// 套用第 i 条预设（壳层合成一条 Batch）。
    ApplyPreset(usize),
    /// 删除第 i 条预设。
    DeletePreset(usize),
    /// 取消画布上的注解选择。
    ClearAnnotation,
    /// 给这个节点新建一张卡片笔记（壳层构造 NoteCard + 命令）。
    NewCardForNode(String),
    /// 打开卡片阅读页。
    OpenCard(String),
    /// 解除这个节点的卡片链接（卡片保留为孤儿）。
    DetachCard(String),
    /// 把一张已有卡片关联到这个节点（`SetNodeCard`）。卡片保留原正文与标签。
    LinkCard {
        node: String,
        card: String,
    },
    /// 选中一个节点（关系线面板里跳转）。
    SelectNode(String),
    /// 从列表里选中一条关系线。
    SelectRelationship {
        from: String,
        id: String,
    },
    /// 取消关系线选择。
    ClearRelationship,
    /// 把键盘焦点还给画布（检查器里 Esc）。
    FocusCanvas,
    /// 选一张图片挂到主选节点。
    PickImage,
    /// 移除主选节点的图片。
    ClearImage,
    /// 折叠/展开一个分组（`docs/30` §4.1：折叠状态记忆到设置）。
    ToggleGroup {
        key: String,
        open: bool,
    },
    /// 概念图整理全部布局（从检查器直通画布）。
    ArrangeConcepts,
    /// 概念图整理选中概念布局。
    ArrangeSelectedConcepts,
}

/// 检查器要显示的「面板外状态」：预设列表与画布上选中的注解。
///
/// 打成一包是因为 `show` 的参数已经到 7 个上限，再拆两个就要吃 clippy 的
/// `too_many_arguments`——顺带也让调用点更容易读。
#[derive(Clone, Copy)]
pub struct Extras<'a> {
    pub presets: &'a [crate::theme_presets::Preset],
    pub annotation: Option<&'a crate::canvas::AnnotationSel>,
    /// 全部选中的注解（末位主选）；多选时批量改颜色/删除。
    pub annotations: &'a [crate::canvas::AnnotationSel],
    /// 当前选中的关系线；优先于注解/节点显示。
    pub relationship: Option<&'a crate::canvas::RelationshipSel>,
    /// 全部选中的关系线（CM-17）；多于一条时检查器切「批量命题」页。
    pub relationships: &'a [crate::canvas::RelationshipSel],
    /// 用户记过的分组开合（`window_state.inspector_groups`，每条 `键=0/1`）。
    pub groups: &'a [String],
    /// 与 `selected` 同序的节点盒（世界坐标）。只给圆角 stepper 算「盒高能容纳的上限」——
    /// 渲染层把半径再钳到 `min(w,h)/2`，控件不想谎报一个拖了没变化的 24（V1-02 G-4）。
    pub boxes: &'a [Option<Rect>],
}

/// 概要在父节点下的跨度：每个直接子节点一格 (id, 是否命中, 显示用的两字标签)。
fn ann_span(
    doc: &Document,
    parent_id: &str,
    kind: mw_core::model::AnnotationKind,
    ann_id: &str,
) -> Vec<(String, bool, String)> {
    let Some(parent) = doc.node(parent_id) else {
        return Vec::new();
    };
    let targets = match kind {
        mw_core::model::AnnotationKind::Summary => parent
            .summaries
            .iter()
            .find(|s| s.id == ann_id)
            .map(|s| s.targets.clone()),
        mw_core::model::AnnotationKind::Boundary => parent
            .boundaries
            .iter()
            .find(|b| b.id == ann_id)
            .map(|b| b.targets.clone()),
    }
    .unwrap_or_default();
    parent
        .children
        .iter()
        .map(|c| {
            let text = doc
                .node(c)
                .map(|n| n.text.trim().chars().take(2).collect::<String>())
                .unwrap_or_default();
            let label = if text.is_empty() {
                "·".to_string()
            } else {
                text
            };
            (c.clone(), targets.contains(c), label)
        })
        .collect()
}

/// 概要文字的行内编辑状态。
struct SummaryEdit {
    parent: String,
    id: String,
    draft: String,
    before: String,
    just_started: bool,
}

struct ItemView {
    id: String,
    text: String,
    /// 节点自己的覆盖（合并新改动时保留其它字段）。
    style: StyleOverride,
    /// 级联后的最终样式（控件显示的是这个，不是"空覆盖"）。
    resolved: ResolvedStyle,
    note: Option<String>,
    link: Option<String>,
    icon: Option<String>,
    labels: Vec<String>,
    task: Option<TaskState>,
    image: Option<String>,
    /// 自己 + 全部后代（含各自当前覆盖），供「应用到子树」使用。
    subtree: Vec<(String, StyleOverride)>,
}

pub struct Inspector {
    tab: Tab,
    /// 用户记过的分组开合（每帧从 `Extras.groups` 抄一份，`Vec` 很短）。
    groups: Vec<String>,
    /// 样式改动是否连同子树一起应用（层级样式）。
    style_subtree: bool,
    /// 强调色作用域：false = 整图，true = 只改选中（写到 branchColor）。
    accent_selected: bool,
    /// 关系线标签草稿。
    rel_id: Option<String>,
    rel_draft: String,
    rel_before: String,
    /// 关系线注释草稿（CM-16）。与标签分开存：标签提交靠 Enter，注释是多行、只靠失焦。
    rel_note_draft: String,
    rel_note_before: String,
    /// 缓存对应的文档修订号 + 选择指纹。
    rev: u64,
    sel_key: u64,
    items: Vec<ItemView>,
    note_id: Option<String>,
    note_draft: String,
    note_before: String,
    /// 大纲点备注点后置位：下一帧切到备注页并把焦点交给输入框。
    note_focus_pending: bool,
    /// 备注框「展开」：长文超过 8 行时用户点了展开（`docs/30` §4.2）。
    note_expanded: bool,
    /// Esc 的两级（`docs/30` §4.4）：第一下松掉控件焦点并置位，第二下才把键盘还给画布。
    esc_latch: bool,
    /// 面板内的错误条（`docs/30` §4.3）：命令失败时贴一条 28px 的说明，同时外面还有 toast。
    /// 不按墙钟过期（金样要稳定）——下一次成功、关面板或点掉它时清空。
    error: Option<String>,
    /// 哪一格数值正在被键入（`docs/30` §4.4「每个可改属性都有键盘路径」）。
    /// 同一时刻只允许一格：数值格的编辑是「一个手势=一条撤销」，两格同时开就成了两个手势。
    stepper_edit: Option<StepperEdit>,
    /// 验收入口（`MW_STEPPER_EDIT`）：金样拍不出「正在输入」的样子——真实路径要先点一下，
    /// 而离屏 harness 没有脚本时每帧清指针。这里记一个「每帧补齐」的种子，`show()` 见到就摆上。
    stepper_seed: Option<(String, String)>,
    link_id: Option<String>,
    link_draft: String,
    link_before: String,
    label_for: Option<String>,
    label_draft: String,
    /// 「关联已有」候选列表正开在哪个节点上（`None` = 收起）。
    card_pick: Option<String>,
    /// 正在行内改写的概要（parent / id / 草稿 / 原值）。
    summary_edit: Option<SummaryEdit>,
    /// 文档属性的草稿：滑杆拖动期间不逐帧发命令，松手才落一条（撤销栈才干净）。
    layout_draft: LayoutSettings,
    /// 「聚焦问题」的输入草稿（CM-10 ⑥）。落盘走 `SetFocusQuestion`，
    /// 与 `layout_draft` 分开：`SetLayout` 是整块替换，用它改问题会把撤销文案说成「更换布局」。
    /// 概念图检查器（`docs/33` §6）的草稿。
    concept_draft: String,
    concept_before: String,
    fq_draft: String,
    fq_before: String,
    /// 聚焦问题草稿（CM-10 ⑥）。
    focus_draft: String,
    layout_dirty: bool,
    /// 强调色取色器的草稿，同上：拖动色板期间只改本地，松手才发 `SetThemeAccent`。
    accent_draft: Option<Color32>,
    accent_dirty: bool,
    /// 色条：这套主题把强调色放在各个色相上**实际会长成什么样**（覆盖只借色相）。
    accent_chips: Vec<Rgba>,
    /// 当前生效的强调色（可能已被覆盖旋转过），色条的选中态按它挑最近的一格。
    accent: Rgba,
    /// 本帧配色：跟随主题，与壳层面板底色同源。
    ink: Ink,
    /// 验收轨迹只报一次几何（`MW_TRACE`），别让每条轨迹都长几百行。
    pub(crate) traced: bool,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            tab: Tab::Style,
            groups: Vec::new(),
            style_subtree: false,
            accent_selected: false,
            rel_id: None,
            rel_draft: String::new(),
            rel_note_draft: String::new(),
            rel_note_before: String::new(),
            rel_before: String::new(),
            rev: u64::MAX,
            sel_key: 0,
            items: Vec::new(),
            note_id: None,
            note_draft: String::new(),
            note_before: String::new(),
            note_focus_pending: false,
            note_expanded: false,
            esc_latch: false,
            error: None,
            stepper_edit: None,
            stepper_seed: None,
            link_id: None,
            link_draft: String::new(),
            link_before: String::new(),
            label_for: None,
            label_draft: String::new(),
            card_pick: None,
            summary_edit: None,
            layout_draft: LayoutSettings::default(),
            focus_draft: String::new(),
            layout_dirty: false,
            // 概念图检查器（`docs/33` §6）的三份草稿。与 `focus_draft` 分开存：
            // 那一份归思维导图那一套，两套同时开着会互相踩。
            concept_draft: String::new(),
            concept_before: String::new(),
            fq_draft: String::new(),
            fq_before: String::new(),
            accent_draft: None,
            accent_dirty: false,
            accent_chips: Vec::new(),
            accent: get_theme("snow").ui.accent,
            ink: Ink::of(&get_theme("snow")),
            traced: false,
        }
    }
}

mod inspector_methods_1;
mod inspector_methods_2;
mod inspector_methods_3;

// ───────────────────────── 小工具 ─────────────────────────

/// 自己 + 全部后代（先序），带各自当前覆盖。
fn subtree_styles(doc: &Document, root: &str) -> Vec<(String, StyleOverride)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_string()];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.node(&id) else { continue };
        out.push((id, n.style.clone().unwrap_or_default()));
        for c in n.children.iter().rev() {
            stack.push(c.clone());
        }
    }
    out
}

/// 选择集指纹：顺序敏感（末尾是主选）。
fn selection_key(ids: &[String]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for id in ids {
        for b in id.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h ^= 0xff;
        h = h.wrapping_mul(1099511628211);
    }
    h ^ ids.len() as u64
}

fn batch(cmds: Vec<Command>) -> Vec<Command> {
    if cmds.len() > 1 {
        vec![Command::Batch { commands: cmds }]
    } else {
        cmds
    }
}

fn wrap(cmds: Vec<Command>) -> Option<Action> {
    let mut cmds = batch(cmds);
    cmds.pop().map(Action::Apply)
}

fn same<T: PartialEq>(mut it: impl Iterator<Item = T>) -> Option<T> {
    let first = it.next()?;
    if it.all(|v| v == first) {
        Some(first)
    } else {
        None
    }
}

fn color32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

/// 一行对比度：数值 + WCAG 等级。只读，不自动改主题。
fn contrast_row(ink: Ink, ui: &mut egui::Ui, label: &str, fg: Rgba, bg: Rgba) {
    let ratio = fg.contrast_ratio(bg);
    let (grade, color) = if ratio >= 7.0 {
        ("AAA", ink.ch.text)
    } else if ratio >= 4.5 {
        ("AA", ink.ch.text)
    } else {
        (
            "偏低",
            crate::chrome::c32(mw_core::design::shell::inspector::WARN),
        )
    };
    ui.horizontal(|ui| {
        ui.add_space(crate::panels::PAD);
        ui.label(
            egui::RichText::new(label)
                .size(Type::SMALL)
                .color(ink.ch.dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(crate::panels::PAD);
            ui.label(egui::RichText::new(grade).size(Type::CAPTION).color(color));
            ui.label(
                egui::RichText::new(format!("{ratio:.2}:1"))
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
        });
    });
}

/// 正文小标题与提示文字：统一 14px 左缩进，跟页签、文档属性对齐。
fn body_label(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(crate::panels::PAD);
        ui.label(egui::RichText::new(text).size(size).color(color));
    });
}

/// 输入框底板：白面板上纯白输入框会"消失"，用极浅灰 + 细边兜住边界。
fn text_box_frame(ink: Ink) -> egui::Frame {
    egui::Frame::NONE
        .fill(ink.ch.faded(ink.ch.panel2, 0.5))
        .stroke(Stroke::new(1.0, ink.ch.border_hot))
        .corner_radius(6)
}

fn info_row(ink: Ink, ui: &mut egui::Ui, label: &str, value: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let p = ui.painter();
    let pad = crate::panels::PAD;
    p.text(
        Pos2::new(rect.left() + pad, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(Type::BODY),
        ink.ch.dim,
    );
    let max_w = (rect.width() - pad * 2.0 - 72.0).max(24.0);
    let max_chars = ((max_w / 13.0) as usize).clamp(2, 24);
    let shown = crate::shorten(value, max_chars);
    p.text(
        Pos2::new(rect.right() - pad, rect.center().y),
        Align2::RIGHT_CENTER,
        &shown,
        FontId::proportional(Type::BODY),
        ink.ch.text,
    );
    if shown != value {
        let resp = ui.interact(
            rect,
            ui.scope_id().with(("info-row", label)),
            Sense::hover(),
        );
        resp.on_hover_text(value);
    }
}

fn color_rgba(c: Color32) -> Rgba {
    Rgba::rgba(c.r(), c.g(), c.b(), c.a())
}

fn hex_of(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0[0], c.0[1], c.0[2])
}

fn link_host(url: &str) -> Option<String> {
    let s = url.trim();
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// 一排色板的当前状态：选中的颜色、自定义取色的落点色、悬停提示、当前是否「无覆盖」。
///
/// 打进一个结构体是为了让 `swatches` 的参数不越过 clippy 的 7 个门槛：
/// 这几项本来就是「这一排」的属性，分开传只是让调用点更长。
struct SwatchRow<'a> {
    current: &'a mut Option<Rgba>,
    fallback: Rgba,
    tip: &'a str,
    /// 这一项当前没有覆盖（= 继承）：斜杠格画选中环，用户才知道「现在是无颜色」。
    inheriting: bool,
}

/// 一次色板点击。
#[derive(Clone, Copy, PartialEq, Debug)]
enum SwatchPick {
    /// 选了某个颜色。
    Color(Rgba),
    /// 点了斜杠格：清掉这一项的覆盖，回到继承。
    None,
}

/// 色板网格（`docs/30` §4.2）：8 列 × 24px、列距 8。
/// 面板收窄到 240 时按版心降列，而不是把圆点挤出面板。
fn swatch_grid(available: f32) -> (usize, f32, f32) {
    const CELL: f32 = 24.0;
    const GAP: f32 = 8.0;
    let cols = (((available + GAP) / (CELL + GAP)).floor() as usize).clamp(1, 8);
    (cols, CELL, GAP)
}

/// 虚线圈：egui 没有虚线圆，按 12 段短弧现画（§4.2「第 9 格自定义虚线圆」）。
fn dashed_circle(p: &egui::Painter, center: Pos2, r: f32, stroke: Stroke) {
    use std::f32::consts::TAU;
    const N: usize = 12;
    for i in 0..N {
        let a0 = i as f32 / N as f32 * TAU;
        let a1 = a0 + TAU / (N as f32 * 2.0);
        p.line_segment(
            [
                center + Vec2::new(a0.cos(), a0.sin()) * r,
                center + Vec2::new(a1.cos(), a1.sin()) * r,
            ],
            stroke,
        );
    }
}

/// 一个色点格子的三种形态。
#[derive(Clone, Copy)]
enum Cell {
    Color(Rgba),
    /// 斜杠格：清掉覆盖、回到继承。
    None,
    /// 虚线圆：展开取色。
    Custom,
}

/// 色板一行的格子序列（`docs/30` §4.2）：8 个主题色 →「自定义」虚线圆 →「无颜色」斜杠格。
///
/// 抽成纯函数只为把**顺序**钉在测试里：8 列格数下「第 9 格是自定义」靠的就是这个次序，
/// 一旦有人把「无颜色」插到前面，像素上第 9 格就不再是虚线圆了。
fn palette_cells(theme: &Theme) -> Vec<Cell> {
    let mut cells: Vec<Cell> = theme.palette.iter().copied().map(Cell::Color).collect();
    cells.push(Cell::Custom);
    cells.push(Cell::None);
    cells
}

/// 色板（`docs/30` §4.2）：主题调色板 **8 列 × 24px、列距 8**，第 9 格是「自定义」虚线圆，
/// 另有一格「无颜色」斜杠格。返回这次点了什么（`None` = 没点）。
///
/// 键鼠共用这一个返回出口（5.3.3）：鼠标点击与键盘 Space/Enter 都落成 `resp.clicked()`，
/// 于是样式变更与撤销粒度**不可能**分叉。`row` 是这一排色板的稳定身份前缀，每个色点的
/// id 由「前缀 + 序号」派生，与它在换行后落到第几行无关。
fn swatches(
    ink: Ink,
    ui: &mut egui::Ui,
    theme: &Theme,
    row: &FocusKey,
    state: SwatchRow<'_>,
    focus: &mut FocusMemory,
) -> Option<SwatchPick> {
    let SwatchRow {
        current,
        fallback,
        tip,
        inheriting,
    } = state;
    let mut picked = None;
    let mut ids: Vec<egui::Id> = Vec::new();
    let mut focused_at: Option<usize> = None;
    let mut custom: Option<egui::Response> = None;

    let cells = palette_cells(theme);

    let (cols, cell, gap) = swatch_grid(ui.available_width() - GUTTER);
    for (row_index, chunk) in cells.chunks(cols).enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.spacing_mut().item_spacing = Vec2::new(gap, gap);
            for (col_index, cell_kind) in chunk.iter().copied().enumerate() {
                let i = row_index * cols + col_index;
                let key = FocusKey::indexed(row.base(), i);
                let (_, rect) = ui.allocate_space(Vec2::splat(cell));
                let resp = ui.interact(rect, focus::widget_id(ui, &key), Sense::click());
                focus.note(PanelId::Inspector, ui.ctx(), &key);
                ids.push(resp.id);
                if resp.has_focus() {
                    focused_at = Some(i);
                }
                let c = rect.center();
                let p = ui.painter();
                match cell_kind {
                    Cell::Color(rgba) => {
                        let selected = *current == Some(rgba);
                        p.circle_filled(c, 9.0, color32(rgba));
                        p.circle_stroke(c, 9.0, Stroke::new(1.0, ink.well));
                        if selected {
                            // §4.2：2px 强调环 + 1px 内白，环压在 8px 列距里，不压邻居。
                            p.circle_stroke(c, 11.0, Stroke::new(2.0, ink.ch.accent));
                            p.circle_stroke(c, 9.5, Stroke::new(1.0, Color32::WHITE));
                        }
                    }
                    Cell::None => {
                        p.circle_stroke(c, 9.0, Stroke::new(1.0, ink.ch.border_hot));
                        let d = 5.5;
                        p.line_segment(
                            [c + Vec2::new(-d, d), c + Vec2::new(d, -d)],
                            Stroke::new(1.4, ink.ch.dim),
                        );
                        if inheriting {
                            p.circle_stroke(c, 11.0, Stroke::new(2.0, ink.ch.accent));
                        }
                    }
                    Cell::Custom => {
                        dashed_circle(p, c, 9.0, Stroke::new(1.2, ink.ch.dim));
                    }
                }
                // 键盘焦点自己画一圈——压在选中环（外缘 12.0）之外，两件事不互相冒充。
                if resp.has_focus() {
                    p.circle_stroke(c, 12.5, Stroke::new(1.0, ink.ch.dim));
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let resp = match cell_kind {
                    Cell::Color(rgba) => resp.on_hover_text(hex_of(rgba)),
                    Cell::None => resp.on_hover_text("无颜色（继承主题）"),
                    Cell::Custom => resp.on_hover_text(tip),
                };
                match cell_kind {
                    Cell::Color(rgba) if resp.clicked() => {
                        *current = Some(rgba);
                        picked = Some(SwatchPick::Color(rgba));
                    }
                    Cell::None if resp.clicked() => {
                        *current = None;
                        picked = Some(SwatchPick::None);
                    }
                    Cell::Custom => custom = Some(resp),
                    _ => {}
                }
            }
        });
    }

    // 自定义取色：点虚线圆就地展开 egui 的调色板（不再占一整行「自定义…」）。
    if let Some(resp) = custom {
        egui::Popup::menu(&resp)
            .id(egui::Id::unique(("inspector-swatch-custom", row.base())))
            .gap(6.0)
            .show(|ui| {
                ui.set_min_width(184.0);
                let mut rgba = color32(current.unwrap_or(fallback));
                if egui::color_picker::color_picker_color32(
                    ui,
                    &mut rgba,
                    egui::color_picker::Alpha::Opaque,
                ) {
                    let c = color_rgba(rgba);
                    *current = Some(c);
                    picked = Some(SwatchPick::Color(c));
                }
            });
    }

    // 方向键在色板内移动焦点：左右在同排之间走，上下跨行（雪主题 8 色排一行、
    // 「自定义 + 无颜色」另起一行，共两行）。Space/Enter 由 egui 视作一次主键点击
    // （`Sense::click()` 自带该语义），所以不另开一条激活路径——键鼠必然等价。
    if let Some(i) = focused_at {
        let mut target = None;
        let last = ids.len().saturating_sub(1);
        if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::ArrowRight)) {
            target = Some((i + 1).min(last));
        } else if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::ArrowLeft)) {
            target = Some(i.saturating_sub(1));
        } else if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            target = Some((i + cols).min(last));
        } else if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            target = Some(i.saturating_sub(cols));
        }
        if let Some(id) = target.and_then(|t| ids.get(t)).copied() {
            ui.memory_mut(|m| m.request_focus(id));
        }
    }

    // §4.4：色板用**数字键 1–9 直选**，Enter 与空格同义（激活聚焦的那一格）。
    // 只在色板自己握着焦点时才吃键——否则在面板别处敲数字会把颜色改掉。
    if let Some(i) = focused_at {
        let mut target = None;
        if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::Enter)) {
            target = Some(i);
        }
        for d in 1..=9u32 {
            if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, digit_key(d))) {
                target = Some(d as usize - 1);
            }
        }
        if let Some(t) = target {
            match cells.get(t).copied() {
                Some(Cell::Color(rgba)) => {
                    *current = Some(rgba);
                    picked = Some(SwatchPick::Color(rgba));
                }
                Some(Cell::None) => {
                    *current = None;
                    picked = Some(SwatchPick::None);
                }
                _ => {}
            }
            if let Some(id) = ids.get(t).copied() {
                ui.memory_mut(|m| m.request_focus(id));
            }
        }
    }
    picked
}

/// 数字键 1–9（§4.4「色板用数字键 1–9 直选」）。
fn digit_key(d: u32) -> egui::Key {
    match d {
        1 => egui::Key::Num1,
        2 => egui::Key::Num2,
        3 => egui::Key::Num3,
        4 => egui::Key::Num4,
        5 => egui::Key::Num5,
        6 => egui::Key::Num6,
        7 => egui::Key::Num7,
        8 => egui::Key::Num8,
        _ => egui::Key::Num9,
    }
}

/// 一格 stepper 要显示与调节的东西（`− 值 +`）。
struct StepperRow<'a> {
    /// 中间显示什么；多选不一致时是「混合」。
    text: &'a str,
    /// 加减的起点（`text` 是「混合」时按这个值算下一档）。
    value: f32,
    /// 允许区间，端点都能停住。
    range: (f32, f32),
    /// 点一次走几格。
    step: f32,
}

/// 一格数值正在被键入时的草稿（`docs/30` §4.4「每个可改属性都有键盘路径」）。
///
/// `fresh` 只标记「这一帧刚开」：egui 的焦点只能在控件存在之后请求，而开编辑的那一帧
/// 输入框还没画出来，所以对焦点的请求要留到下一帧；此后不再抢焦点，用户 `Tab` 走就让它走。
#[derive(Debug, Clone)]
pub(crate) struct StepperEdit {
    /// 哪一格（`FocusKey::base()`）：同一时刻只开一格。
    row: String,
    draft: String,
    fresh: bool,
    /// 金样种子（`MW_STEPPER_EDIT`）：不抢焦点、也不因「没焦点」自己提交。
    /// 离屏 harness 里焦点与光标闪烁都会让像素不稳，种子只负责把样子摆出来。
    pinned: bool,
}

/// 键入的文本 → 新值。空、非数字、非有限值一律 `None`（不发命令、退回原值）；
/// 越界夹进区间（§4.2 字号 12–96、边宽 0–8）。抽成纯函数，端点全在单测里钉住。
fn parse_stepper_input(draft: &str, range: (f32, f32)) -> Option<f32> {
    let v: f32 = draft.trim().parse().ok()?;
    v.is_finite().then(|| v.clamp(range.0, range.1))
}

/// 本帧敲下的数字键（0–9）。只在数值格自己握着焦点时调用——
/// 否则在面板别处敲数字会把字号的输入框打开。
fn typed_digit(ui: &mut egui::Ui) -> Option<char> {
    for d in 0..=9u32 {
        let k = match d {
            0 => egui::Key::Num0,
            1 => egui::Key::Num1,
            2 => egui::Key::Num2,
            3 => egui::Key::Num3,
            4 => egui::Key::Num4,
            5 => egui::Key::Num5,
            6 => egui::Key::Num6,
            7 => egui::Key::Num7,
            8 => egui::Key::Num8,
            _ => egui::Key::Num9,
        };
        if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, k)) {
            return Some(char::from_digit(d, 10).unwrap_or('0'));
        }
    }
    None
}

/// 数值 stepper（§4.2）：24 高、宽 72，`− 值 +`，值右对齐。
///
/// 中间那格是**可聚焦、可点、可敲**的：点一下（或 `Enter`）变输入框，敲数字直接进编辑，
/// `↑`/`↓` 单步与 `+`/`−` 等价，`Enter` 提交、`Esc` 取消。
/// `docs/30` §4.2 只写了 stepper 的形状，§4.4 要的是「每个可改属性都有键盘路径」——
/// 对数值来说那条路径就是**敲数字**，光会连点十几下 `+` 不算。
///
/// 返回被改出来的新值（已夹到 `range`）；没改返回 `None`。
fn stepper(
    ui: &mut egui::Ui,
    ink: Ink,
    row: &FocusKey,
    state: StepperRow<'_>,
    focus: &mut FocusMemory,
    edit: &mut Option<StepperEdit>,
) -> Option<f32> {
    let StepperRow {
        text,
        value,
        range,
        step,
    } = state;
    const W: f32 = 72.0;
    const H: f32 = 24.0;
    const BTN: f32 = 20.0;
    let base = row.base();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(W, H), Sense::hover());
    let minus = Rect::from_min_size(rect.min, Vec2::new(BTN, H));
    let plus = Rect::from_min_size(Pos2::new(rect.right() - BTN, rect.top()), Vec2::new(BTN, H));
    let mid = Rect::from_min_max(minus.right_top(), plus.left_bottom());
    let k_minus = FocusKey::indexed(base, 0);
    let k_plus = FocusKey::indexed(base, 1);
    let k_val = FocusKey::indexed(base, 2);
    let r_minus = ui.interact(minus, focus::widget_id(ui, &k_minus), Sense::click());
    let r_plus = ui.interact(plus, focus::widget_id(ui, &k_plus), Sense::click());
    // 编辑态下这格是输入框自己在处理指针，别再叠一层命中区上去抢。
    let editing = edit.as_ref().is_some_and(|e| e.row == base);
    let r_val = if editing {
        None
    } else {
        let r = ui.interact(mid, focus::widget_id(ui, &k_val), Sense::click());
        focus.note(PanelId::Inspector, ui.ctx(), &k_val);
        Some(r)
    };
    focus.note(PanelId::Inspector, ui.ctx(), &k_minus);
    focus.note(PanelId::Inspector, ui.ctx(), &k_plus);
    {
        let p = ui.painter();
        p.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, ink.ch.sep),
            egui::StrokeKind::Inside,
        );
        for (r, ch) in [(&r_minus, "−"), (&r_plus, "+")] {
            if r.hovered() {
                p.rect_filled(r.rect.shrink(1.0), 3.0, ink.ch.hover);
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            crate::ui_kit::paint_focus_ring(p, ink.ch, r.rect, 3.0, r);
            p.text(
                r.rect.center(),
                Align2::CENTER_CENTER,
                ch,
                FontId::proportional(Type::BODY),
                if r.hovered() { ink.ch.text } else { ink.ch.dim },
            );
        }
        if !editing {
            p.text(
                Pos2::new(mid.right() - 4.0, mid.center().y),
                Align2::RIGHT_CENTER,
                text,
                FontId::proportional(Type::SMALL),
                ink.ch.text,
            );
        }
    }

    if editing {
        // 编辑态：格内换成右对齐的单行输入框，无框，只留一条底线（与备注域同一套观感）。
        //
        // 用 `new_child` 而不是 `ui.put`：`put` 会把父光标推到输入框的下沿，输入框比这一格高
        // 一两像素时，**下一行会被顶下去**——检查器里开始打字、下面的行抖一下，很难看。
        // `new_child` 只是在一个别人定好的矩形里画，不碰布局。
        let e = edit.as_mut()?;
        let mut out = None;
        {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(mid.shrink2(Vec2::new(3.0, 1.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            child.add(
                egui::TextEdit::singleline(&mut e.draft)
                    .id(focus::widget_id(ui, &k_val))
                    .font(FontId::proportional(Type::SMALL))
                    .text_color(ink.ch.text)
                    .hint_text(text)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::symmetric(2, 0))
                    // 与不编辑时那一段字同一个右对齐基线：点进去不该看见数字「跳」到左边。
                    .align(egui::Align2::RIGHT_CENTER)
                    .desired_width(mid.width() - 10.0),
            );
        }
        let id = focus::widget_id(ui, &k_val);
        let had_focus = ui.ctx().memory(|m| m.focused()) == Some(id);
        let was_fresh = e.fresh;
        let pinned = e.pinned;
        if e.fresh {
            ui.ctx().memory_mut(|m| m.request_focus(id));
            e.fresh = false;
        }
        if had_focus {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        ui.painter().hline(
            mid.x_range().shrink(3.0),
            mid.bottom() - 3.0,
            Stroke::new(1.0, ink.ch.accent),
        );
        let esc = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        // 点到别处也算提交（与画布上的行内编辑同一套手感），`Esc` 才是取消。
        // 「没焦点」要排除开编辑的这一帧与金样种子（`pinned`）——它们本来就没有焦点。
        let commit = enter || (!esc && !pinned && !had_focus && !was_fresh);
        if commit || esc {
            let draft = e.draft.clone();
            *edit = None;
            if !esc {
                out = parse_stepper_input(&draft, range);
            }
            crate::trace::event(&format!(
                "inspector_stepper row={base} edit=0 input={draft:?} value={:?}",
                out.map(|v| (v * 100.0).round() / 100.0)
            ));
            return out;
        }
        return None;
    }

    // 键盘：`↑`/`↓` 单步、`Enter` 或数字键进编辑。与 `+`/`−` 完全等价的两条路，
    // 差别只在按了几下——这正是「连点十几下」要消失的地方。
    let mut dir = 0.0;
    let mut typed: Option<char> = None;
    if let Some(r) = &r_val {
        if r.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        crate::ui_kit::paint_focus_ring(ui.painter(), ink.ch, mid, 3.0, r);
        if r.has_focus() {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                dir = 1.0;
            } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                dir = -1.0;
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                typed = Some(' ');
            }
            if typed.is_none() {
                typed = typed_digit(ui);
            }
        }
        if r.clicked() {
            typed = Some(' ');
        }
    }
    if let Some(c) = typed {
        // 空的草稿 + 底下的占位值是刻意的：点进去重打一个数，比在旧值后面接着敲更少出错。
        let draft = if c == ' ' {
            String::new()
        } else {
            c.to_string()
        };
        *edit = Some(StepperEdit {
            row: base.to_string(),
            draft,
            fresh: true,
            pinned: false,
        });
        crate::trace::event(&format!("inspector_stepper row={base} edit=1"));
        return None;
    }
    if r_minus.clicked() {
        return Some(stepped(value, -1.0, range, step));
    }
    if r_plus.clicked() {
        return Some(stepped(value, 1.0, range, step));
    }
    if dir != 0.0 {
        return Some(stepped(value, dir, range, step));
    }
    None
}

/// stepper 的一步：夹在区间里（字号 12–96、边宽 0–8）。抽成纯函数才好钉住端点。
fn stepped(value: f32, dir: f32, range: (f32, f32), step: f32) -> f32 {
    (value + dir * step).clamp(range.0, range.1)
}

/// 圆角 stepper 的可见上限（V1-02 G-4）：渲染层把半径钳到 `min(w,h)/2`，
/// 盒子高 36 时上限就是 18——控件再摆一个 24 只是骗人。合同上限 24 仍保留，
/// 这里只按**实际盒子**把它压到能看出差异的那个值；拿不到盒子时退回 24。
pub(crate) fn radius_ceiling(boxes: &[Option<Rect>]) -> f32 {
    let mut ceil = 24.0_f32;
    for r in boxes.iter().flatten() {
        ceil = ceil.min(r.width().min(r.height()) * 0.5);
    }
    // 控件显示整数、步进 2，上限也取整：19.5 这种东西只在像素里存在，
    // 摆进 stepper 会让「显示 20、实际 19.5」。合同值本身也是整数。
    ceil.clamp(0.0, 24.0).floor()
}

/// 分段按钮（形状/页签/布局都用它）：选中态用主题强调色，而不是 egui 默认蓝。
///
/// `key` 为 `Some` 时给按钮一个稳定身份并纳入焦点记忆；为 `None` 时沿用自动 id。
/// 面板底色里掺 `t` 比例的强调色（§4.2「选中 8% 强调底」）。
///
/// 不用主题的 `accent_soft`：那是一支独立的不透明色，跟强调色并不总是同一条线，
/// 换主题后会出现「底色是绿的、底条是蓝的」。按比例现混，底与条永远同源。
fn tint(base: Color32, accent: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(
        f(base.r(), accent.r()),
        f(base.g(), accent.g()),
        f(base.b(), accent.b()),
    )
}

/// 分段控件（`docs/30` §4.2）：选中 **8% 强调底 + 底部 1.5px 强调条**，**不描边**。
///
/// 与 `seg`（旧的 `chip` 胶囊）的分工：`seg` 还用在组内的「当前节点 / 含子树」
/// 这类小开关上，样式上是一枚小胶囊；页头这三段是**面**不是胶囊，所以另画。
fn segment(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    size: Vec2,
    ink: Ink,
    key: &FocusKey,
    focus: &mut FocusMemory,
) -> bool {
    let (_, rect) = ui.allocate_space(size);
    let resp = ui.interact(rect, focus::widget_id(ui, key), Sense::click());
    focus.note(PanelId::Inspector, ui.ctx(), key);
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0.0, tint(ink.ch.panel, ink.ch.accent, 0.08));
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.bottom() - geo::SEG_BAR),
                rect.right_bottom(),
            ),
            0.0,
            ink.ch.accent,
        );
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, ink.ch.hover);
    }
    crate::ui_kit::paint_focus_ring(p, ink.ch, rect, 0.0, &resp);
    let font = FontId::proportional(geo::SEG_TEXT);
    let color = if selected { ink.on } else { ink.ch.dim };
    let c = Pos2::new(rect.center().x, rect.center().y - geo::SEG_BAR * 0.5);
    p.text(c, Align2::CENTER_CENTER, label, font, color);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// 分组标题行（`docs/30` §4.1）：高 28，chevron 12 + 名称 13/19 semibold + 右侧摘要 12/18。
///
/// `first` 为假时先画一条 8px 的 `hairline` 间隔——组与组之间靠线分开，不靠留白堆。
///
/// `scope` 给样式组用：右侧画「本节点 / 含子树」两段（§4.2「作用域」），
/// 有它的时候**不画摘要**——两个东西都靠右，288 宽放不下。
///
/// 返回 `(开合, 作用域)`：前者 `Some(新状态)` 表示点了这一行；后者 `Some(true/false)`
/// 表示点了右段的「含子树 / 本节点」。点在开关上的那一下不算点行。
#[allow(clippy::too_many_arguments)] // 一整行的绘制参数，拆成结构体只会多一层搬运
fn group_header(
    ui: &mut egui::Ui,
    ink: Ink,
    label: &str,
    summary: &str,
    open: bool,
    scope: Option<bool>,
    key: &FocusKey,
    focus: &mut FocusMemory,
    first: bool,
) -> (Option<bool>, Option<bool>) {
    let gap = geo::GROUP_GAP;
    if !first {
        let (_, line) = ui.allocate_space(Vec2::new(ui.available_width(), gap));
        ui.painter().hline(
            line.x_range().shrink(GUTTER),
            line.center().y,
            Stroke::new(design::stroke::HAIRLINE, ink.ch.sep),
        );
    } else {
        ui.add_space(gap);
    }
    let (_, rect) = ui.allocate_space(Vec2::new(ui.available_width(), geo::GROUP_H));
    let resp = ui.interact(rect, focus::widget_id(ui, key), Sense::click());
    focus.note(PanelId::Inspector, ui.ctx(), key);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, ink.ch.hover);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    crate::ui_kit::paint_focus_ring(ui.painter(), ink.ch, rect, 0.0, &resp);
    // chevron：展开朝下、折叠朝右。两段线，不用字形图标（§4.5）。
    let cc = Pos2::new(rect.left() + GUTTER + 4.0, rect.center().y);
    let r = 3.0;
    let h = 2.0;
    let (p0, p1, p2) = if open {
        (
            cc + Vec2::new(-r, -h),
            cc + Vec2::new(0.0, h),
            cc + Vec2::new(r, -h),
        )
    } else {
        (
            cc + Vec2::new(-h, -r),
            cc + Vec2::new(h, 0.0),
            cc + Vec2::new(-h, r),
        )
    };
    let stroke = Stroke::new(1.4, ink.ch.dim);
    ui.painter().line_segment([p0, p1], stroke);
    ui.painter().line_segment([p1, p2], stroke);
    let text_x = cc.x + r + 8.0;
    let font = FontId::new(
        geo::GROUP_TEXT,
        FontFamily::Name(crate::fonts::SEMIBOLD.into()),
    );
    let mid = rect.center().y;
    ui.painter().text(
        Pos2::new(text_x, mid),
        Align2::LEFT_CENTER,
        label,
        font,
        ink.ch.text,
    );
    // 作用域开关（样式组）：两块 48 宽的文字，选中的那块给 8% 强调底 + 强调色文字。
    let mut scope_hit = None;
    let mut scope_rect = Rect::NOTHING;
    if let Some(subtree) = scope {
        let w = geo::SCOPE_W;
        scope_rect = Rect::from_center_size(
            Pos2::new(rect.right() - GUTTER - w * 0.5, mid),
            Vec2::new(w, geo::SCOPE_H),
        );
        let half = w * 0.5;
        let left = Rect::from_min_size(scope_rect.min, Vec2::new(half, geo::SCOPE_H));
        let right = Rect::from_min_size(
            Pos2::new(scope_rect.min.x + half, scope_rect.min.y),
            Vec2::new(half, geo::SCOPE_H),
        );
        let font = FontId::proportional(geo::GROUP_SUMMARY);
        for (on, r, text) in [(!subtree, left, "本节点"), (subtree, right, "含子树")] {
            if on {
                ui.painter()
                    .rect_filled(r, 4.0, ink.ch.accent.gamma_multiply(0.08));
            }
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                text,
                font.clone(),
                if on { ink.ch.accent } else { ink.ch.dim },
            );
        }
        // 键里必须带上**组**：样式页有两个带作用域的组，共用一个 `FocusKey::new` 会让两处
        // 拿到同一个 egui id，后一个把前一个的命中矩形顶掉（点了没反应或串到另一组）。
        let hit = ui.interact(
            scope_rect,
            focus::widget_id(ui, &FocusKey::dynamic("inspector/style/scope", key.base())),
            Sense::click(),
        );
        if hit.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if hit.clicked() {
            let x = hit
                .interact_pointer_pos()
                .map(|p| p.x)
                .unwrap_or(scope_rect.center().x);
            scope_hit = Some(x > scope_rect.center().x);
        }
    } else if !summary.is_empty() {
        ui.painter().text(
            Pos2::new(rect.right() - GUTTER, mid),
            Align2::RIGHT_CENTER,
            summary,
            FontId::proportional(geo::GROUP_SUMMARY),
            ink.ch.dim,
        );
    }
    // 点在开关上的那一下不折叠分组：否则想切换作用域却把组收起来了。
    let inside_scope = resp
        .interact_pointer_pos()
        .is_some_and(|p| scope_rect.contains(p));
    let toggle = (resp.clicked() && !inside_scope).then_some(!open);
    (toggle, scope_hit)
}

/// 按像素宽度截断并补省略号（§4.1 对象行标题「省略」）。
///
/// 直接量字符串宽度而不是靠 `Label::truncate`：这一行是用画笔画的，
/// 两条基线（标题/副行）必须落在固定 y 上，交给 egui 排版会随行高漂。
fn ellipsize(ui: &egui::Ui, text: &str, font: FontId, max_w: f32) -> String {
    let measure = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if measure(text) <= max_w {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let mut s: String = chars.iter().collect();
        s.push('…');
        if measure(&s) <= max_w {
            return s;
        }
    }
    "…".to_string()
}

/// Esc 的一步（`docs/30` §4.4「Esc 先退出当前控件，再退出面板」）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EscStep {
    /// 控件还握着焦点：松掉它，面板留着。
    Release,
    /// 已经松过一次：把键盘交给画布。
    Exit,
    /// 面板里本来就没焦点，这一下不归检查器管。
    Ignore,
}

/// 两级 Esc 的判据：控件握着焦点 → 先松；上一帧刚松过（latch）→ 退出；否则不吃这一下。
pub(crate) fn esc_step(holding_control: bool, latch: bool) -> EscStep {
    if holding_control {
        EscStep::Release
    } else if latch {
        EscStep::Exit
    } else {
        EscStep::Ignore
    }
}

/// `docs/30` §4.3：连续拖滑杆 / 连点 stepper 在 **500ms** 内合并为一条撤销。
pub(crate) const STYLE_MERGE_MS: u128 = 500;

/// 这条命令属于哪一类「手势」：同一次连点必须落在**同一批 id** 上。
///
/// 只有 `SetStyle`（或整包都是 `SetStyle` 的 `Batch`——作用域选「含子树」时就是这种）
/// 能合并。换颜色、写备注、删除节点都不在这一类，不会被并进前一条撤销。
pub(crate) fn style_gesture_key(cmd: &Command) -> Option<String> {
    fn collect(cmd: &Command, out: &mut Vec<String>) -> bool {
        match cmd {
            Command::SetStyle { ids, .. } => {
                out.extend(ids.iter().cloned());
                true
            }
            Command::Batch { commands } => commands.iter().all(|c| collect(c, out)),
            _ => false,
        }
    }
    let mut ids = Vec::new();
    if !collect(cmd, &mut ids) || ids.is_empty() {
        return None;
    }
    ids.sort();
    ids.dedup();
    Some(ids.join(","))
}

/// 两次样式编辑算不算同一次手势：同一批 id，且上一次就在 500ms 以内。
pub(crate) fn same_gesture(prev_key: Option<&str>, prev_since_ms: u128, key: &str) -> bool {
    prev_key == Some(key) && prev_since_ms <= STYLE_MERGE_MS
}

/// 组内的一行，高度钉死 [`geo::ROW_H`]（`docs/30` §4.1「组内控件行高 28」）。
///
/// 别用 `ui.horizontal`：它按全局 `spacing.interact_size.y` 要高度，而这个值在本项目里是
/// `design::control::INPUT = 30`。一行因此变成 30 高，两行之间再叠 8 的行距就是 38——
/// 合同写的是 28。这里的 `allocate_ui_with_layout` 用显式的 `desired_size`，压过那个默认。
///
/// `labels_are_text_with_a_six_px_dot` 与 `content_rows_are_twenty_eight_high` 盯着这条。
pub(crate) fn content_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(w, geo::ROW_H),
        egui::Layout::left_to_right(egui::Align::Center),
        add,
    )
    .inner
}

/// 分组列表的滚动体（`docs/30` §4.1「组内行间距 6」）。
///
/// 检查器的三页分组内容（样式 / 内容 / 标记）与概念图那一页都画在这一层里。
/// 进来先把 `item_spacing.y` 从全局的 `design::space::SM`（8）压到 [`geo::ROW_GAP`]（6）：
/// 组内相邻两行因此是 6——这个值以前没人写，跟着外壳的全局 8 走，比合同松 2px。
///
/// 两件事不算在这一条里：组与组之间的 8px hairline 由 [`group_header`] 自己
/// `allocate_space`（那是线宽，不是行距）；行内显式 `add_space(6.0)` 的段间距照旧。
///
/// 上层若把 `item_spacing` 又改回 8，组内行距会跟着回 8：
/// `the_group_body_compresses_the_row_gap_to_six` 把「外面 8、里面 6」两头都钉住了。
fn group_list<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(2.0);
            ui.spacing_mut().item_spacing.y = geo::ROW_GAP;
            let r = add(ui);
            ui.add_space(12.0);
            r
        })
        .inner
}

/// 命题行里短语/概念名各自最多几个字（C14 的「命题」组用）。
///
/// 面板最窄 240 时内容宽只有 212，一行里除了这两段还要放箭头字形与行尾 `×`。
/// 不设上限的话，一个长短语就能把内容最小宽顶大——面板宽度是合同值（`docs/30` §4.1），
/// SE-05 修过「288 被顶到 316 且每帧 +2」那一类。常见概念名与短语（2–4 字）碰不到上限，
/// 超长的在这里省略成「…」，全文进悬停说明。
const CLIP_PHRASE: usize = 5;
const CLIP_NAME: usize = 6;

/// 按**字符**（不是字节）截断并补省略号，免得把汉字劈成半个。
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// 标签输入行：给定「减掉行内间距之后的可用宽」，输入框该开多宽。
///
/// 这一行是「输入框 + 44 宽的按钮」两件东西，输入框的 `desired_width` **只算文字区**，
/// 左右 8+8 的内距另加在控件总宽上。漏掉那 16px，整行就比可用宽多 16——
/// 面板的最小宽被内容顶大，每帧 +2 一路爬（SE-07 实测 288 → 316 还在涨）。
/// 抽成纯函数是为了把这条算术钉在测试里：改任何一个常量都会当场算给你看。
fn label_field_width(avail_after_gap: f32) -> f32 {
    (avail_after_gap - LABEL_BTN_W - LABEL_PAD_X).max(60.0)
}

/// 标签输入行里输入框之外的两笔固定宽：「添加」按钮 + 输入框左右内距。
const LABEL_BTN_W: f32 = 44.0;
/// 输入框的左右内距（`TextEdit::margin` 的 x），`desired_width` 不含它。
const LABEL_MARGIN_X: i8 = 8;
/// 内距合计。由 [`LABEL_MARGIN_X`] 推出来，改内距不会再忘这边。
const LABEL_PAD_X: f32 = LABEL_MARGIN_X as f32 * 2.0;

/// 备注框这一刻占几行（`docs/30` §4.2：「3 行起、超过 8 行可展开」）。
///
/// 折叠态封顶 8 行，超出的部分靠「展开」按钮放出来；展开态按实际行数给足。
/// `lines()` 不数末尾那个换行——正在敲的输入框不该因为按下 Enter 就多长一格。
fn note_rows(text: &str, expanded: bool) -> usize {
    const MIN: usize = 3;
    const CAP: usize = 8;
    let lines = text.lines().count().max(MIN);
    if expanded {
        lines
    } else {
        lines.min(CAP)
    }
}

/// 卡片绑定的「来源」一行（`docs/30` §4.2：已绑定显示卡片标题 + 来源）。
///
/// 卡片本身是独立实体，可能来自别的文档、也可能是没挂过节点的独立卡片；
/// 检查器里只说清楚**它从哪来**，不替用户判断该不该解绑。
fn card_source(doc: &Document, card: &mw_core::NoteCard, word: &str) -> String {
    if card.source_doc.is_empty() {
        "独立卡片".to_string()
    } else if card.source_doc == doc.id {
        if card.source_node.is_some() {
            format!("本文档 · 挂在{word}上")
        } else {
            "本文档 · 独立卡片".to_string()
        }
    } else {
        "来自其它文档".to_string()
    }
}

/// 对象行副行（§4.1）：`一级主题 · 2 个子主题`；概念图显示`概念 · N 条命题`，钉住时前置`已钉住`。
fn object_meta(doc: &Document, id: &str) -> String {
    let Some(n) = doc.node(id) else {
        return String::new();
    };
    if doc.is_concept_map() {
        let mut bits = Vec::new();
        if doc.is_concept_pinned(id) {
            bits.push("已钉住".to_string());
        }
        bits.push("概念".to_string());
        let prop_count = doc.concept_proposition_count(id);
        if prop_count > 0 {
            bits.push(format!("{} 条命题", prop_count));
        }
        if n.card_id.is_some() {
            bits.push("已挂卡片".into());
        }
        if !n.labels.is_empty() {
            bits.push(format!("{} 个标签", n.labels.len()));
        }
        return bits.join(" · ");
    }
    let kind = if id == doc.root_id {
        "中心主题"
    } else if n.parent_id.as_deref() == Some(doc.root_id.as_str()) {
        "一级主题"
    } else {
        "分支主题"
    };
    let mut bits = vec![kind.to_string()];
    if !n.children.is_empty() {
        bits.push(format!("{} 个子主题", n.children.len()));
    }
    if n.card_id.is_some() {
        bits.push("已挂卡片".into());
    }
    if !n.labels.is_empty() {
        bits.push(format!("{} 个标签", n.labels.len()));
    }
    bits.join(" · ")
}

/// 某个分组这一刻是开是合：用户动过就按用户的，没动过就按组的默认。
fn group_open(stored: &[String], key: &str, default_open: bool) -> bool {
    stored
        .iter()
        .find_map(|e| {
            let (k, v) = e.split_once('=')?;
            (k == key).then_some(v == "1")
        })
        .unwrap_or(default_open)
}

impl Inspector {
    /// 画一个分组标题；被点过就返回折叠/展开动作。
    ///
    /// 开合状态来自窗口状态（`Extras.collapsed`），面板自己不存——
    /// 换标签页、关掉面板再打开，折叠都得还在（§4.1）。
    pub(crate) fn group(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        label: &str,
        summary: &str,
        key: &'static str,
        first: bool,
    ) -> (bool, Option<Action>) {
        self.group_with(ui, focus, label, summary, key, first, true)
    }

    /// 同上，但能指定「用户没动过时」的默认开合（§4.1：标记组默认折叠）。
    #[allow(clippy::too_many_arguments)] // 分组标题的六个属性，拆开只会让调用点更长
    pub(crate) fn group_with(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        label: &str,
        summary: &str,
        key: &'static str,
        first: bool,
        default_open: bool,
    ) -> (bool, Option<Action>) {
        let open = group_open(&self.groups, key, default_open);
        let (toggled, _) = group_header(
            ui,
            self.ink,
            label,
            summary,
            open,
            None,
            &FocusKey::new(key),
            focus,
            first,
        );
        let action = toggled.map(|open| Action::ToggleGroup {
            key: key.to_string(),
            open,
        });
        (open, action)
    }

    /// 样式组的分组标题：右侧带「本节点 / 含子树」两段（§4.2「作用域」）。
    ///
    /// 作用域只对**样式类属性**生效——颜色、字号、边框、形状都走 `style_cmd`，
    /// 备注/任务/标签/链接各走自己的 `Command`，天然不吃这个开关。
    #[allow(clippy::too_many_arguments)] // 与 `group_with` 同因：分组标题的属性本就多
    pub(crate) fn group_scoped(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        label: &str,
        summary: &str,
        key: &'static str,
        first: bool,
        subtree: bool,
    ) -> (bool, Option<Action>, Option<bool>) {
        let open = group_open(&self.groups, key, true);
        let (toggled, scope) = group_header(
            ui,
            self.ink,
            label,
            summary,
            open,
            Some(subtree),
            &FocusKey::new(key),
            focus,
            first,
        );
        let action = toggled.map(|open| Action::ToggleGroup {
            key: key.to_string(),
            open,
        });
        (open, action, scope)
    }
}

/// 「形状」四段（圆角/直角/胶囊/椭圆）每段的宽度：按可用宽度均分，
/// 扣掉左侧内距与三段 4px 间隙。面板拖到 240 时仍放得下——写死 66×4 会让
/// 内容最小宽变成 292，面板就再也缩不到 240（§4.1 的区间失效）。
fn shape_seg_w(available: f32) -> f32 {
    ((available - 3.0 * 4.0 - GUTTER) / 4.0).max(44.0)
}

/// 「字重三段 / 对齐三段」每段的宽度：同样按可用宽度均分（SE-13）。
/// 单字标签（400 / 左）比形状行更窄，但三段共用一条计算公式才好钉住「不顶宽」。
fn text_seg_w(available: f32) -> f32 {
    ((available - 2.0 * 4.0 - GUTTER) / 3.0).max(44.0)
}

fn seg(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    size: Vec2,
    ink: Ink,
    key: Option<&FocusKey>,
    focus: &mut FocusMemory,
) -> bool {
    let (rect, resp) = match key {
        Some(k) => {
            let (_, rect) = ui.allocate_space(size);
            let resp = ui.interact(rect, focus::widget_id(ui, k), Sense::click());
            focus.note(PanelId::Inspector, ui.ctx(), k);
            (rect, resp)
        }
        None => ui.allocate_exact_size(size, Sense::click()),
    };
    crate::ui_kit::chip(
        ui,
        ink.ch,
        rect,
        &resp,
        label,
        FontId::proportional(Type::SMALL),
        crate::ui_kit::ChipPaint {
            selected,
            fill: ink.ch.accent_soft,
            ring: ink.ring,
            ink: ink.on,
            mixed: false,
            bold: false,
        },
    )
}

/// B / I 切换按钮。`bold` 只影响字形粗细（用双描边伪粗体——egui 默认字体没有粗体族）。
fn toggle(
    ui: &mut egui::Ui,
    label: &str,
    state: Option<bool>,
    bold: bool,
    ink: Ink,
    key: &FocusKey,
    focus: &mut FocusMemory,
) -> bool {
    let (_, rect) = ui.allocate_space(Vec2::new(30.0, 26.0));
    let resp = ui.interact(rect, focus::widget_id(ui, key), Sense::click());
    focus.note(PanelId::Inspector, ui.ctx(), key);
    let on = state == Some(true);
    let clicked = crate::ui_kit::chip(
        ui,
        ink.ch,
        rect,
        &resp,
        label,
        FontId::proportional(Type::LABEL),
        crate::ui_kit::ChipPaint {
            selected: on,
            fill: ink.ch.accent_soft,
            ring: ink.ring,
            ink: ink.on,
            mixed: state.is_none(),
            bold,
        },
    );
    let _ = resp.on_hover_text(if bold { "粗体" } else { "斜体" });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::theme::get_theme;
    use mw_core::theme::resolve_all;
    use mw_core::theme::themes;

    /// `docs/30` §4.1/§4.2 的四段行高与默认宽度。这四条是「检查器像不像玩具」的
    /// 硬指标，改了就必然是基线漂移 + 合同不符，所以直接钉死。
    #[test]
    #[allow(clippy::assertions_on_constants)] // 断言的正是常量本身，常量「恒真」才算钉住
    fn the_skeleton_geometry_matches_the_contract() {
        assert_eq!(PANEL_W, 288.0, "默认宽 §4.1");
        assert_eq!((PANEL_W_MIN, PANEL_W_MAX), (240.0, 360.0), "可拖区间 §4.1");
        assert_eq!(geo::HEAD_H, 40.0, "头行 §4.1");
        assert_eq!(geo::OBJ_H, 44.0, "对象行 §4.1");
        assert_eq!(geo::SEG_H, 30.0, "分段控件 §4.2");
        assert_eq!(geo::GROUP_H, 28.0, "分组标题行 §4.1");
        assert_eq!(geo::ROW_H, 28.0, "组内控件行高 §4.1");
        assert_eq!(geo::ROW_TEXT, 13.0, "开关/标签左标签字阶 §4.1");
        assert_eq!(GUTTER, 12.0, "组内左右内距 §4.1");
        assert_eq!(geo::ROW_GAP, 6.0, "组内行间距 §4.1");
        // 「组间 8px hairline」和「组内行间距 6」是两件事：前者是那条线的带宽，
        // 后者是同一组里相邻两行的空隙。同一个数会被下一个改外观的人顺手化简掉。
        assert_ne!(
            geo::GROUP_GAP,
            geo::ROW_GAP,
            "组间 8px hairline 与组内行间距 6 不能合并成一个常量"
        );
        // 头行必须装得下 24 的关闭按钮，对象行必须装得下 14/20 + 12/18 两行。
        assert!(geo::HEAD_H >= geo::CLOSE_BTN);
        assert!(geo::OBJ_H >= geo::OBJ_TITLE + geo::OBJ_META + 4.0);
    }

    #[test]
    fn panel_width_clamps_to_the_drag_range() {
        assert_eq!(clamp_width(100.0), PANEL_W_MIN);
        assert_eq!(clamp_width(300.0), 300.0);
        assert_eq!(clamp_width(9999.0), PANEL_W_MAX);
    }

    /// 色板格子顺序（§4.2）：主题色在前，「自定义」虚线圆紧随其后，「无颜色」斜杠格收尾。
    ///
    /// 主题色的**个数**各主题不一样（雪 8 色排满一行、墨夜/雾/沙 6 色），这是主题令牌
    /// 既有的事实，SE-06 不动它：色板网格永远是 8 列，格子按流式排，色少的主题一行就装下
    /// 「8 色 + 两格」的全部内容。所以这里钉的是**次序**与**上限**，不是「第 9 格」这个
    /// 只在雪主题成立的巧合。
    #[test]
    fn palette_ends_with_custom_then_none() {
        for id in ["snow", "ink", "mist", "sand"] {
            let theme = get_theme(id);
            let cells = palette_cells(&theme);
            assert!(
                (1..=8).contains(&theme.palette.len()),
                "{id}：主题色 {} 个，超过 8 个会挤到第二行、把自定义格顶走",
                theme.palette.len()
            );
            assert_eq!(
                cells.len(),
                theme.palette.len() + 2,
                "{id}：主题色 + 自定义 + 无颜色"
            );
            let n = theme.palette.len();
            assert!(
                matches!(cells[n], Cell::Custom),
                "{id}：主题色之后第一格必须是自定义虚线圆"
            );
            assert!(
                matches!(cells[n + 1], Cell::None),
                "{id}：末格必须是无颜色斜杠格"
            );
            assert!(
                cells[..n].iter().all(|c| matches!(*c, Cell::Color(_))),
                "{id}：前段只能是纯色"
            );
        }
    }

    /// 「形状」四段必须跟着版心缩：给 240 的面板算出 292 的固定宽，
    /// egui 会把面板顶到内容最小宽，用户就再也拖不回 240（回归过的 bug）。
    #[test]
    fn shape_segments_fit_the_narrowest_panel() {
        for panel_w in [PANEL_W_MIN, PANEL_W, PANEL_W_MAX] {
            let total = GUTTER + 4.0 * shape_seg_w(panel_w) + 3.0 * 4.0;
            assert!(
                total <= panel_w + 0.01,
                "面板 {panel_w} 时四段合计 {total} 会顶宽"
            );
        }
        // 288 版心下每段仍有余量容纳两个汉字（12.5px 字），不至于挤成「…」。
        assert!(shape_seg_w(PANEL_W) >= 60.0, "默认宽下不该瘦成窄条");
    }

    /// SE-13 的「字重 / 对齐」三段的宽度同样得跟着版心缩：它们与形状行
    /// 共用一条 `可用宽度均分` 纪律，不能又冒出一个写死的 292。
    #[test]
    fn the_weight_and_align_segments_fit_the_narrowest_panel() {
        for panel_w in [PANEL_W_MIN, PANEL_W, PANEL_W_MAX] {
            let total = GUTTER + 3.0 * text_seg_w(panel_w) + 2.0 * 4.0;
            assert!(
                total <= panel_w + 0.01,
                "面板 {panel_w} 时三段合计 {total} 会顶宽"
            );
        }
        // 默认宽下每段仍放得下三个数字（「600」在 12.5px 下约 24px）。
        assert!(text_seg_w(PANEL_W) >= 60.0, "默认宽下不该瘦成窄条");
    }

    /// 色板网格（§4.2）：288 面板 8 列、240 面板降到 7 列，永远 24px 格 + 8px 距。
    #[test]
    fn swatch_grid_keeps_eight_columns_until_the_panel_gets_narrow() {
        let content = |w: f32| w - GUTTER * 2.0;
        assert_eq!(
            swatch_grid(content(PANEL_W)),
            (8, 24.0, 8.0),
            "288 面板 = 8 列"
        );
        assert_eq!(
            swatch_grid(content(PANEL_W_MIN)),
            (7, 24.0, 8.0),
            "240 面板降一列，不是把圆点挤出去"
        );
        assert_eq!(swatch_grid(48.0), (1, 24.0, 8.0), "再窄也留一格");
        // 8 列是上限：再宽也不铺成一长排（§4.2 写死 8 列）。
        assert_eq!(swatch_grid(2000.0).0, 8);
    }

    /// stepper 端点：字号 12–96、边宽 0–8，加减到头就停住（§4.2）。
    #[test]
    fn stepper_stops_at_the_contract_ends() {
        assert_eq!(stepped(12.0, -1.0, (12.0, 96.0), 2.0), 12.0, "12 是底");
        assert_eq!(stepped(95.0, 1.0, (12.0, 96.0), 2.0), 96.0, "96 是顶");
        assert_eq!(stepped(14.0, 1.0, (12.0, 96.0), 2.0), 16.0, "一步 2");
        assert_eq!(stepped(0.0, -1.0, (0.0, 8.0), 1.0), 0.0, "边宽 0 是底");
        assert_eq!(stepped(8.0, 1.0, (0.0, 8.0), 1.0), 8.0, "边宽 8 是顶");
    }

    /// 圆角上限跟着实际盒子走（V1-02 G-4）：渲染层钳 `min(w,h)/2`，控件也照这个压，
    /// 免得用户把 24 拖满却看不出和 18 的差别。
    #[test]
    fn radius_ceiling_follows_the_tallest_short_side() {
        let at = |w: f32, h: f32| Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h)));
        assert_eq!(radius_ceiling(&[]), 24.0, "没盒子时退回合同上限");
        assert_eq!(radius_ceiling(&[None]), 24.0, "找不到节点时也是上限");
        assert_eq!(radius_ceiling(&[at(200.0, 36.0)]), 18.0, "盒高 36 → 18");
        assert_eq!(
            radius_ceiling(&[at(200.0, 49.0)]),
            24.0,
            "盒高 49 → 24 封顶"
        );
        assert_eq!(
            radius_ceiling(&[at(200.0, 52.0), at(120.0, 30.0)]),
            15.0,
            "多选取最小"
        );
        assert_eq!(radius_ceiling(&[at(200.0, 0.0)]), 0.0, "退化盒不上报负数");
    }

    /// 键入的文本 → 新值：空/非数字/非有限一律不要，越界夹进区间（§4.2 的两条值域）。
    #[test]
    fn typed_numbers_are_clamped_and_garbage_is_ignored() {
        assert_eq!(parse_stepper_input("48", (12.0, 96.0)), Some(48.0));
        assert_eq!(
            parse_stepper_input(" 5 ", (12.0, 96.0)),
            Some(12.0),
            "低于下限夹到 12"
        );
        assert_eq!(
            parse_stepper_input("300", (12.0, 96.0)),
            Some(96.0),
            "高于上限夹到 96"
        );
        assert_eq!(parse_stepper_input("8", (0.0, 8.0)), Some(8.0), "边宽顶格");
        assert_eq!(parse_stepper_input("", (12.0, 96.0)), None, "空的不发命令");
        assert_eq!(
            parse_stepper_input("abc", (12.0, 96.0)),
            None,
            "脏输入不发命令"
        );
        assert_eq!(parse_stepper_input("NaN", (12.0, 96.0)), None, "NaN 不算数");
    }

    /// 无头跑一帧数值格：喂几个事件，把返回值与编辑态一起带回来。
    fn stepper_frame(
        ctx: &egui::Context,
        focus: &mut FocusMemory,
        edit: &mut Option<StepperEdit>,
        events: Vec<egui::Event>,
    ) -> Option<f32> {
        let ink = Ink::of(&get_theme("snow"));
        let row = FocusKey::new("inspector/style/size");
        let mut out = None;
        ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                out = stepper(
                    ui,
                    ink,
                    &row,
                    StepperRow {
                        text: "16",
                        value: 16.0,
                        range: (12.0, 96.0),
                        step: 2.0,
                    },
                    focus,
                    edit,
                );
            },
        )
        .drop_without_applying_deltas();
        out
    }

    /// §4.4「每个可改属性都有键盘路径」：`↑`/`↓` 与点 `+`/`−` 是同一件事。
    #[test]
    fn arrow_keys_step_a_number_like_the_buttons_do() {
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        let mut edit: Option<StepperEdit> = None;
        let cell = FocusKey::indexed("inspector/style/size", 2);
        // 焦点按 id 记，键本身与 ui 无关，所以可以先请求、下一帧就有。
        ctx.memory_mut(|m| m.request_focus(cell.id()));
        let up = stepper_frame(&ctx, &mut focus, &mut edit, vec![key(Key::ArrowUp)]);
        assert_eq!(up, Some(18.0), "↑ 一步 = 点一次 +（16 → 18）");
        ctx.memory_mut(|m| m.request_focus(cell.id()));
        let down = stepper_frame(&ctx, &mut focus, &mut edit, vec![key(Key::ArrowDown)]);
        assert_eq!(down, Some(14.0), "↓ 一步 = 点一次 −（16 → 14）");
        assert!(edit.is_none(), "方向键不进编辑态");
    }

    /// 在数值格上直接敲数字：当场进编辑态，草稿就是那个数字（不用先把旧值删掉）。
    #[test]
    fn typing_a_digit_opens_the_field_with_that_digit() {
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        let mut edit: Option<StepperEdit> = None;
        let cell = FocusKey::indexed("inspector/style/size", 2);
        ctx.memory_mut(|m| m.request_focus(cell.id()));
        let out = stepper_frame(&ctx, &mut focus, &mut edit, vec![key(Key::Num4)]);
        assert_eq!(out, None, "敲数字只开编辑，不当场改值");
        let e = edit.expect("应当进编辑态");
        assert_eq!(e.row, "inspector/style/size");
        assert_eq!(e.draft, "4", "敲下的那个数字就是草稿");
    }

    /// 提交与取消：`Enter` 发值、`Esc` 与脏输入都不发（§4.3「一次操作一条撤销」）。
    #[test]
    fn enter_commits_the_typed_value_and_escape_throws_it_away() {
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        let row = "inspector/style/size";
        // 真实路径里输入框握着焦点；没焦点的那一帧会按「点到别处」当作提交。
        let cell = FocusKey::indexed(row, 2);
        ctx.memory_mut(|m| m.request_focus(cell.id()));
        let mut edit = Some(StepperEdit {
            row: row.to_string(),
            draft: "48".into(),
            fresh: false,
            pinned: false,
        });
        let out = stepper_frame(&ctx, &mut focus, &mut edit, vec![key(Key::Enter)]);
        assert_eq!(out, Some(48.0), "回车提交键入的值");
        assert!(edit.is_none(), "提交后编辑态收起");

        ctx.memory_mut(|m| m.request_focus(cell.id()));
        let mut edit = Some(StepperEdit {
            row: row.to_string(),
            draft: "48".into(),
            fresh: false,
            pinned: false,
        });
        let out = stepper_frame(&ctx, &mut focus, &mut edit, vec![key(Key::Escape)]);
        assert_eq!(out, None, "Esc 不发命令");
        assert!(edit.is_none(), "Esc 之后编辑态也收起");

        ctx.memory_mut(|m| m.request_focus(cell.id()));
        let mut edit = Some(StepperEdit {
            row: row.to_string(),
            draft: "abc".into(),
            fresh: false,
            pinned: false,
        });
        let out = stepper_frame(&ctx, &mut focus, &mut edit, vec![key(Key::Enter)]);
        assert_eq!(out, None, "脏输入不发命令（值保持原样）");
        assert!(edit.is_none());
    }

    /// 分组开合：用户记过的优先；没记过就用组的默认（§4.1 标记组默认折叠）。
    #[test]
    fn group_state_prefers_what_the_user_toggled() {
        let stored: Vec<String> = vec!["style/color=0".into(), "mark/icons=1".into()];
        assert!(
            !group_open(&stored, "style/color", true),
            "用户折叠过就收着"
        );
        assert!(group_open(&stored, "mark/icons", false), "用户展开过就开着");
        assert!(group_open(&stored, "style/shape", true), "没记过走默认");
        assert!(!group_open(&stored, "mark/labels", false));
        // 键名是前缀关系也不能串：`mark/icon` 不该吃到 `mark/icons=1`。
        assert!(!group_open(&stored, "mark/icon", false));
        assert!(group_open(&[], "note/text", true));
    }

    #[test]
    fn subtree_styles_include_self_and_descendants_once() {
        let (mut doc, a, _b) = doc_with_two();
        doc.apply(&Command::AddNode {
            parent: a.clone(),
            index: None,
            text: "甲一".into(),
            style: None,
        })
        .unwrap();
        let ids: Vec<String> = subtree_styles(&doc, &a)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids.len(), 2, "自己 + 一个孩子");
        assert_eq!(ids[0], a, "先序从自己开始");
        assert_eq!(doc.node(&ids[1]).unwrap().text, "甲一");
    }

    fn doc_with_two() -> (Document, String, String) {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        for t in ["甲", "乙"] {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: t.into(),
                style: None,
            })
            .unwrap();
        }
        let a = doc
            .doc_order()
            .into_iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "甲"))
            .unwrap();
        let b = doc
            .doc_order()
            .into_iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "乙"))
            .unwrap();
        (doc, a, b)
    }

    fn inspector_with(doc: &Document, ids: &[String]) -> Inspector {
        let theme = get_theme("snow");
        let mut i = Inspector::new();
        i.sync(doc, &theme, ids);
        i
    }

    /// 换主题不改文档、也不改选择集，`sync` 会走早退分支——配色必须照样跟上，
    /// 否则墨夜下画布黑了、检查器还是白的。
    #[test]
    fn ink_follows_the_theme_even_when_nothing_else_changes() {
        let (doc, a, _) = doc_with_two();
        let ids = [a.clone()];
        let mut i = Inspector::new();
        i.sync(&doc, &get_theme("snow"), &ids);
        let light = (i.ink.ch.panel, i.ink.ch.text, i.ink.on);
        i.sync(&doc, &get_theme("ink"), &ids);
        let dark = (i.ink.ch.panel, i.ink.ch.text, i.ink.on);
        assert_ne!(light.0, dark.0, "面板底色要跟着主题换");
        assert_ne!(light.1, dark.1, "正文色要跟着主题换");
        assert_ne!(light.2, dark.2, "选中态强调色要按外观分流");
        // 再同步一次同一个主题：早退分支不该把配色改回去
        i.sync(&doc, &get_theme("ink"), &ids);
        assert_eq!(i.ink.ch.panel, dark.0);
    }

    #[test]
    fn style_edits_merge_into_each_nodes_own_override() {
        let (mut doc, a, b) = doc_with_two();
        // 乙已经有别的覆盖；改字号不能把它冲掉。
        doc.apply(&Command::SetStyle {
            ids: vec![b.clone()],
            style: Some(StyleOverride {
                fill: Some("#123456".into()),
                ..Default::default()
            }),
        })
        .unwrap();
        let inspector = inspector_with(&doc, &[a.clone(), b.clone()]);
        let Some(Action::Apply(Command::Batch { commands })) = inspector.style_cmd(|s| {
            s.font_size = Some(20);
        }) else {
            panic!("两个节点都应产生一条 SetStyle，并打包成 Batch");
        };
        assert_eq!(commands.len(), 2);
        for c in &commands {
            let Command::SetStyle { ids, style } = c else {
                panic!("只允许 SetStyle");
            };
            let style = style.as_ref().expect("应有覆盖");
            assert_eq!(style.font_size, Some(20));
            if ids[0] == b {
                assert_eq!(style.fill.as_deref(), Some("#123456"), "原有填充必须保留");
            } else {
                assert!(style.fill.is_none());
            }
        }
    }

    #[test]
    fn branch_color_is_a_per_node_override_that_clears_back_to_inheritance() {
        let (mut doc, a, _) = doc_with_two();
        let inspector = inspector_with(&doc, std::slice::from_ref(&a));
        let Some(Action::Apply(Command::SetStyle { style, .. })) =
            inspector.style_cmd(|s| s.branch_color = Some("#e4635b".into()))
        else {
            panic!("分支线色要产生一条 SetStyle");
        };
        let style = style.expect("应有覆盖");
        assert_eq!(style.branch_color.as_deref(), Some("#e4635b"));

        doc.apply(&Command::SetStyle {
            ids: vec![a.clone()],
            style: Some(style),
        })
        .unwrap();
        let inspector = inspector_with(&doc, std::slice::from_ref(&a));
        assert_eq!(
            inspector.items[0].style.branch_color.as_deref(),
            Some("#e4635b")
        );
        let Some(Action::Apply(Command::SetStyle { style, .. })) =
            inspector.style_cmd(|s| s.branch_color = None)
        else {
            panic!("恢复继承要发 SetStyle");
        };
        assert!(style.is_none(), "清掉唯一一项后应发 None");
    }

    #[test]
    fn clearing_the_last_field_removes_the_override_entirely() {
        let (mut doc, a, _) = doc_with_two();
        doc.apply(&Command::SetStyle {
            ids: vec![a.clone()],
            style: Some(StyleOverride {
                bold: Some(true),
                ..Default::default()
            }),
        })
        .unwrap();
        let inspector = inspector_with(&doc, &[a]);
        let Some(Action::Apply(Command::SetStyle { style, .. })) = inspector.style_cmd(|s| {
            s.bold = None;
        }) else {
            panic!("清掉唯一一项后要发 SetStyle");
        };
        assert!(style.is_none(), "空覆盖应当发 None，而不是 Some(default)");
    }

    #[test]
    fn mixed_state_is_none_and_uniform_state_is_some() {
        let (mut doc, a, b) = doc_with_two();
        doc.apply(&Command::SetStyle {
            ids: vec![a.clone()],
            style: Some(StyleOverride {
                font_size: Some(30),
                ..Default::default()
            }),
        })
        .unwrap();
        let inspector = inspector_with(&doc, &[a.clone(), b.clone()]);
        assert_eq!(
            same(inspector.items.iter().map(|i| i.resolved.font_size)),
            None,
            "30 和主题默认值不同，控件应显示混合态"
        );
        let inspector = inspector_with(&doc, &[a]);
        assert_eq!(
            same(inspector.items.iter().map(|i| i.resolved.font_size)),
            Some(30.0)
        );
    }

    #[test]
    fn selection_key_is_order_sensitive_because_primary_is_last() {
        assert_ne!(
            selection_key(&["a".into(), "b".into()]),
            selection_key(&["b".into(), "a".into()])
        );
        assert_eq!(
            selection_key(&["a".into(), "b".into()]),
            selection_key(&["a".into(), "b".into()])
        );
    }

    #[test]
    fn note_drafts_survive_revisions_but_follow_external_changes_when_clean() {
        let (mut doc, a, _) = doc_with_two();
        let theme = get_theme("snow");
        let mut inspector = Inspector::new();
        inspector.sync(&doc, &theme, std::slice::from_ref(&a));
        inspector.note_draft = "正在写的草稿".into();
        doc.apply(&Command::SetText {
            id: a.clone(),
            text: "甲改".into(),
        })
        .unwrap();
        inspector.sync(&doc, &theme, std::slice::from_ref(&a));
        assert_eq!(
            inspector.note_draft, "正在写的草稿",
            "未提交的草稿不能被 revision 冲掉"
        );

        inspector.note_draft = inspector.note_before.clone();
        doc.apply(&Command::SetNote {
            id: a.clone(),
            note: Some("外部写入".into()),
        })
        .unwrap();
        inspector.sync(&doc, &theme, std::slice::from_ref(&a));
        assert_eq!(inspector.note_draft, "外部写入", "干净草稿应跟随文档");
    }

    #[test]
    fn open_note_arms_note_tab_and_one_shot_focus() {
        let mut inspector = Inspector::new();
        assert!(matches!(inspector.tab, Tab::Style));
        inspector.focus_note();
        assert!(matches!(inspector.tab, Tab::Note));
        assert!(inspector.note_focus_pending, "下一帧应把焦点交给备注框");
    }

    #[test]
    fn link_host_accepts_http_and_rejects_everything_else() {
        assert_eq!(
            link_host("https://example.com/a?b#c").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            link_host("http://例子.测试/path").as_deref(),
            Some("例子.测试")
        );
        assert!(link_host("file:///C:/a.txt").is_none());
        assert!(link_host("javascript:alert(1)").is_none());
        assert!(link_host("   ").is_none());
    }

    #[test]
    fn theme_switch_syncs_session_theme_from_doc_meta() {
        // 这是画布 rebuild 的职责，但检查器的文档页依赖它：命令只改 meta。
        let mut doc = Document::new("根");
        doc.apply(&Command::SetTheme {
            theme: "mist".into(),
        })
        .unwrap();
        assert_eq!(doc.meta.theme, "mist");
        let theme = get_theme(&doc.meta.theme);
        assert_eq!(theme.id, "mist");
        let styles = resolve_all(&doc, &theme);
        assert!(styles.contains_key(&doc.root_id));
    }

    /// 面板的强调色镜像必须每帧跟着**生效主题**走：撤销强调色只改 meta，
    /// 而 `sync` 在 revision 未变时会早退——配色同步若写在早退之后就永远刷不上。
    #[test]
    fn the_accent_mirror_follows_the_effective_theme() {
        let (doc, _a, _b) = doc_with_two();
        let mut inspector = Inspector::new();
        let snow = get_theme("snow");
        inspector.sync(&doc, &snow, &[]);
        assert_eq!(inspector.accent, snow.ui.accent);
        assert_eq!(inspector.accent_chips.len(), ACCENT_HUES.len());

        let spun = snow.with_accent(Some(Rgba::parse("#e5484d").unwrap()));
        inspector.sync(&doc, &spun, &[]);
        assert_eq!(
            inspector.accent, spun.ui.accent,
            "换色相不改文档，色条也得跟上"
        );
        assert_eq!(inspector.accent_chips.len(), ACCENT_HUES.len());
    }

    #[test]
    fn a_drag_in_progress_keeps_its_accent_draft() {
        let (mut doc, a, _) = doc_with_two();
        let mut inspector = Inspector::new();
        inspector.sync(&doc, &get_theme("snow"), std::slice::from_ref(&a));
        inspector.accent_draft = Some(Color32::from_rgb(9, 9, 9));
        inspector.accent_dirty = true;
        // 拖色板的同时文档完全可能在动：切主题不改选中，revision 却已经涨了。
        doc.apply(&Command::SetTheme {
            theme: "mist".into(),
        })
        .unwrap();
        inspector.sync(&doc, &get_theme("mist"), std::slice::from_ref(&a));
        assert_eq!(
            inspector.accent_draft,
            Some(Color32::from_rgb(9, 9, 9)),
            "拖动中的取色草稿不能被同步冲掉"
        );
        inspector.accent_dirty = false;
        inspector.sync(&doc, &get_theme("mist"), std::slice::from_ref(&a));
        assert_eq!(inspector.accent_draft, None, "干净的草稿该让位给当前生效色");
    }

    /// 点一格存的是这格的 hex，而覆盖只借色相——两头必须长成同一个颜色，
    /// 否则色条就是「预览一个色、画布上另一个色」。
    #[test]
    fn a_chip_is_wysiwyg_after_it_goes_through_the_document() {
        for theme in themes() {
            for h in ACCENT_HUES {
                let chip = theme.accent_at_hue(h);
                let stored = Some(hex_of(chip));
                let painted = theme.with_accent(Rgba::parse_opt(&stored));
                for i in 0..3 {
                    assert!(
                        (chip.0[i] as i16 - painted.ui.accent.0[i] as i16).abs() <= 2,
                        "{} 在 {h:.0}° 的色条 {:?} 落到画布上变成 {:?}",
                        theme.name,
                        chip.0,
                        painted.ui.accent.0
                    );
                }
            }
        }
    }

    /// `docs/30` §4.2 文本域：「3 行起、超过 8 行可展开」。
    ///
    /// 三个数各管一件事：空备注给 3 行（够落笔）、折叠态封顶 8 行（不让一段长文把
    /// 整个内容页顶走）、展开态按实际行数（长文写作时不靠滚动条猜自己在第几行）。
    #[test]
    fn note_rows_starts_at_three_and_caps_at_eight() {
        assert_eq!(note_rows("", false), 3, "空备注也要有能落笔的高度");
        assert_eq!(note_rows("一行", false), 3, "一行字不该只占一行");
        assert_eq!(note_rows("一\n二\n三\n四", false), 4, "四行就给四行");
        assert_eq!(
            note_rows("一\n二\n三\n四", true),
            4,
            "没到 8 行时展开与折叠同高"
        );
        let twelve = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12";
        assert_eq!(note_rows(twelve, false), 8, "折叠态封顶 8 行");
        assert_eq!(note_rows(twelve, true), 12, "展开态按实际行数");
    }

    /// `docs/30` §4.2：已绑定的卡片要显示「标题 + 来源」。来源只有这四种说法，
    /// 它们对应 `NoteCard.source_doc` / `source_node` 的四种组合，界面不得自己编排文字。
    #[test]
    fn card_source_says_where_the_card_comes_from() {
        let (doc, a, _) = doc_with_two();
        let mut card = mw_core::NoteCard::new(doc.id.clone(), Some(a.clone()), "卡", "", 1);
        assert_eq!(card_source(&doc, &card, "主题"), "本文档 · 挂在主题上");
        card.source_node = None;
        assert_eq!(card_source(&doc, &card, "主题"), "本文档 · 独立卡片");
        card.source_doc = "另一个文档".into();
        assert_eq!(card_source(&doc, &card, "概念"), "来自其它文档");
        card.source_doc.clear();
        assert_eq!(card_source(&doc, &card, "概念"), "独立卡片");
    }

    /// 帧里画出来的所有文字（递归展开 `Shape::Vec`）。验收要判断「某一组到底有没有展开」，
    /// 而分组的开合状态不落在任何可断言的数据结构里——只能看它有没有把内容画出来。
    fn texts_in(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                egui::Shape::Vec(v) => {
                    for s in v {
                        walk(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for c in shapes {
            walk(&c.shape, &mut out);
        }
        out
    }

    /// `docs/30` §4.1「组内控件行高 28」：标签行必须真的 28。
    ///
    /// 这不是空断言：全局 `interact_size.y` 是 `design::control::INPUT = 30`，
    /// `ui.horizontal` 照它要高度，于是标签行 30 高、行距 38——SE-07 的金样里两颗
    /// 色点相距 38px，就是这么来的，肉眼上「标签一行怎么这么松」。
    /// 下半段用同内容量一遍 `ui.horizontal` 拿到 30，证明陷阱还在、这条测得出东西。
    #[test]
    fn content_rows_are_twenty_eight_high() {
        let ctx = egui::Context::default();
        ctx.all_styles_mut(|s| s.spacing.interact_size.y = mw_core::design::control::INPUT);
        let mut pinned = 0.0_f32;
        let mut plain = 0.0_f32;
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let body = |ui: &mut egui::Ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let _ = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                ui.label(egui::RichText::new("渲染").size(geo::ROW_TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
                });
            };
            pinned = content_row(ui, |ui| {
                body(ui);
                ui.min_rect().height()
            });
            plain = ui.horizontal(|ui| body(ui)).response.rect.height();
        });
        out.drop_without_applying_deltas();
        assert!(
            (pinned - geo::ROW_H).abs() < 0.01,
            "`content_row` 要给 28，实际 {pinned}"
        );
        assert!(
            (plain - mw_core::design::control::INPUT).abs() < 0.01,
            "`ui.horizontal` 本该被 interact_size 顶到 30（陷阱的依据），实际 {plain}"
        );
    }

    /// 无头跑一帧样式页：喂一个按键，把动作带回来。焦点跨帧复用，与真实壳层一致。
    fn press_style(
        ctx: &egui::Context,
        insp: &mut Inspector,
        doc: &Document,
        a: &str,
        focus: &mut FocusMemory,
        k: Key,
    ) -> Option<Action> {
        let ids = [a.to_string()];
        let mut action = None;
        ctx.run_ui(
            egui::RawInput {
                events: vec![key(k)],
                ..Default::default()
            },
            |ui| {
                action = insp.show(
                    ui,
                    doc,
                    &get_theme("snow"),
                    &ids,
                    focus,
                    Extras {
                        presets: &[],
                        annotation: None,
                        annotations: &[],
                        relationship: None,
                        relationships: &[],
                        groups: &[],
                        boxes: &[],
                    },
                );
            },
        )
        .drop_without_applying_deltas();
        action
    }

    /// `docs/30` §4.4：Esc **先退出当前控件**，第二下才把键盘交给画布。
    ///
    /// 只测判据本身：无头 harness 里 egui 的焦点在两次 `run_ui` 之间不保留，
    /// 摆一个「控件握着焦点」的现场再跨帧按 Esc 是拍不出来的（试过，第二帧焦点已经是 None）。
    /// 真正接线的两处——`surrender_focus` 与 `Action::FocusCanvas`——靠实机与 SE-10 的证据包复核。
    #[test]
    fn escape_goes_through_the_control_before_the_panel() {
        assert_eq!(
            esc_step(true, false),
            EscStep::Release,
            "焦点在控件上：先松它"
        );
        assert_eq!(esc_step(false, true), EscStep::Exit, "刚松过一次：退出面板");
        assert_eq!(
            esc_step(true, true),
            EscStep::Release,
            "又回到控件上，计数从头开始"
        );
        assert_eq!(
            esc_step(false, false),
            EscStep::Ignore,
            "面板没焦点就不吃这一下"
        );
    }

    /// `docs/31` §5 SE-09：检查器文字对面板底色 **≥ 4.5:1**（WCAG AA 正文）。
    #[test]
    fn every_theme_keeps_inspector_text_readable() {
        fn lin(c: u8) -> f32 {
            let s = c as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        fn lum(c: egui::Color32) -> f32 {
            0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
        }
        fn ratio(a: egui::Color32, b: egui::Color32) -> f32 {
            let (x, y) = (lum(a), lum(b));
            let (hi, lo) = if x > y { (x, y) } else { (y, x) };
            (hi + 0.05) / (lo + 0.05)
        }
        // 只量**真正用来写字的两级**：`text` 与 `dim`。检查器里已经不用 `faint` 写文字
        // （它在本轮被换成 `dim`——浅主题下 faint 对面板底色只有 2.37:1，不到 AA），
        // `faint` 只留给定线、色点和轨道底这些非文字的地方。
        let mut worst: Vec<(String, f32)> = Vec::new();
        let mut on_sunk: Vec<(String, f32)> = Vec::new();
        for id in ["snow", "ink", "mist", "sand"] {
            let ch = chrome_of(&get_theme(id));
            for (name, c) in [("text", ch.text), ("dim", ch.dim)] {
                worst.push((format!("{id}/{name}"), ratio(c, ch.panel)));
                // 下沉一层的面（卡片候选列表的底）单独记：浅主题下 `dim` 在这儿是 4.14，
                // 差 AA 一点点，已记进 `shots/x100/SE-09/report.md` 的已知缺口。
                on_sunk.push((format!("{id}/{name}@panel2"), ratio(c, ch.panel2)));
            }
        }
        let sunk_bad: Vec<&(String, f32)> = on_sunk.iter().filter(|(_, r)| *r < 4.0).collect();
        assert!(
            sunk_bad.is_empty(),
            "panel2 上的文字连 4.0:1 都不到了：{:?}",
            sunk_bad
                .iter()
                .map(|(n, r)| format!("{n}={r:.2}"))
                .collect::<Vec<_>>()
        );
        let bad: Vec<&(String, f32)> = worst.iter().filter(|(_, r)| *r < 4.5).collect();
        assert!(
            bad.is_empty(),
            "这些组合对面板底色低于 4.5:1：{:?}",
            bad.iter()
                .map(|(n, r)| format!("{n}={r:.2}"))
                .collect::<Vec<_>>()
        );
    }

    /// `docs/30` §4.4：色板里的**数字键 1–9 直选**、Enter 与空格同义。
    #[test]
    fn digits_pick_a_swatch_while_the_palette_holds_focus() {
        let (doc, a, _) = doc_with_two();
        let mut insp = inspector_with(&doc, std::slice::from_ref(&a));
        insp.tab = Tab::Style;
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        for _ in 0..40 {
            press_style(&ctx, &mut insp, &doc, &a, &mut focus, Key::Tab);
            // 数字键只在色板握着焦点时才吃键；换一处焦点再试，直到它出命令。
            if let Some(action) = press_style(&ctx, &mut insp, &doc, &a, &mut focus, Key::Num3) {
                assert!(
                    matches!(
                        action,
                        Action::Apply(Command::SetStyle { .. })
                            | Action::Apply(Command::Batch { .. })
                    ),
                    "数字键该发出一条改样式的命令"
                );
                return;
            }
        }
        panic!("Tab 走不到色板：数字键直选没有入口");
    }

    /// `docs/30` §4.2「作用域」：两段文字切换「本节点 / 含子树」，住在**分组标题右侧**。
    ///
    /// 改之前是样式页顶上单独一行「作用 / 当前节点 / 含子树」——位置与文案两处都不符。
    /// 这条直接画一帧，看标题行里有没有那两段、旧文案还在不在。
    #[test]
    fn the_scope_switch_lives_in_the_group_header() {
        let (doc, a, _) = doc_with_two();
        let mut insp = inspector_with(&doc, std::slice::from_ref(&a));
        insp.tab = Tab::Style;
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.show(
                ui,
                &doc,
                &get_theme("snow"),
                std::slice::from_ref(&a),
                &mut focus,
                Extras {
                    presets: &[],
                    annotation: None,
                    annotations: &[],
                    relationship: None,
                    relationships: &[],
                    groups: &[],
                    boxes: &[],
                },
            );
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        // 样式页有两个带作用域的组（颜色 / 形状与文字），两处都得画出来。
        // 这条同时钉住「两组的开关不能共用一个 egui id」——共用了第二处会拿不到命中。
        assert_eq!(
            texts.iter().filter(|t| *t == "本节点").count(),
            2,
            "两个样式组的标题右侧都要有「本节点」：{texts:?}"
        );
        assert_eq!(
            texts.iter().filter(|t| *t == "含子树").count(),
            2,
            "两个样式组的标题右侧都要有「含子树」：{texts:?}"
        );
        assert_ne!(
            FocusKey::dynamic("inspector/style/scope", "style/color").id(),
            FocusKey::dynamic("inspector/style/scope", "style/shape").id(),
            "两组的开关必须是两个 id，否则 egui 的命中会串"
        );
        assert!(
            !texts.iter().any(|t| t == "当前节点"),
            "旧文案「当前节点」该没有了：{texts:?}"
        );
        assert!(
            !texts.iter().any(|t| t == "作用"),
            "顶上那一行「作用」该搬进标题了：{texts:?}"
        );
    }

    /// §4.3：连点 stepper / 拖滑杆要在 500ms 内并成一条撤销，但**只并同一批 id 的样式编辑**。
    #[test]
    fn style_gestures_merge_only_on_the_same_ids_within_500ms() {
        let one = Command::SetStyle {
            ids: vec!["a".into()],
            style: None,
        };
        let two = Command::SetStyle {
            ids: vec!["a".into(), "b".into()],
            style: None,
        };
        let batch = Command::Batch {
            commands: vec![
                Command::SetStyle {
                    ids: vec!["b".into()],
                    style: None,
                },
                Command::SetStyle {
                    ids: vec!["a".into()],
                    style: None,
                },
            ],
        };
        assert_eq!(style_gesture_key(&one).as_deref(), Some("a"));
        assert_eq!(style_gesture_key(&two).as_deref(), Some("a,b"));
        // 作用域「含子树」发的是 Batch：id 集合与顺序无关，排序去重后同键。
        assert_eq!(style_gesture_key(&batch).as_deref(), Some("a,b"));
        assert_eq!(
            style_gesture_key(&Command::DeleteNodes {
                ids: vec!["a".into()]
            }),
            None,
            "删除不是样式手势"
        );
        assert_eq!(
            style_gesture_key(&Command::Batch {
                commands: vec![
                    one.clone(),
                    Command::DeleteNodes {
                        ids: vec!["b".into()]
                    }
                ]
            }),
            None,
            "混了别的命令就不合并"
        );

        assert!(same_gesture(Some("a"), 0, "a"), "同一批 id、立刻连点");
        assert!(same_gesture(Some("a"), 500, "a"), "500ms 本身还在窗内");
        assert!(!same_gesture(Some("a"), 501, "a"), "过了 500ms 就是两条");
        assert!(!same_gesture(Some("a"), 10, "a,b"), "换了作用域就不并");
        assert!(!same_gesture(None, 0, "a"), "上一件事不是样式编辑就不并");
    }

    /// `docs/30` §4.1：内容组的五组**默认展开**——打开内容页就该看见备注框、任务三态、
    /// 标签、链接、卡片，而不是一排收起的标题。
    ///
    /// 这条以前是反的（五组全 `false`），而分组默认值只是 `group()` 里的一个字面量，
    /// 没有任何数据结构能断言它，所以这里直接画一帧、看内容有没有出现在画面里。
    #[test]
    fn content_groups_are_open_by_default() {
        let (doc, a, _) = doc_with_two();
        let mut insp = inspector_with(&doc, std::slice::from_ref(&a));
        insp.tab = Tab::Note;
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.show(
                ui,
                &doc,
                &get_theme("snow"),
                std::slice::from_ref(&a),
                &mut focus,
                Extras {
                    presets: &[],
                    annotation: None,
                    annotations: &[],
                    relationship: None,
                    relationships: &[],
                    groups: &[],
                    boxes: &[],
                },
            );
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        assert!(
            texts.iter().any(|t| t.contains("写点备注")),
            "备注框默认要展开（§4.1 内容组默认展开），实际画出来：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "未开始"),
            "任务三态默认要展开，实际画出来：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("还没有标签")),
            "标签组默认要展开，实际画出来：{texts:?}"
        );
    }

    /// `docs/30` §4.2 / §4.5：标签是「纯文字 + 6px 色点」，不是胶囊 chip。
    ///
    /// 像素级判断在这儿做不了（没有合成器），但两件事能钉死：色点的**半径**（6px 直径
    /// → r=3 的实心圆）与**颜色来源**（`note_view::tag_color`，与卡片页/笔记库同一个函数），
    /// 以及标签确实是当纯文字画出来的。改回胶囊 chip 时文字与色点还在，这条**不会**变红——
    /// 那条要靠 `inspector-content` 基线人工看形状。
    #[test]
    fn labels_are_text_with_a_six_px_dot() {
        let (mut doc, a, _) = doc_with_two();
        for label in ["渲染", "性能"] {
            doc.apply(&Command::AddLabel {
                ids: vec![a.clone()],
                label: label.into(),
            })
            .unwrap();
        }
        let mut insp = inspector_with(&doc, std::slice::from_ref(&a));
        insp.tab = Tab::Note;
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.show(
                ui,
                &doc,
                &get_theme("snow"),
                std::slice::from_ref(&a),
                &mut focus,
                Extras {
                    presets: &[],
                    annotation: None,
                    annotations: &[],
                    relationship: None,
                    relationships: &[],
                    groups: &[],
                    boxes: &[],
                },
            );
        });
        let texts = texts_in(&out.shapes);
        fn count_dots(s: &egui::Shape, colors: &mut Vec<Color32>) {
            match s {
                egui::Shape::Circle(c) if (c.radius - 3.0).abs() < 0.01 => colors.push(c.fill),
                egui::Shape::Vec(v) => {
                    for s in v {
                        count_dots(s, colors);
                    }
                }
                _ => {}
            }
        }
        let mut colors = Vec::new();
        for c in &out.shapes {
            count_dots(&c.shape, &mut colors);
        }
        out.drop_without_applying_deltas();
        let want = [
            crate::note_view::tag_color("渲染"),
            crate::note_view::tag_color("性能"),
        ];
        let dots = colors.iter().filter(|c| want.contains(c)).count();
        assert_eq!(
            dots, 2,
            "两个标签各一颗 6px 色点（颜色走 `tag_color`），实际 {dots} 颗"
        );
        for label in ["渲染", "性能"] {
            assert!(
                texts.iter().any(|t| t == label),
                "标签 {label} 要作为纯文字画出来，实际：{texts:?}"
            );
        }
    }

    /// 标签输入行**不得比面板宽**（`docs/30` §4.1：默认 288、可拖 240–360）。
    ///
    /// SE-05 修过同类回归（形状四段写死 66×4 → 内容最小宽 292）。SE-07 又踩了一次：
    /// 输入框的 `desired_width` 只算文字区，左右 8+8 的内距另加，整行比可用宽多 16，
    /// 面板被顶到 316 且每帧 +2。这条把那条算术钉死：任意可用宽下，
    /// 输入框 + 按钮 + **真的**内距 + 行内间距 都要装得进这一行。
    ///
    /// 内距按 [`LABEL_MARGIN_X`] 现算而不是用 [`LABEL_PAD_X`]：两者一旦脱钩
    /// （有人把 `LABEL_PAD_X` 改小），这条会当场红，而不是像先前那版——
    /// 拿同一个常量算进去再拿它比较，恒等式怎么改都过。
    #[test]
    fn the_label_row_fits_the_panel() {
        let real_pad = LABEL_MARGIN_X as f32 * 2.0;
        for avail in [240.0_f32, 288.0, 360.0] {
            for gap in [0.0_f32, 4.0, 6.0, 8.0] {
                let field = label_field_width(avail - gap);
                let total = field + LABEL_BTN_W + real_pad + gap;
                assert!(
                    total <= avail + 0.01,
                    "可用宽 {avail}、间距 {gap} 时整行要 {total}：多出来的部分会把面板顶大"
                );
            }
        }
    }

    fn key(k: Key) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    /// 无头跑一帧：喂一个按键、画文档页、把动作带回来。`focus` 得是同一个实例
    /// 跨帧复用——真实壳层里 `FocusMemory` 随 `Shell` 长期存活，这里照搬。
    /// 测试里没有渲染后端，`textures_delta` 必须主动丢弃，否则 epaint 在析构时断言。
    fn press(
        ctx: &egui::Context,
        insp: &mut Inspector,
        doc: &Document,
        focus: &mut FocusMemory,
        k: Key,
    ) -> Option<Action> {
        let mut action = None;
        ctx.run_ui(
            egui::RawInput {
                events: vec![key(k)],
                ..Default::default()
            },
            |ui| {
                action = insp.doc_body(ui, doc, focus, &[]);
            },
        )
        .drop_without_applying_deltas();
        action
    }

    /// 色板是自定义绘制的圆盘，`Sense::click()` 在 egui 0.37 里含 FOCUSABLE，
    /// 所以 Tab 能走到、Space 会被当作一次主键点击——但这条链路只有跑起来才知道通不通。
    /// Tab 到第几步碰上色板取决于前面排了多少可聚焦控件，那个数字会随面板改动，
    /// 所以这里不写死步数：一路 Tab，每步试一次 Space，直到出现强调色命令。
    #[test]
    fn tab_walks_onto_the_accent_chips_and_space_applies_one() {
        let (doc, _, _) = doc_with_two();
        let mut insp = inspector_with(&doc, &[]);
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        for _ in 0..40 {
            press(&ctx, &mut insp, &doc, &mut focus, Key::Tab);
            if let Some(Action::Apply(Command::SetThemeAccent { accent: Some(hex) })) =
                press(&ctx, &mut insp, &doc, &mut focus, Key::Space)
            {
                assert!(
                    insp.accent_chips.iter().any(|c| hex_of(*c) == hex),
                    "空格发出去的强调色 {hex} 不在色板上"
                );
                return;
            }
        }
        panic!("Tab 走不到强调色色板：色盘键盘不可达");
    }

    /// 「主题默认」是内置按钮，空格同样该生效——发出去的必须是清空（`accent: None`），
    /// 而不是某一格的颜色。它只在已有覆盖时可点，所以这份文档得先带上覆盖。
    #[test]
    fn the_reset_button_clears_the_override_from_the_keyboard() {
        let (mut doc, _, _) = doc_with_two();
        doc.meta.theme_accent = Some("#e5484d".into());
        let mut insp = inspector_with(&doc, &[]);
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        for _ in 0..60 {
            press(&ctx, &mut insp, &doc, &mut focus, Key::Tab);
            if let Some(Action::Apply(Command::SetThemeAccent { accent: None })) =
                press(&ctx, &mut insp, &doc, &mut focus, Key::Space)
            {
                return;
            }
        }
        panic!("键盘走不到「主题默认」");
    }

    /// 5.3.3 / 5.3.4：同一格色点，鼠标点击与键盘 Space 必须返回同一个颜色——
    /// 两条路径共用 `resp.clicked()` 这一个出口，命令与撤销粒度因此不可能分叉。
    #[test]
    fn clicking_and_space_return_the_same_swatch() {
        let (doc, a, _) = doc_with_two();
        let insp = inspector_with(&doc, &[a]);
        let ink = insp.ink;
        let theme = get_theme("snow");
        let row = FocusKey::new(ROW_BRANCH);
        let target = FocusKey::indexed(ROW_BRANCH, 3);
        let ctx = egui::Context::default();
        let mut focus = FocusMemory::new();
        let mut current = None;

        // 帧 1：只画不点。色点 id 与屏幕矩形都从这一帧拿到。
        let mut hit = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            hit = swatches(
                ink,
                ui,
                &theme,
                &row,
                SwatchRow {
                    current: &mut current,
                    fallback: theme.ui.accent,
                    tip: "",
                    inheriting: true,
                },
                &mut focus,
            );
        })
        .drop_without_applying_deltas();
        assert!(hit.is_none(), "没点没按，不该产出颜色");
        let rect = ctx
            .read_response(target.id())
            .expect("色点应已登记进 egui")
            .rect;

        // 鼠标路径：在该格中心按下再抬起。
        let pos = rect.center();
        let mut mouse = None;
        ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                mouse = swatches(
                    ink,
                    ui,
                    &theme,
                    &row,
                    SwatchRow {
                        current: &mut current,
                        fallback: theme.ui.accent,
                        tip: "",
                        inheriting: true,
                    },
                    &mut focus,
                );
            },
        )
        .drop_without_applying_deltas();

        // 键盘路径：先把焦点放到同一格（控件上一帧已登记），再按 Space。
        current = None;
        ctx.memory_mut(|m| m.request_focus(target.id()));
        let mut kbd = None;
        ctx.run_ui(
            egui::RawInput {
                events: vec![key(Key::Space)],
                ..Default::default()
            },
            |ui| {
                kbd = swatches(
                    ink,
                    ui,
                    &theme,
                    &row,
                    SwatchRow {
                        current: &mut current,
                        fallback: theme.ui.accent,
                        tip: "",
                        inheriting: true,
                    },
                    &mut focus,
                );
            },
        )
        .drop_without_applying_deltas();

        let want = SwatchPick::Color(theme.palette[3]);
        assert_eq!(mouse, Some(want), "鼠标点第 4 格");
        assert_eq!(kbd, Some(want), "键盘按第 4 格");
        assert_eq!(mouse, kbd, "键鼠必须返回同一个颜色（5.3.3）");
    }

    /// 5.3.6：改色后由文档解析出的当前值跟着变，一次撤销就回到原值——
    /// 键盘与鼠标都走同一条 `SetStyle`，撤销粒度因此一致。
    #[test]
    fn a_swatch_change_is_one_undo_away_from_the_original_color() {
        let (mut doc, a, _) = doc_with_two();
        let before = inspector_with(&doc, std::slice::from_ref(&a));
        let old = before.items[0].resolved.branch_color;

        let Some(Action::Apply(cmd)) =
            before.style_cmd(|s| s.branch_color = Some("#e4635b".into()))
        else {
            panic!("改色应产生一条 SetStyle");
        };
        doc.apply(&cmd).unwrap();
        let applied = inspector_with(&doc, std::slice::from_ref(&a));
        assert_eq!(
            applied.items[0].style.branch_color.as_deref(),
            Some("#e4635b")
        );
        assert_ne!(
            applied.items[0].resolved.branch_color, old,
            "当前值要跟着变"
        );

        doc.undo();
        let undone = inspector_with(&doc, std::slice::from_ref(&a));
        assert_eq!(undone.items[0].style.branch_color, None);
        assert_eq!(undone.items[0].resolved.branch_color, old, "撤销回到原值");
    }

    // ---------------- C14：概念图的「命题」组与箭头键盘路径 ----------------

    fn find_by_text(doc: &Document, text: &str) -> String {
        doc.doc_order()
            .into_iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == text))
            .unwrap_or_else(|| panic!("夹具里没有「{text}」"))
    }

    /// 概念图夹具：甲（主概念）—导致→ 乙，丙 —支撑→ 甲。
    ///
    /// 返回（文档, 甲, 乙, 甲→乙 那条边的 id）。
    fn concept_doc_with_propositions() -> (Document, String, String, String) {
        let mut doc = Document::new("甲");
        doc.apply(&Command::SetLayout {
            layout: mw_core::model::LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let a = doc.root_id.clone();
        // `Document::new` 只设文档标题，根节点的正文是空的——行里要显示「甲」就得显式写。
        doc.apply(&Command::SetText {
            id: a.clone(),
            text: "甲".into(),
        })
        .unwrap();
        for (text, x, y) in [("乙", 320.0, 0.0), ("丙", 0.0, 320.0)] {
            doc.apply(&Command::AddFloatingTopic {
                text: text.into(),
                x,
                y,
            })
            .unwrap();
        }
        let b = find_by_text(&doc, "乙");
        let c = find_by_text(&doc, "丙");
        // 出去的：边挂在甲身上。进来的：边挂在丙身上、指向甲。
        doc.apply(&Command::AddRelationship {
            from: a.clone(),
            to: b.clone(),
            label: Some("导致".into()),
        })
        .unwrap();
        doc.apply(&Command::AddRelationship {
            from: c.clone(),
            to: a.clone(),
            label: Some("支撑".into()),
        })
        .unwrap();
        let rel = doc.node(&a).unwrap().relationships[0].id.clone();
        (doc, a, b, rel)
    }

    /// C14 ④：概念图里选中概念时，内容页要有「命题」组，出去/进来各列一行。
    ///
    /// 以前检查器里只有备注/标签/卡片——一颗概念牵了哪些命题只能回头看画布上的线。
    /// 思维导图里不许出现这一组（概念图语义不能漏到树里）。
    #[test]
    fn concept_maps_show_their_own_propositions() {
        let (doc, a, _b, _rel) = concept_doc_with_propositions();
        let mut insp = inspector_with(&doc, std::slice::from_ref(&a));
        insp.tab = Tab::Note;
        let mut focus = FocusMemory::new();
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.note_body(ui, &mut focus, &doc);
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        assert!(
            texts.iter().any(|t| t == "命题"),
            "要有「命题」组：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "出去 · 1"),
            "出去的命题要列出来：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "进来 · 1"),
            "进来的命题也要列出来：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "→ 导致 乙"),
            "出去的命题行要写出短语与目标：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "→ 支撑 丙"),
            "进来的命题行要写出短语与来源：{texts:?}"
        );
        // 摘要行也要有（分组标题右侧「出 1 · 入 1」）。
        assert!(
            texts.iter().any(|t| t == "出 1 · 入 1"),
            "分组标题要带摘要：{texts:?}"
        );

        // 反面：思维导图里不能长出这一组。
        let (mm, ma, _mb) = doc_with_two();
        let mut insp = inspector_with(&mm, std::slice::from_ref(&ma));
        insp.tab = Tab::Note;
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.note_body(ui, &mut focus, &mm);
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        assert!(
            !texts.iter().any(|t| t == "命题"),
            "思维导图不该有「命题」组：{texts:?}"
        );
    }

    /// C14 ⑥：关系线选中时数字键 1–4 直选箭头四档（`docs/30` §4.4「每个可改属性都有键盘路径」）。
    ///
    /// 四格以前只能用指针点，键盘用户改不了箭头方向。
    #[test]
    fn digit_keys_pick_the_arrow_head() {
        let (doc, a, b, rel) = concept_doc_with_propositions();
        let sel = crate::canvas::RelationshipSel {
            from: a.clone(),
            id: rel.clone(),
            to: b.clone(),
        };
        let theme = get_theme("snow");
        let ids = [a.clone()];
        // (按键, 期望的箭头)；`None` = 当前就是这一档，不该重复发命令。
        for (k, expect) in [
            (Key::Num3, Some(mw_core::model::ArrowHead::Backward)),
            (Key::Num1, Some(mw_core::model::ArrowHead::Off)),
            (Key::Num2, None),
            (Key::Num4, Some(mw_core::model::ArrowHead::Both)),
        ] {
            let mut insp = inspector_with(&doc, &ids);
            insp.tab = Tab::Note;
            let mut focus = FocusMemory::new();
            let ctx = egui::Context::default();
            let mut action = None;
            let out = ctx.run_ui(
                egui::RawInput {
                    events: vec![key(k)],
                    ..Default::default()
                },
                |ui| {
                    action = insp.show(
                        ui,
                        &doc,
                        &theme,
                        &ids,
                        &mut focus,
                        Extras {
                            presets: &[],
                            annotation: None,
                            annotations: &[],
                            relationship: Some(&sel),
                            relationships: &[],
                            groups: &[],
                            boxes: &[],
                        },
                    );
                },
            );
            out.drop_without_applying_deltas();
            match expect {
                Some(head) => match action {
                    Some(Action::Apply(Command::SetRelationshipHead { head: got, .. })) => {
                        assert_eq!(got, head, "{k:?} 该选 {head:?}")
                    }
                    other => panic!("{k:?} 没发出改箭头的命令：{other:?}"),
                },
                None => assert!(action.is_none(), "当前就是这一档，不该重发：{action:?}"),
            }
        }
    }

    /// C19 ①：概念图单选时对象行副行识别为「概念」，钉住时前置「已钉住」，显示命题数、卡片与标签。
    #[test]
    fn object_meta_for_concept_map_shows_concept_and_propositions_and_pinned() {
        let (mut doc, a, _b, _rel) = concept_doc_with_propositions();
        // 未钉住、2 条命题
        assert_eq!(object_meta(&doc, &a), "概念 · 2 条命题");

        // 钉住：前置「已钉住」
        doc.apply(&Command::SetConceptPinned {
            id: a.clone(),
            pinned: true,
        })
        .unwrap();
        assert_eq!(object_meta(&doc, &a), "已钉住 · 概念 · 2 条命题");

        // 挂卡片与加标签
        let card = mw_core::NoteCard::new(doc.id.clone(), None, "卡片", "正文", 0);
        doc.apply(&Command::AttachNewCard {
            node: a.clone(),
            card,
        })
        .unwrap();
        doc.apply(&Command::AddLabel {
            ids: vec![a.clone()],
            label: "重要".into(),
        })
        .unwrap();
        assert_eq!(
            object_meta(&doc, &a),
            "已钉住 · 概念 · 2 条命题 · 已挂卡片 · 1 个标签"
        );
    }

    /// C19 ②：概念图多选时副行显示「N 个概念」，根据钉住统计展示「M 个已钉住」或「全部已钉住」。
    #[test]
    fn object_meta_for_concept_map_multiselect_shows_concept_count_and_pinned() {
        let (mut doc, a, b, _rel) = concept_doc_with_propositions();
        let ids = [a.clone(), b.clone()];
        let theme = get_theme("snow");

        // 均未钉住
        let mut insp = inspector_with(&doc, &ids);
        let mut focus = FocusMemory::new();
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.show(
                ui,
                &doc,
                &theme,
                &ids,
                &mut focus,
                Extras {
                    presets: &[],
                    annotation: None,
                    annotations: &[],
                    relationship: None,
                    relationships: &[],
                    groups: &[],
                    boxes: &[],
                },
            );
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        assert!(
            texts.iter().any(|t| t == "2 个概念"),
            "未钉住时显示 2 个概念：{texts:?}"
        );

        // 1 个钉住
        doc.apply(&Command::SetConceptPinned {
            id: a.clone(),
            pinned: true,
        })
        .unwrap();
        let mut insp = inspector_with(&doc, &ids);
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.show(
                ui,
                &doc,
                &theme,
                &ids,
                &mut focus,
                Extras {
                    presets: &[],
                    annotation: None,
                    annotations: &[],
                    relationship: None,
                    relationships: &[],
                    groups: &[],
                    boxes: &[],
                },
            );
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        assert!(
            texts.iter().any(|t| t == "2 个概念 · 1 个已钉住"),
            "部分钉住显示统计：{texts:?}"
        );

        // 全部钉住
        doc.apply(&Command::SetConceptPinned {
            id: b.clone(),
            pinned: true,
        })
        .unwrap();
        let mut insp = inspector_with(&doc, &ids);
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.show(
                ui,
                &doc,
                &theme,
                &ids,
                &mut focus,
                Extras {
                    presets: &[],
                    annotation: None,
                    annotations: &[],
                    relationship: None,
                    relationships: &[],
                    groups: &[],
                    boxes: &[],
                },
            );
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        assert!(
            texts.iter().any(|t| t == "2 个概念 · 全部已钉住"),
            "全部钉住显示全部已钉住：{texts:?}"
        );
    }

    /// 概念图走**专属检查器**（`docs/33` §6），不再进思维导图那三页。
    ///
    /// 改前这条锁的是「`style_body` 里塞一个概念排布组」——那正是「专属 UI」
    /// 缺位的样子：概念图在树语义的面板里借一块地方。现在它有自己的三段，
    /// 树语义（本节点/含子树/文字样式应用到子树）连门都进不来。
    #[test]
    fn concept_map_inspector_shows_the_concept_panel_not_tree_groups() {
        let (doc, a, _b, _rel) = concept_doc_with_propositions();
        let ids = [a.clone()];
        let theme = get_theme("snow");
        let mut insp = inspector_with(&doc, &ids);
        insp.tab = Tab::Style;
        let mut focus = FocusMemory::new();
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            insp.show(
                ui,
                &doc,
                &theme,
                &ids,
                &mut focus,
                Extras {
                    presets: &[],
                    annotation: None,
                    annotations: &[],
                    relationship: None,
                    relationships: &[],
                    groups: &[],
                    boxes: &[],
                },
            );
        });
        let texts = texts_in(&out.shapes);
        out.drop_without_applying_deltas();
        // 专属三段：概念 / 命题 / 图。这里单选一颗概念，落「概念」段。
        assert!(
            texts.iter().any(|t| t == "概念图"),
            "概念图只走专属那一条：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "钉住位置"),
            "要有「钉住位置」开关：{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "形状"),
            "要有「形状」行：{texts:?}"
        );
        // 树语义一律不许出现。
        assert!(
            !texts.iter().any(|t| t == "本节点" || t == "含子树"),
            "概念图检查器不得出现「本节点 / 含子树」：{texts:?}"
        );
        assert!(
            !texts.iter().any(|t| t == "文字样式应用到子树"),
            "概念图不得出现「文字样式应用到子树」：{texts:?}"
        );
        // 旧的「概念排布」组被 §6 取代：钉住与整理都进了专属面板。
        assert!(
            !texts.iter().any(|t| t == "概念排布"),
            "「概念排布」已被 `docs/33` §6 取代，不该再出现：{texts:?}"
        );
    }

    /// C19 ④：钉住命令与撤销：单选与多选批量钉住均能一次撤销复原。
    #[test]
    fn concept_layout_pinned_command_and_undo() {
        let (mut doc, a, b, _rel) = concept_doc_with_propositions();

        // 1. 单选钉住
        assert!(!doc.is_concept_pinned(&a));
        doc.apply(&Command::SetConceptPinned {
            id: a.clone(),
            pinned: true,
        })
        .unwrap();
        assert!(doc.is_concept_pinned(&a));

        // 一次撤销
        doc.undo().unwrap();
        assert!(!doc.is_concept_pinned(&a));

        // 2. 批量钉住
        let batch_cmd = Command::Batch {
            commands: vec![
                Command::SetConceptPinned {
                    id: a.clone(),
                    pinned: true,
                },
                Command::SetConceptPinned {
                    id: b.clone(),
                    pinned: true,
                },
            ],
        };
        doc.apply(&batch_cmd).unwrap();
        assert!(doc.is_concept_pinned(&a));
        assert!(doc.is_concept_pinned(&b));

        // 一次撤销复原两者
        doc.undo().unwrap();
        assert!(!doc.is_concept_pinned(&a));
        assert!(!doc.is_concept_pinned(&b));
    }

    /// `docs/30` §4.1「组内行间距 6」：组内相邻两行之间是 6，不是外壳全局的 8。
    ///
    /// 这条测的是**画出来的间距**，不是常量：`group_list` 里放两行 28 高的组内行，
    /// 量第二行顶到第一行底的距离。上半段先把 `item_spacing` 摆成全局的 8（外壳
    /// `chrome::install_visuals` 用 `design::space::SM` 就是这个值），证明陷阱还在；
    /// 下半段钻进 `group_list` 量同一个距离，必须是 6。谁把 `group_list` 里那行
    /// `item_spacing.y = geo::ROW_GAP` 删了，这里当场变回 8。
    #[test]
    fn the_group_body_compresses_the_row_gap_to_six() {
        let ctx = egui::Context::default();
        let mut plain = 0.0_f32;
        let mut inside = 0.0_f32;
        let mut spacing_inside = f32::NAN;
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            // 外壳装好的全局行距：`design::space::SM`（8），不是 egui 的出厂值。
            ui.spacing_mut().item_spacing.y = mw_core::design::space::SM;
            let two_rows = |ui: &mut egui::Ui| {
                let (a, _) = ui.allocate_exact_size(Vec2::new(100.0, geo::ROW_H), Sense::hover());
                let (b, _) = ui.allocate_exact_size(Vec2::new(100.0, geo::ROW_H), Sense::hover());
                b.top() - a.bottom()
            };
            plain = two_rows(ui);
            inside = group_list(ui, |ui| {
                spacing_inside = ui.spacing().item_spacing.y;
                two_rows(ui)
            });
        });
        out.drop_without_applying_deltas();
        assert!(
            (plain - mw_core::design::space::SM).abs() < 0.01,
            "组外本该是全局 8（陷阱的依据），实际 {plain}"
        );
        assert!(
            (spacing_inside - geo::ROW_GAP).abs() < 0.01,
            "`group_list` 没把 item_spacing.y 压成 {}，实际 {spacing_inside}",
            geo::ROW_GAP
        );
        assert!(
            (inside - geo::ROW_GAP).abs() < 0.01,
            "组内两行之间要给 6，实际 {inside}"
        );
    }
}
