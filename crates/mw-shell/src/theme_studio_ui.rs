//! 主题编辑器窗口与即时预览接线。

use super::*;
use crate::theme_studio::{self, Preset, Studio};
use mw_core::theme::{Rgba, Theme};

/// 把主题色按画布底色合成成不透明色：弹窗里没有 alpha 混合，直接画会露出面板灰。
fn to_color32(c: Rgba, bg: Rgba) -> egui::Color32 {
    let a = c.0[3] as f32 / 255.0;
    let mix = |i: usize| (c.0[i] as f32 * a + bg.0[i] as f32 * (1.0 - a)).round() as u8;
    egui::Color32::from_rgb(mix(0), mix(1), mix(2))
}

/// 四块色卡：画布 / 中心 / 一级 / 连线。改任何槽位都立刻反映在这里。
fn preview_strip(ui: &mut egui::Ui, ch: Chrome, theme: &Theme) {
    let canvas = theme.ui.canvas_bg;
    let level1 = theme.level(1);
    let tiles = [
        ("画布", canvas, theme.ui.text),
        ("中心", theme.level(0).fill, theme.level(0).text),
        ("一级", level1.fill, level1.text),
        (
            "连线",
            canvas,
            theme.palette_at(0).unwrap_or(theme.ui.border),
        ),
    ];
    ui.horizontal(|ui| {
        for (label, fill, ink) in tiles {
            ui.vertical(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(126.0, 44.0), egui::Sense::hover());
                let bg = to_color32(fill, canvas);
                let p = ui.painter();
                p.rect_filled(rect, 7.0, bg);
                p.rect_stroke(
                    rect,
                    7.0,
                    egui::Stroke::new(1.0, to_color32(theme.ui.border, canvas)),
                    egui::StrokeKind::Inside,
                );
                let ink = to_color32(ink, fill);
                if label == "连线" {
                    let y = rect.center().y;
                    let pad = 18.0;
                    p.line_segment(
                        [
                            egui::pos2(rect.left() + pad, y),
                            egui::pos2(rect.right() - pad, y),
                        ],
                        egui::Stroke::new((theme.edge_width * 1.4).clamp(1.5, 5.0), ink),
                    );
                    p.circle_filled(egui::pos2(rect.right() - pad, y), 3.0, ink);
                } else {
                    let y = rect.center().y;
                    let pad = 16.0;
                    p.line_segment(
                        [
                            egui::pos2(rect.left() + pad, y - 5.0),
                            egui::pos2(rect.right() - pad - 22.0, y - 5.0),
                        ],
                        egui::Stroke::new(2.6, ink),
                    );
                    p.line_segment(
                        [
                            egui::pos2(rect.left() + pad, y + 5.0),
                            egui::pos2(rect.right() - pad - 48.0, y + 5.0),
                        ],
                        egui::Stroke::new(2.0, ink.gamma_multiply(0.55)),
                    );
                }
                ui.add_space(3.0);
                ui.label(egui::RichText::new(label).size(Type::CAPTION).color(ch.dim));
            });
            ui.add_space(4.0);
        }
    });
}

impl Shell {
    pub(crate) fn open_theme_studio(&mut self) {
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        let meta = &canvas.session().doc.meta;
        let mut studio = Studio::new(meta.theme.clone(), meta.theme_overrides.clone());
        // QA 钩子：确定性的草稿 + 一个可套用的预设，方便实拍与人工复验。
        if std::env::var_os("MW_THEME_STUDIO").is_some() {
            studio.name = "暖夜".into();
            studio.draft.canvas_bg = Some("#101216".into());
            studio.draft.central_fill = Some("#2F6F5E".into());
            studio.draft.level1_fill = Some("#3A6E62".into());
            studio.draft.text_sub = Some("#C9D2CE".into());
            studio.draft.edge_color = Some("#7FA79A".into());
            studio.draft.edge_scale = Some(1.3);
            studio.draft.edge_taper = Some(false);
            if studio.presets.is_empty() {
                studio.presets.push(Preset {
                    name: "暖夜".into(),
                    base: studio.base.clone(),
                    overrides: studio.draft.clone(),
                });
                studio.presets.push(Preset {
                    name: "墨线".into(),
                    base: "ink".into(),
                    overrides: mw_core::ThemeOverrides {
                        edge_color: Some("#C9A86A".into()),
                        edge_scale: Some(0.8),
                        ..Default::default()
                    },
                });
            }
        }
        self.theme_studio = Some(studio);
    }

    fn apply_studio(&mut self, base: &str, overrides: Option<mw_core::ThemeOverrides>) {
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        let theme = canvas.session().doc.meta.theme.clone();
        if theme != base {
            canvas.apply_command(mw_core::Command::SetTheme {
                theme: base.to_string(),
            });
        }
        canvas.apply_command(mw_core::Command::SetThemeOverrides { overrides });
    }

    pub(crate) fn theme_studio_ui(&mut self, ctx: &egui::Context) {
        let ch = self.chrome;
        let Some(studio) = self.theme_studio.as_mut() else {
            return;
        };
        let mut action: Option<StudioAction> = None;
        let mut base = studio.base.clone();
        let actions = [
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
            dialog::DialogAction::new("完成", ui_kit::ButtonTone::Primary),
        ];
        let spec = dialog::DialogSpec::new("主题编辑器")
            .width(620.0)
            .default_action(1);
        let out = dialog::show(ctx, ch, "theme-studio", spec, &actions, |ui| {
            ui.horizontal(|ui| {
                section_label(ch, ui, "基于");
                let names: Vec<&str> = mw_core::theme::THEME_IDS
                    .iter()
                    .map(|id| mw_core::get_theme(id).name)
                    .collect();
                let current = mw_core::theme::THEME_IDS.iter().position(|id| *id == base);
                if let Some(i) = crate::ui_kit::select(
                    ui,
                    ch,
                    "studio-base",
                    crate::ui_kit::SelectSpec {
                        current,
                        selected_text: mw_core::get_theme(&base).name,
                        options: &names,
                        font: Type::BODY,
                        width: None,
                    },
                ) {
                    let id = mw_core::theme::THEME_IDS[i];
                    base = id.to_string();
                    action = Some(StudioAction::Base(id.to_string()));
                }
                ui.add_space(10.0);
                if studio.is_dirty() {
                    ui.colored_label(ch.warn, "● 有改动（关闭时保存到文档）");
                } else {
                    ui.colored_label(ch.dim, "与打开时一致");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(format!("预设 {}", studio.presets.len()));
                });
            });
            ui.horizontal(|ui| {
                if ui.button("保存预设").clicked() {
                    action = Some(StudioAction::SavePreset);
                }
                if ui.button("清空微调").clicked() {
                    action = Some(StudioAction::ClearOverrides);
                }
                if ui.button("恢复打开时").clicked() {
                    action = Some(StudioAction::Restore);
                }
            });
            ui.add_space(4.0);
            preview_strip(ui, ch, &studio.resolved());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("预设名");
                let name = ui.add(
                    egui::TextEdit::singleline(&mut studio.name)
                        .desired_width(240.0)
                        .hint_text("给这套微调起个名字"),
                );
                if name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    action = Some(StudioAction::SavePreset);
                }
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(348.0);
                    section_label(ch, ui, "微调槽位");
                    section_label(ch, ui, "界面");
                    theme_studio::color_slot(ui, ch, "画布底色", &mut studio.draft.canvas_bg);
                    theme_studio::color_slot(
                        ui,
                        ch,
                        "中心主题填充",
                        &mut studio.draft.central_fill,
                    );
                    theme_studio::color_slot(ui, ch, "一级主题填充", &mut studio.draft.level1_fill);
                    section_label(ch, ui, "文字");
                    theme_studio::color_slot(ui, ch, "全部文字（旧）", &mut studio.draft.text);
                    theme_studio::color_slot(ui, ch, "中心文字", &mut studio.draft.text_central);
                    theme_studio::color_slot(ui, ch, "一级文字", &mut studio.draft.text_main);
                    theme_studio::color_slot(ui, ch, "二级及更深文字", &mut studio.draft.text_sub);
                    section_label(ch, ui, "连线");
                    theme_studio::color_slot(ui, ch, "统一连线色", &mut studio.draft.edge_color);
                    ui.horizontal(|ui| {
                        ui.label("线宽倍率");
                        let mut scale = studio.draft.edge_scale.unwrap_or(1.0);
                        if ui
                            .add(egui::Slider::new(&mut scale, 0.4..=2.5).step_by(0.1))
                            .changed()
                        {
                            studio.draft.edge_scale = Some(scale);
                        }
                        if ui.button("跟随").clicked() {
                            studio.draft.edge_scale = None;
                        }
                    });
                    ui.horizontal(|ui| {
                        let mut taper = studio.draft.edge_taper.unwrap_or(true);
                        if crate::ui_kit::switch(ui, ch, "studio/edge-taper", &mut taper, true)
                            .changed()
                        {
                            studio.draft.edge_taper = Some(taper);
                        }
                        ui.label("分支线收细");
                        if ui.button("跟随").clicked() {
                            studio.draft.edge_taper = None;
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(214.0);
                    section_label(ch, ui, "已保存预设（点名字套用）");
                    if studio.presets.is_empty() {
                        ui.weak("保存当前微调后，会出现在这里。");
                    }
                    for (i, p) in studio.presets.iter().enumerate() {
                        if studio.pending_delete == Some(i) {
                            ui.horizontal(|ui| {
                                ui.colored_label(ch.warn, "删除这个预设？");
                                if ui.button("保留").clicked() {
                                    studio.pending_delete = None;
                                }
                                if ui
                                    .button(egui::RichText::new("删除").color(ch.warn))
                                    .clicked()
                                {
                                    action = Some(StudioAction::DeletePreset(i));
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                // 固定宽度名区 + 紧跟其后的删除钮：右对齐布局会贴到
                                // 窗口滚动条命中带上，点 ✕ 会被滚动条吞掉。
                                let name = ui.add_sized(
                                    [170.0, 24.0],
                                    egui::Button::new(&p.name)
                                        .frame(true)
                                        .wrap_mode(egui::TextWrapMode::Truncate),
                                );
                                if name.clicked() {
                                    action = Some(StudioAction::ApplyPreset(i));
                                }
                                if crate::ui_kit::icon_button(
                                    ui,
                                    ch,
                                    24.0,
                                    crate::icons::Icon::Close,
                                    "删除预设",
                                ) {
                                    studio.pending_delete = Some(i);
                                }
                            });
                            ui.weak(format!("基于 {}", mw_core::get_theme(&p.base).name));
                            ui.add_space(4.0);
                        }
                    }
                });
            });
        });

        if out.action == Some(1) {
            action = Some(StudioAction::Done);
        } else if out.closed {
            action = Some(StudioAction::Cancel);
        }

        if base != studio.base {
            studio.base = base.clone();
            action = Some(StudioAction::Base(base));
        }

        let Some(studio) = self.theme_studio.as_mut() else {
            return;
        };
        let had_action = action.is_some();
        match action {
            Some(StudioAction::Base(id)) => {
                studio.base = id.clone();
                let draft = studio.draft.clone();
                studio.last_applied = Some(draft.clone());
                self.apply_studio(&id, Some(draft));
            }
            Some(StudioAction::SavePreset) => match studio.upsert_named() {
                Ok(replaced) => {
                    let name = studio.name.clone();
                    match theme_studio::save_presets(&studio.presets) {
                        Ok(()) => self.set_toast(
                            if replaced {
                                format!("已更新预设「{name}」")
                            } else {
                                format!("已保存预设「{name}」")
                            },
                            false,
                        ),
                        Err(e) => self.set_toast(format!("保存预设失败：{e}"), true),
                    }
                }
                Err(e) => self.set_toast(e, true),
            },
            Some(StudioAction::ApplyPreset(i)) => {
                if let Some(p) = studio.use_preset(i) {
                    studio.last_applied = Some(p.overrides.clone());
                    self.apply_studio(&p.base, Some(p.overrides));
                    self.set_toast(format!("已套用预设「{}」", p.name), false);
                }
            }
            Some(StudioAction::DeletePreset(i)) => {
                if let Some(p) = studio.delete_at(i) {
                    match theme_studio::save_presets(&studio.presets) {
                        Ok(()) => self.set_toast(format!("已删除预设「{}」", p.name), false),
                        Err(e) => self.set_toast(format!("写回预设失败：{e}"), true),
                    }
                }
            }
            Some(StudioAction::ClearOverrides) => {
                studio.clear_overrides();
            }
            Some(StudioAction::Restore) => {
                studio.restore_baseline();
                let base = studio.base.clone();
                let baseline = studio.draft.clone();
                studio.last_applied = Some(baseline.clone());
                self.apply_studio(&base, Some(baseline));
                self.set_toast("已恢复打开时", false);
            }
            Some(StudioAction::Done) => {
                self.theme_studio = None;
                self.set_toast("主题微调已保留在文档里", false);
            }
            Some(StudioAction::Cancel) => {
                let base = studio.baseline_base.clone();
                let baseline = studio.baseline.clone();
                self.theme_studio = None;
                self.apply_studio(&base, baseline);
            }
            None => {}
        }
        if had_action {
            ctx.request_repaint();
        }

        // 槽位每帧变化都即时打到文档；只在真的变了时才入一次撤销。
        if let Some(studio) = self.theme_studio.as_mut() {
            if studio.last_applied.as_ref() != Some(&studio.draft) {
                let draft = studio.draft.clone();
                let base = studio.base.clone();
                studio.last_applied = Some(draft.clone());
                self.apply_studio(&base, Some(draft));
            }
        }
    }
}

enum StudioAction {
    Base(String),
    SavePreset,
    ApplyPreset(usize),
    DeletePreset(usize),
    ClearOverrides,
    Restore,
    Done,
    Cancel,
}
