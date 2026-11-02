//! 内容表面组件库（CER-02）。
//!
//! 全屏大纲、卡片笔记页、笔记库共用的页面骨架与基础控件。几何与颜色**只**
//! 读 [`mw_core::design::content`] 令牌（`content_metrics(theme, dpi)` 派生），
//! 与 chrome 的 `ui_kit` 并列、互不借值——把这里的行高改成 28 不该动到菜单。
//!
//! 字重口径（CER-02 报告钉死的偏差，G-B2-1 复核）：egui 只注册了雅黑常规
//! 400 与 `zhisi-bold`（700 面）两档，docs/28 要求的 600 用 700 面呈现、
//! 500 用 400 面呈现；不合成粗体、不新增字体文件。
//!
//! 状态切换即时落位，不做透明度过渡：hover/选中动效属于页面层（docs/28 §4.5），
//! 与 golden 稳定性耦合，随页面包（CER-04 起）实现。

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontFamily, FontId, Frame, Id, Layout, Margin, Pos2, Rect,
    Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, UiBuilder, Vec2,
};
use mw_core::design::content::{self, ContentMetrics, TypeStyle};
use mw_core::theme::Rgba;

use crate::icon_paint::paint_icon_sized;
use crate::icons::Icon;

// ---------------------------------------------------------------------------
// 颜色与字体助手
// ---------------------------------------------------------------------------

/// `Rgba` → egui 颜色。**保留源 alpha**：内容令牌里 `text_secondary` /
/// `selected` / `hover` 等都靠 alpha 承载语义（70% / 7% / 4%），丢掉就会
/// 变成实心强调色。不透明令牌的 alpha 字节本来就是 255，结果不变。
fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

fn c32a(c: Rgba, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.0[0],
        c.0[1],
        c.0[2],
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// 字号 + 字重 → egui 字体。600 走 `zhisi-bold` 族（拉丁 Segoe Semibold + 中文粗体），
/// 500 走 `zhisi-semibold` 族（拉丁真 600 面，中文落回常规），400 走常规族。
pub fn font_for(style: TypeStyle) -> FontId {
    let family = match style.weight {
        600 => FontFamily::Name(crate::fonts::BOLD.into()),
        500 => FontFamily::Name(crate::fonts::SEMIBOLD.into()),
        _ => FontFamily::Proportional,
    };
    FontId::new(style.size, family)
}

/// 单行文本宽度（用与绘制相同的字体测量）。
fn text_width(ui: &egui::Ui, style: TypeStyle, s: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(s.to_owned(), font_for(style), Color32::WHITE)
        .size()
        .x
}

/// 在 `rect` 左上角绘制单行文本；rect 只作定位，不裁切。
fn draw_text(
    painter: &egui::Painter,
    anchor: Pos2,
    align: egui::Align2,
    style: TypeStyle,
    s: &str,
    color: Color32,
) {
    painter.text(anchor, align, s.to_owned(), font_for(style), color);
}

// ---------------------------------------------------------------------------
// ContentPage：页面骨架
// ---------------------------------------------------------------------------

/// 页面骨架参数。默认页头 40（大纲/卡片页）；库页头 48、大纲页脚 24 由
/// 调用方传入，组件不写第二份默认值。
pub struct PageSpec<'a> {
    pub id_salt: &'a str,
    pub header_h: f32,
    pub footer_h: f32,
}

/// 骨架拆分：给定总矩形与页头/页脚高，返回 `(页头, 内容, 页脚)` 三段矩形。
/// 内容高最低 0；页头/页脚之和超过总高时按页头优先、页脚截断。
pub fn split_page(total: Rect, header_h: f32, footer_h: f32) -> (Rect, Rect, Rect) {
    let header_h = header_h.clamp(0.0, total.height());
    let footer_h = footer_h.clamp(0.0, total.height() - header_h);
    let header = Rect::from_min_size(total.min, Vec2::new(total.width(), header_h));
    let footer = Rect::from_min_size(
        Pos2::new(total.min.x, total.max.y - footer_h),
        Vec2::new(total.width(), footer_h),
    );
    let content = Rect::from_min_max(
        Pos2::new(total.min.x, header.bottom()),
        Pos2::new(total.max.x, footer.top()),
    );
    (header, content, footer)
}

/// 「说明带」上下各留的白（概念图聚焦问题）。
///
/// 这个 7 与画布那条带子的 `pad.y`、导出那条的 `FOCUS_NOTE_PAD` 是同一个数：三处各写一份，
/// 屏幕上、导出的图里、大纲页里就会出现三种松紧。
pub const QUESTION_BAND_PAD: f32 = 7.0;

/// 内容列顶部的一条说明带（CM-10 ⑥ 的聚焦问题用）：只算几何，不画。
///
/// 带宽 = 页面宽减左右留白；留白比页面还宽时收到 0（负宽会让 egui 画出反向矩形）。
/// 带高 = 文字行高 + 上下各 `QUESTION_BAND_PAD`。
pub fn question_band(page: Rect, margin: f32, line_h: f32) -> Rect {
    let w = (page.width() - 2.0 * margin).max(0.0);
    Rect::from_min_size(
        Pos2::new(page.left() + margin, page.top()),
        Vec2::new(w, line_h + 2.0 * QUESTION_BAND_PAD),
    )
}
/// 一行「聚焦问题」的行内编辑框：画布那条带子与大纲页那条带子共用。
///
/// 两处放的是同一个控件——字号、内边距、占位提示、单行、无边框。各写一份的话，
/// 改一处就会漏掉另一处，和 `QUESTION_BAND_PAD` 那三个 7 是同一条理由。
/// `fill` 传 `Color32::TRANSPARENT` 就让底下的页面自己透出来（大纲页要的就是这个）。
pub fn question_field(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &mut String,
    ink: Color32,
    fill: Color32,
    id: Id,
) -> Response {
    ui.put(
        rect,
        TextEdit::singleline(text)
            .id(id)
            .font(FontId::proportional(content::READING_BODY.size))
            .text_color(ink)
            .hint_text("这张图要回答什么？")
            .frame(
                Frame::NONE
                    .fill(fill)
                    .inner_margin(Margin::symmetric(16, 7)),
            )
            .desired_width(rect.width()),
    )
}
/// 行内编辑框的结局。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestionEditOutcome {
    /// 还没完：编辑框继续立着。
    Continue,
    /// 关掉，但一个字都没改（或只改了空白）——不发命令，撤销栈里不留空记录。
    Cancel,
    /// 提交这份（已 trim、可能为空串 = 清空问题）。
    Commit(String),
}

/// 一行字写完没有：Esc 取消、Enter / 点走提交、没改就都不发命令。
///
/// 画布那条带子与大纲页那条带子共用这一个判定——同一条规则写两遍，迟早有一遍先改。
/// 比较用 **trim 后**的文本：只在末尾多敲一个空格不算改动。
pub fn question_edit_outcome(
    text: &str,
    opening: &str,
    esc: bool,
    done: bool,
) -> QuestionEditOutcome {
    if esc {
        return QuestionEditOutcome::Cancel;
    }
    if !done {
        return QuestionEditOutcome::Continue;
    }
    let (now, was) = (text.trim(), opening.trim());
    if now == was {
        QuestionEditOutcome::Cancel
    } else {
        QuestionEditOutcome::Commit(now.to_string())
    }
}

/// 页头 / 内容（内建垂直滚动）/ 页脚三段式页面。
///
/// 整页铺 `surface.page`；页头、页脚与内容之间各画一条 1px `divider`
/// （线宽取 `metrics.hairline`，1 物理像素）。滚动条由页面包自绘
/// （harness 下 egui 原生滚动条已收零宽），组件只负责滚动路径本身。
///
/// 三个子区的 `item_spacing` 一律清零：内容表面的垂直节奏全部由组件与
/// 页面代码用显式间距管理，不随调用方环境的 item_spacing 漂移。
pub fn content_page<R>(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    spec: PageSpec<'_>,
    header: impl FnOnce(&mut egui::Ui),
    body: impl FnOnce(&mut egui::Ui) -> R,
    footer: impl FnOnce(&mut egui::Ui),
) -> R {
    let total = ui.max_rect();
    ui.painter().rect_filled(total, 0.0, c32(m.colors.page));
    let (header_rect, content_rect, footer_rect) = split_page(total, spec.header_h, spec.footer_h);
    let line = Stroke::new(m.hairline, c32(m.colors.divider));
    if spec.header_h > 0.0 {
        ui.painter()
            .hline(header_rect.x_range(), header_rect.bottom(), line);
    }
    if spec.footer_h > 0.0 && footer_rect.top() < total.bottom() {
        ui.painter()
            .hline(footer_rect.x_range(), footer_rect.top(), line);
    }
    if spec.header_h > 0.0 {
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(format!("{}-header", spec.id_salt))
                .max_rect(header_rect),
        );
        child.spacing_mut().item_spacing = Vec2::ZERO;
        header(&mut child);
    }
    let mut body_child = ui.new_child(
        UiBuilder::new()
            .id_salt(format!("{}-body", spec.id_salt))
            .max_rect(content_rect),
    );
    body_child.spacing_mut().item_spacing = Vec2::ZERO;
    let out = ScrollArea::vertical()
        .id_salt(format!("{}-scroll", spec.id_salt))
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(&mut body_child, body)
        .inner;
    if spec.footer_h > 0.0 {
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(format!("{}-footer", spec.id_salt))
                .max_rect(footer_rect),
        );
        child.spacing_mut().item_spacing = Vec2::ZERO;
        footer(&mut child);
    }
    out
}

// ---------------------------------------------------------------------------
// FocusRing
// ---------------------------------------------------------------------------

/// 键盘焦点环：1.5px 强调色 80%，外偏移 1px。只在调用方声明"键盘焦点"时画，
/// 与鼠标悬停/选中是三种状态（docs/28 §4.6）。
pub fn focus_ring(painter: &egui::Painter, rect: Rect, m: &ContentMetrics, radius: f32) {
    let stroke = Stroke::new(m.focus_ring, c32a(m.colors.accent, m.focus_ring_alpha));
    painter.rect_stroke(
        rect.expand(m.focus_ring_offset),
        radius,
        stroke,
        StrokeKind::Outside,
    );
}

// ---------------------------------------------------------------------------
// PageTitle
// ---------------------------------------------------------------------------

/// 页面标题：按 `TypeStyle` 渲染单行文本，返回占位矩形。
/// 编辑行为（双击/F2）由页面包基于返回的矩形实现，组件只管排版。
pub fn page_title(ui: &mut egui::Ui, m: &ContentMetrics, style: TypeStyle, text: &str) -> Rect {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), font_for(style), c32(m.colors.text_primary));
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, Color32::WHITE);
    rect
}

// ---------------------------------------------------------------------------
// TextAction
// ---------------------------------------------------------------------------

/// 文字动作：13/20 `link` 色，hover 下划线，无底无框；禁用态 `text.faint`。
pub fn text_action(ui: &mut egui::Ui, m: &ContentMetrics, label: &str, enabled: bool) -> Response {
    let style = content::NAV;
    let w = text_width(ui, style, label).ceil();
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(w, style.line_height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let id = Id::unique(("content-text-action", ui.scope_id(), label));
    action_at(ui, m, id, rect, label, enabled)
}

/// 在指定矩形上呈现一个文字动作（供横幅等布局器复用）。
fn action_at(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    id: Id,
    rect: Rect,
    label: &str,
    enabled: bool,
) -> Response {
    let resp = ui
        .interact(
            rect,
            id,
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_text(label);
    let color = if enabled {
        c32(m.colors.link)
    } else {
        c32(m.colors.text_faint)
    };
    if enabled && resp.hovered() {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 1.0,
            Stroke::new(1.0, c32(m.colors.link)),
        );
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    draw_text(
        ui.painter(),
        Pos2::new(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        content::NAV,
        label,
        color,
    );
    resp
}

// ---------------------------------------------------------------------------
// InlineBanner
// ---------------------------------------------------------------------------

/// 行内提示条高度（docs/28 §6.7 / §7.7）。
pub const BANNER_H: f32 = 32.0;
/// 行内条左右内距。
pub const BANNER_PAD: f32 = 16.0;

/// 32px 行内提示条：13/20 `text.secondary` 文案 + 右侧若干文字动作；
/// 上下各一条 1px `divider`，无填充、无圆角、无图标。返回动作的 Response
/// （顺序与传入一致）。
pub fn inline_banner(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    message: &str,
    actions: &[&str],
) -> Vec<Response> {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, BANNER_H), Sense::hover());
    let line = Stroke::new(m.hairline, c32(m.colors.divider));
    let p = ui.painter();
    p.hline(rect.x_range(), rect.top(), line);
    p.hline(rect.x_range(), rect.bottom(), line);
    draw_text(
        p,
        Pos2::new(rect.left() + BANNER_PAD, rect.center().y),
        egui::Align2::LEFT_CENTER,
        content::NAV,
        message,
        c32(m.colors.text_secondary),
    );
    // 动作从右往左排，间距 16；返回时翻回传入顺序。
    let mut responses = Vec::with_capacity(actions.len());
    let mut right = rect.right() - BANNER_PAD;
    for label in actions.iter().rev() {
        let w = text_width(ui, content::NAV, label).ceil();
        let ar = Rect::from_min_size(
            Pos2::new(right - w, rect.center().y - content::NAV.line_height * 0.5),
            Vec2::new(w, content::NAV.line_height),
        );
        let id = Id::unique(("content-banner-action", message, label));
        responses.push(action_at(ui, m, id, ar, label, true));
        right = ar.left() - BANNER_PAD;
    }
    responses.reverse();
    responses
}

// ---------------------------------------------------------------------------
// EmptyState
// ---------------------------------------------------------------------------

/// 左对齐空状态：16/26/600 标题 + 13/20 说明（`text.secondary`）+ 可选一个
/// 文字动作。无图标、不居中（docs/28 §3.1 / §7.7）。返回动作 Response。
pub fn empty_state(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    title: &str,
    desc: &str,
    action: Option<&str>,
) -> Option<Response> {
    // 16/26/600 与 H4 同档，语义是"空状态标题"而非正文标题。
    let title_style = content::HEADING_3;
    let (trect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), title_style.line_height),
        Sense::hover(),
    );
    draw_text(
        ui.painter(),
        trect.min,
        egui::Align2::LEFT_TOP,
        title_style,
        title,
        c32(m.colors.text_primary),
    );
    ui.add_space(4.0);
    let (drect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), content::NAV.line_height),
        Sense::hover(),
    );
    draw_text(
        ui.painter(),
        drect.min,
        egui::Align2::LEFT_TOP,
        content::NAV,
        desc,
        c32(m.colors.text_secondary),
    );
    // 动作行在说明之后；Option<Response> 直接作为返回值。
    action.map(|label| {
        ui.add_space(8.0);
        text_action(ui, m, label, true)
    })
}

// ---------------------------------------------------------------------------
// SearchField
// ---------------------------------------------------------------------------

/// 搜索框高度（docs/28 §7.6）。
pub const SEARCH_H: f32 = 32.0;
/// 图标距左边框的内距。
pub const SEARCH_ICON_INSET: f32 = 10.0;
/// 图标边长。
pub const SEARCH_ICON: f32 = 14.0;
/// 图标与文本的间距。
pub const SEARCH_TEXT_GAP: f32 = 6.0;
/// 文本起点（相对框左）：内距 + 图标 + 间距。
pub const SEARCH_TEXT_X: f32 = SEARCH_ICON_INSET + SEARCH_ICON + SEARCH_TEXT_GAP;

/// 32px 搜索输入框：圆角 6、1px `hairline` 边（键盘焦点换焦点环）、
/// 左侧 14px 放大镜、13/20 文本。IME 由 egui 既有路径承接。
pub fn search_field(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    id_salt: &str,
    text: &mut String,
    placeholder: &str,
    width: f32,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, SEARCH_H), Sense::hover());
    let radius = content::radius::ROW;
    // 文本区从图标之后开始，避免放大镜压住首字。
    let edit_rect = Rect::from_min_max(
        Pos2::new(rect.left() + SEARCH_TEXT_X, rect.top()),
        Pos2::new(rect.right() - 8.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(format!("{id_salt}-edit"))
            .max_rect(edit_rect),
    );
    let resp = child.add(
        TextEdit::singleline(text)
            .id(Id::unique(id_salt))
            .font(font_for(content::NAV))
            .hint_text(
                RichText::new(placeholder.to_owned())
                    .size(content::NAV.size)
                    .color(c32(m.colors.text_faint)),
            )
            .desired_width(edit_rect.width())
            .frame(Frame::NONE)
            .margin(Margin::symmetric(0, 6)),
    );
    let p = ui.painter();
    paint_icon_sized(
        p,
        Pos2::new(
            rect.left() + SEARCH_ICON_INSET + SEARCH_ICON * 0.5,
            rect.center().y,
        ),
        SEARCH_ICON,
        Icon::Search,
        c32(if resp.has_focus() {
            m.colors.text_secondary
        } else {
            m.colors.text_faint
        }),
    );
    if resp.has_focus() {
        focus_ring(p, rect, m, radius);
    } else {
        p.rect_stroke(
            rect,
            radius,
            Stroke::new(m.hairline, c32(m.colors.hairline)),
            StrokeKind::Inside,
        );
    }
    resp
}

// ---------------------------------------------------------------------------
// TagText
// ---------------------------------------------------------------------------

/// 标签圆点直径（docs/28 §4.3：仅 6px）。
pub const TAG_DOT: f32 = 6.0;
/// 圆点与文字的间距。
pub const TAG_DOT_GAP: f32 = 6.0;
/// 标签左右内距（hover 底的呼吸空间）。
pub const TAG_PAD: f32 = 4.0;

/// 13/20 纯文字标签，可选 6px 圆点前缀；hover：6% 中性底 + 下划线。
/// 返回 Response 由调用方接跳转/删除；禁止彩色 chip。
pub fn tag_text(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    label: &str,
    dot: Option<Color32>,
    enabled: bool,
) -> Response {
    let style = content::NAV;
    let tw = text_width(ui, style, label);
    let dot_w = if dot.is_some() {
        TAG_DOT + TAG_DOT_GAP
    } else {
        0.0
    };
    let w = (TAG_PAD * 2.0 + dot_w + tw).ceil();
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(w, style.line_height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(
            rect,
            content::radius::ROW,
            c32a(m.colors.text_primary, 0.06),
        );
        let text_left = rect.left() + TAG_PAD + dot_w;
        p.hline(
            text_left..=(text_left + tw),
            rect.bottom() - 1.0,
            Stroke::new(1.0, c32(m.colors.link)),
        );
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if let Some(d) = dot {
        p.circle_filled(
            Pos2::new(rect.left() + TAG_PAD + TAG_DOT * 0.5, rect.center().y),
            TAG_DOT * 0.5,
            d,
        );
    }
    let text_left = rect.left() + TAG_PAD + dot_w;
    draw_text(
        p,
        Pos2::new(text_left, rect.center().y),
        egui::Align2::LEFT_CENTER,
        style,
        label,
        c32(m.colors.text_secondary),
    );
    resp
}

// ---------------------------------------------------------------------------
// 页头图标按钮 / MoreButton
// ---------------------------------------------------------------------------

/// 页头图标按钮的命中边长（docs/28 §5.1：控件 28×28，图标 16）。
pub const PAGE_ICON_BUTTON: f32 = 28.0;
/// 页头图标边长。
pub const PAGE_ICON: f32 = 16.0;

/// 28×28 图标按钮：hover `state.hover` 底 + `text.primary` 图标；
/// `selected` 给内容选中底（`state.selected`）。
pub fn page_icon_button(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    icon: Icon,
    tip: &str,
    selected: bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(PAGE_ICON_BUTTON), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, content::radius::ROW, c32(m.colors.selected));
    }
    if resp.hovered() {
        p.rect_filled(rect, content::radius::ROW, c32(m.colors.hover));
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let color = if resp.hovered() {
        c32(m.colors.text_primary)
    } else {
        c32(m.colors.text_secondary)
    };
    paint_icon_sized(p, rect.center(), PAGE_ICON, icon, color);
    resp.on_hover_text(tip).clicked()
}

/// 16px `⋯` 更多按钮：页头/行尾"按需出现"动作的统一入口。
pub fn more_button(ui: &mut egui::Ui, m: &ContentMetrics, tip: &str) -> bool {
    page_icon_button(ui, m, Icon::More, tip, false)
}

// ---------------------------------------------------------------------------
// 陈列页（MW_CONTENT_KIT=1，仅供 content-kit golden / 人工复核）
// ---------------------------------------------------------------------------

/// 各节固定行高下的 hover 脚本锚点：正文列左缘、各节 y（见 `gallery_body`）。
/// y 值按基线 PNG 实测校准（1200×860 @ppp1.5，基线锁定后不得再改布局，
/// 否则 hover 脚本要重新校准）。只被 golden 输入脚本与本模块测试引用，
/// 不参与绘制，故放行 dead_code。
#[allow(dead_code)]
pub mod gallery_anchors {
    /// 正文列左缘 x（1200 窗、大纲版心时）。
    pub const COL_X: f32 = 200.0;
    /// 列宽（正文列 760 的可交互范围）。
    pub const COL_W: f32 = 760.0;
    /// S3 标签行中心 y（实测基线）。
    pub const TAG_ROW_Y: f32 = 365.0;
    /// S4 动作行中心 y（实测基线）。
    pub const ACTION_ROW_Y: f32 = 432.0;
    /// S4 更多按钮中心 x（实测基线）。
    pub const MORE_X: f32 = 985.0;
}

/// 组件陈列页：九组件按节平铺，`content-kit` golden 的被摄体。
/// 不读时钟、不滚动（内容按 1200×860 设计），全部状态静态可拍。
pub fn show_gallery(ui: &mut egui::Ui) {
    let theme_id = std::env::var("MW_THEME").unwrap_or_else(|_| "snow".to_owned());
    let theme = mw_core::theme::get_theme(&theme_id);
    let m = content::content_metrics(&theme, ui.ctx().pixels_per_point());
    content_page(
        ui,
        &m,
        PageSpec {
            id_salt: "content-kit",
            header_h: 40.0,
            footer_h: 24.0,
        },
        |ui| gallery_header(ui, &m),
        |ui| gallery_body(ui, &m),
        |ui| gallery_footer(ui, &m),
    );
}

fn gallery_header(ui: &mut egui::Ui, m: &ContentMetrics) {
    ui.painter().text(
        Pos2::new(ui.max_rect().left() + 16.0, ui.max_rect().center().y),
        egui::Align2::LEFT_CENTER,
        "组件陈列 · 内容设计系统".to_owned(),
        font_for(content::NAV),
        c32(m.colors.text_secondary),
    );
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(6.0);
        if more_button(ui, m, "更多") {
            // 陈列页不接动作。
        }
        if page_icon_button(ui, m, Icon::Search, "搜索", false) {}
    });
}

fn gallery_footer(ui: &mut egui::Ui, m: &ContentMetrics) {
    ui.painter().text(
        Pos2::new(ui.max_rect().left() + 16.0, ui.max_rect().center().y),
        egui::Align2::LEFT_CENTER,
        "织思 · 内容设计系统 · CER-02".to_owned(),
        font_for(content::TAG),
        c32(m.colors.text_secondary),
    );
    ui.painter().text(
        Pos2::new(ui.max_rect().right() - 16.0, ui.max_rect().center().y),
        egui::Align2::RIGHT_CENTER,
        "?".to_owned(),
        font_for(content::NAV),
        c32(m.colors.text_secondary),
    );
}

/// 一节 = 12/18/600 小节标题 + 6px 间距 + 内容。
fn section_caption(ui: &mut egui::Ui, m: &ContentMetrics, label: &str) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), content::TAG.line_height),
        Sense::hover(),
    );
    draw_text(
        ui.painter(),
        rect.min,
        egui::Align2::LEFT_TOP,
        content::TAG,
        label,
        c32(m.colors.text_faint),
    );
    ui.add_space(6.0);
}

fn gallery_body(ui: &mut egui::Ui, m: &ContentMetrics) {
    let col = content::OUTLINE.column(ui.available_width());
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(col.side);
        ui.vertical(|ui| {
            ui.set_width(col.page);

            // S1 页面标题两档：30/40 与 28/38
            section_caption(ui, m, "页面标题 PAGE_TITLE 30/40 · OUTLINE_TITLE 28/38");
            page_title(ui, m, content::PAGE_TITLE, "把知识变成自己的");
            ui.add_space(4.0);
            page_title(ui, m, content::OUTLINE_TITLE, "纯 Rust 原生");
            ui.add_space(16.0);

            // S2 搜索框：空（占位）与已填充
            section_caption(ui, m, "搜索框 SearchField 32px");
            let mut empty = String::new();
            search_field(
                ui,
                m,
                "content-kit-search-empty",
                &mut empty,
                "搜索标题、正文、标签…",
                280.0,
            );
            ui.add_space(6.0);
            let mut filled = String::from("间隔重复");
            search_field(
                ui,
                m,
                "content-kit-search-filled",
                &mut filled,
                "搜索标题、正文、标签…",
                280.0,
            );
            ui.add_space(16.0);

            // S3 标签：六色圆点 + 无点（hover 锚点 TAG_ROW_Y）
            section_caption(ui, m, "标签 TagText 13/20 + 6px 圆点");
            ui.horizontal(|ui| {
                for (i, label) in ["知识", "方法", "整理术", "写作", "卡片", "大纲"]
                    .into_iter()
                    .enumerate()
                {
                    tag_text(
                        ui,
                        m,
                        &format!("#{label}"),
                        Some(c32(m.colors.tag_dots[i])),
                        true,
                    );
                    ui.add_space(8.0);
                }
                tag_text(ui, m, "#无点", None, true);
            });
            ui.add_space(16.0);

            // S4 动作与更多（hover 锚点 ACTION_ROW_Y / MORE_X）
            section_caption(ui, m, "文字动作 TextAction · 更多 MoreButton 28×28");
            ui.horizontal(|ui| {
                text_action(ui, m, "导入…", true);
                ui.add_space(16.0);
                text_action(ui, m, "重试", true);
                ui.add_space(16.0);
                text_action(ui, m, "不可用", false);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.set_width(ui.available_width());
                    more_button(ui, m, "更多");
                });
            });
            ui.add_space(16.0);

            // S5 行内提示条两条
            section_caption(ui, m, "行内提示条 InlineBanner 32px");
            inline_banner(ui, m, "文件已在外部修改。", &["重新加载", "覆盖", "取消"]);
            ui.add_space(6.0);
            inline_banner(ui, m, "同步失败，稍后重试。", &[]);
            ui.add_space(16.0);

            // S6 空状态
            section_caption(ui, m, "空状态 EmptyState 左对齐");
            empty_state(
                ui,
                m,
                "还没有卡片",
                "在导图节点上按 Ctrl+Shift+N 新建，或从文件导入",
                Some("导入…"),
            );
            ui.add_space(16.0);

            // S7 焦点环样张：选中底 7% + 焦点环 1.5px
            section_caption(ui, m, "键盘焦点环 FocusRing 1.5px · 外偏移 1px");
            let (row, _) = ui.allocate_exact_size(Vec2::new(320.0, 32.0), Sense::hover());
            ui.painter()
                .rect_filled(row, content::radius::ROW, c32(m.colors.selected));
            draw_text(
                ui.painter(),
                Pos2::new(row.left() + 12.0, row.center().y),
                egui::Align2::LEFT_CENTER,
                content::NAV,
                "键盘选中行样张",
                c32(m.colors.text_primary),
            );
            focus_ring(ui.painter(), row, m, content::radius::ROW);
        });
    });
    ui.add_space(16.0);
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // 组件几何是刻意钉死的运行期不变量（与 docs/28 §4/§5/§7 对表）。
    #![allow(clippy::assertions_on_constants)]

    use super::*;
    use mw_core::design::content as ck;
    use mw_core::theme::get_theme;

    #[test]
    fn question_edit_outcome_says_cancel_continue_or_commit() {
        use QuestionEditOutcome::{Cancel, Commit, Continue};
        // 还在打字。
        assert_eq!(
            question_edit_outcome("新问题", "旧问题", false, false),
            Continue
        );
        // Esc 优先：哪怕同一个键也点了提交，也算取消。
        assert_eq!(
            question_edit_outcome("新问题", "旧问题", true, true),
            Cancel
        );
        // 一个字没改 / 只多敲了空格：都不发命令。
        assert_eq!(
            question_edit_outcome("旧问题", "旧问题", false, true),
            Cancel
        );
        assert_eq!(
            question_edit_outcome("旧问题  ", "旧问题", false, true),
            Cancel
        );
        // 真改了：提交 trim 后的版本。
        assert_eq!(
            question_edit_outcome("  新问题 ", "旧问题", false, true),
            Commit("新问题".to_string())
        );
        // 清空 = 提交空串，由调用方转成 `None`（Command 的归一化）。
        assert_eq!(
            question_edit_outcome("   ", "旧问题", false, true),
            Commit(String::new())
        );
    }

    #[test]
    fn question_band_sits_in_the_column_and_keeps_seven_above_and_below() {
        let page = Rect::from_min_size(Pos2::new(0.0, 41.0), Vec2::new(1200.0, 700.0));
        let band = question_band(page, 200.0, 22.0);
        assert_eq!(band.left(), 200.0);
        assert_eq!(band.width(), 800.0);
        assert_eq!(band.top(), page.top());
        assert_eq!(band.height(), 22.0 + 2.0 * QUESTION_BAND_PAD);

        // 窄窗：留白比页面还宽时收到 0，不许出负宽。
        let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 300.0));
        assert_eq!(question_band(narrow, 32.0, 22.0).width(), 0.0);
    }

    #[test]
    fn split_page_sums_to_total_and_clamps_overflow() {
        let total = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 860.0));
        let (h, c, f) = split_page(total, 40.0, 24.0);
        assert_eq!(h.height(), 40.0);
        assert_eq!(f.height(), 24.0);
        assert_eq!(c.height(), 860.0 - 64.0);
        assert_eq!(c.top(), h.bottom());
        assert_eq!(c.bottom(), f.top());
        assert_eq!(h.width() + 0.0, total.width());

        // 页头/页脚之和超过总高：页头优先，内容高 0，页脚截断。
        let tiny = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 50.0));
        let (h, c, f) = split_page(tiny, 40.0, 24.0);
        assert_eq!(h.height(), 40.0);
        assert_eq!(c.height(), 0.0);
        assert_eq!(f.height(), 10.0);

        // 负值按 0 处理。
        let (h, _c, f) = split_page(total, -5.0, -1.0);
        assert_eq!(h.height(), 0.0);
        assert_eq!(f.height(), 0.0);
    }

    #[test]
    fn search_field_geometry_matches_contract() {
        assert_eq!(SEARCH_H, 32.0, "docs/28 §7.6：搜索框 32px");
        assert_eq!(SEARCH_ICON, 14.0, "图标 14px");
        assert_eq!(SEARCH_ICON_INSET, 10.0, "内距 10");
        assert_eq!(SEARCH_TEXT_X, 30.0, "文本起点 = 10 + 14 + 6");
    }

    #[test]
    fn banner_and_tag_rows_match_contract() {
        assert_eq!(BANNER_H, 32.0, "行内提示条 32px");
        assert_eq!(content::NAV.line_height, 21.0);
        assert!(
            (BANNER_H - content::NAV.line_height) * 0.5 >= 4.0,
            "文案上下各留至少 4px"
        );
        assert_eq!(TAG_DOT, 6.0, "标签圆点仅 6px");
        assert_eq!(content::TAG.line_height, 18.0);
    }

    #[test]
    fn focus_ring_matches_content_tokens() {
        let theme = get_theme("snow");
        let m = ck::content_metrics(&theme, 1.5);
        assert!((m.focus_ring - ck::focus::RING).abs() < 1e-6);
        assert!((m.focus_ring_offset - ck::focus::OFFSET).abs() < 1e-6);
        assert!((m.focus_ring_alpha - ck::focus::ALPHA).abs() < 1e-6);
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(100.0, 32.0));
        let expected = rect.expand(m.focus_ring_offset);
        assert_eq!(expected.left(), 9.0, "外偏移 1px");
        assert_eq!(expected.width(), 102.0);
    }

    #[test]
    fn font_for_maps_each_weight_to_its_own_family() {
        // 400 常规；500 走 zhisi-semibold（拉丁真 600 面、中文常规）；600 走 zhisi-bold。
        for style in ck::TYPE_SCALE {
            let font = font_for(style);
            match style.weight {
                600 => assert!(
                    matches!(font.family, FontFamily::Name(ref n) if &**n == crate::fonts::BOLD),
                    "{} 应走 zhisi-bold 族",
                    style.size
                ),
                500 => assert!(
                    matches!(font.family, FontFamily::Name(ref n) if &**n == crate::fonts::SEMIBOLD),
                    "{} 应走 zhisi-semibold 族",
                    style.size
                ),
                400 => assert_eq!(font.family, FontFamily::Proportional),
                other => panic!("内容字阶不允许字重 {other}"),
            }
            assert_eq!(font.size, style.size);
        }
    }

    #[test]
    fn empty_state_and_gallery_type_matches_docs28() {
        // 空状态：16/26/600 标题 + 14/21 说明（docs/28 §7.7，D2 改 14/21）。
        assert_eq!(content::HEADING_3.size, 16.0);
        assert_eq!(content::HEADING_3.line_height, 26.0);
        assert_eq!(content::HEADING_3.weight, 600);
        assert_eq!(content::NAV.size, 14.0);
        // 版心：1200 窗下大纲列居中 800。
        let col = content::OUTLINE.column(1200.0);
        assert_eq!(col.page, 800.0);
        assert_eq!(col.side, 200.0);
        // 陈列页锚点与正文列一致（hover 脚本依赖）。
        assert_eq!(gallery_anchors::COL_X, 200.0);
        assert_eq!(gallery_anchors::COL_X + gallery_anchors::COL_W, 960.0);
    }

    #[test]
    fn page_icon_button_and_more_geometry_match_outline_header() {
        assert_eq!(PAGE_ICON_BUTTON, 28.0, "docs/28 §5.1：控件 28×28");
        assert_eq!(PAGE_ICON, 16.0, "docs/28 §5.1：图标 16");
        assert!(PAGE_ICON < PAGE_ICON_BUTTON);
    }
}
