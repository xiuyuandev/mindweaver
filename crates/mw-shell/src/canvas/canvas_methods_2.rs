use super::*;

impl Canvas {
    pub fn show(&mut self, ui: &mut egui::Ui, render_state: &egui_wgpu::RenderState) {
        let available = ui.available_size();
        if available.x < 2.0 || available.y < 2.0 {
            return;
        }
        // 键盘交接给面板时，画布连「可聚焦」都要摘掉：只交出手里的焦点没用，
        // `Tab` 一转又转回画布，焦点锁又把 `Tab` 锁死在这里——面板就此键盘不可达。
        let (rect, response) = canvas_area(ui, available, self.wants_keys);
        // 轨迹里认出画布来（只在 `MW_TRACE` 下工作）：面板开合之后焦点还在不在画布上，
        // 是这次修复的唯一判据，而画布不按焦点画任何东西，像素问不出这个答案。
        crate::trace::name(ui.ctx(), response.id, "canvas");
        let viewport = [rect.width(), rect.height()];
        self.last_viewport = viewport;
        self.last_rect = rect;
        let ppp = ui.ctx().pixels_per_point();
        self.camera.ppp = ppp;
        // 本帧时钟：真机是墙钟，金样是 M16 的帧计数虚拟时钟（`raw_input.time`）。
        // 按时间老化的反馈（重叠告警边）只读这里，截图才拍得住。
        self.now = ui.input(|i| i.time);

        // 演示相机自己走 350ms 缓动：每帧从这里取插值结果，结束时正好落在目标节点上。
        let demo_now = Instant::now();
        let demo_active = self.demo.is_some();
        let demo_animating = if let Some(demo) = self.demo.as_ref() {
            self.camera = demo.camera(demo_now);
            !demo.settled(demo_now)
        } else {
            false
        };
        if demo_animating {
            ui.ctx().request_repaint();
        }

        let fitted_now = !self.fitted && !demo_active;
        if fitted_now {
            self.fit(viewport);
            self.fitted = true;
        }
        // 面板开合（`F8` 大纲 / `F9` 检查器 / 标签条出现）会改变画布矩形：画布中心在
        // 窗口里挪了，画面就会跟着整体平移——打开检查器时整张图往左滑、右边被面板
        // 吃掉，就是这条。把相机同向平移 `shift / zoom`，屏幕上的世界点原地不动，
        // **缩放不变**（13 §4.20 的口径）。整窗尺寸变了说明是窗口缩放，此时不补偿。
        let window_size = ui.ctx().input(|i| i.viewport_rect().size());
        if !demo_active && !fitted_now && self.last_window_size == Some(window_size) {
            if let Some(prev) = self.last_view_center {
                self.pan_for_viewport_shift(rect.center() - prev);
            }
        }
        self.last_view_center = Some(rect.center());
        self.last_window_size = Some(window_size);
        // 面板握着键盘时用户点了一下画布，就是把键盘要回去。必须同一帧讨回焦点：
        // 画布上一帧不可聚焦，egui 不会替我们补这次 request_focus，点完的第一下
        // 按键会整个落空。
        if !self.wants_keys && response.clicked() {
            self.wants_keys = true;
            self.refocus_canvas = true;
        }
        // 检查器提交后会讨回画布焦点；这一帧不能再跑画布快捷键——同一次 Enter
        // 既被检查器用来提交，又被这里当成「加同级节点」的旧问题就出在这。
        let just_refocused = if self.wants_keys && self.editor.is_none() && self.refocus_canvas {
            response.request_focus();
            self.refocus_canvas = false;
            true
        } else {
            false
        };
        // Tab / 方向键在 egui 里默认是「移动焦点」，Escape 默认「交出焦点」，而画布是
        // 这些键的语义主人，必须先把它们锁在自己身上（见 `canvas_key_lock`）。
        ui.ctx().memory_mut(|m| {
            m.set_focus_lock_filter(response.id, canvas_key_lock());
        });

        // 上一帧 TextEdit 里的字符已经落到 editor.text：先按新文本重排，
        // 节点盒才会跟着输入变宽/变高。晚了会看到编辑器先溢出、下一帧才追上。
        self.sync_editing_layout();

        // 悬停工具条（＋ / ⋯）挂在当前悬停节点的外侧，尺寸是屏幕空间的。
        // 它必须先于指针处理登记：按在按钮上的那一下若被当成画布按下，
        // 就会顺带拉框/拖拽——这正是命中区必须离开节点盒的原因。
        let bar = if demo_active {
            None
        } else {
            self.hover_bar(rect, viewport)
        };
        self.chrome_gate = bar.map(|b| b.buttons().expand(2.0));

        let local = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p))
            .map(|p| Point::new(p.x - rect.min.x, p.y - rect.min.y));

        let mut dirty = false;
        let editing = self.editor.is_some();

        // 带子编辑器也抢键盘：在里面按 Home/Shift+Home 是选行首，不该顺手把画布重新适应一次。
        // 修饰键必须精确匹配——否则 Shift+Home（编辑框里选到行首）会连带触发「适应视图」，
        // 整张图缩放跳一下（2026-10-27 脚本里撞出来的）。
        if !editing && self.focus_edit.is_none() && !self.input_blocked {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::Home)) {
                self.fit(viewport);
                dirty = true;
            }
            let (zoom_in, zoom_out) = ui.input(|i| {
                let cmd = i.modifiers.command;
                (
                    cmd && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)),
                    cmd && i.key_pressed(egui::Key::Minus),
                )
            });
            if zoom_in || zoom_out {
                // 倍率只有一处定义（缩放条与小地图滚轮共用同一档，`canvas_chrome::ZOOM_STEP`）。
                let factor = if zoom_in {
                    crate::canvas_chrome::ZOOM_STEP
                } else {
                    1.0 / crate::canvas_chrome::ZOOM_STEP
                };
                // 键盘缩放也要带锚点：只改 `zoom` 的话画面会绕世界原点漂，和滚轮缩放
                // 变成两套手感。指针在画布内就以指针为锚，否则退回视口中心——但**浮层上的
                // 指针不算画布指针**：停在右下缩放条上按 `Ctrl+=`，锚点必须是视口中心，
                // 不能把画面钉到缩放条那一角。闸门复用画布别处同名的那道判定。
                let center = Point::new(viewport[0] * 0.5, viewport[1] * 0.5);
                let at = local
                    .filter(|_| !self.pointer_over_overlay(ui))
                    .unwrap_or(center);
                self.zoom_step_at(at, factor, ZoomSrc::Key);
                dirty = true;
            }
        }
        dirty |= self.handle_pointer(ui, &response, rect, local, viewport);

        // 提交/取消键要在 TextEdit 之前吃掉：Enter 不能让 TextEdit 先插入换行。
        // 真正的提交放到编辑器 UI 之后，这样同一帧里先到的字符不会丢。
        //
        // `Tab` 也必须在这里接走。以前不接，egui 把焦点转给下一个控件，编辑框失焦
        // 顺带提交，树上却什么都没建——「Tab、打字、Tab、打字」这条最常用的建树节奏
        // 从第二下就断了，而且焦点还漏到壳层的某个按钮上（`docs/07` §3.2 要求的是
        // 「提交编辑 → 新建子节点」）。
        //
        // 取键一律用「精确修饰键」（`take_exact_key`）：`consume_key` 会忽略多按的
        // `Shift`，于是 `Shift+Enter`（软换行）会被纯 `Enter`（提交 + 新建同级）吞掉。
        // `Shift+Enter` 必须排在纯 `Enter` 之前——两个都命中时，窄的那个先拿走。
        let (commit_enter, commit_tab, cancel, soft_enter) =
            if crate::canvas::edit_keys_owned(editing, self.editor.as_ref().is_some_and(|e| e.ime))
            {
                let esc = canvas_methods_3::take_exact_key(ui, Modifiers::NONE, Key::Escape);
                let soft =
                    !esc && canvas_methods_3::take_exact_key(ui, Modifiers::SHIFT, Key::Enter);
                let enter = !esc
                    && !soft
                    && canvas_methods_3::take_exact_key(ui, Modifiers::NONE, Key::Enter);
                let tab = !esc
                    && !soft
                    && !enter
                    && canvas_methods_3::take_exact_key(ui, Modifiers::NONE, Key::Tab);
                (enter, tab, esc, soft)
            } else {
                (false, false, false, false)
            };
        // 软换行在光标处插一个 `\n`：编辑框是 multiline，但 Enter 已被摘走，换行只能由
        // 画布插进文本；插完立刻重排，节点盒这一帧就要变高。
        if soft_enter {
            dirty |= self.insert_soft_newline(ui);
        }
        // 提交之后要不要顺手建一个节点：思维导图里 `Enter` = 同级、`Tab` = 子级；
        // 概念图没有「同级」这回事，两个键都只提交（与 CmapTools 一致）。
        let edit_follow_up =
            crate::canvas::commit_follow_up(commit_enter, commit_tab, self.concept_map());
        if cancel {
            self.cancel_edit();
            dirty = true;
        }

        // 落字即进编辑：非编辑态、画布握着焦点、选中了节点时，敲字直接打开编辑框
        // （等价「全选 + 输入」）。空白处敲字什么都不做——没有落点。
        // 关系线选中时不接：那里落字应该改关系线的标签，归 F2 那条路径。
        if response.has_focus()
            && !just_refocused
            && !self.input_blocked
            && self.editor.is_none()
            && self.summary_edit.is_none()
            && self.rel_edit.is_none()
            && self.relationship.is_empty()
            && self.type_to_edit(ui, viewport)
        {
            dirty = true;
        }

        if self.editor.is_none()
            && self.summary_edit.is_none()
            && self.rel_edit.is_none()
            && !just_refocused
        {
            dirty |= self.shortcuts(ui, &response, viewport);
        }

        // 点击：先结束编辑，再按**重排后**的场景做选择/折叠。
        //
        // 必须用 `clicked_by(Primary)` 而不是 `clicked()`：后者把「焦点控件上按
        // 空格/回车」也算作点击（egui 的无障碍激活语义）。画布是焦点控件，于是
        // 回车加同级节点会在同一帧被当成「点了画布」，刚打开的编辑器立刻被提交。
        //
        // 框选松手那一帧 egui 仍可能判成单击（我们的 4px 阈值小于它的 6px），所以
        // 先看 `swallow_click`——否则刚框出来的一组会被「点空白清空选择」立刻抹掉。
        let swallowed = std::mem::take(&mut self.swallow_click);
        if crate::trace::is_enabled() && response.clicked_by(egui::PointerButton::Primary) {
            // 命中点名到节点 id：靠坐标反推「这一点上是哪个节点」既脆又费事，
            // 而拖拽落点、编辑框定位这些验收都得先知道点在谁身上。
            let hit = self
                .hit_at(local, viewport)
                .and_then(|t| owner_node(&self.scene, t))
                .and_then(|n| self.scene.ids.get(n as usize))
                .cloned()
                .unwrap_or_else(|| "none".into());
            let at = ui.input(|i| i.pointer.interact_pos());
            crate::trace::event(&format!(
                "canvas_click at=({:.0},{:.0}) blocked={} swallowed={swallowed} overlay={} hit={hit}",
                at.map_or(-1.0, |p| p.x),
                at.map_or(-1.0, |p| p.y),
                self.input_blocked,
                self.pointer_over_overlay(ui)
            ));
        }
        if !swallowed
            && !self.input_blocked
            && self.summary_edit.is_none()
            && self.rel_edit.is_none()
            && !self.pointer_over_overlay(ui)
            && response.clicked_by(egui::PointerButton::Primary)
        {
            if self.editor.is_some() {
                self.commit_edit();
                dirty = true;
            }
            response.request_focus();
            let additive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
            let subtract = ui.input(|i| i.modifiers.alt);
            // 顺序：折叠箭头 → 注解 → 节点 → 空白。
            // 箭头在节点盒子外面，所以必须排在注解前面，否则边界里的折叠点不动。
            let hit = self.hit_at(local, viewport);
            // 插入关系线只认节点盒：点在另一个主题上就建线并吃掉这一击；
            // 点回源主题/空白/注解/关系线 = 退出插入态，这一击照常往下走。
            let target_node = match hit {
                Some(Target::Node(n)) => self.scene.ids.get(n as usize).cloned(),
                _ => None,
            };
            let commit_to = match rel_link_step(self.rel_link.as_deref(), target_node.as_deref()) {
                RelLinkStep::Commit(to) => Some(to),
                RelLinkStep::Cancel => {
                    // 取消的原因分开记：点回源主题、点空白、点到折叠点/注解/线
                    // 在证据里应该是三种不同的手势，别混成一个「click」。
                    let reason = match (&target_node, hit) {
                        (Some(_), _) => "self",
                        (None, None) => "blank",
                        (None, Some(_)) => "other",
                    };
                    self.cancel_rel_link(reason);
                    None
                }
                RelLinkStep::Idle => None,
            };
            if let Some(to) = commit_to {
                // 别的动作落地：折叠圆点的双击记忆作废（M32）。
                self.toggle_clicks.clear();
                dirty |= self.commit_relationship_link(&to);
            } else {
                // 点任何别的东西都先退出关系线选择；下面命中关系线时再设回来。
                // **例外**：按住 Ctrl/Shift 点另一条线是加选/减选（CM-17），不能先清空。
                let rel_additive = matches!(hit, Some(Target::Relationship(_))) && additive;
                if !rel_additive {
                    self.relationship.clear();
                }
                if !matches!(hit, Some(Target::Toggle(_))) {
                    // 这一击没落在折叠圆点上：上一击不能跟它配成双击（M32）。
                    self.toggle_clicks.clear();
                }
                match hit {
                    Some(Target::Relationship(i)) => {
                        if let Some(r) = self.scene.relationships.get(i as usize) {
                            let from = self.scene.ids[r.from as usize].clone();
                            let to = self.scene.ids[r.to as usize].clone();
                            let sel = RelationshipSel {
                                from: from.clone(),
                                id: r.id.clone(),
                                to,
                            };
                            if additive {
                                // 已选中的再点一次 = 减选；这是「框选错了撤一条」的唯一手势。
                                self.relationship.toggle(sel);
                                crate::trace::event(&format!(
                                    "select relationship {} {} additive=1 n={}",
                                    from,
                                    r.id,
                                    self.relationship.len()
                                ));
                            } else {
                                crate::trace::event(&format!(
                                    "select relationship {} {}",
                                    from, r.id
                                ));
                                self.relationship.select_only(sel);
                            }
                            self.annotations.clear();
                            self.sel = Selection::default();
                            dirty = true;
                        }
                    }
                    Some(Target::Toggle(n)) => {
                        // M32：圆点的第二击是「下钻一层」，不是再折一次。
                        // 第一击按普通单击处理并记下现场；第二击命中同一点时，
                        // 先撤掉第一击（回到点之前的状态），再按那个状态下发下钻命令。
                        let now = ui.input(|i| i.time);
                        let pos = local.unwrap_or_default();
                        let id = self.scene.ids.get(n as usize).cloned().unwrap_or_default();
                        let provisional = ToggleClick {
                            id,
                            at: now,
                            pos: mw_ui::drill_click::ClickPoint::new(pos.x, pos.y),
                            collapsed_before: self.scene.node(n).collapsed,
                            undo_depth: self.session.doc.undo_depth(),
                            pushed: false,
                        };
                        // 别的点击（节点/关系线/空白）会把这条记忆清掉——见下面各分支。
                        if self.toggle_clicks.is_double(&provisional) {
                            if let Some(first) = self.toggle_clicks.take() {
                                dirty |= self.toggle_drill(n, &first);
                            }
                        } else {
                            let pushed = self.toggle(n);
                            let mut landed = provisional;
                            landed.undo_depth = self.session.doc.undo_depth();
                            landed.pushed = pushed;
                            self.toggle_clicks.remember(landed);
                            dirty = true;
                        }
                    }
                    Some(Target::Group(i)) => {
                        // 点分组框 = 选中它圈住的全部概念（CM-18）。不新增拖动类型：
                        // 选中之后就是普通的多选拖动，整组一起走。
                        let members = self
                            .session
                            .doc
                            .groups
                            .get(i as usize)
                            .map(|g| g.members.clone())
                            .unwrap_or_default();
                        crate::trace::event(&format!("select group members={}", members.len()));
                        self.annotations.clear();
                        self.relationship.clear();
                        let ids: Vec<String> = members
                            .into_iter()
                            .filter(|m| self.scene.find(m).is_some())
                            .collect();
                        if let Some(first) = ids.first().cloned() {
                            self.select_many(&ids, &first);
                            dirty = true;
                        }
                    }
                    _ => {
                        if let Some(ann) = self.annotation_at(local, viewport) {
                            crate::trace::event(&format!(
                                "select {} {}",
                                match ann.kind {
                                    AnnotationKind::Summary => "summary",
                                    AnnotationKind::Boundary => "boundary",
                                },
                                ann.id
                            ));
                            dirty |= self.select_annotation(ann, additive);
                        } else {
                            match hit {
                                Some(t) => {
                                    if let Some(node) = owner_node(&self.scene, t) {
                                        // 点节点 = 退出注解选择（两者互斥）。
                                        self.annotations.clear();
                                        dirty |= self.sel.click_with(node, additive, subtract);
                                    }
                                }
                                None => {
                                    self.annotations.clear();
                                    dirty |= self.sel.clear();
                                }
                            }
                        }
                    }
                }
            }
        }

        // 注解选中环：每条都画一圈 2px 强调色，多选时能看出「删的是这几条」。
        // 先收集、贴图后再画——它曾经在这个位置直接画，被合成纹理整张盖掉。
        let mut ann_rings: Vec<egui::Rect> = Vec::new();
        for ann in self.annotations.clone() {
            let world_rect = match ann.kind {
                AnnotationKind::Boundary => self
                    .scene
                    .boundaries
                    .iter()
                    .find(|b| b.id == ann.id)
                    .map(|b| b.rect),
                AnnotationKind::Summary => self
                    .scene
                    .summaries
                    .iter()
                    .find(|s| s.id == ann.id)
                    .map(|s| s.topic),
            };
            if let Some(wr) = world_rect {
                let a = self
                    .camera
                    .world_to_screen(Point::new(wr.x, wr.y), viewport);
                let b = self
                    .camera
                    .world_to_screen(Point::new(wr.right(), wr.bottom()), viewport);
                let origin = self.last_rect.min;
                let ring = egui::Rect::from_min_max(
                    egui::pos2(origin.x + a.x, origin.y + a.y),
                    egui::pos2(origin.x + b.x, origin.y + b.y),
                )
                .expand(3.0);
                ann_rings.push(ring);
            }
        }

        // 带子是画布上的覆盖层：落在带子里的双击归带子（进编辑），不能再当「点空白新建」。
        // 用这一击自己的位置判：`local` 来自 hover_pos，离屏/脚本下可能为空。
        let over_band = response
            .interact_pointer_pos()
            .is_some_and(|p| self.focus_band.is_some_and(|b| b.contains(p)));
        // 双击节点 → 行内编辑（第二击同时会 clicked，上面的选择已把节点选中）。
        if !self.input_blocked
            && !over_band
            && response.double_clicked_by(egui::PointerButton::Primary)
            && self.editor.is_none()
            && self.summary_edit.is_none()
            && self.rel_edit.is_none()
        {
            match self.hit_at(local, viewport) {
                Some(Target::Node(n)) => {
                    self.start_edit(n, viewport);
                    dirty = true;
                }
                Some(Target::Relationship(i)) => {
                    let sel = self
                        .scene
                        .relationships
                        .get(i as usize)
                        .map(|r| RelationshipSel {
                            from: self.scene.ids[r.from as usize].clone(),
                            id: r.id.clone(),
                            to: self.scene.ids[r.to as usize].clone(),
                        });
                    if let Some(sel) = sel {
                        if self.start_rel_edit(sel, viewport, false) {
                            dirty = true;
                        }
                    }
                }
                Some(Target::SummaryTopic(i)) => {
                    if let Some(ann) = self
                        .scene
                        .summaries
                        .get(i as usize)
                        .and_then(|s| self.scene.ids.get(s.owner as usize))
                        .map(|owner| AnnotationSel {
                            parent: owner.clone(),
                            kind: AnnotationKind::Summary,
                            id: self.scene.summaries[i as usize].id.clone(),
                        })
                    {
                        if self.start_summary_edit(&ann, viewport) {
                            dirty = true;
                        }
                    }
                }
                // 双击空白 = 新建自由主题。XMind 的浮动主题就是这么起的：
                // 不用先去菜单里找命令，随手一点就能在任意位置开一张便签。
                None => {
                    if let Some(local) = local {
                        let world = self.camera.screen_to_world(local, viewport);
                        dirty |= self.add_floating_world(world.x, world.y);
                    }
                }
                Some(_) => {}
            }
        }

        // 右键：选中并请壳层弹菜单。右击未选中的节点先把它变成唯一选中；
        // 右击已选组内的节点保留整组，壳层据此弹多选菜单。
        if !self.input_blocked
            && !self.pointer_over_overlay(ui)
            && response.clicked_by(egui::PointerButton::Secondary)
        {
            if self.editor.is_some() {
                self.commit_edit();
                dirty = true;
            }
            response.request_focus();
            // 注解优先于节点：边界内部可能压着节点，但 `annotation_at` 已经保证
            // 「点在节点盒里就判给节点」，所以这里先问注解不会抢走节点菜单。
            let hit = self.hit_at(local, viewport);
            let rel = match hit {
                Some(Target::Relationship(i)) => {
                    self.scene
                        .relationships
                        .get(i as usize)
                        .map(|r| RelationshipSel {
                            from: self.scene.ids[r.from as usize].clone(),
                            id: r.id.clone(),
                            to: self.scene.ids[r.to as usize].clone(),
                        })
                }
                _ => None,
            };
            let ann = self.annotation_at(local, viewport);
            let group = match hit {
                Some(Target::Group(i)) => self
                    .session
                    .doc
                    .groups
                    .get(i as usize)
                    .map(|g| g.id.clone()),
                _ => None,
            };
            let node = hit.and_then(|t| owner_node(&self.scene, t));
            let target = if let Some(rel) = rel {
                crate::trace::event(&format!("context relationship {} {}", rel.from, rel.id));
                self.relationship.select_only(rel);
                self.annotations.clear();
                self.sel = Selection::default();
                dirty = true;
                ContextTarget::Relationship
            } else if let Some(gid) = group {
                crate::trace::event(&format!("context group {gid}"));
                self.context_group = Some(gid);
                self.relationship.clear();
                self.sel = Selection::default();
                self.annotations.clear();
                dirty = true;
                ContextTarget::Group
            } else if let Some(ann) = ann {
                crate::trace::event(&format!("context annotation {}", ann.id));
                self.annotations = vec![ann];
                self.relationship.clear();
                self.sel = Selection::default();
                dirty = true;
                ContextTarget::Annotation
            } else {
                // 右键落在一个节点上（或空白）：这一击要么选中该节点、要么保留原有节点
                // 选中；无论哪种，关系线/注解两道都必须先退场（M26 互斥）。
                self.clear_foreign_selection(SelectionLane::Node, "context-node");
                if let Some(idx) = node {
                    if !self.sel.contains(idx) {
                        self.sel.replace(vec![idx]);
                        dirty = true;
                    }
                }
                if node.is_some() {
                    ContextTarget::Node
                } else {
                    ContextTarget::Blank
                }
            };
            let pos = ui
                .input(|i| i.pointer.latest_pos())
                .unwrap_or_else(|| response.rect.center());
            self.context = Some(ContextRequest { pos, target });
        }

        // 悬停工具条：＋ 加子主题（与 Tab 同源），⋯ 弹节点菜单（与右键同源）。
        // 交互必须放在 `Frame` 构建之前，新节点才会在同一帧进入场景与编辑器。
        let plus_id = egui::Id::unique("mw-canvas-node-plus");
        let more_id = egui::Id::unique("mw-canvas-node-more");
        let plus_response = bar.map(|b| ui.interact(b.plus, plus_id, Sense::click()));
        let more_response = bar
            .filter(|b| !b.compact)
            .map(|b| ui.interact(b.more, more_id, Sense::click()));
        if bar.is_some() {
            // 语义名挂在控件自己的 id 上，`MW_TRACE` 的焦点轨迹里就能直接认出它们；
            // 点击再补一条 `event`，因为鼠标点击不一定抢焦点。
            crate::trace::name(ui.ctx(), plus_id, "node-plus");
            crate::trace::name(ui.ctx(), more_id, "node-more");
        }
        if let (Some(b), Some(resp)) = (bar, plus_response.as_ref()) {
            if !self.input_blocked && resp.clicked() {
                crate::trace::event("node-plus");
                // 工具条属于被悬停的那个节点：先把它变成唯一选中，再加子主题。
                self.select_node_only(b.node, "hover-add");
                dirty |= self.add_child();
            }
            resp.clone().on_hover_text("添加子主题（Tab）");
        }
        if let (Some(b), Some(resp)) = (bar, more_response.as_ref()) {
            if !self.input_blocked && resp.clicked() {
                crate::trace::event("node-more");
                self.select_node_only(b.node, "hover-more");
                self.context = Some(ContextRequest {
                    pos: b.more.left_bottom() + Vec2::new(0.0, design::space::XS),
                    target: ContextTarget::Node,
                });
                dirty = true;
            }
            resp.clone().on_hover_text("更多操作");
        }

        let hot = self.hit_at(local, viewport);
        // 跨度手柄是屏幕空间的小圆点：命中一次给光标用，真正按下在 `handle_pointer` 里。
        let span_hover = self.span_handle_at(local, viewport).is_some();
        let relink_hover = self.relink_handle_at(local, viewport).is_some();
        // 指针从节点移到按钮要跨过 `HOVER_GAP`：这一段落在节点盒外，`hit_at`
        // 会说「空白」。不把缝隙并进悬停区，工具条会在指尖前一闪就没。
        let pointer_over_bar = bar.is_some_and(|b| {
            ui.input(|i| i.pointer.hover_pos())
                .is_some_and(|p| b.hover_zone().contains(p))
        });
        let hover = if self.editor.is_none()
            && !self.input_blocked
            && self.drag.is_none()
            // 指针压在工具条上时，画布本身已被按钮盖住（`response.hovered()` 会转 false）。
            // 这两个条件必须取或，否则工具条刚出现就会被自己的按钮「赶走」。
            && (response.hovered() || pointer_over_bar)
            && !response.dragged()
        {
            hot.and_then(|t| match t {
                Target::Toggle(n) | Target::Node(n) | Target::Label(n, _) => Some(n),
                _ => None,
            })
            .or(if pointer_over_bar {
                bar.map(|b| b.node)
            } else {
                None
            })
        } else {
            None
        };
        let hover = if self.hover_enabled { hover } else { None };
        if hover != self.hovered {
            // 悬停概念会点亮它两端的命题线（`paint_relationship_emphasis` 第三档）。
            // 记一条 trace：截图能证明「有线亮了」，只有 trace 能证明「亮的是这个概念该亮的那些线」。
            if let Some(n) = hover {
                let incident = self
                    .scene
                    .relationships
                    .iter()
                    .filter(|r| r.from == n || r.to == n)
                    .count();
                if crate::trace::is_enabled() {
                    let node = self
                        .scene
                        .ids
                        .get(n as usize)
                        .map(String::as_str)
                        .unwrap_or("?");
                    crate::trace::event(&format!("rel_adjacent node={node} links={incident}"));
                }
            }
            self.hovered = hover;
            dirty = true;
        }
        // 折叠圆点 / 概要 / 边界的悬停反馈（打磨轮 37）：节点与关系线已有自己的反馈，
        // 这里只补 `hit.rs` 能命中、画面上却「没有指得到」的那三类。门与节点悬停同一组：
        // 编辑/拖拽/浮层锁定时不亮，避免和工具条抢注意力。
        let detail = if self.hover_enabled
            && self.editor.is_none()
            && self.summary_edit.is_none()
            && self.rel_edit.is_none()
            && !self.input_blocked
            && self.drag.is_none()
            && !response.dragged()
            && (response.hovered() || pointer_over_bar)
        {
            hot.and_then(super::hover::hover_detail_of)
        } else {
            None
        };
        if detail != self.hover_detail {
            // 截图只能证明「亮了一点」，trace 才能证明「亮的是这一条」。
            let named = match detail {
                Some(super::hover::HoverDetail::Toggle(n)) => Some(format!(
                    "toggle {}",
                    self.scene
                        .ids
                        .get(n as usize)
                        .map(String::as_str)
                        .unwrap_or("?")
                )),
                Some(super::hover::HoverDetail::Summary(i)) => Some(format!(
                    "summary {}",
                    self.scene
                        .summaries
                        .get(i as usize)
                        .map(|s| s.id.as_str())
                        .unwrap_or("?")
                )),
                Some(super::hover::HoverDetail::Boundary(i)) => Some(format!(
                    "boundary {}",
                    self.scene
                        .boundaries
                        .get(i as usize)
                        .map(|b| b.id.as_str())
                        .unwrap_or("?")
                )),
                None => None,
            };
            crate::trace::event(&format!(
                "hover_detail {}",
                named.as_deref().unwrap_or("none")
            ));
            self.hover_detail = detail;
            dirty = true;
        }
        // 关系线没有节点那种悬停工具条，光看指针没人知道它能点。悬停时把这条线自己提亮，
        // 选中态另有一套更实的强调（`paint_relationship_emphasis`）。存 id 不存下标：
        // 重建会重排 `scene.relationships`，下标一漂就强调错线。
        let rel_hover = if self.hover_enabled
            && self.editor.is_none()
            && !self.input_blocked
            && self.drag.is_none()
            && !response.dragged()
            && (response.hovered() || pointer_over_bar)
        {
            match hot {
                Some(Target::Relationship(i)) => self
                    .scene
                    .relationships
                    .get(i as usize)
                    .map(|r| (self.scene.ids[r.from as usize].clone(), r.id.clone())),
                _ => None,
            }
        } else {
            None
        };
        if rel_hover != self.hovered_rel {
            // 悬停态本身不可断言（只有像素），所以把「指向哪条线」记进 trace：
            // 截图只能证明「亮了」，trace 才能证明「亮的是这条」。
            if let Some((from, id)) = rel_hover.as_ref() {
                crate::trace::event(&format!("rel_hover {from} {id}"));
            }
            self.hovered_rel = rel_hover;
            dirty = true;
        }
        let dt = ui.ctx().input(|i| i.stable_dt).clamp(0.0, 0.1);
        let hover_target = if self.hovered.is_some() { 1.0 } else { 0.0 };
        // 框选预览也算「有东西被选中」：否则选中环的透明度停在 0，预览算出来了也画不出来
        // （打磨轮 9 的第一版就是这样，A/B 逐字节零差异）。
        let select_target = if self.sel.is_empty()
            && self
                .marquee_preview
                .as_ref()
                .is_none_or(|preview| preview.is_empty())
        {
            0.0
        } else {
            1.0
        };
        self.hover_alpha = advance_alpha(
            self.hover_alpha,
            hover_target,
            dt,
            HOVER_TRANSITION,
            self.reduce_motion,
        );
        self.select_alpha = advance_alpha(
            self.select_alpha,
            select_target,
            dt,
            SELECT_TRANSITION,
            self.reduce_motion,
        );
        let detail_target = if self.hover_detail.is_some() {
            1.0
        } else {
            0.0
        };
        self.hover_detail_alpha = advance_alpha(
            self.hover_detail_alpha,
            detail_target,
            dt,
            HOVER_TRANSITION,
            self.reduce_motion,
        );
        if (self.hover_alpha - hover_target).abs() > 0.001
            || (self.select_alpha - select_target).abs() > 0.001
            || (self.hover_detail_alpha - detail_target).abs() > 0.001
        {
            ui.ctx().request_repaint();
        }
        let panning = self.drag.is_some_and(|d| d.kind == DragKind::Pan);
        let boxed = self.drag.is_some_and(|d| d.drawing);
        let moving = self.moving.is_some();
        let linking = self.link.is_some();
        let space_ready = ui.input(|i| i.key_down(egui::Key::Space));
        let over_summary_edit = self.summary_edit.as_ref().is_some_and(|ed| {
            ui.input(|i| i.pointer.hover_pos())
                .is_some_and(|p| ed.rect.contains(p))
        });
        let over_rel_edit = self.rel_edit.as_ref().is_some_and(|ed| {
            ui.input(|i| i.pointer.hover_pos())
                .is_some_and(|p| ed.rect.contains(p))
        });
        ui.ctx().set_cursor_icon(if demo_active {
            CursorIcon::Default
        } else if self.moving.as_ref().is_some_and(Moving::illegal) {
            // 拖到自己肚子里：禁止光标 + 红色指示（docs/06 §5）。静默会让用户以为拖拽卡住。
            CursorIcon::NotAllowed
        } else if linking || self.rel_link.is_some() {
            CursorIcon::Crosshair
        } else if panning || moving {
            CursorIcon::Grabbing
        } else if boxed {
            CursorIcon::Crosshair
        } else if self.editor.is_some() || over_summary_edit || over_rel_edit {
            CursorIcon::Text
        } else if span_hover {
            CursorIcon::ResizeVertical
        } else if relink_hover {
            CursorIcon::Crosshair
        } else if hot.is_some() || pointer_over_bar {
            CursorIcon::PointingHand
        } else if response.hovered() {
            // 空白处左键已经是框选，光标要诚实：十字而不是抓手。
            if space_ready {
                CursorIcon::Grab
            } else {
                CursorIcon::Crosshair
            }
        } else {
            CursorIcon::Default
        });

        if dirty {
            ui.ctx().request_repaint();
        }

        // 只统计连续重绘（拖动/缩放/演示步进）；空闲帧的间隔没有意义（docs/08 §6）。
        self.compositor
            .set_low_render(self.budget.tick(dirty || demo_animating));

        let mut selected: Vec<u32> = if demo_active {
            Vec::new()
        } else {
            self.sel.ids().to_vec()
        };
        // 框选进行中就把「松手会选中谁」画出来：以前只在松手时结算，橡皮筋拉了半个屏幕
        // 也不知道到底框住了几颗，只能松手再拉一次（打磨轮 9）。
        // 预览集合只在**变化**时写一行轨迹——每帧都写会把轨迹淹掉。
        let prev_preview = self.marquee_preview.take();
        if !demo_active {
            if let Some(drag) = self
                .drag
                .filter(|d| d.drawing && d.kind == DragKind::Marquee)
            {
                let picked = marquee_pick(&self.camera, &self.scene, rect, &drag, viewport);
                let picked_n = picked.len();
                let ids = apply_marquee(self.sel.ids(), picked, drag.additive, drag.subtract);
                if crate::trace::is_enabled() && prev_preview.as_deref() != Some(ids.as_slice()) {
                    crate::trace::event(&format!(
                        "marquee preview picked={picked_n} selected={}",
                        ids.len()
                    ));
                }
                selected = ids.clone();
                self.marquee_preview = Some(ids);
            }
        }
        let editing_idx = if demo_active {
            None
        } else {
            self.editor.as_ref().and_then(|e| self.scene.find(&e.id))
        };
        // 幽灵必须由合成器画：它要和真节点用同一套着色与文字，egui 层拼不出来。
        let ghost = self.moving.as_ref().map(|m| mw_render::Ghost {
            nodes: m.nodes.clone(),
            offset: m.world_offset(self.camera.zoom),
            illegal: m.illegal(),
        });
        let guides: Vec<mw_render::GuideLine> = self
            .moving
            .as_ref()
            .map(|m| m.snap)
            .map(|s| {
                [s.guide_x.map(|g| (g, true)), s.guide_y.map(|g| (g, false))]
                    .into_iter()
                    .flatten()
                    .map(|(g, vertical)| mw_render::GuideLine {
                        at: g.at,
                        from: g.from,
                        to: g.to,
                        vertical,
                    })
                    .collect()
            })
            .unwrap_or_default();
        // 过渡动画：渲染用插值帧，命中/面板/选择仍按目标场景算。
        // 动画跑完就丢掉起点场景，避免以后每帧都白算一次 blend。
        if self.motion.as_ref().is_some_and(|m| !m.active()) {
            self.motion = None;
        }
        let animated = self.motion.as_ref().map(|m| m.scene(&self.scene));
        if self.motion.is_some() {
            ui.ctx().request_repaint();
        }
        // 演示用高亮副本 + 深色主题；平时走插值帧或目标场景。
        let scene_for_frame: &Scene = if let Some(demo) = self.demo.as_ref() {
            demo.scene()
        } else {
            animated.as_ref().unwrap_or(&self.scene)
        };
        let theme_for_frame = self
            .demo
            .as_ref()
            .map(|d| d.theme())
            .unwrap_or(&self.session.theme);
        let frame_camera = self
            .demo
            .as_ref()
            .map(|d| d.camera(demo_now))
            .unwrap_or(self.camera);
        let frame = mw_render::Frame {
            scene: scene_for_frame,
            theme: theme_for_frame,
            camera: frame_camera,
            selected,
            hovered: if demo_active { None } else { self.hovered },
            hover_alpha: if demo_active { 1.0 } else { self.hover_alpha },
            select_alpha: if demo_active { 1.0 } else { self.select_alpha },
            editing: editing_idx,
            ghost,
            guides,
            viewport: Some(viewport),
        };
        let mut layers: [LayerGeom; LAYERS] = build_layers(&frame);
        if let Some(text) = self.text.as_mut() {
            let update = text.paint(&frame, &mut layers);
            self.compositor.upload_atlas(&update);
            if self.compositor.take_atlas_desync() {
                text.reset_atlas();
            }
        }

        let physical = [
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        ];
        let recreated = self.compositor.render_texture(&frame, &layers, physical);

        if let Some(view) = self.compositor.target_view() {
            let mut renderer = render_state.renderer.write();
            match self.texture {
                Some(_) if !recreated => {}
                Some(id) => renderer.update_egui_texture_from_wgpu_texture(
                    &render_state.device,
                    view,
                    eframe::wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    self.texture = Some(renderer.register_native_texture(
                        &render_state.device,
                        view,
                        eframe::wgpu::FilterMode::Linear,
                    ));
                }
            }
        }

        if let Some(id) = self.texture {
            let uv = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            ui.painter_at(rect)
                .image(id, rect, uv, egui::Color32::WHITE);
        }
        // 关系线的悬停/选中强调贴在合成纹理上、其它叠加层之下：它是「这条线」的属性，
        // 不该压住锚点、角标、工具条。
        // 强调线跟着**插值帧**走（不是 `scene_for_frame`：那是演示副本，下标与
        // `self.hovered` / `self.relationship` 不同源）。
        self.paint_relationship_emphasis(
            ui,
            rect,
            viewport,
            animated.as_ref().unwrap_or(&self.scene),
        );

        // 分组框的名字（CM-18）：框本身在合成纹理里，字必须画在纹理之上。
        self.paint_group_labels(ui, rect, viewport);

        // 折叠点/概要/边界的悬停反馈（打磨轮 37）：叠在合成纹理上，压在手柄与
        // 强调线之下——它只是「这儿能点」的提示，不该盖住操作手柄。
        if !demo_active {
            self.paint_hover_detail(ui, viewport);
        }

        // 这四个覆盖层**必须画在合成纹理之上**：`ui.painter_at(rect).image(...)` 是一张不透明
        // 位图，画在它之前的一切都会被整张盖掉——它们曾经就画在上面那段里，于是手柄、
        // 拖连线的预览线、落空占位提示在真实界面里同样看不见（第 50 轮用一张红色诊断线实测：
        // 贴图前的红线零像素、`sha` 与默认逐位相同）。
        if self.editor.is_none() && !self.input_blocked && !demo_active {
            self.paint_span_handles(ui, viewport);
            self.paint_span_preview(ui, viewport);
            self.paint_relationship_handles(ui, viewport);
            self.paint_relink_preview(ui, viewport);
        }
        for ring in ann_rings {
            ui.painter_at(rect).rect_stroke(
                ring,
                6.0,
                egui::Stroke::new(2.0, color32(self.session.theme.ui.accent)),
                egui::StrokeKind::Middle,
            );
        }

        if !demo_active {
            if self.show_grid || self.show_rulers {
                self.paint_grid_rulers(ui, rect, viewport);
            }
            // X29 G-5：空白新文档的一行引导语。只对「新建且一次编辑都没有」的文档出现。
            self.paint_empty_hint(ui, rect);
            // 聚焦问题在卡片角标之前画：它是「这张图在回答什么」，压在所有装饰下面。
            self.paint_focus_question(ui, rect, viewport);
            self.paint_card_badges(ui, rect, viewport);
            self.paint_pinned_badges(ui, rect, viewport);
            // 悬停可能在这一帧换了节点（帧首的 `bar` 还是旧的）：只画仍然属于
            // 当前悬停节点的那一条，避免工具条在旧节点旁多留一帧。
            if let Some(b) = bar
                .filter(|b| self.hovered == Some(b.node) && self.editor.is_none() && !self.reading)
            {
                paint_hover_bar(
                    ui,
                    &self.session.theme,
                    b,
                    plus_response.as_ref().is_some_and(|r| r.hovered()),
                    more_response.as_ref().is_some_and(|r| r.hovered()),
                );
            }
            self.paint_concept_tip(ui, rect, viewport);
            self.paint_glow(ui, rect, viewport);
            self.paint_overlap_flash(ui, rect, viewport);
        }

        // 落点指示与橡皮筋都画在合成纹理**之上**（docs/06 §5「虚线框 + 半透明填充」）：
        // 它们是纯交互反馈，不该污染 `Frame`（否则离屏导出会带上这些临时的东西）。
        // 悬停节点上露出四边锚点（拖出连线的入口）：不露出来没人知道这儿能拉线。
        if let Some(n) = self
            .hovered
            .filter(|_| self.drag.is_none() && !editing && self.rel_link.is_none())
        {
            paint_anchors(
                ui,
                rect,
                self.camera,
                &self.scene,
                &self.session.theme,
                viewport,
                n,
            );
        }
        // 插入关系线的预览：源环 + 虚线跟手 + 目标环。指针已经压在另一个节点上时
        // 目标环先亮，用户不必「点一下才知道连到了谁」。
        if let Some(src) = self.rel_link.as_ref().and_then(|id| self.scene.find(id)) {
            if let Some(p) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p))
            {
                let target = match self.hit_at(local, viewport) {
                    Some(Target::Node(n)) if n != src => Some(n),
                    _ => None,
                };
                paint_rel_link_preview(
                    ui,
                    rect,
                    self.camera,
                    &self.scene,
                    &self.session.theme,
                    viewport,
                    src,
                    p,
                    target,
                );
            }
        }
        if let Some(l) = self.link.as_ref() {
            paint_link(
                ui,
                rect,
                self.camera,
                &self.scene,
                &self.session.theme,
                viewport,
                l,
                self.concept_map(),
            );
        }
        if let Some(drop) = self.moving.as_ref().and_then(|m| m.drop) {
            paint_drop_target(
                ui,
                rect,
                self.camera,
                &self.scene,
                &self.session.theme,
                viewport,
                drop,
            );
        }
        if let Some(drag) = self.drag.filter(|d| d.drawing) {
            paint_marquee(ui, rect, &self.session.theme, drag.origin, drag.current);
        }
        // 场景几何的一次性导出（只在 `MW_TRACE` 下）：节点 id / 文字 / 屏幕矩形。
        // 「把 A 拖到 B 上」「Tab 建出来的编辑框落在哪」这类验收都得先知道节点在屏幕上
        // 占哪一块；靠点击网格反推既慢，又会被概要/边界的命中盖掉（`owner_node` 会把
        // 注解算到自己的主人头上）。
        if crate::trace::is_enabled() {
            let mut dump = String::new();
            for (i, n) in self.scene.nodes.iter().enumerate() {
                let id = self.scene.ids.get(i).map(String::as_str).unwrap_or("?");
                let r = n.geom.rect;
                let a = self.camera.world_to_screen(Point::new(r.x, r.y), viewport);
                let b = self
                    .camera
                    .world_to_screen(Point::new(r.right(), r.bottom()), viewport);
                let text = self
                    .session
                    .doc
                    .node(id)
                    .map(|n| n.text.as_str())
                    .unwrap_or("");
                dump.push_str(&format!(
                    "\nscene_node {id} text={text:?} rect=({:.0},{:.0},{:.0},{:.0})",
                    rect.min.x + a.x,
                    rect.min.y + a.y,
                    b.x - a.x,
                    b.y - a.y
                ));
            }
            // 注解与折叠点的屏幕几何（打磨轮 37）：悬停基线要指针**真的**落在
            // 花括号/边界描边/圆点上，靠点击网格反推既慢又会被 owner 映射骗。
            for s in self.scene.summaries.iter() {
                let a = self
                    .camera
                    .world_to_screen(Point::new(s.topic.x, s.topic.y), viewport);
                let b = self
                    .camera
                    .world_to_screen(Point::new(s.topic.right(), s.topic.bottom()), viewport);
                dump.push_str(&format!(
                    "\nscene_summary {} topic=({:.0},{:.0},{:.0},{:.0})",
                    s.id,
                    rect.min.x + a.x,
                    rect.min.y + a.y,
                    b.x - a.x,
                    b.y - a.y
                ));
                let c = self.camera.world_to_screen(s.brace.start, viewport);
                dump.push_str(&format!(
                    "\nscene_summary_brace {} at=({:.0},{:.0})",
                    s.id,
                    rect.min.x + c.x,
                    rect.min.y + c.y
                ));
            }
            for b in self.scene.boundaries.iter() {
                let a = self
                    .camera
                    .world_to_screen(Point::new(b.rect.x, b.rect.y), viewport);
                let z = self
                    .camera
                    .world_to_screen(Point::new(b.rect.right(), b.rect.bottom()), viewport);
                dump.push_str(&format!(
                    "\nscene_boundary {} rect=({:.0},{:.0},{:.0},{:.0})",
                    b.id,
                    rect.min.x + a.x,
                    rect.min.y + a.y,
                    z.x - a.x,
                    z.y - a.y
                ));
            }
            // 关系线的屏幕几何（打磨轮 39）：平行命题要脚本点**指定的那条车道**。
            // 没有 id 与坐标就只能靠点击网格反推，第二条点中没点中说不清。
            for (i, r) in self.scene.relationships.iter().enumerate() {
                let a = self.camera.world_to_screen(r.path.start, viewport);
                let b = self.camera.world_to_screen(r.path.end(), viewport);
                let path_mid = match r.path.segs.as_slice() {
                    [PathSeg::Line { to }] => {
                        Point::new((r.path.start.x + to.x) * 0.5, (r.path.start.y + to.y) * 0.5)
                    }
                    [PathSeg::Quad { ctrl, to }] => Point::new(
                        0.25 * r.path.start.x + 0.5 * ctrl.x + 0.25 * to.x,
                        0.25 * r.path.start.y + 0.5 * ctrl.y + 0.25 * to.y,
                    ),
                    _ => r.label_at,
                };
                let m = self.camera.world_to_screen(path_mid, viewport);
                let l = self.camera.world_to_screen(r.label_at, viewport);
                dump.push_str(&format!(
                    "\nscene_rel idx={i} id={} from={} to={} start=({:.0},{:.0}) end=({:.0},{:.0}) mid=({:.0},{:.0}) label=({:.0},{:.0}) w={:.0}",
                    r.id,
                    self.scene
                        .ids
                        .get(r.from as usize)
                        .map(String::as_str)
                        .unwrap_or("?"),
                    self.scene
                        .ids
                        .get(r.to as usize)
                        .map(String::as_str)
                        .unwrap_or("?"),
                    rect.min.x + a.x,
                    rect.min.y + a.y,
                    rect.min.x + b.x,
                    rect.min.y + b.y,
                    rect.min.x + m.x,
                    rect.min.y + m.y,
                    rect.min.x + l.x,
                    rect.min.y + l.y,
                    r.label_w
                ));
            }
            for (i, n) in self.scene.nodes.iter().enumerate() {
                if !n.has_children {
                    continue;
                }
                // 与 `HitMap` 同一条判据：**没有父级的节点（根 / 聚焦根）不画圆点也不吃命中**。
                // 少了这个 else，轨迹会列出一颗屏幕上不存在、点也点不到的圆点。
                let Some(parent_x) = n
                    .parent
                    .and_then(|p| self.scene.nodes.get(p as usize))
                    .map(|p| p.geom.rect.x)
                else {
                    continue;
                };
                let local =
                    mw_core::layout::toggle_pos(n.geom.rect, self.scene.kind, Some(parent_x));
                let a = self.camera.world_to_screen(
                    Point::new(n.geom.rect.x + local.x, n.geom.rect.y + local.y),
                    viewport,
                );
                dump.push_str(&format!(
                    "\nscene_toggle {} center=({:.0},{:.0})",
                    self.scene.ids.get(i).map(String::as_str).unwrap_or("?"),
                    rect.min.x + a.x,
                    rect.min.y + a.y
                ));
            }
            let id = egui::Id::unique("mw_scene_dump");
            if ui.ctx().data(|d| d.get_temp::<String>(id)).as_deref() != Some(dump.as_str()) {
                for line in dump.lines().filter(|l| !l.is_empty()) {
                    crate::trace::event(line);
                }
                ui.ctx().data_mut(|d| d.insert_temp(id, dump.clone()));
            }
        }
        if let Some(points) = self.lasso.as_ref().filter(|p| p.len() > 1) {
            let p = ui.painter_at(rect);
            let mut line = points.clone();
            line.push(points[0]);
            p.add(egui::Shape::line(
                line,
                Stroke::new(1.2, color32(self.session.theme.ui.accent)),
            ));
        }

        // 概要文字编辑器与节点编辑器同一层：都在纹理之上，且互斥。
        if self.summary_edit.is_some() {
            dirty |= self.summary_editor_ui(ui, viewport);
        }
        // 聚焦问题带的编辑器与关系线标签编辑器是两件事：前者只依赖 focus_edit，
        // 曾经被误挂在 rel_edit 的分支里，导致双击问题带永远画不出编辑框（2026-10-27 修）。
        if self.focus_edit.is_some() {
            dirty |= self.focus_editor_ui(ui);
        }
        if self.rel_edit.is_some() {
            dirty |= self.rel_editor_ui(ui);
        }

        // 编辑器最后画：坐标来自同一相机，压在画布纹理之上。
        let mut commit_after = commit_enter;
        if let Some(ed) = &mut self.editor {
            // 新节点刚加进来时编辑器立刻打开，但节点还在从父盒滑出：
            // 让编辑器跟着插值位置走，否则文字会先落在终点、盒子后到。
            let editor_pos = animated
                .as_ref()
                .and_then(|s| s.find(&ed.id).map(|i| s.rect(i)));
            let action = editor_ui(
                ui,
                rect.min,
                self.camera,
                &self.scene,
                &self.session.theme,
                ed,
                EditorPlacement {
                    viewport,
                    rect: editor_pos,
                },
            );
            commit_after |= action.lost_focus;
            // 下一帧的 `handle_pointer` 要知道按下点是不是落在这块盒子上。
            self.edit_rect = action.rect;
            // 编辑框压在哪个节点盒上、有没有出视口（只在 `MW_TRACE` 下）：
            // `Tab` / `Enter` 新建节点会立刻进编辑，而「框跟不跟得上节点、字会不会
            // 顶到屏幕边上」这两件事，靠截图只能量个大概，靠轨迹能直接判。
            if crate::trace::is_enabled() {
                let node = self
                    .scene
                    .find(&ed.id)
                    .map(|i| self.scene.node(i).geom.rect);
                let label = match (self.edit_rect, node) {
                    (Some(e), Some(r)) => {
                        let a = self.camera.world_to_screen(Point::new(r.x, r.y), viewport);
                        let b = self
                            .camera
                            .world_to_screen(Point::new(r.right(), r.bottom()), viewport);
                        let in_view = e.min.x >= 0.0
                            && e.min.y >= 0.0
                            && e.max.x <= viewport[0]
                            && e.max.y <= viewport[1];
                        format!(
                            "editor_open id={} z={:.3} node=({:.0},{:.0},{:.0},{:.0}) \
                             edit=({:.0},{:.0},{:.0},{:.0}) in_view={}",
                            ed.id,
                            self.camera.zoom,
                            a.x + rect.min.x,
                            a.y + rect.min.y,
                            b.x - a.x,
                            b.y - a.y,
                            e.min.x,
                            e.min.y,
                            e.width(),
                            e.height(),
                            u8::from(in_view)
                        )
                    }
                    (Some(e), None) => format!(
                        "editor_open id={} node=none edit=({:.0},{:.0},{:.0},{:.0})",
                        ed.id,
                        e.min.x,
                        e.min.y,
                        e.width(),
                        e.height()
                    ),
                    (None, _) => format!("editor_open id={} edit=none", ed.id),
                };
                let tid = egui::Id::unique("mw_editor_trace");
                if ui.ctx().data(|d| d.get_temp::<String>(tid)).as_deref() != Some(label.as_str()) {
                    ui.ctx().data_mut(|d| d.insert_temp(tid, label.clone()));
                    crate::trace::event(&label);
                }
            }
            if action.changed {
                dirty = true;
                ui.ctx().request_repaint();
            }
        } else {
            self.edit_rect = None;
            // 退出编辑也写一行，否则轨迹停在最后一条 `editor_open` 上，
            // 分不清「还开着」和「早关了」。
            if crate::trace::is_enabled() {
                let tid = egui::Id::unique("mw_editor_trace");
                let open = ui
                    .ctx()
                    .data(|d| d.get_temp::<String>(tid))
                    .is_some_and(|s| !s.starts_with("editor_close"));
                if open {
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(tid, "editor_close".to_owned()));
                    crate::trace::event("editor_close");
                }
            }
        }
        if commit_after && self.editor.is_some() {
            self.commit_edit();
            dirty = true;
        }
        // 建节点必须排在提交**之后**：提交才会把这一帧敲进去的字落到文档，
        // 而 `add_relative` 建完立刻在新节点上开编辑、把选中挪过去。
        if let Some(kind) = edit_follow_up {
            if self.editor.is_none() {
                dirty |= self.add_relative(kind, viewport);
            }
        }
        // 删除确认框画在最外层：它是画布自有的模态，必须盖住编辑器与浮层的指针。
        if self.confirm_delete.is_some() && self.confirm_delete_ui(ui) {
            dirty = true;
        }
        if dirty {
            ui.ctx().request_repaint();
        }
    }

    /// 指针语义：按下定手势 → 拖动（平移 / 拉框）→ 松手提交。
    ///
    /// 手势在**按下瞬间**定死（docs/06 §5），中途不改——否则「拉了一半突然变平移」
    /// 是最典型的廉价手感。中键与空格按住都是平移；空白处左键是框选；节点上左键
    /// 暂按 M2b-2 的口径仍是平移（M2b-3 换成幽灵拖拽）。
    /// 指针语义：按下定手势 → 拖动（平移 / 拉框）→ 松手提交。
    ///
    /// 手势在**按下瞬间**定死（docs/06 §5），中途不改——否则「拉了一半突然变平移」
    /// 是最典型的廉价手感。中键与空格按住都是平移；空白处左键是框选；节点上左键
    /// 暂按 M2b-2 的口径仍是平移（M2b-3 换成幽灵拖拽）。
    pub(crate) fn handle_pointer(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        local: Option<Point>,
        viewport: [f32; 2],
    ) -> bool {
        let mut changed = false;
        let editing = self.editor.is_some();

        // 浮层开着时指针也归浮层。只挡键盘不够：菜单/命令面板盖住画布后，
        // 底下按一下仍会起框选或拖拽，只是看不见——松开时选择已经被改掉了。
        // 画布自己的删除确认框同理：它盖住画布，底下不该还能起手势。
        if self.input_blocked || self.confirm_delete.is_some() {
            self.drag = None;
            self.moving = None;
            self.link = None;
            self.span_drag = None;
            self.relink_drag = None;
            self.rel_edit = None;
            return false;
        }

        if self.summary_edit.is_some() || self.rel_edit.is_some() {
            return false;
        }

        // Esc 取消进行中的手势（拖拽的重排、框选、平移）。必须在这里把键吃掉：
        // 否则同一次按键会继续走到 shortcuts，把选择也一并清掉——用户按 Esc 是想
        // 「这一下不算」，不是想「什么都不选」。
        // X28 D5：只拖注解/关系线手柄时 `self.drag` 是 `None`，但 `span_drag` /
        // `relink_drag` 非空；Esc 也必须能把这两种手势取消掉。
        let handle_drag = self.span_drag.is_some() || self.relink_drag.is_some();
        if (self.drag.is_some() || handle_drag)
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.drag = None;
            self.lasso = None;
            self.moving = None;
            self.link = None;
            self.span_drag = None;
            self.relink_drag = None;
            self.rel_edit = None;
            return true;
        }

        // ── 1. 按下：定手势。press_origin 只在「本帧真的按下」时有意义。
        let (primary, middle, origin, space) = ui.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_pressed(egui::PointerButton::Middle),
                i.pointer.press_origin(),
                i.key_down(egui::Key::Space),
            )
        });
        if primary || middle {
            let edit = self.edit_rect;
            if let Some(origin) = origin.filter(|p| {
                rect.contains(*p)
                    && !self.pointer_blocked_at(*p)
                    && !editor_owns_press(edit, primary, middle, space, *p)
            }) {
                let press_local = Point::new(origin.x - rect.min.x, origin.y - rect.min.y);
                // 用按下点自己的命中结果，而不是本帧悬停的——两者在快速连点时可能差一帧。
                let hot = self.hit_at(Some(press_local), viewport);
                // 锚点是屏幕空间的小圆点：命中半径按 px 给、再换成世界单位。
                // `docs/33` §2 B1b：锚点长在盒边上的小圆点，命中圆有一半落在盒外，
                // 以前要求「先按在盒里」再找锚点，那一半就白白丢了——用户明明按在
                // 锚点上，却被判成拖画布，于是「概念之间连不上线」。现在扫全图取最近。
                let anchor = if editing || self.rel_link.is_some() {
                    None
                } else {
                    let world = self.camera.screen_to_world(press_local, viewport);
                    mw_ui::link::nearest_anchor(
                        &self.scene,
                        world,
                        self.camera.css_to_world(ANCHOR_HIT),
                    )
                    .map(|(n, side)| (n, side, world))
                };
                let mut kind = if primary && !middle && !space && !editing {
                    if let Some(rel) = self.relink_handle_at(Some(press_local), viewport) {
                        let at = self.camera.screen_to_world(press_local, viewport);
                        if let Some(rd) = self.start_relink(rel, at) {
                            self.relink_drag = Some(rd);
                            self.swallow_click = true;
                            DragKind::Relink
                        } else {
                            classify(hot.is_none(), anchor.is_some(), middle, space, editing)
                        }
                    } else if let Some((ann, end)) =
                        self.span_handle_at(Some(press_local), viewport)
                    {
                        if let Some(sd) = self.span_drag_start(ann, end) {
                            self.span_drag = Some(sd);
                            // 按下手柄不是「点了注解/空白」：松手那帧的 click 必须吞掉，
                            // 否则一次没挪动的点击会把刚选中的注解取消掉。
                            self.swallow_click = true;
                            DragKind::Span
                        } else {
                            classify(hot.is_none(), anchor.is_some(), middle, space, editing)
                        }
                    } else {
                        classify(hot.is_none(), anchor.is_some(), middle, space, editing)
                    }
                } else {
                    classify(hot.is_none(), anchor.is_some(), middle, space, editing)
                };
                // 插入关系线时按下只用来「点目标」：不许搬节点（手抖 4px 就把目标挪走，
                // 松手也不再是点击）。拖动仍可平移画布。
                if self.rel_link.is_some() && kind == DragKind::Move {
                    kind = DragKind::Pan;
                }
                if primary
                    && !middle
                    && !space
                    && !editing
                    && hot.is_none()
                    && ui.input(|i| i.modifiers.alt)
                {
                    kind = DragKind::Lasso;
                }
                if let Some((from, side, at)) = anchor {
                    self.link = Some(Link {
                        from,
                        side,
                        at,
                        to: None,
                    });
                    self.select_node_only(from, "link-drag");
                    changed = true;
                }
                // 拖一个「不在选中组里」的节点：立刻把它变成唯一选中，幽灵才是它。
                // 单击路径的选中发生在松手那一帧，而拖拽会把那次 click 吃掉。
                //
                // Ctrl/Shift 按下例外：按下就收窄的话，松手那帧的 click 会把刚点中的
                // 节点当成「已在组内」再 toggle 掉，整组反而清空。加选只在松手时生效；
                // 若真的拖起来，下面的越阈值分支再把幽灵收成这个节点。
                let additive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                if kind == DragKind::Move && !additive {
                    if let Some(node) = hot.and_then(|t| owner_node(&self.scene, t)) {
                        if !self.sel.contains(node) {
                            self.select_node_only(node, "drag-node");
                            changed = true;
                        }
                    }
                }
                self.drag = Some(Drag {
                    kind,
                    origin,
                    current: origin,
                    applied: origin,
                    drawing: false,
                    copy: ui.input(|i| i.modifiers.command),
                    additive: ui.input(|i| i.modifiers.command || i.modifiers.shift),
                    subtract: ui.input(|i| i.modifiers.alt),
                });
                if kind == DragKind::Lasso {
                    self.lasso = Some(vec![origin]);
                }
                if !editing {
                    response.request_focus();
                    self.refocus_canvas = false;
                }
            }
        }

        // ── 2. 拖动。平移用「按下点 → 当前点」的增量自己结算：egui 的 drag_delta
        //    要等越过 max_click_dist 才开始，那会让画布永远落后指针 6px。
        if let Some(mut drag) = self.drag {
            if let Some(cur) = ui.input(|i| i.pointer.latest_pos()) {
                drag.current = cur;
                if drag.kind != DragKind::Pan {
                    let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.05);
                    if self.auto_scroll(cur, rect, dt) {
                        changed = true;
                        ui.ctx().request_repaint();
                    }
                }
                match drag.kind {
                    DragKind::Pan => {
                        if (cur - drag.origin).length() >= PAN_DEADZONE {
                            let step = cur - drag.applied;
                            if step != Vec2::ZERO {
                                self.camera.center.x -= step.x / self.camera.zoom;
                                self.camera.center.y -= step.y / self.camera.zoom;
                                drag.applied = cur;
                                changed = true;
                            }
                        }
                    }
                    DragKind::Marquee => {
                        if !drag.drawing && (cur - drag.origin).length() > DRAG_THRESHOLD {
                            drag.drawing = true;
                        }
                        // 橡皮筋要跟手，拉到哪儿就得重画到哪儿。
                        changed |= drag.drawing;
                    }
                    DragKind::Lasso => {
                        if let Some(points) = self.lasso.as_mut() {
                            let far = points.last().is_none_or(|p| (cur - *p).length() > 2.0);
                            if far {
                                points.push(cur);
                                changed = true;
                            }
                        }
                    }
                    DragKind::Link => {
                        let at = self.camera.screen_to_world(
                            Point::new(cur.x - rect.min.x, cur.y - rect.min.y),
                            viewport,
                        );
                        if let Some(l) = self.link.as_mut() {
                            l.at = at;
                            l.to = mw_ui::link::target_node_snapped(
                                &self.scene,
                                l.from,
                                at,
                                self.camera.css_to_world(LINK_SNAP_CSS),
                            );
                            changed = true;
                        }
                    }
                    DragKind::Span => {
                        let at = self.camera.screen_to_world(
                            Point::new(cur.x - rect.min.x, cur.y - rect.min.y),
                            viewport,
                        );
                        if let Some(sd) = self.span_drag.as_mut() {
                            if let Some(pos) = span_candidate(&sd.centers, at.y) {
                                let (s, e) = span_range(sd.start, sd.end_idx, sd.end, pos);
                                if s != sd.start || e != sd.end_idx {
                                    sd.start = s;
                                    sd.end_idx = e;
                                    sd.preview = sd.children[s..=e].to_vec();
                                    changed = true;
                                }
                            }
                        }
                    }
                    DragKind::Relink => {
                        let at = self.camera.screen_to_world(
                            Point::new(cur.x - rect.min.x, cur.y - rect.min.y),
                            viewport,
                        );
                        if let Some(rd) = self.relink_drag.as_mut() {
                            rd.at = at;
                            let from = self.scene.find(&rd.sel.from);
                            rd.target = from.and_then(|f| {
                                mw_ui::link::target_node_snapped(
                                    &self.scene,
                                    f,
                                    at,
                                    self.camera.css_to_world(LINK_SNAP_CSS),
                                )
                            });
                            changed = true;
                        }
                    }
                    DragKind::Move => {
                        // 树快照在**第一次越过阈值**时拍：这之前它还可能变成一次单击。
                        if self.moving.is_none() && (cur - drag.origin).length() > DRAG_THRESHOLD {
                            // 加选按下时没在 press 阶段改选择；真拖起来时幽灵必须以
                            // 光标下这个节点为准（不在组里就收成单选），否则拖的是别人。
                            let press =
                                Point::new(drag.origin.x - rect.min.x, drag.origin.y - rect.min.y);
                            if let Some(node) = self
                                .hit_at(Some(press), viewport)
                                .and_then(|t| owner_node(&self.scene, t))
                            {
                                if !self.sel.contains(node) {
                                    self.select_node_only(node, "drag-threshold");
                                }
                            }
                            let mut moving = Moving::start(&self.scene, self.sel.ids());
                            moving.copy = drag.copy;
                            self.moving = Some(moving);
                        }
                        let now = ui.input(|i| i.time);
                        let zoom = self.camera.zoom.max(1e-4);
                        // 概念图标志要在借 `self.moving` 之前取（借用检查器不许混用）。
                        let concept = self.concept_map();
                        if let Some(m) = self.moving.as_mut() {
                            let raw = cur - drag.origin;
                            // 吸附：拿「幽灵并集盒 + 原始偏移」去贴别人。参照里必须剔掉
                            // 自己这一坨，否则它永远以自己为参照、一动就吸上。
                            let box_ = shift_rect(m.union, raw.x / zoom, raw.y / zoom);
                            let refs: Vec<mw_core::Rect> = self
                                .scene
                                .draw_order
                                .iter()
                                .filter(|i| !m.banned.contains(i))
                                .map(|i| self.scene.rect(*i))
                                .collect();
                            let mut snap =
                                mw_ui::snap_offsets(box_, &refs, mw_ui::snap::SNAP_PX / zoom);
                            if self.show_grid {
                                let grid = mw_ui::snap::snap_to_grid(
                                    box_,
                                    self.grid_step,
                                    mw_ui::snap::SNAP_PX / zoom,
                                );
                                if snap.guide_x.is_none() {
                                    snap.dx = grid.dx;
                                }
                                if snap.guide_y.is_none() {
                                    snap.dy = grid.dy;
                                }
                            }
                            // 60ms 延迟：候选要稳定存在一小会儿才真吸，否则手一抖就粘住。
                            if snap.is_none() {
                                m.snap_since = None;
                            } else if m.snap_since.is_none() {
                                m.snap_since = Some(now);
                            }
                            if !snap_ready(m.snap_since, now) {
                                snap = mw_ui::Snap::default();
                            }
                            // 偏移里带上吸附量：幽灵、参考线、落点三者才对得上。
                            m.snap = snap;
                            m.offset = Vec2::new(
                                (raw.x / zoom + snap.dx) * zoom,
                                (raw.y / zoom + snap.dy) * zoom,
                            );
                            let at = self.camera.screen_to_world(
                                Point::new(cur.x - rect.min.x, cur.y - rect.min.y),
                                viewport,
                            );
                            // 概念图没有「挂到谁下面」这回事：拖到别的概念上也只是换个位置，
                            // 所以干脆不产生落点（CM-04）。树布局照旧算落点。
                            m.drop = if concept {
                                None
                            } else {
                                mw_ui::drag::drop_target(&self.scene, at, &m.banned)
                            };
                            // 落点三形态（上缘条 / 整框 / 下缘条）在截图里只差十几个像素，
                            // 「现在到底是哪一态」读轨迹比读像素可靠；同态不重复写。
                            if crate::trace::is_enabled() {
                                let label = match m.drop {
                                    None => "drop_zone none".to_owned(),
                                    Some(dt) => format!(
                                        "drop_zone node={} zone={} illegal={} at=({:.0},{:.0})",
                                        self.scene
                                            .ids
                                            .get(dt.node as usize)
                                            .cloned()
                                            .unwrap_or_else(|| "?".into()),
                                        match dt.zone {
                                            mw_ui::Zone::Before => "before",
                                            mw_ui::Zone::Child => "child",
                                            mw_ui::Zone::After => "after",
                                        },
                                        dt.illegal,
                                        at.x,
                                        at.y
                                    ),
                                };
                                let id = egui::Id::unique("mw_drop_zone_trace");
                                if ui.ctx().data(|d| d.get_temp::<String>(id)).as_deref()
                                    != Some(label.as_str())
                                {
                                    ui.ctx().data_mut(|d| d.insert_temp(id, label.clone()));
                                    crate::trace::event(&label);
                                }
                            }
                            m.inside = rect.contains(cur);
                            // 幽灵每帧都得跟手，脏标记直接给上。
                            changed = true;
                        }
                        // 自动滚动：指针贴近视口边缘就每帧推一点。
                        // 长文档里拖到边界外时不必「松手 → 滚轮 → 再拖」。
                        {
                            let pan = autoscroll_step(cur, rect, 26.0, 14.0);
                            if pan != Vec2::ZERO {
                                let zoom = self.camera.zoom.max(1e-4);
                                self.camera.center.x += pan.x / zoom;
                                self.camera.center.y += pan.y / zoom;
                                crate::trace::event(&format!(
                                    "autoscroll dx={:.1} dy={:.1}",
                                    pan.x / zoom,
                                    pan.y / zoom
                                ));
                                changed = true;
                            }
                        }
                    }
                }
            }
            self.drag = Some(drag);
        }

        // ── 3. 松手：框选与重排都在这里提交。
        //
        //    不靠 `drag_stopped()`：位移落在 4–6px 时 egui 根本不认为发生过拖拽，
        //    那个回调永远不会来，橡皮筋就会一直挂在屏幕上。也不只盯 `any_released`：
        //    窗口失焦或别处按了键时收不到 release，用一个「指针已经不在按下状态」兜底，
        //    否则幽灵会永远停在画布上。
        let released = ui.input(|i| i.pointer.any_released());
        let pointer_up = !ui.input(|i| i.pointer.any_down());
        if released || (pointer_up && self.drag.is_some()) {
            if let Some(drag) = self.drag.take() {
                if drag.kind == DragKind::Marquee && drag.drawing {
                    let picked = marquee_pick(&self.camera, &self.scene, rect, &drag, viewport);
                    let picked_n = picked.len();
                    let ids = apply_marquee(self.sel.ids(), picked, drag.additive, drag.subtract);
                    self.clear_foreign_selection(SelectionLane::Node, "marquee");
                    self.sel.replace(ids);
                    // 框选结果要能数出来：多选之后按 Alt+↓ 是整组动还是只动一颗，全看这里选中了几颗。
                    if crate::trace::is_enabled() {
                        crate::trace::event(&format!(
                            "marquee picked={picked_n} selected={}",
                            self.sel.ids().len()
                        ));
                    }
                    // 刚框出来的组不能被同一帧的「单击空白」清掉。
                    self.swallow_click = true;
                    // 橡皮筋必须立刻从屏幕上消失，这一帧就要重画。
                    changed = true;
                }
                if drag.kind == DragKind::Lasso {
                    if let Some(points) = self.lasso.take() {
                        let picked = lasso_pick(&self.camera, &self.scene, rect, &points, viewport);
                        let picked_n = picked.len();
                        self.clear_foreign_selection(SelectionLane::Node, "lasso");
                        // `None` = Alt 套索圈空，选择原样保留（见 `apply_lasso`）。
                        if let Some(ids) =
                            apply_lasso(self.sel.ids(), picked, drag.additive, drag.subtract)
                        {
                            self.sel.replace(ids);
                        }
                        // 套索与矩形共用 `marquee` 前缀，靠 `path=lasso` 才分得清走了哪条
                        // 手势；矩形那条保持原样，既有证据口径不动。
                        if crate::trace::is_enabled() {
                            crate::trace::event(&format!(
                                "marquee picked={picked_n} selected={} path=lasso",
                                self.sel.ids().len()
                            ));
                        }
                        self.swallow_click = true;
                        changed = true;
                    }
                }
                if drag.kind == DragKind::Link {
                    if let Some(l) = self.link.take() {
                        changed |= self.commit_link(&l);
                        self.swallow_click = true;
                    }
                }
                if drag.kind == DragKind::Move {
                    if let Some(m) = self.moving.take() {
                        changed |= self.commit_move(&m, m.inside);
                        // 同一条理由：位移落在 4–6px 时 egui 仍会报 click，
                        // 那次 click 会把刚拖好的多选组换成单选。
                        self.swallow_click = true;
                    }
                }
                if drag.kind == DragKind::Span {
                    if let Some(sd) = self.span_drag.take() {
                        changed |= self.commit_span_drag(sd);
                        self.swallow_click = true;
                    }
                }
                if drag.kind == DragKind::Relink {
                    if let Some(rd) = self.relink_drag.take() {
                        changed |= self.commit_relink(rd);
                        self.swallow_click = true;
                    }
                }
            }
        }

        // ── 4. 缩放与滚轮平移（与拖拽互不影响）。浮层上的滚轮留给浮层。
        if !response.hovered() || self.pointer_over_overlay(ui) {
            return changed;
        }
        let (zoom_delta, scroll, pinch) = ui.input(|i| {
            (
                i.zoom_delta(),
                i.smooth_scroll_delta,
                // `zoom_delta` 把两条硬件通道合成一个数：真·双指捏合来自多点触控状态，
                // 其余（Ctrl+滚轮 / 门控合成）走 `zoom_factor_delta`。轨迹要分得开。
                i.multi_touch().is_some(),
            )
        });
        if zoom_delta != 1.0 {
            // 编辑期锁缩放（docs/06:108）已收进 `zoom_step_at`：五条入口一条合同，
            // 被锁住时它返回 false 并记 `zoom_blocked src=… editing=1`，这里不必再判一次。
            if let Some(p) = local {
                let src = if pinch {
                    ZoomSrc::Pinch
                } else {
                    ZoomSrc::Wheel
                };
                changed |= self.zoom_step_at(p, zoom_delta, src);
            }
        } else if scroll != Vec2::ZERO {
            // `Shift+滚轮` 换轴（docs/06:149）。不按 Shift 时 `wheel_pan` 原样返回，
            // 纵向滚动仍走纵向平移——与改动前逐像素一致，别顺手把 y 去掉。
            let d = wheel_pan(scroll, ui.input(|i| i.modifiers.shift));
            self.camera.center.x -= d.x / self.camera.zoom;
            self.camera.center.y -= d.y / self.camera.zoom;
            changed = true;
        }

        changed
    }

    pub(crate) fn fit(&mut self, viewport: [f32; 2]) {
        let b = self.scene.bounds;
        self.fit_bounds(b, viewport);
    }

    pub(crate) fn fit_bounds(&mut self, b: Rect, viewport: [f32; 2]) {
        let w = (viewport[0] - FIT_PAD * 2.0).max(80.0);
        let h = (viewport[1] - FIT_PAD * 2.0).max(80.0);
        let zoom = (w / b.w.max(1.0)).min(h / b.h.max(1.0));
        self.camera.zoom = zoom.clamp(FIT_MIN_ZOOM, FIT_MAX_ZOOM);
        self.camera.center = b.center();
        // 记下「这份取景是按哪个视口算的」：启动阶段壳层拿它判断窗口有没有落定。
        self.fit_viewport = Some(viewport);
        crate::trace::event(&format!(
            "fit vp={:.0}x{:.0} zoom={:.4} center=({:.2},{:.2}) bounds=({:.1},{:.1},{:.1},{:.1})",
            viewport[0],
            viewport[1],
            self.camera.zoom,
            self.camera.center.x,
            self.camera.center.y,
            b.x,
            b.y,
            b.w,
            b.h
        ));
    }

    /// 面板开合把画布中心在窗口里挪了 `shift`（逻辑 px）时，反向补偿相机。
    /// 屏幕坐标 = `(世界 - 相机中心) * zoom + 画布中心`，所以相机中心同向加
    /// `shift / zoom` 才能让画面在屏幕上原地不动；缩放一律不动（13 §4.20）。
    /// 面板开合把画布中心在窗口里挪了 `shift`（逻辑 px）时，反向补偿相机。
    /// 屏幕坐标 = `(世界 - 相机中心) * zoom + 画布中心`，所以相机中心同向加
    /// `shift / zoom` 才能让画面在屏幕上原地不动；缩放一律不动（13 §4.20）。
    pub(crate) fn pan_for_viewport_shift(&mut self, shift: Vec2) {
        let (dx, dy) = camera_shift_for_viewport(shift, self.camera.zoom);
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        self.camera.center.x += dx;
        self.camera.center.y += dy;
    }

    /// 注解命中：先看概要到摘要盒，再看边界内部。
    ///
    /// 落在任何节点盒子里都返回 `None`——那种点按属于节点选择，不该被边界抢走。
    /// 注解命中：先看概要到摘要盒，再看边界内部。
    ///
    /// 落在任何节点盒子里都返回 `None`——那种点按属于节点选择，不该被边界抢走。
    pub(crate) fn annotation_at(
        &self,
        local: Option<Point>,
        viewport: [f32; 2],
    ) -> Option<AnnotationSel> {
        let local = local?;
        annotation_hit(&self.scene, &self.camera, local, viewport)
    }

    /// 当前选中的注解（供检查器联动）。最后一个是主选。
    pub fn annotation_selection(&self) -> Option<&AnnotationSel> {
        self.annotations.last()
    }

    /// 全部选中的注解（多选时检查器做批量颜色/删除）。
    pub fn annotation_selections(&self) -> &[AnnotationSel] {
        &self.annotations
    }

    /// 当前选中的关系线（检查器联动）。
    pub fn relationship_selection(&self) -> Option<&RelationshipSel> {
        self.relationship.primary()
    }

    /// 全部选中的关系线（CM-17）；空 = 没有选中任何命题。
    pub fn relationship_selections(&self) -> &[RelationshipSel] {
        self.relationship.as_slice()
    }

    /// 选中的命题条数（菜单文案「删除 N 条」用）。
    pub fn relationship_count(&self) -> usize {
        self.relationship.len()
    }

    /// 从检查器列表里选中一条关系线。
    pub fn select_relationship(&mut self, from: &str, id: &str) -> bool {
        let Some(node) = self.session.doc.node(from) else {
            return false;
        };
        let Some(rel) = node.relationships.iter().find(|r| r.id == id) else {
            return false;
        };
        self.relationship.select_only(RelationshipSel {
            from: from.to_string(),
            id: id.to_string(),
            to: rel.to.clone(),
        });
        self.clear_foreign_selection(SelectionLane::Relationship, "select-relationship");
        self.refocus_canvas = true;
        true
    }

    /// 搜索结果跳转：选中命题、相机居中到两端概念盒中点，但**不抢键盘**——
    /// 搜索框还开着，`Enter` 继续走下一行（与 `select_node_keep_focus` 同一条纪律）。
    pub fn select_relationship_keep_focus(&mut self, from: &str, id: &str) -> bool {
        if !self.select_relationship(from, id) {
            return false;
        }
        let to = self.relationship.primary().map(|r| r.to.clone());
        if let (Some(a), Some(b)) = (
            self.scene.find(from),
            to.as_deref().and_then(|t| self.scene.find(t)),
        ) {
            let ca = self.scene.rect(a).center();
            let cb = self.scene.rect(b).center();
            self.camera.center = Point::new((ca.x + cb.x) * 0.5, (ca.y + cb.y) * 0.5);
            self.fitted = true;
        }
        self.refocus_canvas = false;
        true
    }

    pub fn clear_relationship_selection(&mut self) {
        self.relationship.clear();
    }

    /// 选中的注解条数（右键菜单据此显示「删除 N 条」）。
    pub fn annotation_count(&self) -> usize {
        self.annotations.len()
    }

    /// 选中一条注解；`additive`（Shift/Ctrl）时切换进/出多选组。
    /// 返回是否改动了任何一道选中（含被清掉的节点/关系线道）。
    pub(crate) fn select_annotation(&mut self, ann: AnnotationSel, additive: bool) -> bool {
        let mut changed =
            self.clear_foreign_selection(SelectionLane::Annotation, "select-annotation");
        let before = self.annotations.clone();
        annotation_click(&mut self.annotations, ann, additive);
        changed |= before != self.annotations;
        changed
    }

    /// 取消注解选择（检查器的「取消选择」用）。
    pub fn clear_annotation_selection(&mut self) {
        self.annotations.clear();
    }

    /// 删除当前选中的关系线（Delete 键 / 右键菜单）。多选时一条 `Batch` 全删，一次撤销。
    pub fn delete_selected_relationship(&mut self) -> bool {
        let rels = self.relationship.as_slice().to_vec();
        if rels.is_empty() {
            return false;
        }
        for r in &rels {
            crate::trace::event(&format!("delete relationship {} {}", r.from, r.id));
        }
        let commands: Vec<Command> = rels
            .iter()
            .map(|r| Command::DeleteRelationship {
                from: r.from.clone(),
                id: r.id.clone(),
            })
            .collect();
        let cmd = match commands.len() {
            1 => commands.into_iter().next().unwrap(),
            _ => Command::Batch { commands },
        };
        self.relationship.clear();
        self.apply_command(cmd)
    }

    /// 旧的单条删除（保留给「只删这一条」的调用方）。
    #[allow(dead_code)]
    fn delete_one_relationship(&mut self, rel: RelationshipSel) -> bool {
        crate::trace::event(&format!("delete relationship {} {}", rel.from, rel.id));
        self.apply_command(Command::DeleteRelationship {
            from: rel.from,
            id: rel.id,
        })
    }

    /// 当前选中关系线的箭头方向（右键菜单「箭头方向」打勾用）。
    ///
    /// 从文档里现读而不是缓存：撤销之后缓存会骗人。
    pub fn selected_relationship_head(&self) -> Option<mw_core::model::ArrowHead> {
        let rel = self.relationship.primary()?;
        self.session
            .doc
            .node(&rel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == rel.id))
            .map(|r| r.head)
    }

    /// 改当前选中关系线的箭头方向（右键菜单「箭头方向」/ 检查器四格）。
    pub fn set_selected_relationship_head(&mut self, head: mw_core::model::ArrowHead) -> bool {
        let rels = self.relationship.as_slice().to_vec();
        if rels.is_empty() {
            return false;
        }
        for r in &rels {
            crate::trace::event(&format!(
                "relationship head {} {} {}",
                r.from,
                r.id,
                head.as_str()
            ));
        }
        let commands: Vec<Command> = rels
            .iter()
            .map(|r| Command::SetRelationshipHead {
                from: r.from.clone(),
                id: r.id.clone(),
                head,
            })
            .collect();
        let cmd = match commands.len() {
            1 => commands.into_iter().next().unwrap(),
            _ => Command::Batch { commands },
        };
        self.apply_command(cmd)
    }

    #[allow(dead_code)]
    fn set_one_relationship_head(
        &mut self,
        rel: RelationshipSel,
        head: mw_core::model::ArrowHead,
    ) -> bool {
        crate::trace::event(&format!(
            "relationship head {} {} {}",
            rel.from,
            rel.id,
            head.as_str()
        ));
        self.apply_command(Command::SetRelationshipHead {
            from: rel.from,
            id: rel.id,
            head,
        })
    }

    /// 选中关系线后，终点手柄的命中（屏幕空间 9px）。
    pub(crate) fn relink_handle_at(
        &self,
        local: Option<Point>,
        viewport: [f32; 2],
    ) -> Option<RelationshipSel> {
        let rel = self.relationship.primary().cloned()?;
        let sr = self
            .scene
            .relationships
            .iter()
            .find(|r| r.id == rel.id && self.scene.ids[r.from as usize] == rel.from)?;
        let world = self.camera.screen_to_world(local?, viewport);
        let hit = self.camera.css_to_world(REL_HANDLE_HIT);
        let end = sr.path.end();
        let d = ((world.x - end.x).powi(2) + (world.y - end.y).powi(2)).sqrt();
        (d <= hit).then_some(rel)
    }

    pub(crate) fn start_relink(&mut self, rel: RelationshipSel, at: Point) -> Option<RelinkDrag> {
        if !self
            .scene
            .relationships
            .iter()
            .any(|r| r.id == rel.id && self.scene.ids[r.from as usize] == rel.from)
        {
            return None;
        }
        Some(RelinkDrag {
            sel: rel,
            at,
            target: None,
        })
    }

    /// 松手提交换目标：目标为空白、原目标或起点自身都不发命令。
    /// 视觉验收（`cm-relink-blank` 场景）：把某个短语的线改到**空白处**。
    ///
    /// 走的就是鼠标松手那一步（`commit_relink`，`target = None`），
    /// 所以这张图钉住的是「落空 = 新建概念并改线」，不是指针几何。
    pub fn retarget_phrase_to_blank(&mut self, label: &str, dx: i32, dy: i32) -> bool {
        if !self.concept_map() {
            return false;
        }
        let Some(rel) = self
            .scene
            .relationships
            .iter()
            .find(|r| r.label.as_deref() == Some(label))
        else {
            return false;
        };
        let sel = RelationshipSel {
            from: self.scene.ids[rel.from as usize].clone(),
            id: rel.id.clone(),
            to: self.scene.ids[rel.to as usize].clone(),
        };
        let base = self.scene.rect(rel.to);
        let rd = RelinkDrag {
            sel,
            at: Point::new(base.x + dx as f32, base.y + dy as f32),
            target: None,
        };
        self.commit_relink(rd)
    }

    /// 视觉验收（`cm-relink-drag` 场景，CM-10 ⑦）：把某条关系**选中并停在拖终点途中**，
    /// 指针停在目标端偏移 (dx,dy) 处，**不提交**。
    ///
    /// 与鼠标走的是同一状态（`relationship` 选中 + `relink_drag`），只是不经过命中测试：
    /// 离屏里标定句柄坐标既脆又难复现，而这张图要钉的是「拖终点期间不画 3px 强调、只画预览线」。
    pub fn begin_relink_drag_label(&mut self, label: &str, dx: i32, dy: i32) -> bool {
        if !self.concept_map() {
            return false;
        }
        let Some(idx) = self
            .scene
            .relationships
            .iter()
            .position(|r| r.label.as_deref() == Some(label))
        else {
            return false;
        };
        let (sel, end) = {
            let rel = &self.scene.relationships[idx];
            (
                RelationshipSel {
                    from: self.scene.ids[rel.from as usize].clone(),
                    id: rel.id.clone(),
                    to: self.scene.ids[rel.to as usize].clone(),
                },
                rel.path.end(),
            )
        };
        let at = Point::new(end.x + dx as f32, end.y + dy as f32);
        // 夹具也要走一遍真实的落点判定：否则「吸没吸上」在截图里看不出来
        // （以前这里写死 `target: None`，预览永远画成落空，等于拍了个寂寞）。
        let target = self.scene.find(&sel.from).and_then(|f| {
            mw_ui::link::target_node_snapped(
                &self.scene,
                f,
                at,
                self.camera.css_to_world(LINK_SNAP_CSS),
            )
        });
        let rd = RelinkDrag {
            sel: sel.clone(),
            at,
            target,
        };
        self.relationship.select_only(sel);
        self.clear_foreign_selection(SelectionLane::Relationship, "relink-drag");
        self.relink_drag = Some(rd);
        self.swallow_click = true;
        true
    }
    /// 只选中某条关系（按短语），不进入拖终点。给 `cm-relink-drag` 的**对照实验**用：
    /// 两张图只差「有没有 `relink_drag`」，于是「强调被抑制」这件事才被单独隔离出来。
    pub fn select_relationship_by_label(&mut self, label: &str) -> bool {
        if !self.concept_map() {
            return false;
        }
        let Some(idx) = self
            .scene
            .relationships
            .iter()
            .position(|r| r.label.as_deref() == Some(label))
        else {
            return false;
        };
        let rel = &self.scene.relationships[idx];
        self.relationship.select_only(RelationshipSel {
            from: self.scene.ids[rel.from as usize].clone(),
            id: rel.id.clone(),
            to: self.scene.ids[rel.to as usize].clone(),
        });
        self.clear_foreign_selection(SelectionLane::Relationship, "select-relationship-label");
        true
    }
    /// 给当前文档加一条概要并选中它（`MW_SUMMARY_SELECT` 夹具）。
    ///
    /// 概要/边界的选中环与跨度手柄曾经画在合成纹理贴图之前、在界面上从来没显示过
    /// （第 50 轮修的就是这个次序）。这个夹具的作用是让那条路径在像素上留下证据：
    /// 选中环是圈住跨度的 2px 强调色矩形，跨度手柄在两端。
    ///
    /// 只接受真正有子级的文档：概念图模板是平铺的（根节点不挂子级），在那里建不出概要。
    pub fn add_summary_and_select(&mut self, text: &str) -> bool {
        let Some((parent_id, targets)) = self.first_two_children() else {
            return false;
        };
        let before: Vec<String> = self.scene.summaries.iter().map(|s| s.id.clone()).collect();
        let cmd = Command::AddSummary {
            parent: parent_id.clone(),
            targets,
            text: text.to_string(),
        };
        if !self.apply_structural(Some(cmd)) {
            crate::trace::event("summary_fail apply=false");
            return false;
        }
        let Some(new_id) = self
            .scene
            .summaries
            .iter()
            .map(|s| s.id.clone())
            .find(|id| !before.contains(id))
        else {
            return false;
        };
        crate::trace::event(&format!("summary_added {} {}", parent_id, new_id));
        self.select_annotation(
            AnnotationSel {
                parent: parent_id,
                kind: AnnotationKind::Summary,
                id: new_id,
            },
            false,
        );
        true
    }
    /// 取「根 + 它的前两个子级」——注解（概要/边界）只能建在有子级的父节点上。
    /// 概念图模板是平铺的（根不挂子级），在那上面 `AddSummary`/`AddBoundary` 根本立不住。
    fn first_two_children(&self) -> Option<(String, Vec<String>)> {
        let root = self.scene.nodes.iter().position(|n| n.parent.is_none())?;
        let kids: Vec<u32> = (0..self.scene.nodes.len() as u32)
            .filter(|i| self.scene.nodes[*i as usize].parent == Some(root as u32))
            .collect();
        crate::trace::event(&format!("annotation_try root={} kids={}", root, kids.len()));
        if kids.len() < 2 {
            return None;
        }
        let parent_id = self.scene.ids[root].clone();
        let targets: Vec<String> = kids
            .iter()
            .take(2)
            .map(|i| self.scene.ids[*i as usize].clone())
            .collect();
        Some((parent_id, targets))
    }

    /// 跨度拖拽**松手提交**的完整路径（`MW_SPAN_COMMIT` 夹具）。
    ///
    /// 第 53 轮只拍了拖拽中的预览框；这里补三件事：提交到底发没发命令、是不是只发一条（撤一次就回来）、
    /// 撤销之后文档的目标数回到原值。画面最后停在「跨度真的变宽」的状态，供像素比对。
    pub fn add_span_commit_demo(&mut self, text: &str) -> bool {
        if !self.add_span_drag_demo(text) {
            return false;
        }
        let Some(ann) = self.annotations.first().cloned() else {
            return false;
        };
        let before = self.annotation_targets(&ann);
        let Some(sd) = self.span_drag.take() else {
            return false;
        };
        let preview = sd.preview.len();
        let changed = self.commit_span_drag(sd);
        let after = self.annotation_targets(&ann);
        crate::trace::event(&format!(
            "span_commit before={} after={} changed={} preview={}",
            before, after, changed, preview
        ));
        let undone = self.undo();
        let back = self.annotation_targets(&ann);
        crate::trace::event(&format!("span_commit_undo back={} undone={}", back, undone));
        // 再走一遍「重做」，让画面停在变宽之后的终态：这张要拍的是提交结果，不是撤销结果。
        let redone = self.redo();
        let end = self.annotation_targets(&ann);
        crate::trace::event(&format!(
            "span_commit_redo targets={} redone={}",
            end, redone
        ));
        redone && end == after
    }

    /// 读文档里那条注解当前的目标数（不信缓存：撤销之后缓存会骗人）。
    fn annotation_targets(&self, ann: &AnnotationSel) -> usize {
        let Some(node) = self.session.doc.node(&ann.parent) else {
            return 0;
        };
        match ann.kind {
            AnnotationKind::Summary => node
                .summaries
                .iter()
                .find(|s| s.id == ann.id)
                .map(|s| s.targets.len())
                .unwrap_or(0),
            AnnotationKind::Boundary => node
                .boundaries
                .iter()
                .find(|b| b.id == ann.id)
                .map(|b| b.targets.len())
                .unwrap_or(0),
        }
    }

    /// 加一条概要 + 一条边界，并把**两条都选中**（`MW_ANNOT_MULTI` 夹具）。
    ///
    /// additive 选择只有「两圈环同时在」这一种形态能被看见；单选那一圈第 51/52 轮已经拍过。
    pub fn add_two_annotations_and_select(&mut self, text: &str) -> bool {
        if !self.add_summary_and_select(text) {
            return false;
        }
        let Some(summary) = self.annotations.first().cloned() else {
            return false;
        };
        if !self.add_boundary_and_select() {
            return false;
        }
        // 边界那一步把选择换成了边界；把概要按 additive 补回来 → 两条都在选。
        self.select_annotation(summary, true);
        crate::trace::event(&format!("annot_multi selected={}", self.annotations.len()));
        true
    }
    /// 加两条注解、都选中，再把**边界**按 additive 取消（`MW_ANNOT_MULTI_OFF` 夹具）。
    ///
    /// 取消之后应当与「只选概要」那张基线（`mm-summary-select`）**逐像素一致**：
    /// `annotation_click` 的 toggle 语义只要留下一点残影（多选环没撤、检查器目标没换），两张就会分叉。
    /// 这条比「再拍一张新图」硬：断言用的是**已有基线**，不是自己跟自己比。
    pub fn add_two_annotations_and_deselect(&mut self, text: &str) -> bool {
        if !self.add_two_annotations_and_select(text) {
            return false;
        }
        let Some(boundary) = self
            .annotations
            .iter()
            .find(|a| a.kind == AnnotationKind::Boundary)
            .cloned()
        else {
            return false;
        };
        // 同一个 additive 选择再走一次 = 从多选组里移出（`annotation_click` 是切换语义）。
        self.select_annotation(boundary, true);
        crate::trace::event(&format!(
            "annot_multi_off left={} kinds={:?}",
            self.annotations.len(),
            self.annotations.iter().map(|a| a.kind).collect::<Vec<_>>()
        ));
        self.annotations.len() == 1
    }

    /// 加一条概要、选中它，并把跨度拖拽停在「再吞一个兄弟」的预览态（`MW_SPAN_DRAG` 夹具）。
    ///
    /// 跨度手柄拖拽中会画一排强调色预览框（`paint_span_preview` 读 `sd.preview`），
    /// 这段绘制在第 50 轮之前同样被合成纹理盖掉过；不实拍就没有证据。
    pub fn add_span_drag_demo(&mut self, text: &str) -> bool {
        if !self.add_summary_and_select(text) {
            return false;
        }
        let Some(ann) = self.annotations.first().cloned() else {
            return false;
        };
        let Some(mut sd) = self.span_drag_start(ann, SpanEnd::Bottom) else {
            crate::trace::event("span_drag_fail start=none");
            return false;
        };
        // 把预览扩到「再多含一个兄弟」：这就是拖到第三个子级时该看到的画面。
        if let Some(extra) = sd
            .children
            .iter()
            .find(|id| !sd.preview.contains(id))
            .cloned()
        {
            sd.preview.push(extra);
        }
        crate::trace::event(&format!("span_drag_demo preview={}", sd.preview.len()));
        self.span_drag = Some(sd);
        true
    }
    /// 给当前文档加一条**边界**并选中它（`MW_BOUNDARY_SELECT` 夹具）。
    ///
    /// 与概要共用同一段选中环绘制代码，但边界有自己的几何；不单独拍一张就没有证据。
    pub fn add_boundary_and_select(&mut self) -> bool {
        let Some((parent_id, targets)) = self.first_two_children() else {
            return false;
        };
        let before: Vec<String> = self.scene.boundaries.iter().map(|b| b.id.clone()).collect();
        let cmd = Command::AddBoundary {
            parent: parent_id.clone(),
            targets,
            color: Some("#8fb9a8".to_string()),
        };
        if !self.apply_structural(Some(cmd)) {
            crate::trace::event("boundary_fail apply=false");
            return false;
        }
        let Some(new_id) = self
            .scene
            .boundaries
            .iter()
            .map(|b| b.id.clone())
            .find(|id| !before.contains(id))
        else {
            return false;
        };
        crate::trace::event(&format!("boundary_added {} {}", parent_id, new_id));
        self.select_annotation(
            AnnotationSel {
                parent: parent_id,
                kind: AnnotationKind::Boundary,
                id: new_id,
            },
            false,
        );
        true
    }
    pub(crate) fn commit_relink(&mut self, rd: RelinkDrag) -> bool {
        let Some(target) = rd.target else {
            // 拖到空白：概念图里就地长出一个新概念，把这条线改指到它。
            // 与「从概念拖到空白」（CM-10 ①）同一语义——同一个手势族，落空不该静默取消。
            if !self.concept_map() {
                // X28 D4：思维导图里拖到空白不能静默取消——用户已经拖了一路，
                // 至少要告诉他一件事：端点必须落在另一个主题上。
                crate::trace::event("relink_cancel reason=blank_tree");
                self.notice = Some((
                    "改端点要落在另一个主题上；在概念图里拖到空白会新建概念".to_string(),
                    true,
                ));
                return false;
            }
            let before = self.session.doc.doc_order();
            let cmd = Command::RetargetLinkToNewConcept {
                from: rd.sel.from.clone(),
                id: rd.sel.id.clone(),
                text: super::CONCEPT_PLACEHOLDER.into(),
                x: rd.at.x,
                y: rd.at.y,
            };
            if !self.apply_structural(Some(cmd)) {
                return false;
            }
            let Some(new_id) = self
                .session
                .doc
                .doc_order()
                .into_iter()
                .find(|id| !before.contains(id))
            else {
                return false;
            };
            crate::trace::event(&format!(
                "relink_new_concept {} {} -> {}",
                rd.sel.from, rd.sel.id, new_id
            ));
            let sel = RelationshipSel {
                from: rd.sel.from,
                id: rd.sel.id,
                to: new_id,
            };
            self.relationship.select_only(sel.clone());
            // 与「从概念拉新线」一致：新起的线立刻进短语编辑态。
            // 否则同一个手势族里，一条能直接打短语、另一条要先双击一下，说不通。
            let viewport = self.last_viewport;
            self.start_rel_edit(sel, viewport, true);
            return true;
        };
        let Some(to) = self.scene.ids.get(target as usize).cloned() else {
            return false;
        };
        if to == rd.sel.to || to == rd.sel.from {
            return false;
        }
        crate::trace::event(&format!("relink {} {} -> {}", rd.sel.from, rd.sel.id, to));
        let ok = self.apply_command(Command::SetRelationshipTarget {
            from: rd.sel.from.clone(),
            id: rd.sel.id.clone(),
            to: to.clone(),
        });
        if ok {
            self.relationship.select_only(RelationshipSel {
                from: rd.sel.from,
                id: rd.sel.id,
                to,
            });
        }
        ok
    }

    /// 当前选中注解的两个跨度手柄（世界坐标）。没有选中/注解已消失时为空。
    pub(crate) fn span_handles(&self) -> Option<SpanHandles> {
        let ann = self.annotations.last().cloned()?;
        let (top, bottom) = match ann.kind {
            AnnotationKind::Summary => {
                let s = self.scene.summaries.iter().find(|s| s.id == ann.id)?;
                (s.brace.start, s.brace.end())
            }
            AnnotationKind::Boundary => {
                let b = self.scene.boundaries.iter().find(|b| b.id == ann.id)?;
                let cx = b.rect.center().x;
                (Point::new(cx, b.rect.y), Point::new(cx, b.rect.bottom()))
            }
        };
        Some(SpanHandles { ann, top, bottom })
    }

    /// 指针落在哪个跨度手柄上。只检查当前选中的那一条注解，命中半径 9px。
    pub(crate) fn span_handle_at(
        &self,
        local: Option<Point>,
        viewport: [f32; 2],
    ) -> Option<(AnnotationSel, SpanEnd)> {
        if self.editor.is_some() || self.input_blocked {
            return None;
        }
        let world = self.camera.screen_to_world(local?, viewport);
        let h = self.span_handles()?;
        let hit = self.camera.css_to_world(SPAN_HANDLE_HIT);
        let near = |p: Point| ((world.x - p.x).powi(2) + (world.y - p.y).powi(2)).sqrt() <= hit;
        if near(h.top) {
            Some((h.ann, SpanEnd::Top))
        } else if near(h.bottom) {
            Some((h.ann, SpanEnd::Bottom))
        } else {
            None
        }
    }

    /// 按下跨度手柄：把父级的直接子节点、当前区间、同侧兄弟的 y 中心一次拍好。
    ///
    /// 只考虑同一侧的兄弟（放射式布局左右分开），否则拖右侧概要会把左侧分支也圈进来。
    /// 返回 `None` 时不进入跨度拖拽，交给原来的手势分类。
    /// 按下跨度手柄：把父级的直接子节点、当前区间、同侧兄弟的 y 中心一次拍好。
    ///
    /// 只考虑同一侧的兄弟（放射式布局左右分开），否则拖右侧概要会把左侧分支也圈进来。
    /// 返回 `None` 时不进入跨度拖拽，交给原来的手势分类。
    pub(crate) fn span_drag_start(&self, ann: AnnotationSel, end: SpanEnd) -> Option<SpanDrag> {
        let node = self.session.doc.node(&ann.parent)?;
        let targets = match ann.kind {
            AnnotationKind::Summary => node
                .summaries
                .iter()
                .find(|s| s.id == ann.id)
                .map(|s| s.targets.clone())?,
            AnnotationKind::Boundary => node
                .boundaries
                .iter()
                .find(|b| b.id == ann.id)
                .map(|b| b.targets.clone())?,
        };
        let children = node.children.clone();
        let (start, end_idx) = span_bounds(&targets, &children)?;
        let owner = self.scene.find(&ann.parent)?;
        let owner_cx = self.scene.rect(owner).center().x;

        let mut sum = 0.0f32;
        let mut visible = 0usize;
        for t in &targets {
            if let Some(i) = self.scene.find(t) {
                sum += self.scene.rect(i).center().x;
                visible += 1;
            }
        }
        let right = visible == 0 || sum / visible as f32 >= owner_cx;

        let centers: Vec<(usize, f32)> = children
            .iter()
            .enumerate()
            .filter_map(|(pos, id)| {
                let idx = self.scene.find(id)?;
                let r = self.scene.rect(idx);
                if self.scene.kind == LayoutKind::Mindmap && (r.center().x >= owner_cx) != right {
                    return None;
                }
                Some((pos, r.center().y))
            })
            .collect();
        if centers.is_empty() {
            return None;
        }
        Some(SpanDrag {
            ann,
            end,
            children,
            start,
            end_idx,
            centers,
            origin: targets.clone(),
            preview: targets,
        })
    }

    /// 松手提交：目标没变就什么都不发；变了只发一条命令（一次撤销）。
    pub(crate) fn commit_span_drag(&mut self, sd: SpanDrag) -> bool {
        if sd.preview == sd.origin {
            return false;
        }
        let cmd = match sd.ann.kind {
            AnnotationKind::Summary => Command::SetSummaryTargets {
                parent: sd.ann.parent.clone(),
                id: sd.ann.id.clone(),
                targets: sd.preview.clone(),
            },
            AnnotationKind::Boundary => Command::SetBoundaryTargets {
                parent: sd.ann.parent.clone(),
                id: sd.ann.id.clone(),
                targets: sd.preview.clone(),
            },
        };
        crate::trace::event(&format!(
            "span {} {} {}..{}",
            sd.ann.label(),
            sd.ann.id,
            sd.start,
            sd.end_idx
        ));
        self.apply_command(cmd)
    }

    /// 选中注解后的两个跨度手柄：白底 + 强调色描边，压在花括号/边界缘上。
    pub(crate) fn paint_span_handles(&self, ui: &egui::Ui, viewport: [f32; 2]) {
        let Some(h) = self.span_handles() else {
            return;
        };
        let painter = ui.painter();
        let origin = self.last_rect.min;
        let accent = with_alpha(self.session.theme.ui.accent, 255);
        let fill = with_alpha(self.session.theme.ui.panel, 255);
        let halo = with_alpha(self.session.theme.ui.canvas_bg, 220);
        for p in [h.top, h.bottom] {
            let s = self.camera.world_to_screen(p, viewport);
            let c = Pos2::new(origin.x + s.x, origin.y + s.y);
            painter.circle_filled(c, SPAN_HANDLE_R + 1.5, halo);
            painter.circle_filled(c, SPAN_HANDLE_R, fill);
            painter.circle_stroke(c, SPAN_HANDLE_R, Stroke::new(1.5, accent));
        }
    }

    /// 选中关系线后的终点手柄（start 只画一个小点，不可拖）。
    pub(crate) fn paint_relationship_handles(&self, ui: &egui::Ui, viewport: [f32; 2]) {
        let Some(rel) = self.relationship.primary() else {
            return;
        };
        let Some(sr) = self
            .scene
            .relationships
            .iter()
            .find(|r| r.id == rel.id && self.scene.ids[r.from as usize] == rel.from)
        else {
            return;
        };
        let painter = ui.painter();
        let origin = self.last_rect.min;
        let accent = with_alpha(self.session.theme.ui.accent, 255);
        let fill = with_alpha(self.session.theme.ui.panel, 255);
        let halo = with_alpha(self.session.theme.ui.canvas_bg, 220);
        let s = self.camera.world_to_screen(sr.start, viewport);
        painter.circle_filled(Pos2::new(origin.x + s.x, origin.y + s.y), 2.6, accent);
        let e = self.camera.world_to_screen(sr.path.end(), viewport);
        let c = Pos2::new(origin.x + e.x, origin.y + e.y);
        painter.circle_filled(c, REL_HANDLE_R + 1.5, halo);
        painter.circle_filled(c, REL_HANDLE_R, fill);
        painter.circle_stroke(c, REL_HANDLE_R, Stroke::new(1.5, accent));
    }

    /// 拖终点时的橡皮筋 + 目标节点高亮。
    pub(crate) fn paint_relink_preview(&self, ui: &egui::Ui, viewport: [f32; 2]) {
        let Some(rd) = self.relink_drag.as_ref() else {
            return;
        };
        let Some(sr) = self.scene.relationships.iter().find(|r| r.id == rd.sel.id) else {
            return;
        };
        crate::trace::event(&format!(
            "relink-preview drew from={:?} at={:?} target={:?}",
            (sr.start.x, sr.start.y),
            (rd.at.x, rd.at.y),
            rd.target.map(|t| self.scene.ids[t as usize].as_str())
        ));
        let painter = ui.painter();
        let origin = self.last_rect.min;
        let accent = with_alpha(self.session.theme.ui.accent, 220);
        let a = self.camera.world_to_screen(sr.start, viewport);
        let b = self.camera.world_to_screen(rd.at, viewport);
        painter.line_segment(
            [
                Pos2::new(origin.x + a.x, origin.y + a.y),
                Pos2::new(origin.x + b.x, origin.y + b.y),
            ],
            Stroke::new(1.6, accent),
        );
        if let Some(t) = rd.target {
            let r = self.scene.rect(t);
            let p0 = self.camera.world_to_screen(Point::new(r.x, r.y), viewport);
            let p1 = self
                .camera
                .world_to_screen(Point::new(r.right(), r.bottom()), viewport);
            let box_ = egui::Rect::from_min_max(
                Pos2::new(origin.x + p0.x, origin.y + p0.y),
                Pos2::new(origin.x + p1.x, origin.y + p1.y),
            )
            .expand(2.0);
            painter.rect_stroke(
                box_,
                design::radius::SM as u8,
                Stroke::new(2.0, accent),
                egui::StrokeKind::Middle,
            );
        } else if self.concept_map() {
            // 落空不再是「静默取消」：概念图里它会长出一个新概念接上这条线。
            super::paint_blank_concept_hint(
                painter,
                Pos2::new(origin.x + b.x, origin.y + b.y),
                self.camera.zoom,
                accent,
                "松手新建概念并改线",
            );
        }
    }

    pub(crate) fn paint_span_preview(&self, ui: &egui::Ui, viewport: [f32; 2]) {
        let Some(sd) = self.span_drag.as_ref() else {
            return;
        };
        let accent = self.session.theme.ui.accent;
        let painter = ui.painter();
        let origin = self.last_rect.min;
        for id in &sd.preview {
            let Some(idx) = self.scene.find(id) else {
                continue;
            };
            let r = self.scene.rect(idx);
            let a = self.camera.world_to_screen(Point::new(r.x, r.y), viewport);
            let b = self
                .camera
                .world_to_screen(Point::new(r.right(), r.bottom()), viewport);
            let box_ = egui::Rect::from_min_max(
                Pos2::new(origin.x + a.x, origin.y + a.y),
                Pos2::new(origin.x + b.x, origin.y + b.y),
            )
            .expand(2.0);
            painter.rect_filled(box_, design::radius::SM as u8, with_alpha(accent, 26));
            painter.rect_stroke(
                box_,
                design::radius::SM as u8,
                Stroke::new(1.0, with_alpha(accent, 120)),
                egui::StrokeKind::Inside,
            );
        }
    }

    /// 删掉当前选中的注解（边界/概要）；多选时包成一条 `Batch`，一次撤销全回去。
    pub fn delete_annotation(&mut self) -> bool {
        let anns = self.annotations.clone();
        if anns.is_empty() {
            return false;
        }
        let mut commands = Vec::with_capacity(anns.len());
        for ann in &anns {
            commands.push(match ann.kind {
                AnnotationKind::Summary => Command::DeleteSummary {
                    parent: ann.parent.clone(),
                    id: ann.id.clone(),
                },
                AnnotationKind::Boundary => Command::DeleteBoundary {
                    parent: ann.parent.clone(),
                    id: ann.id.clone(),
                },
            });
            crate::trace::event(&format!("delete {} {}", ann.label(), ann.id));
        }
        let cmd = if commands.len() == 1 {
            commands.pop().expect("至少一条")
        } else {
            Command::Batch { commands }
        };
        let ok = self.apply_command(cmd);
        if ok {
            self.annotations.clear();
        }
        ok
    }

    /// 选中项里的自由主题（主根除外）：`(id, 当前屏幕矩形, 存储的 pos)`。
    ///
    /// 树内节点不在此列：它们的位置由布局引擎算，前端改 `pos` 会被核心拒绝。
    /// 选中项里的自由主题（主根除外）：`(id, 当前屏幕矩形, 存储的 pos)`。
    ///
    /// 树内节点不在此列：它们的位置由布局引擎算，前端改 `pos` 会被核心拒绝。
    pub(crate) fn selected_free_topics(&self) -> Vec<(String, Rect, (f32, f32))> {
        let root = self.session.doc.root_id.as_str();
        self.sel
            .ids()
            .iter()
            .filter_map(|&i| {
                let id = self.scene.ids.get(i as usize)?;
                let n = self.session.doc.node(id)?;
                if n.parent_id.is_some() || id == root {
                    return None;
                }
                let rect = self.scene.rect(i);
                let pos = n
                    .pos
                    .as_ref()
                    .map(|p| (p.x, p.y))
                    .unwrap_or((rect.x, rect.y));
                Some((id.clone(), rect, pos))
            })
            .collect()
    }

    /// 选中项里能自由摆放的主题数；右键菜单据此决定是否出现对齐/分布。
    pub fn free_topic_count(&self) -> usize {
        self.selected_free_topics().len()
    }

    pub(crate) fn apply_free_moves(&mut self, moves: Vec<(String, f32, f32)>) -> bool {
        if moves.is_empty() {
            return false;
        }
        let mut commands: Vec<Command> = moves
            .into_iter()
            .map(|(id, x, y)| Command::SetNodePos { id, x, y })
            .collect();
        let cmd = if commands.len() == 1 {
            commands.pop().expect("至少一条")
        } else {
            Command::Batch { commands }
        };
        self.apply_command(cmd)
    }
}
