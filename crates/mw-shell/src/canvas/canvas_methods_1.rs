use super::*;

impl Canvas {
    pub fn new(render_state: &egui_wgpu::RenderState) -> Self {
        let (text, measure): (Option<TextEngine>, Box<dyn TextMeasurer>) =
            match (TextEngine::new(), DWrite::new()) {
                (Some(t), Some(d)) => (Some(t), Box::new(DWriteMeasurer::new(d))),
                _ => (None, Box::new(MonoMeasurer)),
            };

        let session = Session::sample();
        let scene = build_scene(
            &session.doc,
            &session.doc.root_id,
            &session.settings,
            &session.theme,
            &*measure,
            None::<(&str, &str)>,
        );
        let hit = HitIndex::new(&scene);
        let camera = Camera::new(scene.bounds.center(), 1.0, 1.0);
        let compositor = Compositor::from_device(
            render_state.device.clone(),
            render_state.queue.clone(),
            render_state.adapter.get_info(),
        );

        Self {
            session,
            scene,
            scene_cache: SceneCache::new(),
            hit,
            measure,
            text,
            compositor,
            camera,
            sel: Selection::default(),
            annotations: Vec::new(),
            span_drag: None,
            summary_edit: None,
            relationship: RelationshipSelection::default(),
            context_group: None,
            rel_link: None,
            relink_drag: None,
            rel_edit: None,
            focus_edit: None,
            hovered: None,
            marquee_preview: None,
            hovered_rel: None,
            hover_enabled: true,
            tip: None,
            tip_at: 0.0,
            hover_alpha: 0.0,
            hover_detail: None,
            hover_detail_alpha: 0.0,
            select_alpha: 0.0,
            card_open: None,
            notice: None,
            texture: None,
            fitted: false,
            lasso: None,
            show_grid: false,
            grid_step: 40.0,
            show_rulers: false,
            reading: false,
            last_viewport: [1080.0, 720.0],
            last_rect: egui::Rect::ZERO,
            last_view_center: None,
            last_window_size: None,
            budget: FrameBudget::new(),
            editor: None,
            edit_rect: None,
            refocus_canvas: true,
            wants_keys: true,
            drag: None,
            moving: None,
            link: None,
            input_blocked: false,
            overlay: Vec::new(),
            chrome_gate: None,
            focus_band: None,
            swallow_click: false,
            clipboard: Vec::new(),
            style_clipboard: None,
            focus: Vec::new(),
            glow: None,
            reduce_motion: false,
            context: None,
            motion: None,
            demo: None,
            now: 0.0,
            overlap_flash: None,
            flash_window: overlap_flash_window(),
            arrange_undo_depth: None,
            arrange_redo_fit: false,
            fit_viewport: None,
            confirm_delete: None,
            toggle_clicks: ToggleClickMemory::default(),
        }
    }

    /// 壳层每帧告知浮层的屏幕矩形（缩放控件/聚焦面包屑）。空表示没有浮层。
    pub fn set_overlays(&mut self, rects: Vec<egui::Rect>) {
        self.overlay = rects;
    }

    /// 系统「减少动画」设置，由壳层在启动时灌入。
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.reduce_motion = reduce;
        if reduce {
            self.motion = None;
        }
    }

    /// 离屏 golden 用：关闭 hover，基线不受宿主鼠标位置影响。
    pub fn set_hover_enabled(&mut self, on: bool) {
        self.hover_enabled = on;
        if !on {
            self.hovered = None;
            self.hover_detail = None;
            self.hover_detail_alpha = 0.0;
        }
    }

    pub(crate) fn pointer_blocked_at(&self, p: egui::Pos2) -> bool {
        self.overlay.iter().any(|r| r.contains(p))
            || self.chrome_gate.is_some_and(|r| r.contains(p))
            || self.focus_band.is_some_and(|r| r.contains(p))
    }

    pub(crate) fn pointer_over_overlay(&self, ui: &egui::Ui) -> bool {
        ui.input(|i| {
            i.pointer
                .interact_pos()
                .is_some_and(|p| self.pointer_blocked_at(p))
        })
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// X29 G-1：活动标签的只读态由壳层每帧灌进来；文档层在 `apply/undo/redo`
    /// 统一拒绝，画布/大纲/检查器不用各写一遍闸。
    pub fn set_read_only(&mut self, on: bool) {
        self.session.doc.read_only = on;
    }

    /// 撤销栈上限是进程级偏好：设置中心与每帧对账都从这里落到活动文档。
    pub fn set_history_limit(&mut self, limit: usize) {
        self.session.doc.set_history_limit(limit);
    }

    /// 把图片字节加入文档资源包，返回内容哈希。重复字节只存一份。
    pub fn add_asset(&mut self, bytes: Vec<u8>) -> String {
        self.session.doc.assets.insert(bytes)
    }

    /// 当前场景（SVG 导出直接读它：纯 CPU 标记管线，不经过合成器）。
    /// 换一份文档（打开文件、将来的"新建"）。场景/命中/选择/编辑器全部作废重来，
    /// 相机交回首帧 `fit`——不这么做，打开一份 20 倍大的文档会看到一片空白。
    /// 当前场景（SVG 导出直接读它：纯 CPU 标记管线，不经过合成器）。
    /// 换一份文档（打开文件、将来的"新建"）。场景/命中/选择/编辑器全部作废重来，
    /// 相机交回首帧 `fit`——不这么做，打开一份 20 倍大的文档会看到一片空白。
    pub fn replace_session(&mut self, session: Session) {
        // 演示的是旧文档：换文档即退出演示，别把高亮副本带到新场景上。
        self.demo = None;
        self.session = session;
        self.editor = None;
        self.summary_edit = None;
        self.relationship.clear();
        self.rel_link = None;
        self.relink_drag = None;
        self.rel_edit = None;
        self.drag = None;
        self.moving = None;
        self.link = None;
        self.span_drag = None;
        self.sel = Selection::default();
        self.hovered = None;
        self.hover_detail = None;
        self.hover_detail_alpha = 0.0;
        self.focus.clear();
        self.glow = None;
        self.rebuild();
        self.motion = None;
        self.fitted = false;
        self.refocus_canvas = true;
    }

    /// 当前相机。多标签切换时把它存回标签槽。
    pub fn camera(&self) -> Camera {
        self.camera
    }

    pub fn demo_active(&self) -> bool {
        self.demo.is_some()
    }

    /// 进入演示：从主选节点（无选中则根）开始，先序走可见树。
    ///
    /// 编辑中的那点文字先提交——演示时画布不收输入，编辑器挂在上面只会挡住节点。
    /// 进入演示：从主选节点（无选中则根）开始，先序走可见树。
    ///
    /// 编辑中的那点文字先提交——演示时画布不收输入，编辑器挂在上面只会挡住节点。
    pub fn start_demo(&mut self) -> bool {
        if self.demo.is_some() {
            return true;
        }
        if self.editor.is_some() {
            self.commit_editing();
        }
        let start = self.sel.primary();
        let Some(demo) = Demo::start(
            &self.scene,
            &self.session.theme,
            self.camera,
            start,
            self.last_viewport,
            !self.reduce_motion,
        ) else {
            return false;
        };
        self.camera = demo.camera(Instant::now());
        self.demo = Some(demo);
        // 演示期间的画面完全由高亮副本决定；残留的过渡/手势只会把索引带错。
        self.motion = None;
        self.drag = None;
        self.moving = None;
        self.link = None;
        self.span_drag = None;
        self.hovered = None;
        self.hover_detail = None;
        self.hover_detail_alpha = 0.0;
        self.glow = None;
        self.context = None;
        true
    }

    /// 退出演示，相机回到进入前的视口（docs/02 §9）。
    pub fn stop_demo(&mut self) -> bool {
        let Some(demo) = self.demo.take() else {
            return false;
        };
        self.camera = demo.saved_camera();
        self.refocus_canvas = true;
        true
    }

    pub fn demo_next(&mut self) -> bool {
        let reduce = self.reduce_motion;
        let viewport = self.last_viewport;
        let Some(demo) = self.demo.as_mut() else {
            return false;
        };
        let moved = demo.next(viewport, !reduce);
        if moved {
            self.camera = demo.camera(Instant::now());
        }
        moved
    }

    pub fn demo_prev(&mut self) -> bool {
        let reduce = self.reduce_motion;
        let viewport = self.last_viewport;
        let Some(demo) = self.demo.as_mut() else {
            return false;
        };
        let moved = demo.prev(viewport, !reduce);
        if moved {
            self.camera = demo.camera(Instant::now());
        }
        moved
    }

    pub fn toggle_demo_bar(&mut self) -> bool {
        let Some(demo) = self.demo.as_mut() else {
            return false;
        };
        demo.toggle_bar();
        true
    }

    /// 演示当前节点的卡片（有卡片才能打开卡片页）。
    pub fn demo_card(&self) -> Option<mw_core::NoteCard> {
        let demo = self.demo.as_ref()?;
        let id = self.scene.ids.get(demo.current() as usize)?;
        let node = self.session.doc.node(id)?;
        node.card_id
            .as_deref()
            .and_then(|cid| self.session.doc.card(cid))
            .cloned()
    }

    pub fn demo_info(&self) -> Option<DemoInfo> {
        let demo = self.demo.as_ref()?;
        let id = self.scene.ids.get(demo.current() as usize)?;
        let title = self
            .node_text(id)
            .filter(|t| !t.trim().is_empty())
            .unwrap_or("未命名主题")
            .to_string();
        let node = self.session.doc.node(id);
        let note = node
            .and_then(|n| n.note.clone())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                node.and_then(|n| n.card_id.as_deref())
                    .and_then(|cid| self.session.doc.card(cid))
                    .map(|c| c.body.clone())
                    .filter(|s| !s.trim().is_empty())
            });
        let card = node.and_then(|n| n.card_id.clone());
        Some(DemoInfo {
            index: demo.index(),
            total: demo.len(),
            title,
            note,
            card,
            bar_hidden: demo.bar_hidden(),
        })
    }

    /// 导出取景框：场景包围盒 + 边界框/关系线鼓出部分/折叠圆点（`export::content_bounds`）。
    /// 导出对话框用它预告成品像素尺寸，离屏渲染也用它定相机。
    /// 导出取景框：场景包围盒 + 边界框/关系线鼓出部分/折叠圆点（`export::content_bounds`）。
    /// 导出对话框用它预告成品像素尺寸，离屏渲染也用它定相机。
    pub fn bounds(&self) -> mw_core::Rect {
        crate::export::content_bounds(&self.scene)
    }

    /// 与另一个标签槽**对调**会话后重排（多标签切换）。
    ///
    /// 刻意不重建 `Canvas`：合成器与 3072² 字形图集是 9MB 级的资源，按标签线性复制
    /// 会把内存预算撑爆。切换只换文档、场景、选择与相机，图集继续复用（字形键与文档无关）。
    /// 与另一个标签槽**对调**会话后重排（多标签切换）。
    ///
    /// 刻意不重建 `Canvas`：合成器与 3072² 字形图集是 9MB 级的资源，按标签线性复制
    /// 会把内存预算撑爆。切换只换文档、场景、选择与相机，图集继续复用（字形键与文档无关）。
    pub fn swap_session(&mut self, other: &mut Session) {
        std::mem::swap(&mut self.session, other);
        // 换文档即作废所有交互中间态：编辑、拖拽、连线、悬停都指向旧场景的索引。
        self.demo = None;
        self.editor = None;
        self.summary_edit = None;
        self.relationship.clear();
        self.rel_link = None;
        self.relink_drag = None;
        self.rel_edit = None;
        self.drag = None;
        self.moving = None;
        self.link = None;
        self.span_drag = None;
        self.sel = Selection::default();
        self.hovered = None;
        self.hover_detail = None;
        self.hover_detail_alpha = 0.0;
        self.focus.clear();
        self.glow = None;
        self.rebuild();
        // 不同文档之间不做位移过渡：两棵树的 id 大多对不上，动起来只会像闪屏。
        self.motion = None;
        self.fitted = false;
        self.refocus_canvas = true;
    }

    /// 恢复某标签页的视图（相机 + 选择）。`fitted = true`：别再首帧 fit 把对准的相机拽走；
    /// 选择按节点 id 重映射到新场景的索引（场景索引随文档而变，不能直接搬）。
    /// 恢复某标签页的视图（相机 + 选择）。`fitted = true`：别再首帧 fit 把对准的相机拽走；
    /// 选择按节点 id 重映射到新场景的索引（场景索引随文档而变，不能直接搬）。
    pub fn set_view(&mut self, camera: Camera, sel_ids: &[String]) {
        self.demo = None;
        self.camera = camera;
        let ids: Vec<u32> = sel_ids
            .iter()
            .filter_map(|id| self.scene.find(id))
            .collect();
        self.sel = Selection::default();
        self.sel.replace(ids);
        self.fitted = true;
        self.refocus_canvas = true;
    }

    /// 有浮层盖在画布上时，画布应当暂时"聋"掉：键盘快捷键（Tab/Enter/Esc/Delete/F2/撤销）
    /// 与 Home / Ctrl±、浮层上的点击拖拽都不再响应。指针在画布上的手势**不**受它管。
    pub fn set_input_blocked(&mut self, blocked: bool) {
        self.input_blocked = blocked;
    }

    /// 画布输入是否被浮层/幕布接管。窗口级和弦要读这个位：右键菜单开着的时候，
    /// `Ctrl+C` 归菜单，不该同时把选中子树压进剪贴板。
    /// 画布输入是否被浮层/幕布接管。窗口级和弦要读这个位：右键菜单开着的时候，
    /// `Ctrl+C` 归菜单，不该同时把选中子树压进剪贴板。
    pub fn input_blocked(&self) -> bool {
        self.input_blocked
    }

    /// 当前选中的节点 id（有序，末位是主选）。面板要拿它高亮。
    pub fn selected_ids(&self) -> Vec<String> {
        self.sel
            .ids()
            .iter()
            .filter_map(|&i| self.scene.ids.get(i as usize).cloned())
            .collect()
    }

    pub fn zoom(&self) -> f32 {
        self.camera.zoom
    }

    /// 有没有选中节点（导出对话框据此决定「选中子树」可不可点）。
    pub fn has_selection(&self) -> bool {
        !self.sel.ids().is_empty()
    }

    /// 当前相机看到的**世界坐标**矩形。导出「可视区域」用它当取景框。
    pub fn view_bounds(&self) -> Rect {
        let [w, h] = self.last_viewport;
        let zoom = self.camera.zoom.max(1e-4);
        let hw = w * 0.5 / zoom;
        let hh = h * 0.5 / zoom;
        Rect::new(
            self.camera.center.x - hw,
            self.camera.center.y - hh,
            hw * 2.0,
            hh * 2.0,
        )
    }

    /// 文本格式的「选中子树」：把主选子树摘成一份临时文档；没有选中时返回 `None`。
    pub fn selection_document(&self) -> Option<mw_core::Document> {
        let id = self.primary_id()?;
        subtree_document(&self.session.doc, &id)
    }

    /// 导出用的场景：`selection` 为真时以主选为布局根，只排那棵子树。
    ///
    /// 刻意不传行内编辑快照——导出的永远是**已提交**的文档，不然导出图里会带着
    /// 半截输入框的文字。布局与主题直接取 `doc.meta`，不受画布当前聚焦层级影响。
    pub fn export_scene(&self, selection: bool) -> Scene {
        let doc = &self.session.doc;
        let root = if selection {
            self.primary_id().unwrap_or_else(|| doc.root_id.clone())
        } else {
            doc.root_id.clone()
        };
        let theme = theme_for(&doc.meta);
        let mut scene = build_scene(doc, &root, &doc.meta.layout, &theme, &*self.measure, None);
        // ⑥ 导出补齐（docs/32 §6）：聚焦问题也要出现在导出的图里。
        // 选中一棵子树导出时不带——问题属于整张图。
        if !selection {
            crate::export::attach_focus_note(&mut scene, doc, &theme);
        }
        scene
    }

    /// 性能浮层的一帧快照（docs/08 §8）。只在 `MW_STATS=1` 时被调用；
    /// 可见数按当前视口与目标场景算，不含过渡中的插值盒。
    /// 性能浮层的一帧快照（docs/08 §8）。只在 `MW_STATS=1` 时被调用；
    /// 可见数按当前视口与目标场景算，不含过渡中的插值盒。
    pub fn perf_snapshot(&self) -> PerfSnapshot {
        let [w, h] = self.last_viewport;
        let zoom = self.camera.zoom.max(1e-4);
        let hw = w * 0.5 / zoom;
        let hh = h * 0.5 / zoom;
        let view = Rect::new(
            self.camera.center.x - hw,
            self.camera.center.y - hh,
            hw * 2.0,
            hh * 2.0,
        );
        let visible = self
            .scene
            .nodes
            .iter()
            .filter(|n| view.intersects(n.geom.rect))
            .count();
        PerfSnapshot {
            nodes: self.scene.nodes.len(),
            visible,
            zoom: self.camera.zoom,
            low_render: self.budget.low_render(),
            motion: self.motion.is_some(),
        }
    }

    /// 执行一条来自面板/检查器的命令。与 `toggle_collapsed` 的区别只有一个：
    /// **不抢键盘焦点**——检查器的备注/链接正在输入时，每次样式提交都抢一次焦点，
    /// 用户下一键就打到画布上了。
    /// 执行一条来自面板/检查器的命令。与 `toggle_collapsed` 的区别只有一个：
    /// **不抢键盘焦点**——检查器的备注/链接正在输入时，每次样式提交都抢一次焦点，
    /// 用户下一键就打到画布上了。
    pub fn apply_command(&mut self, cmd: Command) -> bool {
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        self.rebuild();
        true
    }

    /// 同上，但允许并进撤销栈顶那一条（§4.3：500ms 内同类连续操作合并为一条撤销）。
    ///
    /// `merge` 由调用方给：它只看得到「栈顶那条 ChangeSet」，判断「算不算同一次手势」
    /// 是壳层的事（时间窗 + 命令种类都在那边）。
    pub fn apply_command_coalesced(
        &mut self,
        cmd: Command,
        merge: impl FnOnce(&mw_core::ChangeSet) -> bool,
    ) -> bool {
        if self.session.doc.apply_coalesced(&cmd, merge).is_err() {
            return false;
        }
        self.rebuild();
        true
    }

    /// 面板关闭/收起之后把键盘还给画布（快捷键才能继续用）。
    pub fn request_canvas_focus(&mut self) {
        self.refocus_canvas = true;
    }

    /// 从面板选中一个节点：换选择 + 把相机移过去。
    ///
    /// `fitted = true` 是必须的：首帧的自动 fit 会把刚对准的位置又拽回全图。
    /// 从面板选中一个节点：换选择 + 把相机移过去。
    ///
    /// `fitted = true` 是必须的：首帧的自动 fit 会把刚对准的位置又拽回全图。
    pub fn select_node(&mut self, id: &str) {
        let Some(idx) = self.scene.find(id) else {
            return;
        };
        self.select_node_only(idx, "select-node");
        let c = self.scene.rect(idx).center();
        self.camera.center = Point::new(c.x, c.y);
        self.fitted = true;
        self.refocus_canvas = true;
        self.glow = None;
    }

    /// 面板多选：切换一个节点的选中态，不动相机（连续点选时画面不该跳）。
    pub fn toggle_select_node(&mut self, id: &str) {
        let Some(idx) = self.scene.find(id) else {
            return;
        };
        self.sel.click_with(idx, true, false);
        self.annotations.clear();
        self.relationship.clear();
        self.fitted = true;
        self.refocus_canvas = true;
        self.glow = None;
    }

    /// 大纲区间选择：整组替换，`primary` 永远排在最后（键盘当前行）。
    /// 不动相机——拉选时画面跟着跳会让人抓不住行。
    pub fn select_many(&mut self, ids: &[String], primary: &str) {
        let mut indices: Vec<u32> = Vec::new();
        for id in ids {
            if let Some(idx) = self.scene.find(id) {
                if !indices.contains(&idx) {
                    indices.push(idx);
                }
            }
        }
        if let Some(p) = self.scene.find(primary) {
            indices.retain(|&i| i != p);
            indices.push(p);
        }
        self.sel.replace(indices);
        self.annotations.clear();
        self.relationship.clear();
        self.fitted = true;
        self.refocus_canvas = true;
        self.glow = None;
    }

    /// 语义上「这个节点现在排得出来吗」：折叠子树与聚焦模式之外的节点为 `false`。
    pub(crate) fn node_visible(&self, id: &str) -> bool {
        self.scene.find(id).is_some()
    }

    /// 搜索结果跳转：选节点、对相机，但**不抢键盘**——搜索框还开着，
    /// 用户接着按 Enter 就该走下一条，而不是变成「加同级主题」。
    ///
    /// 命中折叠子树里的节点时，先展开挡路的祖先（一条 `Batch`，一次撤销全收回）再选中；
    /// 命中在**聚焦视图之外**时，用户点结果就是明确要过去：先退出全部聚焦（视图态，
    /// 不入撤销栈）再照常展开/选中，轨迹写 `search_focus_exit`。其余不可达情况返回
    /// `false`，由壳层给提示。
    pub fn select_node_keep_focus(&mut self, id: &str) -> bool {
        self.reveal_and_select(id, "search")
    }

    /// `why` 只进轨迹前缀（搜索 `search`、卡片来源 `card`），揭示逻辑同一条。
    pub fn reveal_and_select(&mut self, id: &str, why: &str) -> bool {
        if self.scene.find(id).is_none() {
            if let Some(levels) = focus_exit_levels(&self.session.doc, &self.focus, id) {
                self.focus_to(0);
                crate::trace::event(&format!("{why}_focus_exit id={id} levels={levels}"));
            }
        }
        if self.scene.find(id).is_none() {
            if !in_layout_subtree(&self.session.doc, &self.layout_root(), id) {
                return false;
            }
            let commands = reveal_commands(&self.session.doc, id);
            if commands.is_empty() {
                return false;
            }
            let expanded = commands.len();
            if !self.apply_command(Command::Batch { commands }) {
                return false;
            }
            crate::trace::event(&format!("{why}_reveal id={id} expanded={expanded}"));
        }
        if self.scene.find(id).is_none() {
            return false;
        }
        self.select_node(id);
        self.refocus_canvas = false;
        self.glow = Some(Glow {
            id: id.to_string(),
            start: Instant::now(),
        });
        true
    }

    /// 方向键在节点间移动选择（核心手势）：按屏幕几何找该方向上最近的节点。
    /// 没有选中时先落到根；只动选择与相机，不改文档。
    /// 方向键在节点间移动选择（核心手势）：按屏幕几何找该方向上最近的节点。
    /// 没有选中时先落到根；只动选择与相机，不改文档。
    pub fn navigate(&mut self, dx: f32, dy: f32) -> bool {
        // 方向键是「选节点」的手势：关系线/注解选中必须先退场，
        // 否则节点环与关系线强调会并存，下一次 Delete 跨道删两样东西（M26）。
        let Some(from) = self.sel.primary() else {
            let Some(root) = self.scene.find(&self.session.doc.root_id) else {
                return false;
            };
            self.select_node_only(root, "navigate");
            self.reveal_node(root);
            return true;
        };
        let Some(next) = directional_neighbor(&self.scene, from, dx, dy) else {
            return false;
        };
        self.select_node_only(next, "navigate");
        self.reveal_node(next);
        true
    }

    /// 最小平移：目标盒不在视口里时，只挪到刚好露出（四周留 48 CSS px）。
    /// 方向键连按时相机不会一跳一跳地重新居中，手感接近原生列表。
    /// 最小平移：目标盒不在视口里时，只挪到刚好露出（四周留 48 CSS px）。
    /// 方向键连按时相机不会一跳一跳地重新居中，手感接近原生列表。
    pub(crate) fn reveal_node(&mut self, idx: u32) {
        let r = self.scene.rect(idx);
        self.camera.center = self.reveal_center(r);
        self.fitted = true;
        if crate::trace::is_enabled() {
            let c = self.camera.center;
            crate::trace::event(&format!(
                "reveal idx={idx} cam=({:.0},{:.0}) z={:.2} node=({:.0},{:.0},{:.0},{:.0}) vp=({:.0},{:.0})",
                c.x,
                c.y,
                self.camera.zoom,
                r.x,
                r.y,
                r.w,
                r.h,
                self.last_viewport[0],
                self.last_viewport[1]
            ));
        }
    }

    /// 最小平移的纯计算：目标盒不在视口里时，只挪到刚好露出（四周留 48 CSS px）。
    /// 抽成纯函数是为了让「跟随永远留在屏内」可以被单测钉住（W50）。
    pub(crate) fn reveal_center(&self, r: Rect) -> Point {
        reveal_center(self.camera.center, self.camera.zoom, self.last_viewport, r)
    }

    /// 聚焦当前主选子树（Alt+Enter）。只影响视图，不改文档。
    pub fn focus_selected(&mut self) -> bool {
        let Some(id) = self.primary_id() else {
            return false;
        };
        if self.focus.last() == Some(&id) {
            return false;
        }
        self.focus.push(id);
        self.rebuild();
        let vp = self.last_viewport;
        self.fit(vp);
        self.refocus_canvas = true;
        true
    }

    /// 退出最内层聚焦（Esc / 面包屑）。返回是否真的退了。
    pub fn focus_pop(&mut self) -> bool {
        if self.focus.pop().is_none() {
            return false;
        }
        self.rebuild();
        let vp = self.last_viewport;
        self.fit(vp);
        self.refocus_canvas = true;
        true
    }

    /// 面包屑跳转：`depth` 是路径下标（0 = 主根，即退出全部聚焦）。
    pub fn focus_to(&mut self, depth: usize) -> bool {
        if depth == 0 {
            if self.focus.is_empty() {
                return false;
            }
            self.focus.clear();
        } else {
            if depth >= self.focus.len() {
                return false;
            }
            self.focus.truncate(depth);
        }
        self.rebuild();
        let vp = self.last_viewport;
        self.fit(vp);
        self.refocus_canvas = true;
        true
    }

    /// 聚焦面包屑：主根 → … → 当前聚焦根。未聚焦时为空。
    pub fn focus_path(&self) -> Vec<(String, String)> {
        let Some(cur) = self.focus.last() else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        let mut at = Some(cur.as_str());
        while let Some(id) = at {
            ids.push(id.to_string());
            at = self
                .session
                .doc
                .node(id)
                .and_then(|n| n.parent_id.as_deref());
        }
        ids.reverse();
        ids.into_iter()
            .map(|id| {
                let text = self
                    .session
                    .doc
                    .node(&id)
                    .map(|n| n.text.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| "未命名主题".to_string());
                (id, text)
            })
            .collect()
    }

    /// 文本 + 拼音检索。索引在 `Document` 里，重复查询不重建拼音键。
    pub fn search_nodes(
        &mut self,
        query: &str,
        mode: mw_core::MatchMode,
    ) -> Vec<mw_core::SearchHit> {
        self.session.doc.search_with_mode(query, mode)
    }

    /// 概念图命题短语检索（关系线单独一组，不混进节点结果）。
    pub fn search_relationships(
        &mut self,
        query: &str,
        mode: mw_core::MatchMode,
    ) -> Vec<mw_core::RelationshipHit> {
        self.session.doc.search_relationships_with_mode(query, mode)
    }

    /// 路径命中：节点文字不一定含查询，但「根 › 分支 › 父」含。返回 (id, 文字, 路径)。
    pub fn search_paths(&self, query: &str) -> Vec<(String, String, String)> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for id in self.session.doc.doc_order() {
            let path = path_of(&self.session.doc, &id);
            if path.to_lowercase().contains(&q) {
                let text = self
                    .session
                    .doc
                    .node(&id)
                    .map(|n| n.text.clone())
                    .unwrap_or_default();
                out.push((id, text, path));
                if out.len() >= 4 {
                    break;
                }
            }
        }
        out
    }

    /// 节点在树里的路径（不含自己）：根 › 甲分支 › 父。搜索结果用它说明"这条在哪儿"。
    pub fn node_path(&self, id: &str) -> String {
        path_of(&self.session.doc, id)
    }

    /// 搜索结果行的显示文本。节点可能没有文本，由壳层兜底。
    pub fn node_text(&self, id: &str) -> Option<&str> {
        self.session.doc.node(id).map(|n| n.text.as_str())
    }

    /// 拖拽时贴近画布边缘自动平移视野。指针用窗口坐标，`rect` 是画布屏幕矩形。
    pub(crate) fn auto_scroll(&mut self, pointer: Pos2, rect: egui::Rect, dt: f32) -> bool {
        if self.camera.zoom <= f32::EPSILON {
            return false;
        }
        let local = pointer - rect.min;
        let dx = edge_scroll_speed(
            local.x,
            0.0,
            rect.width(),
            AUTO_SCROLL_MARGIN,
            AUTO_SCROLL_SPEED,
        );
        let dy = edge_scroll_speed(
            local.y,
            0.0,
            rect.height(),
            AUTO_SCROLL_MARGIN,
            AUTO_SCROLL_SPEED,
        );
        if dx.abs() < 0.01 && dy.abs() < 0.01 {
            return false;
        }
        self.camera.center.x += dx * dt / self.camera.zoom;
        self.camera.center.y += dy * dt / self.camera.zoom;
        true
    }

    /// 切换网格（默认关）。网格同时参与移动吸附。
    pub fn toggle_grid(&mut self) -> bool {
        self.show_grid = !self.show_grid;
        self.show_grid
    }

    pub fn toggle_rulers(&mut self) -> bool {
        self.show_rulers = !self.show_rulers;
        self.show_rulers
    }

    /// 阅读模式只影响装饰性 UI，不改相机/选择/文档。
    pub fn set_reading(&mut self, on: bool) {
        self.reading = on;
    }

    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
        self.fitted = true;
        // 相机是外部给定的（阅读模式/场景夹具），不是按当前视口算出来的；
        // 记成「已按此视口定过」，启动重取景才不会把它覆盖掉。
        self.fit_viewport = Some(self.last_viewport);
    }

    /// 上一次取景用的视口（`None` = 还没取过景）。
    pub fn fit_viewport(&self) -> Option<[f32; 2]> {
        self.fit_viewport
    }

    /// 本帧画布视口（逻辑 px）。
    pub fn viewport(&self) -> [f32; 2] {
        self.last_viewport
    }

    /// 只读的当前场景。右下角缩略卡（M30）据此算代理块。
    ///
    /// **只给取景导航用**：`Scene` 是几何与视觉的唯一真相源，画布/导出/缩略图共用同一份，
    /// 但别拿它另起一条绘制路径（docs/04 核心纪律 3）。
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// 把相机中心搬到世界点——小地图点击/拖动定位的落点。
    ///
    /// `fitted = true` 与 [`Canvas::select_node`] 同一条道理：这是用户手定的取景，
    /// 启动或改窗口时的自动 fit 不该再把它拽回全图。
    pub fn pan_to(&mut self, center: Point) {
        self.camera.center = center;
        self.fitted = true;
    }

    /// 以画布内的屏幕点 `anchor` 为锚缩放一档，并记一条 `zoom_anchor` 轨迹（X21）。
    ///
    /// **五条入口共用这一处**：画布滚轮、双指捏合、`Ctrl+=`/`Ctrl+-`、缩放条/命令面板、
    /// 小地图滚轮。锚点换算、钳位、轨迹只写一遍——键盘缩放曾经自己改 `camera.zoom`，
    /// 画面绕世界原点漂，与滚轮变成两套手感。
    ///
    /// 返回相机是否真的动了：停在下限/上限或编辑期锁住时调用方不该白重绘一帧。
    pub fn zoom_step_at(&mut self, anchor: Point, factor: f32, src: ZoomSrc) -> bool {
        let viewport = self.last_viewport;
        // 锚点下的世界点在缩放前后必须相同（这正是 `zoom_at_anchor` 保证的不变量），
        // 轨迹里那个 world 因此取缩放前的值——读者一眼能对上「哪个世界点被钉住了」。
        let world = self.camera.screen_to_world(anchor, viewport);
        // 编辑期锁缩放（`docs/06:108`）：缩放会让 caret 与候选窗错位，平移则可以。
        // 锁收在这里而不是滚轮分支里，四条入口才共用同一条合同。被锁住写**另一条**
        // `zoom_blocked`，不再借 `zoom_anchor` 的位置——两条轨迹各自一种句法，探针
        // 不必再从 `locked=` 后缀去猜这一行是不是「没发生」。
        if self.editor.is_some() {
            self.trace_zoom_blocked(src);
            return false;
        }
        let moved = zoom_step_camera(&mut self.camera, anchor, viewport, factor);
        // 限位：请求的倍率是合法的、也确实比 1.0 大/小，但相机纹丝没动——上下限把它
        // 整口吃掉了。这种情况同样要留一行，否则「按了没反应」在日志里跟「事件没送达」
        // 一模一样（`docs/06` 的 10%–500% 是合同值，用户撞墙必须看得见）。
        let clamped =
            !moved && factor.is_finite() && factor > 0.0 && (factor - 1.0).abs() > f32::EPSILON;
        self.trace_zoom_anchor(src, anchor, world, clamped);
        moved
    }

    /// 一次缩放的验收轨迹（X21）：`src` 说明哪个入口，`at`/`world`/`drift`/`zoom` 说明锚点落点。
    ///
    /// `at` 是画布内屏幕点，`world` 是它缩放前对着的世界点。`drift` 是**同一个世界点**
    /// 缩放后在屏幕上离 `at` 多远（css px，绝对值两分量）：正确实现只该有浮点误差
    /// （≤ 0.01），于是「锚住了」这句话不必靠两张截图比对，日志自己就证完了。
    /// 被上下限吃掉的请求多一个 `clamped=1` 后缀：倍率没变也得有这一行。
    /// `format!` 必须在 `trace::is_enabled()` 门后。
    fn trace_zoom_anchor(&self, src: ZoomSrc, at: Point, world: Point, clamped: bool) {
        if !crate::trace::is_enabled() {
            return;
        }
        let back = self.camera.world_to_screen(world, self.last_viewport);
        crate::trace::event(&format!(
            "zoom_anchor src={} at=({:.2},{:.2}) world=({:.2},{:.2}) zoom={:.4} drift=({:.2},{:.2}){}",
            src.as_str(),
            at.x,
            at.y,
            world.x,
            world.y,
            self.camera.zoom,
            (back.x - at.x).abs(),
            (back.y - at.y).abs(),
            if clamped { " clamped=1" } else { "" }
        ));
    }

    /// 编辑期锁缩放（X21）：与 `zoom_anchor` 分开一条句法，一眼能看出「没发生」。
    ///
    /// 只由 [`Canvas::zoom_step_at`](Canvas::zoom_step_at) 的早退写：五条入口共用它，
    /// 不会出现「滚轮被锁、按钮还能缩」的半套合同。
    fn trace_zoom_blocked(&self, src: ZoomSrc) {
        if !crate::trace::is_enabled() {
            return;
        }
        crate::trace::event(&format!("zoom_blocked src={} editing=1", src.as_str()));
    }

    /// 按文档 id 恢复多选；目标不在场景里就跳过。
    pub fn select_ids(&mut self, ids: &[String]) -> bool {
        // 恢复节点选中（阅读模式退出）同样要收掉关系线/注解那两道。
        self.clear_foreign_selection(SelectionLane::Node, "select-ids");
        let picked: Vec<u32> = ids.iter().filter_map(|id| self.scene.find(id)).collect();
        let changed = self.sel.ids() != picked.as_slice();
        self.sel.replace(picked);
        changed
    }

    /// 适应画布（命令面板 / 将来的菜单都调它）。
    pub fn fit_view(&mut self) {
        let vp = self.last_viewport;
        self.fit(vp);
        self.fitted = true;
    }

    /// 让下一帧按当时的视口重新取景（窗口几何恢复后，画布尺寸会晚一两帧才落定）。
    pub fn request_fit(&mut self) {
        self.fitted = false;
    }

    /// 以画布中心为锚点缩放一档（缩放条 −/+ 与命令面板/菜单）。
    ///
    /// `src` 只影响轨迹：按钮和菜单键在日志里要分得开，数学完全一样。
    /// 这里过去只改 `camera.zoom`（注释说的「以画布中心为锚」其实没做），于是从工具条
    /// 或命令面板缩放时画面绕世界原点漂——滚轮缩放走的是 `zoom_at_anchor`，同一份相机
    /// 两条口径。现在统一走 [`Canvas::zoom_step_at`]。
    pub fn zoom_by(&mut self, factor: f32, src: ZoomSrc) {
        let viewport = self.last_viewport;
        let at = Point::new(viewport[0] * 0.5, viewport[1] * 0.5);
        self.zoom_step_at(at, factor, src);
    }

    /// 取走本帧的右键请求。壳层在 `show` 之后调用，构建菜单。
    pub fn take_context_request(&mut self) -> Option<ContextRequest> {
        self.context.take()
    }

    /// 顶部工具条的「更多」：按当前选区弹节点菜单，落点是按钮下缘。
    pub fn request_node_context_at(&mut self, pos: egui::Pos2) {
        if !self.sel.is_empty() {
            self.context = Some(ContextRequest {
                pos,
                target: ContextTarget::Node,
            });
        }
    }

    /// 取走"点了卡片角标"的请求。
    pub fn take_card_open(&mut self) -> Option<String> {
        self.card_open.take()
    }

    /// 取走画布留在这一帧的提示（文案, 是否告警）。
    pub fn take_notice(&mut self) -> Option<(String, bool)> {
        self.notice.take()
    }

    pub fn selected_count(&self) -> usize {
        self.sel.len()
    }

    /// 主选有孩子时返回折叠态；菜单用它把条目写成「折叠」或「展开」。
    pub fn primary_collapsed(&self) -> Option<bool> {
        self.primary_id()
            .and_then(|id| self.session.doc.node(&id))
            .filter(|n| !n.children.is_empty())
            .map(|n| n.collapsed)
    }

    /// 进程内节点剪贴板是否有内容。
    pub fn has_clipboard(&self) -> bool {
        !self.clipboard.is_empty()
    }

    pub fn has_style_clipboard(&self) -> bool {
        self.style_clipboard.is_some()
    }

    /// 进程内剪贴板的纯文本（先序、按层级缩进）。菜单复制时同步到系统剪贴板。
    pub fn clipboard_plain_text(&self) -> String {
        self.clipboard_text()
    }

    /// 主选还能不能聚焦：已经是当前聚焦根时再聚焦什么都不会变。
    pub fn can_focus_primary(&self) -> bool {
        self.primary_id()
            .is_some_and(|id| self.focus.last() != Some(&id))
    }

    /// 多选里是否至少有一个还能折叠的节点。
    pub fn can_collapse_selection(&self) -> bool {
        self.selected_ids().iter().any(|id| {
            self.session
                .doc
                .node(id)
                .is_some_and(|n| !n.children.is_empty() && !n.collapsed)
        })
    }

    /// 「移入同一父级」是否至少能挪动一个节点（都已在主选的父级下就没有意义）。
    pub fn can_move_same_parent(&self) -> bool {
        let Some(primary) = self.primary_id() else {
            return false;
        };
        let Some(target) = self
            .session
            .doc
            .node(&primary)
            .and_then(|n| n.parent_id.clone())
        else {
            return false;
        };
        let selected = self.selected_ids();
        selected_roots(&self.session.doc, &selected)
            .into_iter()
            .any(|id| {
                id != target
                    && self
                        .session
                        .doc
                        .node(&id)
                        .and_then(|n| n.parent_id.as_deref())
                        != Some(target.as_str())
            })
    }

    /// 菜单入口：复制（与 Ctrl+C 同一份进程内剪贴板）。
    pub fn copy_action(&mut self) {
        self.copy_selected();
    }

    /// 菜单入口：剪切 = 复制 + 删除，撤销一次即可回来。
    pub fn cut_action(&mut self) -> bool {
        if !self.can_delete() {
            return false;
        }
        self.copy_selected();
        self.delete_selected()
    }

    pub fn paste_action(&mut self) -> bool {
        self.paste_clipboard()
    }

    /// 空白处右键的粘贴：始终落到主根，忽略残留选择。
    pub fn paste_at_root_action(&mut self) -> bool {
        let root = self.session.doc.root_id.clone();
        self.paste_under(Some(root))
    }

    pub fn copy_style_action(&mut self) {
        self.copy_style();
    }

    pub fn paste_style_action(&mut self) -> bool {
        self.paste_style()
    }

    /// 执行一条窗口级剪贴板和弦。复制/剪切顺带把纯文本写进系统剪贴板——空剪贴板时
    /// egui 连 `Event::Paste` 都不会发，只填我们自己的那份剪贴板会让 `Ctrl+V` 看起来没反应。
    /// 执行一条窗口级剪贴板和弦。复制/剪切顺带把纯文本写进系统剪贴板——空剪贴板时
    /// egui 连 `Event::Paste` 都不会发，只填我们自己的那份剪贴板会让 `Ctrl+V` 看起来没反应。
    pub fn apply_clipboard(&mut self, ctx: &egui::Context, chord: ClipboardChord) -> bool {
        match chord {
            ClipboardChord::Copy => {
                self.copy_selected();
                let text = self.clipboard_text();
                if !text.is_empty() {
                    ctx.copy_text(text);
                }
                false
            }
            ClipboardChord::Cut => {
                self.copy_selected();
                let text = self.clipboard_text();
                if !text.is_empty() {
                    ctx.copy_text(text);
                }
                self.delete_selected()
            }
            ClipboardChord::Paste => self.paste_clipboard(),
            ClipboardChord::StyleCopy => {
                self.copy_style();
                false
            }
            ClipboardChord::StylePaste => self.paste_style(),
        }
    }

    pub fn toggle_collapse_action(&mut self) -> bool {
        self.toggle_selected_collapsed()
    }

    /// 多选的「折叠」：把每个还有孩子的选中节点都折起来，一条命令一次撤销。
    pub fn collapse_selected_action(&mut self) -> bool {
        let mut commands = Vec::new();
        for id in self.selected_ids() {
            if let Some(n) = self.session.doc.node(&id) {
                if !n.children.is_empty() && !n.collapsed {
                    commands.push(Command::SetCollapsed {
                        id,
                        collapsed: true,
                    });
                }
            }
        }
        if commands.is_empty() {
            return false;
        }
        if self
            .session
            .doc
            .apply(&Command::Batch { commands })
            .is_err()
        {
            return false;
        }
        self.rebuild();
        self.refocus_canvas = true;
        true
    }

    pub fn collapse_all_action(&mut self) -> bool {
        self.set_all_collapsed(true)
    }

    pub fn expand_all_action(&mut self) -> bool {
        self.set_all_collapsed(false)
    }

    /// 菜单入口：编辑主选文字（无主选时编辑根，与 F2 一致）。
    pub fn edit_primary_action(&mut self) -> bool {
        if self.editor.is_some() {
            return false;
        }
        let target = self
            .sel
            .primary()
            .or_else(|| self.scene.find(&self.session.doc.root_id));
        let Some(idx) = target else {
            return false;
        };
        let viewport = self.last_viewport;
        self.start_edit(idx, viewport);
        true
    }

    /// 菜单入口：100%。已在 100% 时不动，菜单可以据此显示为禁用。
    ///
    /// `Ctrl+0` 与缩放条上的百分比读数都走这里，**不再各写一遍 `camera.zoom = 1.0`**：
    /// 复位和缩放一样要留 `zoom_anchor src=reset`，否则「谁把倍率改了」在日志里断线。
    pub fn zoom_reset_action(&mut self) -> bool {
        if !zoom_reset(&mut self.camera) {
            return false;
        }
        let viewport = self.last_viewport;
        let at = Point::new(viewport[0] * 0.5, viewport[1] * 0.5);
        // 复位只改倍率、不动中心，所以视口中心那颗世界点恰好就是 `camera.center`。
        self.trace_zoom_anchor(ZoomSrc::Reset, at, self.camera.center, false);
        true
    }

    /// 多选的「移入同一父级」：以主选的父级为落点，把散落的选中根追加进去。
    pub fn move_same_parent_action(&mut self) -> bool {
        let Some(primary) = self.primary_id() else {
            return false;
        };
        let selected = self.selected_ids();
        let command = same_parent_command(&self.session.doc, &selected, &primary);
        self.apply_structural(command)
    }

    /// 选中项能不能加概要/边界：主选必须有父级（根没有「兄弟」可圈）。
    pub fn can_annotate_selection(&self) -> bool {
        self.primary_id()
            .and_then(|p| annotation_targets(&self.session.doc, &self.selected_ids(), &p))
            .is_some()
    }

    /// 加概要：覆盖与主选同一父级的那些选中项，文字先给一个可编辑的占位。
    pub fn add_summary_action(&mut self) -> bool {
        let Some(primary) = self.primary_id() else {
            return false;
        };
        let Some((parent, targets)) =
            annotation_targets(&self.session.doc, &self.selected_ids(), &primary)
        else {
            return false;
        };
        crate::trace::event(&format!(
            "annotation_add kind=summary targets={} parent={parent}",
            targets.len()
        ));
        self.apply_structural(Some(Command::AddSummary {
            parent,
            targets,
            text: "概要".into(),
        }))
    }

    /// 加边界：同一批选中项，颜色跟随主题默认。
    pub fn add_boundary_action(&mut self) -> bool {
        let Some(primary) = self.primary_id() else {
            return false;
        };
        let Some((parent, targets)) =
            annotation_targets(&self.session.doc, &self.selected_ids(), &primary)
        else {
            return false;
        };
        crate::trace::event(&format!(
            "annotation_add kind=boundary targets={} parent={parent}",
            targets.len()
        ));
        self.apply_structural(Some(Command::AddBoundary {
            parent,
            targets,
            color: None,
        }))
    }

    /// 主选能不能分离：根节点没有父级可脱离。
    pub fn can_detach_primary(&self) -> bool {
        self.primary_id()
            .and_then(|id| self.session.doc.node(&id).map(|n| n.parent_id.is_some()))
            .unwrap_or(false)
    }

    /// 把主选原地分离为自由主题（保留它自己的整棵子树与屏幕位置）。
    pub fn detach_primary_action(&mut self) -> bool {
        let Some(id) = self.primary_id() else {
            return false;
        };
        let Some(idx) = self.scene.find(&id) else {
            return false;
        };
        let rect = self.scene.node(idx).geom.rect;
        let command = Some(Command::DetachNode {
            id,
            x: rect.x,
            y: rect.y,
        });
        self.apply_structural(command)
    }

    /// M31：主选能不能提为根主题。单选的树内节点、且它不是根、且不是概念图。
    ///
    /// 自由主题（`parent_id` 为 `None`）也算树内节点——提根后它交出自由坐标，
    /// 下一帧由布局摆到中心，这正是用户按「设为根主题」想要的结果。
    pub fn can_set_root_primary(&self) -> bool {
        if self.is_concept_map() || self.selected_count() != 1 {
            return false;
        }
        self.primary_id()
            .map(|id| id != self.session.doc.root_id)
            .unwrap_or(false)
    }

    /// M31：把主选提为中心主题（旧根整棵树挂到它下面），一次撤销。
    pub fn set_root_primary(&mut self) -> bool {
        match self.primary_id() {
            Some(id) => self.set_root_for(&id),
            None => false,
        }
    }

    /// M31：把 `id` 提为中心主题。大纲行菜单直接给目标 id，不依赖画布主选。
    ///
    /// 提根后整张图的包围盒会变，这里顺手 `fit` 一次——否则新根按布局落在
    /// 视口外，用户会以为「点了没反应」（M6b 同一类问题）。
    pub fn set_root_for(&mut self, id: &str) -> bool {
        let applied = self.apply_structural(Some(Command::SetRoot { id: id.to_string() }));
        crate::trace::event(&format!("set-root id={id} ok={}", applied as u8));
        if applied {
            self.fit_view();
        }
        applied
    }

    /// 在世界坐标 `(x, y)` 新建自由主题（`(x, y)` 是节点盒子左上角）。
    pub fn add_floating_world(&mut self, x: f32, y: f32) -> bool {
        let Some(new_id) = self.insert_floating_world(x, y) else {
            return false;
        };
        // 新建自由主题和 Tab/Enter 一致：选中它并直接进入行内编辑。
        if let Some(i) = self.scene.find(&new_id) {
            self.select_node_only(i, "floating");
            self.start_edit(i, self.last_viewport);
            // X28 D2：新建 + 写名字合并成一条撤销，别让 Ctrl+Z 只撤字、留下空盒子。
            self.mark_editor_fresh();
        }
        true
    }

    /// 在概念 `after` 正下方一个层距处新建概念（大纲 Enter 用，CM-07）。
    ///
    /// 不选中、不进编辑：调用方是全屏大纲，它要自己把**行内编辑**立起来，
    /// 这里要是也 `start_edit`，两套编辑框会抢同一帧的 IME 焦点。
    pub fn add_concept_below(&mut self, after: &str) -> Option<String> {
        let (x, y) = self
            .session
            .doc
            .node(after)
            .and_then(|n| n.pos.as_ref())
            .map(|p| (p.x, p.y + 56.0))
            .unwrap_or((0.0, 0.0));
        self.insert_floating_world(x, y)
    }

    /// 大纲 `Tab`：在当前概念下新建概念**并连一条边**（CM-09，docs/30 §5.6.1）。
    ///
    /// 为什么不是「只加概念」：概念图的层级只由 `relationships` 表达，没有边就没有层级，
    /// 大纲里按 Tab 得到的会是一行**没有前缀的孤立概念**——用户看到的「加了个子级」
    /// 与文档里发生的「加了个孤儿」不一致。所以这里一次做两件事，且**一个 Batch**：
    /// 撤销一次就回到按 Tab 之前。
    ///
    /// 短语留空由调用方负责（大纲立刻立起短语编辑框；留空则删掉这条边）。
    pub fn link_concept_below(&mut self, from: &str) -> Option<String> {
        if !self.concept_map() {
            return None;
        }
        let (x, y) = self
            .session
            .doc
            .node(from)
            .and_then(|n| n.pos.as_ref())
            .map(|p| (p.x, p.y + 56.0))
            .unwrap_or((0.0, 0.0));
        // 一条原子命令：概念与边同生同灭，撤销一次就回到按 Tab 之前（CM-10 ①）。
        let before = self.session.doc.doc_order();
        let cmd = Command::AddLinkedConcept {
            from: from.to_string(),
            text: CONCEPT_PLACEHOLDER.into(),
            x,
            y,
        };
        if !self.apply_structural(Some(cmd)) {
            return None;
        }
        self.session
            .doc
            .doc_order()
            .into_iter()
            .find(|id| !before.contains(id))
    }

    /// 大纲 `Shift+Delete`：只断「进来那条边」，概念留成孤立概念（docs/30 §5.6.1）。
    pub fn unlink_concept(&mut self, from: &str, rel: &str) -> bool {
        if !self.concept_map() {
            return false;
        }
        let cmd = Command::DeleteRelationship {
            from: from.to_string(),
            id: rel.to_string(),
        };
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        self.rebuild();
        true
    }

    /// 大纲短语编辑提交：改这条边的链接短语。
    ///
    /// 空串只有思维导图能清掉短语；概念图会在模型层被拒（C16），调用方先按
    /// `docs/30` §5.6 的口径判一次并提示，不要指望这里返回 `false` 来说话。
    pub fn set_concept_phrase(&mut self, from: &str, rel: &str, label: &str) -> bool {
        let trimmed = label.trim();
        let cmd = Command::SetRelationshipLabel {
            from: from.to_string(),
            id: rel.to_string(),
            label: (!trimmed.is_empty()).then(|| trimmed.to_string()),
        };
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        self.rebuild();
        true
    }

    /// 新建自由主题/概念的共用落地点：返回新节点 id，不动选择与编辑态。
    pub(crate) fn insert_floating_world(&mut self, x: f32, y: f32) -> Option<String> {
        let before: HashSet<String> = self.session.doc.doc_order().into_iter().collect();
        // 占位文字按文档类型分开：概念是名词短语，「自由主题」是树的口径（CM-05）。
        let text = if self.concept_map() {
            CONCEPT_PLACEHOLDER
        } else {
            "自由主题"
        };
        let command = Some(Command::AddFloatingTopic {
            text: text.into(),
            x,
            y,
        });
        if !self.apply_structural(command) {
            return None;
        }
        self.session
            .doc
            .doc_order()
            .into_iter()
            .find(|id| !before.contains(id))
    }

    /// 在窗口坐标 `screen` 处新建自由主题：右键菜单与双击空白共用。
    pub fn add_floating_at(&mut self, screen: egui::Pos2) -> bool {
        if self.last_rect.width() < 1.0 {
            return false;
        }
        let local = Point::new(
            screen.x - self.last_rect.min.x,
            screen.y - self.last_rect.min.y,
        );
        let world = self.camera.screen_to_world(local, self.last_viewport);
        self.add_floating_world(world.x, world.y)
    }

    /// 给主选节点生成卡片（已有则返回已有），供快速捕获/右键菜单使用。
    pub fn capture_card_for_primary(&mut self) -> Option<String> {
        let node_id = self.primary_id()?;
        if let Some(card) = self.session.doc.card_for_node(&node_id) {
            return Some(card.id.clone());
        }
        let title = self
            .session
            .doc
            .node(&node_id)
            .map(|n| n.text.trim())
            .filter(|t| !t.is_empty())
            .unwrap_or("卡片笔记")
            .to_string();
        let card = NoteCard::new(self.session.doc.id.clone(), None, title, "", now_ms());
        let id = card.id.clone();
        let ok = self.apply_command(Command::AttachNewCard {
            node: node_id,
            card,
        });
        ok.then_some(id)
    }

    pub(crate) fn primary_id(&self) -> Option<String> {
        self.sel
            .primary()
            .and_then(|i| self.scene.ids.get(i as usize).cloned())
    }

    /// 复制选中节点（整棵子树）。多选时去掉「祖先也在选区」的重复子树。
    pub(crate) fn copy_selected(&mut self) {
        let selected = self.selected_ids();
        let roots = selected_roots(&self.session.doc, &selected);
        if self.concept_map() {
            // 概念图只复制命题网里的**独立概念**：没有子树、没有父子边，只保留内部关系。
            let nodes = concept_snapshot(&self.session.doc, &roots);
            if !nodes.is_empty() {
                self.clipboard = nodes;
            }
            return;
        }
        let nodes = copy_subtrees(&self.session.doc, &roots);
        if !nodes.is_empty() {
            self.clipboard = nodes;
        }
    }

    /// 概念图粘贴的原点：有主选概念就贴着它放，否则落在视口中心。
    fn concept_paste_origin(&self) -> (f32, f32) {
        if let Some(id) = self.primary_id() {
            if let Some(i) = self.scene.find(&id) {
                let r = self.scene.rect(i);
                return (r.x, r.y);
            }
        }
        let vp = self.last_viewport;
        let p = self
            .camera
            .screen_to_world(Point::new(vp[0] * 0.5, vp[1] * 0.5), vp);
        (p.x, p.y)
    }

    /// 概念图的粘贴/复制统一出口：独立概念、整体偏移、一次撤销、选中副本。
    fn paste_concepts(&mut self, offset: (f32, f32)) -> bool {
        let anchor = self.concept_paste_origin();
        let Some((command, tops)) = concept_clone_command(&self.clipboard, offset, anchor) else {
            return false;
        };
        if self.session.doc.apply(&command).is_err() {
            return false;
        }
        self.rebuild();
        let sel: Vec<u32> = tops.iter().filter_map(|id| self.scene.find(id)).collect();
        self.sel.replace(sel);
        self.refocus_canvas = true;
        crate::trace::event(&format!(
            "concept-copy n={} dx={:.1} dy={:.1}",
            tops.len(),
            offset.0,
            offset.1
        ));
        true
    }

    /// 复制到系统剪贴板的纯文本：节点文本按先序逐行排列，保留层级缩进。
    pub(crate) fn clipboard_text(&self) -> String {
        let mut depth: HashMap<&str, usize> = HashMap::new();
        let mut out = String::new();
        for node in &self.clipboard {
            let d = node
                .parent_id
                .as_deref()
                .and_then(|p| depth.get(p))
                .map(|d| d + 1)
                .unwrap_or(0);
            depth.insert(node.id.as_str(), d);
            let text = node.text.trim();
            if text.is_empty() {
                continue;
            }
            for _ in 0..d {
                out.push_str("  ");
            }
            out.push_str(text);
            out.push('\n');
        }
        out
    }

    /// 粘贴到主选节点之下（无主选时挂到根）。整批新 ID，一次撤销。
    pub fn clipboard_is_empty(&self) -> bool {
        self.clipboard.is_empty() || self.clipboard_text().trim().is_empty()
    }

    /// 纯文本粘贴：按缩进还原成树，挂到主选节点下；一次撤销。
    pub fn paste_plain_text(&mut self, text: &str) -> bool {
        let parent = self.primary_id();
        self.paste_plain_text_under(text, parent)
    }

    /// Ctrl+Shift+V：纯文本挂到根末尾。
    pub fn paste_plain_text_at_root(&mut self, text: &str) -> bool {
        let root = self.session.doc.root_id.clone();
        self.paste_plain_text_under(text, Some(root))
    }

    fn paste_plain_text_under(&mut self, text: &str, parent: Option<String>) -> bool {
        let parent = parent.unwrap_or_else(|| self.session.doc.root_id.clone());
        let Some(nodes) = parse_plain_outline(text, &parent) else {
            return false;
        };
        let tops: Vec<String> = nodes
            .iter()
            .filter(|n| n.parent_id.as_deref() == Some(parent.as_str()))
            .map(|n| n.id.clone())
            .collect();
        if self
            .session
            .doc
            .apply(&Command::InsertSubtree {
                parent,
                index: None,
                nodes,
            })
            .is_err()
        {
            return false;
        }
        self.rebuild();
        let sel: Vec<u32> = tops.iter().filter_map(|id| self.scene.find(id)).collect();
        self.sel.replace(sel);
        self.refocus_canvas = true;
        true
    }

    pub(crate) fn paste_clipboard(&mut self) -> bool {
        let parent = self.primary_id();
        self.paste_under(parent)
    }

    /// 粘贴到指定父级；`None` = 根。空白处右键的「粘贴」永远落到根，
    /// 不因为画布上恰好还留着上一步的选择就挂进某个节点。
    /// 粘贴到指定父级；`None` = 根。空白处右键的「粘贴」永远落到根，
    /// 不因为画布上恰好还留着上一步的选择就挂进某个节点。
    pub(crate) fn paste_under(&mut self, parent: Option<String>) -> bool {
        if self.clipboard.is_empty() {
            return false;
        }
        if self.concept_map() {
            // 概念图没有「粘贴到谁下面」：一律成为独立概念，避免把树语义带进来。
            return self.paste_concepts((32.0, 44.0));
        }
        let parent = parent.unwrap_or_else(|| self.session.doc.root_id.clone());
        let Some((command, tops)) = paste_command(&self.session.doc, &self.clipboard, &parent)
        else {
            return false;
        };
        if self.session.doc.apply(&command).is_err() {
            return false;
        }
        self.rebuild();
        let sel: Vec<u32> = tops.iter().filter_map(|id| self.scene.find(id)).collect();
        self.sel.replace(sel);
        self.refocus_canvas = true;
        true
    }

    /// Ctrl+D：原地复制，副本插在原节点之后；多根时按文档顺序逐个插入。
    pub(crate) fn duplicate_selected(&mut self) -> bool {
        let selected = self.selected_ids();
        let roots = selected_roots(&self.session.doc, &selected);
        if self.concept_map() {
            // 概念图：副本是独立概念，贴着原件偏移（32,44）放，整批一次撤销。
            let nodes = concept_snapshot(&self.session.doc, &roots);
            let anchor = self.concept_paste_origin();
            let Some((command, tops)) = concept_clone_command(&nodes, (32.0, 44.0), anchor) else {
                return false;
            };
            if self.session.doc.apply(&command).is_err() {
                return false;
            }
            self.rebuild();
            let sel: Vec<u32> = tops.iter().filter_map(|id| self.scene.find(id)).collect();
            self.sel.replace(sel);
            self.refocus_canvas = true;
            crate::trace::event(&format!("concept-copy n={} dx=32.0 dy=44.0", tops.len()));
            return true;
        }
        let Some((command, tops)) = duplicate_command(&self.session.doc, &roots) else {
            return false;
        };
        if self.session.doc.apply(&command).is_err() {
            return false;
        }
        self.rebuild();
        let sel: Vec<u32> = tops.iter().filter_map(|id| self.scene.find(id)).collect();
        self.sel.replace(sel);
        self.refocus_canvas = true;
        true
    }

    pub(crate) fn copy_style(&mut self) {
        if let Some(id) = self.primary_id() {
            self.style_clipboard = Some(self.session.doc.node(&id).and_then(|n| n.style.clone()));
        }
    }

    pub(crate) fn paste_style(&mut self) -> bool {
        let Some(style) = self.style_clipboard.clone() else {
            return false;
        };
        let ids = self.selected_ids();
        if ids.is_empty() {
            return false;
        }
        if self
            .session
            .doc
            .apply(&Command::SetStyle { ids, style })
            .is_err()
        {
            return false;
        }
        self.rebuild();
        self.refocus_canvas = true;
        true
    }

    /// 执行一条针对主选的层级命令（重排/升降级共用同一套应用与重建）。
    pub(crate) fn apply_structural(&mut self, command: Option<Command>) -> bool {
        let Some(command) = command else {
            return false;
        };
        if self.session.doc.apply(&command).is_err() {
            return false;
        }
        self.rebuild();
        self.refocus_canvas = true;
        true
    }

    /// 结构变化之后把主选拉回视口：连按十几次 `Alt+↓`，节点不该一路走出屏幕。
    /// 折叠、方向键遍历、新建节点早就会跟住，这几条层级命令以前是漏的（打磨轮 7）。
    pub(crate) fn reveal_id(&mut self, id: &str) {
        if let Some(i) = self.scene.find(id) {
            self.reveal_node(i);
        }
    }

    /// Alt+↑/↓：主选在兄弟中上移/下移一格；多选时整组移动一格。
    ///
    /// 多选走的是大纲与右键菜单同一条批量命令（组内顺序不翻、一次撤销），
    /// 以前这里只认主选，选了三颗按 `Alt+↓` 只动一颗，和菜单行为不一致。
    pub(crate) fn move_selected(&mut self, delta: i32) -> bool {
        let ids = self.selected_ids();
        if crate::trace::is_enabled() {
            crate::trace::event(&format!(
                "structure move={delta} selected={} path={}",
                ids.len(),
                if ids.len() > 1 { "batch" } else { "single" }
            ));
        }
        if ids.len() > 1 {
            let moved = self.move_step_many(&ids, delta);
            if !moved {
                // M6b：整组到边界时不能静默——键盘上按了没反应最容易被当成 bug。
                crate::trace::event(&format!(
                    "structure_move_blocked delta={delta} selected={} reason=batch",
                    ids.len()
                ));
                self.notice = Some(("选中的主题已经在边界，无法再移动".to_string(), false));
            }
            return moved;
        }
        let Some(id) = self.primary_id() else {
            return false;
        };
        let command = move_command(&self.session.doc, &id, delta);
        if command.is_none() {
            let reason = crate::canvas::move_block_reason(&self.session.doc, &id, delta);
            crate::trace::event(&format!(
                "structure_move_blocked delta={delta} selected=1 reason={reason:?}"
            ));
            self.notice = Some((
                match reason {
                    crate::canvas::MoveBlock::Root => "根主题不能移动",
                    crate::canvas::MoveBlock::First => "已经是第一个同级主题",
                    crate::canvas::MoveBlock::Last => "已经是最后一个同级主题",
                    crate::canvas::MoveBlock::None => "无法移动",
                }
                .to_string(),
                false,
            ));
            return false;
        }
        if !self.apply_structural(command) {
            return false;
        }
        self.reveal_id(&id);
        true
    }

    /// Ctrl+Shift+←：升级为父节点的同级（插在父节点之后）。根的孩子不能再升。
    pub(crate) fn promote_selected(&mut self) -> bool {
        let ids = self.selected_ids();
        if ids.len() > 1 {
            return self.outdent_many(&ids);
        }
        let Some(id) = self.primary_id() else {
            return false;
        };
        let command = promote_command(&self.session.doc, &id);
        if !self.apply_structural(command) {
            return false;
        }
        self.reveal_id(&id);
        true
    }

    /// Ctrl+Shift+→：降级为前一个兄弟的子节点（没有前兄弟时无处可降）。
    pub(crate) fn demote_selected(&mut self) -> bool {
        let ids = self.selected_ids();
        if ids.len() > 1 {
            return self.indent_many(&ids);
        }
        let Some(id) = self.primary_id() else {
            return false;
        };
        let command = demote_command(&self.session.doc, &id);
        if !self.apply_structural(command) {
            return false;
        }
        self.reveal_id(&id);
        true
    }

    pub(crate) fn toggle_selected_collapsed(&mut self) -> bool {
        let Some(id) = self.primary_id() else {
            return false;
        };
        let Some(node) = self.session.doc.node(&id) else {
            return false;
        };
        if node.children.is_empty() {
            return false;
        }
        let collapsed = node.collapsed;
        if self
            .session
            .doc
            .apply(&Command::SetCollapsed {
                id: id.clone(),
                collapsed: !collapsed,
            })
            .is_err()
        {
            return false;
        }
        self.rebuild();
        // 折叠后整张图会重排：刚折起的节点可能被别的分支挤出视口。
        // 用户按下的那一下以它为中心，视图就必须跟过去（W48）。
        if let Some(idx) = self.scene.find(&id) {
            self.reveal_node(idx);
        }
        self.refocus_canvas = true;
        true
    }

    pub(crate) fn set_all_collapsed(&mut self, collapsed: bool) -> bool {
        if self
            .session
            .doc
            .apply(&Command::SetCollapsedAll { collapsed })
            .is_err()
        {
            return false;
        }
        let primary = self.primary_id();
        self.rebuild();
        if let Some(idx) = primary.and_then(|id| self.scene.find(&id)) {
            self.reveal_node(idx);
        }
        self.refocus_canvas = true;
        true
    }

    /// Ctrl+2：把选中节点及其子树装进视口。
    pub(crate) fn fit_selection(&mut self) -> bool {
        let selected = self.sel.ids().to_vec();
        if selected.is_empty() {
            return false;
        }
        let mut bounds: Option<Rect> = None;
        for (i, node) in self.scene.nodes.iter().enumerate() {
            let idx = i as u32;
            if !selected.contains(&idx) && !has_selected_ancestor(&self.scene, idx, &selected) {
                continue;
            }
            bounds = Some(match bounds {
                Some(b) => b.union(node.geom.rect),
                None => node.geom.rect,
            });
        }
        let Some(b) = bounds else {
            return false;
        };
        let vp = self.last_viewport;
        self.fit_bounds(b, vp);
        self.fitted = true;
        true
    }

    /// 从面板折叠/展开。走命令层，撤销栈才不会断。
    pub fn toggle_collapsed(&mut self, id: &str) {
        let Some(n) = self.session.doc.node(id) else {
            return;
        };
        let collapsed = !n.collapsed;
        let cmd = Command::SetCollapsed {
            id: id.to_string(),
            collapsed,
        };
        if self.session.doc.apply(&cmd).is_ok() {
            self.rebuild();
            // 点和它自己有关的折叠圆点，视图不该飘走（W48）。
            if let Some(idx) = self.scene.find(id) {
                self.reveal_node(idx);
            }
            self.refocus_canvas = true;
        }
    }

    /// 存回当前路径。没有路径时返回 `Err`，由壳层去弹"另存为"。
    pub fn save(&mut self) -> Result<(), String> {
        self.session.save()
    }

    pub fn save_to(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.session.save_to(path)
    }

    /// `F6` 交出键盘：画布退出焦点环，`Tab` 才走得到检查器里的强调色色板。
    /// 面板的第一个控件由下一次 `Tab` 接手——多按一次，比把焦点猜到一个说不清的控件上强。
    /// `F6` 交出键盘：画布退出焦点环，`Tab` 才走得到检查器里的强调色色板。
    /// 面板的第一个控件由下一次 `Tab` 接手——多按一次，比把焦点猜到一个说不清的控件上强。
    pub fn hand_keys_to_panels(&mut self, ctx: &egui::Context) {
        self.wants_keys = false;
        self.refocus_canvas = false;
        ctx.memory_mut(|m| m.surrender_focus(canvas_id()));
        ctx.request_repaint();
    }

    /// `F6` 再按一次：键盘收回画布，裸键（Tab 加子节点、方向键选节点）重新生效。
    pub fn take_keys_back(&mut self, ctx: &egui::Context) {
        self.wants_keys = true;
        ctx.memory_mut(|m| m.request_focus(canvas_id()));
        ctx.request_repaint();
    }

    /// 键盘在面板手里（状态栏要说清楚，否则 `Tab` 看起来像失灵）。
    /// 这也是 `F6` 唯一的判断依据：只看我们自己的模式位，不看 egui 的记忆层——
    /// 焦点会被别处（点面板、`Esc`）抢走，按实时焦点判断会让同一个键走进不同分支，
    /// 状态栏指示与实际行为各说各话。
    /// 键盘在面板手里（状态栏要说清楚，否则 `Tab` 看起来像失灵）。
    /// 这也是 `F6` 唯一的判断依据：只看我们自己的模式位，不看 egui 的记忆层——
    /// 焦点会被别处（点面板、`Esc`）抢走，按实时焦点判断会让同一个键走进不同分支，
    /// 状态栏指示与实际行为各说各话。
    pub fn keys_in_panels(&self) -> bool {
        !self.wants_keys
    }
}
