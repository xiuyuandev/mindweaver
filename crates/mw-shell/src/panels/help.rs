//! 快捷键速查浮层（F1）。
//!
//! 帮助走**浮层而不是模态对话框**：查键位时通常想边看边试，关掉浮层不该丢掉选择。
//! 表里只列已经实现的键；文档里的规划项在实现前不许先写进来——这张表是承诺。

use eframe::egui::{self, FontFamily, Id, Key, Modifiers, Pos2, RichText, Vec2};

use crate::chrome::{Tone, Type};

const WIDTH: f32 = 640.0;
/// 大纲键位表三栏，比总表宽一档；仍留出屏幕两侧的呼吸。
const OUTLINE_WIDTH: f32 = 820.0;

const GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "文件",
        &[
            ("Ctrl+N", "新建"),
            ("Ctrl+O", "打开"),
            ("Ctrl+S", "保存"),
            ("Ctrl+Shift+S", "另存为"),
            ("Ctrl+W", "关闭标签"),
            ("Ctrl+E", "导出"),
            ("Ctrl+P", "导出 PDF"),
        ],
    ),
    (
        "编辑",
        &[
            ("Ctrl+Z / Y", "撤销 / 重做"),
            ("Ctrl+C / X / V", "复制 / 剪切 / 粘贴"),
            ("Ctrl+D", "复制节点"),
            ("Ctrl+F", "查找（支持拼音）"),
            ("Ctrl+K", "命令面板"),
            ("F2", "编辑文字"),
        ],
    ),
    (
        "节点",
        &[
            ("Tab", "子主题"),
            ("Enter", "同级主题"),
            ("Delete", "删除子树"),
            ("Alt+↑ / ↓", "同级上移 / 下移"),
            ("Ctrl+Shift+← / →", "升级 / 降级"),
            ("Ctrl+Shift+C / V", "复制 / 粘贴样式"),
        ],
    ),
    (
        "鼠标",
        &[
            ("拖拽主题", "移动 / 改父级"),
            ("Ctrl+拖拽", "复制整棵子树"),
            ("拖到空白", "分离为自由主题"),
            ("双击空白", "新建自由主题"),
            ("双击主题", "编辑文字"),
        ],
    ),
    (
        "层级与视图",
        &[
            ("/", "折叠 / 展开"),
            ("Ctrl+/", "全部折叠"),
            ("Ctrl+Shift+/", "全部展开"),
            ("Alt+Enter", "聚焦当前子树"),
            ("Esc", "退回上一级 / 取消选择"),
            ("Ctrl+0 / 1 / 2", "100% / 适应画布 / 适应选中"),
        ],
    ),
    (
        "面板与窗口",
        &[
            ("F8 / F9", "大纲 / 检查器"),
            ("F6", "键盘在画布与面板之间交接"),
            ("F11", "全屏"),
            ("Ctrl+滚轮 / ±", "缩放"),
            ("Ctrl+Tab", "切换标签"),
            ("Alt+1…9", "跳到第 n 个标签"),
            ("F1", "本浮层"),
            ("Ctrl+Comma", "快捷键设置（可改键）"),
        ],
    ),
];

/// 大纲专用的三栏键位表：导航 / 编辑 / 结构。只列已经实现的键——
/// 这张表是承诺，O03/O04 做到哪一步再往这里补。
const OUTLINE_GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "导航",
        &[
            ("↑ / ↓", "上一行 / 下一行"),
            ("←", "折叠当前行 / 回到父级"),
            ("→", "展开当前行 / 进入首个子级"),
            ("Space", "折叠 / 展开"),
            ("Alt+Enter", "聚焦当前子树"),
            ("Esc", "清搜索 / 退聚焦 / 退出"),
            ("Ctrl+Shift+F", "搜索大纲"),
            ("Tab", "从搜索框回到行表"),
        ],
    ),
    (
        "编辑",
        &[
            ("Enter", "新建同级"),
            ("Ctrl+] / Ctrl+[", "进入 / 退出聚焦"),
            ("Alt+.", "折叠 / 展开"),
            ("Tab / Shift+Tab", "降级 / 升级"),
            ("F2 / 双击", "行内改名"),
            ("Delete", "删除子树"),
            ("Ctrl+Shift+↑ / ↓", "同级上移 / 下移"),
            ("Ctrl+T", "切换任务状态"),
            ("Ctrl+Z / Y", "撤销 / 重做"),
        ],
    ),
    (
        "结构",
        &[
            ("Shift / Ctrl+单击", "加选 / 减选"),
            ("拖到行间", "同级重排"),
            ("拖到行中", "成为子级"),
            ("页头折叠菜单", "展开全部 / 折叠全部 / 显示层级"),
            ("页头密度菜单", "紧凑 / 标准 / 宽松"),
            ("F8", "回到画布"),
            ("? / F1", "本表"),
        ],
    ),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Section {
    #[default]
    Global,
    Outline,
}

#[derive(Default)]
pub struct Help {
    open: bool,
    section: Section,
}

impl Help {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.section = Section::Global;
        }
    }

    /// F1 / `?` 在大纲里：第一次打开大纲表，已经开着就关上。
    pub fn toggle_outline(&mut self) {
        if self.open && self.section == Section::Outline {
            self.close();
        } else {
            self.open = true;
            self.section = Section::Outline;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Esc 关闭。返回是否消费了这次按键（壳层据此刷新输入门禁）。
    pub fn handle_keys(&mut self, ctx: &egui::Context) -> bool {
        if !self.open {
            return false;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.close();
            return true;
        }
        false
    }

    pub fn show(&mut self, ctx: &egui::Context, tone: Tone) {
        if !self.open {
            return;
        }
        let screen = ctx.input(|i| i.content_rect());
        let (width, groups, cols, title, note) = match self.section {
            Section::Global => (
                WIDTH,
                GROUPS,
                2,
                "快捷键",
                "macOS 之外的所有组合键都用 Ctrl；编辑文字时快捷键让位给输入法。",
            ),
            Section::Outline => (
                OUTLINE_WIDTH,
                OUTLINE_GROUPS,
                3,
                "大纲快捷键",
                "根主题始终展开；编辑文字时快捷键让位给输入法。",
            ),
        };
        let width = width.min((screen.width() - 24.0).max(360.0));
        let pos = Pos2::new(
            (screen.center().x - width * 0.5).max(12.0),
            screen.top() + 64.0,
        );
        let mut close = false;
        egui::Area::new(Id::unique("mw-shortcut-help"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let frame = egui::Frame::NONE
                    .fill(tone.card)
                    .stroke(egui::Stroke::new(1.0, tone.stroke))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(16))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 12],
                        blur: 32,
                        spread: 0,
                        color: tone.shadow,
                    });
                frame.show(ui, |ui| {
                    ui.set_width(width - 32.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(title)
                                .size(Type::HEADING)
                                .family(FontFamily::Name(crate::fonts::SEMIBOLD.into()))
                                .color(tone.ink),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .selectable_label(
                                    false,
                                    RichText::new("关闭 · Esc").size(Type::BODY),
                                )
                                .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(10.0);
                    egui::ScrollArea::vertical()
                        .max_height((screen.height() - 180.0).max(220.0))
                        .show(ui, |ui| {
                            let per = groups.len().div_ceil(cols);
                            ui.columns(cols, |cols_ui| {
                                for (side, chunk) in
                                    groups.chunks(per).enumerate().take(cols_ui.len())
                                {
                                    let ui = &mut cols_ui[side];
                                    for (group, rows) in chunk {
                                        ui.add_space(2.0);
                                        ui.label(
                                            RichText::new(*group)
                                                .size(Type::SMALL)
                                                .family(FontFamily::Name(
                                                    crate::fonts::SEMIBOLD.into(),
                                                ))
                                                .color(tone.ch.dim),
                                        );
                                        ui.add_space(3.0);
                                        egui::Grid::new(format!("mw-help-{side}-{group}"))
                                            .num_columns(2)
                                            .spacing(Vec2::new(12.0, 5.0))
                                            .show(ui, |ui| {
                                                for (key, desc) in *rows {
                                                    key_badge(ui, key, tone);
                                                    ui.label(
                                                        RichText::new(*desc)
                                                            .size(Type::BODY)
                                                            .color(tone.ch.text),
                                                    );
                                                    ui.end_row();
                                                }
                                            });
                                        ui.add_space(10.0);
                                    }
                                }
                            });
                        });
                    ui.add_space(6.0);
                    ui.label(RichText::new(note).size(Type::CAPTION).color(tone.ch.dim));
                });
            });
        if close {
            self.close();
        }
    }
}

fn key_badge(ui: &mut egui::Ui, key: &str, tone: Tone) {
    egui::Frame::NONE
        .fill(tone.ch.panel2)
        .stroke(egui::Stroke::new(1.0, tone.ch.border_hot))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(key)
                    .monospace()
                    .size(Type::CAPTION)
                    .color(tone.ch.text),
            );
        });
}
