//! 设置中心：外观 / 编辑 / 自动保存 / 导出默认值 / 笔记库路径。
//!
//! 设置保存在 `settings.json`，应用时立即作用到画布、撤销栈、自动保存定时器、
//! 导出对话框与笔记库实例；不要求重启。

use super::*;

impl Shell {
    pub(crate) fn open_settings(&mut self) {
        self.settings_open = true;
    }

    pub(crate) fn settings_ui(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut s = self.settings.clone();
        // 验收旋钮（只改草稿、不落盘、不影响真实偏好）：让金样拍得到「调低会丢历史」。
        if let Some(n) = std::env::var("MW_SETTINGS_DRAFT_UNDO")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
        {
            s.undo_limit = n.clamp(mw_core::MIN_HISTORY_LIMIT, mw_core::MAX_HISTORY_LIMIT);
        }
        // 笔记库路径会显示在对话框里：金样换一台机器/换一个 APPDATA，那行字就变，
        // 基线会随环境漂。给个固定值，路径行才是确定的（只改草稿）。
        if let Some(dir) = std::env::var_os("MW_SETTINGS_NOTES_DIR") {
            s.notes_dir = dir.to_string_lossy().to_string();
        }
        let saved_limit = self.settings.undo_limit();
        let history_depth = self
            .canvas
            .as_ref()
            .map(|c| c.session().doc.undo_depth() + c.session().doc.redo_depth())
            .unwrap_or(0);
        let ch = self.chrome;
        let old_notes = self.notes.as_ref().map(|n| n.root().to_path_buf());
        let actions = [
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
            dialog::DialogAction::new("保存并应用", ui_kit::ButtonTone::Primary),
        ];
        let spec = dialog::DialogSpec::new("设置")
            .width(520.0)
            .default_action(1);
        let out = dialog::show(ctx, ch, "settings", spec, &actions, |ui| {
            section_label(ch, ui, "外观");
            ui_kit::switch_row(
                ui,
                ch,
                "settings/reduce-motion",
                &mut s.reduce_motion,
                "减少动画（无过渡、立即落位）",
            );
            ui.label(
                egui::RichText::new("覆盖系统“减少动态效果”设置；截图 harness 仍强制关闭动画。")
                    .size(Type::BODY)
                    .color(ch.dim),
            );
            ui.add_space(10.0);

            section_label(ch, ui, "自动保存");
            ui.add(
                egui::Slider::new(&mut s.autosave_secs, 1.0..=30.0)
                    .text("间隔（秒）")
                    .step_by(1.0),
            );
            ui.label(
                egui::RichText::new("只影响恢复点写入频率；Ctrl+S 仍然立即写正本。")
                    .size(Type::BODY)
                    .color(ch.dim),
            );
            ui.add_space(10.0);

            section_label(ch, ui, "编辑");
            ui.add(
                egui::Slider::new(
                    &mut s.undo_limit,
                    mw_core::MIN_HISTORY_LIMIT..=mw_core::MAX_HISTORY_LIMIT,
                )
                .text("撤销栈上限（步）")
                .step_by(100.0),
            );
            ui.label(
                egui::RichText::new("调低会立即丢掉最旧的记录；上限是偏好，不写进 .zmind。")
                    .size(Type::BODY)
                    .color(ch.dim),
            );
            // M28b：把「要丢几步」在按下保存之前说清楚（28px 警告行；不丢时不占位）。
            let dropped =
                crate::settings::undo_limit_dropped(saved_limit, s.undo_limit, history_depth);
            if dropped > 0 {
                let (row, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 28.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(row, 5.0, ch.warn.gamma_multiply(0.10));
                ui.painter().text(
                    egui::pos2(row.left() + 10.0, row.center().y),
                    Align2::LEFT_CENTER,
                    format!("调低会立即丢弃最旧的 {dropped} 步撤销记录"),
                    FontId::proportional(Type::SMALL),
                    ch.warn,
                );
            }
            ui.add_space(10.0);

            section_label(ch, ui, "导出默认值");
            ui.add(
                egui::Slider::new(&mut s.export_scale, 0.5..=8.0)
                    .text("默认倍率")
                    .step_by(0.5),
            );
            ui_kit::switch_row(
                ui,
                ch,
                "settings/export-opaque",
                &mut s.export_opaque,
                "默认不透明背景",
            );
            ui.add_space(10.0);

            section_label(ch, ui, "笔记库路径");
            ui_kit::text_input(
                ui,
                ch,
                "settings/notes-dir",
                &mut s.notes_dir,
                "留空 = 默认目录",
                f32::INFINITY,
            );
            if let Some(root) = s.notes_root() {
                ui.label(
                    egui::RichText::new(format!("当前：{}", root.display()))
                        .size(Type::SMALL)
                        .color(ch.dim),
                );
            }
            ui.label(
                egui::RichText::new("改路径后立即重新打开笔记库；不移动旧文件。")
                    .size(Type::BODY)
                    .color(ch.dim),
            );
        });
        // 警告行只在「真的会丢记录」时出现；轨迹只在步数变化时记一笔（M28b 的机器判据）。
        let warned = crate::settings::undo_limit_dropped(saved_limit, s.undo_limit, history_depth);
        let stamp = (warned, history_depth);
        if warned > 0 && self.settings_warn_log != Some(stamp) {
            self.settings_warn_log = Some(stamp);
            crate::trace::event(&format!(
                "settings warning dropped={warned} depth={history_depth} limit={}",
                s.undo_limit
            ));
        }
        let save = out.action == Some(1);
        let cancel = out.closed && out.action != Some(1);
        if cancel {
            self.settings_open = false;
        } else if save {
            self.settings = s;
            self.settings.save();
            let limit = self.settings.undo_limit();
            self.apply_history_limit(limit);
            crate::trace::event(&format!("settings save undo_limit={limit}"));
            if let Some(canvas) = self.canvas.as_mut() {
                canvas.set_reduce_motion(system_reduce_motion() || self.settings.reduce_motion);
            }
            self.export.scale = self.settings.export_scale.clamp(0.5, 8.0);
            self.export.opaque = self.settings.export_opaque;
            export_prefs::Prefs::capture(
                self.export.fmt,
                self.export.scope,
                self.export.scale,
                self.export.opaque,
            )
            .save();
            let new_notes = self.settings.notes_root();
            if new_notes != old_notes {
                self.notes = new_notes.map(note_store::NoteStore::open_at);
                self.refresh_library();
            }
            self.settings_open = false;
            self.set_toast("设置已保存并生效", false);
        }
    }

    /// 把撤销栈上限作用到活动画布与所有后台标签。
    /// 上限是进程级偏好：同一份设置对所有打开的文档立即生效，不按文档记。
    pub(crate) fn apply_history_limit(&mut self, limit: usize) {
        if let Some(canvas) = self.canvas.as_mut() {
            let changed = canvas.session().doc.history_limit() != limit;
            canvas.set_history_limit(limit);
            if changed {
                let doc = &canvas.session().doc;
                crate::trace::event(&format!(
                    "history limit={} depth={}/{}",
                    doc.history_limit(),
                    doc.undo_depth(),
                    doc.redo_depth()
                ));
            }
        }
        for tab in &mut self.tabs {
            tab.session.doc.set_history_limit(limit);
        }
    }

    /// 每帧对账：新开的文档 / 新标签默认是 500，设置值一变就立即跟上；
    /// 值没变时 `set_history_limit` 只做一次 clamp 与空循环，开销可忽略。
    pub(crate) fn sync_history_limit(&mut self) {
        let limit = self.settings.undo_limit();
        self.apply_history_limit(limit);
    }
}
