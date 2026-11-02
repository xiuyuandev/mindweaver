//! 概念图专属检查器（`docs/33` §6）。
//!
//! 概念图里检查器不再借用思维导图那三页（样式/内容/标记）——那三页的问题是
//! 树语义渗得到处都是（「含子树」「层级样式」「任务状态」）。这里是**概念图自己的**
//! 三段：
//!
//! - **概念**（选中 1 颗）：文字、形状、钉住位置、坐标；
//! - **命题**（选中 1 条）：短语、箭头、源 → 目标；
//! - **图**（没选中）：聚焦问题、整理布局、概念/命题计数。
//!
//! 分组键走 `concept/*`，与 `style/*` / `note/*` / `mark/*` **不共用**——
//! 折叠记忆不会串味，用户在思维导图里合上「颜色」不该让概念图的「概念」也合上。
//!
//! 三条纪律照旧：只读会话、只发命令；缓存只在 `doc.revision` / 选择集变化时重建；
//! 面板自己从不改文档。

use eframe::egui;
use mw_core::model::{ArrowHead, StyleOverride};
use mw_core::{Command, Document};

use super::focus::FocusMemory;
use super::geo;
use super::{color32, hex_of, Action, Extras, Inspector};

/// 概念图检查器的三段（`docs/33` §6）。
///
/// 抽成枚举是为了让「显示哪一段」变成一条可测的纯判定，
/// 而不是散在 UI 里的三个 `if`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConceptPanel {
    /// 选中 1 颗概念。
    Concept,
    /// 选中 1 条命题（优先于概念：改短语比改盒子更常见）。
    Proposition,
    /// 什么都没选：整张图的属性。
    Graph,
}

/// 概念图里检查器该显示哪一段（`docs/33` §6）。
///
/// 优先级：**命题 > 概念 > 图**。同时选中一条命题和几颗概念时命题赢；
/// 多于一颗概念时不硬塞「概念」页——批量改样式归思维导图那套，这里落到「图」。
pub(crate) fn concept_panel(concepts: usize, propositions: usize) -> ConceptPanel {
    if propositions >= 1 {
        ConceptPanel::Proposition
    } else if concepts == 1 {
        ConceptPanel::Concept
    } else {
        ConceptPanel::Graph
    }
}

impl Inspector {
    /// 概念图检查器：按选中情况分派到概念 / 命题 / 图 三段。
    pub(crate) fn concept_body(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        doc: &Document,
        selected: &[String],
        extras: Extras,
        theme: &mw_core::theme::Theme,
    ) -> Option<Action> {
        match concept_panel(selected.len(), extras.relationships.len()) {
            ConceptPanel::Proposition => match extras.relationship {
                Some(sel) => self.concept_proposition(ui, focus, doc, sel, theme),
                // 多选命题的批量页归 CM-17 那套；这里落到「图」，别让面板一片空白。
                None => self.concept_graph(ui, focus, doc),
            },
            ConceptPanel::Concept => self.concept_concept(ui, focus, doc, &selected[0]),
            ConceptPanel::Graph => self.concept_graph(ui, focus, doc),
        }
    }

    /// 概念段（`docs/33` §6.1）：一颗概念是什么、摆在哪。
    fn concept_concept(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        doc: &Document,
        id: &str,
    ) -> Option<Action> {
        let node = doc.node(id)?;
        let mut action = None;
        // 选中变了就把草稿对齐到真值，别让上一颗的概念留在框里。
        if self.concept_before != node.text {
            self.concept_draft = node.text.clone();
            self.concept_before = node.text.clone();
        }
        let (open, a) = self.group(ui, focus, "概念", &node.text, "concept/text", true);
        if let Some(a) = a {
            action = Some(a);
        }
        if open {
            ui.add_space(6.0);
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.concept_draft)
                    .id(egui::Id::unique("concept/text/field"))
                    .desired_width(ui.available_width())
                    .desired_rows(2)
                    .frame(crate::ui_kit::input_frame(self.ink.ch)),
            );
            if resp.lost_focus()
                || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                let t = self.concept_draft.trim().to_string();
                if !t.is_empty() && t != node.text {
                    self.concept_before = t.clone();
                    action = Some(Action::Apply(Command::SetText {
                        id: id.to_string(),
                        text: t,
                    }));
                }
            }
            ui.add_space(10.0);

            // ── 钉住位置（§6.1）──
            let was_pinned = node.pinned.unwrap_or(false);
            let mut pinned = was_pinned;
            ui.horizontal(|ui| {
                ui.label("钉住位置");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r =
                        crate::ui_kit::switch(ui, self.ink.ch, "concept/pinned", &mut pinned, true);
                    if r.changed() && pinned != was_pinned {
                        action = Some(Action::Apply(Command::SetConceptPinned {
                            id: id.to_string(),
                            pinned,
                        }));
                    }
                });
            });
            ui.add_space(10.0);

            // ── 形状（§6.1）──
            let shape = self
                .items
                .iter()
                .find(|i| i.id == id)
                .map(|i| shape_tag(i.resolved.shape))
                .unwrap_or("ellipse");
            ui.horizontal(|ui| {
                ui.label("形状");
                ui.add_space(8.0);
                for (label, want) in [("椭圆", "ellipse"), ("圆角", "rounded"), ("矩形", "rect")]
                {
                    let on = shape == want;
                    let r = ui.selectable_label(on, label);
                    if r.clicked() && !on {
                        action = Some(Action::Apply(Command::SetStyle {
                            ids: vec![id.to_string()],
                            style: Some(StyleOverride {
                                shape: Some(want.to_string()),
                                ..Default::default()
                            }),
                        }));
                    }
                }
            });
            ui.add_space(10.0);

            // ── 坐标（只读，§6.1）──
            if let Some(p) = &node.pos {
                ui.label(
                    egui::RichText::new(format!("位置  {:.1}, {:.1}", p.x, p.y))
                        .size(geo::GROUP_SUMMARY)
                        .color(self.ink.ch.faint),
                );
            }
            // ── 所属分组（只读，§6.1）──
            // 分组是文档级字段（`Document.groups`），一个概念最多属于一组
            // （`CreateGroup` 会把它从旧组摘掉），所以这里 `find` 就够了。
            let group = doc
                .groups
                .iter()
                .find(|g| g.members.iter().any(|m| m == id));
            ui.label(
                egui::RichText::new(match group {
                    Some(g) => format!("分组  {}", g.label),
                    None => "分组  未分组".to_string(),
                })
                .size(geo::GROUP_SUMMARY)
                .color(self.ink.ch.faint),
            );
            ui.add_space(10.0);
        }
        action
    }

    /// 命题段（`docs/33` §6.2）：一条连接是什么、什么样子、连到谁。
    fn concept_proposition(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        doc: &Document,
        sel: &crate::canvas::RelationshipSel,
        theme: &mw_core::theme::Theme,
    ) -> Option<Action> {
        let mut action = None;
        // 短语草稿跟着选中走。
        if self.rel_id.as_deref() != Some(sel.id.as_str()) {
            self.rel_id = Some(sel.id.clone());
            self.rel_draft = String::new();
            self.rel_before = String::new();
        }
        let rel = doc
            .node(&sel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == sel.id));
        let label = rel.and_then(|r| r.label.clone()).unwrap_or_default();
        if self.rel_before != label {
            self.rel_draft = label.clone();
            self.rel_before = label.clone();
        }
        let summary = if label.is_empty() {
            "未命名".to_string()
        } else {
            label
        };
        let (open, a) = self.group(ui, focus, "命题", &summary, "concept/prop", true);
        if let Some(a) = a {
            action = Some(a);
        }
        if open {
            ui.add_space(6.0);
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.rel_draft)
                    .id(egui::Id::unique("concept/prop/field"))
                    .desired_width(ui.available_width())
                    .hint_text("连接短语")
                    .frame(crate::ui_kit::input_frame(self.ink.ch)),
            );
            if resp.lost_focus()
                || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                let t = self.rel_draft.trim().to_string();
                if !t.is_empty() && t != self.rel_before {
                    self.rel_before = t.clone();
                    action = Some(Action::Apply(Command::SetRelationshipLabel {
                        from: sel.from.clone(),
                        id: sel.id.clone(),
                        label: Some(t),
                    }));
                }
            }
            ui.add_space(10.0);

            // ── 箭头（四档，`docs/30` §5.3）──
            let head = rel.map(|r| r.head).unwrap_or(ArrowHead::Forward);
            ui.horizontal(|ui| {
                ui.label("箭头");
                ui.add_space(8.0);
                for (label, want) in [
                    ("无", ArrowHead::Off),
                    ("→", ArrowHead::Forward),
                    ("←", ArrowHead::Backward),
                    ("↔", ArrowHead::Both),
                ] {
                    let on = head == want;
                    let r = ui.selectable_label(on, label);
                    if r.clicked() && !on {
                        action = Some(Action::Apply(Command::SetRelationshipHead {
                            from: sel.from.clone(),
                            id: sel.id.clone(),
                            head: want,
                        }));
                    }
                }
            });
            ui.add_space(10.0);

            // ── 颜色 / 线宽 / 虚线（§6.2）──
            let color = rel.and_then(|r| r.color.clone());
            let width = rel.and_then(|r| r.width);
            let dashed = rel.map(|r| r.dashed).unwrap_or(false);
            ui.horizontal(|ui| {
                ui.label("颜色");
                ui.add_space(8.0);
                // 跟随主题 + 六支固定色。命题是「这张图的判断」，不该有 12 色循环——
                // 那是节点分支色的语义。
                for (label, want) in [
                    ("默认", None),
                    ("红", Some("#c4644e")),
                    ("琥珀", Some("#b98a3e")),
                    ("绿", Some("#3e8e6e")),
                    ("蓝", Some("#5a6fa8")),
                    ("紫", Some("#8a5f86")),
                ] {
                    let on = color.as_deref() == want;
                    let r = ui.selectable_label(on, label);
                    if r.clicked() && !on {
                        action = Some(Action::Apply(Command::SetRelationshipColor {
                            from: sel.from.clone(),
                            id: sel.id.clone(),
                            color: want.map(|s| s.to_string()),
                        }));
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("线宽");
                ui.add_space(8.0);
                let mut w = width.unwrap_or(1.5);
                let r = ui.add(egui::Slider::new(&mut w, 0.5..=6.0).fixed_decimals(1));
                if r.changed() {
                    action = Some(Action::Apply(Command::SetRelationshipWidth {
                        from: sel.from.clone(),
                        id: sel.id.clone(),
                        width: Some(w),
                    }));
                }
            });
            ui.add_space(8.0);
            let mut dashed_now = dashed;
            ui.horizontal(|ui| {
                ui.label("虚线");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = crate::ui_kit::switch(
                        ui,
                        self.ink.ch,
                        "concept/prop/dashed",
                        &mut dashed_now,
                        true,
                    );
                    if r.changed() && dashed_now != dashed {
                        action = Some(Action::Apply(Command::SetRelationshipDashed {
                            from: sel.from.clone(),
                            id: sel.id.clone(),
                            dashed: dashed_now,
                        }));
                    }
                });
            });
            ui.add_space(10.0);

            // ── 备注（§6.2）：给这条命题的解释，不参与「命题」的读法 ──
            let note = rel.and_then(|r| r.note.clone()).unwrap_or_default();
            if self.rel_note_before != note {
                self.rel_note_draft = note.clone();
                self.rel_note_before = note.clone();
            }
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.rel_note_draft)
                    .id(egui::Id::unique("concept/prop/note"))
                    .desired_width(ui.available_width())
                    .desired_rows(2)
                    .hint_text("备注：这条命题的解释")
                    .frame(crate::ui_kit::input_frame(self.ink.ch)),
            );
            if resp.lost_focus() {
                let t = self.rel_note_draft.trim().to_string();
                if t != self.rel_note_before {
                    self.rel_note_before = t.clone();
                    action = Some(Action::Apply(Command::SetRelationshipNote {
                        from: sel.from.clone(),
                        id: sel.id.clone(),
                        note: (!t.is_empty()).then_some(t),
                    }));
                }
            }
            ui.add_space(10.0);

            // ── 线型 / 线宽（CM-16）──
            let dashed = rel.map(|r| r.dashed).unwrap_or(false);
            let width = rel.and_then(|r| r.width).unwrap_or(1.5);
            ui.horizontal(|ui| {
                ui.label("线型");
                ui.add_space(8.0);
                let mut d = dashed;
                if crate::ui_kit::switch(ui, self.ink.ch, "concept/prop/dashed", &mut d, true)
                    .changed()
                    && d != dashed
                {
                    action = Some(Action::Apply(Command::SetRelationshipDashed {
                        from: sel.from.clone(),
                        id: sel.id.clone(),
                        dashed: d,
                    }));
                }
                ui.add_space(10.0);
                for (label, w) in [("细", 1.0f32), ("中", 1.5), ("粗", 2.5)] {
                    let on = (width - w).abs() < 0.01;
                    if ui.selectable_label(on, label).clicked() && !on {
                        action = Some(Action::Apply(Command::SetRelationshipWidth {
                            from: sel.from.clone(),
                            id: sel.id.clone(),
                            width: Some(w),
                        }));
                    }
                }
            });
            ui.add_space(10.0);

            // ── 颜色 ──
            let color = rel.and_then(|r| r.color.clone());
            ui.horizontal(|ui| {
                ui.label("颜色");
                ui.add_space(8.0);
                if ui.selectable_label(color.is_none(), "跟随主题").clicked() && color.is_some()
                {
                    action = Some(Action::Apply(Command::SetRelationshipColor {
                        from: sel.from.clone(),
                        id: sel.id.clone(),
                        color: None,
                    }));
                }
                ui.spacing_mut().item_spacing.x = 4.0;
                for c in theme.palette.iter().take(4) {
                    let hex = hex_of(*c);
                    let on = color.as_deref() == Some(hex.as_str());
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::Vec2::splat(16.0), egui::Sense::click());
                    ui.painter().circle_filled(rect.center(), 7.0, color32(*c));
                    if on {
                        ui.painter().circle_stroke(
                            rect.center(),
                            8.0,
                            egui::Stroke::new(1.5, self.ink.ch.text),
                        );
                    }
                    if resp.clicked() && !on {
                        action = Some(Action::Apply(Command::SetRelationshipColor {
                            from: sel.from.clone(),
                            id: sel.id.clone(),
                            color: Some(hex),
                        }));
                    }
                }
            });
            ui.add_space(10.0);

            // ── 备注（CM-16）──
            let note = rel.and_then(|r| r.note.clone()).unwrap_or_default();
            if self.rel_note_before != note {
                self.rel_note_draft = note.clone();
                self.rel_note_before = note.clone();
            }
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.rel_note_draft)
                    .id(egui::Id::unique("concept/prop/note"))
                    .desired_width(ui.available_width())
                    .hint_text("备注")
                    .frame(crate::ui_kit::input_frame(self.ink.ch)),
            );
            if resp.lost_focus() {
                let t = self.rel_note_draft.trim().to_string();
                if t != self.rel_note_before {
                    self.rel_note_before = t.clone();
                    action = Some(Action::Apply(Command::SetRelationshipNote {
                        from: sel.from.clone(),
                        id: sel.id.clone(),
                        note: (!t.is_empty()).then_some(t),
                    }));
                }
            }
            ui.add_space(10.0);

            // ── 源 → 目标（可点跳转，§6.2）──
            ui.horizontal(|ui| {
                ui.label("连接");
                ui.add_space(8.0);
                let src = doc
                    .node(&sel.from)
                    .map(|n| n.text.clone())
                    .unwrap_or_default();
                if ui.link(src).clicked() {
                    action = Some(Action::SelectNode(sel.from.clone()));
                }
                ui.label(" → ");
                match doc.node(&sel.to) {
                    Some(dst) => {
                        if ui.link(dst.text.clone()).clicked() {
                            action = Some(Action::SelectNode(sel.to.clone()));
                        }
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("（目标已删除）")
                                .size(geo::GROUP_SUMMARY)
                                .color(self.ink.ch.faint),
                        );
                    }
                }
            });
            ui.add_space(10.0);
        }
        action
    }

    /// 图段（`docs/33` §6.3）：整张图在回答什么、怎么摆、有多大。
    fn concept_graph(
        &mut self,
        ui: &mut egui::Ui,
        focus: &mut FocusMemory,
        doc: &Document,
    ) -> Option<Action> {
        let mut action = None;
        let fq = doc.meta.layout.focus_question.clone().unwrap_or_default();
        if self.fq_before != fq {
            self.fq_draft = fq.clone();
            self.fq_before = fq.clone();
        }
        let summary = if fq.is_empty() { "未设" } else { "已设" };
        let (open, a) = self.group(ui, focus, "图", summary, "concept/graph", true);
        if let Some(a) = a {
            action = Some(a);
        }
        if open {
            ui.add_space(6.0);
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.fq_draft)
                    .id(egui::Id::unique("concept/graph/fq"))
                    .desired_width(ui.available_width())
                    .hint_text("这张图要回答什么？")
                    .frame(crate::ui_kit::input_frame(self.ink.ch)),
            );
            if resp.lost_focus()
                || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                let t = self.fq_draft.trim().to_string();
                if t != self.fq_before {
                    self.fq_before = t.clone();
                    action = Some(Action::Apply(Command::SetFocusQuestion {
                        text: (!t.is_empty()).then_some(t),
                    }));
                }
            }
            ui.add_space(12.0);
            if ui.button("整理布局").clicked() {
                action = Some(Action::ArrangeConcepts);
            }
            ui.add_space(10.0);
            let concepts = doc.nodes.len();
            let props: usize = doc.nodes.values().map(|n| n.relationships.len()).sum();
            ui.label(
                egui::RichText::new(format!("{concepts} 个概念 · {props} 条命题"))
                    .size(geo::GROUP_SUMMARY)
                    .color(self.ink.ch.faint),
            );
            ui.add_space(10.0);
        }
        action
    }
}

/// `ResolvedStyle.shape` → 三个稳定标签（供分段控件比对）。
fn shape_tag(s: mw_core::theme::Shape) -> &'static str {
    use mw_core::theme::Shape;
    match s {
        Shape::Rounded => "rounded",
        Shape::Rect => "rect",
        _ => "ellipse",
    }
}

#[cfg(test)]
mod tests {
    use super::{concept_panel, ConceptPanel};

    /// `docs/33` §6：哪一段由选中情况决定，优先级 命题 > 概念 > 图。
    #[test]
    fn concept_panel_picks_by_whats_selected() {
        assert_eq!(concept_panel(0, 0), ConceptPanel::Graph);
        assert_eq!(concept_panel(1, 0), ConceptPanel::Concept);
        assert_eq!(concept_panel(0, 1), ConceptPanel::Proposition);
        // 命题赢：改短语比改盒子更常见。
        assert_eq!(concept_panel(1, 1), ConceptPanel::Proposition);
        assert_eq!(concept_panel(3, 1), ConceptPanel::Proposition);
        // 多于一颗概念不硬塞「概念」页：批量改样式归思维导图那套。
        assert_eq!(concept_panel(2, 0), ConceptPanel::Graph);
    }
}
