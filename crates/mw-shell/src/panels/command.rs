//! 命令面板（`Ctrl+K`）。
//!
//! 没有菜单栏的产品必须有一块"什么都能找到"的地方。这一块的取舍：
//! * **纯本地过滤**：命令表是编译期常量，没有搜索服务、没有索引、没有分配（除了结果 Vec）；
//! * **键盘优先**：`↑↓` 移动、`Enter` 执行、`Esc` 关闭，鼠标只是补充；
//! * **不引入新依赖**：模糊匹配用子串（中文没有大小写，够用），不上 fancy 算法。
//!
//! 输入处理放在 `show()` 里、绘制走 `Order::Foreground` 的 `Area`——这样即使本函数
//! 在 `ui()` 里被很早调用，画面仍在最上层，而 `Esc`/`↑↓`/`Enter` 会先被这里消费掉，
//! 不会漏到画布的快捷键里。

use eframe::egui::{self, Align2, FontId, Id, Key, Modifiers, Pos2, Sense, Vec2};

use crate::chrome::{Tone, Type};
use crate::shortcuts;

/// 面板能发起的动作。壳层负责执行——**面板不碰文档**。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    New,
    Open,
    Import,
    /// 打开本地卡片笔记库。
    Notes,
    /// 给当前选中主题一键生成卡片。
    CaptureCard,
    /// 打开今天的每日笔记。
    DailyNote,
    /// 切换网格。
    ToggleGrid,
    /// 切换标尺。
    ToggleRulers,
    /// 开始一次本地主动回忆。
    Review,
    /// 打开卡片/标签/主题的关系图。
    Knowledge,
    /// 打开本地学习统计。
    Stats,
    /// 导出导图 + 卡片合集（Markdown + 可打印 HTML）。
    Bundle,
    /// 打开恢复中心。
    Recovery,
    /// 打开设置中心。
    Settings,
    /// 打开主题编辑器。
    ThemeStudio,
    /// 打开版本历史。
    History,
    Save,
    SaveAs,
    Export,
    Find,
    Undo,
    Redo,
    Delete,
    FitView,
    ZoomIn,
    ZoomOut,
    ToggleOutline,
    ToggleInspector,
    /// 右下角缩略卡（小地图，M30）。
    ToggleMinimap,
    /// 打开快捷键设置面板。
    Preferences,
    /// 切换文档主题（配色）。携带 `&'static str` 的主题 id，执行侧走检查器同一条
    /// `Command::SetTheme`——**面板仍然不碰文档**。
    SetTheme(&'static str),
}

struct Item {
    label: &'static str,
    hint: &'static str,
    action: Action,
}

const ITEMS: &[Item] = &[
    Item {
        label: "新建文档",
        hint: "Ctrl+N",
        action: Action::New,
    },
    Item {
        label: "打开…",
        hint: "Ctrl+O",
        action: Action::Open,
    },
    Item {
        label: "导入 Markdown / OPML / FreeMind / XMind…",
        hint: "",
        action: Action::Import,
    },
    Item {
        label: "笔记库",
        hint: "所有卡片 / 标签 / 搜索",
        action: Action::Notes,
    },
    Item {
        label: "为选中主题新建卡片",
        hint: "",
        action: Action::CaptureCard,
    },
    Item {
        label: "今日笔记",
        hint: "同一天只建一张",
        action: Action::DailyNote,
    },
    Item {
        label: "显示/隐藏网格",
        hint: "默认关",
        action: Action::ToggleGrid,
    },
    Item {
        label: "显示/隐藏标尺",
        hint: "默认关",
        action: Action::ToggleRulers,
    },
    Item {
        label: "版本历史",
        hint: "本地快照 · 可回滚",
        action: Action::History,
    },
    Item {
        label: "主题编辑器",
        hint: "微调 · 实时预览 · 预设",
        action: Action::ThemeStudio,
    },
    Item {
        label: "设置",
        hint: "外观 · 备份 · 导出 · 笔记库",
        action: Action::Settings,
    },
    Item {
        label: "恢复中心",
        hint: "崩溃恢复点",
        action: Action::Recovery,
    },
    Item {
        label: "导出学习合集",
        hint: "导图 + 卡片 · Markdown/HTML",
        action: Action::Bundle,
    },
    Item {
        label: "学习统计",
        hint: "卡片 · 复习 · 连续天数",
        action: Action::Stats,
    },
    Item {
        label: "知识视图",
        hint: "卡片 · 标签 · 主题",
        action: Action::Knowledge,
    },
    Item {
        label: "主动回忆复习",
        hint: "本地间隔重复",
        action: Action::Review,
    },
    Item {
        label: "保存",
        hint: "Ctrl+S",
        action: Action::Save,
    },
    Item {
        label: "另存为…",
        hint: "Ctrl+Shift+S",
        action: Action::SaveAs,
    },
    Item {
        label: "快捷键设置…",
        hint: "Ctrl+,",
        action: Action::Preferences,
    },
    Item {
        label: "导出…",
        hint: "Ctrl+E",
        action: Action::Export,
    },
    Item {
        label: "查找主题…",
        hint: "Ctrl+F",
        action: Action::Find,
    },
    Item {
        label: "撤销",
        hint: "Ctrl+Z",
        action: Action::Undo,
    },
    Item {
        label: "重做",
        hint: "Ctrl+Y",
        action: Action::Redo,
    },
    Item {
        label: "删除选中",
        hint: "Delete",
        action: Action::Delete,
    },
    Item {
        label: "适应画布",
        hint: "Home",
        action: Action::FitView,
    },
    Item {
        label: "放大",
        hint: "Ctrl+=",
        action: Action::ZoomIn,
    },
    Item {
        label: "缩小",
        hint: "Ctrl+-",
        action: Action::ZoomOut,
    },
    // 主题（配色）此前只有检查器宫格一个入口，命令面板与菜单都摸不到——这是配色线上
    // 最大的可达性缺口。四条独立命令而不是「打开主题选择器」：面板不该再叠一层浮层，
    // 且「主题：墨夜」这种写法本身就能被中文子串搜到。
    Item {
        label: "主题：纸感",
        hint: "浅色",
        action: Action::SetTheme("snow"),
    },
    Item {
        label: "主题：墨夜",
        hint: "深色",
        action: Action::SetTheme("ink"),
    },
    Item {
        label: "主题：雾青",
        hint: "浅色",
        action: Action::SetTheme("mist"),
    },
    Item {
        label: "主题：暖砂",
        hint: "浅色",
        action: Action::SetTheme("sand"),
    },
    Item {
        label: "显示/隐藏大纲",
        hint: "F8",
        action: Action::ToggleOutline,
    },
    Item {
        label: "显示/隐藏检查器",
        hint: "F9",
        action: Action::ToggleInspector,
    },
    Item {
        label: "显示/隐藏小地图",
        hint: "右下角缩略导航",
        action: Action::ToggleMinimap,
    },
];

const ROW_H: f32 = 32.0;
const WIDTH: f32 = 460.0;
const MAX_ROWS: usize = 8;

#[derive(Default)]
pub struct Palette {
    open: bool,
    query: String,
    cursor: usize,
    /// 可见窗口的首行下标。命令比一屏多时必须能滚到第 9 条以后，否则「主题：…」
    /// 这类靠后的命令只能用键盘打字才能找到。
    top: usize,
    /// 下一帧把焦点交给输入框（刚打开时用）。
    grab: bool,
}

impl Palette {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.cursor = 0;
        self.top = 0;
        self.grab = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.cursor = 0;
        self.top = 0;
    }

    /// 画面板 + 读键盘。返回本帧要执行的动作。
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        tone: Tone,
        bindings: &shortcuts::Bindings,
        keyboard: bool,
    ) -> Option<Action> {
        if !self.open {
            return None;
        }
        let hits = filter(&self.query);
        let n = hits.len();
        if self.cursor >= n {
            self.cursor = n.saturating_sub(1);
        }
        let before = self.query.clone();
        let top = self.top.min(n.saturating_sub(1));

        let screen = ctx.input(|i| i.content_rect());
        let pos = Pos2::new(screen.center().x - WIDTH * 0.5, screen.top() + 96.0);
        let mut action = None;
        let mut clicked_row = None;

        egui::Area::new(Id::unique("mw-command-palette"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let frame = egui::Frame::NONE
                    .fill(tone.card)
                    .stroke(egui::Stroke::new(1.0, tone.stroke))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(10))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 10],
                        blur: 28,
                        spread: 0,
                        color: tone.shadow,
                    });
                frame.show(ui, |ui| {
                    ui.set_width(WIDTH - 20.0);
                    let edit = egui::TextEdit::singleline(&mut self.query)
                        .font(FontId::proportional(Type::HEADING))
                        .hint_text(
                            egui::RichText::new("输入命令…（↑↓ 选择，Enter 执行）")
                                .size(Type::LABEL)
                                .color(tone.muted),
                        )
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(6, 5));
                    let resp = ui.add(edit);
                    if self.grab {
                        resp.request_focus();
                        self.grab = false;
                    }
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(2.0);

                    if hits.is_empty() {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("没有匹配的命令")
                                .size(Type::BODY)
                                .color(tone.ch.dim),
                        );
                        ui.add_space(8.0);
                        return;
                    }

                    for (i, item) in hits[top..(top + MAX_ROWS).min(n)].iter().enumerate() {
                        let idx = top + i;
                        let width = ui.available_width();
                        let (rect, row) =
                            ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::click());
                        let chosen = idx == self.cursor;
                        let painter = ui.painter_at(rect);
                        if chosen {
                            painter.rect_filled(rect, 6.0, tone.current);
                        } else if row.hovered() {
                            painter.rect_filled(rect, 6.0, tone.hover);
                        }
                        painter.text(
                            Pos2::new(rect.left() + 10.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            item.label,
                            FontId::proportional(Type::LABEL),
                            if chosen { tone.hit } else { tone.ch.text },
                        );
                        // 快捷键提示跟随用户改键；主题行这类没有快捷键的仍用表里的说明。
                        let hint = shortcut_of(item.action)
                            .map(|a| bindings.hint(a))
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| item.hint.to_string());
                        painter.text(
                            Pos2::new(rect.right() - 10.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            &hint,
                            FontId::proportional(Type::SMALL),
                            tone.ch.dim,
                        );
                        if row.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if row.clicked() {
                            clicked_row = Some(idx);
                        }
                    }
                });
            });

        // 键盘：先消费再判断，避免同一帧漏到画布。弹窗在上时整块跳过，
        // 否则回车会在确认框背后悄悄执行一条命令。
        if keyboard {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.close();
                return None;
            }
            // 查询变过：选择回到第一条。光标停在一个"上一帧才存在"的行上，比跳回顶部更让人
            // 找不到自己在哪。
            if self.query != before {
                self.cursor = 0;
            }
            if n > 0 && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                self.cursor = (self.cursor + 1) % n;
            }
            if n > 0 && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                self.cursor = (self.cursor + n - 1) % n;
            }
            self.top = window_top(self.cursor, self.top, n, MAX_ROWS);
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                if let Some(item) = hits.get(self.cursor) {
                    action = Some(item.action);
                }
                self.close();
            }
        }
        if let Some(i) = clicked_row {
            if let Some(item) = hits.get(i) {
                action = Some(item.action);
            }
            self.close();
        }
        action
    }
}

/// 让选中行留在可见窗口里：只在选中跑出窗口时平移整窗，不做居中——居中会让长列表
/// 一跳一跳，也让人找不到刚才选到了哪。
fn window_top(cursor: usize, top: usize, hits: usize, cap: usize) -> usize {
    if hits == 0 || cap == 0 {
        return 0;
    }
    let cap = cap.min(hits);
    let mut t = top.min(hits - cap);
    if cursor >= t + cap {
        t = cursor + 1 - cap;
    } else if cursor < t {
        t = cursor;
    }
    t
}

/// 子串匹配（不区分大小写）。空查询返回全部。
fn filter(query: &str) -> Vec<&'static Item> {
    let q = query.trim().to_lowercase();
    ITEMS
        .iter()
        .filter(|it| {
            q.is_empty()
                || it.label.to_lowercase().contains(&q)
                || it.hint.to_lowercase().contains(&q)
        })
        .collect()
}

/// 测试用：匹配到的命令名。
#[cfg(test)]
pub fn matching_labels(query: &str) -> Vec<&'static str> {
    filter(query).into_iter().map(|i| i.label).collect()
}

/// 测试用：匹配到的命令动作。
/// 面板动作 → 窗口级快捷键动作：只有这些行显示可配置的提示。
fn shortcut_of(action: Action) -> Option<shortcuts::Action> {
    use shortcuts::Action as S;
    Some(match action {
        Action::New => S::New,
        Action::Open => S::Open,
        Action::Save => S::Save,
        Action::SaveAs => S::SaveAs,
        Action::Export => S::Export,
        Action::Find => S::Find,
        Action::ToggleOutline => S::Outline,
        Action::ToggleInspector => S::Inspector,
        Action::Preferences => S::Preferences,
        _ => return None,
    })
}

#[cfg(test)]
pub fn matching_actions(query: &str) -> Vec<Action> {
    filter(query).into_iter().map(|i| i.action).collect()
}

#[cfg(test)]
mod tests {
    use super::{matching_actions, matching_labels, Action, Palette};

    /// 面板提示必须跟着用户改键走，而不是永远印出厂值。
    #[test]
    fn shortcut_rows_show_the_configured_chord() {
        let mut bindings = crate::shortcuts::Bindings::defaults();
        bindings
            .set(
                crate::shortcuts::Action::Save,
                crate::shortcuts::Chord::parse("Ctrl+Alt+S").unwrap(),
            )
            .unwrap();
        let item = super::ITEMS
            .iter()
            .find(|i| i.action == Action::Save)
            .expect("面板里必须有「保存」");
        let hint = super::shortcut_of(item.action)
            .map(|a| bindings.hint(a))
            .expect("保存必须有快捷键映射");
        assert_eq!(hint, "Ctrl+Alt+S");
        // 主题行不挂快捷键，提示仍用表里的「浅色/深色」
        assert!(super::shortcut_of(Action::SetTheme("snow")).is_none());
        assert!(super::shortcut_of(Action::Import).is_none());
    }

    #[test]
    fn an_empty_query_lists_everything() {
        assert_eq!(matching_labels("").len(), super::ITEMS.len());
    }

    #[test]
    fn chinese_substrings_match() {
        // 「保存」是「另存为…」的子串吗？不是——这正是这条断言存在的价值：
        // 用户打「存」才该同时看到两条。
        assert_eq!(matching_labels("保存"), vec!["保存"]);
        let hit = matching_labels("存");
        assert!(hit.contains(&"保存"), "{hit:?}");
        assert!(hit.contains(&"另存为…"), "{hit:?}");
        assert!(!hit.contains(&"撤销"));
    }

    #[test]
    fn the_shortcut_is_searchable_too() {
        let hit = matching_labels("ctrl+z");
        assert_eq!(hit, vec!["撤销"]);
    }

    /// 配色可达性：每套主题都要能只打「主题：」+ 名字就直达，且各自只命中一条。
    #[test]
    fn every_theme_is_one_keystroke_away() {
        for theme in mw_core::theme::themes() {
            assert_eq!(
                matching_actions(&format!("主题：{}", theme.name)),
                vec![Action::SetTheme(theme.id)],
                "「{}」应当只命中它自己那一条主题命令",
                theme.name
            );
        }
    }

    /// 提示列写「浅色 / 深色」就得和 `Appearance` 对得上——它同时还是搜索词。
    #[test]
    fn theme_hints_state_the_right_appearance() {
        for theme in mw_core::theme::themes() {
            let want = if matches!(theme.appearance, mw_core::theme::Appearance::Dark) {
                "深色"
            } else {
                "浅色"
            };
            let Some(item) = super::ITEMS
                .iter()
                .find(|it| it.action == Action::SetTheme(theme.id))
            else {
                panic!("命令面板里没有主题「{}」", theme.name);
            };
            assert_eq!(item.hint, want, "「{}」的外观提示不对", theme.name);
        }
    }

    #[test]
    fn the_appearance_words_also_find_themes() {
        assert_eq!(
            matching_labels("深色"),
            vec!["主题：墨夜"],
            // 浅色主题不该被这条命中
            "提示列必须能当搜索词用"
        );
    }

    /// 命令比一屏多时光标必须能走到第 9 条以后，且窗口只做最小平移。
    #[test]
    fn the_selection_scrolls_into_view() {
        // 19 条、窗口 8 行：选中下标 8（第 9 条）时窗口必须推到 1。
        assert_eq!(super::window_top(8, 0, 19, 8), 1);
        // 已经在窗口里就不动——否则连按方向键会一跳一跳。
        assert_eq!(super::window_top(8, 1, 19, 8), 1);
        // 从第 0 条按「上」环绕到最后一条：窗口贴到末尾。
        assert_eq!(super::window_top(18, 0, 19, 8), 11);
        // 往上越过上沿时整窗下移。
        assert_eq!(super::window_top(5, 8, 19, 8), 5);
        // 结果不足一屏：永远贴顶，查询变窄后窗口也不能越界。
        assert_eq!(super::window_top(2, 0, 3, 8), 0);
        assert_eq!(super::window_top(1, 12, 5, 8), 0);
        assert_eq!(super::window_top(0, 0, 0, 8), 0);
    }

    #[test]
    fn a_query_with_no_hits_returns_empty() {
        assert!(matching_labels("量子纠缠").is_empty());
    }

    #[test]
    fn open_and_close_reset_the_query() {
        let mut p = Palette::new();
        assert!(!p.is_open());
        p.open();
        p.query = "保存".into();
        p.close();
        assert!(!p.is_open());
        assert!(p.query.is_empty(), "关掉之后查询词不该留着");
    }
}
