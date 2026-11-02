use super::*;

impl Shell {
    /// 无框窗口的 8 条缩发热区；只负责触发系统的 `BeginResize`，缩放本身交给 OS。
    pub(crate) fn resize_edges(ui: &mut egui::Ui, win: Rect, ctx: &egui::Context) {
        let (l, t) = (win.min.x, win.min.y);
        let (r, b) = (win.max.x, win.max.y);

        let zones: [(Rect, egui::ResizeDirection); 8] = [
            (
                Rect::from_min_max(Pos2::new(l + CORNER, t), Pos2::new(r - CORNER, t + EDGE)),
                egui::ResizeDirection::North,
            ),
            (
                Rect::from_min_max(Pos2::new(l + CORNER, b - EDGE), Pos2::new(r - CORNER, b)),
                egui::ResizeDirection::South,
            ),
            (
                Rect::from_min_max(Pos2::new(l, t + CORNER), Pos2::new(l + EDGE, b - CORNER)),
                egui::ResizeDirection::West,
            ),
            (
                Rect::from_min_max(Pos2::new(r - EDGE, t + CORNER), Pos2::new(r, b - CORNER)),
                egui::ResizeDirection::East,
            ),
            (
                Rect::from_min_max(Pos2::new(l, t), Pos2::new(l + CORNER, t + CORNER)),
                egui::ResizeDirection::NorthWest,
            ),
            (
                Rect::from_min_max(Pos2::new(r - CORNER, t), Pos2::new(r, t + CORNER)),
                egui::ResizeDirection::NorthEast,
            ),
            (
                Rect::from_min_max(Pos2::new(l, b - CORNER), Pos2::new(l + CORNER, b)),
                egui::ResizeDirection::SouthWest,
            ),
            (
                Rect::from_min_max(Pos2::new(r - CORNER, b - CORNER), Pos2::new(r, b)),
                egui::ResizeDirection::SouthEast,
            ),
        ];

        // 显式基 id：热区依附的覆盖层随「是否最大化」出现/消失，
        // 若用自动 salt 派生，id 会随控件顺序漂移，导致按下状态串位。
        let root = egui::Id::unique("mw-resize-overlay");
        for (rect, dir) in zones {
            // `Sense::DRAG` 而不是 `Sense::drag()`：前者不可聚焦。隐形热区一旦进了
            // Tab 序列，用户按 Tab 焦点就掉进一个看不见的控件里，快捷键全废。
            let resp = ui.interact(rect, root.with(dir as u8), egui::Sense::DRAG);
            if resp.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        }
    }

    /// 正文占位：M0 只要证明「Mica 底 + 原生文本 + 无框内容区」三者共存不撕裂。
    /// `live = false` 是快照场景：视口尺寸、焦点、最大化都随窗口状态漂移，不进基线。
    /// 正文占位：M0 只要证明「Mica 底 + 原生文本 + 无框内容区」三者共存不撕裂。
    /// `live = false` 是快照场景：视口尺寸、焦点、最大化都随窗口状态漂移，不进基线。
    pub(crate) fn body(ui: &mut egui::Ui, ctx: &egui::Context, backdrop: &Backdrop, live: bool) {
        let ppp = ctx.pixels_per_point();
        let scale = format!(
            "pixels_per_point = {ppp:.2}（{}%）",
            (ppp * 100.0).round() as i32
        );

        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.label(
                egui::RichText::new("织思 · M0 壳层骨架")
                    .heading()
                    .size(Type::DISPLAY),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Mica 材质 · 合并标题栏 · 自绘窗口按钮 · 原生拖拽与缩放")
                    .size(Type::LABEL)
                    .color(Color32::from_gray(110)),
            );
            ui.add_space(28.0);
            if live {
                // 注意：这是**本视口**的 rect，不是显示器尺寸。
                let viewport = ctx.input(|i| i.raw.screen_rect.map(|r| r.size()));
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
                ui.label(format!(
                    "{scale}   视口 = {:?}   最大化 = {maximized}   焦点 = {focused}",
                    viewport.map(|s| (s.x as i32, s.y as i32))
                ));
            } else {
                ui.label(scale);
            }
            ui.label(match backdrop {
                Backdrop::Pending => "材质：等待窗口句柄…".into(),
                Backdrop::Applied { corner } => format!("材质：Mica 已应用   圆角：{corner:?}"),
                Backdrop::Failed(e) => format!("材质：失败 —— {e}"),
            });
            ui.add_space(20.0);
            ui.label("拖动标题栏到屏幕顶部可触发 Win11 快照布局；边缘可缩放；双击标题栏最大化。");
        });
    }
}
