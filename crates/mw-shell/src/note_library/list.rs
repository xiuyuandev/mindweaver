//! 列表栏（`docs/28` §7.4 / §7.6 / §7.7）：60px 行的目录、搜索、筛选 chip、批量条。
//!
//! 三条硬规矩：
//! 1. **行没有卡片外形**——不铺白底、不圆角、不加阴影，只有 1px 分隔线（内缩 16）。
//!    旧版每行一张圆角白卡，一万行就是一万张贴纸，§7.9 点名禁止。
//! 2. **虚拟化**：`ScrollArea::show_rows` 只画可视区那十几行；行序是 `cards` 的下标
//!    （`lib.rows`），一万张也不会每帧克隆元数据。
//! 3. **两种选中语义，不多不少**（§7.9）：焦点行（预览跟着走，7% 强调底）与
//!    勾选行（选择模式下的 16px 复选框）。旧版把「当前行」和「勾中行」画成同一个
//!    底色，批量操作到底作用在谁身上看不出来。
//!
//! 行内交互一律先在绘制里收集成 [`RowEvt`]，出闭包再落到 `lib`：`show_rows` 的闭包
//! 只能拿 `&NoteLibrary`，而 `click_row` / `is_double_click` 要 `&mut`。

use eframe::egui::{
    self, scroll_area::ScrollBarVisibility, Align2, Color32, Id, Modifiers, Pos2, Rect, Response,
    ScrollArea, Sense, Stroke, StrokeKind, UiBuilder, Vec2,
};
use mw_core::design::content::{self, ContentMetrics, TypeStyle};
use mw_core::note::{CardScope, CardSort};

use super::{
    c32, c32a, draw_line, elide, match_ranges, scrollbar, tag_dot, text_w, LibraryAction,
    NoteLibrary, Pick, Seg, Zone, BATCH_BAR_H, CARD_ROW_H, CHECK_BOX, CHECK_GAP, CHIP_ROW_H,
    DIVIDER_INSET, EMPTY_LIBRARY, EMPTY_LIBRARY_DESC, HIT_ALPHA, LIST_PAD, NO_RESULTS,
    SEARCH_PLACEHOLDER, SIDE_TEXT, SORT_ROW_H,
};
use crate::content_kit;
use crate::icon_paint::paint_icon_sized;
use crate::icons::Icon;
use crate::menu::{Entry, Menu};
use crate::note_store::CardMeta;
use crate::note_view::format_date;

/// 搜索框上方的呼吸（§7.4 只钉了「顶部搜索 32px」，上下留白由这里给）。
const HEAD_TOP: f32 = 10.0;
const HEAD_BOTTOM: f32 = 6.0;
/// 列表底部那条短反馈的高度。
const HINT_H: f32 = 26.0;
/// 加标签行内输入占的高度（含上下留白）。
const TAG_INPUT_H: f32 = 42.0;
/// chip 的可视高度（行高 28，chip 本身 20，居中）。
const CHIP_H: f32 = 20.0;
const CHIP_GAP: f32 = 6.0;
/// chip 右侧那颗 ×。
const CHIP_X: f32 = 10.0;
/// chip 文字左右内距。
const CHIP_INNER: f32 = 6.0;
/// 复选框圆角。
const CHECK_R: f32 = 3.0;
/// 排序按钮右侧的 chevron。
const SORT_CHEV: f32 = 12.0;
/// 批量条按钮之间的间距。
const BAR_GAP: f32 = 16.0;
/// 焦点环相对行的内缩：贴着分隔线画会让上下两行的环连成一片。
const RING_INSET_X: f32 = 6.0;
const RING_INSET_Y: f32 = 2.0;
/// §7.7「加载中：3 行 × 60px 骨架，不转菊花」。
const SKELETON_ROWS: usize = 3;
/// 骨架条：(高, 占宽比例)。三根长短不一才看得出「这是占位」而不是「这是内容」。
const SKELETON_BARS: [(f32, f32); 3] = [(10.0, 0.62), (8.0, 0.86), (8.0, 0.34)];
/// 骨架条之间的竖向间距。
const SKELETON_GAP: f32 = 6.0;
/// meta 行里各段之间的间距。
const META_GAP: f32 = 8.0;
/// 空标题的占位。
const UNTITLED: &str = "无标题";
/// 清掉标签筛选与搜索词的文字动作。
const CLEAR_FILTERS: &str = "清除筛选";

/// 一行的交互意图。
enum RowEvt {
    /// 单击（含 `Ctrl` / `Shift`）。是否算双击由出闭包后统一判。
    Click(usize, Modifiers),
    /// 右键开行菜单，`Pos2` 是菜单锚点。
    Menu(usize, Pos2),
}

pub(super) fn paint_list(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    lib: &mut NoteLibrary,
    hint: Option<&str>,
    loading: bool,
    sink: &mut Vec<LibraryAction>,
) {
    if rect.width() < LIST_PAD * 2.0 + 80.0 || rect.height() < 60.0 {
        return;
    }

    // -- 头部：搜索 32 + 排序 28 (+ chip 28) --------------------------------
    let chip_h = if lib.tag_filter.is_empty() {
        0.0
    } else {
        CHIP_ROW_H
    };
    let fields_h = content_kit::SEARCH_H + SORT_ROW_H + chip_h;
    let head_h = (HEAD_TOP + fields_h + HEAD_BOTTOM).min(rect.height() * 0.6);
    let head_rect = Rect::from_min_max(
        Pos2::new(rect.left() + LIST_PAD, rect.top() + HEAD_TOP),
        Pos2::new(
            rect.left() + rect.width() - LIST_PAD,
            rect.top() + HEAD_TOP + fields_h,
        ),
    );
    let mut head = ui.new_child(
        UiBuilder::new()
            .id_salt("mw-library-head")
            .max_rect(head_rect),
    );
    head.spacing_mut().item_spacing = Vec2::ZERO;
    head.set_clip_rect(head_rect);

    let typed_before = lib.draft.clone();
    let resp = content_kit::search_field(
        &mut head,
        m,
        "mw-library-search",
        &mut lib.draft,
        SEARCH_PLACEHOLDER,
        head_rect.width(),
    );
    lib.search_id = Some(resp.id);
    if lib.draft != typed_before {
        // 去抖的时间戳读虚拟时钟（`lib.clock` 由 `show` 每帧盖成 `input.time`）。
        lib.typed_at = Some(lib.clock);
        lib.cursor = 0;
        lib.list_scroll = 0.0;
    }

    let sort_rect = Rect::from_min_size(
        Pos2::new(head_rect.left(), head_rect.top() + content_kit::SEARCH_H),
        Vec2::new(head_rect.width(), SORT_ROW_H),
    );
    let mut sort_anchor: Option<Pos2> = None;
    if sort_row(&mut head, m, sort_rect, lib) {
        sort_anchor = Some(Pos2::new(sort_rect.left() + 2.0, sort_rect.bottom() + 2.0));
    }
    let mut chip_remove: Option<String> = None;
    let mut clear_filters = false;
    if chip_h > 0.0 {
        let chip_rect = Rect::from_min_size(
            Pos2::new(head_rect.left(), sort_rect.bottom()),
            Vec2::new(head_rect.width(), CHIP_ROW_H),
        );
        let (rm, clr) = chip_row(&mut head, m, chip_rect, lib);
        chip_remove = rm;
        clear_filters = clr;
    }

    // -- 底部占多高 ---------------------------------------------------------
    let bottom_h = if lib.tag_input.is_some() {
        TAG_INPUT_H
    } else {
        (if lib.select_mode { BATCH_BAR_H } else { 0.0 })
            + if hint.is_some() { HINT_H } else { 0.0 }
    };
    let rows_rect = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + head_h),
        Pos2::new(
            rect.right(),
            (rect.bottom() - bottom_h).max(rect.top() + head_h),
        ),
    );
    lib.list_view_h = rows_rect.height();

    // -- 行区 ---------------------------------------------------------------
    let mut evts: Vec<RowEvt> = Vec::new();
    if loading {
        skeleton(ui, m, rows_rect);
    } else if lib.cards.is_empty() {
        if empty_library(ui, m, rows_rect) {
            sink.push(LibraryAction::Import);
        }
    } else if lib.rows.is_empty() {
        if no_results(ui, m, rows_rect, lib.has_filters()) {
            clear_filters = true;
        }
    } else {
        evts = rows(ui, m, rows_rect, lib);
    }

    if lib.tag_input.is_some() {
        let bar = Rect::from_min_size(
            Pos2::new(rect.left(), rect.bottom() - TAG_INPUT_H),
            Vec2::new(rect.width(), TAG_INPUT_H),
        );
        tag_input_row(ui, m, bar, lib);
    } else {
        if lib.select_mode {
            let bar = Rect::from_min_size(
                Pos2::new(rect.left(), rect.bottom() - bottom_h),
                Vec2::new(rect.width(), BATCH_BAR_H),
            );
            if let Some(a) = batch_bar(ui, m, bar, lib) {
                sink.push(a);
            }
        }
        if let Some(text) = hint {
            let bar = Rect::from_min_size(
                Pos2::new(rect.left(), rect.bottom() - HINT_H),
                Vec2::new(rect.width(), HINT_H),
            );
            ui.painter().text(
                Pos2::new(bar.left() + LIST_PAD, bar.center().y),
                Align2::LEFT_CENTER,
                elide(
                    ui.painter(),
                    content::NAV,
                    text,
                    rect.width() - LIST_PAD * 2.0,
                ),
                content_kit::font_for(content::NAV),
                c32(m.colors.text_secondary),
            );
        }
    }

    // -- 出闭包后落地 -------------------------------------------------------
    if let Some(anchor) = sort_anchor {
        lib.sort_menu = Some(Menu::new(anchor, sort_entries(lib)));
    }
    if let Some(path) = chip_remove {
        // chip 上的 × = 从 AND 里去掉这一项（`multi = true` 就是 toggle）。
        lib.set_tag_filter(path, true);
    }
    if clear_filters {
        lib.clear_filters();
        crate::trace::event("library_clear_filters");
    }
    for e in evts {
        match e {
            RowEvt::Click(at, mods) => {
                let id = lib.row_id(at);
                if id.as_deref().is_some_and(|i| lib.is_double_click(i)) {
                    if let Some(id) = id {
                        crate::trace::event(&format!("library_open id={id} via=double_click"));
                        sink.push(LibraryAction::Open(id));
                    }
                } else {
                    lib.row_clicked(at, mods);
                }
            }
            RowEvt::Menu(at, anchor) => {
                // 行右键菜单作用在**这一行**：先把焦点挪过去；未处于选择模式时把
                // 选择集也收成这一行，否则菜单里的「归档」会作用在别的行上。
                let was_selecting = lib.select_mode;
                let id = lib.row_id(at);
                lib.click_row(at, Modifiers::NONE);
                if !was_selecting {
                    lib.selected.clear();
                    if let Some(id) = id.clone() {
                        lib.selected.insert(id);
                    }
                }
                lib.row_menu = Some(Menu::new(anchor, row_entries(lib, id.is_some())));
            }
        }
    }
}

/// 排序行：左边排序按钮，右边 `N 张`（§7.6：计数在列表右上、12 辅助色、不做胶囊）。
///
/// 这个计数与页头那个不是一回事：页头数的是**当前范围**有多少张，这里数的是
/// **过滤之后**剩多少张；两个数不同才说明筛选收窄了多少。返回是否点了排序按钮。
fn sort_row(ui: &mut egui::Ui, m: &ContentMetrics, rect: Rect, lib: &NoteLibrary) -> bool {
    let label = lib.sort.label();
    let p = ui.painter();
    let tw = text_w(ui, SIDE_TEXT, label);
    let ctrl = Rect::from_min_size(
        Pos2::new(rect.left(), rect.center().y - SORT_ROW_H * 0.5 + 1.0),
        Vec2::new(tw + 6.0 + SORT_CHEV + 6.0, SORT_ROW_H - 2.0),
    );
    let resp = ui.interact(ctrl, Id::unique("mw-library-sort"), Sense::click());
    if resp.hovered() {
        p.rect_filled(ctrl, content::radius::ROW, c32(m.colors.hover));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let ink = c32(if resp.hovered() {
        m.colors.text_primary
    } else {
        m.colors.text_secondary
    });
    p.text(
        Pos2::new(ctrl.left() + 6.0, ctrl.center().y),
        Align2::LEFT_CENTER,
        label,
        content_kit::font_for(SIDE_TEXT),
        ink,
    );
    paint_icon_sized(
        p,
        Pos2::new(ctrl.right() - 6.0 - SORT_CHEV * 0.5, ctrl.center().y),
        SORT_CHEV,
        Icon::ChevronDown,
        c32(m.colors.text_faint),
    );
    let count = format!("{} 张", lib.rows.len());
    p.text(
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        count,
        content_kit::font_for(content::META),
        c32(m.colors.text_secondary),
    );
    resp.clicked()
}

fn sort_entries(lib: &NoteLibrary) -> Vec<Entry<Pick>> {
    CardSort::OPTIONS
        .iter()
        .map(|s| Entry::action_checked(s.label(), "", Pick::Sort(*s), true, lib.sort == *s))
        .collect()
}

/// 一行可删的纯文字 chip（§7.6：中性 6% 底、无彩色、逐个可删）+「清除筛选」。
///
/// 返回 `(要删掉的标签路径, 是否点了清除)`。
fn chip_row(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    lib: &NoteLibrary,
) -> (Option<String>, bool) {
    let p = ui.painter();
    let chip_y = rect.center().y;
    let mut x = rect.left();
    let mut remove: Option<String> = None;
    let bg = c32a(m.colors.text_primary, 0.06);
    for path in lib.tag_filter.clone() {
        if x > rect.right() - 60.0 {
            break;
        }
        let label = format!("#{path}");
        let tw = text_w(ui, SIDE_TEXT, &label);
        let cw = CHIP_INNER + tw + CHIP_INNER + CHIP_X + CHIP_INNER;
        let cr = Rect::from_min_size(Pos2::new(x, chip_y - CHIP_H * 0.5), Vec2::new(cw, CHIP_H));
        let resp = ui.interact(cr, Id::unique(("mw-lib-chip", &path)), Sense::click());
        p.rect_filled(cr, content::radius::ROW, bg);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        p.text(
            Pos2::new(cr.left() + CHIP_INNER, chip_y),
            Align2::LEFT_CENTER,
            label,
            content_kit::font_for(SIDE_TEXT),
            c32(m.colors.text_primary),
        );
        paint_icon_sized(
            p,
            Pos2::new(cr.right() - CHIP_INNER - CHIP_X * 0.5, chip_y),
            CHIP_X,
            Icon::Close,
            c32(if resp.hovered() {
                m.colors.text_primary
            } else {
                m.colors.text_faint
            }),
        );
        if resp.clicked() && remove.is_none() {
            remove = Some(path);
        }
        x += cw + CHIP_GAP;
    }
    // 「清除筛选」用 `new_child` 摆在 chips 之后：`ui.interact` 不推进游标，
    // 直接调 `text_action` 会把它 allocate 到 head 的左上角、压住搜索框。
    let mut clear = false;
    let clear_w = text_w(ui, SIDE_TEXT, CLEAR_FILTERS) + 2.0;
    if x + clear_w <= rect.right() {
        let cr = Rect::from_min_size(
            Pos2::new(x, chip_y - CHIP_H * 0.5),
            Vec2::new(clear_w, CHIP_H),
        );
        let mut sub = ui.new_child(UiBuilder::new().id_salt("mw-lib-chip-clear").max_rect(cr));
        sub.spacing_mut().item_spacing = Vec2::ZERO;
        clear = content_kit::text_action(&mut sub, m, CLEAR_FILTERS, true).clicked();
    }
    (remove, clear)
}

/// 虚拟化的行区。
fn rows(ui: &mut egui::Ui, m: &ContentMetrics, rect: Rect, lib: &mut NoteLibrary) -> Vec<RowEvt> {
    let w = rect.width();
    let total = lib.rows.len();
    let mut child = ui.new_child(UiBuilder::new().id_salt("mw-library-rows").max_rect(rect));
    child.spacing_mut().item_spacing = Vec2::ZERO;
    child.set_clip_rect(rect);
    let out = ScrollArea::vertical()
        .id_salt("mw-library-list-scroll")
        .auto_shrink([false, false])
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
        .vertical_scroll_offset(lib.list_scroll)
        .show_rows(&mut child, CARD_ROW_H, total, |ui, range| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let mut evts: Vec<RowEvt> = Vec::with_capacity(range.len());
            for at in range {
                if let Some(ev) = card_row(ui, m, rect, w, at, lib) {
                    evts.push(ev);
                }
            }
            evts
        });
    lib.list_scroll = out.state.clamped_offset().y;
    if let Some(req) = scrollbar(
        ui,
        m,
        rect,
        out.content_size.y,
        lib.list_scroll,
        Id::unique("mw-library-list-bar"),
    ) {
        lib.list_scroll = req;
    }
    out.inner
}

/// 一行 60px（§7.4）：标题 15/22/600、摘要 13/19/400、meta 12/18/400。
///
/// 三个行盒叠起来正好 59，在 60 里居中——**行内留白来自行盒自身的行距**，
/// 不再另塞 padding：§7.4 同时写了「行 60」与「内距 12×16」，12+22+19+18+12 = 83
/// 根本塞不进 60，所以竖向内距只能由行盒提供，横向内距按 16 照办。
fn card_row(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    clip_all: Rect,
    w: f32,
    at: usize,
    lib: &NoteLibrary,
) -> Option<RowEvt> {
    let idx = *lib.rows.get(at)?;
    let card: &CardMeta = &lib.cards[idx];
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, CARD_ROW_H), Sense::click());
    let p = ui.painter();

    let focused = lib.focused.as_deref() == Some(card.id.as_str());
    if focused {
        p.rect_filled(rect, 0.0, c32(m.colors.selected));
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, c32(m.colors.hover));
    }
    if lib.zone == Zone::List && lib.cursor_by_key && at == lib.cursor {
        let ring = Rect::from_min_max(
            Pos2::new(rect.left() + RING_INSET_X, rect.top() + RING_INSET_Y),
            Pos2::new(rect.right() - RING_INSET_X, rect.bottom() - RING_INSET_Y),
        );
        content_kit::focus_ring(p, ring, m, content::radius::ROW);
    }
    // 分隔线内缩 16（§7.4）：贴到列边就成了表格线，不是目录。
    p.hline(
        (rect.left() + DIVIDER_INSET)..=(rect.right() - DIVIDER_INSET),
        rect.bottom() - m.hairline * 0.5,
        Stroke::new(m.hairline, c32(m.colors.divider)),
    );

    let mut text_x = rect.left() + LIST_PAD;
    if lib.select_mode {
        let cr = Rect::from_min_size(
            Pos2::new(text_x, rect.center().y - CHECK_BOX * 0.5),
            Vec2::splat(CHECK_BOX),
        );
        // 焦点行也算勾上：选择模式里「当前行」必定在批量作用范围内。
        checkbox(p, cr, lib.selected.contains(&card.id) || focused, m);
        text_x += CHECK_BOX + CHECK_GAP;
    }
    let text_right = rect.right() - LIST_PAD;
    if text_right - text_x < 48.0 {
        return row_event(resp, at, ui);
    }

    let stack = content::LIST_TITLE.line_height
        + content::LIST_EXCERPT.line_height
        + content::LIST_META.line_height;
    let top = rect.center().y - stack * 0.5;
    let title_y = top + content::LIST_TITLE.line_height * 0.5;
    let excerpt_y =
        title_y + (content::LIST_TITLE.line_height + content::LIST_EXCERPT.line_height) * 0.5;
    let meta_y =
        excerpt_y + (content::LIST_EXCERPT.line_height + content::LIST_META.line_height) * 0.5;
    let clip = Rect::from_min_max(
        Pos2::new(text_x, clip_all.top()),
        Pos2::new(text_right, clip_all.bottom()),
    );
    let q = lib.active_query.as_str();
    let hit_bg = c32a(m.colors.accent, HIT_ALPHA);
    let hit_ink = c32(m.colors.accent);

    let title = if card.title.trim().is_empty() {
        UNTITLED
    } else {
        card.title.as_str()
    };
    let title = elide(p, content::LIST_TITLE, title, text_right - text_x);
    draw_line(
        p,
        text_x,
        title_y,
        0.0,
        &hit_segs(
            &title,
            content::LIST_TITLE,
            c32(m.colors.text_primary),
            q,
            hit_ink,
        ),
        hit_bg,
        clip,
    );

    let excerpt = elide(
        p,
        content::LIST_EXCERPT,
        card.excerpt.trim(),
        text_right - text_x,
    );
    if !excerpt.is_empty() {
        draw_line(
            p,
            text_x,
            excerpt_y,
            0.0,
            &hit_segs(
                &excerpt,
                content::LIST_EXCERPT,
                c32(m.colors.text_secondary),
                q,
                hit_ink,
            ),
            hit_bg,
            clip,
        );
    }

    // meta：最多一个纯文字标签（前置 6px 圆点，§7.1「颜色只出现在圆点上」）+ 日期 + `+n`。
    let mut mx = text_x;
    if let Some(t) = card.tags.first() {
        p.circle_filled(
            Pos2::new(mx + content_kit::TAG_DOT * 0.5, meta_y),
            content_kit::TAG_DOT * 0.5,
            tag_dot(m, t),
        );
        mx += content_kit::TAG_DOT + content_kit::TAG_DOT_GAP;
    }
    draw_line(p, mx, meta_y, META_GAP, &meta_segs(card, m), hit_bg, clip);

    row_event(resp, at, ui)
}

/// 单击 / 右键。修饰键从 `ui.input` 读：egui 0.37 的 `Response` 上没有 `modifiers`。
fn row_event(resp: Response, at: usize, ui: &egui::Ui) -> Option<RowEvt> {
    if resp.context_menu_opened() {
        let anchor = resp
            .interact_pointer_pos()
            .unwrap_or_else(|| resp.rect.left_bottom());
        return Some(RowEvt::Menu(at, Pos2::new(anchor.x, anchor.y + 4.0)));
    }
    if resp.clicked() {
        let mods = ui.input(|i| i.modifiers);
        return Some(RowEvt::Click(at, mods));
    }
    None
}

fn meta_segs(card: &CardMeta, m: &ContentMetrics) -> Vec<Seg> {
    let style = content::LIST_META;
    let sec = c32(m.colors.text_secondary);
    let faint = c32(m.colors.text_faint);
    let mut out = Vec::with_capacity(3);
    if let Some(t) = card.tags.first() {
        out.push(Seg {
            text: format!("#{t}"),
            style,
            color: sec,
            hit: false,
        });
        if card.tags.len() > 1 {
            out.push(Seg {
                text: format!("+{}", card.tags.len() - 1),
                style,
                color: faint,
                hit: false,
            });
        }
    }
    out.push(Seg {
        text: format_date(card.updated_at),
        style,
        color: faint,
        hit: false,
    });
    out
}

/// 按命中区间把一行文字切成「普通段 / 高亮段」（§7.6：命中处 12% 强调底）。
///
/// 拼音命中在原文里没有字面对应串，`match_ranges` 对那种情况返回空——宁可不高亮，
/// 也不在用户没打的地方画一块颜色。
fn hit_segs(text: &str, style: TypeStyle, ink: Color32, q: &str, hit_ink: Color32) -> Vec<Seg> {
    let ranges = match_ranges(text, q);
    if ranges.is_empty() {
        return vec![Seg {
            text: text.to_owned(),
            style,
            color: ink,
            hit: false,
        }];
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::with_capacity(ranges.len() * 2 + 1);
    let mut at = 0usize;
    for (a, b) in ranges {
        let a = a.min(chars.len());
        let b = b.clamp(a, chars.len());
        if a > at {
            out.push(Seg {
                text: chars[at..a].iter().collect(),
                style,
                color: ink,
                hit: false,
            });
        }
        out.push(Seg {
            text: chars[a..b].iter().collect(),
            style,
            color: hit_ink,
            hit: true,
        });
        at = b;
    }
    if at < chars.len() {
        out.push(Seg {
            text: chars[at..].iter().collect(),
            style,
            color: ink,
            hit: false,
        });
    }
    out
}

fn checkbox(p: &eframe::egui::Painter, r: Rect, on: bool, m: &ContentMetrics) {
    if on {
        p.rect_filled(r, CHECK_R, c32(m.colors.accent));
        // 勾用面板色而不是纯白：深色主题的强调色本来就亮，纯白会刺眼。
        let s = Stroke::new(1.6, c32(m.colors.panel));
        let a = Pos2::new(
            r.left() + r.width() * 0.27,
            r.center().y + r.height() * 0.02,
        );
        let b = Pos2::new(r.left() + r.width() * 0.44, r.top() + r.height() * 0.68);
        let c = Pos2::new(r.left() + r.width() * 0.75, r.top() + r.height() * 0.31);
        p.line_segment([a, b], s);
        p.line_segment([b, c], s);
    } else {
        p.rect_stroke(
            r,
            CHECK_R,
            Stroke::new(m.hairline.max(1.0), c32(m.colors.text_faint)),
            StrokeKind::Inside,
        );
    }
}

/// §7.7「加载中：3 行 × 60px 骨架，4% 中性底，不转菊花」。
fn skeleton(ui: &mut egui::Ui, m: &ContentMetrics, rect: Rect) {
    let p = ui.painter();
    let fill = c32(m.colors.hover);
    let avail = (rect.width() - LIST_PAD * 2.0).max(40.0);
    let stack = SKELETON_BARS
        .iter()
        .map(|(h, _)| h + SKELETON_GAP)
        .sum::<f32>()
        - SKELETON_GAP;
    for i in 0..SKELETON_ROWS {
        let top = rect.top() + i as f32 * CARD_ROW_H;
        if top + CARD_ROW_H > rect.bottom() {
            break;
        }
        let mut y = top + (CARD_ROW_H - stack) * 0.5;
        for (h, frac) in SKELETON_BARS {
            let bar = Rect::from_min_size(
                Pos2::new(rect.left() + LIST_PAD, y),
                Vec2::new(avail * frac, h),
            );
            p.rect_filled(bar, h * 0.5, fill);
            y += h + SKELETON_GAP;
        }
        p.hline(
            (rect.left() + DIVIDER_INSET)..=(rect.right() - DIVIDER_INSET),
            top + CARD_ROW_H - m.hairline * 0.5,
            Stroke::new(m.hairline, c32(m.colors.divider)),
        );
    }
}

/// §7.7「全空：左对齐 16/26/600 + 13/20 说明 + 一个文字动作，不画 56px 大图标」。
/// 返回是否点了「导入…」。
fn empty_library(ui: &mut egui::Ui, m: &ContentMetrics, rect: Rect) -> bool {
    let inner = Rect::from_min_max(
        Pos2::new(rect.left() + LIST_PAD, rect.top() + 32.0),
        Pos2::new(rect.right() - LIST_PAD, rect.bottom()),
    );
    let mut child = ui.new_child(UiBuilder::new().id_salt("mw-library-empty").max_rect(inner));
    child.spacing_mut().item_spacing = Vec2::splat(2.0);
    child.set_clip_rect(inner);
    content_kit::empty_state(
        &mut child,
        m,
        EMPTY_LIBRARY,
        EMPTY_LIBRARY_DESC,
        Some("导入…"),
    )
    .is_some_and(|r| r.clicked())
}

/// §7.6「无结果：左对齐一句 + 一个「清除筛选」文字动作，不画插画」。
fn no_results(ui: &mut egui::Ui, m: &ContentMetrics, rect: Rect, has_filters: bool) -> bool {
    let inner = Rect::from_min_max(
        Pos2::new(rect.left() + LIST_PAD, rect.top() + 24.0),
        Pos2::new(rect.right() - LIST_PAD, rect.bottom()),
    );
    let mut child = ui.new_child(UiBuilder::new().id_salt("mw-library-nores").max_rect(inner));
    child.spacing_mut().item_spacing = Vec2::ZERO;
    child.set_clip_rect(inner);
    let (r, _) = child.allocate_exact_size(
        Vec2::new(inner.width(), content::NAV.line_height),
        Sense::hover(),
    );
    child.painter().text(
        Pos2::new(r.left(), r.center().y),
        Align2::LEFT_CENTER,
        NO_RESULTS,
        content_kit::font_for(content::NAV),
        c32(m.colors.text_secondary),
    );
    if !has_filters {
        return false;
    }
    child.add_space(8.0);
    content_kit::text_action(&mut child, m, CLEAR_FILTERS, true).clicked()
}

/// 44px 批量条（§7.4）：**在列表栏内**，不横跨整窗——横跨会让「作用在谁身上」
/// 失去边界，三栏时还会让预览被一条无关的横杠压住。
///
/// 一次批量 = 一次确认 + 一条结果提示；「移出」是危险动作，只置 `confirm_remove`，
/// 真正的确认弹窗由父模块的 `remove_dialog` 走 `crate::dialog`（§7.4）。
fn batch_bar(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    lib: &mut NoteLibrary,
) -> Option<LibraryAction> {
    let p = ui.painter();
    p.hline(
        rect.x_range(),
        rect.top(),
        Stroke::new(m.hairline, c32(m.colors.divider)),
    );
    let n = lib.selected.len();
    let head = format!("已选 {n} 张");
    let mut x = rect.left() + LIST_PAD;
    p.text(
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        &head,
        content_kit::font_for(content::META),
        c32(m.colors.text_secondary),
    );
    x += text_w(ui, content::META, &head) + BAR_GAP;

    let archived = lib.scope == CardScope::Archived;
    let enabled = n > 0;
    // 「取消」永远可点：选择模式必须有一条不需要先取消勾选就能退出的路。
    let labels: [&str; 5] = [
        if archived { "取消归档" } else { "归档" },
        "导出",
        "加标签",
        "移出",
        "取消",
    ];
    let mut out: Option<LibraryAction> = None;
    for (i, label) in labels.iter().enumerate() {
        let on = enabled || i == 4;
        let danger = i == 3;
        let w = bar_action(ui, m, label, danger, on, x, rect);
        if ui
            .interact(
                Rect::from_min_size(
                    Pos2::new(x, rect.center().y - content::NAV.line_height * 0.5),
                    Vec2::new(w, content::NAV.line_height),
                ),
                Id::unique(("mw-lib-batch", i)),
                if on { Sense::click() } else { Sense::hover() },
            )
            .clicked()
        {
            out = match i {
                0 if archived => Some(LibraryAction::Unarchive(lib.take_sel())),
                0 => Some(LibraryAction::Archive(lib.take_sel())),
                1 => Some(LibraryAction::Export(lib.export_ids())),
                2 => {
                    lib.tag_input = Some(String::new());
                    lib.tag_input_fresh = true;
                    None
                }
                3 => {
                    lib.confirm_remove = true;
                    crate::trace::event(&format!("library_remove_prompt n={n}"));
                    None
                }
                _ => {
                    lib.exit_select();
                    None
                }
            };
            break;
        }
        x += w + BAR_GAP;
    }
    out
}

/// 画一个批量条按钮并返回它的宽度。命中判定用同一个宽度，绘制与命中就不会漂。
fn bar_action(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    label: &str,
    danger: bool,
    enabled: bool,
    x: f32,
    rect: Rect,
) -> f32 {
    let w = text_w(ui, content::NAV, label).ceil();
    let r = Rect::from_min_size(
        Pos2::new(x, rect.center().y - content::NAV.line_height * 0.5),
        Vec2::new(w, content::NAV.line_height),
    );
    let resp = ui.interact(r, Id::unique(("mw-lib-batch-ink", label)), Sense::hover());
    let base = if danger { m.colors.warn } else { m.colors.link };
    let color = if !enabled {
        c32(m.colors.text_faint)
    } else if resp.hovered() {
        c32(m.colors.text_primary)
    } else {
        c32(base)
    };
    if enabled && resp.hovered() {
        ui.painter()
            .hline(r.x_range(), r.bottom() - 1.0, Stroke::new(1.0, c32(base)));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        Pos2::new(r.left(), r.center().y),
        Align2::LEFT_CENTER,
        label,
        content_kit::font_for(content::NAV),
        color,
    );
    w
}

/// 行右键菜单。`Pick` 必须 `Copy`（`Menu<T: Copy>`），所以带 id 的动作一律读当前
/// 选择集——`paint_list` 已经把选择集收成被右键的那一行了。
fn row_entries(lib: &NoteLibrary, has: bool) -> Vec<Entry<Pick>> {
    let archived = lib.scope == CardScope::Archived;
    let mut out = vec![Entry::action_with_icon(
        "打开卡片",
        "Enter",
        Icon::Note,
        Pick::OpenFocused,
        has,
    )];
    if lib.jump_source().is_some() {
        out.push(Entry::action_with_icon(
            "跳到来源节点",
            "",
            Icon::Focus,
            Pick::JumpSource,
            true,
        ));
    }
    out.push(Entry::separator());
    out.push(Entry::action_with_icon(
        if archived { "取消归档" } else { "归档" },
        "",
        Icon::Exit,
        if archived {
            Pick::UnarchiveSel
        } else {
            Pick::ArchiveSel
        },
        has,
    ));
    out.push(Entry::action_with_icon(
        "挂到当前主题",
        "",
        Icon::Mindmap,
        Pick::AttachSel,
        has,
    ));
    out.push(Entry::action_with_icon(
        "加标签…",
        "",
        Icon::Tag,
        Pick::AddTagSel,
        has,
    ));
    out.push(Entry::separator());
    out.push(Entry::action_with_icon(
        "从本地库移出…",
        "",
        Icon::Delete,
        Pick::RemoveSel,
        has,
    ));
    out
}

/// 加标签的行内输入（§7.4）。不开模态：`overlay::Modal` 是全壳层共用的栈，
/// 为一个非危险动作往里加变体不值当，而且它那条「每个模态都可达」的穷举测试
/// 会跟着改。Enter 提交 / Esc 取消由父模块的 `handle_tag_input` 收。
fn tag_input_row(ui: &mut egui::Ui, m: &ContentMetrics, rect: Rect, lib: &mut NoteLibrary) {
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        Stroke::new(m.hairline, c32(m.colors.divider)),
    );
    let inner = Rect::from_min_max(
        Pos2::new(
            rect.left() + LIST_PAD,
            rect.center().y - content_kit::SEARCH_H * 0.5,
        ),
        Pos2::new(
            rect.right() - LIST_PAD,
            rect.center().y + content_kit::SEARCH_H * 0.5,
        ),
    );
    let Some(mut buf) = lib.tag_input.take() else {
        return;
    };
    // 只在刚打开那一帧抢焦点：每帧都 `request_focus` 会把 Esc 一起吃掉。
    // 标志存在 `lib` 上而不是 `ctx.data`：egui 的 `IdTypeMap` 只能按类型删，
    // 按 id 删不掉，第二次打开就再也抢不到焦点。
    let fresh = lib.tag_input_fresh;
    lib.tag_input_fresh = false;
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt("mw-library-tag-input")
            .max_rect(inner),
    );
    child.spacing_mut().item_spacing = Vec2::ZERO;
    let id = Id::unique("mw-library-tag-field");
    let resp = child.add_sized(
        inner.size(),
        egui::TextEdit::singleline(&mut buf)
            .id(id)
            .font(content_kit::font_for(content::NAV))
            .hint_text(
                egui::RichText::new("新标签，Enter 确认 · Esc 取消")
                    .size(content::NAV.size)
                    .color(c32(m.colors.text_faint)),
            )
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::symmetric(0, 6)),
    );
    if fresh {
        resp.request_focus();
    }
    lib.tag_input = Some(buf);
    // 重新取一次画笔：`ui.painter()` 的借用不能跨 `new_child`。
    let p = ui.painter();
    if resp.has_focus() {
        content_kit::focus_ring(p, inner, m, content::radius::ROW);
    } else {
        p.rect_stroke(
            inner,
            content::radius::ROW,
            Stroke::new(m.hairline, c32(m.colors.hairline)),
            StrokeKind::Inside,
        );
    }
}
