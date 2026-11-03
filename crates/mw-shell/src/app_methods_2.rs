use super::*;

impl Shell {
    /// 窗口级剪贴板：复制/剪切/粘贴节点，以及复制/粘贴样式。和撤销同一条纪律——
    /// 主人是窗口，不是画布，否则「鼠标点一下工具条，再按 Ctrl+C」会静默失灵。
    ///
    /// **文本框握着键盘时一个事件都不碰**：egui 0.37 没有「先替 `TextEdit` 摘走剪贴板键」
    /// 的全局预过滤，`Copy/Cut/Paste` 就是 `events` 里的普通条目，谁先 `retain` 谁赢，
    /// 而面板的 `TextEdit` 要到本帧后面才画。所以我们只在确认没有文本框要它们之后才动手。
    pub(crate) fn window_clipboard(&mut self, ctx: &egui::Context) {
        // 大纲行内编辑的粘贴归编辑器（docs/28 §5.4 多行拆节点）：在节点粘贴
        // 路径之前摘走，否则多行文本会被 TextEdit 整段吃进草稿。
        if self.outline_view && self.outline.paste_into_edit(ctx) {
            return;
        }
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        // `text_edit_focused` 管搜索条 / 命令面板 / 检查器的标题与备注；行内编辑器另有一道
        // 显式护栏：它是画布自己维护的覆盖层，编辑态我们比 egui 更清楚。
        if canvas.input_blocked() || canvas.is_editing() || ctx.text_edit_focused() {
            return;
        }
        let cmd = egui::Modifiers::COMMAND;
        let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        // 系统剪贴板的纯文本只在粘贴帧取一次；复制/剪切帧不碰剪贴板。
        let pasted_text = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| match e {
                egui::Event::Paste(t) => Some(t.clone()),
                _ => None,
            })
        });
        // 带 Shift 的先查（`consume_key` 会放过多余的 Shift），并且查到就不再摘事件。
        let style_copy = ctx.input_mut(|i| i.consume_key(cmd_shift, egui::Key::C));
        let style_paste = ctx.input_mut(|i| i.consume_key(cmd_shift, egui::Key::V));
        let (mut copy, mut cut, mut paste) = (false, false, false);
        if !style_copy && !style_paste {
            // 非拉丁布局的兜底：egui-winit 在这种情况下不发 `Event::Copy`，只发普通按键。
            copy = ctx.input_mut(|i| i.consume_key(cmd, egui::Key::C));
            cut = ctx.input_mut(|i| i.consume_key(cmd, egui::Key::X));
            paste = ctx.input_mut(|i| i.consume_key(cmd, egui::Key::V));
            let (c, t, p) = ctx.input_mut(|i| {
                let (mut copy, mut cut, mut paste) = (false, false, false);
                i.events.retain(|e| match e {
                    egui::Event::Copy => {
                        copy = true;
                        false
                    }
                    egui::Event::Cut => {
                        cut = true;
                        false
                    }
                    egui::Event::Paste(_) => {
                        paste = true;
                        false
                    }
                    _ => true,
                });
                (copy, cut, paste)
            });
            copy |= c;
            cut |= t;
            paste |= p;
        }
        let Some(chord) = clipboard_chord(style_copy, style_paste, copy, cut, paste) else {
            return;
        };
        let outline = self.outline_view;
        let dirty = match chord {
            crate::canvas::ClipboardChord::StylePaste if outline => {
                // 大纲里 Ctrl+Shift+V 是「粘贴到根末尾」；样式粘贴仍在检查器/右键菜单。
                ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
                self.canvas.as_mut().is_some_and(|c| {
                    if c.clipboard_is_empty() {
                        pasted_text
                            .as_deref()
                            .is_some_and(|t| c.paste_plain_text_at_root(t))
                    } else {
                        c.paste_at_root_action()
                    }
                })
            }
            crate::canvas::ClipboardChord::Paste => self.canvas.as_mut().is_some_and(|c| {
                let plain = pasted_text.as_deref().filter(|t| !t.trim().is_empty());
                if c.clipboard_is_empty() {
                    if let Some(text) = plain {
                        let ok = c.paste_plain_text(text);
                        crate::trace::event(&format!(
                            "clipboard paste_text lines={} applied={ok}",
                            text.lines().count()
                        ));
                        return ok;
                    }
                }
                c.apply_clipboard(ctx, chord)
            }),
            other => self
                .canvas
                .as_mut()
                .is_some_and(|c| c.apply_clipboard(ctx, other)),
        };
        crate::trace::event(&format!("clipboard {chord:?} dirty={dirty}"));
        if dirty {
            // 粘贴/剪切/复制出来的新选择可能落在视野外：大纲跟着滚过去。
            if self.outline_view {
                if let Some(id) = self.canvas.as_ref().and_then(|c| c.primary_id()) {
                    self.outline.reveal(&id);
                    self.outline.request_row_focus(id);
                }
            }
            ctx.request_repaint();
        }
    }

    /// 历史走一步。要不要接这两下按键由 `window_shortcuts` 里的 `history_is_ours` 决定，
    /// 而且必须在摘事件**之前**决定——见那里的注释。
    /// 历史走一步。要不要接这两下按键由 `window_shortcuts` 里的 `history_is_ours` 决定，
    /// 而且必须在摘事件**之前**决定——见那里的注释。
    pub(crate) fn step_history(&mut self, redo: bool) {
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        let ok = if redo { canvas.redo() } else { canvas.undo() };
        crate::trace::event(&format!("history step redo={redo} applied={ok}"));
        // 撤销可能把当前行整行删掉（比如撤销新建）：选区的指向跟着文档变了，
        // 行焦点会落在空处。这里按撤销后的主选区把焦点交回大纲。
        if ok && self.outline_view {
            if let Some(canvas) = self.canvas.as_ref() {
                let target = canvas
                    .primary_id()
                    .filter(|id| canvas.session().doc.node(id).is_some());
                if let Some(id) = target {
                    self.outline.request_row_focus(id);
                }
            }
        }
    }

    /// 进入演示（F5）。浮层一律先收起：幕布上不该叠着搜索条或导出对话框。
    pub(crate) fn start_demo(&mut self) {
        if self.home {
            return;
        }
        self.palette.close();
        self.search.close();
        self.help.close();
        self.export.open = false;
        self.context_menu = None;
        self.toast = None;
        // 演示中不弹检查器：先把右侧面板收起，幕布只留画布。
        self.show_inspector = false;
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        if !canvas.start_demo() {
            self.set_toast("没有可演示的节点", true);
        }
    }

    /// 演示态快捷键。只认 Esc / F5 / ← / → / 空格 / H，其余键既不动作也不漏给画布。
    pub(crate) fn demo_shortcuts(&mut self, ctx: &egui::Context) {
        // 卡片页开着时键盘归卡片页：Esc 先关卡片，不直接退出演示。
        if self.note_reader.is_some() {
            return;
        }
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        let mut changed = false;
        let mut demo_card = None;
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::F5)
        }) {
            canvas.stop_demo();
            changed = true;
        } else {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::H)) {
                changed |= canvas.toggle_demo_bar();
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::C)) {
                demo_card = canvas.demo_card();
            }
            let next = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
            });
            let prev =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft));
            if next {
                changed |= canvas.demo_next();
            } else if prev {
                changed |= canvas.demo_prev();
            }
        }
        if let Some(card) = demo_card {
            let reader = self.card_reader_for(&card);
            self.close_light_overlays();
            self.reader_from_library = false;
            self.note_reader = Some(reader);
        }
        if changed {
            ctx.request_repaint();
        }
    }

    /// 底部演示控制条（docs/02 §9）：进度 + 当前主题 + 上一步/下一步/退出，`H` 隐藏。
    /// 演讲者备注：幕布左下角显示当前节点备注/卡片摘要，`C` 打开完整卡片页。
    pub(crate) fn demo_notes(&mut self, ctx: &egui::Context) {
        let Some(info) = self.canvas.as_ref().and_then(Canvas::demo_info) else {
            return;
        };
        let Some(note) = info.note.filter(|s| !s.trim().is_empty()) else {
            return;
        };
        let screen = ctx.input(|i| i.content_rect());
        let width = 360.0_f32.min(screen.width() - 24.0);
        let pos = egui::pos2(16.0, screen.max.y - 132.0);
        egui::Area::new(egui::Id::unique("mw-demo-notes"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                ui.set_width(width);
                egui::Frame::new()
                    .fill(crate::chrome::c32a(
                        mw_core::design::shell::overlay::NOTES_BG,
                    ))
                    .stroke(Stroke::new(
                        1.0,
                        crate::chrome::c32a(mw_core::design::shell::overlay::NOTES_STROKE),
                    ))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(12, 9))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("演讲者备注")
                                .size(Type::SMALL)
                                .color(Color32::from_rgb(0x9A, 0xA2, 0xAD)),
                        );
                        let text: String = note.chars().take(320).collect();
                        ui.label(
                            egui::RichText::new(text)
                                .size(Type::BODY)
                                .color(Color32::from_rgb(0xE9, 0xEC, 0xF1)),
                        );
                        if info.card.is_some() {
                            ui.label(
                                egui::RichText::new("按 C 打开卡片页")
                                    .size(Type::CAPTION)
                                    .color(Color32::from_rgb(0x7F, 0x8A, 0x99)),
                            );
                        }
                    });
            });
    }

    pub(crate) fn demo_bar(&mut self, ctx: &egui::Context) {
        let Some(info) = self.canvas.as_ref().and_then(Canvas::demo_info) else {
            return;
        };
        if info.bar_hidden {
            return;
        }
        let screen = ctx.input(|i| i.content_rect());
        let width = 460.0_f32.min(screen.width() - 24.0).max(280.0);
        let pos = egui::pos2(screen.center().x - width * 0.5, screen.max.y - 76.0);
        let mut action = None;
        egui::Area::new(egui::Id::unique("mw-demo-bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                ui.set_width(width);
                egui::Frame::new()
                    .fill(crate::chrome::c32a(mw_core::design::shell::overlay::BAR_BG))
                    .stroke(Stroke::new(
                        1.0,
                        crate::chrome::c32a(mw_core::design::shell::overlay::BAR_STROKE),
                    ))
                    .corner_radius(14)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(
                            mw_core::design::shell::overlay::BAR_SHADOW_ALPHA,
                        ),
                    })
                    .inner_margin(egui::Margin::symmetric(12, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{} / {}", info.index + 1, info.total))
                                    .size(Type::LABEL)
                                    .family(eframe::egui::FontFamily::Name(
                                        crate::fonts::SEMIBOLD.into(),
                                    ))
                                    .color(Color32::from_rgb(0xE9, 0xEC, 0xF1)),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(shorten(&info.title, 16))
                                    .size(Type::BODY)
                                    .color(Color32::from_rgb(0x9A, 0xA2, 0xAD)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if demo_button(ui, "退出", 40.0, true, "退出演示（Esc）")
                                        .clicked()
                                    {
                                        action = Some(DemoBarAction::Exit);
                                    }
                                    if demo_button(
                                        ui,
                                        "›",
                                        30.0,
                                        info.index + 1 < info.total,
                                        "下一步（→ / 空格）",
                                    )
                                    .clicked()
                                    {
                                        action = Some(DemoBarAction::Next);
                                    }
                                    if demo_button(ui, "‹", 30.0, info.index > 0, "上一步（←）")
                                        .clicked()
                                    {
                                        action = Some(DemoBarAction::Prev);
                                    }
                                },
                            );
                        });
                    });
            });
        let handled = match action {
            Some(DemoBarAction::Prev) => self.canvas.as_mut().is_some_and(Canvas::demo_prev),
            Some(DemoBarAction::Next) => self.canvas.as_mut().is_some_and(Canvas::demo_next),
            Some(DemoBarAction::Exit) => self.canvas.as_mut().is_some_and(Canvas::stop_demo),
            None => false,
        };
        if handled {
            ctx.request_repaint();
        }
    }

    /// 搜索和节点内编辑不能同时抢键盘：编辑中先按 F2 提交或 Esc 取消再搜。
    pub(crate) fn open_search(&mut self) {
        if self.canvas.as_ref().is_some_and(Canvas::is_editing) {
            return;
        }
        self.palette.close();
        self.search.open();
    }

    pub(crate) fn run_search(&mut self, action: panels::search::Action) {
        use panels::search::Action as SA;
        match action {
            SA::Jump(id) => {
                let (ok, focus_exited) = match self.canvas.as_mut() {
                    Some(c) => {
                        let focused = !c.focus_path().is_empty();
                        let ok = c.select_node_keep_focus(&id);
                        (ok, ok && focused && c.focus_path().is_empty())
                    }
                    None => (false, false),
                };
                crate::trace::event(&format!("search_jump id={id} ok={}", u8::from(ok)));
                if !ok {
                    self.set_toast("这个主题在当前视图之外", true);
                } else if focus_exited {
                    self.set_toast("已退出聚焦并跳转", false);
                }
            }
            SA::JumpRelationship { from, id } => {
                if let Some(canvas) = self.canvas.as_mut() {
                    let ok = canvas.select_relationship_keep_focus(&from, &id);
                    crate::trace::event(&format!(
                        "search_rel_jump from={from} id={id} ok={}",
                        u8::from(ok)
                    ));
                }
            }
            SA::JumpCard(id) => {
                self.search.close();
                self.open_library_card(&id);
            }
            SA::Close => {
                self.search.close();
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.request_canvas_focus();
                }
            }
            SA::ReplaceAll { query, to } => {
                // 只替换字面命中（拼音命中没有可替换的位置），整体一条 Batch：一次撤销全回去。
                let Some(canvas) = self.canvas.as_mut() else {
                    return;
                };
                let ids: Vec<String> = canvas
                    .search_nodes(&query, mw_core::MatchMode::Pinyin)
                    .into_iter()
                    .map(|h| h.id)
                    .collect();
                let mut commands = Vec::new();
                let mut count = 0usize;
                for id in ids {
                    let Some(text) = canvas.node_text(&id).map(str::to_string) else {
                        continue;
                    };
                    let (next, n) = mw_core::search::replace_all(&text, &query, &to);
                    if n > 0 {
                        count += n;
                        commands.push(mw_core::Command::SetText { id, text: next });
                    }
                }
                if commands.is_empty() {
                    self.set_toast("没有可替换的字面匹配", true);
                    return;
                }
                let ok = canvas.apply_command(mw_core::Command::Batch { commands });
                if ok {
                    self.set_toast(format!("已替换 {count} 处"), false);
                } else {
                    self.set_toast("替换未生效", true);
                }
            }
            SA::ReplaceOne { id, text } => {
                // 一次 SetText 就是一步撤销；只给可见节点选中+居中，
                // 不借 `select_node_keep_focus` 的展开（那会多一条撤销，违背「一次只回这一步」）。
                let Some(canvas) = self.canvas.as_mut() else {
                    return;
                };
                let Some(old) = canvas.node_text(&id).map(str::to_string) else {
                    self.set_toast("这条结果已经不存在", true);
                    return;
                };
                if !canvas.apply_command(mw_core::Command::SetText {
                    id: id.clone(),
                    text: text.clone(),
                }) {
                    self.set_toast("替换未生效", true);
                    return;
                }
                crate::trace::event(&format!(
                    "search_replace_one id={id} chars={} before_chars={}",
                    text.chars().count(),
                    old.chars().count()
                ));
                // 只在节点已经排得出来时选中/居中（不经过展开分支，保证一次撤销只回这一步）。
                if canvas.node_visible(&id) {
                    canvas.select_node_keep_focus(&id);
                }
                self.set_toast("已替换 1 处", false);
            }
        }
    }

    /// `F6`：在画布与面板之间交接键盘（Windows 的跨面板惯例）。
    /// 画布默认握着键盘——`Tab` 是「加子主题」，这是思维导图的肌肉记忆；但画布一旦
    /// 握着键盘又把 `Tab` 锁在自己身上，整排面板就只剩鼠标能点。交出去之后画布连焦点
    /// 都不争（不可聚焦），`Tab` 才会在面板里转；点一下画布或再按 `F6` 收回。
    ///
    /// 判断只看模式位，不看 egui 里当下的焦点：焦点会被点面板、`Esc`、面板控件抢走，
    /// 拿它当依据会让同一个键走进不同分支，状态栏的指示也跟着说谎。
    /// `F6`：在画布与面板之间交接键盘（Windows 的跨面板惯例）。
    /// 画布默认握着键盘——`Tab` 是「加子主题」，这是思维导图的肌肉记忆；但画布一旦
    /// 握着键盘又把 `Tab` 锁在自己身上，整排面板就只剩鼠标能点。交出去之后画布连焦点
    /// 都不争（不可聚焦），`Tab` 才会在面板里转；点一下画布或再按 `F6` 收回。
    ///
    /// 判断只看模式位，不看 egui 里当下的焦点：焦点会被点面板、`Esc`、面板控件抢走，
    /// 拿它当依据会让同一个键走进不同分支，状态栏的指示也跟着说谎。
    pub(crate) fn cycle_pane_focus(&mut self, ctx: &egui::Context) {
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        if canvas.keys_in_panels() {
            canvas.take_keys_back(ctx);
        } else {
            canvas.hand_keys_to_panels(ctx);
        }
        // 探针要区分两件事：`F6` 根本没送到，还是送到了但判给了另一边。像素分不出——
        // 画布不按焦点画任何东西，两种情况的画面一模一样。状态栏那行指示倒是有像素，
        // 但它整条是强调色，改一次配色就会重画一遍，不能当焦点证据用。
        crate::trace::event(if canvas.keys_in_panels() {
            "pane-panel"
        } else {
            "pane-canvas"
        });
    }

    pub(crate) fn toggle_inspector(&mut self) {
        self.show_inspector = !self.show_inspector;
        if !self.show_inspector {
            if let Some(canvas) = self.canvas.as_mut() {
                canvas.request_canvas_focus();
            }
        }
    }

    /// 缩略卡（M30）开合。展开态与折叠态一起回到展开：用户按的是「显示/隐藏」，
    /// 不是「在折叠与隐藏之间切换」——隐藏后再打开却只剩一个 34×26 角标会很意外。
    pub(crate) fn toggle_minimap(&mut self) {
        self.show_minimap = !self.show_minimap;
        if self.show_minimap {
            self.minimap_collapsed = false;
        }
        self.window_state.set_show_minimap(self.show_minimap);
        crate::trace::event(&format!("minimap toggle on={}", self.show_minimap as u8));
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.request_canvas_focus();
        }
    }

    /// 按右键落点与当前选择组装菜单（docs/06 §6）。
    pub(crate) fn build_context_menu(&self, req: canvas::ContextRequest) -> Option<panels::Menu> {
        use canvas::ContextTarget;
        use panels::context::{Action as A, Item};
        let canvas = self.canvas.as_ref()?;
        let items = match req.target {
            ContextTarget::Blank => {
                let mut items = vec![
                    Item::row(
                        if canvas.is_concept_map() {
                            "新建概念"
                        } else {
                            "新建自由主题"
                        },
                        "双击空白",
                        A::AddFloating,
                        true,
                    ),
                    Item::row("粘贴", "Ctrl+V", A::PasteAtRoot, canvas.has_clipboard()),
                ];
                // 概念图是命题网：位置靠人摆，摆乱了给一次自动整理（CM-04）。
                if canvas.is_concept_map() {
                    items.push(Item::sep());
                    items.push(Item::row("整理布局", "", A::ArrangeConceptMap, true));
                }
                items.push(Item::sep());
                items.push(Item::row("适应画布", "Ctrl+1", A::FitView, true));
                items.push(Item::row(
                    "100%",
                    "Ctrl+0",
                    A::ResetZoom,
                    (canvas.zoom() - 1.0).abs() > f32::EPSILON,
                ));
                // 折叠/展开是树语义：概念图没有子树，菜单里不许出现按不动的条目（CM-05）。
                if !canvas.is_concept_map() {
                    items.push(Item::sep());
                    items.push(Item::row("全部展开", "Ctrl+Shift+/", A::ExpandAll, true));
                    items.push(Item::row("全部折叠", "Ctrl+/", A::CollapseAll, true));
                }
                items.push(Item::sep());
                items.push(Item::row(
                    "打开文档所在文件夹",
                    "",
                    A::RevealFolder,
                    canvas.session().path.is_some(),
                ));
                items
            }
            ContextTarget::Relationship => {
                // 当前生效那一档打勾：四档互斥，菜单里必须能看出现在是哪一档，
                // 否则点了「反向 ←」之后只有端点变了，菜单看起来跟点之前一模一样。
                let current = canvas.selected_relationship_head();
                let mut heads = Vec::new();
                for (label, head) in [
                    ("无箭头", mw_core::model::ArrowHead::Off),
                    ("单向 →", mw_core::model::ArrowHead::Forward),
                    ("反向 ←", mw_core::model::ArrowHead::Backward),
                    ("双向 ↔", mw_core::model::ArrowHead::Both),
                ] {
                    heads.push(Item::row_checked(
                        label,
                        "",
                        A::SetRelationshipHead(head),
                        true,
                        current == Some(head),
                    ));
                }
                let n = canvas.relationship_count();
                vec![
                    Item::row("编辑关系线标签", "F2", A::EditRelationshipLabel, n == 1),
                    Item::sep(),
                    Item::submenu("箭头方向", true, heads),
                    Item::submenu(
                        "线型",
                        true,
                        vec![
                            Item::row_checked(
                                "实线",
                                "",
                                A::SetRelationshipDashed(false),
                                true,
                                canvas.selected_relationship_dashed() == Some(false),
                            ),
                            Item::row_checked(
                                "虚线",
                                "",
                                A::SetRelationshipDashed(true),
                                true,
                                canvas.selected_relationship_dashed() == Some(true),
                            ),
                        ],
                    ),
                    // 「反向」与「箭头方向」是同一件事的两个粒度（改箭头 vs 改整条命题），
                    // 放在一起；也避免把上面的子菜单整体下移——`cm-arrow-menu-click`
                    // 的脚本点的是子菜单那一行的绝对坐标。
                    Item::row("反向连接", "", A::ReverseRelationship, true),
                    Item::sep(),
                    Item::row(
                        if n > 1 {
                            format!("删除 {n} 条关系线")
                        } else {
                            "删除关系线".to_string()
                        },
                        "Delete",
                        A::DeleteRelationship,
                        true,
                    ),
                ]
            }
            ContextTarget::Group => vec![
                Item::row("选中组内概念", "", A::SelectGroupMembers, true),
                Item::sep(),
                Item::row("解组（保留概念）", "Delete", A::DeleteGroup, true),
                Item::sep(),
                {
                    let mut colors = Vec::new();
                    for (label, hex) in [
                        ("默认", None),
                        ("红", Some("#f6212d")),
                        ("蓝", Some("#318de1")),
                        ("绿", Some("#6dc354")),
                        ("橙", Some("#fe8f0b")),
                    ] {
                        colors.push(Item::row(label, "", A::SetGroupColor(hex), true));
                    }
                    Item::submenu("分组颜色", true, colors)
                },
            ],
            ContextTarget::Annotation => {
                let ann = canvas.annotation_selection()?.clone();
                let count = canvas.annotation_count();
                if count > 1 {
                    vec![Item::row(
                        format!("删除选中的 {count} 条注解"),
                        "Delete",
                        A::DeleteAnnotation,
                        true,
                    )]
                } else {
                    let mut items = Vec::new();
                    match ann.kind {
                        mw_core::model::AnnotationKind::Summary => {
                            items.push(Item::row("编辑概要文字", "F2", A::EditSummary, true));
                            items.push(Item::sep());
                            items.push(Item::row("删除概要", "Delete", A::DeleteAnnotation, true));
                        }
                        mw_core::model::AnnotationKind::Boundary => {
                            items.push(Item::row("删除边界", "Delete", A::DeleteAnnotation, true));
                            items.push(Item::sep());
                            let mut colors = Vec::new();
                            for (label, hex) in [
                                ("默认", None),
                                ("红", Some("#f6212d")),
                                ("蓝", Some("#318de1")),
                                ("绿", Some("#6dc354")),
                                ("橙", Some("#fe8f0b")),
                            ] {
                                colors.push(Item::row(label, "", A::SetBoundaryColor(hex), true));
                            }
                            items.push(Item::submenu("边界颜色", true, colors));
                        }
                    }
                    items
                }
            }
            // 概念图（CM-05）：节点菜单不许留树语义——没有子主题、没有概要/边界、
            // 没有折叠/聚焦。概念之间的结构只由「连线 + 链接词」表达。
            ContextTarget::Node if canvas.is_concept_map() && canvas.selected_count() > 1 => {
                let all_pinned = canvas
                    .selected_concept_ids()
                    .iter()
                    .all(|id| canvas.is_concept_pinned(id));
                let pin_label = if all_pinned {
                    "取消钉住位置"
                } else {
                    "钉住位置"
                };
                let mut items = vec![
                    Item::row("整理选中概念", "", A::ArrangeSelectedConcepts, true),
                    Item::row("整理全部布局", "", A::ArrangeConceptMap, true),
                    Item::sep(),
                    Item::row(pin_label, "", A::ToggleConceptPinned, true),
                    Item::sep(),
                    Item::row("剪切", "Ctrl+X", A::Cut, true),
                    Item::row("复制", "Ctrl+C", A::Copy, true),
                    Item::row("粘贴", "Ctrl+V", A::Paste, canvas.has_clipboard()),
                    Item::sep(),
                    Item::row("删除概念", "Delete", A::Delete, canvas.can_delete()),
                ];
                // 两颗概念是「一条命题」的天然单位：菜单最上面直接给连起来（`docs/30` §5.4.2）。
                // 三颗以上不出现——命题只连两端，给一个点不动的条目等于骗人。
                if canvas.selected_count() == 2 {
                    items.insert(
                        0,
                        Item::row("连接这两个概念", "Ctrl+L", A::BeginRelationship, true),
                    );
                    items.insert(1, Item::sep());
                }
                // 分组（CM-18）：两颗以上就能圈。放在最上面——它是「把这几颗打包」，
                // 和下面的排列/复制不是一类动作。
                items.insert(0, Item::row("创建分组", "", A::CreateGroup, true));
                if let Some(sub) = arrange_submenu(canvas) {
                    items.push(Item::sep());
                    items.push(sub);
                }
                items
            }
            ContextTarget::Node if canvas.is_concept_map() => {
                let is_pinned = canvas
                    .primary_id()
                    .as_deref()
                    .is_some_and(|id| canvas.is_concept_pinned(id));
                let pin_label = if is_pinned {
                    "取消钉住位置"
                } else {
                    "钉住位置"
                };
                let items = vec![
                    // CmapTools 的 New Concept：从概念出发新建的**是连着的**概念（`docs/30` §5.4.2）。
                    // 想要一颗孤立的盒子另有一行——此前只有前者，结果和思维导图「新建子主题」一样。
                    Item::row_icon(
                        "新建概念并连接",
                        "Tab",
                        Icon::AddChild,
                        A::AddLinkedConcept,
                        true,
                    ),
                    Item::row("新建独立概念", "", A::AddFloating, true),
                    Item::row("编辑概念", "F2", A::EditText, true),
                    Item::row("连接到…", "Ctrl+L", A::BeginRelationship, true),
                    Item::sep(),
                    Item::row(pin_label, "", A::ToggleConceptPinned, true),
                    Item::row("整理布局", "", A::ArrangeConceptMap, true),
                    Item::sep(),
                    Item::row("剪切", "Ctrl+X", A::Cut, true),
                    Item::row("复制", "Ctrl+C", A::Copy, true),
                    Item::row("粘贴", "Ctrl+V", A::Paste, canvas.has_clipboard()),
                    Item::row_icon(
                        "删除概念",
                        "Delete",
                        Icon::Delete,
                        A::Delete,
                        canvas.can_delete(),
                    ),
                    Item::sep(),
                    Item::row_icon("关联卡片笔记", "", Icon::Note, A::NewCard, true),
                    // 概念菜单此前只有「打开样式检查器」，没有复制/粘贴样式——
                    // 快捷键（Ctrl+Shift+C/V）一直是全局可用的，缺的只是入口（CM-18）。
                    Item::row("复制样式", "Ctrl+Shift+C", A::CopyStyle, true),
                    Item::row(
                        "粘贴样式",
                        "Ctrl+Shift+V",
                        A::PasteStyle,
                        canvas.has_style_clipboard(),
                    ),
                    Item::row("打开样式检查器", "", A::SetStyle, true),
                    Item::sep(),
                    Item::row("适应画布", "Ctrl+1", A::FitView, true),
                ];
                items
            }
            ContextTarget::Node if canvas.selected_count() > 1 => {
                let mut items = vec![
                    Item::row("设置样式", "", A::SetStyle, true),
                    Item::row(
                        "添加概要",
                        "Ctrl+]",
                        A::AddSummary,
                        canvas.can_annotate_selection(),
                    ),
                    Item::row(
                        "添加边界",
                        "Ctrl+B",
                        A::AddBoundary,
                        canvas.can_annotate_selection(),
                    ),
                    Item::row(
                        "折叠",
                        "",
                        A::CollapseSelected,
                        canvas.can_collapse_selection(),
                    ),
                    Item::row(
                        "移入同一父级",
                        "",
                        A::MoveSameParent,
                        canvas.can_move_same_parent(),
                    ),
                    Item::sep(),
                    Item::row("复制", "Ctrl+C", A::Copy, true),
                    Item::row("删除", "Delete", A::Delete, canvas.can_delete()),
                ];
                // 树内节点由布局摆位，只有自由主题能被前端对齐/分布。
                // 八条排布命令一次性铺开会把菜单拉得比屏幕还长，收进子菜单。
                if let Some(sub) = arrange_submenu(canvas) {
                    items.push(Item::sep());
                    items.push(sub);
                }
                items
            }
            ContextTarget::Node => {
                let mut items = vec![
                    Item::row_icon("新建子主题", "Tab", Icon::AddChild, A::AddChild, true),
                    Item::row_icon(
                        "新建同级主题",
                        "Enter",
                        Icon::AddSibling,
                        A::AddSibling,
                        true,
                    ),
                    Item::sep(),
                    Item::row("剪切", "Ctrl+X", A::Cut, canvas.can_delete()),
                    Item::row("复制", "Ctrl+C", A::Copy, true),
                    Item::row("粘贴", "Ctrl+V", A::Paste, canvas.has_clipboard()),
                    Item::row_icon(
                        "删除",
                        "Delete",
                        Icon::Delete,
                        A::Delete,
                        canvas.can_delete(),
                    ),
                    Item::sep(),
                    Item::row("编辑文字", "F2", A::EditText, true),
                    Item::row("插入关系线", "Ctrl+L", A::BeginRelationship, true),
                    Item::row_icon("新建卡片笔记", "", Icon::Note, A::NewCard, true),
                    Item::sep(),
                    Item::row(
                        "添加概要",
                        "Ctrl+]",
                        A::AddSummary,
                        canvas.can_annotate_selection(),
                    ),
                    Item::row(
                        "添加边界",
                        "Ctrl+B",
                        A::AddBoundary,
                        canvas.can_annotate_selection(),
                    ),
                    Item::row("分离为自由主题", "", A::Detach, canvas.can_detach_primary()),
                    Item::row("设为根主题", "", A::SetRoot, canvas.can_set_root_primary()),
                    Item::sep(),
                    Item::submenu(
                        "样式",
                        true,
                        vec![
                            Item::row("复制样式", "Ctrl+Shift+C", A::CopyStyle, true),
                            Item::row(
                                "粘贴样式",
                                "Ctrl+Shift+V",
                                A::PasteStyle,
                                canvas.has_style_clipboard(),
                            ),
                            Item::sep(),
                            Item::row("打开样式检查器", "", A::SetStyle, true),
                        ],
                    ),
                    Item::sep(),
                ];
                match canvas.primary_collapsed() {
                    Some(collapsed) => items.push(Item::row(
                        if collapsed { "展开" } else { "折叠" },
                        "/",
                        A::ToggleCollapse,
                        true,
                    )),
                    None => items.push(Item::row("折叠", "/", A::ToggleCollapse, false)),
                }
                items.push(Item::row_icon(
                    "聚焦",
                    "Alt+Enter",
                    Icon::Focus,
                    A::Focus,
                    canvas.can_focus_primary(),
                ));
                items
            }
        };
        Some(panels::Menu::new(req.pos, items))
    }

    /// 大纲行菜单：与画布节点菜单同一套引擎，但只列大纲面看得见结果的动作。
    /// 画布专属项（关系线/卡片/概要/边界/分离/样式）不进来——点了像没反应。
    fn build_outline_row_menu(
        &self,
        target: &str,
        pos: egui::Pos2,
        multi: bool,
    ) -> panels::outline::RowMenu {
        use crate::menu::Entry;
        use panels::outline::RowAction as R;
        let canvas = self.canvas.as_ref();
        let node = canvas.and_then(|c| c.session().doc.node(target));
        let is_root = canvas.is_some_and(|c| c.session().doc.root_id == target);
        let has_children = node.is_some_and(|n| !n.children.is_empty());
        let collapsed = node.is_some_and(|n| n.collapsed);
        let can_delete = canvas.is_some_and(|c| c.can_delete());
        // 概念图大纲（C20）：动作集只有命题网语义——建边 / 断边 / 改短语。
        // 树级项（新建子主题/同级、折叠、聚焦）一条都不收（docs/30 §5.2/§5.7）。
        let concept = canvas.is_some_and(|c| c.session().doc.is_concept_map());
        let has_edge = self.outline.row_edge(target).is_some();
        let items = if concept {
            panels::outline::concept_row_items(has_edge, can_delete, multi)
        } else if multi {
            vec![
                Entry::action(
                    "折叠",
                    "",
                    R::CollapseSelected,
                    canvas.is_some_and(|c| c.can_collapse_selection()),
                ),
                Entry::action(
                    "移入同一父级",
                    "",
                    R::MoveSameParent,
                    canvas.is_some_and(|c| c.can_move_same_parent()),
                ),
                Entry::Separator,
                Entry::action("复制", "Ctrl+C", R::Copy, true),
                Entry::action("删除", "Delete", R::Delete, can_delete),
            ]
        } else {
            vec![
                // 大纲里 Tab 是缩进（docs/28 §5.5），加子级没有直接键：标签留空，
                // 不把画布菜单的 Tab 抄过来骗人。
                Entry::action("新建子主题", "", R::AddChild, true),
                Entry::action("新建同级主题", "Enter", R::AddSibling, !is_root),
                Entry::Separator,
                Entry::action("剪切", "Ctrl+X", R::Cut, can_delete),
                Entry::action("复制", "Ctrl+C", R::Copy, true),
                Entry::action(
                    "粘贴",
                    "Ctrl+V",
                    R::Paste,
                    canvas.is_some_and(|c| c.has_clipboard()),
                ),
                Entry::action("删除", "Delete", R::Delete, can_delete),
                Entry::Separator,
                Entry::action("编辑文字", "F2", R::EditText, true),
                Entry::Separator,
                Entry::action(
                    if collapsed { "展开" } else { "折叠" },
                    "Alt+.",
                    R::ToggleCollapse,
                    has_children,
                ),
                Entry::action(
                    "聚焦",
                    "Alt+Enter",
                    R::Focus,
                    !is_root && canvas.is_some_and(|c| c.can_focus_primary()),
                ),
                Entry::action("设为根主题", "", R::SetRoot, !is_root && !concept),
            ]
        };
        crate::trace::event(&format!(
            "outline_row_menu target={target} multi={multi} concept={} entries={}",
            concept as u8,
            items.len()
        ));
        panels::outline::RowMenu {
            target: target.to_string(),
            multi,
            menu: crate::menu::Menu::new(pos, items),
        }
    }

    /// 行菜单动作落地：全部复用大纲既有动作，键盘与菜单共享一套语义与撤销。
    pub(crate) fn run_outline_row_action(
        &mut self,
        ctx: &egui::Context,
        action: panels::outline::RowAction,
        target: &str,
        multi: bool,
    ) {
        use panels::outline::Action as OA;
        use panels::outline::RowAction as R;
        crate::trace::event(&format!(
            "outline_row_action kind={action:?} target={target} multi={multi}"
        ));
        match action {
            R::AddChild => self.apply_outline_action(OA::AddChild(target.to_string())),
            R::AddSibling => self.apply_outline_action(OA::AddSibling(target.to_string())),
            // 概念图行菜单（C20）：四个动作与快捷键落到同一个 `Action`，撤销/焦点全同源。
            R::LinkConcept => self.apply_outline_action(OA::LinkConcept(target.to_string())),
            R::NewConcept => self.apply_outline_action(OA::NewConcept(target.to_string())),
            R::EditVia => self.apply_outline_action(OA::EditVia(target.to_string())),
            R::Unlink => match self.outline.row_edge(target) {
                Some((from, rel)) => self.apply_outline_action(OA::UnlinkConcept { from, rel }),
                // 根行/孤立概念没有进来的边：菜单项本就置灰，这里再兜一层。
                None => crate::trace::event(&format!(
                    "outline_concept_unlink_skipped id={target} reason=no_incoming_edge"
                )),
            },
            R::EditText => self.outline.begin_edit(target),
            R::Cut => {
                // 与画布菜单同一口径：进程内剪贴板 + 系统纯文本，再按行删除。
                let text = self.canvas.as_mut().map(|canvas| {
                    canvas.copy_action();
                    canvas.clipboard_plain_text()
                });
                if let Some(text) = text.filter(|t| !t.is_empty()) {
                    ctx.copy_text(text);
                }
                if multi {
                    let ids = self
                        .canvas
                        .as_ref()
                        .map(|c| c.selected_ids())
                        .unwrap_or_default();
                    self.apply_outline_action(OA::DeleteMany(ids));
                } else {
                    self.apply_outline_action(OA::Delete(target.to_string()));
                }
            }
            R::Copy => {
                let text = self.canvas.as_mut().map(|canvas| {
                    canvas.copy_action();
                    canvas.clipboard_plain_text()
                });
                if let Some(text) = text.filter(|t| !t.is_empty()) {
                    ctx.copy_text(text);
                }
            }
            R::Paste => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.paste_action();
                }
                if let Some(id) = self.canvas.as_ref().and_then(|c| c.primary_id()) {
                    self.outline.request_row_focus(id);
                }
            }
            R::Delete => {
                if multi {
                    let ids = self
                        .canvas
                        .as_ref()
                        .map(|c| c.selected_ids())
                        .unwrap_or_default();
                    self.apply_outline_action(OA::DeleteMany(ids));
                } else {
                    self.apply_outline_action(OA::Delete(target.to_string()));
                }
            }
            R::ToggleCollapse => self.apply_outline_action(OA::Toggle(target.to_string())),
            R::Focus => self.apply_outline_action(OA::Focus(target.to_string())),
            // M31：大纲行菜单的根主题项直接对着这一行，不要求它已经是画布主选。
            R::SetRoot => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.set_root_for(target);
                }
                self.outline_keep_selection_visible();
            }
            R::CollapseSelected => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.collapse_selected_action();
                }
                self.outline_keep_selection_visible();
            }
            R::MoveSameParent => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.move_same_parent_action();
                }
                if let Some(id) = self.canvas.as_ref().and_then(|c| c.primary_id()) {
                    self.outline.request_row_focus(id);
                }
            }
        }
    }

    /// 右键菜单的动作落地。和命令面板一样：菜单只发意图，改文档的活儿在这儿。
    pub(crate) fn run_context_action(
        &mut self,
        ctx: &egui::Context,
        action: panels::context::Action,
    ) {
        use panels::context::Action as A;
        match action {
            A::SetStyle => self.show_inspector = true,
            A::NewCard => self.capture_card_for_selected(),
            A::RevealFolder => {
                let path = self
                    .canvas
                    .as_ref()
                    .and_then(|canvas| canvas.session().path.clone());
                match path {
                    Some(path) => {
                        if let Err(e) = files::reveal_in_explorer(&path) {
                            self.set_toast(e, true);
                        }
                    }
                    None => self.set_toast("文档尚未保存到磁盘", true),
                }
            }
            _ => {
                let Some(canvas) = self.canvas.as_mut() else {
                    return;
                };
                // 整理布局会丢掉所有手工位置，动过几颗要在落地处说出来（可撤销也要说）。
                let mut arranged: Option<usize> = None;
                let mut pinned_toast: Option<(&'static str, bool)> = None;
                match action {
                    A::AddChild => {
                        canvas.add_child();
                    }
                    A::AddSibling => {
                        canvas.add_sibling();
                    }
                    A::Cut => {
                        canvas.cut_action();
                        let text = canvas.clipboard_plain_text();
                        if !text.is_empty() {
                            ctx.copy_text(text);
                        }
                    }
                    A::Copy => {
                        canvas.copy_action();
                        let text = canvas.clipboard_plain_text();
                        if !text.is_empty() {
                            ctx.copy_text(text);
                        }
                    }
                    A::Paste => {
                        canvas.paste_action();
                    }
                    A::PasteAtRoot => {
                        canvas.paste_at_root_action();
                    }
                    A::Delete => {
                        canvas.delete_selected();
                    }
                    A::EditText => {
                        canvas.edit_primary_action();
                    }
                    A::BeginRelationship => {
                        canvas.begin_relationship_action();
                    }
                    A::AddLinkedConcept => {
                        canvas.add_concept_relative(crate::canvas::AddKind::Child);
                    }
                    A::ReverseRelationship => {
                        canvas.reverse_selected_relationship();
                    }
                    A::SetRelationshipDashed(dashed) => {
                        canvas.set_selected_relationship_dashed(dashed);
                    }
                    A::CreateGroup => {
                        canvas.create_group_from_selection();
                    }
                    A::DeleteGroup => {
                        canvas.delete_context_group();
                    }
                    A::SetGroupColor(color) => {
                        canvas.set_context_group_color(color);
                    }
                    A::SelectGroupMembers => {
                        canvas.select_context_group_members();
                    }
                    A::CopyStyle => canvas.copy_style_action(),
                    A::PasteStyle => {
                        canvas.paste_style_action();
                    }
                    A::ToggleCollapse => {
                        canvas.toggle_collapse_action();
                    }
                    A::CollapseSelected => {
                        canvas.collapse_selected_action();
                    }
                    A::MoveSameParent => {
                        canvas.move_same_parent_action();
                    }
                    A::AddSummary => {
                        canvas.add_summary_action();
                    }
                    A::AddBoundary => {
                        canvas.add_boundary_action();
                    }
                    A::Detach => {
                        canvas.detach_primary_action();
                    }
                    A::SetRoot => {
                        canvas.set_root_primary();
                    }
                    A::AddFloating => {
                        if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                            canvas.add_floating_at(pos);
                        }
                    }
                    A::Focus => {
                        canvas.focus_selected();
                    }
                    A::FitView => canvas.fit_view(),
                    A::ResetZoom => {
                        canvas.zoom_reset_action();
                    }
                    A::ExpandAll => {
                        canvas.expand_all_action();
                    }
                    A::CollapseAll => {
                        canvas.collapse_all_action();
                    }
                    A::DeleteAnnotation => {
                        canvas.delete_annotation();
                    }
                    A::DeleteRelationship => {
                        canvas.delete_selected_relationship();
                    }
                    A::EditRelationshipLabel => {
                        canvas.edit_relationship_label_action();
                    }
                    A::SetRelationshipHead(head) => {
                        canvas.set_selected_relationship_head(head);
                    }
                    A::AlignFree(how) => {
                        canvas.align_free_topics(how);
                    }
                    A::DistributeFree(axis) => {
                        canvas.distribute_free_topics(axis);
                    }
                    A::ArrangeConceptMap => {
                        arranged = canvas.arrange_concepts();
                    }
                    A::ArrangeSelectedConcepts => {
                        arranged = canvas.arrange_selected_concepts();
                    }
                    A::ToggleConceptPinned => {
                        if canvas.selected_count() > 1 {
                            if let Some(pinned) = canvas.toggle_concept_pinned_selected() {
                                pinned_toast = Some((
                                    if pinned {
                                        "已钉住选中概念位置 · 整理时保持原位"
                                    } else {
                                        "已取消钉住选中概念位置"
                                    },
                                    false,
                                ));
                            }
                        } else if let Some(id) = canvas.primary_id() {
                            let was_pinned = canvas.is_concept_pinned(&id);
                            if canvas.toggle_concept_pinned(&id) {
                                pinned_toast = Some((
                                    if !was_pinned {
                                        "已钉住概念位置 · 整理时保持原位"
                                    } else {
                                        "已取消钉住概念位置"
                                    },
                                    false,
                                ));
                            }
                        }
                    }
                    A::SetBoundaryColor(hex) => {
                        canvas.set_boundary_color(hex);
                    }
                    A::EditSummary => {
                        canvas.edit_summary_action();
                    }
                    A::SetStyle | A::RevealFolder | A::NewCard => {}
                }
                canvas.request_canvas_focus();
                if let Some(n) = arranged {
                    self.set_toast(format!("已整理 {n} 颗概念 · Ctrl+Z 撤销"), false);
                }
                if let Some((msg, bad)) = pinned_toast {
                    self.set_toast(msg, bad);
                }
            }
        }
        ctx.request_repaint();
    }

    /// 命令面板的动作落地。**面板只发意图，改文档的活儿都在这儿。**
    pub(crate) fn run_palette(&mut self, action: panels::command::Action) {
        use panels::command::Action as A;
        match action {
            A::New => self.action_new(),
            A::Open => self.action_open(),
            A::Import => self.action_import(),
            A::Notes => self.open_library(),
            A::CaptureCard => self.capture_card_for_selected(),
            A::DailyNote => self.open_daily_note(),
            A::ToggleGrid => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.toggle_grid();
                }
            }
            A::ToggleRulers => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.toggle_rulers();
                }
            }
            A::Review => self.start_review(),
            A::Knowledge => self.open_knowledge(),
            A::Stats => self.stats_open = true,
            A::Bundle => self.export_bundle(),
            A::Recovery => self.open_recovery_center(),
            A::Settings => self.open_settings(),
            A::ThemeStudio => self.open_theme_studio(),
            A::History => self.open_history(),
            A::Save => self.action_save(),
            A::SaveAs => self.action_save_as(),
            A::Export => self.open_export(),
            A::Find => self.open_search(),
            A::ToggleOutline => self.toggle_outline_view(),
            A::ToggleInspector => self.toggle_inspector(),
            A::ToggleMinimap => self.toggle_minimap(),
            A::Preferences => self.shortcuts_dialog.open(),
            A::SetTheme(id) => self.apply_theme(id),
            other => {
                let Some(canvas) = self.canvas.as_mut() else {
                    return;
                };
                match other {
                    A::Undo => {
                        canvas.undo();
                    }
                    A::Redo => {
                        canvas.redo();
                    }
                    A::Delete => {
                        canvas.delete_selected();
                    }
                    A::FitView => canvas.fit_view(),
                    A::ZoomIn => canvas.zoom_by(
                        crate::canvas_chrome::ZOOM_STEP,
                        crate::canvas::ZoomSrc::Button,
                    ),
                    A::ZoomOut => canvas.zoom_by(
                        1.0 / crate::canvas_chrome::ZOOM_STEP,
                        crate::canvas::ZoomSrc::Button,
                    ),
                    _ => {}
                }
            }
        }
    }

    /// 标题栏下方的标签条。**只有 ≥2 个标签页时才画**：单标签视图保持与旧版逐像素一致，
    /// 免得每次加个功能 golden 基线都跟着漂。
    ///
    /// 加号固定在右端（不随横向滚动移走）；标签本体负责切换与拖拽，关闭格是独立命中区。
    pub(crate) fn tab_bar(&mut self, ui: &mut egui::Ui) {
        use eframe::egui::containers::scroll_area::{DragScroll, ScrollSource};

        let n = self.tabs.len();
        let active = self.active;
        let ch = self.chrome;
        let labels: Vec<(String, bool)> = (0..n).map(|i| self.tab_label(i)).collect();
        let mut action: Option<TabAction> = None;
        let mut open_menu: Option<(usize, egui::Pos2)> = None;
        let mut drag_started: Option<usize> = None;
        let mut drag = self.tab_drag.take();
        // 前台有浮层时标签条只画不收指针：滚轮和重排起手都读裸输入（不看图层命中），
        // 不拦就会在暗幕/菜单背后滚动、重排，甚至换掉弹窗所属的活动文档。
        let foreground = self.overlay_state().top_modal().is_some()
            || self.dialog_demo
            || egui::Popup::is_any_open(ui.ctx());
        if foreground {
            drag = None;
        }
        // 开着菜单时标签条只画不收输入：菜单外的下一次点击属于「关闭菜单」。
        let interactive = !foreground && self.tab_menu.is_none() && self.context_menu.is_none();
        let mut centers: Vec<f32> = Vec::with_capacity(n);
        let mut rects: Vec<Rect> = Vec::with_capacity(n);

        let full = ui.max_rect();
        // 切页/新建/关闭/窗口变窄后把活动标签滚回视野；滚轮不改这个键，不会被回拉。
        let seen_key = (active, n, full.width().round() as u32);
        let reveal = self.tab_seen != Some(seen_key);
        let plus_zone = 40.0;
        let scroll_rect = Rect::from_min_max(
            full.min,
            Pos2::new((full.right() - plus_zone).max(full.left()), full.bottom()),
        );
        let mut tabs_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(scroll_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let out = egui::ScrollArea::horizontal()
            .id_salt("mw-tabbar")
            // 滚轮自己做横向映射；拖拽要留给标签重排，不能让滚动区抢走。
            .scroll_source(ScrollSource {
                scroll_bar: false,
                drag: DragScroll::Never,
                mouse_wheel: false,
            })
            .show(&mut tabs_ui, |ui| {
                // 滚轮/触控板横滑 → 横向滚动：每帧只合成一次。写在标签循环里会
                // 按标签数重复累加（9 个标签滚 9 倍），命中区取滚动视口、不含加号。
                if interactive && drag.is_none() && drag_started.is_none() {
                    let over = ui
                        .ctx()
                        .pointer_interact_pos()
                        .is_some_and(|p| scroll_rect.contains(p));
                    if over {
                        // 普通鼠标报 Line（一格 ±1），触控板报 Point；按 egui 自己的
                        // 口径换算，否则真实滚轮一格只走 1px。
                        let line_speed = ui.ctx().options(|o| o.input_options.line_scroll_speed);
                        let page_h = ui.ctx().input(|i| i.viewport_rect().height());
                        let wheel = ui.input(|inp| {
                            crate::tab_strip::wheel_scroll_x(
                                inp.events.iter().filter_map(|e| match e {
                                    egui::Event::MouseWheel {
                                        delta,
                                        unit,
                                        modifiers,
                                        ..
                                    } => {
                                        let usable = !modifiers.ctrl
                                            && !modifiers.alt
                                            && !modifiers.mac_cmd
                                            && !modifiers.command;
                                        Some((*delta, *unit, usable))
                                    }
                                    _ => None,
                                }),
                                line_speed,
                                page_h,
                            )
                        });
                        if let Some(dx) = wheel {
                            crate::trace::event(&format!("tab_wheel dx={dx:.2}"));
                            ui.scroll_with_delta_animation(
                                egui::vec2(dx, 0.0),
                                egui::style::ScrollAnimation::none(),
                            );
                        }
                    }
                }
                ui.horizontal_centered(|ui| {
                    ui.add_space(mw_core::design::component::TAB_INSET);
                    ui.spacing_mut().item_spacing.x = mw_core::design::component::TAB_GAP;
                    for (i, (title, dirty)) in labels.iter().enumerate() {
                        let selected = i == active;
                        let tab = crate::ui_kit::tab(ui, ch, title, *dirty, selected);
                        crate::trace::name(ui.ctx(), tab.body.id, &format!("标签页 · {title}"));
                        crate::trace::name(
                            ui.ctx(),
                            tab.close.id,
                            &format!("关闭标签页 · {title}"),
                        );
                        centers.push(tab.body.rect.center().x);
                        rects.push(tab.body.rect);
                        // 只揭示「被裁掉」的活动标签；完整可见时不滚动，
                        // 否则每次切页都会把标签条横跳一下。
                        if reveal && selected && !ui.clip_rect().contains_rect(tab.body.rect) {
                            crate::trace::event(&format!(
                                "tab_reveal active={active} tab={:.1}..{:.1} view={:.1}..{:.1}",
                                tab.body.rect.left(),
                                tab.body.rect.right(),
                                ui.clip_rect().left(),
                                ui.clip_rect().right()
                            ));
                            ui.scroll_to_rect_animation(
                                tab.body.rect,
                                Some(egui::Align::Center),
                                egui::style::ScrollAnimation::none(),
                            );
                        }

                        // egui 的拖拽判定要 6px；X19 口径是横向超过 4px 就进重排。
                        // 不能用 is_pointer_button_down_on：指针离开标签后它就为假，
                        // 而判定恰恰发生在指针已经移出去之后。
                        let armed = interactive && drag.is_none() && drag_started.is_none();
                        if armed {
                            let press = ui.ctx().input(|inp| {
                                if !inp.pointer.any_down() {
                                    return None;
                                }
                                Some((inp.pointer.press_origin()?, inp.pointer.interact_pos()?))
                            });
                            if let Some((origin, pos)) = press {
                                if tab.body.rect.contains(origin)
                                    && crate::tab_strip::past_drag_threshold(pos.x - origin.x)
                                {
                                    drag_started = Some(i);
                                }
                            }
                        }
                        // 拖拽期间不认单击/关闭/菜单：松手那一下只负责落槽。
                        let live = interactive && drag.is_none() && drag_started.is_none();
                        if !live {
                            continue;
                        }
                        if tab.body.middle_clicked() {
                            action = Some(TabAction::Close(i));
                        }
                        if crate::ui_kit::activated(ui, &tab.body) {
                            action = Some(TabAction::Switch(i));
                        }
                        if crate::ui_kit::activated(ui, &tab.close) {
                            action = Some(TabAction::Close(i));
                        }
                        if tab.body.secondary_clicked() {
                            let anchor = ui
                                .ctx()
                                .input(|inp| inp.pointer.interact_pos())
                                .unwrap_or(tab.body.rect.center());
                            open_menu = Some((i, anchor));
                        }
                    }
                    ui.add_space(mw_core::design::component::TAB_INSET);
                });
            });

        // 横向溢出时给左右边缘渐隐：4px 滑块会和选中标签的下划线/分隔线抢边，
        // 渐隐只提示「还有内容」，不接管命中。
        let (left_fade, right_fade) = crate::tab_strip::edge_fade_strengths(
            out.content_size.x,
            out.inner_rect.width(),
            out.state.clamped_offset().x,
        );
        // 横向偏移变化记一笔：滚轮去重、活动标签揭示的验收都靠这行数字。
        let scroll_x = out.state.clamped_offset().x;
        if (scroll_x - self.tab_scroll_x).abs() > 0.5 {
            crate::trace::event(&format!(
                "tab_scroll x={scroll_x:.1} content={:.1} view={:.1}",
                out.content_size.x,
                out.inner_rect.width()
            ));
            self.tab_scroll_x = scroll_x;
        }
        if left_fade > 0.0 {
            paint_tab_fade(ui, out.inner_rect, true, left_fade, ch.panel);
        }
        if right_fade > 0.0 {
            paint_tab_fade(ui, out.inner_rect, false, right_fade, ch.panel);
        }
        self.tab_seen = Some(seen_key);

        if let Some(i) = drag_started {
            drag = Some(TabDrag { from: i });
        }
        // Esc 取消重排：标签条画在画布之前，先消费这一下，画布不会顺手清空选择。
        if drag.is_some()
            && ui
                .ctx()
                .input_mut(|inp| inp.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            drag = None;
        }
        if let Some(d) = drag.as_ref() {
            if ui.ctx().input(|inp| inp.pointer.any_down()) {
                if let Some(x) = ui
                    .ctx()
                    .input(|inp| inp.pointer.interact_pos().map(|p| p.x))
                {
                    let y = rects
                        .first()
                        .map(|r| r.center().y)
                        .unwrap_or_else(|| full.center().y);
                    let slot = crate::tab_strip::drop_slot(&centers, x);
                    let noop = slot == d.from || slot == d.from + 1;
                    if !noop {
                        let line_x = if slot == 0 {
                            rects[0].left() - 1.0
                        } else if slot >= rects.len() {
                            rects[rects.len() - 1].right() + 1.0
                        } else {
                            (rects[slot - 1].right() + rects[slot].left()) * 0.5
                        };
                        ui.painter().line_segment(
                            [Pos2::new(line_x, y - 10.0), Pos2::new(line_x, y + 10.0)],
                            Stroke::new(2.0, ch.accent),
                        );
                    }
                    // 原标签压到 60% 透明度：位置还在，但一眼能看出它正被搬走。
                    if let Some(rect) = rects.get(d.from).copied() {
                        let radius = mw_core::design::component::TAB_RADIUS as u8;
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius {
                                nw: radius,
                                ne: radius,
                                sw: 0,
                                se: 0,
                            },
                            ch.panel.gamma_multiply(0.4),
                        );
                    }
                    // 幽灵：同一张标签的 60% 拷贝跟着指针，宽度不缩水。
                    if let (Some((title, _)), Some(from_rect)) =
                        (labels.get(d.from), rects.get(d.from))
                    {
                        let ghost = Rect::from_min_size(
                            Pos2::new(x + 10.0, y - from_rect.height() * 0.5),
                            Vec2::new(from_rect.width(), from_rect.height()),
                        );
                        let radius = mw_core::design::component::TAB_RADIUS as u8;
                        let p = ui.painter();
                        p.rect_filled(
                            ghost,
                            egui::CornerRadius {
                                nw: radius,
                                ne: radius,
                                sw: 0,
                                se: 0,
                            },
                            ch.accent_soft.gamma_multiply(0.6),
                        );
                        p.text(
                            ghost.left_center() + egui::vec2(10.0, 0.0),
                            Align2::LEFT_CENTER,
                            shorten(title, 16),
                            FontId::proportional(Type::SMALL),
                            ch.text.gamma_multiply(0.7),
                        );
                    }
                }
            }
            if ui.ctx().input(|inp| inp.pointer.any_released()) {
                let slot = ui
                    .ctx()
                    .input(|inp| inp.pointer.interact_pos().map(|p| p.x))
                    .map(|x| crate::tab_strip::drop_slot(&centers, x))
                    .unwrap_or(d.from);
                if slot != d.from && slot != d.from + 1 {
                    action = Some(TabAction::Reorder(d.from, slot));
                }
                drag = None;
            }
            ui.ctx().request_repaint();
        }
        self.tab_drag = drag;

        // 加号固定在最右端：标签再多也不该把「新建」滚出视野。
        let plus_rect = Rect::from_center_size(
            Pos2::new(full.right() - plus_zone * 0.5, full.center().y),
            Vec2::splat(mw_core::design::component::TAB_H),
        );
        let mut plus_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(plus_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let plus = crate::ui_kit::plus(
            &mut plus_ui,
            ch,
            mw_core::design::component::TAB_H,
            "新建标签页（Ctrl+T）",
        );
        crate::trace::name(ui.ctx(), plus.id, "新建标签页");
        if interactive && drag.is_none() && plus.clicked() {
            action = Some(TabAction::NewTab);
        }

        if let Some((i, anchor)) = open_menu {
            self.context_menu = None;
            self.search.close();
            self.palette.close();
            self.tab_menu = self.build_tab_menu(i, anchor);
        }
        if let Some(action) = action {
            self.run_tab_action(ui.ctx(), action);
        }
    }

    /// 主题（配色）切换：命令面板与菜单共用这一条路径，与检查器宫格发的是**同一个
    /// `Command::SetTheme`**——入口再多，改文档的口子只有一个。
    /// `sync_chrome` 下一帧自己跟上（它只认主题 id 变没变）。
    /// 主题（配色）切换：命令面板与菜单共用这一条路径，与检查器宫格发的是**同一个
    /// `Command::SetTheme`**——入口再多，改文档的口子只有一个。
    /// `sync_chrome` 下一帧自己跟上（它只认主题 id 变没变）。
    pub(crate) fn apply_theme(&mut self, id: &'static str) {
        let applied = self.canvas.as_mut().is_some_and(|canvas| {
            let ok = canvas.apply_command(mw_core::Command::SetTheme {
                theme: id.to_string(),
            });
            if ok {
                // 菜单项/按钮会抢焦点，改完把键盘还给画布（与检查器一致）。
                canvas.request_canvas_focus();
            }
            ok
        });
        if !applied {
            self.set_toast("主题切换未生效：没有可改的文档", true);
        }
    }

    /// 主题换了就把壳层颜色与 egui visuals 重刷一遍。
    ///
    /// 判据是「主题 id + 生效强调色」这一对，不是整份主题：`install_visuals` 会让所有控件重新
    /// 布局，每帧调等于自己给自己造重排抖动。只比 id 也不行——自定义强调色（`SetThemeAccent`）
    /// 不换主题 id 却要换一整套 accent，面板描边、选中行、输入法下划线都从 `Chrome` 派生。
    /// 主题换了就把壳层颜色与 egui visuals 重刷一遍。
    ///
    /// 判据是「主题 id + 生效强调色」这一对，不是整份主题：`install_visuals` 会让所有控件重新
    /// 布局，每帧调等于自己给自己造重排抖动。只比 id 也不行——自定义强调色（`SetThemeAccent`）
    /// 不换主题 id 却要换一整套 accent，面板描边、选中行、输入法下划线都从 `Chrome` 派生。
    pub(crate) fn sync_chrome(&mut self, ctx: &egui::Context) {
        let theme = self
            .canvas
            .as_ref()
            .map(|c| c.session().theme.clone())
            .unwrap_or_else(|| mw_core::get_theme("snow"));
        if theme.id == self.chrome_theme && c32(theme.ui.accent) == self.chrome.accent {
            return;
        }
        self.chrome = chrome_of(&theme);
        self.tone = Tone::of(&theme);
        self.chrome_theme = theme.id.to_string();
        install_visuals(ctx, self.chrome);
    }

    /// 状态栏：保存状态 / 主题数 / 选中摘要；右侧是主题名与键盘归属提示。
    ///
    /// 缩放数字不在这里重复——右下角的缩放条已经是它的家。状态栏只做「读一眼」的
    /// 低频信息，所有文字用 CAPTION，保存状态才升到 SMALL 并配一枚色点。
    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let ch = self.chrome;
        if self.home {
            ui.horizontal_centered(|ui| {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("开始页")
                        .size(Type::CAPTION)
                        .color(ch.dim),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    let hint = if self.home_return {
                        "Esc 返回当前文档 · Ctrl+N 新建 · Ctrl+O 打开"
                    } else {
                        "Ctrl+N 新建 · Ctrl+O 打开"
                    };
                    ui.label(egui::RichText::new(hint).size(Type::CAPTION).color(ch.dim));
                });
            });
            return;
        }
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        let save = self.save_state();
        let s = canvas.session();
        let selected = canvas.selected_count();
        let primary = canvas.primary_id();
        let primary_text = primary.as_deref().and_then(|id| canvas.node_text(id));
        let summary = selection_summary(selected, primary_text);
        let (dot_color, save_color) = match save {
            SaveState::Dirty | SaveState::Failed => (ch.warn, ch.warn),
            SaveState::Saving => (ch.accent, ch.dim),
            SaveState::Untitled => (ch.dim, ch.dim),
            _ => (ch.faint, ch.dim),
        };

        let hairline = |ui: &mut egui::Ui| {
            ui.add_space(4.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, 12.0), Sense::hover());
            ui.painter().vline(
                r.center().x,
                r.y_range(),
                Stroke::new(mw_core::design::stroke::HAIRLINE, ch.border),
            );
            ui.add_space(4.0);
        };

        ui.horizontal_centered(|ui| {
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 2.5, dot_color);
            ui.label(
                egui::RichText::new(save.label())
                    .size(Type::SMALL)
                    .family(eframe::egui::FontFamily::Name(
                        crate::fonts::SEMIBOLD.into(),
                    ))
                    .color(save_color),
            );
            hairline(ui);
            ui.label(
                egui::RichText::new(format!("{} 个主题", s.doc.node_count()))
                    .size(Type::CAPTION)
                    .color(ch.dim),
            );
            if let Some(summary) = summary {
                hairline(ui);
                ui.label(
                    egui::RichText::new(summary)
                        .size(Type::CAPTION)
                        .color(ch.dim),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                if self.outline_view {
                    ui.label(
                        egui::RichText::new("大纲视图 · F8 返回画布")
                            .size(Type::CAPTION)
                            .color(ch.dim),
                    );
                    return;
                }
                // 主题名带一枚强调色小点，和主题菜单里的色标同一语义。
                let theme = s.theme.name;
                let font = FontId::proportional(Type::CAPTION);
                let galley = ui
                    .painter()
                    .layout_no_wrap(theme.to_string(), font.clone(), ch.dim);
                let (r, _) =
                    ui.allocate_exact_size(Vec2::new(galley.size().x + 12.0, 16.0), Sense::hover());
                ui.painter()
                    .circle_filled(Pos2::new(r.left() + 3.0, r.center().y), 2.5, ch.accent);
                ui.painter().text(
                    Pos2::new(r.left() + 9.0, r.center().y),
                    Align2::LEFT_CENTER,
                    theme,
                    font,
                    ch.dim,
                );
                if canvas.keys_in_panels() {
                    ui.add_space(10.0);
                    // 键盘交给面板之后，Tab 不再是「加子主题」。不说清楚它在哪儿，
                    // 用户只会觉得按键突然失灵。
                    ui.label(
                        egui::RichText::new("键盘：面板 · F6 收回")
                            .size(Type::CAPTION)
                            .color(ch.accent),
                    );
                }
            });
        });
    }

    /// 面板开合进度 0..1。尊重系统「减少动画」：直接切换，不做过渡。
    /// 截图 harness 也取终态——否则基线取决于 warmup 恰好跑了几帧。
    pub(crate) fn panel_motion(&self, ctx: &egui::Context, id: &str, open: bool) -> f32 {
        if self.reduce_motion || self.harness.is_some() {
            return if open { 1.0 } else { 0.0 };
        }
        // 时长与曲线都来自设计令牌：悬停、面板、抽屉共用同一拍子。
        ctx.animate_bool_with_time_and_easing(
            egui::Id::unique(id),
            open,
            mw_core::design::motion::BASE,
            mw_core::design::motion::ease_out_cubic,
        )
    }

    /// 全屏大纲：页头是控制台，页脚是状态，中间只留一列正文。
    pub(crate) fn outline_screen(
        &mut self,
        ui: &mut egui::Ui,
        blocked: bool,
    ) -> Option<panels::outline::Action> {
        if let Some(canvas) = self.canvas.as_ref() {
            let s = canvas.session();
            self.outline.sync(&s.doc, &s.theme);
            // 筛选的可见表要读标签等文档字段（docs/28 §5.7），放在这里有 doc 可用。
            self.outline.prepare_filter(&s.doc);
        }
        let (theme, selected, primary, root_id, title, total, concept, question) = {
            let canvas = self.canvas.as_ref()?;
            let s = canvas.session();
            let title = s
                .doc
                .node(&s.doc.root_id)
                .map(|n| n.text.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "未命名导图".into());
            let concept = s.doc.is_concept_map();
            // ⑥：概念图的「聚焦问题」也进大纲页——它就是这份大纲在回答的那句话。
            let question = concept
                .then(|| {
                    s.doc
                        .meta
                        .layout
                        .focus_question
                        .as_deref()
                        .map(str::trim)
                        .filter(|q| !q.is_empty())
                        .map(str::to_string)
                })
                .flatten();
            (
                s.theme.clone(),
                canvas.selected_ids(),
                canvas.primary_id(),
                s.doc.root_id.clone(),
                title,
                s.doc.node_count(),
                concept,
                question,
            )
        };
        let ch = self.chrome;
        let stats = self.outline.stats();
        let shown = self.outline.len();
        let crumbs = self.outline.crumbs().to_vec();
        let focus_active = self.outline.focus_id().is_some();
        let search_open = self.outline.search_open();
        let density = self.outline.density();
        let hits = self.outline.search_hits();
        let dirty = self.canvas.as_ref().is_some_and(|c| c.session().dirty());
        let can_focus = primary.as_deref().is_some_and(|id| id != root_id.as_str());
        let mut action = None;

        // 中央面板是 no_frame 的：画布平时自己铺底，大纲就得自己铺。
        // 漏掉这一笔，暗色背景会从窗口透上来，白字直接糊成一片（第一版实拍即如此）。
        ui.painter().rect_filled(ui.max_rect(), 0.0, ch.panel);
        let full = ui.max_rect();
        let width = full.width();

        // ---- 页头 40px：返回 / 面包屑 / 计数 / 搜索 / 更多（docs/28 §5.1）----
        let (_, header) = ui.allocate_space(egui::vec2(width, 40.0));
        draw_outline_header(
            ui,
            ch,
            header,
            &OutlineHeader {
                doc_title: &title,
                total,
                selected: selected.len(),
                hits,
                focus_active,
                can_focus,
                primary: primary.as_deref(),
                search_open,
                density,
                crumbs: &crumbs,
                concept,
            },
            &mut action,
        );
        ui.painter().hline(
            header.x_range(),
            header.bottom(),
            egui::Stroke::new(1.0, ch.sep),
        );
        // 视觉验收：`MW_OUTLINE_MENU=collapse/density/more` 让弹层常开。
        if let Ok(menu) = std::env::var("MW_OUTLINE_MENU") {
            egui::Popup::open_id(ui.ctx(), outline_menu_id(&menu));
        }

        // ---- ⑥ 聚焦问题：它不属于任何一行，属于整张图 ----
        // 画布那条贴视口顶；大纲页没有视口，就贴页头下沿——同样是「读的时候一直在」。
        // 只在「概念图 + 真的写了问题」时出现，所以既有四张 `cm-outline*` 基线零漂移。
        // 编辑态要在这一块里吃掉 Enter / Esc，正文行不许再抢（`docs/32` §8）。
        let keys_blocked = blocked || self.outline_question_edit.is_some();
        if let Some(q) = question.as_deref() {
            let font = egui::FontId::proportional(mw_core::design::content::READING_BODY.size);
            let margin = panels::outline::column_margin(width);
            // 按列宽换行，不是截断：问题可以被问得很长，长到两行也比赛过省略号强。
            let galley = ui.painter().layout(
                q.to_owned(),
                font,
                egui::Color32::PLACEHOLDER,
                (width - 2.0 * margin).max(120.0),
            );
            let band_h = galley.size().y + 2.0 * crate::content_kit::QUESTION_BAND_PAD;
            let (_, page) = ui.allocate_space(egui::vec2(width, band_h));
            let col = crate::content_kit::question_band(page, margin, galley.size().y);
            if let Some(mut ed) = self.outline_question_edit.take() {
                // 就地改问题：与画布那条带子**同一个控件、同一套键规则**
                // （Enter 提交、Esc 取消、点走也提交、IME 组合期间 Enter 是选字）。
                // 带宽仍按「已提交的那句」算，所以立起编辑框时机身不跳。
                ed.rect = col;
                if let Some(v) = ui.input(|i| crate::canvas::ime_transition(&i.events)) {
                    ed.ime = v;
                }
                let esc = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                let enter = !ed.ime
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                // 底色透明：这一页的背景由页面自己铺，这里填死一个 token 迟早和它对不上。
                let response = crate::content_kit::question_field(
                    ui,
                    col,
                    &mut ed.text,
                    crate::chrome::c32(theme.ui.text),
                    egui::Color32::TRANSPARENT,
                    egui::Id::unique("mw-outline-focus-edit"),
                );
                if ed.focus {
                    response.request_focus();
                    ed.focus = false;
                }
                let done = enter || response.lost_focus();
                match crate::content_kit::question_edit_outcome(&ed.text, &ed.opening, esc, done) {
                    // 还在打字：编辑框继续立着。
                    crate::content_kit::QuestionEditOutcome::Continue => {
                        self.outline_question_edit = Some(ed);
                    }
                    // Esc 与「一个字没改就点走」都不发命令：撤销栈里不留空记录。
                    crate::content_kit::QuestionEditOutcome::Cancel => {}
                    crate::content_kit::QuestionEditOutcome::Commit(text) => {
                        crate::trace::event("outline-focus-question");
                        if let Some(c) = self.canvas.as_mut() {
                            c.apply_command(mw_core::Command::SetFocusQuestion {
                                text: (!text.is_empty()).then_some(text),
                            });
                        }
                    }
                }
            } else {
                // 与画布那条同一套排版：居中、`READING_BODY` 字号、`ui.dim` 墨色。
                let pos = col.center() - galley.size() * 0.5;
                // 单击不给编辑：画布那边是双击，两处只留一条规矩。悬停给底色 + 文本光标 +
                // 一条强调线，是在说「这行字能点」。
                let hit = ui.interact(
                    col,
                    ui.scope_id().with("mw-outline-focus-band"),
                    egui::Sense::click(),
                );
                let hot = hit.hovered();
                if hot {
                    ui.painter().rect_filled(
                        col,
                        mw_core::design::radius::MD,
                        crate::chrome::c32(theme.ui.hover),
                    );
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                }
                ui.painter().galley(
                    pos,
                    galley,
                    panels::outline::c32a(if hot { theme.ui.text } else { theme.ui.dim }),
                );
                if hot {
                    ui.painter().hline(
                        col.x_range().shrink(12.0),
                        col.bottom() - 4.0,
                        egui::Stroke::new(1.0, crate::chrome::c32(theme.ui.accent)),
                    );
                }
                if hit.double_clicked() {
                    self.outline_question_edit = Some(crate::canvas::FocusEdit {
                        text: q.to_string(),
                        opening: q.to_string(),
                        focus: true,
                        rect: col,
                        ime: false,
                    });
                }
            }
            // 不画分隔线：带子与上下邻居之间还各有一个 8px 项间距（egui 的 item_spacing），
            // 再加一条 1px 线就成了「双线」，反而像第二个页头。
        }

        // ---- 正文 ----
        if let Some(a) = self.outline.show(
            ui,
            &theme,
            ch,
            &selected,
            primary.as_deref(),
            None,
            keys_blocked,
            &mut self.focus_mem,
        ) {
            // 页头这一帧已经点了动作时，不再让面板的按键覆盖它。
            if action.is_none() {
                action = Some(a);
            }
        }
        // 滚动条在 ScrollArea 收尾时才建，焦点请求要排在它后面才不会被抢走。
        // 但问题带的编辑框立着时键盘归它：这一帧的行焦点请求必须让开，否则「首次进大纲
        // 把焦点给第一行」会把刚立起来的编辑框挤掉——第 24 轮实测到的就是这个。
        if let Some(id) = self.outline.take_focus_request() {
            if keys_blocked {
                crate::trace::event(&format!("outline_focus_request_skipped {id}"));
            } else {
                ui.memory_mut(|m| m.request_focus(panels::outline::row_id(&id)));
                crate::trace::event(&format!("outline_focus_request {id}"));
            }
        }

        // ---- 页脚 24px：左计数与选中摘要、右帮助入口（docs/28 §5.1；无按键表）----
        let footer =
            egui::Rect::from_min_max(egui::pos2(full.left(), full.bottom() - 24.0), full.max);
        let (stats_text, short_stats) = if focus_active {
            (
                format!("聚焦 {shown} / 全图 {total}"),
                format!("聚焦 {shown}/{total}"),
            )
        } else if search_open {
            (
                format!("{hits} / {total} 命中"),
                format!("{hits}/{total} 命中"),
            )
        } else {
            // CM-07：概念图里数的是「概念」，不是「主题」——措辞跟着文档类型走。
            let (unit, short_unit) = if concept {
                ("个概念", "概念")
            } else {
                ("个主题", "主题")
            };
            let full_stats = if selected.is_empty() {
                format!("{total} {unit} · {} 个已完成", stats.done)
            } else {
                format!(
                    "{total} {unit} · {} 个已完成 · 选中 {}",
                    stats.done,
                    selected.len()
                )
            };
            (
                full_stats,
                format!("{total} {short_unit} · {} 完成", stats.done),
            )
        };
        draw_outline_footer(
            ui,
            ch,
            footer,
            &stats_text,
            &short_stats,
            dirty,
            &mut action,
        );

        // ---- 文档信息：前景弹层，Esc 由壳层统一路由 ----
        // 只有它是栈顶时才画：低层模态保留状态，但不能在被压住时露出第二层卡片。
        if self.outline.info_open()
            && self.overlay_state().top_modal() == Some(crate::overlay::Modal::OutlineInfo)
        {
            let muted = ch.dim;
            let actions = [dialog::DialogAction::new(
                "关闭",
                ui_kit::ButtonTone::Secondary,
            )];
            let spec = dialog::DialogSpec::new("文档信息")
                .width(420.0)
                .default_action(0);
            let out = dialog::show(ui.ctx(), ch, "outline-info", spec, &actions, |ui| {
                for (label, value) in [
                    ("主题", stats.total),
                    ("已完成", stats.done),
                    ("备注", stats.notes),
                    ("卡片", stats.cards),
                    ("标签", stats.labels),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(label).size(Type::BODY).color(muted));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(value.to_string()).size(20.0).family(
                                eframe::egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()),
                            ));
                        });
                    });
                    ui.add_space(6.0);
                }
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("统计口径：整份文档，不受大纲聚焦或筛选影响。")
                        .size(Type::CAPTION)
                        .color(muted),
                );
            });
            if out.closed {
                self.outline.close_info();
            }
        }
        action
    }

    /// F8 / 菜单 / 命令面板共用的视图切换。
    pub(crate) fn toggle_outline_view(&mut self) {
        self.outline_view = !self.outline_view;
        if self.outline_view {
            // 没有选中时把根主题选中：大纲一进来就该有一行「当前行」，
            // 否则方向键和 Enter 都不知道从哪开始。
            let target = self.canvas.as_mut().map(|canvas| {
                let id = canvas
                    .primary_id()
                    .unwrap_or_else(|| canvas.session().doc.root_id.clone());
                if canvas.primary_id().is_none() {
                    canvas.select_node(&id);
                }
                canvas.commit_editing();
                id
            });
            if let Some(id) = target {
                self.outline.reveal(&id);
                // 进大纲就把焦点环放在当前行上：不用先按一下方向键才知道键盘归谁。
                self.outline.request_row_focus(id);
            }
        } else if let Some(canvas) = self.canvas.as_mut() {
            canvas.request_canvas_focus();
        }
        if !self.outline_view {
            self.outline.close_info();
        }
    }

    /// 大纲动作 → 画布命令。与面板里的“只出海动作、不改文档”保持同一条纪律。
    pub(crate) fn apply_outline_action(&mut self, action: panels::outline::Action) {
        use panels::outline::Action as OA;
        match action {
            OA::Select(id) => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.select_node(&id);
                }
                // 点击的是已经画在眼前的一行：只选中，不再居中滚动。
                // 键盘走行/搜索跳转由面板自己写 `scroll_to`，这里再 reveal 会把
                // 刚点中的行重新居中，连点两下甚至会落到相邻行上。
            }
            OA::ToggleSelect(id) => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.toggle_select_node(&id);
                }
            }
            OA::SetSelection { ids, primary } => {
                crate::trace::event(&format!(
                    "outline_set_selection n={} primary={primary}",
                    ids.len()
                ));
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.select_many(&ids, &primary);
                }
            }
            OA::Toggle(id) => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.toggle_collapsed(&id);
                }
                // CER-11：单节点折叠/展开走行表增量；20k 下不重建全表。
                if let Some(canvas) = self.canvas.as_ref() {
                    let s = canvas.session();
                    let collapsed = s.doc.node(&id).is_some_and(|n| n.collapsed);
                    if self
                        .outline
                        .apply_collapse(&s.doc, &s.theme, &id, collapsed)
                    {
                        crate::trace::event("outline_collapse_incremental");
                    }
                }
            }
            OA::Rename { id, text } => {
                crate::trace::event(&format!("outline_rename id={id} text={text:?}"));
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.apply_command(mw_core::Command::SetText {
                        id: id.clone(),
                        text,
                    });
                }
                // Tab 建的那一行：名字提交后接着立起短语编辑框（§5.6.1 第二步）。
                // 名字留空时面板会自己删行，那时 `id` 已经不在，短语框自然不该出现。
                if self.via_after_rename.as_deref() == Some(id.as_str()) {
                    self.via_after_rename = None;
                    if self.outline.has_row(&id) {
                        self.outline.begin_via_edit(&id, true);
                    }
                }
            }
            OA::NewConcept(after) => {
                // CM-07：概念图的大纲里 Enter 只加「点」，不加「边」——
                // 加边必须有链接短语（docs/30 §5.9），那要画布上的短语编辑态。
                let new_id = self
                    .canvas
                    .as_mut()
                    .and_then(|canvas| canvas.add_concept_below(&after));
                crate::trace::event(&format!("outline_new_concept after={after} new={new_id:?}"));
                if let Some(new_id) = new_id {
                    if let Some(canvas) = self.canvas.as_mut() {
                        canvas.select_node(&new_id);
                    }
                    self.outline.begin_edit(&new_id);
                }
            }
            OA::LinkConcept(from) => {
                // docs/30 §5.6.1：Tab = 一个 Batch 里「加概念 + 加边」，随后先给概念
                // 起名、再给边填短语。短语留空就整条撤掉，概念留着（§5.9）。
                let new_id = self
                    .canvas
                    .as_mut()
                    .and_then(|canvas| canvas.link_concept_below(&from));
                crate::trace::event(&format!(
                    "outline_concept_link_done from={from} new={new_id:?}"
                ));
                if let Some(new_id) = new_id {
                    if let Some(canvas) = self.canvas.as_mut() {
                        canvas.select_node(&new_id);
                    }
                    self.outline.begin_edit(&new_id);
                    self.via_after_rename = Some(new_id);
                }
            }
            OA::UnlinkConcept { from, rel } => {
                let ok = self
                    .canvas
                    .as_mut()
                    .is_some_and(|canvas| canvas.unlink_concept(&from, &rel));
                crate::trace::event(&format!(
                    "outline_concept_unlink_done from={from} rel={rel} ok={ok}"
                ));
                // 断边后这行成了孤立概念：选择留在同一个概念上，别把焦点甩丢。
                if ok {
                    if let Some(canvas) = self.canvas.as_mut() {
                        canvas.select_node(&from);
                    }
                }
            }
            OA::SetVia { from, rel, label } => {
                let text = label.clone().unwrap_or_default();
                // 概念图里没有短语的边不构成命题（`docs/30` §5）：既有命题被清空时不发命令，
                // 保留原短语并说明。思维导图的空串仍表示清空（模型只禁止概念图）。
                let concept_map = self
                    .canvas
                    .as_ref()
                    .is_some_and(|canvas| canvas.is_concept_map());
                if concept_map && text.trim().is_empty() {
                    crate::trace::event(&format!(
                        "outline_concept_via_reject from={from} rel={rel} reason=empty"
                    ));
                    self.set_toast("概念图的连接短语不能为空", false);
                } else {
                    let ok = self
                        .canvas
                        .as_mut()
                        .is_some_and(|canvas| canvas.set_concept_phrase(&from, &rel, &text));
                    crate::trace::event(&format!(
                        "outline_concept_via_commit from={from} rel={rel} label={label:?} ok={ok}"
                    ));
                }
            }
            OA::EditVia(id) => {
                self.outline.begin_via_edit(&id, false);
            }
            OA::CommitAndAdd {
                id,
                text,
                changed,
                child,
            } => {
                let new_id = self.canvas.as_mut().and_then(|canvas| {
                    if changed {
                        canvas.apply_command(mw_core::Command::SetText {
                            id: id.clone(),
                            text: text.clone(),
                        });
                    }
                    if child {
                        canvas.add_child_to(&id)
                    } else {
                        canvas.add_sibling_to(&id)
                    }
                });
                crate::trace::event(&format!(
                    "outline_commit_add anchor={id} child={child} changed={changed} new={new_id:?}"
                ));
                if let Some(new_id) = new_id {
                    self.outline.begin_edit(&new_id);
                }
            }
            OA::SplitSibling { id, head, tail } => {
                let new_id = self
                    .canvas
                    .as_mut()
                    .and_then(|canvas| canvas.split_sibling_text(&id, &head, &tail));
                crate::trace::event(&format!(
                    "outline_split_sibling id={id} new={new_id:?} head={head:?} tail={tail:?}"
                ));
                if let Some(new_id) = new_id {
                    self.outline.begin_edit_at(&new_id, Some(0), false);
                }
            }
            OA::PasteLines { id, text, siblings } => {
                // docs/28 §5.4：本行 SetText + 余行 AddNode，一条 Batch 一次撤销。
                let plan = self.canvas.as_ref().and_then(|canvas| {
                    let doc = &canvas.session().doc;
                    let parent = doc
                        .node(&id)
                        .and_then(|n| n.parent_id.clone())
                        .unwrap_or_else(|| doc.root_id.clone());
                    let base = doc
                        .node(&parent)
                        .and_then(|p| p.children.iter().position(|c| c == &id))
                        .map(|i| i + 1)?;
                    let mut commands = Vec::new();
                    let current = doc.node(&id).map(|n| n.text.clone()).unwrap_or_default();
                    if text != current {
                        commands.push(mw_core::Command::SetText {
                            id: id.clone(),
                            text: text.clone(),
                        });
                    }
                    for (i, line) in siblings.iter().enumerate() {
                        commands.push(mw_core::Command::AddNode {
                            parent: parent.clone(),
                            index: Some(base + i),
                            text: line.clone(),
                            style: None,
                        });
                    }
                    (!commands.is_empty()).then_some(commands)
                });
                if let (Some(canvas), Some(commands)) = (self.canvas.as_mut(), plan) {
                    let _ = canvas.apply_command(mw_core::Command::Batch { commands });
                }
                crate::trace::event(&format!(
                    "outline_paste_lines id={id} siblings={}",
                    siblings.len()
                ));
                self.outline.request_row_focus(&id);
            }
            OA::SplitChild { id, head, tail } => {
                let new_id = self
                    .canvas
                    .as_mut()
                    .and_then(|canvas| canvas.split_child_text(&id, &head, &tail));
                crate::trace::event(&format!(
                    "outline_split_child id={id} new={new_id:?} head={head:?} tail={tail:?}"
                ));
                if let Some(new_id) = new_id {
                    self.outline.begin_edit_at(&new_id, Some(0), false);
                }
            }
            OA::MergePrev { id, text } => {
                let merged = self
                    .canvas
                    .as_mut()
                    .and_then(|canvas| canvas.merge_into_previous(&id, &text));
                crate::trace::event(&format!("outline_merge_prev id={id} merged={merged:?}"));
                if let Some((prev, caret)) = merged {
                    self.outline.reveal(&prev);
                    // 不请求行焦点：下一帧立起来的是 TextEdit，行焦点会把光标抢走。
                    self.outline.begin_edit_at(&prev, Some(caret), false);
                }
            }
            OA::DeleteEmpty { id, prev } => {
                crate::trace::event(&format!("outline_delete_empty id={id} prev={prev:?}"));
                self.delete_outline_row(&id, prev.as_deref());
            }
            OA::Move {
                id,
                new_parent,
                index,
            } => {
                crate::trace::event(&format!(
                    "outline_move_drag id={id} parent={new_parent} index={index:?}"
                ));
                if let Some(canvas) = self.canvas.as_mut() {
                    // docs/28 §5.6：应用前先探测目标父级折叠态（move 会消费 new_parent）。
                    let parent_collapsed = canvas
                        .session()
                        .doc
                        .node(&new_parent)
                        .is_some_and(|n| n.collapsed);
                    let parent_for_toggle = new_parent.clone();
                    canvas.apply_command(mw_core::Command::MoveNode {
                        id,
                        new_parent,
                        index,
                    });
                    if parent_collapsed {
                        crate::trace::event(&format!(
                            "outline_expand_parent id={parent_for_toggle}"
                        ));
                        canvas.toggle_collapsed(&parent_for_toggle);
                    }
                }
            }
            OA::MoveMany {
                ids,
                new_parent,
                index,
            } => {
                crate::trace::event(&format!(
                    "outline_move_many_drag n={} parent={new_parent}",
                    ids.len()
                ));
                if let Some(canvas) = self.canvas.as_mut() {
                    if canvas.move_many(&ids, &new_parent, index) {
                        // docs/28 §5.6：跨父级移动后展开目标父级。
                        if canvas
                            .session()
                            .doc
                            .node(&new_parent)
                            .is_some_and(|n| n.collapsed)
                        {
                            crate::trace::event(&format!("outline_expand_parent id={new_parent}"));
                            canvas.toggle_collapsed(&new_parent);
                        }
                        if let Some(primary) = canvas.primary_id() {
                            self.outline.request_row_focus(primary);
                        }
                    }
                }
            }
            OA::AddSibling(id) => {
                let new_id = self
                    .canvas
                    .as_mut()
                    .and_then(|canvas| canvas.add_sibling_to(&id));
                crate::trace::event(&format!("outline_add_sibling anchor={id} new={new_id:?}"));
                if let Some(new_id) = new_id {
                    self.outline.begin_edit(&new_id);
                }
            }
            OA::AddChild(id) => {
                let new_id = self
                    .canvas
                    .as_mut()
                    .and_then(|canvas| canvas.add_child_to(&id));
                crate::trace::event(&format!("outline_add_child anchor={id} new={new_id:?}"));
                if let Some(new_id) = new_id {
                    self.outline.begin_edit(&new_id);
                }
            }
            OA::Indent(id) => {
                crate::trace::event(&format!("outline_indent id={id}"));
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.indent_node(&id);
                }
                // 结构变了（父链/深度），行还是那一行：焦点必须跟着它。
                self.outline.request_row_focus(id);
            }
            OA::Outdent(id) => {
                crate::trace::event(&format!("outline_outdent id={id}"));
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.outdent_node(&id);
                }
                // Shift+Tab 的焦点巡游会把焦点带走；动作落地后再钉一次。
                self.outline.request_row_focus(id);
            }
            OA::IndentMany(ids) => {
                crate::trace::event(&format!("outline_indent_many n={}", ids.len()));
                if let Some(canvas) = self.canvas.as_mut() {
                    if canvas.indent_many(&ids) {
                        if let Some(primary) = canvas.primary_id() {
                            self.outline.request_row_focus(primary);
                        }
                    }
                }
            }
            OA::OutdentMany(ids) => {
                crate::trace::event(&format!("outline_outdent_many n={}", ids.len()));
                if let Some(canvas) = self.canvas.as_mut() {
                    if canvas.outdent_many(&ids) {
                        if let Some(primary) = canvas.primary_id() {
                            self.outline.request_row_focus(primary);
                        }
                    }
                }
            }
            OA::MoveStepMany { ids, delta } => {
                crate::trace::event(&format!("outline_move_many n={} delta={delta}", ids.len()));
                if let Some(canvas) = self.canvas.as_mut() {
                    if canvas.move_step_many(&ids, delta) {
                        if let Some(primary) = canvas.primary_id() {
                            self.outline.request_row_focus(primary);
                        }
                    }
                }
            }
            OA::DeleteMany(ids) => {
                crate::trace::event(&format!("outline_delete_many n={}", ids.len()));
                let next = self.outline.next_visible_after_set(&ids);
                if let Some(canvas) = self.canvas.as_mut() {
                    if canvas.delete_many(&ids) {
                        let target = next
                            .filter(|id| canvas.session().doc.node(id).is_some())
                            .or_else(|| canvas.primary_id());
                        if let Some(target) = target {
                            canvas.select_node(&target);
                            self.outline.request_row_focus(target);
                        }
                    }
                }
            }
            OA::SetTaskMany { ids, task } => {
                crate::trace::event(&format!("outline_task_many n={} task={task:?}", ids.len()));
                if let Some(canvas) = self.canvas.as_mut() {
                    if canvas.set_task_many(&ids, task) {
                        if let Some(primary) = canvas.primary_id() {
                            self.outline.request_row_focus(primary);
                        }
                    }
                }
            }
            OA::Duplicate => {
                crate::trace::event("outline_duplicate");
                if let Some(canvas) = self.canvas.as_mut() {
                    if canvas.duplicate_selected() {
                        if let Some(primary) = canvas.primary_id() {
                            self.outline.request_row_focus(primary);
                        }
                    }
                }
            }
            OA::Delete(id) => {
                crate::trace::event(&format!("outline_delete id={id}"));
                self.delete_outline_row(&id, None);
            }
            OA::MoveStep { id, delta } => {
                crate::trace::event(&format!("outline_move_step id={id} delta={delta}"));
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.move_node_step(&id, delta);
                }
            }
            OA::SetTask { id, task } => {
                crate::trace::event(&format!("outline_set_task id={id} task={task:?}"));
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.apply_command(mw_core::Command::SetTask { id, task });
                }
            }
            OA::OpenNote(id) => {
                let has_note = self
                    .canvas
                    .as_ref()
                    .and_then(|c| c.session().doc.node(&id))
                    .is_some_and(|n| n.note.as_deref().is_some_and(|s| !s.trim().is_empty()));
                crate::trace::event(&format!("outline_open_note node={id} has_note={has_note}"));
                if !has_note {
                    self.set_toast("这个主题还没有备注", true);
                } else {
                    if let Some(canvas) = self.canvas.as_mut() {
                        canvas.select_node(&id);
                    }
                    // 大纲整块占着中央面板，检查器不在场上；退回画布再聚焦备注框。
                    self.outline_view = false;
                    self.show_inspector = true;
                    self.inspector.focus_note();
                }
            }
            OA::OpenCard(id) => {
                let card = self
                    .canvas
                    .as_ref()
                    .and_then(|c| c.session().doc.card_for_node(&id).cloned());
                crate::trace::event(&format!(
                    "outline_open_card node={id} card={}",
                    card.as_ref().map(|c| c.id.as_str()).unwrap_or("-")
                ));
                match card {
                    Some(card) => {
                        let reader = self.card_reader_for(&card);
                        self.reader_from_library = false;
                        self.note_reader = Some(reader);
                    }
                    None => self.set_toast("这个主题还没有卡片笔记", true),
                }
            }
            OA::Focus(id) => {
                crate::trace::event(&format!("outline_focus id={id}"));
                if let Some(canvas) = self.canvas.as_mut() {
                    // 折叠着聚焦会只剩一行：先展开目标，再选它。
                    if canvas.session().doc.node(&id).is_some_and(|n| n.collapsed) {
                        canvas.toggle_collapsed(&id);
                    }
                    canvas.select_node(&id);
                }
                self.outline.focus_on(&id);
                self.outline.request_row_focus(&id);
            }
            OA::Unfocus => {
                let old = self.outline.unfocus();
                crate::trace::event(&format!("outline_unfocus old={old:?}"));
                if let Some(id) = old {
                    if let Some(canvas) = self.canvas.as_mut() {
                        canvas.select_node(&id);
                    }
                    self.outline.reveal(&id);
                    self.outline.request_row_focus(id);
                }
            }
            OA::Close => {
                crate::trace::event("outline_close");
                // 离开大纲就退出聚焦：下次进来先看到全图，而不是上次的局部。
                self.outline.unfocus();
                self.outline_view = false;
                self.outline_row_menu = None;
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.request_canvas_focus();
                }
            }
            OA::ToggleSearch => {
                crate::trace::event("outline_toggle_search");
                self.outline.toggle_search();
            }
            OA::SetCollapsedAll { collapsed } => {
                crate::trace::event(&format!("outline_collapse_all collapsed={collapsed}"));
                let focus = self.outline.focus_id().map(str::to_string);
                let cmd = if collapsed && focus.is_some() {
                    // 聚焦根和文档根一样永远展开：用「只显示 1 级」实现折叠全部，
                    // 既不会把视野折成一行，也只占一次撤销。
                    self.canvas.as_ref().and_then(|c| {
                        panels::outline::collapse_level_command(
                            &c.session().doc,
                            focus.as_deref(),
                            1,
                        )
                    })
                } else {
                    Some(mw_core::Command::SetCollapsedAll { collapsed })
                };
                if let (Some(canvas), Some(cmd)) = (self.canvas.as_mut(), cmd) {
                    canvas.apply_command(cmd);
                }
                self.outline_keep_selection_visible();
            }
            OA::SetCollapsedLevel(level) => {
                crate::trace::event(&format!("outline_collapse_level level={level}"));
                let focus = self.outline.focus_id().map(str::to_string);
                let cmd = self.canvas.as_ref().and_then(|c| {
                    panels::outline::collapse_level_command(
                        &c.session().doc,
                        focus.as_deref(),
                        level,
                    )
                });
                if let (Some(canvas), Some(cmd)) = (self.canvas.as_mut(), cmd) {
                    canvas.apply_command(cmd);
                }
                self.outline_keep_selection_visible();
            }
            OA::SetDensity(density) => {
                crate::trace::event(&format!("outline_density {:?}", density));
                self.outline.set_density(density);
                self.window_state.set_outline_density(density.as_u8());
            }
            OA::OpenHelp => {
                crate::trace::event("outline_help");
                self.help.toggle_outline();
            }
            OA::ShowInfo => {
                crate::trace::event("outline_info");
                self.outline.toggle_info();
            }
            OA::RowMenu { id, pos } => {
                // 行菜单是「上下文」层：先让别的菜单退场，再按这一行建新菜单。
                self.context_menu = None;
                self.tab_menu = None;
                let selected = self
                    .canvas
                    .as_ref()
                    .map(|c| c.selected_ids())
                    .unwrap_or_default();
                let multi = panels::outline::row_menu_multi(&selected, &id);
                if let Some(canvas) = self.canvas.as_mut() {
                    if multi {
                        // 右键多选里的成员：整组保留，主选钉到这一行。
                        canvas.select_many(&selected, &id);
                    } else {
                        canvas.select_node(&id);
                    }
                }
                self.outline_row_menu = Some(self.build_outline_row_menu(&id, pos, multi));
            }
        }
    }

    /// 大纲里删一行，顺手把焦点安置好。
    ///
    /// 删掉聚焦根等于整段视野消失：先记下父级，删完退回上一级。其余情况优先落到
    /// `focus_pref`（空行退格时的上一可见行），没有再跟随画布已经选中的邻居。
    fn delete_outline_row(&mut self, id: &str, focus_pref: Option<&str>) {
        let is_focus_root = self.outline.focus_id() == Some(id);
        let focus_parent = if is_focus_root {
            self.canvas
                .as_ref()
                .and_then(|c| c.session().doc.node(id))
                .and_then(|n| n.parent_id.clone())
        } else {
            None
        };
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.delete_node(id);
            if is_focus_root {
                self.outline.unfocus();
                if let Some(parent) = focus_parent {
                    canvas.select_node(&parent);
                    self.outline.request_row_focus(parent);
                } else if let Some(next) = canvas.primary_id() {
                    self.outline.request_row_focus(next);
                }
            } else if let Some(prev) = focus_pref {
                canvas.select_node(prev);
                self.outline.reveal(prev);
                self.outline.request_row_focus(prev);
            } else if let Some(next) = canvas.primary_id() {
                // 原来那一行已经没了：焦点跟着选中落到相邻行，键盘才能接着走。
                self.outline.request_row_focus(next);
            }
        }
    }

    /// 折叠之后当前行可能被折进某个祖先里。键盘焦点和选中都挪到**最高一层被折叠的
    /// 祖先**上——它一定还可见，而且是用户刚刚折叠的那一块，上下文不会丢。
    fn outline_keep_selection_visible(&mut self) {
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        let s = canvas.session();
        let Some(primary) = canvas.primary_id() else {
            return;
        };
        let base = self
            .outline
            .focus_id()
            .unwrap_or(s.doc.root_id.as_str())
            .to_string();
        let Some(anchor) = visible_ancestor(&s.doc, &base, &primary) else {
            return;
        };
        if anchor == primary {
            return;
        }
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.select_node(&anchor);
        }
        self.outline.reveal(&anchor);
        self.outline.request_row_focus(anchor);
    }

    /// 右侧检查器。与大纲同一套：闭包里只收动作，出了闭包再借 `canvas`——
    /// 面板里拿到 `&mut Canvas` 会同时违反借用规则和"面板不改文档"的分工。
    /// 右侧检查器。与大纲同一套：闭包里只收动作，出了闭包再借 `canvas`——
    /// 面板里拿到 `&mut Canvas` 会同时违反借用规则和"面板不改文档"的分工。
    pub(crate) fn inspector_panel(
        &mut self,
        ui: &mut egui::Ui,
        t: f32,
    ) -> Option<panels::inspector::Action> {
        let canvas = self.canvas.as_ref()?;
        let doc = &canvas.session().doc;
        let theme = &canvas.session().theme;
        let selected = canvas.selected_ids();
        let ch = self.chrome;
        // 预设列表很短（几条），每帧克隆一次换来干净的借用关系。
        let presets = self.presets.items.clone();
        let annotation = canvas.annotation_selection().cloned();
        let annotations = canvas.annotation_selections().to_vec();
        let relationship = canvas.relationship_selection().cloned();
        let relationships = canvas.relationship_selections().to_vec();
        // 分组开合住在窗口状态里（关掉面板再打开也要还在），这里只借一份快照。
        let groups = self.window_state.inspector_groups().to_vec();
        // 与 `selected` 同序的节点盒，给圆角 stepper 算「盒高能容纳的上限」（V1-02 G-4）。
        let scene = canvas.scene();
        let boxes: Vec<Option<Rect>> = selected
            .iter()
            .map(|id| {
                scene.find(id).map(|idx| {
                    let r = scene.rect(idx);
                    Rect::from_min_size(egui::pos2(r.x, r.y), egui::vec2(r.w, r.h))
                })
            })
            .collect();
        let inspector = &mut self.inspector;
        let focus = &mut self.focus_mem;
        let mut action = None;
        // 面板自己占一列，画布只会被挤窄，不会被盖住；窄窗给画布留至少 360px。
        // 宽度区间按 `docs/30` §4.1：默认 288，可拖 240–360。
        let max_w = (ui.available_width() - 360.0).clamp(
            panels::inspector::PANEL_W_MIN,
            panels::inspector::PANEL_W_MAX,
        );
        let want_w = panels::inspector::clamp_width(self.inspector_w).min(max_w);
        let mut panel = egui::Panel::right("inspector")
            .resizable(t >= 0.999)
            .min_size(panels::inspector::PANEL_W_MIN)
            .max_size(max_w)
            .frame(egui::Frame::NONE.fill(ch.panel));
        panel = if t < 0.999 {
            panel.exact_size((want_w * t).max(1.0))
        } else {
            panel.default_size(want_w)
        };
        let shown = panel.show(ui, |ui| {
            if t < 0.999 {
                ui.set_min_width(want_w);
                ui.set_opacity(t.clamp(0.0, 1.0));
            }
            let panel = ui.max_rect();
            action = inspector.show(
                ui,
                doc,
                theme,
                &selected,
                focus,
                panels::inspector::Extras {
                    presets: &presets,
                    annotation: annotation.as_ref(),
                    annotations: &annotations,
                    relationship: relationship.as_ref(),
                    relationships: &relationships,
                    groups: &groups,
                    boxes: &boxes,
                },
            );
            ui.painter().vline(
                panel.left() + 0.5,
                panel.y_range(),
                Stroke::new(1.0, ch.sep),
            );
        });
        if t >= 0.999 {
            let actual = shown.response.rect.width();
            // 只有**用户真的拖了**才回写宽度：内容最小宽大于目标宽时面板会被撑开，
            // 那时把 actual 记成用户的选择，等于「拖到 240，下次打开变 293」。
            if shown.response.drag_stopped() && (actual - self.inspector_w).abs() > 0.5 {
                self.inspector_w = actual;
                self.window_state.set_inspector_width(actual);
            } else if shown.response.dragged() && (actual - self.inspector_w).abs() > 0.5 {
                // 拖动中只更新内存，松手才落盘，避免每个像素写一次 JSON。
                self.inspector_w = actual;
            }
            // 证据（`docs/31` SE-05）：头行/对象行/分段/分组标题的高度只在布局里知道，
            // 记一次就够，不让轨迹每帧长一行。
            if !self.inspector.traced {
                self.inspector.traced = true;
                crate::trace::event(&format!(
                    "inspector w={actual:.2} head={:.0} obj={:.0} seg={:.0} bar={:.1} group={:.0} gutter={:.0}",
                    panels::inspector::HEAD_H,
                    panels::inspector::OBJ_H,
                    panels::inspector::SEG_H,
                    panels::inspector::SEG_BAR,
                    panels::inspector::GROUP_H,
                    panels::inspector::GUTTER,
                ));
            }
        }
        action
    }

    /// 存盘成功之后统一收尾：清恢复点、刷最近文件、给提示。
    pub(crate) fn after_saved(&mut self) {
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        let s = canvas.session();
        let (Some(path), title) = (s.path.clone(), s.title()) else {
            return;
        };
        let rev = s.doc.revision;
        // 同一笔保存只收尾一次。现在这条链有四个入口（普通保存 / 另存为 / 覆盖保存 /
        // 历史版本恢复），重复调用不能把同一个修订号再塞进 `.history`（那会白吃
        // `KEEP=10` 的额度）、也不能再刷一遍最近文件（SR-02 问题 5）。
        let last = self.last_finish.as_ref().map(|(p, r)| (p.as_path(), *r));
        if !save_finish_is_new(last, &path, rev) {
            crate::trace::event("after_saved_skip dup=1");
            return;
        }
        self.last_finish = Some((path.clone(), rev));
        // 在途的恢复点写盘停不下来，但不能再让它把刚删掉的槽位"复活"。
        self.autosave.invalidate(s);
        self.note_session_rev(rev);
        self.recent.touch(&path, &title, mw_core::now_ms());
        history::capture(&path);
        self.set_toast(format!("已保存 {title}"), false);
    }

    pub(crate) fn action_save(&mut self) {
        if self.save_phase == SavePhase::Saving {
            return;
        }
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.commit_editing();
        }
        // 只读是**这份文档**的属性（另一个实例占着协作锁），从标签自己的守卫上问。
        if self.active_read_only() {
            self.set_toast("当前文档只读：请另存为，或关闭另一个实例", true);
            return;
        }
        let path = self.canvas.as_ref().and_then(|c| c.session().path.clone());
        if let Some(p) = path.as_ref() {
            // 已知指纹也是这一份文档的；`None`（没有指纹）不算冲突，见 `file_guard::changed_since`。
            let known = self.active_print();
            if known.is_some() && file_guard::fingerprint(p) != known {
                self.conflict = Some(p.clone());
                return;
            }
        }
        // 没有路径（新建文档）时直接转"另存为"——Ctrl+S 对用户来说就是"存下来"。
        let Some(_) = path else {
            self.action_save_as();
            return;
        };
        // 先让「保存中…」完整画一帧：真正写盘在下一帧开头（见 `pump_save`）。
        self.save_phase = SavePhase::Saving;
        crate::trace::event("save_start");
        self.ctx.request_repaint();
    }

    /// 把「保存中」真正落盘。调用点只有两处：每帧开头（上一帧已画出 Saving）
    /// 与关闭确认（用户等不起下一帧）。失败绑定当前修订号，之后的编辑自动回到 Dirty。
    pub(crate) fn pump_save(&mut self) {
        if self.save_phase != SavePhase::Saving {
            return;
        }
        let Some(canvas) = self.canvas.as_mut() else {
            self.save_phase = SavePhase::Idle;
            return;
        };
        let rev = canvas.session().doc.revision;
        // 还没落过盘：转「另存为」，跟 `action_save` 走同一条路。
        if canvas.session().path.is_none() {
            self.save_phase = SavePhase::Idle;
            self.action_save_as();
            return;
        }
        match canvas.save() {
            Ok(()) => {
                self.save_phase = SavePhase::Idle;
                // 指纹重新取自磁盘，但只写回这一份文档自己的守卫。
                if let Some(g) = self.guard_mut() {
                    g.refresh_print();
                }
                crate::trace::event(&format!("save_ok rev={rev}"));
                self.after_saved();
            }
            Err(e) => {
                crate::trace::event(&format!("save_fail rev={rev} raw={e}"));
                let message = crate::app_helpers::human_save_error(&e);
                self.set_toast(format!("保存失败：{message}"), true);
                self.save_phase = SavePhase::Failed { rev, message };
            }
        }
    }

    /// 回到前台时比对一次磁盘指纹：发现外部修改就先提示，保存时仍会走冲突弹窗。
    pub(crate) fn warn_if_external_change(&mut self) {
        if self.save_phase == SavePhase::Saving || self.active_read_only() {
            return;
        }
        let Some(path) = self.canvas.as_ref().and_then(|c| c.session().path.clone()) else {
            return;
        };
        let Some(known) = self.active_print() else {
            return;
        };
        if file_guard::fingerprint(&path) != Some(known) {
            crate::trace::event(&format!("external_change path={}", file_label(&path)));
            self.set_toast("文件已在外部修改；保存时会提示冲突，可重新加载", true);
        }
    }

    pub(crate) fn action_save_as(&mut self) {
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        canvas.commit_editing();
        // 开放格式：思维导图 → .xmind、概念图 → .cxl（docs/34 §6）。
        let is_concept = canvas.session().doc.meta.layout.kind == "conceptMap";
        let ext = if is_concept { "cxl" } else { "xmind" };
        let label = if is_concept {
            "概念图"
        } else {
            "思维导图"
        };
        let raw = canvas.session().title();
        // 对话框默认名取主文件名（去旧扩展），兜底后缀由 save_doc 按 ext 补。
        let default = raw
            .rsplit_once('.')
            .map(|(stem, _)| stem.to_string())
            .unwrap_or_else(|| raw.clone());
        match files::save_doc(&default, ext, label, self.hwnd) {
            Ok(Some(path)) => {
                let rev = canvas.session().doc.revision;
                match canvas.save_to(&path) {
                    Ok(()) => {
                        self.save_phase = SavePhase::Idle;
                        // 另存为换了路径：旧文件的协作锁必须放掉，再按新路径重新武装。
                        if let Some(g) = self.guard_mut() {
                            g.arm(Some(&path));
                        }
                        crate::trace::event(&format!("save_ok rev={rev} save_as=1"));
                        self.after_saved()
                    }
                    Err(e) => {
                        crate::trace::event(&format!("save_fail rev={rev} save_as=1"));
                        self.set_toast(e.clone(), true);
                        self.save_phase = SavePhase::Failed {
                            rev,
                            message: crate::app_helpers::human_save_error(&e),
                        };
                    }
                }
            }
            Ok(None) => {}
            Err(e) => self.set_toast(e, true),
        }
    }

    /// 打开导出对话框。先把行内编辑提交掉：编辑器的 Enter/Esc 与浮层的 Enter/Esc 是同一批键，
    /// 编辑还开着时它们会被编辑器先吃掉（并且 Enter 只在编辑器里换行）。
    /// 打开导出对话框。先把行内编辑提交掉：编辑器的 Enter/Esc 与浮层的 Enter/Esc 是同一批键，
    /// 编辑还开着时它们会被编辑器先吃掉（并且 Enter 只在编辑器里换行）。
    pub(crate) fn open_export(&mut self) {
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.commit_editing();
        }
        // 记住这次的选项：下次打开对话框直接是上次的样子。
        export_prefs::Prefs::capture(
            self.export.fmt,
            self.export.scope,
            self.export.scale,
            self.export.opaque,
        )
        .save();
        self.export.open = true;
    }

    /// Ctrl+P：打印语义落到「导出 PDF」，并预先选中 PDF 格式。
    pub(crate) fn open_export_pdf(&mut self) {
        self.open_export();
        self.export.fmt = export::Format::Pdf;
        export_prefs::Prefs::capture(
            self.export.fmt,
            self.export.scope,
            self.export.scale,
            self.export.opaque,
        )
        .save();
    }

    /// 导出对话框：选格式 →（位图选比例；位图/矢量选背景）→ 选落盘位置。
    ///
    /// 顺序和保存一致：**先问路径，再干活**。路径取消就什么都没发生，不需要「回滚一次导出」。
    /// 选项存在 `self.export` 里而不随对话框销毁——连着导两份不同格式是常事。
    /// 导出对话框：选格式 →（位图选比例；位图/矢量选背景）→ 选落盘位置。
    ///
    /// 顺序和保存一致：**先问路径，再干活**。路径取消就什么都没发生，不需要「回滚一次导出」。
    /// 选项存在 `self.export` 里而不随对话框销毁——连着导两份不同格式是常事。
    pub(crate) fn export_dialog(
        &mut self,
        ctx: &egui::Context,
        rs: Option<&egui_wgpu::RenderState>,
    ) {
        if !self.export.open {
            return;
        }
        let bounds = self.canvas.as_ref().map(Canvas::bounds);
        let has_selection = self.canvas.as_ref().is_some_and(Canvas::has_selection);
        let ch = self.chrome;
        let mut opts = self.export;
        let presets = self.export_presets.clone();
        let mut apply_preset: Option<usize> = None;
        let mut save_preset = false;
        let actions = [
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
            dialog::DialogAction::new("导出…", ui_kit::ButtonTone::Primary),
        ];
        let spec = dialog::DialogSpec::new("导出")
            .width(440.0)
            .enter_confirms(true)
            .default_action(1);
        let out = dialog::show(ctx, ch, "export", spec, &actions, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("预设");
                let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
                if let Some(i) = crate::ui_kit::select(
                    ui,
                    ch,
                    "export-preset",
                    crate::ui_kit::SelectSpec {
                        current: None,
                        selected_text: "选择…",
                        options: &names,
                        font: Type::BODY,
                        width: None,
                    },
                ) {
                    apply_preset = Some(i);
                }
                if ui.button("保存当前").clicked() {
                    save_preset = true;
                }
            });
            ui.add_space(6.0);
            section_label(ch, ui, "格式");
            ui.horizontal_wrapped(|ui| {
                for f in export::Format::ALL {
                    if ui.selectable_label(opts.fmt == f, f.label()).clicked() {
                        opts.fmt = f;
                    }
                }
            });
            ui.add_space(10.0);
            {
                ui.horizontal(|ui| {
                    section_label(ch, ui, "范围");
                    if ui
                        .selectable_label(
                            opts.scope == export::Scope::Whole,
                            export::Scope::Whole.label(),
                        )
                        .clicked()
                    {
                        opts.scope = export::Scope::Whole;
                    }
                    ui.add_enabled_ui(has_selection, |ui| {
                        if ui
                            .selectable_label(
                                opts.scope == export::Scope::Selection,
                                export::Scope::Selection.label(),
                            )
                            .on_disabled_hover_text("先在画布上选中一个主题")
                            .clicked()
                        {
                            opts.scope = export::Scope::Selection;
                        }
                    });
                    // 「可视区域」只有画面类格式有意义：文本没有视口。
                    if opts.fmt.has_background()
                        && ui
                            .selectable_label(
                                opts.scope == export::Scope::Viewport,
                                export::Scope::Viewport.label(),
                            )
                            .on_hover_text("只导出当前相机看到的范围")
                            .clicked()
                    {
                        opts.scope = export::Scope::Viewport;
                    }
                });
                ui.add_space(6.0);
            }
            if opts.fmt.is_image() {
                ui.horizontal(|ui| {
                    section_label(ch, ui, "比例");
                    for s in [1.0f32, 2.0, 3.0] {
                        if ui
                            .selectable_label(
                                (opts.scale - s).abs() < 0.01,
                                format!("{}×", s as i32),
                            )
                            .clicked()
                        {
                            opts.scale = s;
                        }
                    }
                });
                ui.add_space(6.0);
            }
            if opts.fmt.has_background() {
                ui.horizontal(|ui| {
                    section_label(ch, ui, "背景");
                    ui.radio_value(&mut opts.opaque, true, "文档底色");
                    ui.radio_value(&mut opts.opaque, false, "透明");
                });
                ui.add_space(8.0);
            }
            if opts.fmt == export::Format::Pdf {
                if let Some(b) = bounds {
                    ui.label(
                        egui::RichText::new(pdf_note(b.w, b.h, opts.scale))
                            .size(Type::SMALL)
                            .color(ch.dim),
                    );
                }
                ui.label(
                    egui::RichText::new("内容以位图嵌入：文字不可选中，放得很大能看出像素。")
                        .size(Type::SMALL)
                        .color(ch.dim),
                );
                ui.label(
                    egui::RichText::new(if opts.scope == export::Scope::Selection {
                        "只导出选中主题及其子树；折叠的分支按当前折叠状态。"
                    } else if opts.scope == export::Scope::Viewport {
                        "只导出当前视口范围；视口外的内容会被裁掉。"
                    } else {
                        "导出整棵主题树——折叠起来的分支也照样包含在内。"
                    })
                    .size(Type::SMALL)
                    .color(ch.dim),
                );
            } else if opts.fmt.is_image() {
                if let Some(b) = bounds {
                    ui.label(
                        egui::RichText::new(size_note(b.w, b.h, opts.scale))
                            .size(Type::SMALL)
                            .color(ch.dim),
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new(if opts.scope == export::Scope::Selection {
                        "只导出选中主题及其子树。"
                    } else {
                        "导出整份文档（含全部自由主题）。"
                    })
                    .size(Type::SMALL)
                    .color(ch.dim),
                );
                if opts.fmt == export::Format::Svg {
                    ui.label(
                        egui::RichText::new("矢量格式：任意放大不糊，文字可选中。")
                            .size(Type::SMALL)
                            .color(ch.dim),
                    );
                }
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("回车确认 · Esc 取消")
                    .size(Type::SMALL)
                    .color(ch.dim),
            );
        });
        if let Some(i) = apply_preset {
            if let Some(p) = presets.get(i) {
                if let Some(f) = export::Format::ALL.iter().find(|f| f.ext() == p.format) {
                    opts.fmt = *f;
                }
                if let Some(s) = export::Scope::ALL.iter().find(|s| s.key() == p.scope) {
                    opts.scope = *s;
                }
                opts.scale = p.scale.clamp(0.5, 8.0);
                opts.opaque = p.opaque;
            }
        }
        if save_preset {
            let p = export_presets::Preset {
                name: format!("{} · {}×", opts.fmt.label(), opts.scale),
                format: opts.fmt.ext().to_string(),
                scope: opts.scope.key().to_string(),
                scale: opts.scale,
                opaque: opts.opaque,
            };
            if let Some(old) = self.export_presets.iter_mut().find(|x| x.name == p.name) {
                *old = p;
            } else {
                self.export_presets.push(p);
            }
            match export_presets::save(&self.export_presets) {
                Ok(()) => self.set_toast("导出预设已保存", false),
                Err(e) => self.set_toast(format!("保存预设失败：{e}"), true),
            }
        }
        self.export = opts;
        if out.action == Some(1) {
            self.export.open = false;
            self.run_export(rs);
        } else if out.closed {
            self.export.open = false;
        }
    }

    /// 真正落盘。文本四种走 `mw-io` 的序列化，PNG/PDF 交给后台线程（P11：UI 不在
    /// 渲染/压缩/写盘的几秒里卡住），SVG 走纯 CPU 的几何→标记管线。
    /// 真正落盘。文本四种走 `mw-io` 的序列化，PNG/PDF 交给后台线程（P11：UI 不在
    /// 渲染/压缩/写盘的几秒里卡住），SVG 走纯 CPU 的几何→标记管线。
    pub(crate) fn run_export(&mut self, rs: Option<&egui_wgpu::RenderState>) {
        let fmt = self.export.fmt;
        let (scale, opaque) = (self.export.scale, self.export.opaque);
        // 范围对两种管线都生效：位图/矢量换场景，文本换一份临时子文档。
        let use_selection = self.export.scope == export::Scope::Selection
            && self.canvas.as_ref().is_some_and(Canvas::has_selection);
        // 可视区域只对位图/矢量有意义：文本格式没有「视口」这个概念。
        let use_viewport = self.export.scope == export::Scope::Viewport && fmt.has_background();
        let hwnd = self.hwnd;
        // 位图导出很重：同时开两张会把显存和回读缓冲翻倍，也会让"哪张先完成"变得随机。
        if fmt.is_image() && self.export_job.is_some() {
            self.set_toast("上一张图还在导出，请等它完成", true);
            return;
        }
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        canvas.commit_editing();
        let stem = export::file_stem(&canvas.session().title());
        let path = match files::save_export(&stem, fmt.ext(), fmt.label(), hwnd) {
            Ok(Some(p)) => p,
            Ok(None) => return,
            Err(e) => {
                self.set_toast(e, true);
                return;
            }
        };
        let outcome = match fmt {
            // 矢量 PDF 走字形轮廓，不碰 GPU：没有可用后端时也能导出。
            export::Format::Pdf => match self.canvas.as_ref() {
                Some(c) => {
                    let scene = c.export_scene(use_selection);
                    let theme = c.session().theme.clone();
                    let bounds = use_viewport.then(|| c.view_bounds());
                    self.export_job =
                        Some(export_job::start_vector(path.clone(), scene, theme, bounds));
                    Ok(format!("正在导出 {}…", file_label(&path)))
                }
                None => return,
            },
            export::Format::Png => match rs {
                Some(rs) => {
                    let (scene, theme, bounds) = match self.canvas.as_ref() {
                        Some(c) => (
                            c.export_scene(use_selection),
                            c.session().theme.clone(),
                            use_viewport.then(|| c.view_bounds()),
                        ),
                        None => return,
                    };
                    self.export_job = Some(export_job::start(
                        rs.device.clone(),
                        rs.queue.clone(),
                        rs.adapter.get_info(),
                        export_job::Request {
                            kind: export_job::Kind::Png,
                            path: path.clone(),
                            scene,
                            theme,
                            scale,
                            opaque,
                            bounds,
                        },
                    ));
                    Ok(format!("正在导出 {}…", file_label(&path)))
                }
                None => Err("没有可用的 GPU 后端，PNG 导出用不了".to_string()),
            },
            export::Format::Svg => {
                let Some(canvas) = self.canvas.as_ref() else {
                    return;
                };
                let svg = export_svg::render(
                    &canvas.export_scene(use_selection),
                    &canvas.session().theme,
                    &canvas.session().doc.assets,
                    export_svg::Options {
                        transparent: !opaque,
                        bounds: use_viewport.then(|| canvas.view_bounds()),
                    },
                );
                export::write_text(&path, &svg).map(|()| format!("已导出 {}", file_label(&path)))
            }
            _ => {
                let Some(canvas) = self.canvas.as_ref() else {
                    return;
                };
                // 「选中子树」在这里换成一份临时文档；序列化器本身不用改。
                let sub = if use_selection {
                    canvas.selection_document()
                } else {
                    None
                };
                let doc = sub.as_ref().unwrap_or(&canvas.session().doc);
                // 序列化返回 `None` 时必须报错，不能 `unwrap_or_default()` 写出一份**空文件**
                // 再提示「已导出」—— 那是拿用户的原文件换一个 0 字节。
                // `?` 不行（会从整个方法提前返回、跳过下面的 toast），就地化成 `Err` 走同一条提示通道。
                match fmt.text(doc) {
                    Some(text) => export::write_text(&path, &text)
                        .map(|()| format!("已导出 {}", file_label(&path))),
                    None => Err("导出失败：这个格式无法表示当前文档".to_string()),
                }
            }
        };
        match outcome {
            Ok(msg) => self.set_toast(msg, false),
            Err(e) => self.set_toast(e, true),
        }
    }

    /// 收后台导出结果。在途时 120ms 催一帧：完成提示要跟手，又不该把空闲功耗顶满。
    pub(crate) fn export_tick(&mut self, ctx: &egui::Context) {
        let Some(job) = self.export_job.as_mut() else {
            return;
        };
        let Some(result) = job.poll() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
            return;
        };
        let name = job.label().to_string();
        let total = job.elapsed();
        self.export_job = None;
        if self.bench_started {
            self.bench_started = false;
            match &result {
                Ok(r) => eprintln!(
                    "export_bench\tworker={:.1}ms\ttotal={:.1}ms\t{}x{}\t{}B\tpages={}\teff={:.3}",
                    r.elapsed.as_secs_f64() * 1000.0,
                    total.as_secs_f64() * 1000.0,
                    r.width,
                    r.height,
                    r.bytes,
                    r.pages,
                    r.eff
                ),
                Err(e) => eprintln!("export_bench\tFAILED\t{e}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        match result {
            Ok(r) => {
                let pages = if r.pages > 1 {
                    format!("，{} 页", r.pages)
                } else {
                    String::new()
                };
                let msg = if (r.eff - self.export.scale).abs() > 0.01 {
                    format!(
                        "已导出 {name}（{}×{}{pages}，原比例超出上限，已按 {:.2}× 收小）",
                        r.width, r.height, r.eff
                    )
                } else {
                    format!(
                        "已导出 {name}（{}×{}{pages}，{:.1}s）",
                        r.width,
                        r.height,
                        r.elapsed.as_secs_f64()
                    )
                };
                self.set_toast(msg, false);
            }
            Err(e) => self.set_toast(e, true),
        }
    }

    /// P11 量法：首帧画布后按 `MW_EXPORT_BENCH=<out.png>` 自动导一张 2× PNG，
    /// 打印 UI 线程快照成本与后台总耗时后退出。走的是与 `Ctrl+E` 完全相同的任务管线。
    /// P11 量法：首帧画布后按 `MW_EXPORT_BENCH=<out.png>` 自动导一张 2× PNG，
    /// 打印 UI 线程快照成本与后台总耗时后退出。走的是与 `Ctrl+E` 完全相同的任务管线。
    pub(crate) fn start_bench_export(&mut self, rs: &egui_wgpu::RenderState) {
        let Some(out) = self.bench_export.take() else {
            return;
        };
        // 探针扩展：`MW_EXPORT_SCOPE=selection` 时先选中根的第一个孩子，
        // 再用「选中子树」这条产品路径导出——这样范围功能就有端到端证据，
        // 而不是只靠单测。没有选中节点时行为与整张导图一致。
        let scope_env = std::env::var("MW_EXPORT_SCOPE").unwrap_or_default();
        let want_selection = scope_env.eq_ignore_ascii_case("selection");
        let want_viewport = scope_env.eq_ignore_ascii_case("viewport");
        // 视口导出要给相机一个放大档，否则首帧 fit 之后的视口几乎等于整图，量不出差别。
        if want_viewport {
            let zoom: f32 = std::env::var("MW_EXPORT_ZOOM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2.0);
            if let Some(canvas) = self.canvas.as_mut() {
                // 探针走的也是产品入口，轨迹里记成按钮路径（不是用户操作，只借这条管线）。
                canvas.zoom_by(zoom, crate::canvas::ZoomSrc::Button);
            }
        }
        if want_selection {
            if let Some(canvas) = self.canvas.as_mut() {
                if !canvas.has_selection() {
                    let doc = &canvas.session().doc;
                    let first = doc
                        .node(&doc.root_id)
                        .and_then(|n| n.children.first())
                        .cloned();
                    if let Some(id) = first {
                        canvas.select_node(&id);
                    }
                }
            }
        }
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        let t = std::time::Instant::now();
        let scene = canvas.export_scene(want_selection && canvas.has_selection());
        let bounds = want_viewport.then(|| canvas.view_bounds());
        let theme = canvas.session().theme.clone();
        let snapshot = t.elapsed();
        let scale: f32 = std::env::var("MW_EXPORT_SCALE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2.0);
        // 后缀决定落盘格式：bench 要能端到端跑遍 7 种格式，不只是 PNG/PDF。
        // 文本四种与 SVG 都是纯 CPU，直接在这里写完并报字节数；位图/PDF 才进工作线程。
        let ext = out
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        let text_fmt = match ext.as_str() {
            "svg" => Some(None),
            "md" => Some(Some(export::Format::Markdown)),
            "txt" => Some(Some(export::Format::Outline)),
            "opml" => Some(Some(export::Format::Opml)),
            "mm" => Some(Some(export::Format::Freemind)),
            _ => None,
        };
        if let Some(text_fmt) = text_fmt {
            let t = std::time::Instant::now();
            let body = match text_fmt {
                None => export_svg::render(
                    &scene,
                    &theme,
                    &canvas.session().doc.assets,
                    export_svg::Options {
                        transparent: false,
                        bounds,
                    },
                ),
                Some(fmt) => fmt.text(&canvas.session().doc).unwrap_or_default(),
            };
            let written = export::write_text(&out, &body);
            eprintln!(
                "export_bench	copy={:.2}ms	bytes={}	ext={ext}	ok={}",
                t.elapsed().as_secs_f64() * 1000.0,
                body.len(),
                written.is_ok()
            );
            self.bench_started = true;
            return;
        }
        let kind = if ext == "pdf" {
            export_job::Kind::Pdf
        } else {
            export_job::Kind::Png
        };
        eprintln!(
            "export_bench\tsnapshot={:.2}ms\tnodes={}\tscale={scale}\tkind={kind:?}",
            snapshot.as_secs_f64() * 1000.0,
            scene.nodes.len()
        );
        self.bench_started = true;
        self.export_job = Some(export_job::start(
            rs.device.clone(),
            rs.queue.clone(),
            rs.adapter.get_info(),
            export_job::Request {
                kind,
                path: out,
                scene,
                theme,
                scale,
                opaque: true,
                bounds,
            },
        ));
    }

    /// 换文档前先看脏不脏：脏就挂起，等用户决定；干净就直接做。
    /// 「打开 / 新建」共用这一条路——它们都会让当前文档消失，没有理由只拦其中一个。
    /// 导入不走这里：它开新标签页，不碰当前文档。
    /// 换文档前先看脏不脏：脏就挂起，等用户决定；干净就直接做。
    /// 「打开 / 新建」共用这一条路——它们都会让当前文档消失，没有理由只拦其中一个。
    /// 导入不走这里：它开新标签页，不碰当前文档。
    pub(crate) fn request(&mut self, action: PendingAction) {
        // 这份文件已经开着就直接切过去：既省一次读盘，也避免同一文件出现两个标签页
        // 各改一份、谁覆盖谁的悬案。
        if let PendingAction::Open(path) = &action {
            if let Some(i) = self.tab_index_of_path(path) {
                self.switch_tab(i);
                self.set_toast("该文档已打开，已切到它的标签页", false);
                return;
            }
        }
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.commit_editing();
        }
        if self.canvas.as_ref().is_some_and(|c| c.session().dirty()) {
            self.pending = Some(action);
        } else {
            self.run(action);
        }
    }

    /// 「重新加载」的入口：跟 `request` 一样先过脏检查，只是它不换标签页。
    ///
    /// 不能直接复用 `request(Open(path))`：同一个文件已经在活动标签里，
    /// `request` 会认为「已经打开、切过去就行」而直接返回，什么也没重载。
    pub(crate) fn request_reload(&mut self, path: &std::path::Path) {
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.commit_editing();
        }
        if self.canvas.as_ref().is_some_and(|c| c.session().dirty()) {
            crate::trace::event("reload_needs_confirm");
            self.pending = Some(PendingAction::Reload(path.to_path_buf()));
        } else {
            self.run(PendingAction::Reload(path.to_path_buf()));
        }
    }

    pub(crate) fn run(&mut self, action: PendingAction) {
        // 能走到这里说明用户已经确认过了（干净时 `request` 直接来、脏时在
        // `discard_prompt` 里明确选了「先保存」或「直接继续」）。放行一次
        // `load_open` 的脏检查，否则确认完还会被自己拦一次。
        self.allow_replace = true;
        match action {
            PendingAction::Open(path) => self.start_open(&path),
            PendingAction::Reload(path) => {
                // 重新加载：先把这一页的守卫放掉——旧锁与旧指纹都不再代表将要读进来的内容。
                if let Some(g) = self.guard_mut() {
                    g.release();
                }
                self.start_open(&path);
            }
            PendingAction::New => {
                // 搜索结果指向旧文档的节点 id，不关会跳到一个不存在的选择上。
                self.search.close();
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.replace_session(Session::blank());
                }
                // 新文档没有路径：把这一页的协作锁放掉，别让旧文件到关窗都是只读。
                if let Some(g) = self.guard_mut() {
                    g.release();
                }
                self.note_session_rev(0);
                self.home = false;
                self.home_return = false;
                self.allow_replace = false;
                self.set_toast("已新建空白文档", false);
            }
        }
    }

    /// 打开的第一步：先挂一张「正在打开」计划，完整画一帧后再读盘。
    ///
    /// 读盘放在下一帧开头（[`Shell::pump_open`]），大文件不会把这一帧的按钮/暗幕
    /// 卡成未响应；缺失文件在 `pump_open` 里直接转失败弹窗，不写「最近」列表。
    pub(crate) fn start_open(&mut self, path: &std::path::Path) {
        if self.open_plan.is_some() {
            return;
        }
        self.search.close();
        self.palette.close();
        self.context_menu = None;
        self.tab_menu = None;
        self.open_failed = None;
        crate::trace::event(&format!("open_start name={}", file_label(path)));
        self.open_plan = Some(OpenPlan {
            path: path.to_path_buf(),
            painted: false,
            demo: false,
        });
        self.ctx.request_repaint();
    }

    /// 上一帧已经画出「正在打开」：这一帧开头才真正读盘。
    pub(crate) fn pump_open(&mut self) {
        let Some(plan) = self.open_plan.as_ref() else {
            return;
        };
        if !plan.painted || plan.demo {
            return;
        }
        let plan = self.open_plan.take().expect("open plan checked above");
        if !plan.path.is_file() {
            self.fail_open(&plan.path, "文件不存在或已被移动".into());
            return;
        }
        self.load_open(&plan.path);
    }

    /// 真正的读盘与换文档。失败不改「最近」列表——用户可能只是暂时拔了 U 盘。
    ///
    /// 脏检查守在这一层：任何「换掉当前文档」的通道最后都落到这里，
    /// 只守在按钮那一处不够（SR-02 问题 1 的丢稿链就是从「重新加载」绕过检查来的）。
    pub(crate) fn load_open(&mut self, path: &std::path::Path) {
        let allowed = std::mem::replace(&mut self.allow_replace, false);
        if !allowed && self.canvas.as_ref().is_some_and(|c| c.session().dirty()) {
            crate::trace::event("open_blocked dirty=1");
            self.pending = Some(PendingAction::Open(path.to_path_buf()));
            return;
        }
        match Session::open(path) {
            Ok(session) => {
                let name = session.title();
                let rev = session.doc.revision;
                self.search.close();
                self.palette.close();
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.replace_session(session);
                }
                self.note_session_rev(rev);
                self.recent.touch(path, &name, mw_core::now_ms());
                // 守卫按新路径重新武装：换文档就等于换锁/换指纹/换只读态。
                if let Some(g) = self.guard_mut() {
                    g.arm(Some(path));
                }
                let read_only = self.active_read_only();
                self.conflict = None;
                self.save_phase = SavePhase::Idle;
                // 刚打开就看有没有比它更新的恢复点（上次崩溃留下的）。
                self.recovery = autosave::pending_for(path).map(|slot| (slot, path.to_path_buf()));
                self.home = false;
                self.home_return = false;
                crate::trace::event(&format!(
                    "open_ok name={} read_only={}",
                    file_label(path),
                    read_only
                ));
                if read_only {
                    self.set_toast(
                        format!("{name} 正被另一个实例编辑，本实例只读（可另存为）"),
                        true,
                    );
                } else {
                    self.set_toast(format!("已打开 {name}"), false);
                }
                self.persist_tabs();
            }
            Err(e) => self.fail_open(path, e),
        }
    }

    /// 打开失败落到弹窗：文件名、原因与「从最近列表移除」入口都在这里决定。
    fn fail_open(&mut self, path: &std::path::Path, reason: String) {
        crate::trace::event(&format!("open_fail name={}", file_label(path)));
        self.open_plan = None;
        self.open_failed = Some(OpenFailure {
            path: path.to_path_buf(),
            reason,
            from_recent: self.recent.contains(path),
        });
        self.ctx.request_repaint();
    }

    /// 「正在打开」弹窗：没有动作按钮，也不可 Esc 关闭；画一帧后让下一帧读盘。
    pub(crate) fn opening_ui(&mut self, ctx: &egui::Context) {
        let ch = self.chrome;
        let Some(plan) = self.open_plan.as_mut() else {
            return;
        };
        let name = plan
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("文档")
            .to_string();
        let parent = plan
            .path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let spec = dialog::DialogSpec::new("正在打开…")
            .width(420.0)
            .dismissible(false);
        let out = dialog::show(ctx, ch, "opening", spec, &[], |ui| {
            ui.label(egui::RichText::new(&name).size(Type::BODY).color(ch.text));
            if !parent.is_empty() {
                ui.label(
                    egui::RichText::new(shorten(&parent, 52))
                        .size(Type::SMALL)
                        .color(ch.faint),
                );
            }
            ui.add_space(6.0);
            // 静态进度条：golden 不因动画相位漂移；真实读盘通常只有一两帧。
            let (track, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(track, egui::CornerRadius::same(2), ch.panel2);
            let head = egui::Rect::from_min_size(
                track.min,
                egui::vec2(track.width() * 0.38, track.height()),
            );
            ui.painter()
                .rect_filled(head, egui::CornerRadius::same(2), ch.accent);
            ui.label(
                egui::RichText::new("正在读取文件，请稍候")
                    .size(Type::SMALL)
                    .color(ch.dim),
            );
        });
        if !out.closed {
            if !plan.painted {
                plan.painted = true;
                ctx.request_repaint();
            }
            return;
        }
        // 不可关闭的弹窗理论上来不了这里；万一框架给出关闭，保持计划不丢。
        self.open_plan = None;
    }

    /// 打开失败弹窗：重试 / 换一个文件 / 从最近列表移除 / 关闭。
    pub(crate) fn open_failed_ui(&mut self, ctx: &egui::Context) {
        let Some(failure) = self.open_failed.clone() else {
            return;
        };
        let name = failure
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("文档")
            .to_string();
        let full_path = failure.path.display().to_string();
        let mut actions = vec![
            dialog::DialogAction::new("重试", ui_kit::ButtonTone::Primary),
            dialog::DialogAction::new("打开其他文件…", ui_kit::ButtonTone::Secondary),
        ];
        if failure.from_recent {
            actions.push(dialog::DialogAction::new(
                "从最近列表移除",
                ui_kit::ButtonTone::Ghost,
            ));
        }
        actions.push(dialog::DialogAction::new(
            "关闭",
            ui_kit::ButtonTone::Secondary,
        ));
        let spec = dialog::DialogSpec::new("打不开文档")
            .width(480.0)
            .default_action(open_failed_close_index(failure.from_recent));
        let out = dialog::show(ctx, self.chrome, "open-failed", spec, &actions, |ui| {
            ui.label(
                egui::RichText::new(&name)
                    .size(Type::BODY)
                    .color(self.chrome.text),
            );
            if !full_path.is_empty() {
                ui.label(
                    egui::RichText::new(&full_path)
                        .size(Type::SMALL)
                        .color(self.chrome.faint),
                );
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(&failure.reason)
                    .size(Type::BODY)
                    .color(self.chrome.warn),
            );
            ui.label(
                egui::RichText::new("文件可能已被移动、删除，或当前账号没有访问权限。")
                    .size(Type::SMALL)
                    .color(self.chrome.dim),
            );
        });
        match out
            .action
            .and_then(|i| open_failed_action(i, failure.from_recent))
        {
            Some(OpenFailedAction::Retry) => {
                self.open_failed = None;
                self.start_open(&failure.path);
            }
            Some(OpenFailedAction::OpenOther) => {
                self.open_failed = None;
                self.action_open();
            }
            Some(OpenFailedAction::RemoveRecent) => {
                self.recent.forget(&failure.path);
                self.open_failed = None;
                self.set_toast("已从最近列表移除", false);
            }
            Some(OpenFailedAction::Close) => {
                self.open_failed = None;
            }
            None => {
                if out.closed {
                    self.open_failed = None;
                }
            }
        }
    }

    /// 导入成**新标签页**（docs/02 §8：导入不覆盖当前文档，也不需要未保存确认）。
    /// 导入结果没有文件路径，第一次保存会走「另存为」。
    /// 导入成**新标签页**（docs/02 §8：导入不覆盖当前文档，也不需要未保存确认）。
    /// 导入结果没有文件路径，第一次保存会走「另存为」。
    pub(crate) fn import_path(&mut self, path: &std::path::Path) {
        if self.import_job.is_some() {
            self.set_toast("已有导入任务在进行", true);
            return;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("文档")
            .to_string();
        self.import_job = Some(import_job::start(path.to_path_buf()));
        self.set_toast(format!("正在导入 {name}…"), false);
    }
}

// ---------------------------------------------------------------------------
// O02 页头 / 面包屑 / 页脚
//
// 这三块全部自绘：统一 28px 命中区、hover 底色、圆角与字阶，不混用 `egui::Button`
// 的默认灰框。页头只放高频动作，窄窗把低频动作收进 `…`。
// ---------------------------------------------------------------------------

use panels::outline::Action as OutlineAction;

struct OutlineHeader<'a> {
    doc_title: &'a str,
    total: usize,
    selected: usize,
    hits: usize,
    focus_active: bool,
    can_focus: bool,
    primary: Option<&'a str>,
    search_open: bool,
    density: panels::outline::OutlineDensity,
    /// 聚焦路径（仅聚焦态非空）：页头面包屑 `大纲 › …` 的素材。
    crumbs: &'a [(String, String)],
    /// 概念图（CM-07）：页头要标「概念图 · 按链接层级展开」，否则用户不知道
    /// 这份大纲是**根据连线推出来的**，还以为可以直接拖层级。
    concept: bool,
}

#[derive(Clone, Copy)]
enum OutlineGlyph {
    Back,
    Search,
    More,
}

fn outline_menu_id(name: &str) -> egui::Id {
    egui::Id::unique(("mw-outline-menu", name))
}

/// 28×28 工具按钮，命中区 32。`menu` 有值时把弹层开合也算进激活态。
#[allow(clippy::too_many_arguments)]
fn outline_tool_button(
    ui: &mut egui::Ui,
    ch: Chrome,
    rect: Rect,
    id: &str,
    glyph: OutlineGlyph,
    active: bool,
    enabled: bool,
    tip: &str,
    menu: Option<&str>,
) -> egui::Response {
    let menu_open =
        menu.is_some_and(|name| egui::Popup::is_id_open(ui.ctx(), outline_menu_id(name)));
    let icon = match glyph {
        OutlineGlyph::Back => Icon::Back,
        OutlineGlyph::Search => Icon::Search,
        OutlineGlyph::More => Icon::More,
    };
    crate::ui_kit::tool_button(
        ui,
        ch,
        rect,
        egui::Id::unique(("mw-outline-tool", id)),
        icon,
        crate::ui_kit::ToolButtonState {
            active: active || menu_open,
            enabled,
            tip,
        },
    )
}

fn outline_text_width(ui: &egui::Ui, text: &str, font: &FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x
}

fn outline_elide(ui: &egui::Ui, text: &str, font: &FontId, max: f32) -> String {
    if outline_text_width(ui, text, font) <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    for ch in text.chars() {
        out.push(ch);
        let probe = format!("{out}…");
        if outline_text_width(ui, &probe, font) > max {
            out.pop();
            break;
        }
    }
    if out.trim().is_empty() {
        String::new()
    } else {
        format!("{}…", out.trim_end())
    }
}

fn draw_outline_header(
    ui: &mut egui::Ui,
    ch: Chrome,
    rect: Rect,
    info: &OutlineHeader,
    action: &mut Option<OutlineAction>,
) {
    let margin = panels::outline::column_margin(rect.width());
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    let cy = rect.center().y;
    let narrow = rect.width() < 980.0;
    let btn = 28.0;
    let gap = 8.0; // docs/28 §5.1：组间 8
    let mut x = right;

    // 从右往左：`⋯` 永远最右，旁边搜索；折叠/密度/聚焦收进更多菜单。
    let more_rect = Rect::from_center_size(Pos2::new(x - btn * 0.5, cy), Vec2::splat(btn));
    x -= btn + gap;
    let more = outline_tool_button(
        ui,
        ch,
        more_rect,
        "more",
        OutlineGlyph::More,
        false,
        true,
        "更多：折叠、密度、聚焦、帮助与文档信息",
        Some("more"),
    );

    let search_rect = Rect::from_center_size(Pos2::new(x - btn * 0.5, cy), Vec2::splat(btn));
    x -= btn + gap;
    let search = outline_tool_button(
        ui,
        ch,
        search_rect,
        "search",
        OutlineGlyph::Search,
        info.search_open,
        true,
        if info.search_open {
            "关闭搜索 · Ctrl+Shift+F"
        } else {
            "搜索大纲 · Ctrl+Shift+F"
        },
        None,
    );
    if search.clicked() {
        *action = Some(OutlineAction::ToggleSearch);
    }
    outline_more_menu(ch, &more, info, narrow, action);

    // 主题计数：12px 纯文字贴工具簇左侧；搜索时换命中口径。不再用数量胶囊。
    let count_text = if info.search_open {
        format!("{} / {} 命中", info.hits, info.total)
    } else if info.concept {
        // CM-07：概念图数的是概念，不是主题。
        format!("{} 个概念", info.total)
    } else {
        format!("{} 个主题", info.total)
    };
    let count_font = FontId::proportional(Type::SMALL);
    let count_w = outline_text_width(ui, &count_text, &count_font);
    let count_left = x - 12.0 - count_w;
    if count_left > left + 200.0 {
        ui.painter().text(
            Pos2::new(count_left, cy),
            Align2::LEFT_CENTER,
            &count_text,
            count_font,
            ch.dim,
        );
    }

    // 左：返回 + 面包屑。聚焦时 `大纲 › 路径…`，否则 `大纲 · 文档标题`；窄窗只留「大纲」。
    let back_rect = Rect::from_center_size(Pos2::new(left + btn * 0.5, cy), Vec2::splat(btn));
    let back = outline_tool_button(
        ui,
        ch,
        back_rect,
        "back",
        OutlineGlyph::Back,
        false,
        true,
        "返回画布 · F8",
        None,
    );
    if back.clicked() {
        *action = Some(OutlineAction::Close);
    }
    let mut text = String::from("大纲");
    if info.focus_active && !info.crumbs.is_empty() {
        for (_, t) in info.crumbs {
            text.push_str(" › ");
            text.push_str(t);
        }
    } else if !narrow && !info.doc_title.is_empty() {
        text.push_str(" · ");
        text.push_str(info.doc_title);
    }
    // CM-07：概念图的大纲是**生成树**，不是用户摆的层级，页头必须说清楚。
    if info.concept {
        text.push_str(" · 概念图 · 按链接层级展开");
    }
    let font = FontId::proportional(Type::SMALL);
    let cursor = back_rect.right() + 8.0;
    let avail = (count_left - 16.0 - cursor).max(0.0);
    let text = outline_elide(ui, &text, &font, avail);
    if !text.is_empty() {
        ui.painter().text(
            Pos2::new(cursor, cy),
            Align2::LEFT_CENTER,
            &text,
            font,
            ch.dim,
        );
    }
}

fn outline_menu_row(
    ui: &mut egui::Ui,
    ch: Chrome,
    label: &str,
    hint: &str,
    check: Option<bool>,
    enabled: bool,
) -> bool {
    crate::menu::popup_row(
        ui,
        ch,
        crate::menu::RowSpec {
            icon: None,
            label,
            shortcut: hint,
            check,
            enabled,
            submenu: false,
        },
    )
}

fn outline_more_menu(
    ch: Chrome,
    resp: &egui::Response,
    info: &OutlineHeader,
    narrow: bool,
    action: &mut Option<OutlineAction>,
) {
    let mut picked = None;
    egui::Popup::menu(resp)
        .id(outline_menu_id("more"))
        .gap(6.0)
        .width(228.0)
        .frame(crate::ui_kit::menu_frame(ch))
        .show(|ui| {
            let nav = crate::menu::PopupNav::begin(ui, "outline-more");
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.set_min_width(216.0);
            // 640×460 的最小窗里「更多」会比屏幕还高，宁可内部滚动也不让底部项掉出去。
            // Popup 默认只给按钮下方的“剩余高度”，菜单稍长就被截掉；
            // 先按整个窗口底边放宽，再让 ScrollArea 只在小窗里真正滚动。
            let top = ui.max_rect().top();
            ui.set_max_height((ui.ctx().content_rect().bottom() - top - 12.0).max(180.0));
            let max_h = ui.available_height();
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_min_width(216.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    picked = outline_more_rows(ui, ch, info, narrow);
                });
            if picked.is_some() {
                ui.close();
            }
            nav.end(ui);
        });
    if picked.is_some() {
        *action = picked;
    }
}

fn outline_more_rows(
    ui: &mut egui::Ui,
    ch: Chrome,
    info: &OutlineHeader,
    narrow: bool,
) -> Option<OutlineAction> {
    use panels::outline::OutlineDensity as D;
    let mut picked = None;
    if info.focus_active && outline_menu_row(ui, ch, "退出聚焦", "Esc", None, true) {
        picked = Some(OutlineAction::Unfocus);
    }
    if info.selected > 0 {
        menu_section(ch, ui, &format!("已选 {} 项", info.selected));
    }
    if narrow {
        menu_section(ch, ui, "折叠");
        if outline_menu_row(ui, ch, "展开全部", "", None, true) {
            picked = Some(OutlineAction::SetCollapsedAll { collapsed: false });
        }
        if outline_menu_row(ui, ch, "折叠全部", "", None, true) {
            picked = Some(OutlineAction::SetCollapsedAll { collapsed: true });
        }
        for level in 1u8..=3 {
            if outline_menu_row(ui, ch, &format!("只显示 {level} 级"), "", None, true) {
                picked = Some(OutlineAction::SetCollapsedLevel(level));
            }
        }
        menu_separator(ch, ui);
        menu_section(ch, ui, "行密度");
        for d in [D::Compact, D::Standard, D::Comfortable] {
            if outline_menu_row(ui, ch, d.label(), "", Some(d == info.density), true) {
                picked = Some(OutlineAction::SetDensity(d));
            }
        }
        menu_separator(ch, ui);
    }
    if info.focus_active {
        if outline_menu_row(ui, ch, "退出聚焦", "Esc", None, true) {
            picked = Some(OutlineAction::Unfocus);
        }
    } else if info.can_focus && outline_menu_row(ui, ch, "聚焦当前主题", "Alt+Enter", None, true)
    {
        if let Some(id) = info.primary {
            picked = Some(OutlineAction::Focus(id.to_string()));
        }
    }
    if outline_menu_row(ui, ch, "返回画布", "F8", None, true) {
        picked = Some(OutlineAction::Close);
    }
    menu_separator(ch, ui);
    if outline_menu_row(ui, ch, "快捷键帮助", "?", None, true) {
        picked = Some(OutlineAction::OpenHelp);
    }
    if outline_menu_row(ui, ch, "文档信息", "", None, true) {
        picked = Some(OutlineAction::ShowInfo);
    }
    picked
}

fn draw_outline_footer(
    ui: &mut egui::Ui,
    ch: Chrome,
    rect: Rect,
    stats_text: &str,
    short_stats: &str,
    dirty: bool,
    action: &mut Option<OutlineAction>,
) {
    let margin = panels::outline::column_margin(rect.width());
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    let cy = rect.center().y;
    ui.painter()
        .hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, ch.sep));

    // 右侧 `?`：唯一的帮助入口，不常驻按键表（docs/28 §5.1）。
    let help_rect = Rect::from_center_size(Pos2::new(right - 9.0, cy), Vec2::splat(22.0));
    let help = ui.interact(
        help_rect,
        egui::Id::unique("mw-outline-footer-help"),
        Sense::click(),
    );
    if help.hovered() {
        ui.painter().rect_filled(help_rect, 5.0, ch.hover);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        help_rect.center(),
        Align2::CENTER_CENTER,
        "?",
        FontId::proportional(Type::SMALL),
        if help.hovered() { ch.text } else { ch.faint },
    );
    if help.on_hover_text("快捷键帮助 · ?").clicked() {
        *action = Some(OutlineAction::OpenHelp);
    }

    let font = FontId::proportional(Type::SMALL);
    // 左侧：未保存点 + 计数与选中摘要。
    let mut x = left;
    if dirty {
        ui.painter()
            .circle_filled(Pos2::new(x + 3.0, cy), 3.0, ch.warn);
        let t = ui.painter().text(
            Pos2::new(x + 11.0, cy),
            Align2::LEFT_CENTER,
            "未保存",
            FontId::proportional(Type::CAPTION),
            ch.warn,
        );
        x = t.right() + 12.0;
    }
    let stats_avail = (help_rect.left() - 12.0 - x).max(0.0);
    let stats = if outline_text_width(ui, stats_text, &font) <= stats_avail {
        stats_text.to_owned()
    } else if outline_text_width(ui, short_stats, &font) <= stats_avail {
        short_stats.to_owned()
    } else {
        outline_elide(ui, short_stats, &font, stats_avail)
    };
    ui.painter().text(
        Pos2::new(x, cy),
        Align2::LEFT_CENTER,
        &stats,
        font,
        ch.faint,
    );
}

/// 被折叠隐藏时，返回最靠上的那个被折叠祖先（它自己一定仍可见）。
fn visible_ancestor(doc: &mw_core::Document, base: &str, id: &str) -> Option<String> {
    let mut anchor = id.to_string();
    let mut cur = id;
    for _ in 0..4096 {
        if cur == base {
            return Some(anchor);
        }
        let node = doc.node(cur)?;
        let parent = node.parent_id.as_deref()?;
        if doc.node(parent).is_some_and(|p| p.collapsed) {
            anchor = parent.to_string();
        }
        cur = parent;
    }
    None
}

/// 标签条横向溢出的边缘渐隐。`at_left` 时从 panel 色向透明收敛，否则反向；
/// 画在标签之上、只提示还有内容，不接管命中。
fn paint_tab_fade(ui: &egui::Ui, view: Rect, at_left: bool, strength: f32, panel: Color32) {
    let w = crate::tab_strip::EDGE_FADE_W.min(view.width());
    let solid = Color32::from_rgba_unmultiplied(
        panel.r(),
        panel.g(),
        panel.b(),
        (255.0 * strength.clamp(0.0, 1.0)) as u8,
    );
    let clear = Color32::from_rgba_unmultiplied(panel.r(), panel.g(), panel.b(), 0);
    let (rect, colors) = if at_left {
        (
            Rect::from_min_size(view.left_top(), Vec2::new(w, view.height())),
            [solid, clear],
        )
    } else {
        (
            Rect::from_min_size(
                Pos2::new(view.right() - w, view.top()),
                Vec2::new(w, view.height()),
            ),
            [clear, solid],
        )
    };
    ui.painter().add(egui::Shape::gradient_rect(
        rect,
        egui::epaint::Direction::LeftToRight,
        colors,
    ));
}

/// 「排列」子菜单（对齐 + 分布）：自由主题与概念共用同一套几何动作——
/// 两者都是 `parent_id = None` 的盒子，没有第二套语义。
///
/// 少于 2 个盒子时返回 `None`：菜单里不留按不动的条目。
fn arrange_submenu(canvas: &canvas::Canvas) -> Option<crate::menu::Entry<panels::context::Action>> {
    use panels::context::{Action as A, Item};
    let free = canvas.free_topic_count();
    if free < 2 {
        return None;
    }
    let mut arrange = vec![Item::section("对齐")];
    for (label, how) in [
        ("左对齐", mw_ui::arrange::Align::Left),
        ("水平居中", mw_ui::arrange::Align::CenterX),
        ("右对齐", mw_ui::arrange::Align::Right),
        ("上对齐", mw_ui::arrange::Align::Top),
        ("垂直居中", mw_ui::arrange::Align::CenterY),
        ("下对齐", mw_ui::arrange::Align::Bottom),
    ] {
        arrange.push(Item::row(label, "", A::AlignFree(how), true));
    }
    arrange.push(Item::section("分布"));
    for (label, axis) in [
        ("水平等距", mw_ui::arrange::Distribute::Horizontal),
        ("垂直等距", mw_ui::arrange::Distribute::Vertical),
    ] {
        arrange.push(Item::row(label, "", A::DistributeFree(axis), free >= 3));
    }
    Some(Item::submenu("排列", true, arrange))
}
