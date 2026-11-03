use super::*;

impl Inspector {
    pub fn new() -> Self {
        Self::default()
    }

    /// 验收入口：直接把「关联已有」的候选列表开在某个节点上。
    /// 只有 `MW_CARD_PICK` 演示会用它——真实路径是点「关联已有」按钮。
    pub(crate) fn open_card_picker(&mut self, node: &str) {
        self.card_pick = Some(node.to_string());
        self.tab = Tab::Note;
    }

    /// 验收入口：把检查器开在指定页（`MW_INSPECTOR_TAB=style|note|mark`）。
    /// 内容页这两条基线没有别的途径能拍到——分组折叠与页签都只有真实点击才会变。
    pub(crate) fn open_tab(&mut self, which: &str) {
        self.tab = match which {
            "note" | "content" => Tab::Note,
            "mark" => Tab::Mark,
            _ => Tab::Style,
        };
    }

    /// 验收入口：让某一格数值一进来就处于编辑态（`MW_STEPPER_EDIT=行键:草稿`）。
    /// 真实路径是「点一下数值格」或「在格上敲数字」，两条路进的是同一个 `stepper_edit`。
    pub(crate) fn seed_stepper_edit(&mut self, row: &str, draft: &str) {
        self.stepper_seed = Some((row.to_string(), draft.to_string()));
    }

    /// 同步缓存。`revision` 与选择指纹都没变时只是一次整数比较。
    pub fn sync(&mut self, doc: &Document, theme: &Theme, selected: &[String]) {
        // 换主题不改文档，所以这行必须在早退之前——配色得跟着画布走。
        self.ink = Ink::of(theme);
        // 色条画的是「生效主题」在各色相上的样子，不是用户挑的那支原色：
        // 覆盖只借色相，彩度与明度归主题自己，预览和画布才是同一个颜色。
        self.accent = theme.ui.accent;
        self.accent_chips = ACCENT_HUES
            .iter()
            .map(|h| theme.accent_at_hue(*h))
            .collect();
        if !self.accent_dirty {
            self.accent_draft = None;
        }
        let key = selection_key(selected);
        if self.rev == doc.revision && self.sel_key == key {
            return;
        }
        self.rev = doc.revision;
        self.sel_key = key;
        // 换选中/文档变了，正在敲的那一格就作废：草稿是给「刚才那颗节点」写的。
        self.stepper_edit = None;

        if selected.is_empty() {
            self.items.clear();
            self.sync_note();
            self.sync_link();
            self.label_for = None;
            self.label_draft.clear();
            if !self.layout_dirty {
                self.layout_draft = doc.meta.layout.clone();
                self.focus_draft = doc.meta.layout.focus_question.clone().unwrap_or_default();
            }
            return;
        }

        let styles = resolve_all(doc, theme);
        self.items.clear();
        for id in selected {
            let Some(n) = doc.node(id) else { continue };
            let Some(resolved) = styles.get(id).copied() else {
                continue;
            };
            self.items.push(ItemView {
                id: n.id.clone(),
                text: n.text.clone(),
                style: n.style.clone().unwrap_or_default(),
                resolved,
                note: n.note.clone(),
                link: n.link.clone(),
                icon: n.icon.clone(),
                labels: n.labels.clone(),
                task: n.task,
                image: n.image.clone(),
                subtree: subtree_styles(doc, &n.id),
            });
        }
        self.sync_note();
        self.sync_link();
        let primary = self.items.last().map(|i| i.id.clone());
        if self.label_for != primary {
            self.label_for = primary;
            self.label_draft.clear();
        }
        if !self.layout_dirty {
            self.layout_draft = doc.meta.layout.clone();
            self.focus_draft = doc.meta.layout.focus_question.clone().unwrap_or_default();
        }
    }

    /// 备注草稿跟随主选。用户正在输入（草稿 ≠ 上次已知值）时不覆盖；
    /// 提交后文档值恰好等于草稿，则把它当成"已落盘"更新基准。
    /// 备注草稿跟随主选。用户正在输入（草稿 ≠ 上次已知值）时不覆盖；
    /// 提交后文档值恰好等于草稿，则把它当成"已落盘"更新基准。
    pub(crate) fn sync_note(&mut self) {
        let id = self.items.last().map(|i| i.id.clone());
        let note = self
            .items
            .last()
            .and_then(|i| i.note.clone())
            .unwrap_or_default();
        if id.is_none() {
            self.note_id = None;
            self.note_draft.clear();
            self.note_before.clear();
            self.note_expanded = false;
            return;
        }
        if self.note_id != id {
            self.note_id = id;
            self.note_draft = note.clone();
            self.note_before = note;
            // 「展开」是这段长文的临时视图，不属于节点：换节点就回到折叠态（§4.2 的 3 行起）。
            self.note_expanded = false;
        } else if self.note_draft == self.note_before || self.note_draft == note {
            self.note_draft = note.clone();
            self.note_before = note;
        }
    }

    /// 大纲点备注点后：切到「内容」页，下一帧把键盘焦点交给备注框。
    pub(crate) fn focus_note(&mut self) {
        self.tab = Tab::Note;
        self.note_focus_pending = true;
    }

    pub(crate) fn sync_link(&mut self) {
        let id = self.items.last().map(|i| i.id.clone());
        let link = self
            .items
            .last()
            .and_then(|i| i.link.clone())
            .unwrap_or_default();
        if id.is_none() {
            self.link_id = None;
            self.link_draft.clear();
            self.link_before.clear();
            return;
        }
        if self.link_id != id {
            self.link_id = id;
            self.link_draft = link.clone();
            self.link_before = link;
        } else if self.link_draft == self.link_before || self.link_draft == link {
            self.link_draft = link.clone();
            self.link_before = link;
        }
    }

    /// 画面板并读输入。返回本帧最多一个动作。
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        theme: &Theme,
        selected: &[String],
        focus: &mut FocusMemory,
        extras: Extras,
    ) -> Option<Action> {
        let presets = extras.presets;
        let annotation = extras.annotation;
        // 验收入口（`MW_STEPPER_EDIT`）：把某一格数值摆成编辑态。真实路径是先点一下，
        // 离屏 harness 默认每帧清指针、拍不到那一帧，所以由种子每帧补齐（`sync` 会清它）。
        if let Some((row, draft)) = self.stepper_seed.clone() {
            if self.stepper_edit.is_none() {
                self.stepper_edit = Some(StepperEdit {
                    row,
                    draft,
                    fresh: false,
                    pinned: true,
                });
            }
        }
        if self.groups != extras.groups {
            self.groups = extras.groups.to_vec();
        }
        focus.begin_frame(PanelId::Inspector);
        // 自定义控件（色点/分段/图标格）不走 egui 的默认按钮导航：这里补方向键与 Esc。
        // 文本框聚焦时让路——方向键要在光标里移动，Esc 由文本框自己处理。
        let custom_focus = focus
            .last(PanelId::Inspector)
            .is_some_and(|k| ui.ctx().memory(|m| m.focused()) == Some(k.id()));
        if custom_focus {
            // 又回到控件上了，Esc 的两级计数重新开始。
            self.esc_latch = false;
        }
        // §4.4：Esc 分两级——先松掉控件焦点，再按一次才把键盘交给画布。
        // 一次退到底会让「按错了」变成「面板也没了」。
        if (custom_focus || self.esc_latch)
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            match esc_step(custom_focus, self.esc_latch) {
                EscStep::Release => {
                    if let Some(held) = ui.ctx().memory(|m| m.focused()) {
                        ui.memory_mut(|m| m.surrender_focus(held));
                    }
                    self.esc_latch = true;
                    return None;
                }
                EscStep::Exit => {
                    self.esc_latch = false;
                    return Some(Action::FocusCanvas);
                }
                EscStep::Ignore => {}
            }
        }
        if custom_focus && !ui.ctx().egui_wants_keyboard_input() {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                focus.step_focus(PanelId::Inspector, ui.ctx(), 1);
                return None;
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                focus.step_focus(PanelId::Inspector, ui.ctx(), -1);
                return None;
            }
        }
        self.sync(doc, theme, selected);
        // 盖住当前节点的 + 它自己持有的注解——每帧现算，只扫父/自身两个列表。
        let anns = self
            .items
            .last()
            .map(|i| {
                let mut v = doc.annotations_covering(&i.id);
                v.extend(doc.annotations_owned(&i.id));
                v
            })
            .unwrap_or_default();
        // 每个注解的跨度展开（概要才用得上）：在 show 里算好，mark_body 只画。
        let spans: Vec<Vec<(String, bool, String)>> = anns
            .iter()
            .map(|a| ann_span(doc, &a.parent, a.kind, &a.id))
            .collect();
        let ink = self.ink;
        let mut action = None;

        // 头行 40（§4.1）：左侧 13/19 semibold secondary「检查器」，右侧 24×24 关闭按钮。
        let w = ui.available_width();
        let (head, _) = ui.allocate_exact_size(Vec2::new(w, geo::HEAD_H), Sense::hover());
        ui.painter().text(
            Pos2::new(head.left() + GUTTER, head.center().y),
            Align2::LEFT_CENTER,
            "检查器",
            FontId::new(
                geo::HEAD_TEXT,
                FontFamily::Name(crate::fonts::SEMIBOLD.into()),
            ),
            ink.ch.dim,
        );
        let close_rect = Rect::from_center_size(
            Pos2::new(
                head.right() - GUTTER - geo::CLOSE_BTN * 0.5,
                head.center().y,
            ),
            Vec2::splat(geo::CLOSE_BTN),
        );
        let close_resp = ui.interact(
            close_rect,
            ui.scope_id().with("inspector/close"),
            Sense::click(),
        );
        {
            let p = ui.painter();
            if close_resp.hovered() {
                p.rect_filled(close_rect, 5.0, ink.ch.hover);
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let c = close_rect.center();
            let d = 3.6;
            let stroke = Stroke::new(
                1.4,
                if close_resp.hovered() {
                    ink.ch.text
                } else {
                    ink.ch.dim
                },
            );
            p.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], stroke);
            p.line_segment([c + Vec2::new(d, -d), c + Vec2::new(-d, d)], stroke);
        }
        if close_resp.clicked() {
            action = Some(Action::Close);
        }
        close_resp.on_hover_text("关闭（F9）");

        // 对象行 44（§4.1）：标题 14/20 省略 + 副行 12/18 secondary。
        // 多选时副行固定说「N 个节点」——那才是这一刻真正需要知道的事。
        if let Some(item) = self.items.last() {
            let (obj, _) = ui.allocate_exact_size(Vec2::new(w, geo::OBJ_H), Sense::hover());
            let title_font = FontId::proportional(geo::OBJ_TITLE);
            let meta_font = FontId::proportional(geo::OBJ_META);
            let max_w = (obj.width() - GUTTER * 2.0).max(24.0);
            let raw = if item.text.trim().is_empty() {
                "（未命名）".to_string()
            } else {
                item.text.trim().to_string()
            };
            let title = ellipsize(ui, &raw, title_font.clone(), max_w);
            let meta = if self.items.len() > 1 {
                if doc.is_concept_map() {
                    let total = self.items.len();
                    let pinned_count = self
                        .items
                        .iter()
                        .filter(|it| doc.is_concept_pinned(&it.id))
                        .count();
                    if pinned_count == total {
                        format!("{total} 个概念 · 全部已钉住")
                    } else if pinned_count > 0 {
                        format!("{total} 个概念 · {pinned_count} 个已钉住")
                    } else {
                        format!("{total} 个概念")
                    }
                } else {
                    format!("{} 个节点", self.items.len())
                }
            } else {
                object_meta(doc, &item.id)
            };
            let p = ui.painter();
            p.text(
                Pos2::new(obj.left() + GUTTER, obj.top() + 4.0),
                Align2::LEFT_TOP,
                title,
                title_font,
                ink.ch.text,
            );
            p.text(
                Pos2::new(obj.left() + GUTTER, obj.top() + 24.0),
                Align2::LEFT_TOP,
                meta,
                meta_font,
                ink.ch.dim,
            );
        }
        ui.add_space(4.0);
        ui.separator();

        // 关系线优先于注解/节点：画布上一次只能选中一种。
        // 多选命题走批量页（CM-17）：只给能批量做的事。
        if extras.relationships.len() > 1 {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(a) = self.relationships_body(ui, doc, extras.relationships) {
                        action = Some(a);
                    }
                });
            return action;
        }
        if let Some(rel) = extras.relationship {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(a) = self.relationship_body(ui, doc, rel) {
                        action = Some(a);
                    }
                });
            return action;
        }

        // 画布上选中了边界/概要：检查器切成注解页（与节点选择互斥）。
        if annotation.is_some() {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(a) = self.annotation_body(ui, doc, extras.annotations) {
                        action = Some(a);
                    }
                });
            return action;
        }

        if self.items.is_empty() {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(a) = self.doc_body(ui, doc, focus, presets) {
                        action = Some(a);
                    }
                });
            return action;
        }

        ui.add_space(6.0);
        // 概念图只走**专属那一条**（`docs/33` §6）：样式/内容/标记三页带着树语义
        // （含子树、层级样式、任务状态），进概念图只会让人一头雾水。
        if doc.is_concept_map() {
            self.tab = Tab::Concept;
        }
        self.tabs(ui, focus);
        ui.add_space(6.0);
        self.error_bar(ui);
        // 组内行间距 6（`docs/30` §4.1）：`group_list` 进来先把全局 8 压成 6。
        group_list(ui, |ui| {
            let a = match self.tab {
                Tab::Style => self.style_body(ui, theme, focus, doc, extras.boxes),
                Tab::Note => self.note_body(ui, focus, doc),
                Tab::Mark => self.mark_body(ui, focus, &anns, &spans),
                Tab::Concept => self.concept_body(ui, focus, doc, selected, extras, theme),
            };
            if a.is_some() {
                action = a;
            }
        });
        action
    }

    /// 命令失败时贴一条（`docs/30` §4.3）。壳层在 `apply` 返回 false 时调用。
    pub(crate) fn report_error(&mut self, msg: &str) {
        if self.error.as_deref() != Some(msg) {
            crate::trace::event(&format!("inspector_error shown=1 text={msg}"));
        }
        self.error = Some(msg.to_string());
    }

    /// 改成功了就把错误条撤掉——它不是常驻表单的一部分。
    pub(crate) fn clear_error(&mut self) {
        self.error = None;
    }

    /// 面板内的错误条：28 高、warn 色 8% 底、左侧一个警示点、右侧 14px 关掉。
    fn error_bar(&mut self, ui: &mut egui::Ui) {
        let Some(msg) = self.error.clone() else {
            return;
        };
        let ink = self.ink;
        let w = ui.available_width();
        let (bar, _) = ui.allocate_exact_size(Vec2::new(w, geo::ERROR_H), Sense::hover());
        let r = bar.shrink2(Vec2::new(GUTTER, 0.0));
        ui.painter()
            .rect_filled(r, 5.0, ink.ch.warn.gamma_multiply(0.10));
        let dot = Pos2::new(r.left() + 10.0, r.center().y);
        ui.painter().circle_filled(dot, 3.0, ink.ch.warn);
        let text_x = dot.x + 9.0;
        let font = FontId::proportional(geo::ROW_TEXT);
        let max_w = (r.right() - 22.0 - text_x).max(20.0);
        let shown = ellipsize(ui, &msg, font.clone(), max_w);
        ui.painter().text(
            Pos2::new(text_x, r.center().y),
            Align2::LEFT_CENTER,
            shown,
            font,
            ink.ch.warn,
        );
        let close =
            Rect::from_center_size(Pos2::new(r.right() - 10.0, r.center().y), Vec2::splat(14.0));
        let resp = ui.interact(
            close,
            ui.scope_id().with("inspector/error/close"),
            Sense::click(),
        );
        let c = close.center();
        let d = 3.0;
        let stroke = Stroke::new(
            1.3,
            if resp.hovered() {
                ink.ch.text
            } else {
                ink.ch.warn
            },
        );
        ui.painter()
            .line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], stroke);
        ui.painter()
            .line_segment([c + Vec2::new(d, -d), c + Vec2::new(-d, d)], stroke);
        if resp.clicked() {
            self.error = None;
        }
        ui.add_space(4.0);
    }

    pub(crate) fn tabs(&mut self, ui: &mut egui::Ui, focus: &mut FocusMemory) {
        let ink = self.ink;
        // 概念图只有**一段**（`docs/33` §6）：不需要切页，也就不出分段控件。
        // 三段那套留给思维导图——它的「样式/内容/标记」是树语义的。
        if self.tab == Tab::Concept {
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.label(
                    egui::RichText::new("概念图")
                        .size(geo::SEG_TEXT)
                        .color(ink.ch.dim),
                );
                ui.add_space(GUTTER);
            });
            return;
        }
        // 三段等高铺满版心（§4.2）：30 高、无描边，选中靠 8% 底 + 底部 1.5px 强调条。
        let w = ((ui.available_width() - GUTTER * 2.0) / 3.0).max(40.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.spacing_mut().item_spacing.x = 0.0;
            for (tab, label, key) in [
                (Tab::Style, "样式", FocusKey::new("inspector/tab/style")),
                (Tab::Note, "内容", FocusKey::new("inspector/tab/note")),
                (Tab::Mark, "标记", FocusKey::new("inspector/tab/mark")),
            ] {
                if segment(
                    ui,
                    label,
                    self.tab == tab,
                    Vec2::new(w, geo::SEG_H),
                    ink,
                    &key,
                    focus,
                ) {
                    self.tab = tab;
                }
            }
            ui.add_space(GUTTER);
        });
    }

    // ───────────────────────── 样式 ─────────────────────────

    pub(crate) fn style_body(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        focus: &mut FocusMemory,
        doc: &Document,
        boxes: &[Option<Rect>],
    ) -> Option<Action> {
        let mut action = None;
        let ink = self.ink;
        ui.add_space(2.0);

        // 恢复默认：把选中节点（或子树）的 StyleOverride 整份清空，一次撤销。
        if self
            .items
            .iter()
            .any(|i| i.style != StyleOverride::default())
        {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(egui::RichText::new("恢复默认样式").size(11.5))
                    .on_hover_text("清除选中对象的全部样式覆盖")
                    .clicked()
                {
                    action = self.style_cmd(|s| *s = StyleOverride::default());
                }
            });
        }

        // 作用域（§4.2「作用域」）：**分组标题右侧**两段「本节点 / 含子树」，不再单独占一行。
        // 它只对样式类属性生效——颜色/字号/边框/形状都走 `style_cmd`，备注/标签/链接不吃。
        // 概念图无父子树层级，隐藏作用域切换。
        let (open_color, act) = if doc.is_concept_map() {
            self.group(ui, focus, "颜色", "填充 · 分支 · 文字", "style/color", true)
        } else {
            let (open, act, scope) = self.group_scoped(
                ui,
                focus,
                "颜色",
                "填充 · 分支 · 文字",
                "style/color",
                true,
                self.style_subtree,
            );
            if let Some(s) = scope {
                self.style_subtree = s;
            }
            (open, act)
        };
        if let Some(a) = act {
            action = Some(a);
        }
        if open_color {
            // 填充
            let fill = same(self.items.iter().map(|i| i.resolved.fill));
            if self.row_label(
                ui,
                "填充",
                self.items.iter().any(|i| i.style.fill.is_some()),
                ink,
            ) {
                action = self.style_cmd(|s| s.fill = None);
            } else {
                let mut current = fill;
                let inheriting = !self.items.iter().any(|i| i.style.fill.is_some());
                let pick = swatches(
                    ink,
                    ui,
                    theme,
                    &FocusKey::new(ROW_FILL),
                    SwatchRow {
                        current: &mut current,
                        fallback: theme.ui.panel,
                        tip: "自定义填充色…",
                        inheriting,
                    },
                    focus,
                );
                match pick {
                    Some(SwatchPick::Color(c)) => {
                        action = self.style_cmd(|s| s.fill = Some(hex_of(c)));
                    }
                    Some(SwatchPick::None) => action = self.style_cmd(|s| s.fill = None),
                    None => {}
                }
            }
            ui.add_space(6.0);

            // 分支线：连到本节点的线、子树连线与二三级卡片染色都跟它走（沿子树继承）。
            let branch = same(self.items.iter().map(|i| i.resolved.branch_color));
            if self.row_label(
                ui,
                "分支线",
                self.items.iter().any(|i| i.style.branch_color.is_some()),
                ink,
            ) {
                action = self.style_cmd(|s| s.branch_color = None);
            } else {
                let mut current = branch;
                let inheriting = !self.items.iter().any(|i| i.style.branch_color.is_some());
                let pick = swatches(
                    ink,
                    ui,
                    theme,
                    &FocusKey::new(ROW_BRANCH),
                    SwatchRow {
                        current: &mut current,
                        fallback: branch.unwrap_or(theme.ui.accent),
                        tip: "自定义分支线色…",
                        inheriting,
                    },
                    focus,
                );
                match pick {
                    Some(SwatchPick::Color(c)) => {
                        action = self.style_cmd(|s| s.branch_color = Some(hex_of(c)));
                    }
                    Some(SwatchPick::None) => action = self.style_cmd(|s| s.branch_color = None),
                    None => {}
                }
            }
            ui.add_space(6.0);

            // 文字颜色
            let text = same(self.items.iter().map(|i| i.resolved.text));
            if self.row_label(
                ui,
                "文字",
                self.items.iter().any(|i| i.style.text_color.is_some()),
                ink,
            ) {
                action = self.style_cmd(|s| {
                    s.text_color = None;
                    s.apply_text_to_subtree = None;
                });
            } else {
                let mut current = text;
                let inheriting = !self.items.iter().any(|i| i.style.text_color.is_some());
                let pick = swatches(
                    ink,
                    ui,
                    theme,
                    &FocusKey::new(ROW_TEXT),
                    SwatchRow {
                        current: &mut current,
                        fallback: theme.ui.text,
                        tip: "自定义文字色…",
                        inheriting,
                    },
                    focus,
                );
                match pick {
                    Some(SwatchPick::Color(c)) => {
                        action = self.style_cmd(|s| s.text_color = Some(hex_of(c)));
                    }
                    Some(SwatchPick::None) => {
                        action = self.style_cmd(|s| {
                            s.text_color = None;
                            s.apply_text_to_subtree = None;
                        });
                    }
                    None => {}
                }
            }
            if !doc.is_concept_map() {
                let mut subtree = self
                    .items
                    .iter()
                    .all(|i| i.style.apply_text_to_subtree == Some(true));
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    let changed = crate::ui_kit::switch(
                        ui,
                        ink.ch,
                        "inspector/text/subtree",
                        &mut subtree,
                        true,
                    )
                    .changed();
                    ui.label(
                        egui::RichText::new("文字样式应用到子树")
                            .size(Type::SMALL)
                            .color(ink.ch.text),
                    );
                    if changed {
                        action = self.style_cmd(|s| {
                            s.apply_text_to_subtree = if subtree { Some(true) } else { None };
                        });
                    }
                });
                ui.add_space(6.0);
            }
        }
        if doc.is_concept_map() {
            let is_multi = self.items.len() > 1;
            let all_pinned = self.items.iter().all(|i| doc.is_concept_pinned(&i.id));
            let any_pinned = self.items.iter().any(|i| doc.is_concept_pinned(&i.id));
            let summary = if all_pinned {
                "已钉住"
            } else if any_pinned {
                "部分钉住"
            } else {
                "自动重排"
            };
            let (open_layout, act) = self.group_with(
                ui,
                focus,
                "概念排布",
                summary,
                "concept/layout",
                false,
                true,
            );
            if let Some(a) = act {
                action = Some(a);
            }
            if open_layout {
                let mut pinned = all_pinned;
                let switch_label = if is_multi {
                    "钉住所选概念"
                } else {
                    "钉住位置"
                };
                let w = (ui.available_width() - GUTTER * 2.0).max(100.0);
                let changed = ui
                    .horizontal(|ui| {
                        ui.add_space(GUTTER);
                        ui.allocate_ui_with_layout(
                            Vec2::new(w, geo::ROW_H),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                crate::ui_kit::switch_row(
                                    ui,
                                    ink.ch,
                                    "inspector/concept/pinned",
                                    &mut pinned,
                                    switch_label,
                                )
                            },
                        )
                        .inner
                    })
                    .inner
                    .changed();
                if changed {
                    crate::trace::event(&format!(
                        "inspector_concept_pinned n={} pinned={}",
                        self.items.len(),
                        pinned
                    ));
                    if is_multi {
                        let cmds: Vec<Command> = self
                            .items
                            .iter()
                            .map(|i| Command::SetConceptPinned {
                                id: i.id.clone(),
                                pinned,
                            })
                            .collect();
                        action = wrap(cmds);
                    } else if let Some(i) = self.items.first() {
                        action = Some(Action::Apply(Command::SetConceptPinned {
                            id: i.id.clone(),
                            pinned,
                        }));
                    }
                }
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.label(
                        egui::RichText::new("钉住后整理布局保持原位，仅重排自由概念。")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    if is_multi {
                        if crate::ui_kit::text_button(
                            ui,
                            ink.ch,
                            "整理选中概念",
                            crate::ui_kit::ButtonTone::Secondary,
                            true,
                        )
                        .on_hover_text("仅重新排布选中的概念（已钉住的保持原位）")
                        .clicked()
                        {
                            action = Some(Action::ArrangeSelectedConcepts);
                        }
                        ui.add_space(4.0);
                        if crate::ui_kit::text_button(
                            ui,
                            ink.ch,
                            "整理全部布局",
                            crate::ui_kit::ButtonTone::Ghost,
                            true,
                        )
                        .on_hover_text("重新排布所有概念（已钉住的保持原位）")
                        .clicked()
                        {
                            action = Some(Action::ArrangeConcepts);
                        }
                    } else {
                        if crate::ui_kit::text_button(
                            ui,
                            ink.ch,
                            "整理全部布局",
                            crate::ui_kit::ButtonTone::Secondary,
                            true,
                        )
                        .on_hover_text("重新排布所有概念（已钉住的保持原位）")
                        .clicked()
                        {
                            action = Some(Action::ArrangeConcepts);
                        }
                    }
                });
                ui.add_space(6.0);
            }
        }
        let (open_shape, act) = if doc.is_concept_map() {
            self.group_with(
                ui,
                focus,
                "形状与文字",
                "轮廓 · 字号 · 字重",
                "style/shape",
                false,
                false,
            )
        } else {
            let (open, act, scope) = self.group_scoped(
                ui,
                focus,
                "形状与文字",
                "轮廓 · 字号 · 字重",
                "style/shape",
                false,
                self.style_subtree,
            );
            if let Some(s) = scope {
                self.style_subtree = s;
            }
            (open, act)
        };
        if let Some(a) = act {
            action = Some(a);
        }
        if open_shape {
            // 边框颜色（§4.2：与填充同一套色板网格；斜杠格 = 回到主题描边）。
            let edge = same(self.items.iter().map(|i| i.resolved.stroke));
            if self.row_label(
                ui,
                "边框",
                self.items.iter().any(|i| i.style.stroke.is_some()),
                ink,
            ) {
                action = self.style_cmd(|s| s.stroke = None);
            } else {
                let mut current = edge;
                let inheriting = !self.items.iter().any(|i| i.style.stroke.is_some());
                let pick = swatches(
                    ink,
                    ui,
                    theme,
                    &FocusKey::new(ROW_STROKE),
                    SwatchRow {
                        current: &mut current,
                        fallback: theme.ui.border,
                        tip: "自定义边框色…",
                        inheriting,
                    },
                    focus,
                );
                match pick {
                    Some(SwatchPick::Color(c)) => {
                        action = self.style_cmd(|s| s.stroke = Some(hex_of(c)));
                    }
                    Some(SwatchPick::None) => action = self.style_cmd(|s| s.stroke = None),
                    None => {}
                }
            }
            ui.add_space(6.0);

            // 边框宽度 0–8（§4.2 stepper 24×72）。
            let width = same(self.items.iter().map(|i| i.resolved.stroke_width));
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.label(
                    egui::RichText::new("边宽")
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
                let base = width.unwrap_or(0.0);
                let shown = format!("{}", base.round() as i32);
                if let Some(w) = stepper(
                    ui,
                    ink,
                    &FocusKey::new("inspector/style/stroke-width"),
                    StepperRow {
                        text: &shown,
                        value: base,
                        range: (0.0, 8.0),
                        step: 1.0,
                    },
                    focus,
                    &mut self.stepper_edit,
                ) {
                    action = self.style_cmd(|s| s.stroke_width = Some(w));
                }
                if self.items.iter().any(|i| i.style.stroke_width.is_some())
                    && crate::ui_kit::icon_button(
                        ui,
                        ink.ch,
                        18.0,
                        crate::icons::Icon::Undo,
                        "恢复主题边宽",
                    )
                {
                    action = self.style_cmd(|s| s.stroke_width = None);
                }
            });
            ui.add_space(6.0);

            // 形状
            let shape = same(self.items.iter().map(|i| i.resolved.shape));
            if self.row_label(
                ui,
                "形状",
                self.items.iter().any(|i| i.style.shape.is_some()),
                ink,
            ) {
                action = self.style_cmd(|s| s.shape = None);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.spacing_mut().item_spacing.x = 4.0;
                // 四段按可用宽度均分，面板拖到 240 也不顶宽（§4.1 允许 240–360）。
                // 第四段是**椭圆**（SE-13 / §4.2「矩形｜椭圆｜圆角矩形」）：下划线搬去
                // B / I 同一行当 U 开关——它是「只有一条底线」的形态，与 B/I 同类，
                // 挤在形状行里会既不是形状也放不进 240 面板。
                let seg_w = shape_seg_w(ui.available_width());
                for (s, label) in [
                    (Shape::Rounded, "圆角"),
                    (Shape::Rect, "直角"),
                    (Shape::Capsule, "胶囊"),
                    (Shape::Ellipse, "椭圆"),
                ] {
                    let key = FocusKey::dynamic("inspector/style/shape", s.as_str());
                    if seg(
                        ui,
                        label,
                        shape == Some(s),
                        Vec2::new(seg_w, 26.0),
                        ink,
                        Some(&key),
                        focus,
                    ) {
                        action = self.style_cmd(|o| o.shape = Some(s.as_str().to_string()));
                    }
                }
            });
            ui.add_space(6.0);

            // 圆角 0–24（§4.2 stepper 24×72）。只对圆角矩形/直角有意义：
            // 胶囊取高一半、椭圆走哨兵半径、下划线没有角，那三种形态下不显示这一行，
            // 免得给出一个改了看不出效果的控件。
            let roundable = matches!(shape, None | Some(Shape::Rounded) | Some(Shape::Rect));
            if roundable {
                // 没覆盖时显示形状自带的默认值（圆角矩形 = 令牌 MD、直角 = 0）；
                // 这里不持有节点矩形，所以不复用 `corner()`——胶囊/椭圆已经被
                // `roundable` 挡在外面，剩下这两种的默认值就是常数。
                //
                // 上限跟着**实际盒子**走（V1-02 G-4）：渲染层把半径钳到 `min(w,h)/2`，
                // 盒子 36 高时 24 与 18 画出来一模一样。控件这里也压到同一个上限，
                // 并把已存但画不出来的值显示成实际生效的那个——控件与像素说同一句话。
                let ceiling = radius_ceiling(boxes);
                let radius = same(self.items.iter().map(|i| {
                    i.resolved
                        .radius
                        .unwrap_or(match i.resolved.shape {
                            Shape::Rect => 0.0,
                            _ => mw_core::design::radius::MD,
                        })
                        .min(ceiling)
                }));
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.label(
                        egui::RichText::new("圆角")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                    let label = match radius {
                        Some(v) => format!("{}", v.round() as i32),
                        None => "混合".to_string(),
                    };
                    let base = radius.unwrap_or(mw_core::design::radius::MD.min(ceiling));
                    if let Some(v) = stepper(
                        ui,
                        ink,
                        &FocusKey::new("inspector/style/radius"),
                        StepperRow {
                            text: &label,
                            value: base,
                            range: (0.0, ceiling),
                            step: 2.0,
                        },
                        focus,
                        &mut self.stepper_edit,
                    ) {
                        action = self.style_cmd(|o| o.radius = Some(v.round()));
                    }
                    if self.items.iter().any(|i| i.style.radius.is_some())
                        && crate::ui_kit::icon_button(
                            ui,
                            ink.ch,
                            18.0,
                            crate::icons::Icon::Undo,
                            "恢复默认圆角",
                        )
                    {
                        action = self.style_cmd(|o| o.radius = None);
                    }
                });
                ui.add_space(6.0);
            }

            // 字号
            let size = same(self.items.iter().map(|i| i.resolved.font_size));
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.label(
                    egui::RichText::new("字号")
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
                let label = match size {
                    Some(v) => format!("{}", v.round() as i32),
                    None => "混合".to_string(),
                };
                if let Some(v) = stepper(
                    ui,
                    ink,
                    &FocusKey::new("inspector/style/size"),
                    StepperRow {
                        text: &label,
                        value: size.unwrap_or(16.0),
                        range: (12.0, 96.0),
                        step: 2.0,
                    },
                    focus,
                    &mut self.stepper_edit,
                ) {
                    action = self.style_cmd(|s| s.font_size = Some(v.round() as u16));
                }
                if size.is_some()
                    && self.items.iter().any(|i| i.style.font_size.is_some())
                    && crate::ui_kit::icon_button(
                        ui,
                        ink.ch,
                        18.0,
                        crate::icons::Icon::Undo,
                        "恢复主题字号",
                    )
                {
                    action = self.style_cmd(|s| s.font_size = None);
                }
            });
            ui.add_space(6.0);

            // 字重三段 400 / 500 / 600（§4.2「字重 400/500/600 三段」）。
            // 数值就是字面本体：不译成「常规/中等/半粗」，免得与 B 开关的二值语义打架。
            let weight = same(self.items.iter().map(|i| i.resolved.weight));
            if self.row_label(
                ui,
                "字重",
                self.items.iter().any(|i| i.style.weight.is_some()),
                ink,
            ) {
                action = self.style_cmd(|s| s.weight = None);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.spacing_mut().item_spacing.x = 4.0;
                let seg_w = text_seg_w(ui.available_width());
                for w in [
                    mw_core::theme::WEIGHT_REGULAR,
                    mw_core::theme::WEIGHT_MEDIUM,
                    mw_core::theme::WEIGHT_SEMIBOLD,
                ] {
                    let label = format!("{w}");
                    let key = FocusKey::dynamic("inspector/style/weight", &label);
                    if seg(
                        ui,
                        &label,
                        weight == Some(w),
                        Vec2::new(seg_w, 26.0),
                        ink,
                        Some(&key),
                        focus,
                    ) {
                        action = self.style_cmd(|s| s.weight = Some(w));
                    }
                }
            });
            ui.add_space(6.0);

            // 粗体 / 斜体 / 下划线
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                let bold = same(self.items.iter().map(|i| i.resolved.bold));
                let italic = same(self.items.iter().map(|i| i.resolved.italic));
                if toggle(
                    ui,
                    "B",
                    bold,
                    true,
                    ink,
                    &FocusKey::new("inspector/style/bold"),
                    focus,
                ) {
                    // B 是字重的二值快捷方式（SE-13）：开 = 600、关 = 400。
                    // 旧 `bold` 键只读不写——写它会在 weight 覆盖之下一层被静默忽略，
                    // 用户会看到「B 按了没反应」。
                    let want = if bold == Some(true) {
                        mw_core::theme::WEIGHT_REGULAR
                    } else {
                        mw_core::theme::WEIGHT_SEMIBOLD
                    };
                    action = self.style_cmd(|s| {
                        s.weight = Some(want);
                        s.bold = None;
                    });
                }
                if toggle(
                    ui,
                    "I",
                    italic,
                    false,
                    ink,
                    &FocusKey::new("inspector/style/italic"),
                    focus,
                ) {
                    let want = italic != Some(true);
                    action = self.style_cmd(|s| s.italic = Some(want));
                }
                // U = 下划线形态（`Shape::Underline`）。它原本挤在形状行第四段，
                // 但形状行要让位给椭圆，而下划线本来就是「只有一条底线」的文字形态，
                // 放在 B / I 旁边语义更顺（SE-13）。
                let underline = shape == Some(Shape::Underline);
                if toggle(
                    ui,
                    "U",
                    Some(underline),
                    false,
                    ink,
                    &FocusKey::new("inspector/style/underline"),
                    focus,
                ) {
                    action = if underline {
                        self.style_cmd(|s| s.shape = None)
                    } else {
                        self.style_cmd(|s| s.shape = Some(Shape::Underline.as_str().to_string()))
                    };
                }
                if self
                    .items
                    .iter()
                    .any(|i| i.style.bold.is_some() || i.style.italic.is_some())
                    && crate::ui_kit::icon_button(
                        ui,
                        ink.ch,
                        18.0,
                        crate::icons::Icon::Undo,
                        "恢复主题字重",
                    )
                {
                    action = self.style_cmd(|s| {
                        s.bold = None;
                        s.italic = None;
                    });
                }
            });
            ui.add_space(6.0);

            // 文本对齐 左 / 中 / 右（§4.2）。**概念图不出这一行**：`docs/30` §5.3 的合同是
            // 「概念居中」（`center_text`），而渲染层是「显式 `align` 优先于 `center_text`」——
            // 两个口径只能选一个，这里按合同做：概念图不给入口，于是不会有显式覆盖。
            // `same` 套在 `Option<TextAlign>` 上，所以外面再包一层：
            // `None` = 混合、`Some(None)` = 全部继承、`Some(Some(a))` = 全部这一档。
            if !doc.is_concept_map() {
                let align = same(self.items.iter().map(|i| i.resolved.align));
                if self.row_label(
                    ui,
                    "对齐",
                    self.items.iter().any(|i| i.style.align.is_some()),
                    ink,
                ) {
                    action = self.style_cmd(|s| s.align = None);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let seg_w = text_seg_w(ui.available_width());
                    for (a, label) in [
                        (mw_core::theme::TextAlign::Left, "左"),
                        (mw_core::theme::TextAlign::Center, "中"),
                        (mw_core::theme::TextAlign::Right, "右"),
                    ] {
                        let key = FocusKey::dynamic("inspector/style/align", label);
                        if seg(
                            ui,
                            label,
                            align == Some(Some(a)),
                            Vec2::new(seg_w, 26.0),
                            ink,
                            Some(&key),
                            focus,
                        ) {
                            action = self.style_cmd(|s| s.align = Some(a.as_str().to_string()));
                        }
                    }
                });
            }
        }
        action
    }

    /// 样式区的小标题 + "已覆盖"圆点 + 一键恢复。返回是否点了恢复。
    pub(crate) fn row_label(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        overridden: bool,
        ink: Ink,
    ) -> bool {
        let mut reset = false;
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            if !label.is_empty() {
                ui.label(
                    egui::RichText::new(label)
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
            }
            if overridden {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(9.0, 14.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 2.5, ink.ch.accent);
                resp.on_hover_text("已覆盖（↺ 恢复继承）");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(GUTTER);
                if overridden
                    && crate::ui_kit::icon_button(
                        ui,
                        ink.ch,
                        18.0,
                        crate::icons::Icon::Undo,
                        "恢复继承",
                    )
                {
                    reset = true;
                }
            });
        });
        reset
    }

    /// 把同一改动按节点合并进各自的覆盖，批量下发（一条撤销）。
    pub(crate) fn style_cmd(&self, edit: impl Fn(&mut StyleOverride)) -> Option<Action> {
        let mut cmds = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for item in &self.items {
            let targets: Vec<(String, StyleOverride)> = if self.style_subtree {
                item.subtree.clone()
            } else {
                vec![(item.id.clone(), item.style.clone())]
            };
            for (id, base) in targets {
                if seen.iter().any(|x| x == &id) {
                    continue;
                }
                seen.push(id.clone());
                let before = base.clone();
                let mut s = base;
                edit(&mut s);
                if s == before {
                    continue;
                }
                let style = if s == StyleOverride::default() {
                    None
                } else {
                    Some(s)
                };
                cmds.push(Command::SetStyle {
                    ids: vec![id],
                    style,
                });
            }
        }
        wrap(cmds)
    }

    // ───────────────────────── 备注 / 链接 ─────────────────────────

    pub(crate) fn note_body(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        doc: &Document,
    ) -> Option<Action> {
        if self.items.len() > 1 {
            self.multi_hint(ui, "备注只能对单个主题编辑。");
            return None;
        }
        let mut action = None;
        let ink = self.ink;
        // 备注框同样纳入焦点记忆：焦点在文本框里时关开面板，也要回到文本框。
        let note_key = FocusKey::new("inspector/note/field");
        let note_id = focus::widget_id(ui, &note_key);
        focus.note(PanelId::Inspector, ui.ctx(), &note_key);
        if std::mem::take(&mut self.note_focus_pending) {
            ui.memory_mut(|m| m.request_focus(note_id));
            ui.ctx().request_repaint();
        }
        let focused = ui.memory(|m| m.has_focus(note_id));
        if focused {
            if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)) {
                return self.commit_note();
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.note_draft = self.note_before.clone();
            }
        }

        ui.add_space(2.0);
        // 组标题右侧摘要说明「写没写过」，别让用户为了看有没有备注去展开一组。
        let note_sum = if self.note_draft.trim().is_empty() {
            "空"
        } else {
            "已写"
        };
        let (open_note, act) = self.group(ui, focus, "文本内容", note_sum, "note/text", true);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_note {
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 12,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("备注")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                    ui.add_space(4.0);
                    // §4.2：3 行起；超过 8 行折叠，点「展开」给足行数（长备注不靠滚动条猜）。
                    let rows = note_rows(&self.note_draft, self.note_expanded);
                    let over = note_rows(&self.note_draft, true) > rows;
                    // §4.2：文本域**无白底无框**，只靠一条 `hairline` 底线。
                    // 用 `text_box_frame`（填色 + 1px 描边 + 圆角）会把它画成一个输入框，
                    // 与「备注是一段纸上的字」的观感相反。
                    let field = ui.add(
                        egui::TextEdit::multiline(&mut self.note_draft)
                            .id(note_id)
                            .desired_width(f32::INFINITY)
                            .desired_rows(rows)
                            .hint_text("写点备注…")
                            .frame(egui::Frame::NONE)
                            .margin(egui::Margin::symmetric(0, 4)),
                    );
                    ui.painter().hline(
                        field.rect.x_range(),
                        field.rect.max.y + 3.0,
                        egui::Stroke::new(1.0, ink.ch.sep),
                    );
                    ui.add_space(6.0);
                    if over || self.note_expanded {
                        let total = note_rows(&self.note_draft, true);
                        let tip = if self.note_expanded {
                            "收起".to_string()
                        } else {
                            format!("展开 · 共 {total} 行")
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(tip)
                                        .size(Type::CAPTION)
                                        .color(ink.ch.dim),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            self.note_expanded = !self.note_expanded;
                        }
                        ui.add_space(4.0);
                    }
                    ui.label(
                        egui::RichText::new("Ctrl+Enter 保存 · Esc 取消")
                            .size(Type::CAPTION)
                            .color(ink.ch.dim),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let dirty = self.note_draft.trim_end() != self.note_before.trim_end();
                        if ui.add_enabled(dirty, egui::Button::new("保存")).clicked() {
                            action = self.commit_note();
                        }
                        if ui.button("清除").clicked() {
                            self.note_draft.clear();
                            action = self.commit_note();
                        }
                    });
                });
        }
        let task_sum = self
            .items
            .last()
            .and_then(|i| i.task)
            .map(|t| t.label().to_string())
            .unwrap_or_default();
        let (open_task, act) = self.group(ui, focus, "任务", &task_sum, "note/task", true);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_task {
            if let Some(item) = self.items.last() {
                let current = item.task;
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new("任务")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (state, label) in [
                        (TaskState::Todo, "未开始"),
                        (TaskState::Doing, "进行中"),
                        (TaskState::Done, "完成"),
                    ] {
                        let key = FocusKey::dynamic("inspector/task", state.as_str());
                        if seg(
                            ui,
                            label,
                            current == Some(state),
                            Vec2::new(56.0, 24.0),
                            ink,
                            Some(&key),
                            focus,
                        ) {
                            action = Some(Action::Apply(Command::SetTask {
                                id: item.id.clone(),
                                task: Some(state),
                            }));
                        }
                    }
                    if current.is_some()
                        && crate::ui_kit::icon_button(
                            ui,
                            ink.ch,
                            18.0,
                            crate::icons::Icon::Undo,
                            "清除任务状态",
                        )
                    {
                        action = Some(Action::Apply(Command::SetTask {
                            id: item.id.clone(),
                            task: None,
                        }));
                    }
                });
            }
        }
        // §4.1 的组顺序：文本内容 → 任务 → 标签 → 链接 → 卡片（图片是既有能力，垫在最后）。
        let tag_sum = match self.items.last().map(|i| i.labels.len()).unwrap_or(0) {
            0 => "无".to_string(),
            n => format!("{n} 个"),
        };
        let (open_tag, act) = self.group(ui, focus, "标签", &tag_sum, "note/labels", true);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_tag {
            if let Some(a) = self.labels_section(ui, focus) {
                action = Some(a);
            }
        }
        let link_sum = link_host(&self.link_draft).unwrap_or_default();
        let (open_link, act) = self.group(ui, focus, "链接", &link_sum, "note/link", true);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_link {
            if let Some(a) = self.link_body(ui, focus) {
                action = Some(a);
            }
        }
        let card_sum = self
            .items
            .last()
            .and_then(|i| doc.card_for_node(&i.id))
            .map(|c| c.display_title().to_string())
            .unwrap_or_else(|| "未绑定".to_string());
        let (open_card, act) = self.group(ui, focus, "卡片", &card_sum, "note/card", true);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_card {
            if let Some(a) = self.card_section(ui, doc) {
                action = Some(a);
            }
        }
        ui.add_space(6.0);
        let (open_img, act) = self.group(ui, focus, "图片", "", "note/image", false);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_img {
            if let Some(item) = self.items.last() {
                let has_image = item.image.as_deref().is_some_and(|s| !s.trim().is_empty());
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new("图片")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                    if has_image {
                        ui.label(
                            egui::RichText::new("已设置")
                                .size(Type::CAPTION)
                                .color(ink.ch.dim),
                        );
                        if ui.button("移除").clicked() {
                            action = Some(Action::ClearImage);
                        }
                    } else if ui.button("选择图片…").clicked() {
                        action = Some(Action::PickImage);
                    }
                });
            }
        }
        // 概念图专有的「命题」组（C14）：垫在最后，不动 §4.1 给思维导图定的组序
        // （文本内容 → 任务 → 标签 → 链接 → 卡片 → 图片）。概念图里「谁和谁、用什么短语连着」
        // 就是主信息，不该逼用户回头看画布上的线。
        if doc.is_concept_map() {
            if let Some(id) = self.items.last().map(|i| i.id.clone()) {
                // 只有概念（`parent_id = None`）才有命题；旧文件/导入数据里残留的树节点跳过。
                if doc.node(&id).is_some_and(|n| n.parent_id.is_none()) {
                    if let Some(a) = self.propositions_section(ui, focus, doc, &id) {
                        action = Some(a);
                    }
                }
            }
        }
        action
    }

    /// 「命题」组（概念图专有，C14）：这颗概念**出去**的与**进来**的关系线。
    ///
    /// 概念图的主信息就是「谁和谁、用什么短语连着」，而选中一颗概念时检查器原先只有
    /// 备注/标签/卡片——用户想知道这颗概念牵了哪些命题，只能回头看画布上的线。
    ///
    /// 两段各列一行一条命题：行点一下 = 选中那条线（复用 `Action::SelectRelationship`，
    /// 面板会切到右边的「关系线」页），行尾 `×` = 删掉这条命题（一条命令、一次撤销）。
    /// 出去的按「短语 → 目标」，进来的按「← 源 短语」，两边都带箭头语义（`head.label()`）。
    pub(crate) fn propositions_section(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        doc: &Document,
        id: &str,
    ) -> Option<Action> {
        use mw_core::model::ArrowHead;
        let mut action = None;
        let ink = self.ink;

        // 一行 = 一条命题：(关系线 id, 宿主节点 id（边挂在谁身上）, 另一端 id, 短语, 箭头)。
        type Row = (String, String, String, String, ArrowHead);
        let name_of = |nid: &str| {
            doc.node(nid)
                .map(|n| n.text.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "未命名".into())
        };

        // 出去的：边挂在自己身上（`Relationship` 没有 `from`，源节点就是宿主）。
        let mut out: Vec<Row> = doc
            .node(id)
            .map(|n| {
                n.relationships
                    .iter()
                    .map(|r| {
                        (
                            r.id.clone(),
                            id.to_string(),
                            r.to.clone(),
                            r.label.clone().unwrap_or_default(),
                            r.head,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        // 进来的：边挂在别人身上、`to` 指向自己。行里要写的是**源概念**名，
        // 而源概念就是边的宿主，所以 owner 与 other 都是宿主 id。
        let mut inc: Vec<Row> = Vec::new();
        for n in doc.nodes.values() {
            for r in &n.relationships {
                if r.to == id {
                    inc.push((
                        r.id.clone(),
                        n.id.clone(),
                        n.id.clone(),
                        r.label.clone().unwrap_or_default(),
                        r.head,
                    ));
                }
            }
        }
        // `doc.nodes` 是 `HashMap`，迭代序不稳定：排序才能让行位、焦点、金样都可复现。
        let by_name = |a: &Row, b: &Row| {
            (name_of(&a.2), &a.3)
                .partial_cmp(&(name_of(&b.2), &b.3))
                .unwrap_or(std::cmp::Ordering::Equal)
        };
        out.sort_by(by_name);
        inc.sort_by(by_name);

        let sum = format!("出 {} · 入 {}", out.len(), inc.len());
        // 默认展开：概念图里这就是主信息，不该再点一下才看得见。
        let (open, act) = self.group_with(ui, focus, "命题", &sum, "note/props", false, true);
        if let Some(a) = act {
            action = Some(a);
        }
        if !open {
            return action;
        }

        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 14,
                right: 14,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                if out.is_empty() && inc.is_empty() {
                    ui.label(
                        egui::RichText::new("还没有命题：从这颗概念拖线到另一颗概念，就是一条命题")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                    return;
                }
                for (title, outgoing, rows) in [("出去", true, &out), ("进来", false, &inc)] {
                    if rows.is_empty() {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("{title} · {}", rows.len()))
                            .size(Type::CAPTION)
                            .color(ink.ch.dim),
                    );
                    for (rel_id, owner, other, label, head) in rows {
                        // 行高 28（§4.1「组内控件行高 28」）：必须走 `content_row`，
                        // `ui.horizontal` 会按全局 `interact_size.y = 30` 要高度。
                        content_row(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            // 文字一律截断：不截断的 Label 会把面板最小宽顶出去（SE-05 那类回归，
                            // 面板 240 时内容宽只有 212）。常见概念名/短语（2–4 字）永远碰不到上限。
                            let phrase = if label.is_empty() {
                                "（无短语）".to_string()
                            } else {
                                clip(label, CLIP_PHRASE)
                            };
                            let other = clip(&name_of(other), CLIP_NAME);
                            // 箭头用字形表示方向，`head.label()` 放进悬停说明（行宽只有 212）。
                            let glyph = match head {
                                ArrowHead::Off => "—",
                                ArrowHead::Forward => "→",
                                ArrowHead::Backward => "←",
                                ArrowHead::Both => "↔",
                            };
                            let text = format!("{glyph} {phrase} {other}");
                            let hover = if outgoing {
                                format!(
                                    "{phrase} {glyph} {other}（{}）· 点一下选中这条关系线",
                                    head.label()
                                )
                            } else {
                                format!(
                                    "{other} {glyph} 本概念 · {phrase}（{}）· 点一下选中这条关系线",
                                    head.label()
                                )
                            };
                            let row = ui.selectable_label(
                                false,
                                egui::RichText::new(text).size(geo::ROW_TEXT),
                            );
                            let selected = row.on_hover_text(hover).clicked();
                            let removed = ui
                                .with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| crate::ui_kit::close_x(ui, ink.ch, 14.0, "删除这条命题"),
                                )
                                .inner;
                            if selected {
                                action = Some(Action::SelectRelationship {
                                    from: owner.clone(),
                                    id: rel_id.clone(),
                                });
                            } else if removed {
                                action = Some(Action::Apply(Command::DeleteRelationship {
                                    from: owner.clone(),
                                    id: rel_id.clone(),
                                }));
                            }
                        });
                    }
                }
                ui.add_space(4.0);
            });
        action
    }

    /// 「标签」组（`docs/30` §4.2 / §4.5）：**纯文字 `13/18` + 前导 6px 色点**。
    ///
    /// 没有底色、没有圆角、没有描边——标签是一行字，不是胶囊。色点的颜色由
    /// `note_view::tag_color` 按标签根定，卡片页与笔记库里的同名标签是同一个色，
    /// 所以检查器里这一行和别处看到的是同一个东西。
    pub(crate) fn labels_section(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
    ) -> Option<Action> {
        let ink = self.ink;
        let mut action = None;
        if self.items.len() > 1 {
            self.multi_hint(ui, "标签只能对单个主题编辑。");
            return None;
        }
        let labels = self
            .items
            .last()
            .map(|i| i.labels.clone())
            .unwrap_or_default();
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 14,
                right: 14,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                if labels.is_empty() {
                    ui.label(
                        egui::RichText::new("还没有标签")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                }
                for label in &labels {
                    let mut remove = false;
                    // 行高 28（§4.1「组内控件行高 28」）：色点、文字、移除位同一条基线上。
                    // 必须走 `content_row`（钉死 28）；`ui.horizontal` 会按全局
                    // `interact_size.y = 30` 要高度，一行 30、行距 38，量出来就超标。
                    content_row(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                        ui.painter().circle_filled(
                            dot.center(),
                            3.0,
                            crate::note_view::tag_color(label),
                        );
                        ui.label(
                            egui::RichText::new(label)
                                .size(geo::ROW_TEXT)
                                .color(ink.ch.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::ui_kit::close_x(ui, ink.ch, 14.0, "移除标签") {
                                remove = true;
                            }
                        });
                    });
                    if remove {
                        if let Some(id) = self.items.last().map(|i| i.id.clone()) {
                            action = Some(Action::Apply(Command::RemoveLabel {
                                ids: vec![id],
                                label: label.clone(),
                            }));
                        }
                    }
                }
                ui.add_space(4.0);
                if labels.len() >= mw_core::document::MAX_LABELS {
                    ui.label(
                        egui::RichText::new(format!(
                            "最多 {} 个标签",
                            mw_core::document::MAX_LABELS
                        ))
                        .size(Type::CAPTION)
                        .color(ink.ch.dim),
                    );
                    return;
                }
                let label_key = FocusKey::new("inspector/note/label/field");
                let field_id = focus::widget_id(ui, &label_key);
                focus.note(PanelId::Inspector, ui.ctx(), &label_key);
                let focused = ui.memory(|m| m.has_focus(field_id));
                let submitted = focused
                    && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                    && !self.label_draft.trim().is_empty();
                ui.horizontal(|ui| {
                    // 宽度必须先算死再排：输入框用 `available - 44`、按钮用「自然宽」时，
                    // 两者相加会比可用宽多出几个像素，面板的最小宽就被内容顶大——
                    // 这正是 SE-05 修过的那类回归（288 被顶到 316，且每帧还会长）。
                    let gap = ui.spacing().item_spacing.x;
                    let field_w = label_field_width(ui.available_width() - gap);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.label_draft)
                            .id(field_id)
                            .desired_width(field_w)
                            .hint_text("新标签，回车提交")
                            .frame(text_box_frame(ink))
                            .margin(egui::Margin::symmetric(LABEL_MARGIN_X, 4)),
                    );
                    let add = ui
                        .add_sized([LABEL_BTN_W, 22.0], egui::Button::new("添加"))
                        .clicked()
                        || submitted;
                    if add {
                        let label = self.label_draft.trim().to_string();
                        if !label.is_empty() {
                            if let Some(id) = self.items.last().map(|i| i.id.clone()) {
                                action = Some(Action::Apply(Command::AddLabel {
                                    ids: vec![id],
                                    label,
                                }));
                            }
                            self.label_draft.clear();
                        }
                    }
                });
            });
        action
    }

    /// 「卡片笔记」区：备注是节点的一部分，卡片是独立实体。
    ///
    /// 三态（`docs/30` §4.2）：没绑定 → 「新建卡片笔记 / 关联已有」；绑定了 →
    /// 「打开卡片 / 解绑」。概念图与思维导图共用同一套 `card_id` 语义，
    /// 所以这里不判断文档类型——措辞差异只在「来源」上看得到。
    pub(crate) fn card_section(&mut self, ui: &mut egui::Ui, doc: &Document) -> Option<Action> {
        let id = self.items.last()?.id.clone();
        let ink = self.ink;
        let mut action = None;
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 14,
                right: 14,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("卡片笔记")
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
                ui.add_space(4.0);
                match doc.card_for_node(&id) {
                    Some(card) => {
                        // 卡片标题与摘要是用户内容，长短不可控：一律截断到面板宽度。
                        // 不截断的 Label 会把面板最小宽顶出去（288 → 316），而面板宽度是合同值。
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(card.display_title())
                                    .size(Type::LABEL)
                                    .color(ink.ch.text),
                            )
                            .truncate(),
                        );
                        let excerpt = card.excerpt(90);
                        if !excerpt.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(excerpt)
                                        .size(Type::SMALL)
                                        .color(ink.ch.dim),
                                )
                                .truncate(),
                            );
                        }
                        // 「来源」一行（§4.2）：卡片是独立实体，先说清它从哪来。
                        let word = if doc.is_concept_map() {
                            "概念"
                        } else {
                            "主题"
                        };
                        ui.label(
                            egui::RichText::new(card_source(doc, card, word))
                                .size(Type::CAPTION)
                                .color(ink.ch.dim),
                        );
                        if !card.tags.is_empty() {
                            ui.add_space(3.0);
                            ui.horizontal_wrapped(|ui| {
                                for tag in &card.tags {
                                    ui.label(
                                        egui::RichText::new(format!("#{tag}"))
                                            .size(Type::CAPTION)
                                            .color(crate::note_view::tag_color(tag)),
                                    );
                                }
                            });
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("打开卡片").clicked() {
                                action = Some(Action::OpenCard(card.id.clone()));
                            }
                            if ui.button("解绑").clicked() {
                                action = Some(Action::DetachCard(id.clone()));
                            }
                        });
                    }
                    None => {
                        // 概念图里挂卡片的是「概念」，不是「主题」——同一套 `card_id`
                        // 语义，措辞得跟着文档类型走，否则用户以为自己在编辑别的东西。
                        let word = if doc.is_concept_map() {
                            "概念"
                        } else {
                            "主题"
                        };
                        ui.label(
                            egui::RichText::new(format!("这个{word}还没有卡片笔记。"))
                                .size(Type::SMALL)
                                .color(ink.ch.dim),
                        );
                        ui.add_space(6.0);
                        let picking = self.card_pick.as_deref() == Some(id.as_str());
                        ui.horizontal(|ui| {
                            if ui.button("新建卡片笔记").clicked() {
                                self.card_pick = None;
                                action = Some(Action::NewCardForNode(id.clone()));
                            }
                            if ui
                                .button(if picking {
                                    "收起候选"
                                } else {
                                    "关联已有"
                                })
                                .clicked()
                            {
                                self.card_pick = if picking { None } else { Some(id.clone()) };
                            }
                        });
                        if picking {
                            if let Some(a) = self.card_pick_list(ui, doc, &id) {
                                action = Some(a);
                            }
                        }
                    }
                }
            });
        action
    }

    /// 「关联已有」的候选列表：只列**没被别的节点占用**的卡片（选了也是白选），
    /// 限高滚动，最多 8 条——候选不是浏览界面，找不到就去笔记库搜。
    fn card_pick_list(&mut self, ui: &mut egui::Ui, doc: &Document, node: &str) -> Option<Action> {
        let ink = self.ink;
        let options = doc.linkable_cards(node, 8);
        ui.add_space(4.0);
        if options.is_empty() {
            ui.label(
                egui::RichText::new("当前文档里没有可关联的卡片。")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
            return None;
        }
        let mut picked = None;
        egui::Frame::NONE
            .fill(ink.ch.panel2)
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(4, 4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .id_salt("mw-inspector-card-pick")
                    .max_height(168.0)
                    .show(ui, |ui| {
                        for card in &options {
                            // 摘要并进同一行：候选列表是「挑一张」的地方，不是阅读区，
                            // 两行一条会让 8 条候选撑成一屏。
                            let brief = card.excerpt(24);
                            let row = if brief.is_empty() {
                                card.display_title().to_string()
                            } else {
                                format!("{} · {}", card.display_title(), brief)
                            };
                            let resp = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(row)
                                        .size(Type::LABEL)
                                        .color(ink.ch.text),
                                )
                                .frame(false)
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(4.0)
                                .min_size(egui::vec2(ui.available_width(), 26.0)),
                            );
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                picked = Some(card.id.clone());
                            }
                        }
                    });
            });
        picked.map(|card| {
            self.card_pick = None;
            Action::LinkCard {
                node: node.to_string(),
                card,
            }
        })
    }

    pub(crate) fn commit_note(&mut self) -> Option<Action> {
        let id = self.items.last()?.id.clone();
        let text = self.note_draft.trim_end().to_string();
        let note = if text.trim().is_empty() {
            None
        } else {
            Some(text)
        };
        self.note_before = note.clone().unwrap_or_default();
        Some(Action::Apply(Command::SetNote { id, note }))
    }

    pub(crate) fn link_body(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
    ) -> Option<Action> {
        if self.items.len() > 1 {
            self.multi_hint(ui, "链接只能对单个主题编辑。");
            return None;
        }
        let mut action = None;
        let ink = self.ink;
        let field_key = FocusKey::new("inspector/link/field");
        let field_id = focus::widget_id(ui, &field_key);
        focus.note(PanelId::Inspector, ui.ctx(), &field_key);
        let focused = ui.memory(|m| m.has_focus(field_id));
        if focused {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                return self.commit_link();
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.link_draft = self.link_before.clone();
            }
        }

        ui.add_space(2.0);
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 14,
                right: 14,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("链接")
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.link_draft)
                        .id(field_id)
                        .desired_width(f32::INFINITY)
                        .hint_text("https://…")
                        .frame(text_box_frame(ink))
                        .margin(egui::Margin::symmetric(8, 4)),
                );
                match link_host(&self.link_draft) {
                    Some(host) => {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new(host)
                                .size(Type::CAPTION)
                                .color(ink.ch.dim),
                        );
                    }
                    None if !self.link_draft.trim().is_empty() => {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new("只支持 http/https 链接")
                                .size(Type::CAPTION)
                                .color(ink.ch.warn),
                        );
                    }
                    None => {}
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let dirty = self.link_draft.trim() != self.link_before.trim();
                    if ui.add_enabled(dirty, egui::Button::new("保存")).clicked() {
                        action = self.commit_link();
                    }
                    let open = link_host(&self.link_draft).is_some();
                    if ui
                        .add_enabled(open, egui::Button::new("打开"))
                        .on_hover_text("用系统默认浏览器打开")
                        .clicked()
                    {
                        action = Some(Action::OpenLink(self.link_draft.trim().to_string()));
                    }
                    if ui.button("清除").clicked() {
                        self.link_draft.clear();
                        action = self.commit_link();
                    }
                });
            });
        action
    }

    pub(crate) fn commit_link(&mut self) -> Option<Action> {
        let id = self.items.last()?.id.clone();
        let text = self.link_draft.trim().to_string();
        let link = if text.is_empty() { None } else { Some(text) };
        self.link_before = link.clone().unwrap_or_default();
        Some(Action::Apply(Command::SetLink { id, link }))
    }

    // ───────────────────────── 标记（图标 / 标签） ─────────────────────────

    pub(crate) fn mark_body(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        anns: &[mw_core::model::AnnotationRef],
        spans: &[Vec<(String, bool, String)>],
    ) -> Option<Action> {
        let mut action = None;
        let ink = self.ink;
        let first_icon = self.items.first().and_then(|i| i.icon.clone());
        let mixed = self.items.iter().any(|i| i.icon != first_icon);
        let current = if mixed { None } else { first_icon.as_deref() };

        ui.add_space(2.0);
        let (open_0, act) = self.group_with(ui, focus, "图标", "", "mark/icons", true, false);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_0 {
            body_label(ui, "图标", crate::chrome::Type::CAPTION, ink.ch.dim);
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(GUTTER);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for def in ICONS {
                    let selected = current == Some(def.key);
                    let color = Rgba::parse(def.color).map(color32).unwrap_or(ink.ch.dim);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                    let painter = ui.painter();
                    if selected {
                        // 淡色底 + 实色描边：整块按图标色染色，选中态一眼能认出来。
                        painter.rect_filled(
                            rect,
                            6.0,
                            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 46),
                        );
                        painter.rect_stroke(
                            rect,
                            6.0,
                            Stroke::new(1.5, color),
                            egui::StrokeKind::Inside,
                        );
                    } else if resp.hovered() {
                        painter.rect_filled(rect, 6.0, ink.ch.hover);
                    }
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        def.glyph,
                        FontId::proportional(Glyph::MEDIUM),
                        color,
                    );
                    let resp = resp.on_hover_text(def.label);
                    if resp.clicked() {
                        let ids: Vec<String> = self.items.iter().map(|i| i.id.clone()).collect();
                        let icon = if selected {
                            None
                        } else {
                            Some(def.key.to_string())
                        };
                        action = Some(Action::Apply(Command::SetIcon { ids, icon }));
                    }
                }
            });
            if current.is_some() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    if ui.button("清除图标").clicked() {
                        let ids: Vec<String> = self.items.iter().map(|i| i.id.clone()).collect();
                        action = Some(Action::Apply(Command::SetIcon { ids, icon: None }));
                    }
                });
            }
            ui.add_space(8.0);
        }
        let (open_2, act) = self.group_with(
            ui,
            focus,
            "概要与边界",
            "",
            "mark/annotations",
            false,
            false,
        );
        if let Some(a) = act {
            action = Some(a);
        }
        if open_2 {
            // ---- 盖住这个节点的概要 / 边界：可改名、可删除 ----
            let mut commit: Option<(String, String, String, String)> = None;
            if !anns.is_empty() {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.label(
                        egui::RichText::new("概要 / 边界")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                });
                ui.add_space(4.0);
                for (i, ann) in anns.iter().enumerate() {
                    let is_summary = ann.kind == mw_core::model::AnnotationKind::Summary;
                    let editing_this = self.summary_edit.as_ref().is_some_and(|e| e.id == ann.id);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(GUTTER);
                        if editing_this && is_summary {
                            // 行内改写概要文字：Enter 提交、Esc 取消、失焦也提交。
                            if let Some(ed) = self.summary_edit.as_mut() {
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut ed.draft)
                                        .id(egui::Id::unique(("summary-field", &ed.id)))
                                        .desired_width(120.0)
                                        .frame(text_box_frame(ink))
                                        .margin(egui::Margin::symmetric(6, 3)),
                                );
                                crate::trace::name(ui.ctx(), resp.id, "summary-field");
                                if ed.just_started {
                                    resp.request_focus();
                                    ed.just_started = false;
                                }
                                let esc =
                                    ui.input_mut(|j| j.consume_key(Modifiers::NONE, Key::Escape));
                                let enter =
                                    ui.input_mut(|j| j.consume_key(Modifiers::NONE, Key::Enter));
                                if esc {
                                    self.summary_edit = None;
                                } else if enter || resp.lost_focus() {
                                    commit = Some((
                                        ed.parent.clone(),
                                        ed.id.clone(),
                                        ed.draft.trim_end().to_string(),
                                        ed.before.clone(),
                                    ));
                                    self.summary_edit = None;
                                }
                            }
                        } else {
                            let piece = if is_summary {
                                format!("概要「{}」", ann.label)
                            } else {
                                "边界".to_string()
                            };
                            ui.label(
                                egui::RichText::new(piece)
                                    .size(Type::CAPTION)
                                    .color(ink.ch.text),
                            );
                            if ann.owned {
                                ui.label(
                                    egui::RichText::new("本节点")
                                        .size(Type::CAPTION)
                                        .color(ink.ch.dim),
                                );
                            }
                            if !is_summary {
                                // 边界颜色：默认 + 四支品牌色，点一下就是一条可撤销命令。
                                let current = ann.color.as_deref();
                                for (k, hex) in [
                                    None,
                                    Some("#f6212d"),
                                    Some("#318de1"),
                                    Some("#6dc354"),
                                    Some("#fe8f0b"),
                                ]
                                .into_iter()
                                .enumerate()
                                {
                                    let (rect, resp) =
                                        ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
                                    crate::trace::name(
                                        ui.ctx(),
                                        resp.id,
                                        &format!("boundary-color {i} {k}"),
                                    );
                                    let center = rect.center();
                                    match hex {
                                        Some(h) => {
                                            let c =
                                                Rgba::parse(h).map(color32).unwrap_or(ink.ch.dim);
                                            ui.painter().circle_filled(center, 4.5, c);
                                            if current == Some(h) {
                                                ui.painter().circle_stroke(
                                                    center,
                                                    6.0,
                                                    Stroke::new(1.4, ink.ch.accent),
                                                );
                                            }
                                        }
                                        None => {
                                            ui.painter().circle_stroke(
                                                center,
                                                4.5,
                                                Stroke::new(1.0, ink.ch.faint),
                                            );
                                            if current.is_none() {
                                                ui.painter().circle_stroke(
                                                    center,
                                                    6.0,
                                                    Stroke::new(1.4, ink.ch.accent),
                                                );
                                            }
                                        }
                                    }
                                    if resp.clicked() {
                                        action = Some(Action::Apply(Command::SetBoundaryColor {
                                            parent: ann.parent.clone(),
                                            id: ann.id.clone(),
                                            color: hex.map(|h| h.to_string()),
                                        }));
                                    }
                                }
                            }
                            if is_summary {
                                // 跨度开关：父节点每个直接子节点一格，点一下加入/移出。
                                let span = spans.get(i).cloned().unwrap_or_default();
                                for (j, (child, on, label)) in span.iter().enumerate() {
                                    let (rect, resp) = ui
                                        .allocate_exact_size(Vec2::new(40.0, 18.0), Sense::click());
                                    crate::trace::name(
                                        ui.ctx(),
                                        resp.id,
                                        &format!("annot-span {i} {j}"),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        5.0,
                                        if *on { ink.ch.accent_soft } else { ink.ch.dim },
                                    );
                                    ui.painter().text(
                                        rect.center(),
                                        Align2::CENTER_CENTER,
                                        label,
                                        FontId::proportional(Type::CAPTION),
                                        if *on { ink.ch.accent } else { ink.ch.dim },
                                    );
                                    if resp.clicked() {
                                        let mut next: Vec<String> = span
                                            .iter()
                                            .filter(|(_, on, _)| *on)
                                            .map(|(id, _, _)| id.clone())
                                            .collect();
                                        if *on {
                                            next.retain(|t| t != child);
                                        } else {
                                            next.push(child.clone());
                                        }
                                        if !next.is_empty() {
                                            action =
                                                Some(Action::Apply(Command::SetSummaryTargets {
                                                    parent: ann.parent.clone(),
                                                    id: ann.id.clone(),
                                                    targets: next,
                                                }));
                                        }
                                    }
                                }
                                let pen = ui
                                    .add(egui::Button::new("✎").frame(false))
                                    .on_hover_text("修改概要文字");
                                crate::trace::name(ui.ctx(), pen.id, &format!("annot-edit {i}"));
                                if pen.clicked() {
                                    self.summary_edit = Some(SummaryEdit {
                                        parent: ann.parent.clone(),
                                        id: ann.id.clone(),
                                        draft: ann.label.clone(),
                                        before: ann.label.clone(),
                                        just_started: true,
                                    });
                                }
                            }
                            if crate::ui_kit::close_x(ui, ink.ch, 16.0, "删除这条注解") {
                                let cmd = match ann.kind {
                                    mw_core::model::AnnotationKind::Summary => {
                                        Command::DeleteSummary {
                                            parent: ann.parent.clone(),
                                            id: ann.id.clone(),
                                        }
                                    }
                                    mw_core::model::AnnotationKind::Boundary => {
                                        Command::DeleteBoundary {
                                            parent: ann.parent.clone(),
                                            id: ann.id.clone(),
                                        }
                                    }
                                };
                                action = Some(Action::Apply(cmd));
                            }
                        }
                    });
                }
                if let Some((parent, id, text, before)) = commit {
                    if text != before {
                        action = Some(Action::Apply(Command::SetSummaryText { parent, id, text }));
                    }
                }
            }
        }
        action
    }

    /// 画布选中注解时的专属页：概要改文字、边界改颜色，外加删除/取消。
    /// 多选命题页（CM-17）：只给能批量做的事——箭头、线型、线宽、删除。
    ///
    /// 短语与注释是**单条**属性，多选时不给入口：一次改 5 条短语没有意义，
    /// 给一个改不动的控件比不给更糟。
    pub(crate) fn relationships_body(
        &mut self,
        ui: &mut egui::Ui,
        _doc: &Document,
        rels: &[crate::canvas::RelationshipSel],
    ) -> Option<Action> {
        let mut action = None;
        let ink = self.ink;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new(format!("{} 条命题", rels.len()))
                    .size(Type::BODY)
                    .family(FontFamily::Name(crate::fonts::SEMIBOLD.into()))
                    .color(ink.ch.text),
            );
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("批量改箭头 / 线型 / 线宽")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
        });
        ui.add_space(8.0);

        let batch = |commands: Vec<Command>| -> Option<Action> {
            (!commands.is_empty()).then_some(Action::Apply(Command::Batch { commands }))
        };

        let heads = [
            (mw_core::model::ArrowHead::Off, "两端都不画"),
            (mw_core::model::ArrowHead::Forward, "只在终点画"),
            (mw_core::model::ArrowHead::Backward, "只在起点画"),
            (mw_core::model::ArrowHead::Both, "两端都画"),
        ];
        ui.horizontal_wrapped(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("箭头")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
            for (head, hint) in heads {
                if ui
                    .selectable_label(false, egui::RichText::new(head.label()).size(Type::SMALL))
                    .on_hover_text(format!("{hint}（应用到全部 {} 条）", rels.len()))
                    .clicked()
                {
                    action = batch(
                        rels.iter()
                            .map(|r| Command::SetRelationshipHead {
                                from: r.from.clone(),
                                id: r.id.clone(),
                                head,
                            })
                            .collect(),
                    );
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("线型")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
            for (dashed, label) in [(false, "实线"), (true, "虚线")] {
                if ui
                    .selectable_label(false, egui::RichText::new(label).size(Type::SMALL))
                    .clicked()
                {
                    action = batch(
                        rels.iter()
                            .map(|r| Command::SetRelationshipDashed {
                                from: r.from.clone(),
                                id: r.id.clone(),
                                dashed,
                            })
                            .collect(),
                    );
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("线宽")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
            for (width, label) in [(Some(1.0f32), "细"), (None, "标准"), (Some(2.5), "粗")] {
                if ui
                    .selectable_label(false, egui::RichText::new(label).size(Type::SMALL))
                    .clicked()
                {
                    action = batch(
                        rels.iter()
                            .map(|r| Command::SetRelationshipWidth {
                                from: r.from.clone(),
                                id: r.id.clone(),
                                width,
                            })
                            .collect(),
                    );
                }
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            if ui.button(format!("删除 {} 条命题", rels.len())).clicked() {
                action = batch(
                    rels.iter()
                        .map(|r| Command::DeleteRelationship {
                            from: r.from.clone(),
                            id: r.id.clone(),
                        })
                        .collect(),
                );
            }
            if ui.button("取消选择").clicked() {
                action = Some(Action::ClearRelationship);
            }
        });
        action
    }

    pub(crate) fn relationship_body(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        rel: &crate::canvas::RelationshipSel,
    ) -> Option<Action> {
        let mut action = None;
        let ink = self.ink;
        let from_text = doc
            .node(&rel.from)
            .map(|n| n.text.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "未命名".into());
        let to_text = doc
            .node(&rel.to)
            .map(|n| n.text.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "未命名".into());
        let current = doc
            .node(&rel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == rel.id))
            .and_then(|r| r.label.clone())
            .unwrap_or_default();

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("关系线")
                    .size(Type::BODY)
                    .family(FontFamily::Name(crate::fonts::SEMIBOLD.into()))
                    .color(ink.ch.text),
            );
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(GUTTER);
            if ui
                .small_button(&from_text)
                .on_hover_text("选中起点")
                .clicked()
            {
                action = Some(Action::SelectNode(rel.from.clone()));
            }
            ui.label(egui::RichText::new("→").color(ink.ch.dim));
            if ui
                .small_button(&to_text)
                .on_hover_text("选中终点")
                .clicked()
            {
                action = Some(Action::SelectNode(rel.to.clone()));
            }
        });
        ui.add_space(8.0);

        let current_note = doc
            .node(&rel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == rel.id))
            .and_then(|r| r.note.clone())
            .unwrap_or_default();
        if self.rel_id.as_deref() != Some(rel.id.as_str()) {
            self.rel_id = Some(rel.id.clone());
            self.rel_draft = current.clone();
            self.rel_before = current.clone();
            self.rel_note_draft = current_note.clone();
            self.rel_note_before = current_note;
        }
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.rel_draft)
                    .id(egui::Id::unique(("rel-label", &rel.id)))
                    .desired_width(160.0)
                    // 概念图里短语是**必填**的：一条没有短语的线不构成命题（`docs/30` §5）。
                    .hint_text(if doc.is_concept_map() {
                        "连接短语（必填）"
                    } else {
                        "关系线标签"
                    })
                    .frame(text_box_frame(ink))
                    .margin(egui::Margin::symmetric(7, 4)),
            );
            let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
            if (enter || resp.lost_focus()) && self.rel_draft.trim_end() != self.rel_before {
                let text = self.rel_draft.trim_end().to_string();
                if doc.is_concept_map() && text.trim().is_empty() {
                    // 概念图里不许把已有短语清空：就地退回原短语、**不发命令**——
                    // 核心层同样会拒（`Invalid("概念图的连接短语必填")`），就地退回省一次
                    // 「修改未生效」toast 的往返，也让输入框里的字不会假装清掉了。
                    // 注意新建的线仍然允许先没有短语（画布拖出线 → 立刻打字），堵的只是清空。
                    self.rel_draft = self.rel_before.clone();
                } else {
                    self.rel_before = text.clone();
                    action = Some(Action::Apply(Command::SetRelationshipLabel {
                        from: rel.from.clone(),
                        id: rel.id.clone(),
                        label: (!text.is_empty()).then_some(text),
                    }));
                }
            }
        });
        // 空短语的提示态：概念图里这条线还不构成命题。提示是一行 12 号浅字，
        // 文案压到 6 个字——长句会把内容最小宽顶大（面板 240 时只有 212），全文进悬停。
        if doc.is_concept_map() && self.rel_draft.trim().is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.label(
                    egui::RichText::new("连接短语必填")
                        .size(Type::CAPTION)
                        .color(ink.ch.dim),
                )
                .on_hover_text(
                    "概念图的线必须有连接短语，否则不构成一条命题。新建的线先写短语再去做别的。",
                );
            });
        }
        ui.add_space(10.0);
        let current_color = doc
            .node(&rel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == rel.id))
            .and_then(|r| r.color.clone());
        let mut color = current_color.clone();
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            crate::theme_studio::color_slot(ui, ink.ch, "自定义线色", &mut color);
        });
        if color != current_color {
            action = Some(Action::Apply(Command::SetRelationshipColor {
                from: rel.from.clone(),
                id: rel.id.clone(),
                color,
            }));
        }
        // ⑦ 端点箭头：CmapTools 里 A → B 与 B → A 不是一回事，所以这里给四档而不是一个开关。
        let current_head = doc
            .node(&rel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == rel.id))
            .map(|r| r.head)
            .unwrap_or_default();
        let heads = [
            (mw_core::model::ArrowHead::Off, "两端都不画，只表示「相关」"),
            (mw_core::model::ArrowHead::Forward, "只在终点画（默认）"),
            (mw_core::model::ArrowHead::Backward, "只在起点画，读作反向"),
            (mw_core::model::ArrowHead::Both, "两端都画，双向"),
        ];
        // §4.4「每个可改属性都有键盘路径」：线被选中时数字键 1–4 直选这四档。
        // 只在没有文本控件吃键盘时生效——短语框里敲数字必须还是数字。
        // （数字键 1–6 在检查器主页面被色板占了，关系线页是独立一页，不冲突。）
        if !ui.ctx().egui_wants_keyboard_input() {
            for (i, (head, _hint)) in heads.iter().enumerate() {
                let digit = digit_key(i as u32 + 1);
                if ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, digit))
                    && *head != current_head
                {
                    action = Some(Action::Apply(Command::SetRelationshipHead {
                        from: rel.from.clone(),
                        id: rel.id.clone(),
                        head: *head,
                    }));
                }
            }
        }
        ui.horizontal_wrapped(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("箭头")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
            for (i, (head, hint)) in heads.iter().enumerate() {
                let on = *head == current_head;
                let resp =
                    ui.selectable_label(on, egui::RichText::new(head.label()).size(Type::SMALL));
                // 形状证据：这四格的屏幕矩形（`MW_TRACE` 才格式化）。脚本要按坐标点它们，
                // 以前没有这行，只能靠猜——§3 最后一条缺口就是「检查器四格没有指针证据」。
                if crate::trace::is_enabled() {
                    let r = resp.rect;
                    crate::trace::event(&format!(
                        "inspector_arrow_cell head={} on={} rect={},{},{},{}",
                        head.label(),
                        on,
                        r.min.x,
                        r.min.y,
                        r.max.x,
                        r.max.y
                    ));
                }
                if resp
                    .on_hover_text(format!("{hint}（数字键 {}）", i + 1))
                    .clicked()
                    && !on
                {
                    action = Some(Action::Apply(Command::SetRelationshipHead {
                        from: rel.from.clone(),
                        id: rel.id.clone(),
                        head: *head,
                    }));
                }
            }
        });
        // ---- 线型与线宽（CM-16，`docs/30` §5.4.3）----
        //
        // 虚线是「暂定/推测」的表达手段：用户不必为了标注不确定而再造一个短语。
        // 线宽只给三档（细 / 标准 / 粗）——连续滑杆在 288 面板上既挤又难精确，
        // CmapTools 也只给几档。
        let style = doc
            .node(&rel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == rel.id));
        let current_dashed = style.map(|r| r.dashed).unwrap_or(false);
        let current_width = style.and_then(|r| r.width);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("线型")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
            for (dashed, label) in [(false, "实线"), (true, "虚线")] {
                let on = current_dashed == dashed;
                if ui
                    .selectable_label(on, egui::RichText::new(label).size(Type::SMALL))
                    .clicked()
                    && !on
                {
                    action = Some(Action::Apply(Command::SetRelationshipDashed {
                        from: rel.from.clone(),
                        id: rel.id.clone(),
                        dashed,
                    }));
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("线宽")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
            for (width, label) in [(Some(1.0f32), "细"), (None, "标准"), (Some(2.5), "粗")] {
                let on = match (current_width, width) {
                    (None, None) => true,
                    (Some(a), Some(b)) => (a - b).abs() < 0.01,
                    _ => false,
                };
                if ui
                    .selectable_label(on, egui::RichText::new(label).size(Type::SMALL))
                    .clicked()
                    && !on
                {
                    action = Some(Action::Apply(Command::SetRelationshipWidth {
                        from: rel.from.clone(),
                        id: rel.id.clone(),
                        width,
                    }));
                }
            }
        });
        ui.add_space(8.0);
        // 注释：多行、失焦提交（多行框里 Enter 是换行，不能当提交）。
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new("注释")
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.rel_note_draft)
                    .id(egui::Id::unique(("rel-note", &rel.id)))
                    .desired_width(168.0)
                    .desired_rows(2)
                    .hint_text("给这条命题加一句说明")
                    .frame(text_box_frame(ink))
                    .margin(egui::Margin::symmetric(7, 4)),
            );
            if resp.lost_focus() && self.rel_note_draft.trim() != self.rel_note_before {
                let text = self.rel_note_draft.trim().to_string();
                self.rel_note_before = text.clone();
                action = Some(Action::Apply(Command::SetRelationshipNote {
                    from: rel.from.clone(),
                    id: rel.id.clone(),
                    note: (!text.is_empty()).then_some(text),
                }));
            }
        });

        ui.add_space(10.0);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            if ui.button("删除关系线").clicked() {
                action = Some(Action::Apply(Command::DeleteRelationship {
                    from: rel.from.clone(),
                    id: rel.id.clone(),
                }));
            }
            if ui.button("取消选择").clicked() {
                action = Some(Action::ClearRelationship);
            }
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);
        let mut all: Vec<(&str, &str, String, String)> = Vec::new();
        for n in doc.nodes.values() {
            for r in &n.relationships {
                let to = doc
                    .node(&r.to)
                    .map(|t| t.text.trim())
                    .filter(|t| !t.is_empty())
                    .unwrap_or("未命名");
                let from = n.text.trim();
                all.push((
                    n.id.as_str(),
                    r.id.as_str(),
                    if from.is_empty() { "未命名" } else { from }.to_string(),
                    to.to_string(),
                ));
            }
        }
        all.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.3.cmp(&b.3)));
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new(format!("全部关系线 · {}", all.len()))
                    .size(Type::CAPTION)
                    .color(ink.ch.dim),
            );
        });
        ui.add_space(2.0);
        for (from, id, from_text, to_text) in all {
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                let on = from == rel.from && id == rel.id;
                if ui
                    .selectable_label(
                        on,
                        egui::RichText::new(format!("{from_text} → {to_text}")).size(Type::SMALL),
                    )
                    .clicked()
                {
                    action = Some(Action::SelectRelationship {
                        from: from.to_string(),
                        id: id.to_string(),
                    });
                }
            });
        }
        action
    }
}
