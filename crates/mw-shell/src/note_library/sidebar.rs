//! 侧栏（`docs/28` §7.1 / §7.3）：四个范围 + 两级标签树。
//!
//! 计数不是这一层自己数出来的：四个范围读 `ScopeCounts`、标签行读 `tag_tree`，
//! 两者都由 `mw_core::note` 按**同一份谓词**算（§7.11「单一查询入口」）。
//! 旧版侧栏自己 `filter().count()`、列表自己再滤一遍，「收件箱 4 = 全部 4」
//! 就是这么来的。
//!
//! 标签树封顶两级（§7.3 / §7.9「禁止标签树无限层级」）：`flatten_tag_rows`
//! 已经把更深的层级挡在外面，这一层只管摆行，不再判深度。

use eframe::egui::{
    self, scroll_area::ScrollBarVisibility, Align2, Color32, Id, Pos2, Rect, ScrollArea, Sense,
    Vec2,
};
use mw_core::design::content::{self, ContentMetrics, TypeStyle};
use mw_core::note::{CardScope, FlatTag};

use super::{
    c32, elide, scope_icon, tag_dot, NoteLibrary, Zone, NAV_BAR_W, NAV_ROW_H, NO_TAG_HINT,
    SECTION_GAP, SECTION_TAGS, SIDE_COUNT, SIDE_PAD, SIDE_TEXT, TAG_ROW_H,
};
use crate::content_kit;
use crate::icon_paint::paint_icon_sized;
use crate::icons::Icon;

/// 标签行每一级的缩进。两级的树最多缩一次，不会滚出视野。
const TAG_INDENT: f32 = 14.0;
/// 行内左右内距（侧栏自己已经有 8 的外内距）。
const ROW_INSET: f32 = 8.0;
/// chevron 边长（§7.3：12）。
const CHEVRON: f32 = 12.0;
/// 导航行图标边长（§7.3：16）。
const NAV_ICON: f32 = 16.0;

pub(super) fn paint_sidebar(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    lib: &mut NoteLibrary,
) {
    ui.painter().rect_filled(rect, 0.0, c32(m.colors.panel));
    if rect.width() < SIDE_PAD * 2.0 + 40.0 {
        return;
    }
    let inner = rect.shrink(SIDE_PAD);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("mw-library-sidebar")
            .max_rect(inner),
    );
    child.spacing_mut().item_spacing = Vec2::ZERO;

    let w = inner.width();
    // 只有键盘把焦点交给侧栏时才画焦点环（§4.6：悬停/选中/键盘焦点是三种状态）。
    let kb = lib.zone == Zone::Sidebar;
    let mut clicked_scope: Option<CardScope> = None;
    let mut clicked_tag: Option<(String, bool)> = None;
    let mut toggled: Option<String> = None;
    let multi = child.input(|i| i.modifiers.ctrl || i.modifiers.command);

    let out = ScrollArea::vertical()
        .id_salt("mw-library-side-scroll")
        .auto_shrink([false, false])
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
        .vertical_scroll_offset(lib.side_scroll)
        .show(&mut child, |ui| {
            for scope in CardScope::ROWS {
                let count = lib.count_for(scope);
                let selected = lib.scope == scope;
                let (rrect, resp) = ui.allocate_exact_size(Vec2::new(w, NAV_ROW_H), Sense::click());
                nav_row(ui, m, rrect, scope, count, selected, kb && selected);
                if resp.clicked() {
                    clicked_scope = Some(scope);
                }
            }
            ui.add_space(SECTION_GAP);
            let (srect, _) =
                ui.allocate_exact_size(Vec2::new(w, SIDE_COUNT.line_height + 6.0), Sense::hover());
            ui.painter().text(
                Pos2::new(srect.left() + ROW_INSET, srect.center().y),
                Align2::LEFT_CENTER,
                SECTION_TAGS,
                content_kit::font_for(SIDE_COUNT),
                c32(m.colors.text_faint),
            );
            ui.add_space(2.0);

            if lib.tree.is_empty() {
                // §7.7「无标签：12/18 `text.faint`，不占大块」——两行高度，不画框。
                let line = content::LIST_META.line_height;
                let (hrect, _) = ui.allocate_exact_size(Vec2::new(w, line * 2.0), Sense::hover());
                let text = elide(
                    ui.painter(),
                    content::LIST_META,
                    NO_TAG_HINT,
                    w - ROW_INSET * 2.0,
                );
                ui.painter().text(
                    Pos2::new(hrect.left() + ROW_INSET, hrect.top() + line * 0.5),
                    Align2::LEFT_CENTER,
                    text,
                    content_kit::font_for(content::LIST_META),
                    c32(m.colors.text_faint),
                );
                return;
            }
            for row in lib.tree.clone() {
                let selected = lib.tag_filter.contains(&row.path);
                let (rrect, resp) = ui.allocate_exact_size(Vec2::new(w, TAG_ROW_H), Sense::click());
                if tag_row(ui, m, rrect, &row, selected, lib, kb && selected) {
                    toggled = Some(row.path.clone());
                } else if resp.clicked() {
                    clicked_tag = Some((row.path.clone(), multi));
                }
            }
        });
    lib.side_scroll = out.state.clamped_offset().y;
    if let Some(req) = super::scrollbar(
        ui,
        m,
        inner,
        out.content_size.y,
        lib.side_scroll,
        Id::unique("mw-library-side-bar"),
    ) {
        lib.side_scroll = req;
    }

    if let Some(scope) = clicked_scope {
        lib.set_scope(scope);
    }
    if let Some(path) = toggled {
        lib.toggle_collapse(&path);
    }
    if let Some((path, multi)) = clicked_tag {
        lib.set_tag_filter(path, multi);
    }
}

/// 一条导航行：图标 16 + 文本 + 计数右对齐，圆角 6；选中 = 4% 底 + 2px 强调条（§7.3）。
fn nav_row(
    ui: &egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    scope: CardScope,
    count: usize,
    selected: bool,
    ring: bool,
) {
    let p = ui.painter();
    let radius = content::radius::ROW;
    let hover = ui
        .interact(
            rect,
            Id::unique(("mw-lib-nav-hover", scope.label())),
            Sense::hover(),
        )
        .hovered();
    if selected {
        p.rect_filled(rect, radius, c32(m.colors.nav_selected));
    } else if hover {
        p.rect_filled(rect, radius, c32(m.colors.hover));
    }
    if selected {
        // 2px 强调条比整行描边安静：§7.3 明写「无边框」。
        let h = rect.height() - 12.0;
        let bar = Rect::from_min_size(
            Pos2::new(rect.left() + 2.0, rect.center().y - h * 0.5),
            Vec2::new(NAV_BAR_W, h),
        );
        p.rect_filled(bar, NAV_BAR_W * 0.5, c32(m.colors.accent));
    }
    if ring {
        content_kit::focus_ring(p, rect, m, radius);
    }
    let ink = c32(if selected || hover {
        m.colors.text_primary
    } else {
        m.colors.text_secondary
    });
    let icon_x = rect.left() + ROW_INSET + NAV_ICON * 0.5;
    paint_icon_sized(
        p,
        Pos2::new(icon_x, rect.center().y),
        NAV_ICON,
        scope_icon(scope),
        ink,
    );
    let text_x = icon_x + NAV_ICON * 0.5 + 8.0;
    row_label(ui, m, rect, text_x, scope.label(), count, ink);
}

/// 一条标签行：6px 圆点 + 文本 + 计数；chevron 只在可展开且（悬停或已展开）时出现（§7.3）。
///
/// 返回 `true` = 点在 chevron 上，要开合而不是筛选。
fn tag_row(
    ui: &egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    row: &FlatTag,
    selected: bool,
    lib: &NoteLibrary,
    ring: bool,
) -> bool {
    let p = ui.painter();
    let radius = content::radius::ROW;
    let hover = ui
        .interact(
            rect,
            Id::unique(("mw-lib-tag-hover", &row.path)),
            Sense::hover(),
        )
        .hovered();
    if selected {
        p.rect_filled(rect, radius, c32(m.colors.nav_selected));
    } else if hover {
        p.rect_filled(rect, radius, c32(m.colors.hover));
    }
    if ring {
        content_kit::focus_ring(p, rect, m, radius);
    }
    let ink = c32(if selected || hover {
        m.colors.text_primary
    } else {
        m.colors.text_secondary
    });
    let mut x = rect.left() + ROW_INSET + row.depth as f32 * TAG_INDENT;
    let mut expand = false;
    if row.expandable {
        let collapsed = lib.collapsed.contains(&row.path);
        let c_rect = Rect::from_min_size(
            Pos2::new(x, rect.center().y - CHEVRON * 0.5),
            Vec2::splat(CHEVRON),
        );
        let icon = if collapsed {
            Icon::ChevronRight
        } else {
            Icon::ChevronDown
        };
        // 没悬停也没展开时**留位不画**：§7.3「chevron 在悬停/展开时出现」，
        // 但位置必须留着，否则文字会随悬停左右跳一格。
        if hover || !collapsed {
            paint_icon_sized(p, c_rect.center(), CHEVRON, icon, c32(m.colors.text_faint));
        }
        if ui
            .interact(
                c_rect.expand(3.0),
                Id::unique(("mw-lib-tag-chev", &row.path)),
                Sense::click(),
            )
            .clicked()
        {
            expand = true;
        }
    }
    x += CHEVRON + 2.0;
    // 圆点色按**根标签**取：`知识/方法` 与 `知识` 同色，扫读时一眼看出同族（§7.1）。
    p.circle_filled(
        Pos2::new(x + content_kit::TAG_DOT * 0.5, rect.center().y),
        content_kit::TAG_DOT * 0.5,
        tag_dot(m, &row.path),
    );
    x += content_kit::TAG_DOT + content_kit::TAG_DOT_GAP;
    row_label(ui, m, rect, x, &row.name, row.count, ink);
    expand
}

/// 右对齐画计数（`text.faint`），并返回左边名称在剩余预算内该显示的文本。
///
/// 名称与计数各写一遍 `painter.text` 会漏掉「先量计数、再给名称算预算」这一步，
/// 长标签就会压到计数上——所以两件事必须在一个函数里。
fn row_label(
    ui: &egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    text_x: f32,
    name: &str,
    count: usize,
    ink: Color32,
) -> String {
    let p = ui.painter();
    let label = count.to_string();
    let cw = text_width(ui, SIDE_COUNT, &label);
    p.text(
        Pos2::new(rect.right() - ROW_INSET, rect.center().y),
        Align2::RIGHT_CENTER,
        label,
        content_kit::font_for(SIDE_COUNT),
        c32(m.colors.text_faint),
    );
    let budget = (rect.right() - ROW_INSET - cw - 8.0 - text_x).max(16.0);
    let out = elide(p, SIDE_TEXT, name, budget);
    p.text(
        Pos2::new(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        out.clone(),
        content_kit::font_for(SIDE_TEXT),
        ink,
    );
    out
}

fn text_width(ui: &egui::Ui, style: TypeStyle, s: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(s.to_owned(), content_kit::font_for(style), Color32::WHITE)
        .size()
        .x
}
