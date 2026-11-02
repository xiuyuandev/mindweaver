use super::*;

impl Shell {
    /// 轮询后台导入；完成时开新标签并弹丢失项报告（如果有）。
    pub(crate) fn import_tick(&mut self, ctx: &egui::Context) {
        let done = self.import_job.as_mut().and_then(|job| job.poll());
        if done.is_none() {
            if self.import_job.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            return;
        }
        self.import_job = None;
        match done.expect("上面已判空") {
            import_job::Done::Ok { session, report } => {
                let name = session
                    .import_name()
                    .map(str::to_string)
                    .unwrap_or_else(|| "导入的文档".to_string());
                self.stash_active();
                self.tabs.push(Tab::new(*session));
                self.active = self.tabs.len() - 1;
                self.load_active();
                self.search.close();
                self.palette.close();
                self.home = false;
                self.home_return = false;
                if report.is_empty() {
                    self.set_toast(format!("已导入 {name}（记得另存为 .zmind）"), false);
                } else {
                    self.set_toast(
                        format!("已导入 {name}，有 {} 条提示", report.warnings.len()),
                        false,
                    );
                    self.import_warnings = Some(report.warnings);
                }
            }
            import_job::Done::Failed(e) => self.set_toast(e, true),
            import_job::Done::Cancelled => self.set_toast("已取消导入", false),
        }
    }

    pub(crate) fn import_ui(&mut self, ctx: &egui::Context) {
        // 导入提示的优先级高于导入进度（overlay::Modal 的声明顺序）：两者同时为真时
        // 只画提示，否则栈顶判定与画面就会各说各话。
        if let Some(warnings) = self.import_warnings.clone() {
            let actions = [dialog::DialogAction::new(
                "知道了",
                ui_kit::ButtonTone::Primary,
            )];
            let spec = dialog::DialogSpec::new("导入提示")
                .width(460.0)
                .default_action(0);
            let out = dialog::show(ctx, self.chrome, "import-warnings", spec, &actions, |ui| {
                ui.label("以下内容没有完全迁移，原文件未被修改：");
                for w in &warnings {
                    ui.label(format!("• {w}"));
                }
            });
            if out.closed {
                self.import_warnings = None;
            }
            return;
        }
        let Some(job) = self.import_job.as_ref() else {
            return;
        };
        let label = job.label().to_string();
        let secs = job.elapsed().as_secs_f32();
        let actions = [dialog::DialogAction::new(
            "取消导入",
            ui_kit::ButtonTone::Secondary,
        )];
        let spec = dialog::DialogSpec::new("正在导入")
            .width(420.0)
            .dismissible(false)
            .default_action(0);
        let out = dialog::show(ctx, self.chrome, "import-busy", spec, &actions, |ui| {
            ui.label(egui::RichText::new(format!("{label} · {secs:.1}s")).size(Type::BODY));
        });
        if out.action == Some(0) {
            if let Some(job) = self.import_job.take() {
                job.cancel();
            }
            self.set_toast("已取消导入", false);
        }
    }

    /// 标签管理：全库标签列表 + 重命名。重命名同时改当前文档（可撤销）与本地库。
    pub(crate) fn onboarding_ui(&mut self, ctx: &egui::Context) {
        if !self.onboarding {
            return;
        }
        let actions = [
            dialog::DialogAction::new("跳过", ui_kit::ButtonTone::Secondary),
            dialog::DialogAction::new("开始使用", ui_kit::ButtonTone::Primary),
        ];
        let spec = dialog::DialogSpec::new("欢迎使用织思")
            .width(460.0)
            .default_action(1);
        let out = dialog::show(ctx, self.chrome, "onboarding", spec, &actions, |ui| {
            ui.label(
                egui::RichText::new("本地优先的思维导图与卡片笔记，打开即用。")
                    .size(Type::BODY)
                    .color(self.chrome.dim),
            );
            ui.add_space(4.0);
            for (title, body) in [
                ("先画结构", "Tab 加子主题，Enter 加同级；双击或 F2 改名。"),
                (
                    "再写卡片",
                    "F9 打开检查器，给主题挂一张 Markdown 卡片；Ctrl+K 找命令。",
                ),
                (
                    "都在本地",
                    "自动保存；Ctrl+S 手动存；卡片库、标签与全文搜索都在本机。",
                ),
            ] {
                ui.label(egui::RichText::new(title).size(Type::BODY).family(
                    eframe::egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()),
                ));
                ui.label(
                    egui::RichText::new(body)
                        .size(Type::SMALL)
                        .color(self.chrome.dim),
                );
                ui.add_space(6.0);
            }
        });
        if out.closed {
            self.finish_onboarding();
        }
    }

    pub(crate) fn finish_onboarding(&mut self) {
        self.onboarding = false;
        self.window_state.set_onboarded(true);
    }

    pub(crate) fn tag_manager_ui(&mut self, ctx: &egui::Context) {
        let ch = self.chrome;
        let Some(manager) = self.tag_manager.as_mut() else {
            return;
        };
        let tags = self
            .notes
            .as_ref()
            .map(|n| n.all_tags())
            .unwrap_or_default();
        let mut apply: Option<(String, String)> = None;
        let actions = [dialog::DialogAction::new(
            "关闭",
            ui_kit::ButtonTone::Secondary,
        )];
        let spec = dialog::DialogSpec::new("标签管理")
            .width(460.0)
            .default_action(0);
        let out = dialog::show(ctx, ch, "tag-manager", spec, &actions, |ui| {
            if let Some(from) = manager.from.clone() {
                let target_exists = tags
                    .iter()
                    .any(|(t, _)| t.eq_ignore_ascii_case(manager.to.trim()));
                ui.label(
                    egui::RichText::new(if target_exists {
                        format!("把 #{from} 合并到已有标签：")
                    } else {
                        format!("把 #{from} 重命名为：")
                    })
                    .size(Type::BODY)
                    .color(ch.text),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut manager.to)
                        .font(FontId::proportional(Type::LABEL))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !manager.to.trim().is_empty(),
                            egui::Button::new(if target_exists { "合并" } else { "重命名" }),
                        )
                        .clicked()
                    {
                        apply = Some((from.clone(), manager.to.clone()));
                    }
                    if ui.button("取消").clicked() {
                        manager.from = None;
                    }
                });
            } else if tags.is_empty() {
                ui.label(
                    egui::RichText::new("还没有标签。在卡片里输入标签后会出现在这里。")
                        .size(Type::BODY)
                        .color(ch.dim),
                );
            } else {
                for (tag, count) in &tags {
                    ui.horizontal(|ui| {
                        let depth = tag.matches('/').count();
                        if depth > 0 {
                            ui.add_space((depth * 12) as f32);
                        }
                        let leaf = tag.rsplit('/').next().unwrap_or(tag);
                        let color = if depth == 0 {
                            note_view::tag_color(tag)
                        } else {
                            note_view::tag_color(tag).gamma_multiply(0.88)
                        };
                        ui.label(egui::RichText::new("●").size(Type::CAPTION).color(color));
                        ui.label(
                            egui::RichText::new(if depth == 0 {
                                format!("#{tag}")
                            } else {
                                format!("↳ #{leaf}")
                            })
                            .size(Type::BODY)
                            .color(ch.text),
                        )
                        .on_hover_text(format!("#{tag}"));
                        ui.label(
                            egui::RichText::new(format!("{count} 张"))
                                .size(Type::CAPTION)
                                .color(ch.dim),
                        );
                        if ui.small_button("重命名").clicked() {
                            manager.from = Some(tag.clone());
                            manager.to = tag.clone();
                        }
                    });
                }
            }
        });
        let close = out.closed;
        if let Some((from, to)) = apply {
            let from_now = from.trim().to_string();
            let to_now = to.trim().to_string();
            let doc_ok = self.canvas.as_mut().is_some_and(|c| {
                c.apply_command(mw_core::Command::RenameTag {
                    from: from_now.clone(),
                    to: to_now.clone(),
                })
            });
            let lib = self
                .notes
                .as_mut()
                .map(|n| n.rename_tag(&from_now, &to_now));
            if doc_ok {
                self.sync_notes();
            }
            match lib {
                Some(Ok(n)) => self.set_toast(format!("已重命名 #{from_now}（{n} 张卡片）"), false),
                Some(Err(e)) => self.set_toast(e, true),
                None => {}
            }
            self.tag_manager = None;
        } else if close {
            self.tag_manager = None;
        }
    }

    /// 打开笔记库页。
    pub(crate) fn open_library(&mut self) {
        self.close_light_overlays();
        self.note_reader = None;
        let Some(notes) = self.notes.as_ref() else {
            self.set_toast("本地卡片库不可用", true);
            return;
        };
        let cards = notes.metas();
        let (sidebar_w, list_w) = self.window_state.library_widths();
        self.library = Some(note_library::NoteLibrary::new(cards, sidebar_w, list_w));
    }

    pub(crate) fn refresh_library(&mut self) {
        let Some(notes) = self.notes.as_ref() else {
            return;
        };
        let cards = notes.metas();
        if let Some(lib) = self.library.as_mut() {
            lib.sync(cards);
        }
    }

    /// 从库打开一张卡片：优先读本地库的完整正文，库不在时退回当前文档内嵌副本。
    pub(crate) fn open_library_card(&mut self, id: &str) {
        let card = self.notes.as_ref().and_then(|n| n.get(id)).or_else(|| {
            self.canvas
                .as_ref()
                .and_then(|c| c.session().doc.card(id).cloned())
        });
        match card {
            Some(card) => {
                let reader = self.card_reader_for(&card);
                self.close_light_overlays();
                self.library = None;
                self.reader_from_library = true;
                self.note_reader = Some(reader);
                // CER-20：记下这张卡在磁盘上的指纹 + 可写性，之后的保存才谈得上「外部修改」。
                self.refresh_card_disk_state(id);
            }
            None => self.set_toast("这张卡片已经不存在", true),
        }
    }

    /// 库内卡片对应的磁盘文件；文档内卡片返回 `None`（它们存在 `.zmind` 里）。
    pub(crate) fn note_card_path(&self, id: &str) -> Option<std::path::PathBuf> {
        let notes = self.notes.as_ref()?;
        let meta = notes.meta(id)?;
        Some(notes.root().join(meta.file.as_str()))
    }

    /// 打开库内卡片后记下磁盘指纹，并顺手探一次可写性（`docs/28` §6.7「只读」）。
    ///
    /// 用「追加打开」探写权限：不会改动文件，也不会像 `metadata().permissions()` 那样
    /// 在只读目录/被占用时给出乐观答案。
    pub(crate) fn refresh_card_disk_state(&mut self, id: &str) {
        self.card_overwrite = false;
        self.card_disk_print = None;
        let Some(path) = self.note_card_path(id) else {
            return;
        };
        self.card_disk_print = file_guard::fingerprint(&path);
        let writable = std::fs::OpenOptions::new().append(true).open(&path).is_ok();
        if let Some(r) = self.note_reader.as_mut() {
            r.set_read_only(!writable);
        }
    }

    /// 「重新加载」：丢掉本页草稿，按磁盘上的内容重开这一张卡。
    pub(crate) fn reload_card_from_disk(&mut self, id: &str) {
        let from_library = self.note_card_path(id).is_some();
        let card = if from_library {
            self.notes.as_ref().and_then(|n| n.get(id))
        } else {
            self.canvas
                .as_ref()
                .and_then(|c| c.session().doc.card(id).cloned())
        };
        let Some(card) = card else {
            self.set_toast("磁盘上没有这张卡片了", true);
            return;
        };
        crate::trace::event(&format!("card_reload id={id}"));
        self.note_reader = Some(self.card_reader_for(&card));
        self.reader_from_library = from_library;
        self.refresh_card_disk_state(id);
    }

    /// 卡片页的写盘：上一帧读者页把请求排进 `pending_card_save`，这一帧开头才真正落盘。
    /// 失败不关页面、不丢草稿——读者页挂「保存失败」，用户接着打字或 `Ctrl+S` 再试。
    pub(crate) fn pump_card_save(&mut self) {
        let Some(save) = self.pending_card_save.take() else {
            return;
        };
        let Some(id) = self.note_reader.as_ref().map(|r| r.card_id.clone()) else {
            return;
        };
        if self.note_reader.as_ref().is_some_and(|r| r.is_read_only()) {
            if let Some(reader) = self.note_reader.as_mut() {
                reader.mark_save_failed("只读");
            }
            self.set_toast("这张卡片只读：请另存为", true);
            return;
        }
        // 外部修改：磁盘在打开/上次写盘之后被别人改过，就先别覆盖，挂行内提示条。
        if !self.card_overwrite {
            if let Some(known) = self.card_disk_print {
                if let Some(path) = self.note_card_path(&id) {
                    if file_guard::changed_since(Some(known), file_guard::fingerprint(&path)) {
                        crate::trace::event(&format!("card_conflict id={id}"));
                        if let Some(reader) = self.note_reader.as_mut() {
                            // 退出「保存中…」：这一笔没写，顶栏该回到「编辑中」+ 提示条。
                            reader.clear_saving();
                            reader.set_conflict(true);
                        }
                        // 保存请求作废：等用户在提示条上选。
                        self.pending_card_save = None;
                        self.ctx.request_repaint();
                        return;
                    }
                }
            }
        }
        self.card_overwrite = false;
        if self.save_card_from_reader(&id, &save.title, &save.body, &save.tags) {
            self.sync_notes();
            if let Some(reader) = self.note_reader.as_mut() {
                reader.mark_saved();
            }
            if let Some(path) = self.note_card_path(&id) {
                self.card_disk_print = file_guard::fingerprint(&path);
            }
            crate::trace::event(&format!("card_save_ok id={id} close={}", save.close));
            if save.close {
                self.note_reader = None;
                if std::mem::take(&mut self.reader_from_library) {
                    self.open_library();
                }
            }
        } else {
            if let Some(reader) = self.note_reader.as_mut() {
                reader.mark_save_failed("写入失败");
            }
            crate::trace::event(&format!("card_save_fail id={id}"));
            self.set_toast("卡片保存失败，草稿还在", true);
        }
    }

    /// 保存阅读页草稿：文档内卡片走可撤销命令；库内独立卡片（每日笔记）直接写库。
    pub(crate) fn save_card_from_reader(
        &mut self,
        id: &str,
        title: &str,
        body: &str,
        tags: &[String],
    ) -> bool {
        let doc_card = self
            .canvas
            .as_ref()
            .and_then(|c| c.session().doc.card(id).cloned());
        if let Some(card) = doc_card {
            let mut commands = Vec::new();
            if card.title != title {
                commands.push(mw_core::Command::SetCardTitle {
                    id: id.to_string(),
                    title: title.to_string(),
                });
            }
            if card.body != body {
                commands.push(mw_core::Command::SetCardBody {
                    id: id.to_string(),
                    body: body.to_string(),
                });
            }
            let normalized = mw_core::NoteCard::normalize_tags(tags);
            if card.tags != normalized {
                commands.push(mw_core::Command::SetCardTags {
                    id: id.to_string(),
                    tags: normalized,
                });
            }
            if commands.is_empty() {
                return true;
            }
            let cmd = if commands.len() == 1 {
                commands.pop().expect("至少一条")
            } else {
                mw_core::Command::Batch { commands }
            };
            return self.canvas.as_mut().is_some_and(|c| c.apply_command(cmd));
        }
        let Some(mut card) = self.notes.as_ref().and_then(|n| n.get(id)) else {
            return false;
        };
        card.title = title.to_string();
        card.body = body.to_string();
        card.tags = mw_core::NoteCard::normalize_tags(tags);
        card.updated_at = mw_core::now_ms();
        self.notes.as_mut().is_some_and(|n| n.upsert(&card).is_ok())
    }

    /// 给当前主选主题一键生成卡片并直接进编辑。
    pub(crate) fn capture_card_for_selected(&mut self) {
        let Some(id) = self
            .canvas
            .as_mut()
            .and_then(|c| c.capture_card_for_primary())
        else {
            self.set_toast("先选中一个主题", true);
            return;
        };
        self.sync_notes();
        let card = self
            .canvas
            .as_ref()
            .and_then(|c| c.session().doc.card(&id).cloned())
            .or_else(|| self.notes.as_ref().and_then(|n| n.get(&id)));
        if let Some(card) = card {
            let reader = self.card_reader_for(&card);
            self.close_light_overlays();
            self.library = None;
            self.reader_from_library = false;
            self.note_reader = Some(reader);
        }
    }

    /// 打开今天的每日笔记：同一天重复点只回到同一张卡。
    pub(crate) fn open_daily_note(&mut self) {
        let now = mw_core::now_ms();
        let date = note_view::format_date(now);
        let Some(notes) = self.notes.as_mut() else {
            self.set_toast("本地卡片库不可用", true);
            return;
        };
        let card = match notes.daily_note(&date, now) {
            Ok(c) => c,
            Err(e) => {
                self.set_toast(e, true);
                return;
            }
        };
        let reader = self.card_reader_for(&card);
        self.close_light_overlays();
        self.library = None;
        self.reader_from_library = false;
        self.note_reader = Some(reader);
    }

    /// 构造阅读页：带标签补全、来源节点状态、反链。
    pub(crate) fn card_reader_for(&self, card: &mw_core::NoteCard) -> note_view::CardReader {
        let mut reader = note_view::CardReader::new(card);
        reader.set_reduce_motion(self.reduce_motion || self.harness.is_some());
        reader.set_available_tags(self.known_tags());
        if let Some(canvas) = self.canvas.as_ref() {
            let doc = &canvas.session().doc;
            let backlinks = doc.backlinks_for_card(&card.id);
            let concept_map = doc.is_concept_map();
            let source = card.source_node.as_ref().map(|nid| {
                let node = doc.node(nid);
                let same_doc = card.source_doc == doc.id;
                note_view::SourceLink {
                    node_id: nid.clone(),
                    text: node
                        .map(|n| n.text.clone())
                        .filter(|t| !t.trim().is_empty())
                        .unwrap_or_else(|| {
                            if concept_map {
                                "未命名概念".into()
                            } else {
                                "未命名主题".into()
                            }
                        }),
                    missing: node.is_none(),
                    // 跨文档时我们手里没有那份文档，类型也就无从谈起；
                    // 只有同一份文档才敢说「这是概念图」。
                    concept_map: same_doc && concept_map,
                    doc_label: if same_doc {
                        doc.display_title().to_string()
                    } else {
                        "另一份文档".into()
                    },
                }
            });
            reader.set_context(source, backlinks);
            if doc.cards().iter().any(|c| c.id == card.id) {
                let (_, broken) = doc.card_refs(&card.body);
                reader.set_broken_refs(broken);
            }
        }
        reader
    }

    /// 库内已有标签 + 当前文档卡片标签，用于补全。
    pub(crate) fn known_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .notes
            .as_ref()
            .map(|n| n.all_tags().into_iter().map(|(t, _)| t).collect())
            .unwrap_or_default();
        if let Some(canvas) = self.canvas.as_ref() {
            for c in canvas.session().doc.cards() {
                for t in &c.tags {
                    if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                        tags.push(t.clone());
                    }
                }
            }
        }
        tags
    }

    /// 笔记库要的「来源」措辞：卡片 id → 它在当前文档里的位置。
    ///
    /// 库面板只认卡片索引、拿不到 `Document`，所以「概念图名 · 概念名」这种
    /// 要查节点的说法必须在这里算好。跨文档的卡片**不进表**——我们手里没有
    /// 那份文档，编不出名字，让库退回「另一份文档」比撒谎强。
    pub(crate) fn card_sources(
        &self,
    ) -> std::collections::HashMap<String, note_library::CardSource> {
        let Some(canvas) = self.canvas.as_ref() else {
            return Default::default();
        };
        let doc = &canvas.session().doc;
        let concept_map = doc.is_concept_map();
        let doc_label = doc.display_title();
        let mut out = std::collections::HashMap::new();
        for n in doc.nodes.values() {
            let Some(cid) = n.card_id.as_ref() else {
                continue;
            };
            let text = if n.text.trim().is_empty() {
                if concept_map {
                    "未命名概念"
                } else {
                    "未命名主题"
                }
                .to_string()
            } else {
                n.text.trim().to_string()
            };
            out.insert(
                cid.clone(),
                note_library::CardSource {
                    doc_label: doc_label.to_string(),
                    node_id: n.id.clone(),
                    node_text: text,
                    concept_map,
                },
            );
        }
        out
    }

    /// 从卡片页 / 笔记库跳回来源节点：关掉浮层、选中它、把焦点交还画布。
    ///
    /// 卡片页的「回到概念」按钮与笔记库的「回到概念」按钮**走同一条路**——
    /// 两处各写一遍的话，「跳回来选中谁」迟早会不一样。返回 `false` = 源节点
    /// 不在当前文档（跨文档），调用方自己决定提示什么。
    pub(crate) fn jump_to_source(&mut self, id: &str) -> bool {
        let here = self
            .canvas
            .as_ref()
            .is_some_and(|c| c.session().doc.node(id).is_some());
        if !here {
            return false;
        }
        let concept = self
            .canvas
            .as_ref()
            .is_some_and(|c| c.session().doc.is_concept_map());
        crate::trace::event(&format!(
            "card-jump node={id} concept={}",
            u8::from(concept)
        ));
        // 与搜索跳转共用揭示逻辑：来源在折叠子树里先展开，在聚焦视图外先退聚焦。
        let ok = self.canvas.as_mut().is_some_and(|canvas| {
            let ok = canvas.reveal_and_select(id, "card");
            if ok {
                canvas.request_canvas_focus();
            }
            ok
        });
        if ok {
            self.note_reader = None;
            self.reader_from_library = false;
            self.library = None;
        }
        ok
    }

    pub(crate) fn library_ui(&mut self, ctx: &egui::Context) {
        if self.library.is_none() {
            return;
        }
        // 「挂到当前主题」的目标：画布主选。没有主选时菜单项禁用而不是静默失败。
        let attach_target = self.canvas.as_ref().and_then(|c| {
            let id = c.selected_ids().last()?.clone();
            c.node_text(&id).map(str::to_string)
        });
        // 移出确认框只在它是栈顶时绘制；设置/历史等压在上面时保留状态但不画。
        let top_modal = self.overlay_state().top_modal();
        let remove_top = top_modal == Some(crate::overlay::Modal::LibraryRemove);
        let keys_owned = note_library::page_keys_owned(top_modal.is_some(), self.dialog_demo);
        let sources = self.card_sources();
        // 库页的颜色一律从内容令牌派生（§7.2 起），不从 `Chrome` 取：
        // `chrome_of` 的亮色分支把 `panel` 写死成纯白，四套主题会塌成同一张灰白纸。
        let theme = self
            .canvas
            .as_ref()
            .map(|c| c.session().theme.clone())
            .unwrap_or_else(|| mw_core::get_theme("snow"));
        let (sidebar_w, list_w) = self.window_state.library_widths();
        let store_errors = self.notes.as_ref().map(|n| n.errors()).unwrap_or(0);
        // 库还没打开：画 §7.7 的三行骨架，不画空态（空态说的是「一张卡都没有」）。
        let loading = self.notes.is_none();
        let action = match (self.library.as_mut(), self.notes.as_mut()) {
            (Some(lib), notes) => lib.show(
                ctx,
                &self.chrome,
                notes,
                note_library::Extras {
                    theme: &theme,
                    attach_target: attach_target.as_deref(),
                    remove_prompt_top: remove_top,
                    keys_owned,
                    sources: &sources,
                    sidebar_w,
                    list_w,
                    store_errors,
                    loading,
                },
            ),
            _ => None,
        };
        // 拖宽落手才写回会话状态（§7.2）：每帧回写会在一手拖动里写几十次盘。
        if let Some((sw, lw)) = self.library.as_mut().and_then(|l| l.take_width_change()) {
            self.window_state.set_library_widths(sw, lw);
        }
        match action {
            Some(note_library::LibraryAction::Close) => self.library = None,
            Some(note_library::LibraryAction::Open(id)) => self.open_library_card(&id),
            Some(note_library::LibraryAction::NewCard) => self.capture_card_for_selected(),
            Some(note_library::LibraryAction::JumpToNode(node)) => {
                let concept = self
                    .canvas
                    .as_ref()
                    .is_some_and(|c| c.session().doc.is_concept_map());
                if self.jump_to_source(&node) {
                    self.set_toast(
                        if concept {
                            "已定位到来源概念"
                        } else {
                            "已定位到来源主题"
                        },
                        false,
                    );
                } else if let Some(lib) = self.library.as_mut() {
                    lib.flash(if concept {
                        "来源概念不在当前文档里，卡片仍在笔记库"
                    } else {
                        "来源主题不在当前文档里，卡片仍在笔记库"
                    });
                }
            }
            Some(note_library::LibraryAction::ManageTags) => {
                self.library = None;
                self.tag_manager = Some(TagManager::default());
            }
            // 「挂到当前主题」：旧版把它叫 Archive，和 §7.1 的归档语义撞了，
            // 现在归档（收起来）与挂节点（整理进图）是两条独立动作。
            Some(note_library::LibraryAction::Attach(ids)) => {
                let Some((doc_id, node_id)) = self.canvas.as_ref().and_then(|c| {
                    let node = c.selected_ids().last()?.clone();
                    Some((c.session().doc.id.clone(), node))
                }) else {
                    self.set_toast("先在画布上选中一个主题", true);
                    return;
                };
                let now = mw_core::now_ms();
                let mut ok = 0;
                if let Some(notes) = self.notes.as_mut() {
                    for id in &ids {
                        if notes.attach_to_node(id, &doc_id, &node_id, now).is_ok() {
                            ok += 1;
                        }
                    }
                }
                self.refresh_library();
                self.set_toast(format!("已挂到当前主题：{ok} 张"), ok == 0);
            }
            Some(note_library::LibraryAction::Archive(ids)) => {
                self.library_batch(ids, true, "归档");
            }
            Some(note_library::LibraryAction::Unarchive(ids)) => {
                self.library_batch(ids, false, "取消归档");
            }
            Some(note_library::LibraryAction::AddTag(ids, tag)) => {
                let now = mw_core::now_ms();
                let report = self
                    .notes
                    .as_mut()
                    .map(|n| n.add_tags(&ids, std::slice::from_ref(&tag), now));
                self.after_library_batch(report, &format!("加标签「{tag}」"));
            }
            Some(note_library::LibraryAction::Remove(ids)) => {
                let report = self.notes.as_mut().map(|n| n.remove_many(&ids));
                self.after_library_batch(report, "从本地库移出");
            }
            Some(note_library::LibraryAction::Export(ids)) => {
                self.library_dialog = Some(LibraryDialog {
                    kind: LibraryDialogKind::Export(ids),
                    path: default_card_dir(),
                });
            }
            Some(note_library::LibraryAction::Import) => {
                self.library_dialog = Some(LibraryDialog {
                    kind: LibraryDialogKind::Import,
                    path: default_card_dir(),
                });
            }
            // §7.1 不设「刷新」按钮：这一条只从「更多」菜单与 §7.7 错误条的「重试」进来。
            Some(note_library::LibraryAction::Reload) => {
                let n = self.notes.as_mut().map(|s| s.rebuild()).unwrap_or(0);
                self.refresh_library();
                self.set_toast(format!("已重读索引：{n} 张卡片"), false);
            }
            None => {}
        }
    }

    /// 归档 / 取消归档：走 `set_archived`，它**不动 `updated_at`**——
    /// 把卡片收起来不算「编辑过」，否则「最近更新」排序会被归档动作打乱。
    fn library_batch(&mut self, ids: Vec<String>, archived: bool, verb: &'static str) {
        let report = self.notes.as_mut().map(|n| n.set_archived(&ids, archived));
        self.after_library_batch(report, verb);
    }

    /// 一批操作落地后的统一收尾（§7.11：一次批量 = 一次确认 + 一次结果提示，
    /// 且「不允许静默部分成功」——失败张数与第一条原因都要说出来）。
    fn after_library_batch(&mut self, report: Option<note_store::BatchReport>, verb: &str) {
        let Some(r) = report else {
            self.set_toast("本地卡片库不可用", true);
            return;
        };
        self.refresh_library();
        let (msg, err) = if r.is_empty() {
            (format!("没有卡片可{verb}"), true)
        } else {
            let mut msg = format!("{verb} {} 张", r.changed_count());
            if r.failed_count() > 0 {
                msg.push_str(&format!("，{} 张失败", r.failed_count()));
                if let Some(e) = r.first_error() {
                    msg.push_str(&format!("（{e}）"));
                }
            }
            if r.index_lagged {
                msg.push_str("；索引没写盘，下次启动自愈");
            }
            (msg, r.failed_count() > 0)
        };
        self.set_toast(msg, err);
    }

    pub(crate) fn library_dialog_ui(&mut self, ctx: &egui::Context) {
        let ch = self.chrome;
        let Some(dialog) = self.library_dialog.as_mut() else {
            return;
        };
        let (title, desc, button) = match &dialog.kind {
            LibraryDialogKind::Export(ids) => (
                "导出卡片到 Markdown 文件夹",
                format!(
                    "将 {} 张卡片写成 <id>.md（front matter + 正文）。",
                    ids.len()
                ),
                "导出",
            ),
            LibraryDialogKind::Import => (
                "从 Markdown 文件夹导入卡片",
                "读取文件夹里的 .md；同 id 卡片会覆盖本地库里的版本。".to_string(),
                "导入",
            ),
        };
        let ready = !dialog.path.trim().is_empty();
        let actions = [
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
            dialog::DialogAction::new(button, ui_kit::ButtonTone::Primary).enabled(ready),
        ];
        let spec = dialog::DialogSpec::new(title)
            .width(460.0)
            .default_action(1);
        let out = dialog::show(ctx, ch, "library-dialog", spec, &actions, |ui| {
            ui.label(egui::RichText::new(desc).size(Type::BODY).color(ch.dim));
            ui.add(
                egui::TextEdit::singleline(&mut dialog.path)
                    .font(FontId::proportional(Type::LABEL))
                    .hint_text("文件夹路径")
                    .desired_width(f32::INFINITY),
            );
        });
        if out.action == Some(1) {
            if let Some(dialog) = self.library_dialog.take() {
                self.run_library_dialog(dialog);
            }
        } else if out.closed {
            self.library_dialog = None;
        }
    }

    pub(crate) fn run_library_dialog(&mut self, dialog: LibraryDialog) {
        let path = dialog.path.trim().to_string();
        if path.is_empty() {
            self.set_toast("路径为空", true);
            return;
        }
        match dialog.kind {
            LibraryDialogKind::Export(ids) => {
                let cards: Vec<mw_core::NoteCard> = self
                    .notes
                    .as_ref()
                    .map(|n| ids.iter().filter_map(|id| n.get(id)).collect())
                    .unwrap_or_default();
                if cards.is_empty() {
                    self.set_toast("没有可导出的卡片", true);
                    return;
                }
                match mw_format::export_cards_dir(&cards, std::path::Path::new(&path)) {
                    Ok(n) => self.set_toast(format!("已导出 {n} 张卡片到 {path}"), false),
                    Err(e) => self.set_toast(format!("导出失败：{e}"), true),
                }
            }
            LibraryDialogKind::Import => {
                match mw_format::import_cards_dir(std::path::Path::new(&path)) {
                    Ok(out) => {
                        let mut written = 0;
                        if let Some(notes) = self.notes.as_mut() {
                            for card in &out.cards {
                                if notes.upsert(card).is_ok() {
                                    written += 1;
                                }
                            }
                        }
                        self.refresh_library();
                        let skipped = out.skipped.len();
                        self.set_toast(
                            if skipped == 0 {
                                format!("已导入 {written} 张卡片")
                            } else {
                                format!("已导入 {written} 张，跳过 {skipped} 个坏文件")
                            },
                            skipped > 0,
                        );
                    }
                    Err(e) => self.set_toast(format!("导入失败：{e}"), true),
                }
            }
        }
    }

    pub(crate) fn action_open(&mut self) {
        match files::open_zmind(self.hwnd) {
            Ok(Some(path)) => self.request(PendingAction::Open(path)),
            Ok(None) => {}
            Err(e) => self.set_toast(e, true),
        }
    }

    pub(crate) fn action_import(&mut self) {
        match files::open_import(self.hwnd) {
            Ok(Some(path)) => self.import_path(&path),
            Ok(None) => {}
            Err(e) => self.set_toast(e, true),
        }
    }

    pub(crate) fn action_new(&mut self) {
        self.request(PendingAction::New);
    }

    /// 拖进来的文件：`.zmind` 是打开，图片挂到主选节点，其它认识的格式是导入。
    pub(crate) fn handle_dropped(&mut self, ctx: &egui::Context) {
        let first = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .find(|p| !p.as_os_str().is_empty())
        });
        let Some(path) = first else {
            return;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "zmind" {
            self.request(PendingAction::Open(path));
        } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp") {
            self.attach_image_file(&path);
        } else if Session::accepts(&path) {
            self.import_path(&path);
        } else {
            self.set_toast(format!("不认识的格式：{ext}"), true);
        }
    }

    /// 选图：文件对话框 → 字节 → 内容资源 → `SetImage`（一条撤销）。
    pub(crate) fn pick_image_for_primary(&mut self) {
        let Some(id) = self
            .canvas
            .as_ref()
            .and_then(|c| c.selected_ids().last().cloned())
        else {
            self.set_toast("先选中一个主题", true);
            return;
        };
        match files::open_image(self.hwnd) {
            Ok(Some(path)) => self.attach_image_to(&id, &path),
            Ok(None) => {}
            Err(e) => self.set_toast(e, true),
        }
    }

    pub(crate) fn attach_image_file(&mut self, path: &std::path::Path) {
        let Some(id) = self
            .canvas
            .as_ref()
            .and_then(|c| c.selected_ids().last().cloned())
        else {
            self.set_toast("先选中一个主题，再拖入图片", true);
            return;
        };
        self.attach_image_to(&id, path);
    }

    pub(crate) fn attach_image_to(&mut self, id: &str, path: &std::path::Path) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.set_toast(format!("读不了图片：{e}"), true);
                return;
            }
        };
        self.attach_image_bytes_to(id, bytes);
    }

    pub(crate) fn attach_image_bytes(&mut self, bytes: Vec<u8>) {
        let Some(id) = self
            .canvas
            .as_ref()
            .and_then(|c| c.selected_ids().last().cloned())
        else {
            self.set_toast("先选中一个主题，再粘贴图片", true);
            return;
        };
        self.attach_image_bytes_to(&id, bytes);
    }

    pub(crate) fn attach_image_bytes_to(&mut self, id: &str, bytes: Vec<u8>) {
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        let key = canvas.add_asset(bytes);
        if canvas.apply_command(mw_core::Command::SetImage {
            id: id.to_string(),
            image: Some(format!("asset:{key}")),
        }) {
            self.set_toast("图片已挂到主题", false);
        } else {
            self.set_toast("图片挂载失败", true);
        }
    }

    /// 自动保存：**每个标签**脏、且修订号变过，才在间隔到了之后各写一份恢复点。
    ///
    /// **必须自己催帧**：egui 空闲时根本不跑 `ui()`，光靠"每帧看一眼时钟"的定时器
    /// 永远不会到期——第一版就是这样，编辑完等 4 秒，磁盘上一个字节都没多。
    ///
    /// 快照在 UI 线程做（克隆节点表），序列化/压缩/写盘在工作线程；队列按槽位去重，
    /// 同一标签只留最新一版，不同标签互不挤占。
    /// 自动保存：**每个标签**脏、且修订号变过，才在间隔到了之后各写一份恢复点。
    ///
    /// **必须自己催帧**：egui 空闲时根本不跑 `ui()`，光靠"每帧看一眼时钟"的定时器
    /// 永远不会到期——第一版就是这样，编辑完等 4 秒，磁盘上一个字节都没多。
    ///
    /// 快照在 UI 线程做（克隆节点表），序列化/压缩/写盘在工作线程；队列按槽位去重，
    /// 同一标签只留最新一版，不同标签互不挤占。
    pub(crate) fn autosave_tick(&mut self, ctx: &egui::Context) {
        // 后台完成随时可能到达：一次可能收回多个槽位的结果。
        let mut any_failed = false;
        let mut fail_toast: Option<String> = None;
        for outcome in self.autosave.poll() {
            match outcome {
                autosave::Outcome::Wrote { slot, revision } => {
                    // **确认**写进磁盘了才记账（问题 7）：提交时只记在途表。
                    self.autosave_submitted.remove(&slot);
                    self.autosave_stalls.remove(&slot);
                    let entry = self.last_autosave_rev.entry(slot).or_insert(revision);
                    *entry = (*entry).max(revision);
                }
                autosave::Outcome::Failed {
                    slot,
                    revision,
                    reason,
                } => {
                    // 失败不再静默（问题 6）：写不进去就等于这一刻没有崩溃保护，
                    // 用户有权知道；但同一槽位只在**第一次**失败时提示，别每 3 秒刷一遍屏。
                    self.autosave_submitted.remove(&slot);
                    let first = !self.autosave_stalls.contains_key(&slot);
                    let attempts = self
                        .autosave_stalls
                        .get(&slot)
                        .map_or(0, |s: &AutosaveStall| s.attempts)
                        + 1;
                    self.autosave_stalls.insert(
                        slot.clone(),
                        AutosaveStall {
                            attempts,
                            rev: revision,
                            retry_at: std::time::Instant::now() + autosave_backoff(attempts),
                        },
                    );
                    crate::trace::event(&format!(
                        "autosave_fail name={} attempts={attempts} reason={reason}",
                        file_label(&slot)
                    ));
                    if first {
                        fail_toast = Some(format!(
                            "自动保存失败：{reason}。恢复点暂时写不进磁盘，请手动 Ctrl+S 保存"
                        ));
                    }
                    any_failed = true;
                }
            }
        }
        if let Some(text) = fail_toast {
            self.set_toast(text, true);
        }
        if any_failed {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(autosave::INTERVAL));
        }
        // 在途写要收尾：400ms 一帧够及时，也不至于把空闲功耗顶满。
        if self.autosave.is_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(400));
        }
        let now = ctx.input(|i| i.time);
        if now < self.next_autosave {
            return;
        }
        self.next_autosave = now + self.settings.autosave_interval();

        // 提交记账要对齐现实：被 `invalidate`（正本已保存）或槽位作废吃掉的提交，
        // 结果永远不会回来。不把它们从账上抹掉，那个修订号就再也不会重投（问题 7）。
        let orphan: Vec<std::path::PathBuf> = self
            .autosave_submitted
            .keys()
            .filter(|slot| !self.autosave.is_pending(slot))
            .cloned()
            .collect();
        for slot in orphan {
            self.autosave_submitted.remove(&slot);
        }

        // 活动标签以画布里的会话为准（`tabs[active]` 只在切页时才同步）；
        // 其余标签直接读自己的 `Tab`。当前无路径的标签用 doc id 区分槽位。
        let mut submitted = false;
        let active = self.active;
        let mut candidates: Vec<(usize, &Session)> = Vec::new();
        if let Some(canvas) = self.canvas.as_ref() {
            candidates.push((active, canvas.session()));
        }
        for (i, tab) in self.tabs.iter().enumerate() {
            if i != active {
                candidates.push((i, &tab.session));
            }
        }
        let wall = std::time::Instant::now();
        for (_, s) in candidates {
            let Some(slot) = autosave::slot_for(s) else {
                continue;
            };
            let rev = s.doc.revision;
            // 三问：确认过没有、在途没有（问题 7）、退避到点没有（问题 6）。
            if !autosave_wants(
                s.dirty(),
                rev,
                self.last_autosave_rev.get(&slot).copied(),
                self.autosave_submitted.get(&slot).copied(),
            ) {
                continue;
            }
            if !autosave_may_retry(self.autosave_stalls.get(&slot), rev, wall) {
                continue;
            }
            if self.autosave.submit(s) {
                // 只记「已提交」；「已确认」要等 `Outcome::Wrote`（问题 7）。
                self.autosave_submitted.insert(slot, rev);
                submitted = true;
            }
        }
        // 脏文档要持续催帧让定时器走：既够 3 秒间隔，又不把空闲功耗顶满。
        if submitted {
            ctx.request_repaint_after(std::time::Duration::from_millis(400));
        }
    }

    /// 关窗前拦一道：**任一**标签有未保存改动就问（SR-02 问题 2）。
    ///
    /// 以前只看活动标签：后台标签的改动连提示都没有，「保存并关闭」也只存一份。
    pub(crate) fn guard_close(&mut self, ctx: &egui::Context) {
        if self.close_confirmed || !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        let names = dirty_tab_names(&self.tab_dirty_snapshot());
        if !names.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
            crate::trace::event(&format!("close_blocked dirty_tabs={}", names.len()));
        }
    }

    /// 全部标签的 `(名字, 脏?)` 快照。活动标签以画布里的会话为准（它是权威的那份）。
    pub(crate) fn tab_dirty_snapshot(&self) -> Vec<(String, bool)> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let dirty = if i == self.active {
                    self.canvas.as_ref().is_some_and(|c| c.session().dirty())
                } else {
                    t.session.dirty()
                };
                (self.tab_label(i).0, dirty)
            })
            .collect()
    }

    pub(crate) fn close_prompt(&mut self, ctx: &egui::Context) {
        if !self.confirm_close {
            return;
        }
        let actions = [
            dialog::DialogAction::new("保存并关闭", ui_kit::ButtonTone::Primary),
            dialog::DialogAction::new("不保存", ui_kit::ButtonTone::Danger),
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
        ];
        let spec = dialog::DialogSpec::new("还有未保存的改动")
            .width(440.0)
            .default_action(2);
        let names = dirty_tab_names(&self.tab_dirty_snapshot());
        let out = dialog::show(ctx, self.chrome, "close-confirm", spec, &actions, |ui| {
            if names.len() > 1 {
                ui.label(format!("这几个标签页还没保存：{}", names.join("、")));
            } else {
                ui.label("关闭前要保存吗？");
            }
        });
        match out.action {
            Some(0) => {
                // 后台标签先各自写回自己的路径（问题 2）：以前只存活动那一份，
                // 其余标签的改动随窗口一起消失。
                let bg = save_background_dirty_tabs(&mut self.tabs, self.active);
                self.action_save();
                // 关闭流程等不到下一帧：这里同步把 Saving 落成磁盘结果。
                self.pump_save();
                let active_clean = self.canvas.as_ref().is_some_and(|c| !c.session().dirty());
                if !bg.failed.is_empty() || !active_clean {
                    let mut names = bg.failed.clone();
                    if !active_clean {
                        names.push("当前文档".to_string());
                    }
                    // 有东西存不下来就不关窗：悄悄关掉是最坏的结局。
                    self.set_toast(format!("还有内容没能保存：{}", names.join("、")), true);
                    return;
                }
                // 未命名标签没法非交互落盘：给它们各写一份恢复点，下次启动会问要不要恢复。
                if !bg.untitled.is_empty() {
                    self.flush_recovery_points_on_exit();
                    self.set_toast(
                        format!("{} 个未命名标签的改动已写入恢复点", bg.untitled.len()),
                        false,
                    );
                }
                self.confirm_close = false;
                self.close_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(1) => {
                self.confirm_close = false;
                self.close_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {
                if out.closed {
                    self.confirm_close = false;
                }
            }
        }
    }

    /// 退出前给每个脏标签补一份恢复点（SR-02 问题 3）。
    ///
    /// 以前 `on_exit` 只收尾导出/导入线程，一个字节都不写：改完直接关窗，
    /// 那点改动就只剩"没有"。这里按 [`needs_recovery_submit`] 的判据补提交，再等它写完。
    pub(crate) fn flush_recovery_points_on_exit(&mut self) {
        let active = self.active;
        let mut queued: Vec<(std::path::PathBuf, u64)> = Vec::new();
        {
            let mut candidates: Vec<&Session> = Vec::new();
            if let Some(canvas) = self.canvas.as_ref() {
                candidates.push(canvas.session());
            }
            for (i, tab) in self.tabs.iter().enumerate() {
                if i != active {
                    candidates.push(&tab.session);
                }
            }
            for s in candidates {
                let Some(slot) = autosave::slot_for(s) else {
                    continue;
                };
                let rev = s.doc.revision;
                if !needs_recovery_submit(
                    s.dirty(),
                    rev,
                    self.last_autosave_rev.get(&slot).copied(),
                ) {
                    continue;
                }
                if self.autosave.submit(s) {
                    self.autosave_submitted.insert(slot.clone(), rev);
                    queued.push((slot, rev));
                }
            }
        }
        for (slot, rev) in queued {
            crate::trace::event(&format!(
                "exit_recovery name={} rev={rev}",
                file_label(&slot)
            ));
        }
    }

    /// 退出前把卡片页去抖（800ms）里还压着的草稿落一次（SR-02 问题 3）。
    ///
    /// 判脏不复制视图层逻辑：`CardReader::is_dirty` 是私有的，这里拿公开的 `draft_*`
    /// 与「库里 / 当前文档里那张卡」的已存字段比（[`draft_differs`]），
    /// 所以不用改 `note_view.rs` 也不用给它加接口。
    pub(crate) fn flush_card_draft_on_exit(&mut self) {
        let Some((id, title, body, tags, read_only)) = self.note_reader.as_ref().map(|r| {
            (
                r.card_id.clone(),
                r.draft_title.clone(),
                r.draft_body.clone(),
                r.draft_tags.clone(),
                r.is_read_only(),
            )
        }) else {
            return;
        };
        let baseline = self
            .notes
            .as_ref()
            .and_then(|n| n.get(&id))
            .map(|c| (c.title.clone(), c.body.clone(), c.tags.clone()))
            .or_else(|| {
                self.canvas.as_ref().and_then(|c| {
                    c.session()
                        .doc
                        .cards()
                        .iter()
                        .find(|k| k.id == id)
                        .map(|k| (k.title.clone(), k.body.clone(), k.tags.clone()))
                })
            });
        if !draft_differs(baseline.as_ref(), &title, &body, &tags) {
            return;
        }
        if read_only {
            // 只读不越权写：卡片页已经说过「这张卡片只读」，退出也不该偷偷落盘。
            crate::trace::event("exit_card_skip read_only=1");
            return;
        }
        let ok = self.save_card_from_reader(&id, &title, &body, &tags);
        crate::trace::event(&format!("exit_card_flush ok={}", u8::from(ok)));
    }

    /// 「当前文档有未保存改动」——换文档之前问一句。
    pub(crate) fn discard_prompt(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending.as_ref() else {
            return;
        };
        let what = match action {
            PendingAction::Open(_) => "打开另一份文档",
            PendingAction::Reload(_) => "重新加载磁盘上的版本",
            PendingAction::New => "新建文档",
        };
        let actions = [
            dialog::DialogAction::new("先保存", ui_kit::ButtonTone::Primary),
            dialog::DialogAction::new("直接继续", ui_kit::ButtonTone::Danger),
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
        ];
        let spec = dialog::DialogSpec::new("当前文档有未保存的改动")
            .width(460.0)
            .default_action(2);
        let out = dialog::show(ctx, self.chrome, "discard-prompt", spec, &actions, |ui| {
            ui.label(format!("{what}会丢掉这些改动。"));
        });
        let decision = match out.action {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        };
        if decision.is_none() {
            if out.closed {
                self.pending = None;
            }
            return;
        }
        if let Some(save) = decision {
            if save {
                self.action_save();
                self.pump_save();
            }
            // 只有"存成功了"或"本来就要丢弃"才继续；存盘失败就停在原地。
            let clean = self.canvas.as_ref().is_some_and(|c| !c.session().dirty());
            if !save || clean {
                if let Some(action) = self.pending.take() {
                    self.run(action);
                }
            }
        }
    }

    /// 关闭有未保存改动的标签页时问一句。
    pub(crate) fn tab_close_prompt(&mut self, ctx: &egui::Context) {
        let Some(idx) = self.confirm_close_tab else {
            return;
        };
        let (name, _) = self.tab_label(idx);
        let actions = [
            dialog::DialogAction::new("保存并关闭", ui_kit::ButtonTone::Primary),
            dialog::DialogAction::new("不保存关闭", ui_kit::ButtonTone::Danger),
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
        ];
        let spec = dialog::DialogSpec::new("标签页有未保存的改动")
            .width(440.0)
            .default_action(2);
        let out = dialog::show(ctx, self.chrome, "tab-close-prompt", spec, &actions, |ui| {
            ui.label(format!("「{name}」还没保存。"));
        });
        let decision = match out.action {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        };
        if decision.is_none() {
            if out.closed {
                self.confirm_close_tab = None;
            }
            return;
        }
        if let Some(save) = decision {
            // 存盘得在该标签页自己的会话上做：先切过去让画布与活动槽对齐，
            // 无路径时 `action_save` 会转成另存为对话框。
            self.switch_tab(idx);
            let target = self.active;
            if save {
                self.action_save();
                self.pump_save();
                // 写失败、只读、冲突或用户在另存为里点了取消：标签仍有改动就停在弹窗原位，
                // 让用户改选「不保存关闭」或先处理失败，绝不悄悄关掉。
                if self.canvas.as_ref().is_some_and(|c| c.session().dirty()) {
                    return;
                }
            }
            self.confirm_close_tab = None;
            self.close_tab(target);
        }
    }

    /// 「发现恢复点」——自动保存写的，比磁盘上的正本还新。
    pub(crate) fn recovery_prompt(&mut self, ctx: &egui::Context) {
        let Some((slot, doc)) = self.recovery.clone() else {
            return;
        };
        let actions = [
            dialog::DialogAction::new("恢复", ui_kit::ButtonTone::Primary),
            dialog::DialogAction::new("忽略并删除", ui_kit::ButtonTone::Danger),
            dialog::DialogAction::new("稍后决定", ui_kit::ButtonTone::Secondary),
        ];
        let spec = dialog::DialogSpec::new("发现未保存的恢复点")
            .width(460.0)
            .default_action(2);
        let out = dialog::show(ctx, self.chrome, "recovery-point", spec, &actions, |ui| {
            ui.label("上次退出前有一份比磁盘文件更新的内容。");
        });
        // 「稍后决定」：恢复点留在磁盘上，下次启动还会问；Esc 等同稍后决定。
        let recover = match out.action {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        };
        if recover.is_none() {
            if out.closed {
                self.recovery = None;
            }
            return;
        }
        self.recovery = None;
        if recover == Some(true) {
            match Session::open_recovered(&slot, &doc) {
                Ok(session) => {
                    self.search.close();
                    if let Some(canvas) = self.canvas.as_mut() {
                        canvas.replace_session(session);
                    }
                    // 开始页启动时恢复的文档必须推到台前，否则用户以为恢复没生效。
                    self.home = false;
                    self.home_return = false;
                    self.note_session_rev(0);
                    self.set_toast("已恢复上次内容，Ctrl+S 写回原文件", false);
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&slot);
                    self.set_toast(e, true);
                }
            }
        } else {
            let _ = std::fs::remove_file(&slot);
        }
    }

    /// 底部状态条：两秒后自己消失。
    pub(crate) fn toast_bar(&mut self, ctx: &egui::Context) {
        let Some(t) = self.toast.as_mut() else {
            return;
        };
        let until = *t.until.get_or_insert_with(|| {
            std::time::Instant::now() + std::time::Duration::from_millis(2400)
        });
        // 金样里不按墙钟过期：2400ms 是真实时钟，截图跟它赛跑会拍出「有时在、有时已消失」两张图
        // （`cm-arrange-undo` 就踩过一次——整理后的提示条到底进没进基线，取决于这台机器多快）。
        if self.harness.is_none() && std::time::Instant::now() >= until {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(120));
        let screen = ctx.input(|i| i.content_rect());
        let area = egui::Rect::from_min_size(
            egui::Pos2::new(screen.min.x + 16.0, screen.max.y - 52.0),
            egui::Vec2::new(360.0, 30.0),
        );
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::unique("mw-toast"),
        ));
        let bg = if t.bad {
            Color32::from_rgb(0xC4, 0x2B, 0x1C)
        } else {
            Color32::from_rgb(0x2B, 0x33, 0x40)
        };
        painter.rect_filled(area, 8.0, bg);
        painter.text(
            egui::Pos2::new(area.min.x + 12.0, area.center().y),
            Align2::LEFT_CENTER,
            &t.text,
            FontId::proportional(Type::LABEL),
            Color32::WHITE,
        );
    }

    /// 标题栏拖拽/双击热区。同一块区域可能叠两个（整条 + 标题文字），`id` 必须不同。
    pub(crate) fn titlebar_drag(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        rect: Rect,
        id: egui::Id,
        maximized: bool,
    ) {
        let drag = ui.interact(rect, id, egui::Sense::CLICK | egui::Sense::DRAG);
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            if self.focus_mode.is_some() {
                self.toggle_focus_mode(ctx);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
        }
    }

    /// 顶栏：品牌 + 真菜单栏 + 工具簇 + 右侧文档标题与窗口按钮。
    ///
    /// 拖拽区仍是整条顶栏（右侧按钮条除外）：后注册的按钮在 egui 命中测试里
    /// 压在上面，所以按按钮不会顺带拖动窗口。工具簇直接排在菜单右侧，
    /// 画布上方不再有悬浮胶囊。
    pub(crate) fn titlebar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        maximized: bool,
        focused: bool,
    ) {
        let bar_all = ui.max_rect();
        let controls_w = mw_core::design::component::WINDOW_CONTROL_W * 3.0;
        // 拖拽区不含右侧按钮条，否则双击按钮会连带触发「双击最大化」。
        let bar = Rect::from_min_max(
            bar_all.min,
            Pos2::new(bar_all.max.x - controls_w, bar_all.max.y),
        );
        self.titlebar_drag(ui, ctx, bar, ui.scope_id().with("tb-drag"), maximized);

        let (doc_name, _) = self.doc_state();
        let save = self.save_state();
        let dim = if focused { 1.0 } else { 0.55 };
        let ch = self.chrome;
        // 窄窗分级：五个菜单先折成「更多」，再隐品牌文字；文档标题与保存芯片
        // 由工具簇之后的实际余量决定，避免窗口一跨过阈值就整组消失。
        let compact_menus = bar_all.width() < 900.0;
        let brand_text = bar_all.width() >= 720.0;
        let items = self.tool_items();
        let theme_id = self.canvas.as_ref().map(|c| c.session().theme.id);
        let recent: Vec<(String, std::path::PathBuf)> = self
            .recent
            .entries()
            .iter()
            .take(5)
            .map(|e| (e.title.clone(), e.path.clone()))
            .collect();
        let editing = self.canvas.as_ref().is_some_and(Canvas::is_editing);
        let has_canvas = self.canvas.is_some() && !self.home;
        let can_undo = has_canvas && self.canvas.as_ref().is_some_and(|c| c.can_undo()) && !editing;
        let can_redo = has_canvas && self.canvas.as_ref().is_some_and(|c| c.can_redo()) && !editing;
        let can_delete =
            has_canvas && self.canvas.as_ref().is_some_and(|c| c.can_delete()) && !editing;
        let can_add = has_canvas && !editing;
        use shortcuts::Action as SA;
        let hint = |a: SA| self.shortcuts.hint(a);
        let k_redo = {
            let h = hint(SA::Redo);
            if h.is_empty() {
                hint(SA::RedoAlt)
            } else {
                h
            }
        };
        let menu = MenuCtx {
            k_new: hint(SA::New),
            k_new_concept: hint(SA::NewConcept),
            k_new_tab: hint(SA::NewTab),
            k_open: hint(SA::Open),
            k_save: hint(SA::Save),
            k_save_as: hint(SA::SaveAs),
            k_export: hint(SA::Export),
            k_undo: hint(SA::Undo),
            k_redo,
            k_find: hint(SA::Find),
            k_palette: hint(SA::Palette),
            k_prefs: hint(SA::Preferences),
            k_outline: hint(SA::Outline),
            k_inspector: hint(SA::Inspector),
            k_focus: hint(SA::FocusMode),
            k_reading: hint(SA::Reading),
            k_help: hint(SA::Help),
            recent,
            theme: theme_id,
            concept_map: self
                .canvas
                .as_ref()
                .is_some_and(crate::canvas::Canvas::is_concept_map),
            can_undo,
            can_redo,
            can_add,
            can_delete,
            has_canvas,
            outline: self.outline_view,
            inspector: self.show_inspector,
            minimap: self.show_minimap,
            focus: self.focus_mode.is_some(),
            reading: self.reading.is_some(),
            notes: self.notes.is_some(),
        };

        let mut action = None;
        let mut tool_clicked = None;
        let mut left_end = bar.left();
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bar)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_clip_rect(bar);
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(10.0);
                paint_brand(ui, ch, dim, brand_text);
                ui.add_space(if brand_text { 14.0 } else { 12.0 });

                if compact_menus {
                    title_menu_button(
                        ui,
                        ch,
                        dim,
                        "更多",
                        "more",
                        330.0,
                        &mut action,
                        |ui, a| more_menu(ch, ui, &menu, a),
                    );
                } else {
                    title_menu_button(
                        ui,
                        ch,
                        dim,
                        "文件",
                        "file",
                        318.0,
                        &mut action,
                        |ui, a| file_menu(ch, ui, &menu, a),
                    );
                    title_menu_button(
                        ui,
                        ch,
                        dim,
                        "编辑",
                        "edit",
                        258.0,
                        &mut action,
                        |ui, a| edit_menu(ch, ui, &menu, a),
                    );
                    title_menu_button(
                        ui,
                        ch,
                        dim,
                        "视图",
                        "view",
                        268.0,
                        &mut action,
                        |ui, a| view_menu(ch, ui, &menu, a),
                    );
                    title_menu_button(
                        ui,
                        ch,
                        dim,
                        "插入",
                        "insert",
                        268.0,
                        &mut action,
                        |ui, a| insert_menu(ch, ui, &menu, a),
                    );
                    title_menu_button(
                        ui,
                        ch,
                        dim,
                        "帮助",
                        "help",
                        248.0,
                        &mut action,
                        |ui, a| help_menu(ch, ui, &menu, a),
                    );
                }

                // 菜单与工具之间的竖分隔：两组语义不同，靠一条细线断开。
                ui.add_space(8.0);
                let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
                ui.painter().vline(
                    sep_rect.center().x,
                    sep_rect.y_range(),
                    Stroke::new(mw_core::design::stroke::HAIRLINE, ch.border),
                );
                ui.add_space(8.0);

                let mut prev_btn = false;
                for item in items.iter() {
                    match item {
                        ToolItem::Sep => {
                            let (r, _) = ui.allocate_exact_size(
                                Vec2::new(
                                    mw_core::design::component::TOOL_GROUP_SEP,
                                    mw_core::design::component::TOOL,
                                ),
                                Sense::hover(),
                            );
                            ui.painter().line_segment(
                                [
                                    Pos2::new(r.center().x, r.center().y - 8.0),
                                    Pos2::new(r.center().x, r.center().y + 8.0),
                                ],
                                Stroke::new(mw_core::design::stroke::HAIRLINE, ch.border),
                            );
                            prev_btn = false;
                        }
                        ToolItem::Button { icon, enabled, tip } => {
                            if prev_btn {
                                ui.add_space(mw_core::design::component::TOOL_GAP);
                            }
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::splat(mw_core::design::component::TOOL),
                                Sense::hover(),
                            );
                            let id = ui.scope_id().with(("title-tool", icon.trace_name()));
                            let resp = crate::ui_kit::tool_button(
                                ui,
                                ch,
                                rect,
                                id,
                                *icon,
                                crate::ui_kit::ToolButtonState {
                                    active: false,
                                    enabled: *enabled,
                                    tip,
                                },
                            );
                            crate::trace::name(ui.ctx(), resp.id, icon.trace_name());
                            if *enabled && resp.clicked() {
                                tool_clicked = Some(*icon);
                            }
                            prev_btn = true;
                        }
                    }
                }
                left_end = ui.cursor().left();
            },
        );

        // 文档标题 + 保存状态靠右，按工具簇之后的真实余量取舍：状态芯片优先，
        // 剩下的宽度够 80px 才画标题；两者都放不下就整组留白，不画省略号。
        {
            let right = bar.right() - 12.0;
            let left = left_end + 24.0;
            let avail = right - left;
            let status_font = FontId::proportional(Type::CAPTION);
            let status_galley =
                ui.painter()
                    .layout_no_wrap(save.label().to_owned(), status_font.clone(), ch.dim);
            let chip_w = status_galley.size().x + 26.0;
            let cluster = crate::app_helpers::title_cluster(avail, chip_w);
            let chip_rect = Rect::from_min_size(
                Pos2::new(
                    right - chip_w,
                    bar.center().y - mw_core::design::component::SAVE_CHIP_H * 0.5,
                ),
                Vec2::new(chip_w, mw_core::design::component::SAVE_CHIP_H),
            );
            if cluster.chip {
                let title_right = chip_rect.left() - 12.0;
                if cluster.title {
                    let title_avail = title_right - left;
                    let max_chars = ((title_avail.min(280.0) / 13.0) as usize).clamp(6, 64);
                    let name = if self.home {
                        "开始页".to_string()
                    } else {
                        shorten(&doc_name, max_chars)
                    };
                    ui.painter().text(
                        Pos2::new(title_right, bar.center().y),
                        Align2::RIGHT_CENTER,
                        name,
                        FontId::proportional(Type::BODY),
                        ch.faded(ch.text, dim),
                    );
                }
                let sense = if save.can_save() {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let resp = ui.interact(chip_rect, ui.scope_id().with("save-chip"), sense);
                if save.can_save() && resp.hovered() {
                    ui.painter().rect_filled(
                        chip_rect,
                        egui::CornerRadius::same(mw_core::design::radius::SM as u8),
                        ch.hover,
                    );
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if save.can_save() {
                    crate::ui_kit::paint_focus_ring(
                        ui.painter(),
                        ch,
                        chip_rect,
                        mw_core::design::radius::SM,
                        &resp,
                    );
                }
                let (dot, ink) = match save {
                    SaveState::Dirty | SaveState::Failed => (ch.warn, ch.warn),
                    SaveState::Untitled => (ch.dim, ch.dim),
                    SaveState::Saving => (ch.accent, ch.dim),
                    SaveState::Saved => (ch.accent, ch.dim),
                    SaveState::Home => (ch.dim, ch.dim),
                };
                ui.painter().circle_filled(
                    Pos2::new(chip_rect.left() + 10.5, chip_rect.center().y),
                    2.5,
                    ch.faded(dot, dim),
                );
                ui.painter().text(
                    Pos2::new(chip_rect.left() + 18.0, chip_rect.center().y),
                    Align2::LEFT_CENTER,
                    save.label(),
                    status_font,
                    ch.faded(ink, dim),
                );
                let resp = resp.on_hover_text(save.tip_with_phase(&self.save_phase));
                if save.can_save() && resp.clicked() {
                    action = Some(TitleMenuAction::Save);
                }
            }
        }

        // 右侧窗口按钮：贴着顶栏右缘，高度吃满，与 Win11 原生行为一致。
        let controls = Rect::from_min_max(
            Pos2::new(bar_all.max.x - controls_w, bar_all.min.y),
            bar_all.max,
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(controls)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if self.win_btn(ui, WinGlyph::Close, "关闭", CLOSE_HOT) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                let glyph = if maximized {
                    WinGlyph::Restore
                } else {
                    WinGlyph::Maximize
                };
                if self.win_btn(
                    ui,
                    glyph,
                    if maximized { "还原" } else { "最大化" },
                    self.chrome.hover,
                ) {
                    if self.focus_mode.is_some() {
                        self.toggle_focus_mode(ctx);
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                }
                if self.win_btn(ui, WinGlyph::Minimize, "最小化", self.chrome.hover) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            },
        );

        ui.painter().hline(
            bar_all.x_range(),
            bar_all.max.y - 0.5,
            Stroke::new(1.0, ch.sep),
        );

        if let Some(icon) = tool_clicked {
            // 「聚焦问题」碰的是文档属性，写入口在检查器（`docs/33` §5/§6）——
            // 这里把检查器打开、把焦点送进那一格，剩下的交给那个输入框。
            if icon == ToolIcon::FocusQ {
                self.show_inspector = true;
                ctx.memory_mut(|m| {
                    m.request_focus(egui::Id::unique("inspector/focus-question/field"))
                });
            }
            if let Some(canvas) = self.canvas.as_mut() {
                match icon {
                    ToolIcon::Undo => {
                        canvas.undo();
                    }
                    ToolIcon::Redo => {
                        canvas.redo();
                    }
                    ToolIcon::AddChild => {
                        canvas.add_child();
                    }
                    ToolIcon::AddSibling => {
                        canvas.add_sibling();
                    }
                    ToolIcon::Delete => {
                        canvas.delete_selected();
                    }
                    ToolIcon::Fit => {
                        canvas.fit_view();
                    }
                    ToolIcon::Concept => {
                        if let Some(id) = canvas.add_free_concept() {
                            canvas.select_node(&id);
                        }
                    }
                    ToolIcon::Link => {
                        canvas.begin_relationship_action();
                    }
                    ToolIcon::Phrase => {
                        // 没选中命题时这个方法**静默**返回 false——按钮灰着但能点，
                        // 点了不给话就等于坏了（`docs/33` §5）。
                        if !canvas.edit_relationship_label_action() {
                            canvas.hint("先选中一条命题，再改短语");
                        }
                    }
                    ToolIcon::Arrange => {
                        if canvas.arrange_concepts().is_none() {
                            canvas.hint("没有可整理的概念");
                        }
                    }
                    ToolIcon::Pin if canvas.toggle_concept_pinned_selected().is_none() => {
                        canvas.hint("先选中一个概念，再钉住位置");
                    }
                    _ => {}
                }
            }
            ctx.request_repaint();
        }
        if let Some(action) = action {
            self.apply_title_action(ctx, action);
        }
    }

    pub(crate) fn apply_title_action(&mut self, ctx: &egui::Context, action: TitleMenuAction) {
        match action {
            TitleMenuAction::New => self.action_new(),
            TitleMenuAction::NewConcept => self.new_concept_tab(),
            TitleMenuAction::NewTab => self.new_tab(),
            TitleMenuAction::Open => self.action_open(),
            TitleMenuAction::Import => self.action_import(),
            TitleMenuAction::Save => self.action_save(),
            TitleMenuAction::SaveAs => self.action_save_as(),
            TitleMenuAction::Export => self.open_export(),
            TitleMenuAction::Recent(path) => self.request(PendingAction::Open(path)),
            TitleMenuAction::Find => self.open_search(),
            TitleMenuAction::Palette => self.palette.open(),
            TitleMenuAction::Preferences => self.shortcuts_dialog.open(),
            TitleMenuAction::Help => self.help.toggle(),
            TitleMenuAction::Home => self.open_home(),
            TitleMenuAction::ToggleOutline => self.toggle_outline_view(),
            TitleMenuAction::ToggleInspector => self.toggle_inspector(),
            TitleMenuAction::ToggleMinimap => self.toggle_minimap(),
            TitleMenuAction::FocusMode => self.toggle_focus_mode(ctx),
            TitleMenuAction::Reading => self.toggle_reading(),
            TitleMenuAction::SetTheme(id) => self.apply_theme(id),
            TitleMenuAction::Undo => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.undo();
                }
            }
            TitleMenuAction::Redo => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.redo();
                }
            }
            TitleMenuAction::AddChild => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.add_child();
                }
            }
            TitleMenuAction::AddSibling => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.add_sibling();
                }
            }
            TitleMenuAction::DeleteNode => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.delete_selected();
                }
            }
            TitleMenuAction::Fit => {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.fit_view();
                }
            }
            TitleMenuAction::Library => self.open_library(),
            TitleMenuAction::DailyNote => self.open_daily_note(),
            TitleMenuAction::CaptureCard => self.capture_card_for_selected(),
            TitleMenuAction::TagManager => {
                self.close_light_overlays();
                self.tag_manager = Some(TagManager::default());
            }
            TitleMenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    /// 中央工具条的可用项。文档与选中状态每帧读一次，按钮本身不缓存状态。
    ///
    /// 概念图走**专属工具条**（`docs/33` §5）：与思维导图那条互斥——画布是概念图时
    /// 只显示这一条。`docs/30` §5.9 禁的是「同一张画布两条工具条」，不是禁
    /// 「概念图有自己的工具」。
    pub(crate) fn tool_items(&self) -> [ToolItem; 8] {
        if self.canvas.as_ref().is_some_and(Canvas::is_concept_map) {
            return self.concept_tool_items();
        }
        let editing = self.canvas.as_ref().is_some_and(Canvas::is_editing);
        let has_canvas = self.canvas.is_some() && !self.home;
        let can_undo = has_canvas && self.canvas.as_ref().is_some_and(|c| c.can_undo()) && !editing;
        let can_redo = has_canvas && self.canvas.as_ref().is_some_and(|c| c.can_redo()) && !editing;
        let can_delete =
            has_canvas && self.canvas.as_ref().is_some_and(|c| c.can_delete()) && !editing;
        let can_add = has_canvas && !editing;
        [
            ToolItem::Button {
                icon: ToolIcon::Undo,
                enabled: can_undo,
                tip: "撤销 (Ctrl+Z)",
            },
            ToolItem::Button {
                icon: ToolIcon::Redo,
                enabled: can_redo,
                tip: "重做 (Ctrl+Y)",
            },
            ToolItem::Sep,
            ToolItem::Button {
                icon: ToolIcon::AddChild,
                enabled: can_add,
                tip: "添加子主题 (Tab)",
            },
            ToolItem::Button {
                icon: ToolIcon::AddSibling,
                enabled: can_add,
                tip: "添加同级主题 (Enter)",
            },
            ToolItem::Button {
                icon: ToolIcon::Delete,
                enabled: can_delete,
                tip: "删除选中主题 (Delete)",
            },
            ToolItem::Sep,
            ToolItem::Button {
                icon: ToolIcon::Fit,
                enabled: has_canvas,
                tip: "适应画布 (Home)",
            },
        ]
    }

    /// 概念图工具条（`docs/33` §5）：六个动作 + 适应画布，与 CmapTools 的常用工具对齐。
    ///
    /// 禁用态（没选中概念时的「连接」「改短语」「钉住」）**仍可点**，点了给提示——
    /// 灰掉是为了说明「现在按不动」，不是为了把按钮藏起来。
    fn concept_tool_items(&self) -> [ToolItem; 8] {
        let editing = self.canvas.as_ref().is_some_and(Canvas::is_editing);
        let has_canvas = self.canvas.is_some() && !self.home;
        let can = has_canvas && !editing;
        let concepts = self
            .canvas
            .as_ref()
            .map(|c| c.selected_concept_ids().len())
            .unwrap_or(0);
        [
            ToolItem::Button {
                icon: ToolIcon::Concept,
                enabled: can,
                tip: "新建概念",
            },
            ToolItem::Button {
                icon: ToolIcon::Link,
                enabled: can && concepts >= 1,
                tip: "连接两个概念 (Ctrl+L)",
            },
            ToolItem::Button {
                icon: ToolIcon::Phrase,
                enabled: can && concepts >= 1,
                tip: "改命题短语 (F2)",
            },
            ToolItem::Sep,
            ToolItem::Button {
                icon: ToolIcon::Arrange,
                enabled: can,
                tip: "整理布局",
            },
            ToolItem::Button {
                icon: ToolIcon::Pin,
                enabled: can && concepts >= 1,
                tip: "钉住位置",
            },
            ToolItem::Button {
                icon: ToolIcon::FocusQ,
                enabled: can,
                tip: "聚焦问题",
            },
            ToolItem::Button {
                icon: ToolIcon::Fit,
                enabled: has_canvas,
                tip: "适应画布 (Home)",
            },
        ]
    }

    /// 自绘窗口按钮：先铺底再写字形，顺序颠倒会把字形盖掉，所以不走 `egui::Button`。
    pub(crate) fn win_btn(
        &self,
        ui: &mut egui::Ui,
        glyph: WinGlyph,
        title: &str,
        hot: Color32,
    ) -> bool {
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(mw_core::design::component::WINDOW_CONTROL_W, TITLEBAR_H),
            egui::Sense::click(),
        );
        let hovered = resp.hovered();
        if hovered {
            let fill = if resp.is_pointer_button_down_on() {
                hot.gamma_multiply(0.86)
            } else {
                hot
            };
            ui.painter().rect_filled(rect, 0, fill);
        }
        let fg = if hovered && hot == CLOSE_HOT {
            Color32::WHITE
        } else {
            self.chrome.text
        };
        paint_win_glyph(ui.painter(), rect.center(), glyph, fg);
        resp.on_hover_text(title).clicked()
    }
}
