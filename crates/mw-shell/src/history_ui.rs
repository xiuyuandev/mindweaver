//! 版本历史窗口：列出 `.history` 快照并恢复。

use super::*;

impl Shell {
    pub(crate) fn open_history(&mut self) {
        let Some(path) = self.canvas.as_ref().and_then(|c| c.session().path.clone()) else {
            self.set_toast("先保存文档，才有版本历史", true);
            return;
        };
        let snaps = history::list(&path);
        if snaps.is_empty() {
            self.set_toast("还没有历史快照；保存一次后会出现", false);
            return;
        }
        self.history = Some((path, snaps));
    }

    pub(crate) fn history_ui(&mut self, ctx: &egui::Context) {
        let Some((doc, snaps)) = self.history.clone() else {
            return;
        };
        let mut restore: Option<usize> = None;
        let ch = self.chrome;
        let actions = [dialog::DialogAction::new(
            "关闭",
            ui_kit::ButtonTone::Secondary,
        )];
        let spec = dialog::DialogSpec::new("版本历史")
            .width(480.0)
            .default_action(0);
        let out = dialog::show(ctx, self.chrome, "history", spec, &actions, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} · 最近 {} 份（每份文档最多保留 10 份，全部在本机）",
                    doc.file_name().and_then(|s| s.to_str()).unwrap_or("文档"),
                    snaps.len()
                ))
                .size(Type::BODY)
                .color(ch.dim),
            );
            ui.separator();
            for (i, s) in snaps.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} 前",
                            human_age(mw_core::now_ms() as u128, s.modified_ms)
                        ))
                        .size(Type::BODY)
                        .color(ch.text),
                    );
                    ui.label(
                        egui::RichText::new(format!("{:.1} KB", s.bytes as f64 / 1024.0))
                            .size(Type::CAPTION)
                            .color(ch.dim),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("恢复此版本").clicked() {
                            restore = Some(i);
                        }
                    });
                });
                ui.separator();
            }
        });
        if out.closed {
            self.history = None;
        } else if let Some(i) = restore {
            if let Some(s) = snaps.get(i) {
                match history::restore(&doc, &s.path) {
                    Ok(()) => match Session::open(&doc) {
                        Ok(session) => {
                            if let Some(canvas) = self.canvas.as_mut() {
                                canvas.replace_session(session);
                            }
                            // 守卫跟着文档走：恢复后磁盘内容换了，指纹要重取（不再有窗口级字段）。
                            if let Some(g) = self.guard_mut() {
                                g.refresh_print();
                            }
                            // 恢复历史版本也是一次写盘成功：被顶掉的那一版该进 `.history`、
                            // 恢复点该作废、最近文件与基线该刷新（SR-02 问题 5）。
                            self.after_saved();
                            self.home = false;
                            self.history = None;
                            self.set_toast("已恢复到该历史版本", false);
                        }
                        Err(e) => self.set_toast(format!("重新打开失败：{e}"), true),
                    },
                    Err(e) => self.set_toast(format!("恢复失败：{e}"), true),
                }
            }
        }
    }
}

fn human_age(now_ms: u128, then_ms: u128) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 60 {
        format!("{secs} 秒")
    } else if secs < 3600 {
        format!("{} 分钟", secs / 60)
    } else if secs < 86_400 {
        format!("{} 小时", secs / 3600)
    } else {
        format!("{} 天", secs / 86_400)
    }
}
