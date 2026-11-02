use super::*;
pub(crate) fn default_card_dir() -> String {
    std::env::var("USERPROFILE")
        .map(|p| format!("{p}\\Desktop\\织思卡片"))
        .unwrap_or_else(|_| "织思卡片".to_string())
}

/// 导出对话框里的分组标题（格式 / 比例 / 背景）。
pub(crate) fn section_label(ch: Chrome, ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(Type::BODY).color(ch.dim));
}

/// 成品尺寸预告。「比例已被上限收小」必须说出来，否则用户会奇怪为什么选了 3× 却不是 3×。
pub(crate) fn size_note(w: f32, h: f32, scale: f32) -> String {
    let (pw, ph, eff) = export::png_size(w, h, scale);
    if (eff - scale).abs() > 0.01 {
        format!("预计 {pw} × {ph} 像素（比例受上限约束，实际 {eff:.2}×）")
    } else {
        format!("预计 {pw} × {ph} 像素")
    }
}

/// PDF 页面预告：页面尺寸按 96dpi 由地图大小换算（与采样倍率无关），
/// 倍率只决定嵌进去那张位图的清晰度。毫米数给的是页面物理尺寸。
pub(crate) fn pdf_note(w: f32, h: f32, scale: f32) -> String {
    let (pw, ph, eff) = export::png_size(w, h, scale);
    let page = format!("页面 {:.0} × {:.0} 毫米", w * 25.4 / 96.0, h * 25.4 / 96.0);
    if (eff - scale).abs() > 0.01 {
        format!("{page}（{pw} × {ph} 像素位图，比例受上限约束，实际 {eff:.2}×）")
    } else {
        format!("{page}（{pw} × {ph} 像素位图）")
    }
}

/// 保存阶段：与 `SaveState` 分开，因为“失败”要绑定失败时的修订号。
///
/// `Saving` 是**已按下、还没写盘**的那一帧：壳层先把「保存中…」画出来，
/// 下一帧开头才阻塞写盘，用户才能真的看见写盘过程而不是一闪而过的假转圈。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum SavePhase {
    #[default]
    Idle,
    Saving,
    Failed {
        rev: u64,
        message: String,
    },
}

/// 卡片页排队中的一次保存：读者页这一帧发出来，下一帧开头才写盘。
///
/// 两段式是为了让顶栏那句「保存中…」说的是真话（同一帧里画完再存，用户永远看不到它），
/// 也顺便把写盘挪出「画这一帧」的路径——和文档 `SavePhase::Saving` 是同一套做法。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CardSave {
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    /// 存完就关页面（返回 / Esc / 「保存并关闭」）。
    pub close: bool,
}

impl SavePhase {
    /// 失败态只对失败那一刻的修订号有效：之后又编辑过，就该回到 Dirty。
    pub(crate) fn failed_at(&self, rev: u64) -> bool {
        matches!(self, Self::Failed { rev: r, .. } if *r == rev)
    }

    pub(crate) fn failure_message(&self) -> Option<&str> {
        match self {
            Self::Failed { message, .. } => Some(message),
            _ => None,
        }
    }
}

/// 标题栏的保存状态。`Untitled` 与 `Saved` 必须分开：新建文档按“无改动”对待，
/// 但它从未落盘，写成“已保存”会骗人。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveState {
    Home,
    Untitled,
    Saving,
    Saved,
    Dirty,
    Failed,
}

impl SaveState {
    pub(crate) fn of(home: bool, has_canvas: bool, dirty: bool, has_path: bool) -> Self {
        if home {
            Self::Home
        } else if !has_canvas {
            Self::Untitled
        } else if dirty {
            Self::Dirty
        } else if has_path {
            Self::Saved
        } else {
            Self::Untitled
        }
    }

    /// 基础四态之上叠保存阶段：Saving 压过一切；Failed 只在同一修订号上可见。
    pub(crate) fn with_phase(base: Self, phase: &SavePhase, rev: u64) -> Self {
        match phase {
            SavePhase::Saving => Self::Saving,
            SavePhase::Failed { .. } if phase.failed_at(rev) => Self::Failed,
            _ => base,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Home => "本地优先",
            Self::Untitled => "未保存到文件",
            Self::Saving => "保存中…",
            Self::Saved => "已保存",
            Self::Dirty => "未保存",
            Self::Failed => "保存失败",
        }
    }

    pub(crate) fn tip(self) -> &'static str {
        match self {
            Self::Home => "本地优先 · 无账号",
            Self::Untitled => "点击保存（Ctrl+S）选择保存位置",
            Self::Saving => "正在写入文件…",
            Self::Saved => "所有修改已写入文件",
            Self::Dirty => "点击保存（Ctrl+S）",
            Self::Failed => "写入失败：点击或按 Ctrl+S 重试",
        }
    }

    /// 失败态把真实原因带进悬停提示；其余状态用固定文案。
    pub(crate) fn tip_with_phase(self, phase: &SavePhase) -> String {
        match (self, phase.failure_message()) {
            (Self::Failed, Some(message)) => format!("写入失败：{message}（Ctrl+S 重试）"),
            _ => self.tip().to_string(),
        }
    }

    /// 只有还没落盘、有未保存修改或上次保存失败时，状态芯片才响应点击。
    pub(crate) fn can_save(self) -> bool {
        matches!(self, Self::Untitled | Self::Dirty | Self::Failed)
    }
}

/// 打开失败弹窗的动作：按钮下标 → 语义。
///
/// 「从最近列表移除」只在来源仍在列表时出现，所以关闭按钮的下标也跟着变；
/// 这个映射单独抽出来，弹窗布局与键盘默认动作才不会各算一套。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenFailedAction {
    Retry,
    OpenOther,
    RemoveRecent,
    Close,
}

pub(crate) fn open_failed_action(index: usize, from_recent: bool) -> Option<OpenFailedAction> {
    match (index, from_recent) {
        (0, _) => Some(OpenFailedAction::Retry),
        (1, _) => Some(OpenFailedAction::OpenOther),
        (2, true) => Some(OpenFailedAction::RemoveRecent),
        (2, false) | (3, true) => Some(OpenFailedAction::Close),
        _ => None,
    }
}

/// 默认动作是「关闭」：失败弹窗先让人看清原因，Enter 不该直接再撞一次。
pub(crate) fn open_failed_close_index(from_recent: bool) -> usize {
    if from_recent {
        3
    } else {
        2
    }
}

/// 标题栏右簇的取舍：状态芯片优先，芯片之后还够 120px 才画文档标题。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TitleCluster {
    pub chip: bool,
    pub title: bool,
}

pub(crate) fn title_cluster(avail: f32, chip_w: f32) -> TitleCluster {
    let chip = avail >= chip_w + 8.0;
    let title = chip && avail - chip_w - 12.0 > 120.0;
    TitleCluster { chip, title }
}

/// 状态栏的选中摘要：单选给主题名，多选给数量；未选中返回 `None`。
pub(crate) fn selection_summary(count: usize, primary: Option<&str>) -> Option<String> {
    match count {
        0 => None,
        1 => {
            let text = primary
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or("未命名主题");
            Some(format!("已选：{}", shorten(text, 18)))
        }
        n => Some(format!("已选 {n} 个主题")),
    }
}

/// 顶栏品牌标记：一枚细线「节点树」徽标 + 产品名。
///
/// 徽标画在强调色淡底上，不依赖字体里有没有「织」这个字形，也不会在 125%/150%
/// 缩放下糊成一团。窄窗只留徽标，把横向空间让给菜单与工具。
pub(crate) fn paint_brand(ui: &mut egui::Ui, ch: Chrome, dim: f32, with_text: bool) {
    let mark = 26.0;
    let width = if with_text { mark + 8.0 + 30.0 } else { mark };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::hover());
    let box_rect = Rect::from_center_size(
        Pos2::new(rect.left() + mark * 0.5, rect.center().y),
        Vec2::splat(mark),
    );
    ui.painter().rect_filled(box_rect, 7.0, ch.accent_soft);
    let c = box_rect.center();
    let ink = ch.accent;
    let s = Stroke::new(1.4, ink);
    let leaves = [
        Pos2::new(c.x + 5.6, c.y - 5.2),
        Pos2::new(c.x + 6.0, c.y + 4.8),
        Pos2::new(c.x - 5.8, c.y + 5.2),
    ];
    for leaf in leaves {
        ui.painter().line_segment([c, leaf], s);
        ui.painter().circle_filled(leaf, 1.8, ink);
    }
    ui.painter().circle_filled(c, 2.2, ink);
    if with_text {
        ui.painter().text(
            Pos2::new(rect.left() + mark + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            "织思",
            FontId::proportional(Type::SMALL),
            ch.faded(ch.text, dim),
        );
    }
}

/// 窗口控制图标：全用 1.2px 矢量线，字符字形在 125%/150% 下会左右跳半像素。
pub(crate) fn paint_win_glyph(painter: &egui::Painter, c: Pos2, glyph: WinGlyph, color: Color32) {
    let s = Stroke::new(mw_core::design::icon::STROKE_CHROME, color);
    match glyph {
        WinGlyph::Minimize => {
            painter.line_segment(
                [
                    Pos2::new(c.x - 4.5, c.y + 0.5),
                    Pos2::new(c.x + 4.5, c.y + 0.5),
                ],
                s,
            );
        }
        WinGlyph::Maximize => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(9.0)),
                0.0,
                s,
                StrokeKind::Inside,
            );
        }
        WinGlyph::Restore => {
            painter.rect_stroke(
                Rect::from_min_size(Pos2::new(c.x - 4.4, c.y - 2.6), Vec2::new(7.0, 7.0)),
                0.0,
                s,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_size(Pos2::new(c.x - 2.0, c.y - 4.4), Vec2::new(7.0, 7.0)),
                0.0,
                s,
                StrokeKind::Inside,
            );
        }
        WinGlyph::Close => {
            painter.line_segment(
                [
                    Pos2::new(c.x - 4.5, c.y - 4.5),
                    Pos2::new(c.x + 4.5, c.y + 4.5),
                ],
                s,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x + 4.5, c.y - 4.5),
                    Pos2::new(c.x - 4.5, c.y + 4.5),
                ],
                s,
            );
        }
    }
}

/// 提示条里只放文件名：全路径在状态栏一行里放不下，也看不出重点。
pub(crate) fn file_label(p: &std::path::Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("文件")
        .to_string()
}

/// 弹层里的整行动作：左图标、主文案、右快捷键，悬停浅灰底。
///
/// `check == Some(..)` 表示这是开关项：开启时图标位画主题色对勾，关闭时画面板图标。
/// 行绘制、快捷键列和键盘导航都走 [`crate::menu`]，标题菜单与右键菜单不再是两套代码。
pub(crate) fn menu_row(
    ch: Chrome,
    ui: &mut egui::Ui,
    icon: MenuIcon,
    label: &str,
    hint: &str,
    check: Option<bool>,
    enabled: bool,
) -> bool {
    crate::menu::popup_row(
        ui,
        ch,
        crate::menu::RowSpec {
            icon: Some(icon),
            label,
            shortcut: hint,
            check,
            enabled,
            submenu: false,
        },
    )
}

/// 菜单里的一行动作：画一行，命中就记下意图并收起菜单。
#[allow(clippy::too_many_arguments)]
pub(crate) fn menu_action(
    ch: Chrome,
    ui: &mut egui::Ui,
    icon: MenuIcon,
    label: &str,
    hint: &str,
    check: Option<bool>,
    enabled: bool,
    action: &mut Option<TitleMenuAction>,
    act: TitleMenuAction,
) {
    if menu_row(ch, ui, icon, label, hint, check, enabled) {
        *action = Some(act);
        ui.close();
    }
}

/// 菜单分组标题（「最近文件」「主题」这类小字行）。
pub(crate) fn menu_section(ch: Chrome, ui: &mut egui::Ui, text: &str) {
    crate::menu::paint_section(ui, ch, text);
}

/// 顶栏文字菜单按钮：宽度随文字走，展开时用强调淡底标出当前组。
/// 按钮本身只是入口，真正的动作行在下方弹层里。
#[allow(clippy::too_many_arguments)]
pub(crate) fn title_menu_button(
    ui: &mut egui::Ui,
    ch: Chrome,
    dim: f32,
    label: &str,
    id: &str,
    menu_w: f32,
    action: &mut Option<TitleMenuAction>,
    add: impl FnOnce(&mut egui::Ui, &mut Option<TitleMenuAction>),
) {
    let font = FontId::proportional(Type::SMALL);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), ch.text);
    let width = (galley.size().x + 24.0).max(48.0);
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(width, mw_core::design::component::MENU_TRIGGER_H),
        Sense::click(),
    );
    let popup_id = egui::Id::unique(("mw-menu", id));
    let open = egui::Popup::menu(&resp).id(popup_id).is_open();
    if resp.hovered() || open {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(mw_core::design::radius::SM as u8),
            if open { ch.accent_soft } else { ch.hover },
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    crate::ui_kit::paint_focus_ring(ui.painter(), ch, rect, mw_core::design::radius::SM, &resp);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        font,
        ch.faded(if open { ch.accent } else { ch.text }, dim),
    );
    egui::Popup::menu(&resp)
        .id(popup_id)
        .gap(8.0)
        .width(menu_w)
        .frame(crate::ui_kit::menu_frame(ch))
        .show(|ui| {
            let nav = crate::menu::PopupNav::begin(ui, id);
            ui.spacing_mut().item_spacing.y = 1.0;
            // 折叠后的「更多」五组菜单一定比屏幕高；普通菜单则让弹层自然长高。
            // 若给普通菜单也套 ScrollArea，它会以弹层“首帧猜高”为上限，
            // 把最后一行截在卡片外面。
            let max_h = (ui.ctx().content_rect().height() - 96.0).max(160.0);
            if id == "more" || max_h < 470.0 {
                // 先撑起弹层，否则 ScrollArea 的视口高度会反过来锁死弹层高度。
                ui.set_min_height(max_h);
                egui::ScrollArea::vertical()
                    .max_height(max_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        add(ui, action);
                    });
            } else {
                add(ui, action);
            }
            nav.end(ui);
        });
}

/// 「文件」菜单。
pub(crate) fn file_menu(
    ch: Chrome,
    ui: &mut egui::Ui,
    m: &MenuCtx,
    act: &mut Option<TitleMenuAction>,
) {
    menu_action(
        ch,
        ui,
        MenuIcon::New,
        "新建",
        &m.k_new,
        None,
        true,
        act,
        TitleMenuAction::New,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::ConceptMap,
        "新建概念图",
        &m.k_new_concept,
        None,
        true,
        act,
        TitleMenuAction::NewConcept,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::New,
        "新建标签页",
        &m.k_new_tab,
        None,
        true,
        act,
        TitleMenuAction::NewTab,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Open,
        "打开…",
        &m.k_open,
        None,
        true,
        act,
        TitleMenuAction::Open,
    );
    if !m.recent.is_empty() {
        menu_section(ch, ui, "最近文件");
        for (title, path) in &m.recent {
            if menu_row(ch, ui, MenuIcon::File, &shorten(title, 26), "", None, true) {
                *act = Some(TitleMenuAction::Recent(path.clone()));
                ui.close();
            }
        }
    }
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::File,
        "导入文件…",
        "Markdown · OPML · XMind",
        None,
        true,
        act,
        TitleMenuAction::Import,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Save,
        "保存",
        &m.k_save,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Save,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::SaveAs,
        "另存为…",
        &m.k_save_as,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::SaveAs,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Export,
        "导出…",
        &m.k_export,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Export,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Home,
        "开始页",
        "",
        None,
        true,
        act,
        TitleMenuAction::Home,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Exit,
        "退出织思",
        "Alt+F4",
        None,
        true,
        act,
        TitleMenuAction::Quit,
    );
}

/// 「编辑」菜单。
pub(crate) fn edit_menu(
    ch: Chrome,
    ui: &mut egui::Ui,
    m: &MenuCtx,
    act: &mut Option<TitleMenuAction>,
) {
    menu_action(
        ch,
        ui,
        MenuIcon::Undo,
        "撤销",
        &m.k_undo,
        None,
        m.can_undo,
        act,
        TitleMenuAction::Undo,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Redo,
        "重做",
        &m.k_redo,
        None,
        m.can_redo,
        act,
        TitleMenuAction::Redo,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::AddChild,
        "添加子主题",
        "Tab",
        None,
        m.can_add,
        act,
        TitleMenuAction::AddChild,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::AddSibling,
        "添加同级主题",
        "Enter",
        None,
        m.can_add,
        act,
        TitleMenuAction::AddSibling,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Delete,
        "删除主题",
        "Delete",
        None,
        m.can_delete,
        act,
        TitleMenuAction::DeleteNode,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Search,
        "查找…",
        &m.k_find,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Find,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Search,
        "命令面板…",
        &m.k_palette,
        None,
        true,
        act,
        TitleMenuAction::Palette,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Theme,
        "快捷键设置…",
        &m.k_prefs,
        None,
        true,
        act,
        TitleMenuAction::Preferences,
    );
}

/// 「视图」菜单。
pub(crate) fn view_menu(
    ch: Chrome,
    ui: &mut egui::Ui,
    m: &MenuCtx,
    act: &mut Option<TitleMenuAction>,
) {
    menu_action(
        ch,
        ui,
        MenuIcon::Panel,
        "全屏大纲",
        &m.k_outline,
        Some(m.outline),
        m.has_canvas,
        act,
        TitleMenuAction::ToggleOutline,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Panel,
        "检查器",
        &m.k_inspector,
        Some(m.inspector),
        m.has_canvas && !m.outline,
        act,
        TitleMenuAction::ToggleInspector,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Panel,
        "小地图",
        "",
        Some(m.minimap),
        m.has_canvas && !m.outline,
        act,
        TitleMenuAction::ToggleMinimap,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Fit,
        "专注模式",
        &m.k_focus,
        Some(m.focus),
        m.has_canvas,
        act,
        TitleMenuAction::FocusMode,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::File,
        "阅读模式",
        &m.k_reading,
        Some(m.reading),
        m.has_canvas,
        act,
        TitleMenuAction::Reading,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Fit,
        "适应画布",
        "Home",
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Fit,
    );
    if let Some(current) = m.theme {
        menu_separator(ch, ui);
        menu_section(ch, ui, "主题");
        // 概念图只列概念图那四套（`docs/33` §4）：混排等于「专属」没做。
        let all = if m.concept_map {
            mw_core::theme::concept_themes()
        } else {
            mw_core::theme::themes()
        };
        for theme in all {
            let hint = if matches!(theme.appearance, mw_core::theme::Appearance::Dark) {
                "深色"
            } else {
                "浅色"
            };
            if menu_row(
                ch,
                ui,
                MenuIcon::Theme,
                &format!("主题：{}", theme.name),
                hint,
                Some(theme.id == current),
                true,
            ) {
                *act = Some(TitleMenuAction::SetTheme(theme.id));
                ui.close();
            }
        }
    }
}

/// 「插入」菜单。
pub(crate) fn insert_menu(
    ch: Chrome,
    ui: &mut egui::Ui,
    m: &MenuCtx,
    act: &mut Option<TitleMenuAction>,
) {
    menu_action(
        ch,
        ui,
        MenuIcon::AddChild,
        "子主题",
        "Tab",
        None,
        m.can_add,
        act,
        TitleMenuAction::AddChild,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::AddSibling,
        "同级主题",
        "Enter",
        None,
        m.can_add,
        act,
        TitleMenuAction::AddSibling,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::Note,
        "笔记卡片",
        "",
        None,
        m.has_canvas,
        act,
        TitleMenuAction::CaptureCard,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Note,
        "今日笔记",
        "",
        None,
        m.notes,
        act,
        TitleMenuAction::DailyNote,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Library,
        "卡片库…",
        "",
        None,
        m.notes,
        act,
        TitleMenuAction::Library,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Tag,
        "标签管理…",
        "",
        None,
        m.notes,
        act,
        TitleMenuAction::TagManager,
    );
    menu_separator(ch, ui);
    menu_action(
        ch,
        ui,
        MenuIcon::File,
        "导入文件…",
        "",
        None,
        true,
        act,
        TitleMenuAction::Import,
    );
}

/// 「帮助」菜单。
pub(crate) fn help_menu(
    ch: Chrome,
    ui: &mut egui::Ui,
    m: &MenuCtx,
    act: &mut Option<TitleMenuAction>,
) {
    menu_action(
        ch,
        ui,
        MenuIcon::Search,
        "快捷键速查",
        &m.k_help,
        None,
        true,
        act,
        TitleMenuAction::Help,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Theme,
        "快捷键设置…",
        &m.k_prefs,
        None,
        true,
        act,
        TitleMenuAction::Preferences,
    );
}

/// 窄窗下的「更多」：五组菜单压进一层，条目收成最常用的一批。
pub(crate) fn more_menu(
    ch: Chrome,
    ui: &mut egui::Ui,
    m: &MenuCtx,
    act: &mut Option<TitleMenuAction>,
) {
    menu_section(ch, ui, "文件");
    menu_action(
        ch,
        ui,
        MenuIcon::New,
        "新建",
        &m.k_new,
        None,
        true,
        act,
        TitleMenuAction::New,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Open,
        "打开…",
        &m.k_open,
        None,
        true,
        act,
        TitleMenuAction::Open,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::File,
        "导入文件…",
        "Markdown · OPML · XMind",
        None,
        true,
        act,
        TitleMenuAction::Import,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Save,
        "保存",
        &m.k_save,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Save,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::SaveAs,
        "另存为…",
        &m.k_save_as,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::SaveAs,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Export,
        "导出…",
        &m.k_export,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Export,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Home,
        "开始页",
        "",
        None,
        true,
        act,
        TitleMenuAction::Home,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Exit,
        "退出织思",
        "Alt+F4",
        None,
        true,
        act,
        TitleMenuAction::Quit,
    );
    menu_separator(ch, ui);
    menu_section(ch, ui, "编辑");
    menu_action(
        ch,
        ui,
        MenuIcon::Undo,
        "撤销",
        &m.k_undo,
        None,
        m.can_undo,
        act,
        TitleMenuAction::Undo,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Redo,
        "重做",
        &m.k_redo,
        None,
        m.can_redo,
        act,
        TitleMenuAction::Redo,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Search,
        "查找…",
        &m.k_find,
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Find,
    );
    menu_separator(ch, ui);
    menu_section(ch, ui, "视图");
    menu_action(
        ch,
        ui,
        MenuIcon::Panel,
        "全屏大纲",
        &m.k_outline,
        Some(m.outline),
        m.has_canvas,
        act,
        TitleMenuAction::ToggleOutline,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Panel,
        "检查器",
        &m.k_inspector,
        Some(m.inspector),
        m.has_canvas && !m.outline,
        act,
        TitleMenuAction::ToggleInspector,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Panel,
        "小地图",
        "",
        Some(m.minimap),
        m.has_canvas && !m.outline,
        act,
        TitleMenuAction::ToggleMinimap,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Fit,
        "适应画布",
        "Home",
        None,
        m.has_canvas,
        act,
        TitleMenuAction::Fit,
    );
    menu_separator(ch, ui);
    menu_section(ch, ui, "插入");
    menu_action(
        ch,
        ui,
        MenuIcon::Note,
        "笔记卡片",
        "",
        None,
        m.has_canvas,
        act,
        TitleMenuAction::CaptureCard,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Library,
        "卡片库…",
        "",
        None,
        m.notes,
        act,
        TitleMenuAction::Library,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Tag,
        "标签管理…",
        "",
        None,
        m.notes,
        act,
        TitleMenuAction::TagManager,
    );
    menu_separator(ch, ui);
    menu_section(ch, ui, "帮助");
    menu_action(
        ch,
        ui,
        MenuIcon::Search,
        "快捷键速查",
        &m.k_help,
        None,
        true,
        act,
        TitleMenuAction::Help,
    );
    menu_action(
        ch,
        ui,
        MenuIcon::Theme,
        "快捷键设置…",
        &m.k_prefs,
        None,
        true,
        act,
        TitleMenuAction::Preferences,
    );
}

/// 菜单分隔线：左右留白，比 egui 默认的整行横线更像原生菜单。
pub(crate) fn menu_separator(ch: Chrome, ui: &mut egui::Ui) {
    crate::menu::paint_separator(ui, ch);
}

/// 从命令行参数里找 `.zmind` 路径（文件关联双击时 Windows 会把路径作为 argv[1]）。
/// 只做扩展名筛选，存在性由调用方检查。
pub(crate) fn zmind_arg(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Option<std::path::PathBuf> {
    args.map(std::path::PathBuf::from).find(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("zmind"))
    })
}

/// 取主窗口句柄（对话框 owner 用）。跟 `apply_backdrop` 同一条路子。
pub(crate) fn window_hwnd(frame: &eframe::Frame) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match frame.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
        _ => None,
    }
}

/// Mica + 圆角。eframe 不暴露 HWND，但 `Frame` 实现了 `HasWindowHandle`，取真句柄。
pub(crate) fn apply_backdrop(frame: &mut eframe::Frame) -> Backdrop {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    let hwnd = match frame.window_handle() {
        Ok(h) => match h.as_raw() {
            RawWindowHandle::Win32(w) => HWND(w.hwnd.get() as *mut core::ffi::c_void),
            other => return Backdrop::Failed(format!("非 Win32 句柄：{other:?}")),
        },
        Err(e) => return Backdrop::Failed(format!("取句柄失败：{e}")),
    };

    unsafe {
        let backdrop: i32 = DWMSBT_MAINWINDOW.0;
        if let Err(e) = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop as *const i32 as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        ) {
            return Backdrop::Failed(format!("DWMWA_SYSTEMBACKDROP_TYPE：{e}"));
        }

        let corner: i32 = DWMWCP_ROUND.0;
        let corner = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const i32 as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        )
        .map_err(|e| e.to_string());

        Backdrop::Applied { corner }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        open_failed_action, open_failed_close_index, zmind_arg, OpenFailedAction, SavePhase,
        SaveState,
    };

    #[test]
    fn open_failed_actions_follow_the_visible_button_set() {
        assert_eq!(open_failed_action(0, true), Some(OpenFailedAction::Retry));
        assert_eq!(
            open_failed_action(1, true),
            Some(OpenFailedAction::OpenOther)
        );
        assert_eq!(
            open_failed_action(2, true),
            Some(OpenFailedAction::RemoveRecent)
        );
        assert_eq!(open_failed_action(3, true), Some(OpenFailedAction::Close));
        assert_eq!(open_failed_close_index(true), 3);

        // 不在最近列表里就没有「移除」，关闭顶到第 2 格。
        assert_eq!(open_failed_action(2, false), Some(OpenFailedAction::Close));
        assert_eq!(open_failed_close_index(false), 2);
        assert_eq!(open_failed_action(3, false), None);
        assert_eq!(open_failed_action(9, true), None);
    }

    #[test]
    fn command_line_finds_a_zmind_argument() {
        let args = ["--foo", "C:/文档/方案.ZMIND", "--bar"]
            .into_iter()
            .map(Into::into);
        assert_eq!(
            zmind_arg(args),
            Some(std::path::PathBuf::from("C:/文档/方案.ZMIND"))
        );
        assert_eq!(zmind_arg(std::iter::empty()), None);
    }

    #[test]
    fn save_phase_overrides_the_base_state() {
        let base = SaveState::of(false, true, true, true);
        assert_eq!(base, SaveState::Dirty);
        assert_eq!(
            SaveState::with_phase(base, &SavePhase::Saving, 7),
            SaveState::Saving
        );

        let failed = SavePhase::Failed {
            rev: 7,
            message: "磁盘只读".into(),
        };
        assert_eq!(
            SaveState::with_phase(base, &failed, 7),
            SaveState::Failed,
            "失败态要压过 Dirty，用户才知道刚才没存进去"
        );
        assert_eq!(
            SaveState::with_phase(base, &failed, 8),
            SaveState::Dirty,
            "失败后又编辑过，旧失败不再代表当前文档"
        );
        assert_eq!(failed.failure_message(), Some("磁盘只读"));
        assert!(!SaveState::Saving.can_save(), "写盘途中不许再排一次保存");
        assert!(SaveState::Failed.can_save(), "失败态点击就是重试");
    }

    #[test]
    fn saving_wins_over_a_stale_failure() {
        let base = SaveState::of(false, true, false, true);
        let failed = SavePhase::Failed {
            rev: 3,
            message: "x".into(),
        };
        assert_eq!(base, SaveState::Saved);
        assert_eq!(SaveState::with_phase(base, &failed, 4), SaveState::Saved);
        assert_eq!(
            SaveState::with_phase(base, &SavePhase::Saving, 3),
            SaveState::Saving
        );
    }
}
