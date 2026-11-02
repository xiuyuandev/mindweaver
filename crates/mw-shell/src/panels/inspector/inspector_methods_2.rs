use super::*;

impl Inspector {
    pub(crate) fn annotation_body(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        anns: &[crate::canvas::AnnotationSel],
    ) -> Option<Action> {
        use mw_core::model::AnnotationKind;
        let ann = anns.last()?;
        let mut action = None;
        let ink = self.ink;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new(ann.label())
                    .size(Type::BODY)
                    .family(FontFamily::Name(crate::fonts::SEMIBOLD.into()))
                    .color(ink.ch.text),
            );
            ui.label(
                egui::RichText::new(if anns.len() > 1 {
                    format!("已选 {} 条", anns.len())
                } else {
                    "已选中".to_string()
                })
                .size(Type::SMALL)
                .color(ink.ch.dim),
            );
        });
        if anns.len() > 1 {
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.label(
                    egui::RichText::new("多选：文字/跨度只改主选，颜色/删除作用于全部")
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
            });
        }
        ui.add_space(8.0);
        match ann.kind {
            AnnotationKind::Summary => {
                let text = doc
                    .node(&ann.parent)
                    .and_then(|p| p.summaries.iter().find(|s| s.id == ann.id))
                    .map(|s| s.text.clone())
                    .unwrap_or_default();
                let mut draft = text.clone();
                let mut commit = false;
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut draft)
                            .id(egui::Id::unique(("annotation-text", &ann.id)))
                            .desired_width(160.0)
                            .hint_text("概要文字")
                            .frame(text_box_frame(ink))
                            .margin(egui::Margin::symmetric(7, 4)),
                    );
                    crate::trace::name(ui.ctx(), resp.id, "annotation-text");
                    let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    commit = (enter || resp.lost_focus()) && draft.trim_end() != text;
                });
                if commit {
                    action = Some(Action::Apply(Command::SetSummaryText {
                        parent: ann.parent.clone(),
                        id: ann.id.clone(),
                        text: draft.trim_end().to_string(),
                    }));
                }
                // 跨度：父节点的每个直接子节点一个开关，点一下加入/移出概要。
                if let Some(parent) = doc.node(&ann.parent) {
                    let cur: Vec<String> = parent
                        .summaries
                        .iter()
                        .find(|s| s.id == ann.id)
                        .map(|s| s.targets.clone())
                        .unwrap_or_default();
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(GUTTER);
                        ui.label(
                            egui::RichText::new("跨度")
                                .size(Type::SMALL)
                                .color(ink.ch.dim),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(GUTTER);
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        for (i, child) in parent.children.iter().enumerate() {
                            let text = doc
                                .node(child)
                                .map(|n| {
                                    if n.text.trim().is_empty() {
                                        "未命名".to_string()
                                    } else {
                                        n.text.trim().chars().take(6).collect()
                                    }
                                })
                                .unwrap_or_default();
                            let on = cur.contains(child);
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(56.0, 22.0), egui::Sense::click());
                            crate::trace::name(ui.ctx(), resp.id, &format!("summary-target {i}"));
                            ui.painter().rect_filled(
                                rect,
                                6.0,
                                if on { ink.ch.accent_soft } else { ink.ch.hover },
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                text,
                                egui::FontId::proportional(Type::SMALL),
                                if on { ink.ch.accent } else { ink.ch.dim },
                            );
                            if resp.clicked() {
                                let mut next = cur.clone();
                                if on {
                                    next.retain(|t| t != child);
                                } else {
                                    next.push(child.clone());
                                }
                                // 至少保留一个目标：全去掉等于删概要，得用「删除」那条路。
                                if !next.is_empty() {
                                    action = Some(Action::Apply(Command::SetSummaryTargets {
                                        parent: ann.parent.clone(),
                                        id: ann.id.clone(),
                                        targets: next,
                                    }));
                                }
                            }
                        }
                    });
                }
            }
            AnnotationKind::Boundary => {
                let current = doc
                    .node(&ann.parent)
                    .and_then(|p| p.boundaries.iter().find(|b| b.id == ann.id))
                    .and_then(|b| b.color.clone());
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.spacing_mut().item_spacing.x = 4.0;
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
                            ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                        crate::trace::name(ui.ctx(), resp.id, &format!("annotation-color {k}"));
                        let center = rect.center();
                        match hex {
                            Some(h) => {
                                let c = Rgba::parse(h).map(color32).unwrap_or(ink.ch.dim);
                                ui.painter().circle_filled(center, 5.0, c);
                                if current.as_deref() == Some(h) {
                                    ui.painter().circle_stroke(
                                        center,
                                        6.8,
                                        Stroke::new(1.5, ink.ch.accent),
                                    );
                                }
                            }
                            None => {
                                ui.painter().circle_stroke(
                                    center,
                                    5.0,
                                    Stroke::new(1.0, ink.ch.faint),
                                );
                                if current.is_none() {
                                    ui.painter().circle_stroke(
                                        center,
                                        6.8,
                                        Stroke::new(1.5, ink.ch.accent),
                                    );
                                }
                            }
                        }
                        if resp.clicked() {
                            let cmds: Vec<Command> = anns
                                .iter()
                                .filter(|a| a.kind == AnnotationKind::Boundary)
                                .map(|a| Command::SetBoundaryColor {
                                    parent: a.parent.clone(),
                                    id: a.id.clone(),
                                    color: hex.map(|h| h.to_string()),
                                })
                                .collect();
                            action = wrap(cmds);
                        }
                    }
                });
                // 跨度：与概要同一套开关，只是换一条命令。
                let span = ann_span(doc, &ann.parent, ann.kind, &ann.id);
                if !span.is_empty() {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(GUTTER);
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        for (k, (child, on, label)) in span.iter().enumerate() {
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(44.0, 20.0), Sense::click());
                            crate::trace::name(ui.ctx(), resp.id, &format!("boundary-span {k}"));
                            ui.painter().rect_filled(
                                rect,
                                5.0,
                                if *on {
                                    ink.ch.accent_soft
                                } else {
                                    ink.ch.hover
                                },
                            );
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                label,
                                FontId::proportional(Type::SMALL),
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
                                    action = Some(Action::Apply(Command::SetBoundaryTargets {
                                        parent: ann.parent.clone(),
                                        id: ann.id.clone(),
                                        targets: next,
                                    }));
                                }
                            }
                        }
                    });
                }
            }
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            let del = ui.add(egui::Button::new(
                egui::RichText::new("删除").size(Type::BODY),
            ));
            crate::trace::name(ui.ctx(), del.id, "annotation-delete");
            if del.clicked() {
                let cmds: Vec<Command> = anns
                    .iter()
                    .map(|a| match a.kind {
                        AnnotationKind::Summary => Command::DeleteSummary {
                            parent: a.parent.clone(),
                            id: a.id.clone(),
                        },
                        AnnotationKind::Boundary => Command::DeleteBoundary {
                            parent: a.parent.clone(),
                            id: a.id.clone(),
                        },
                    })
                    .collect();
                action = wrap(cmds);
            }
            let esc = ui.add(egui::Button::new(
                egui::RichText::new("取消选择").size(Type::BODY),
            ));
            crate::trace::name(ui.ctx(), esc.id, "annotation-cancel");
            if esc.clicked() {
                action = Some(Action::ClearAnnotation);
            }
        });
        action
    }

    pub(crate) fn multi_hint(&self, ui: &mut egui::Ui, text: &str) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new(text)
                    .size(Type::SMALL)
                    .color(self.ink.ch.dim),
            );
        });
    }

    // ───────────────────────── 文档属性（无选中） ─────────────────────────

    pub(crate) fn doc_body(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        focus: &mut FocusMemory,
        presets: &[crate::theme_presets::Preset],
    ) -> Option<Action> {
        let mut action = None;
        let ink = self.ink;
        // 「聚焦问题」这一格要等失焦/回车才落盘，提交动作统一在区块之后发。
        let mut commit_focus = false;
        ui.spacing_mut().item_spacing.y = 2.0;
        // 无选中（§4.3）：一句话 + 一个文字动作，不铺任何属性控件。
        ui.add_space(12.0);
        {
            let (row, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
            let p = ui.painter();
            p.text(
                Pos2::new(row.left() + GUTTER, row.top() + 2.0),
                Align2::LEFT_TOP,
                "还没有选中主题",
                FontId::new(
                    geo::OBJ_TITLE,
                    FontFamily::Name(crate::fonts::SEMIBOLD.into()),
                ),
                ink.ch.text,
            );
            p.text(
                Pos2::new(row.left() + GUTTER, row.top() + 22.0),
                Align2::LEFT_TOP,
                "在画布或大纲里点一个主题，这里就换成它的属性",
                FontId::proportional(geo::OBJ_META),
                ink.ch.dim,
            );
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            if crate::ui_kit::text_button(
                ui,
                ink.ch,
                "选中第一个主题",
                crate::ui_kit::ButtonTone::Secondary,
                true,
            )
            .clicked()
            {
                action = Some(Action::SelectNode(doc.root_id.clone()));
            }
        });
        ui.add_space(6.0);
        let (open_0, act) = self.group(ui, focus, "文档属性", "只读", "doc/info", true);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_0 {
            ui.add_space(2.0);
            let title = if doc.meta.title.trim().is_empty() {
                "未命名"
            } else {
                doc.meta.title.trim()
            };
            info_row(ink, ui, "标题", title);
            info_row(ink, ui, "主题数", &doc.nodes.len().to_string());
            ui.add_space(6.0);
        }
        ui.separator();
        ui.add_space(4.0);

        // 主题
        let (open_1, act) = self.group(ui, focus, "主题", "4 套预设", "doc/theme", false);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_1 {
            ui.add_space(4.0);
            // 概念图看概念图那四套（`docs/33` §4）：两套**不混排**——
            // 混排等于「专属」没做，挑一个纸感主题进来，概念盒又会被
            // `apply_concept_defaults` 改回椭圆加面板底。
            let all = mw_core::theme::themes_for(doc);
            for row in all.chunks(2) {
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for t in row {
                        let selected = t.id == doc.meta.theme;
                        // 当前这张卡片要画的是**生效**的样子：设了自定义强调色时，卡片上的
                        // 调色板得已经转过色相，否则点一下「素白」预览是一套、画布是另一套。
                        let spun = selected
                            .then(|| t.with_accent(Rgba::parse_opt(&doc.meta.theme_accent)));
                        let look = spun.as_ref().unwrap_or(t);
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(112.0, 46.0), Sense::click());
                        let hovered = resp.hovered();
                        let painter = ui.painter();
                        painter.rect_filled(rect, 7.0, color32(look.ui.canvas_bg));
                        painter.rect_stroke(
                            rect,
                            7.0,
                            if selected {
                                Stroke::new(2.0, ink.ch.accent)
                            } else if hovered {
                                Stroke::new(1.0, ink.ch.border_hot)
                            } else {
                                Stroke::new(1.0, ink.ch.border)
                            },
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            Pos2::new(rect.left() + 9.0, rect.top() + 10.0),
                            Align2::LEFT_TOP,
                            t.name,
                            FontId::proportional(Type::BODY),
                            color32(look.ui.text),
                        );
                        for (k, c) in look.palette.iter().take(3).enumerate() {
                            painter.circle_filled(
                                Pos2::new(
                                    rect.left() + 12.0 + k as f32 * 11.0,
                                    rect.bottom() - 10.0,
                                ),
                                4.0,
                                color32(*c),
                            );
                        }
                        let resp = resp.on_hover_text(
                            if t.appearance == mw_core::theme::Appearance::Dark {
                                format!("{}（深色）", t.name)
                            } else {
                                t.name.to_string()
                            },
                        );
                        if hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() && !selected {
                            action = Some(Action::Apply(Command::SetTheme {
                                theme: t.id.to_string(),
                            }));
                        }
                    }
                });
                ui.add_space(6.0);
            }
            // ---- 我的主题：把「基础主题 + 微调 + 强调色」存成命名预设 ----
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                let save = ui
                    .add(egui::Button::new(
                        egui::RichText::new("存为我的主题").size(Type::BODY),
                    ))
                    .on_hover_text("保存当前主题与配色微调，之后可在任何文档里套用");
                crate::trace::name(ui.ctx(), save.id, "preset-save");
                if save.clicked() {
                    action = Some(Action::SavePreset);
                }
                ui.label(
                    egui::RichText::new(format!("已有 {}", presets.len()))
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
            });
            for (i, row) in presets.chunks(2).enumerate() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (k, preset) in row.iter().enumerate() {
                        let idx = i * 2 + k;
                        // 预览用「基础主题 + 预设覆盖」实时算，和套用后的画布一致。
                        let look = mw_core::theme::get_theme(&preset.theme)
                            .with_overrides(preset.overrides.as_ref());
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(112.0, 40.0), Sense::click());
                        // 语义名：Tab 到这里时轨迹里直接写「第几条预设」。
                        crate::trace::name(ui.ctx(), resp.id, &format!("preset {idx}"));
                        ui.painter()
                            .rect_filled(rect, 7.0, color32(look.ui.canvas_bg));
                        ui.painter().rect_stroke(
                            rect,
                            7.0,
                            if resp.hovered() {
                                Stroke::new(1.0, ink.ch.border_hot)
                            } else {
                                Stroke::new(1.0, ink.ch.border)
                            },
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            Pos2::new(rect.left() + 9.0, rect.top() + 8.0),
                            Align2::LEFT_TOP,
                            &preset.name,
                            FontId::proportional(Type::BODY),
                            color32(look.ui.text),
                        );
                        ui.painter().circle_filled(
                            Pos2::new(rect.left() + 12.0, rect.bottom() - 9.0),
                            4.0,
                            color32(look.level(0).fill),
                        );
                        ui.painter().circle_filled(
                            Pos2::new(rect.left() + 23.0, rect.bottom() - 9.0),
                            4.0,
                            color32(look.level(1).fill),
                        );
                        // 右上角的小 ×：删除这条预设
                        let del = egui::Rect::from_min_size(
                            Pos2::new(rect.right() - 18.0, rect.top() + 2.0),
                            Vec2::splat(16.0),
                        );
                        let del_resp =
                            ui.interact(del, egui::Id::unique(("preset-del", idx)), Sense::click());
                        crate::trace::name(ui.ctx(), del_resp.id, &format!("preset-del {idx}"));
                        crate::ui_kit::paint_x(
                            ui.painter(),
                            del.center(),
                            3.0,
                            egui::Stroke::new(
                                1.3,
                                if del_resp.hovered() {
                                    ink.ch.text
                                } else {
                                    ink.ch.dim
                                },
                            ),
                        );
                        if del_resp.clicked() {
                            action = Some(Action::DeletePreset(idx));
                        } else if resp.clicked() {
                            action = Some(Action::ApplyPreset(idx));
                        }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                });
            }
            ui.add_space(6.0);
        }
        ui.separator();
        ui.add_space(10.0);

        // 强调色：覆盖只借色相，所以这一行给的是「一圈色相取样」，不是几支现成的颜色。
        let (open_2, act) = self.group(ui, focus, "强调色", "只借色相", "doc/accent", false);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_2 {
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("作用域")
                        .size(Type::SMALL)
                        .color(ink.ch.dim),
                );
                if ui.selectable_label(!self.accent_selected, "整图").clicked() {
                    self.accent_selected = false;
                }
                let can_selected = !self.items.is_empty();
                if can_selected {
                    if ui
                        .selectable_label(
                            self.accent_selected,
                            format!("选中 {}", self.items.len()),
                        )
                        .on_hover_text("只改选中主题（含子树，若已勾选）的分支色")
                        .clicked()
                    {
                        self.accent_selected = true;
                    }
                } else {
                    ui.add_enabled(false, egui::Button::new("选中 0"));
                }
            });
            let mut accent_pick: Option<Option<Rgba>> = None;
            let chips = self.accent_chips.clone();
            let current = self.accent.hue();
            let nearest = chips
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    hue_gap(a.hue(), current).total_cmp(&hue_gap(b.hue(), current))
                })
                .map(|(i, _)| i);
            // 自然换行会在默认面板宽度留下最后一排单颗色点；先算每排装得下几颗，
            // 再把所有色点均匀分到各排（11 颗 → 6 + 5），面板变宽时自然回到一排。
            let max_cols = (((ui.available_width() - 12.0 + 4.0) / 26.0).floor() as usize).max(1);
            let rows = chips.len().div_ceil(max_cols);
            let cols = chips.len().div_ceil(rows).max(1);
            for (row_index, chunk) in chips.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
                    for (col_index, c) in chunk.iter().enumerate() {
                        let i = row_index * cols + col_index;
                        // 色点身份钉死：面板宽度动画时会改换行结果，自动 id 会漂。
                        let key = FocusKey::indexed("inspector/accent/chip", i);
                        let (_, rect) = ui.allocate_space(Vec2::splat(22.0));
                        let resp = ui.interact(rect, focus::widget_id(ui, &key), Sense::click());
                        focus.note(PanelId::Inspector, ui.ctx(), &key);
                        let on = nearest == Some(i);
                        let focused = resp.has_focus();
                        let p = ui.painter();
                        p.circle_filled(rect.center(), 7.5, color32(*c));
                        p.circle_stroke(rect.center(), 7.5, Stroke::new(1.0, ink.well));
                        p.circle_stroke(
                            rect.center(),
                            if on { 10.0 } else { 8.6 },
                            Stroke::new(
                                if on { 2.0 } else { 1.0 },
                                if on { ink.ch.accent } else { ink.ch.border_hot },
                            ),
                        );
                        // 键盘焦点自己画一圈：egui 只给内置控件画焦点，自定义绘制的圆盘不在其列。
                        // 用中性灰、半径压在选中环外一点，这样「当前生效」和「光标在这」两件事
                        // 能同时看见，不会互相冒充。Space/Enter 由 egui 视作一次主键点击。
                        if focused {
                            p.circle_stroke(rect.center(), 11.0, Stroke::new(1.2, ink.ch.dim));
                        }
                        // 语义名登记在自己的 id 上，帧末由 `trace::observe` 统一落盘。
                        // 名字沿用既有格式（真实输入探针按它断言），所以这里显式覆盖 `widget_id`
                        // 登记的名字，而不是换成 `FocusKey` 的默认写法。
                        if crate::trace::is_enabled() {
                            crate::trace::name(
                                ui.ctx(),
                                resp.id,
                                &format!("accent-chip {i} · {}°", ACCENT_HUES[i].round() as i32),
                            );
                        }
                        let resp = resp.on_hover_text(format!(
                            "{}° · {}",
                            ACCENT_HUES[i].round() as i32,
                            hex_of(*c)
                        ));
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            accent_pick = Some(Some(*c));
                        }
                    }
                });
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.spacing_mut().interact_size = Vec2::new(20.0, 20.0);
                // 取色按钮的方块在亮面板上会和白底糊在一起，跟色板一样用描边色兜住。
                ui.scope(|ui| {
                    let v = ui.visuals_mut();
                    v.widgets.inactive.bg_fill = ink.ch.faint;
                    v.widgets.hovered.bg_fill = ink.ch.dim;
                    v.widgets.active.bg_fill = ink.ch.dim;
                    let mut custom = self.accent_draft.unwrap_or_else(|| color32(self.accent));
                    let resp = ui
                        .color_edit_button_srgba(&mut custom)
                        .on_hover_text("自定义：只取这支颜色的色相");
                    crate::trace::name(ui.ctx(), resp.id, "accent-custom");
                    if resp.changed() {
                        self.accent_draft = Some(custom);
                        self.accent_dirty = true;
                    }
                });
                let reset = ui.add_enabled(
                    doc.meta.theme_accent.is_some(),
                    egui::Button::new(egui::RichText::new("主题默认").size(Type::BODY)),
                );
                crate::trace::name(ui.ctx(), reset.id, "accent-reset");
                if reset.clicked() {
                    accent_pick = Some(None);
                }
            });
            // 拖动取色器期间不发命令：一次拖动 `changed` 几十次，撤销栈会被同一件事填满。
            if self.accent_dirty && !ui.input(|i| i.pointer.any_down()) {
                self.accent_dirty = false;
                if let Some(c) = self.accent_draft.take() {
                    accent_pick = Some(Some(color_rgba(c)));
                }
            }
            body_label(
                ui,
                &match doc.meta.theme_accent.as_deref() {
                    Some(hex) => format!("自定义 {hex}"),
                    None => "跟随主题".to_string(),
                },
                12.0,
                ink.ch.dim,
            );
            if let Some(pick) = accent_pick {
                if self.accent_selected && !self.items.is_empty() {
                    let hex = pick.map(hex_of);
                    action = self.style_cmd(move |s| s.branch_color = hex.clone());
                } else {
                    action = Some(Action::Apply(Command::SetThemeAccent {
                        accent: pick.map(hex_of),
                    }));
                }
            }
            ui.add_space(10.0);
        }
        ui.separator();
        ui.add_space(10.0);

        // 对比度审计：WCAG AA 正文 4.5、AAA 7.0。只报数，不自动改配色。
        let theme = mw_core::theme::theme_for(&doc.meta);
        let (open_3, act) = self.group(
            ui,
            focus,
            "对比度",
            "AA 4.5 · AAA 7.0",
            "doc/contrast",
            false,
        );
        if let Some(a) = act {
            action = Some(a);
        }
        if open_3 {
            ui.add_space(6.0);
            contrast_row(ink, ui, "正文 / 面板", theme.ui.text, theme.ui.panel);
            contrast_row(ink, ui, "强调 / 面板", theme.ui.accent, theme.ui.panel);
            contrast_row(
                ink,
                ui,
                "一级主题文字 / 填充",
                theme.level(0).text,
                theme.level(0).fill,
            );
            ui.add_space(10.0);
        }
        ui.separator();
        ui.add_space(10.0);

        // 配色微调（文档级）：只覆盖三个槽位，其余仍由主题决定。
        let (open_4, act) = self.group(ui, focus, "配色微调", "只覆盖少数槽位", "doc/tune", false);
        if let Some(a) = act {
            action = Some(a);
        }
        if open_4 {
            ui.add_space(7.0);
            let overrides = doc.meta.theme_overrides.clone().unwrap_or_default();
            let mut pending: Option<(usize, Option<String>)> = None;
            let rows: [(usize, &str, &str, [&str; 6]); 7] = [
                (
                    0,
                    "画布",
                    overrides.canvas_bg.as_deref().unwrap_or(""),
                    [
                        "#ffffff", "#f7f7fa", "#f2f4f5", "#fad8df", "#111111", "#f6212d",
                    ],
                ),
                (
                    1,
                    "中心",
                    overrides.central_fill.as_deref().unwrap_or(""),
                    [
                        "#f6212d", "#318de1", "#1b1b1d", "#5a729a", "#0288d1", "#ffffff",
                    ],
                ),
                (
                    2,
                    "一级",
                    overrides.level1_fill.as_deref().unwrap_or(""),
                    [
                        "#8fd0bc", "#f09b94", "#f0be8c", "#a8bdea", "#3a3a3c", "#ffffff",
                    ],
                ),
                (
                    3,
                    "字·中心",
                    overrides.text_central.as_deref().unwrap_or(""),
                    [
                        "#1b1b1d", "#333333", "#5a729a", "#f6212d", "#318de1", "#ffffff",
                    ],
                ),
                (
                    4,
                    "字·分支",
                    overrides.text_main.as_deref().unwrap_or(""),
                    [
                        "#1b1b1d", "#333333", "#5a729a", "#f6212d", "#318de1", "#ffffff",
                    ],
                ),
                (
                    5,
                    "字·子级",
                    overrides.text_sub.as_deref().unwrap_or(""),
                    [
                        "#1b1b1d", "#333333", "#5a729a", "#f6212d", "#318de1", "#ffffff",
                    ],
                ),
                (
                    6,
                    "线色",
                    overrides.edge_color.as_deref().unwrap_or(""),
                    [
                        "#8a8f98", "#1b1b1d", "#318de1", "#0a7d3f", "#f6212d", "#c9a227",
                    ],
                ),
            ];
            for (slot, name, current, palette) in rows {
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.add_sized(
                        [52.0, 20.0],
                        egui::Label::new(
                            egui::RichText::new(name)
                                .size(Type::SMALL)
                                .color(ink.ch.dim),
                        ),
                    );
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for hex in palette {
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                        let c = Rgba::parse(hex).unwrap_or(Rgba::TRANSPARENT);
                        let center = rect.center();
                        ui.painter().circle_filled(center, 6.5, color32(c));
                        if current.eq_ignore_ascii_case(hex) {
                            ui.painter().circle_stroke(
                                center,
                                8.0,
                                Stroke::new(1.6, ink.ch.accent),
                            );
                        } else if resp.hovered() {
                            ui.painter()
                                .circle_stroke(center, 8.0, Stroke::new(1.0, ink.ch.dim));
                        }
                        if resp.clicked() {
                            pending = Some((slot, Some((*hex).to_string())));
                        }
                    }
                    // 恢复这一槽
                    if ui
                        .add_enabled(
                            !current.is_empty(),
                            egui::Button::new(egui::RichText::new("恢复").size(Type::SMALL)),
                        )
                        .clicked()
                    {
                        pending = Some((slot, None));
                    }
                });
                ui.add_space(3.0);
            }
            // 连线粗细：三档，「标准」= 清掉覆盖。
            let mut pending_edge: Option<Option<f32>> = None;
            let mut pending_taper: Option<Option<bool>> = None;
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.add_sized(
                    [32.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new("线宽")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    ),
                );
                ui.spacing_mut().item_spacing.x = 4.0;
                for (label, scale) in [("细", Some(0.7f32)), ("标准", None), ("粗", Some(1.5f32))]
                {
                    let on = match (overrides.edge_scale, scale) {
                        (None, None) => true,
                        (Some(a), Some(b)) => (a - b).abs() < 0.05,
                        _ => false,
                    };
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 20.0), Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        5.0,
                        if on { ink.ch.accent_soft } else { ink.ch.hover },
                    );
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(Type::SMALL),
                        if on { ink.ch.accent } else { ink.ch.dim },
                    );
                    if resp.clicked() {
                        pending_edge = Some(scale.map(Some).unwrap_or(None));
                    }
                }
            });
            if let Some(scale) = pending_edge {
                let mut next = overrides.clone();
                next.edge_scale = scale;
                action = Some(Action::Apply(Command::SetThemeOverrides {
                    overrides: (!next.is_empty()).then_some(next),
                }));
            }
            ui.horizontal(|ui| {
                ui.add_space(GUTTER);
                ui.add_sized(
                    [32.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new("线型")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    ),
                );
                ui.spacing_mut().item_spacing.x = 4.0;
                for (label, taper) in [("跟随", None), ("收细", Some(true)), ("等粗", Some(false))]
                {
                    let on = overrides.edge_taper == taper;
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(44.0, 20.0), Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        5.0,
                        if on { ink.ch.accent_soft } else { ink.ch.hover },
                    );
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(Type::SMALL),
                        if on { ink.ch.accent } else { ink.ch.dim },
                    );
                    if resp.clicked() {
                        pending_taper = Some(taper);
                    }
                }
            });
            if let Some(taper) = pending_taper {
                let mut next = overrides.clone();
                next.edge_taper = taper;
                action = Some(Action::Apply(Command::SetThemeOverrides {
                    overrides: (!next.is_empty()).then_some(next),
                }));
            }
            if let Some((slot, value)) = pending {
                let mut next = overrides.clone();
                match slot {
                    0 => next.canvas_bg = value,
                    1 => next.central_fill = value,
                    2 => next.level1_fill = value,
                    3 => next.text_central = value,
                    4 => next.text_main = value,
                    5 => next.text_sub = value,
                    _ => next.edge_color = value,
                }
                action = Some(Action::Apply(Command::SetThemeOverrides {
                    overrides: (!next.is_empty()).then_some(next),
                }));
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // 聚焦问题（CM-10 ⑥，`docs/32` §2）：概念图独有，所以只在概念图里出现。
            //
            // 这是**唯一的写入入口**：画布上那行字在没写的时候不存在，用户没有别的路
            // 把它写进去。反过来，一旦写了，双击画布顶部那行字就能就地改。
            if doc.is_concept_map() {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                egui::CollapsingHeader::new(
                    egui::RichText::new("聚焦问题")
                        .size(Type::SMALL)
                        .family(FontFamily::Name(crate::fonts::SEMIBOLD.into()))
                        .color(ink.ch.dim),
                )
                .id_salt("inspector/focus-question")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        ui.add_space(GUTTER);
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.focus_draft)
                                .id(egui::Id::unique("inspector/focus-question/field"))
                                .desired_width(ui.available_width() - GUTTER)
                                .hint_text("这张图要回答什么？")
                                .frame(text_box_frame(ink))
                                .margin(egui::Margin::symmetric(8, 5)),
                        );
                        // 这一格是文档属性，不是常驻表单：Enter 或点走就落盘，没有保存按钮。
                        // 保存按钮会让它看起来像「填完要提交」的对话框。
                        if resp.lost_focus()
                            || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            commit_focus = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(GUTTER);
                        ui.label(
                            egui::RichText::new("显示在画布顶部，双击可改")
                                .size(Type::SMALL)
                                .color(ink.ch.dim),
                        );
                    });
                    ui.add_space(8.0);
                });
            }
            if commit_focus {
                let current = doc.meta.layout.focus_question.clone().unwrap_or_default();
                if self.focus_draft.trim() != current.trim() {
                    crate::trace::event("focus-question");
                    action = Some(Action::Apply(Command::SetFocusQuestion {
                        text: Some(self.focus_draft.clone()),
                    }));
                } else {
                    // 没改就退出：把草稿恢复成文档值（含归一化后的空白差异）。
                    self.focus_draft = current;
                }
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // 布局（可折叠，默认展开）
            egui::CollapsingHeader::new(
                egui::RichText::new("布局")
                    .size(Type::SMALL)
                    .family(FontFamily::Name(crate::fonts::SEMIBOLD.into()))
                    .color(ink.ch.dim),
            )
            .id_salt("inspector/layout")
            .default_open(true)
            .show(ui, |ui| {
                ui.add_space(7.0);
                // 方向：只对思维导图有意义；其它布局是固定方向。
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.label(
                        egui::RichText::new("方向")
                            .size(Type::BODY)
                            .color(ink.ch.dim),
                    );
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (value, label) in [("auto", "自动"), ("right", "向右"), ("left", "向左")]
                    {
                        let key = FocusKey::dynamic("inspector/layout/direction", value);
                        if seg(
                            ui,
                            label,
                            self.layout_draft.direction == value,
                            Vec2::new(54.0, 24.0),
                            ink,
                            Some(&key),
                            focus,
                        ) {
                            self.layout_draft.direction = value.to_string();
                            action = Some(Action::Apply(Command::SetLayout {
                                layout: self.layout_draft.clone(),
                            }));
                        }
                    }
                });
                ui.add_space(6.0);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.label(
                        egui::RichText::new("线型")
                            .size(Type::BODY)
                            .color(ink.ch.dim),
                    );
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (value, label) in
                        [("curve", "曲线"), ("elbow", "直角"), ("straight", "直线")]
                    {
                        let key = FocusKey::dynamic("inspector/layout/edge", value);
                        if seg(
                            ui,
                            label,
                            self.layout_draft.edge_shape == value,
                            Vec2::new(54.0, 24.0),
                            ink,
                            Some(&key),
                            focus,
                        ) {
                            self.layout_draft.edge_shape = value.to_string();
                            action = Some(Action::Apply(Command::SetLayout {
                                layout: self.layout_draft.clone(),
                            }));
                        }
                    }
                    let mut arrow = self.layout_draft.edge_arrow;
                    if crate::ui_kit::switch(ui, ink.ch, "inspector/layout/arrow", &mut arrow, true)
                        .changed()
                    {
                        self.layout_draft.edge_arrow = arrow;
                        action = Some(Action::Apply(Command::SetLayout {
                            layout: self.layout_draft.clone(),
                        }));
                    }
                    ui.label(
                        egui::RichText::new("箭头")
                            .size(Type::BODY)
                            .color(ink.ch.text),
                    );
                });
                ui.add_space(6.0);
                // 五个布局入口分两行（3 + 2），避免在 260px 面板里挤成一行
                for row in [
                    &[
                        ("mindmap", "思维导图"),
                        ("logicRight", "逻辑图"),
                        ("orgChart", "组织架构"),
                    ][..],
                    &[("fishbone", "鱼骨图"), ("timeline", "时间轴")][..],
                ] {
                    ui.horizontal(|ui| {
                        ui.add_space(GUTTER);
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for &(kind, label) in row {
                            let key = FocusKey::dynamic("inspector/layout/kind", kind);
                            if seg(
                                ui,
                                label,
                                self.layout_draft.kind == kind,
                                Vec2::new(74.0, 26.0),
                                ink,
                                Some(&key),
                                focus,
                            ) {
                                self.layout_draft.kind = kind.to_string();
                                action = Some(Action::Apply(Command::SetLayout {
                                    layout: self.layout_draft.clone(),
                                }));
                            }
                        }
                    });
                }
                ui.add_space(8.0);
                let mut changed = false;
                for (label, value, range) in [
                    ("同级间距", &mut self.layout_draft.sibling_gap, 8.0..=120.0),
                    ("层级间距", &mut self.layout_draft.level_gap, 32.0..=200.0),
                    (
                        "最大节点宽",
                        &mut self.layout_draft.max_node_width,
                        120.0..=600.0,
                    ),
                ] {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(GUTTER);
                        ui.label(
                            egui::RichText::new(label)
                                .size(Type::BODY)
                                .color(ink.ch.dim),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new(format!("{} px", *value as i32))
                                    .size(Type::BODY)
                                    .color(ink.ch.text),
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        // 默认滑轨在近白面板上几乎看不见；用描边色当轨道，暗色下同样成立。
                        ui.scope(|ui| {
                            let v = ui.visuals_mut();
                            v.widgets.inactive.bg_fill = ink.ch.border;
                            v.widgets.hovered.bg_fill = ink.ch.border_hot;
                            v.widgets.active.bg_fill = ink.ch.border_hot;
                            v.selection.bg_fill = ink.ch.accent_soft;
                            v.slider_trailing_fill = true;
                            ui.spacing_mut().slider_rail_height = 5.0;
                            ui.spacing_mut().slider_width = (ui.available_width() - 24.0).max(60.0);
                            let resp = ui.add(
                                egui::Slider::new(value, range)
                                    .show_value(false)
                                    .trailing_fill(true),
                            );
                            if resp.changed() {
                                changed = true;
                            }
                        });
                    });
                }
                if changed {
                    self.layout_dirty = true;
                }
                // 拖动期间不发命令：撤销栈里应当只有一条"更换布局"。
                if self.layout_dirty && !ui.input(|i| i.pointer.any_down()) {
                    self.layout_dirty = false;
                    action = Some(Action::Apply(Command::SetLayout {
                        layout: self.layout_draft.clone(),
                    }));
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("布局参数随文档保存")
                            .size(Type::SMALL)
                            .color(ink.ch.dim),
                    );
                });
                ui.add_space(10.0);
            });
        }
        action
    }
}
