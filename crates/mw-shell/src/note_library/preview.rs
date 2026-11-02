//! 预览阅读面（`docs/28` §7.5）：版心 600、只读、复用卡片页那一棵 Markdown AST。
//!
//! 三条硬规矩：
//! 1. **不另立字阶**：标题走 `content::PAGE_TITLE`（30/42/600）、正文走
//!    `READING_BODY`（16/28）、元信息走 `META`（12/18）。§7.5 明写「复用 §4.1 的
//!    阅读正文，不另立一套」——旧版预览标题写死 `.size(22.0)`，和卡片页对不上。
//! 2. **不放「打开编辑」大按钮**（§7.9 点名）：入口只有两个——悬停时右上角那颗
//!    28px 铅笔，以及双击标题/正文（键盘是 `Enter`，在父模块的键表里）。
//! 3. **滚动独立**：预览自己的 `preview_scroll`，切卡片回顶（父模块 `set_focus`），
//!    返回列表时列表滚动位置不变（父模块另存 `list_scroll`）。
//!
//! 正文用 `painter.galley` 手摆而不是塞一个只读 `TextEdit`：`TextEdit` 会注册焦点，
//! 用户点一下预览，父模块那句 `typing = memory.focus().is_some()` 就成立，
//! `↑↓` / `J/K` / `Enter` 会被当成「在输入框里打字」全部吞掉。

use std::collections::HashMap;

use eframe::egui::{
    self, scroll_area::ScrollBarVisibility, Align2, Color32, Id, Pos2, Rect, ScrollArea, Sense,
    Stroke, UiBuilder, Vec2,
};
use mw_core::design::content::{self, ContentMetrics};

use super::{
    c32, c32a, draw_line, elide, icon_at, scrollbar, tag_dot, text_w, CardSource, LibraryAction,
    NoteLibrary, Preview, Seg, Zone, EMPTY_PREVIEW,
};
use crate::content_kit;
use crate::icons::Icon;
use crate::note_view::{format_date, reading_count, reading_job, MdPalette};

/// 标题与元信息之间的间距。
const TITLE_GAP: f32 = 12.0;
/// 元信息 / 标签 / 来源三行之间的间距。
const META_GAP: f32 = 6.0;
/// 元信息块与分隔线之间。
const RULE_ABOVE: f32 = 20.0;
/// 分隔线与正文之间。
const RULE_BELOW: f32 = 20.0;
/// 铅笔按钮距预览栏右上角的内距。
const PENCIL_INSET: f32 = 10.0;
/// meta 行里各段之间的间距。
const META_SEG_GAP: f32 = 6.0;
/// 标签之间的间距。
const TAG_GAP: f32 = 12.0;
/// 没有正文时的占位。空着会在分隔线下面留一块莫名其妙的白。
const NO_BODY: &str = "（这张卡片还没有正文）";
/// 没有标题时的占位。
const UNTITLED: &str = "无标题";
/// 跨文档来源：文档名查不到时不编一个假名字（`CardSource::phrase` 的口径）。
const FOREIGN_SOURCE: &str = "来源：另一份文档";

pub(super) fn paint_preview(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    rect: Rect,
    lib: &mut NoteLibrary,
    sources: &HashMap<String, CardSource>,
    sink: &mut Vec<LibraryAction>,
) {
    if rect.width() < 120.0 || rect.height() < 80.0 {
        return;
    }
    // §7.5「悬停时右上角 28px 铅笔」；键盘把焦点交给预览时也常驻，
    // 否则纯键盘用户永远看不到这个入口（`docs/30` §4.4 的键盘可达口径）。
    let over = ui
        .interact(rect, Id::unique("mw-library-preview"), Sense::hover())
        .hovered();
    let show_pencil = over || lib.zone == Zone::Preview;

    let col = content::PREVIEW.column(rect.width());
    let text_x = rect.left() + col.side + col.inset;
    let text_rect = Rect::from_min_max(
        Pos2::new(text_x, rect.top()),
        Pos2::new(text_x + col.text, rect.bottom()),
    );

    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt("mw-library-preview")
            .max_rect(text_rect),
    );
    child.spacing_mut().item_spacing = Vec2::ZERO;
    child.set_clip_rect(text_rect);

    let mut offset = lib.preview_scroll;
    let mut content_h = 0.0;
    let mut wants_open = false;
    let mut open_id: Option<String> = None;
    let mut jump_node: Option<String> = None;

    // `pv` 是 `lib.preview` 的共享借用，只活到这个 `if let` 结束——之后才写
    // `lib.preview_scroll`。整份克隆一遍正文会让一万张卡的库每帧多拷几十 KB。
    if let Some(pv) = lib.preview.as_ref() {
        let src = sources.get(&pv.id);
        let node = pv
            .source_node
            .clone()
            .filter(|_| src.is_some_and(|s| !s.node_id.is_empty()));
        let source_line: Option<(String, bool)> = pv.source_node.as_ref().map(|_| {
            let text = src
                .and_then(|s| s.phrase())
                .map(|s| format!("来源：{s}"))
                .unwrap_or_else(|| FOREIGN_SOURCE.to_string());
            (text, node.is_some())
        });

        let out = ScrollArea::vertical()
            .id_salt("mw-library-preview-scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .vertical_scroll_offset(lib.preview_scroll)
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add_space(content::PREVIEW.pad_top);
                let (open, jumped) = body(ui, m, col.text, pv, source_line.as_ref());
                wants_open = open;
                if jumped {
                    jump_node = node.clone();
                }
                ui.add_space(content::PREVIEW.pad_bottom);
            });
        offset = out.state.clamped_offset().y;
        content_h = out.content_size.y;
        open_id = Some(pv.id.clone());
    } else {
        empty_preview(ui, m, rect);
    }

    lib.preview_scroll = offset;
    if content_h > 0.0 {
        if let Some(req) = scrollbar(
            ui,
            m,
            rect,
            content_h,
            lib.preview_scroll,
            Id::unique("mw-library-preview-bar"),
        ) {
            lib.preview_scroll = req;
        }
    }

    if show_pencil && open_id.is_some() {
        let pr = Rect::from_min_size(
            Pos2::new(
                rect.right() - PENCIL_INSET - content_kit::PAGE_ICON_BUTTON,
                rect.top() + PENCIL_INSET,
            ),
            Vec2::splat(content_kit::PAGE_ICON_BUTTON),
        );
        if icon_at(ui, m, pr, Icon::Note, "编辑这张卡片", false) {
            wants_open = true;
        }
    }

    if wants_open {
        if let Some(id) = open_id {
            crate::trace::event(&format!("library_open id={id} via=preview"));
            sink.push(LibraryAction::Open(id));
        }
        return;
    }
    if let Some(node) = jump_node {
        crate::trace::event(&format!("library_jump_source node={node}"));
        sink.push(LibraryAction::JumpToNode(node));
    }
}

/// 阅读面的正文块。返回 `(是否双击要打开卡片页, 是否点了来源跳转)`。
fn body(
    ui: &mut egui::Ui,
    m: &ContentMetrics,
    width: f32,
    pv: &Preview,
    source_line: Option<&(String, bool)>,
) -> (bool, bool) {
    let mut wants_open = false;

    // 标题：30/42/600，按版心折行（CJK 断行交给 epaint）。
    let title = if pv.title.trim().is_empty() {
        UNTITLED
    } else {
        pv.title.as_str()
    };
    let job = egui::text::LayoutJob::simple(
        title.to_owned(),
        content_kit::font_for(content::PAGE_TITLE),
        c32(m.colors.text_primary),
        width,
    );
    let galley = ui.painter().layout_job(job);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(width, galley.size().y), Sense::click());
    ui.painter().galley(r.min, galley, Color32::WHITE);
    wants_open |= resp.double_clicked();

    ui.add_space(TITLE_GAP);
    let meta = vec![
        Seg {
            text: format!("更新 {}", format_date(pv.updated_at)),
            style: content::META,
            color: c32(m.colors.text_secondary),
            hit: false,
        },
        Seg {
            text: format!("创建 {}", format_date(pv.created_at)),
            style: content::META,
            color: c32(m.colors.text_faint),
            hit: false,
        },
        Seg {
            text: format!("{} 字", reading_count(&pv.body)),
            style: content::META,
            color: c32(m.colors.text_faint),
            hit: false,
        },
    ];
    line(ui, content::META.line_height, &meta, width);

    // 标签一行（§7.5：纯文字 13/19，颜色只在 6px 圆点上；至多一行，放不下就截断）。
    if !pv.tags.is_empty() {
        ui.add_space(META_GAP);
        let (lr, _) = ui.allocate_exact_size(
            Vec2::new(width, content::LIST_EXCERPT.line_height),
            Sense::hover(),
        );
        let mut x = lr.left();
        for t in &pv.tags {
            let label = format!("#{t}");
            let w = text_w(ui, content::LIST_EXCERPT, &label);
            if x + content_kit::TAG_DOT + content_kit::TAG_DOT_GAP + w > lr.right() {
                break;
            }
            ui.painter().circle_filled(
                Pos2::new(x + content_kit::TAG_DOT * 0.5, lr.center().y),
                content_kit::TAG_DOT * 0.5,
                tag_dot(m, t),
            );
            x += content_kit::TAG_DOT + content_kit::TAG_DOT_GAP;
            ui.painter().text(
                Pos2::new(x, lr.center().y),
                Align2::LEFT_CENTER,
                label,
                content_kit::font_for(content::LIST_EXCERPT),
                c32(m.colors.text_secondary),
            );
            x += w + TAG_GAP;
        }
    }

    // 来源一行：能跳就是文字动作，跨文档跳不了就只报一句「另一份文档」。
    let mut jumped = false;
    if let Some((label, can_jump)) = source_line {
        ui.add_space(META_GAP);
        let (lr, _) = ui.allocate_exact_size(
            Vec2::new(width, content::LIST_EXCERPT.line_height),
            Sense::hover(),
        );
        if *can_jump {
            let mut sub = ui.new_child(
                UiBuilder::new()
                    .id_salt("mw-library-preview-source")
                    .max_rect(lr),
            );
            sub.spacing_mut().item_spacing = Vec2::ZERO;
            // `text_action` 用 `content::NAV`（14/21），按同一档量才不会溢出。
            let shown = elide(sub.painter(), content::NAV, label, width);
            if content_kit::text_action(&mut sub, m, &shown, true).clicked() {
                jumped = true;
            }
        } else {
            ui.painter().text(
                Pos2::new(lr.left(), lr.center().y),
                Align2::LEFT_CENTER,
                elide(ui.painter(), content::LIST_EXCERPT, label, width),
                content_kit::font_for(content::LIST_EXCERPT),
                c32(m.colors.text_faint),
            );
        }
    }

    ui.add_space(RULE_ABOVE);
    let (dr, _) = ui.allocate_exact_size(Vec2::new(width, m.hairline.max(1.0)), Sense::hover());
    ui.painter().hline(
        dr.x_range(),
        dr.center().y,
        Stroke::new(m.hairline, c32(m.colors.divider)),
    );
    ui.add_space(RULE_BELOW);

    // 正文：卡片页同一棵 AST，标记全部藏起来（`reading_job` = `OnlyLine(None)`）。
    let palette = MdPalette::from_content(
        c32(m.colors.text_primary),
        c32(m.colors.text_secondary),
        c32(m.colors.text_faint),
        c32(m.colors.accent),
        c32a(m.colors.text_primary, 0.06),
        c32(m.colors.page),
    );
    let text = if pv.body.trim().is_empty() {
        NO_BODY
    } else {
        pv.body.as_str()
    };
    let job = reading_job(text, palette, width);
    let galley = ui.painter().layout_job(job);
    let (r, resp) =
        ui.allocate_exact_size(Vec2::new(width, galley.size().y.max(24.0)), Sense::click());
    ui.painter().galley(r.min, galley, Color32::WHITE);
    wants_open |= resp.double_clicked();

    (wants_open, jumped)
}

/// 在当前游标上画一行分段文字。没有命中高亮，所以强调色传全透明。
fn line(ui: &mut egui::Ui, h: f32, segs: &[Seg], width: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    draw_line(
        ui.painter(),
        r.left(),
        r.center().y,
        META_SEG_GAP,
        segs,
        Color32::TRANSPARENT,
        r,
    );
}

/// §7.5「空预览：左对齐 13/20 `text.faint` 一句话，不画大图标」。
fn empty_preview(ui: &mut egui::Ui, m: &ContentMetrics, rect: Rect) {
    let col = content::PREVIEW.column(rect.width());
    let x = rect.left() + col.side + col.inset;
    let style = content::LIST_EXCERPT;
    ui.painter().text(
        Pos2::new(
            x,
            rect.top() + content::PREVIEW.pad_top + style.line_height * 0.5,
        ),
        Align2::LEFT_CENTER,
        EMPTY_PREVIEW,
        content_kit::font_for(style),
        c32(m.colors.text_faint),
    );
}
