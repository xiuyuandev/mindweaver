//! 开始页。
//!
//! 五段式：品牌头、快速开始、最近文件、模板、底部提示条。回访用户的头号动作是
//! 接着上次工作，所以最近文件排在模板之前。页面只发 [`Action`]，真正的新建/
//! 打开由壳层落地——和面板同一套纪律：**开始页不直接改文档**。
//!
//! 所有图形都是矢量：徽标、模板缩略图、文件角标都不依赖字符字形，
//! 125%/150% 缩放下不会糊或跳半像素。
//!
//! 断点只有 [`layout`] 一处：页面宽、快速开始横排、模板列数、收短副标题、
//! 底部键帽、模板区提示都由它决定，绘制函数不再自己比较宽度。

use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use mw_core::{design, Command, Document, LayoutSettings, Node};

use crate::chrome::{Tone, Type};
use crate::icons::Icon;
use crate::recent;
use crate::session::Session;
use crate::ui_kit::{self, ButtonTone};
use crate::{icon_paint, trace};

const MAX_W: f32 = 880.0;
/// 版心两侧各留这么多（`docs/30` §3.2：两侧 ≥ 32）。
const PAGE_PAD: f32 = 32.0;
const PAGE_PAD_TOP: f32 = 40.0;
const PAGE_PAD_BOTTOM: f32 = 40.0;
const GAP: f32 = 12.0;
/// 品牌头：图标 32 + 两行字。
const BRAND_ICON: f32 = 32.0;
/// 品牌头整行高（`docs/30` §3.2）：20/28 + 13/19 两行 + 2px 间隔。
const BRAND_H: f32 = 48.0;
/// 产品名字号：比 `Type::DISPLAY` 小一号，32px 图标旁边才不压行。
const BRAND_NAME_SIZE: f32 = 20.0;
const QUICK_H: f32 = 72.0;
const TPL_H: f32 = 88.0;
/// 模板分两组：1–6 是结构模板，7–10 是内容模板（`docs/02` §1）。
pub const STRUCTURE_TEMPLATES: usize = 6;
/// 组标题行高（11.5 字 + 上下留白）。组标题不是控件，不进 Tab 序、不吃点击。
const TPL_GROUP_H: f32 = 18.0;
/// 组标题到它下面第一行卡的间距。
const TPL_GROUP_GAP: f32 = 8.0;
/// 上一组最后一行到下一个组标题的间距。
const TPL_GROUP_SPACE: f32 = 14.0;
const ROW_H: f32 = 44.0;
const EMPTY_H: f32 = 76.0;
const FOOTER_H: f32 = 32.0;

/// 起始页字阶：数值直接抄 `docs/30` §3.2。
///
/// 全部收敛到整数档，与外壳 `Type::*` 对齐；保留语义名是为了让调用处
/// 读起来仍知道「这是副标题 / 键帽」，而不是一串裸数字。
mod size {
    pub const SUBTITLE: f32 = 13.0;
    pub const QUICK_TITLE: f32 = 15.0;
    pub const QUICK_HINT: f32 = 13.0;
    pub const ROW_TITLE: f32 = 14.0;
    pub const ROW_META: f32 = 12.0;
    pub const TPL_NAME: f32 = 14.0;
    pub const TPL_HINT: f32 = 12.0;
    pub const KEYCAP: f32 = 12.0;
    pub const FOOTER: f32 = 13.0;
}

/// 起始页只用两个字面：常规与 `zhisi-bold`（600 与 700 都落这一面）。
fn font(size: f32, bold: bool) -> FontId {
    if bold {
        FontId::new(size, FontFamily::Name(crate::fonts::BOLD.into()))
    } else {
        FontId::proportional(size)
    }
}

/// 滚动条几何。静止时也可见——窄窗下它是「还能往下」的唯一信号。
const SCROLL_BAR_W: f32 = 4.0;
const SCROLL_BAR_INSET: f32 = 6.0;
const SCROLL_BAR_MIN_H: f32 = 28.0;
const SCROLL_BAR_PAD: f32 = 4.0;
const SCROLL_FADE_H: f32 = 40.0;
const SCROLL_ALPHA_IDLE: u8 = 130;
const SCROLL_ALPHA_HOT: u8 = 200;
const SCROLL_ALPHA_DRAG: u8 = 235;
/// 键盘焦点滚入视野时，焦点矩形到视口边缘至少留这么多。
const FOCUS_SCROLL_MARGIN: f32 = 8.0;

/// 页面比视口矮时，最多往段间距里摊多少余量（再高就显得松散）。
const SLACK_MAX: f32 = 200.0;
/// 余量摊到四段间距的权重：品牌→快速开始 / 快速→最近 / 最近→模板 / 模板→提示行。
/// 大头给最后一段，提示行因此落在视口近底部，页面才不会「漂在上半屏」。
const SLACK_WEIGHTS: [f32; 4] = [0.10, 0.14, 0.16, 0.60];
/// 四段间距的基础值（内容自然高时就用这四个数）。
const BASE_GAPS: [f32; 4] = [24.0, 28.0, 28.0, 24.0];

/// 把纵向余量按权重分给四段间距。
///
/// 只在「内容比视口矮」时非零，且总和恰好等于 `slack`（不留半像素尾巴，
/// 否则底部空白会多出 1px 的脏缝）。这是纯函数，好测。
pub fn slack_gaps(slack: f32) -> [f32; 4] {
    let slack = slack.clamp(0.0, SLACK_MAX);
    let mut out = BASE_GAPS;
    if slack <= 0.0 {
        return out;
    }
    let mut acc = 0.0;
    for (i, w) in SLACK_WEIGHTS.iter().enumerate() {
        let add = if i + 1 == SLACK_WEIGHTS.len() {
            // 末段吃误差，保证四段加起来正好是 slack。
            slack - acc
        } else {
            (slack * w * 100.0).round() / 100.0
        };
        acc += add;
        out[i] += add;
    }
    out
}

/// 断点阈值。只给 [`layout`] 用，别的函数不许再写魔法数。
const QUICK_4COL_MIN: f32 = 900.0;
const QUICK_2COL_MIN: f32 = 640.0;
const TEMPLATE_3COL_MIN: f32 = 900.0;
const TEMPLATE_2COL_MIN: f32 = 640.0;

/// 开始页的响应式决策。绘制函数只读它，不再各自判断宽度。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// 内容宽：可用宽扣两侧留白后夹在 240–880。
    pub page_w: f32,
    /// 快速开始卡列数：4 / 2 / 1。
    pub quick_cols: usize,
    /// 快速开始卡行数：4 张按列数摊开（4→1、2→2、1→4）。
    pub quick_rows: usize,
    /// 模板列数：3 / 2 / 1。
    pub template_cols: usize,
    /// 最近文件最多显示几行：8 / 5 / 3。
    pub recent_rows: usize,
    /// 副标题收短，避免卡片内挤成两行。
    pub compact_hints: bool,
    /// 模板区是否显示「挑一套开始 / 按 1–9/0 快选」。
    pub show_template_hint: bool,
    /// 底部提示行是否显示键帽。
    pub show_footer_keys: bool,
}

pub fn layout(avail: f32) -> Layout {
    let page_w = (avail - PAGE_PAD * 2.0).clamp(240.0, MAX_W);
    let quick_cols = if avail >= QUICK_4COL_MIN {
        4
    } else if avail >= QUICK_2COL_MIN {
        2
    } else {
        1
    };
    Layout {
        page_w,
        quick_cols,
        quick_rows: 4usize.div_ceil(quick_cols),
        template_cols: if avail >= TEMPLATE_3COL_MIN {
            3
        } else if avail >= TEMPLATE_2COL_MIN {
            2
        } else {
            1
        },
        recent_rows: if avail >= QUICK_4COL_MIN {
            8
        } else if avail >= QUICK_2COL_MIN {
            5
        } else {
            3
        },
        // 副标题长短只跟卡片宽窄有关：四列时每张约 211px，塞不下带快捷键的
        // 长副标题；两列/一列时卡片反而更宽，没理由再收短。
        compact_hints: quick_cols >= 4,
        show_template_hint: avail >= TEMPLATE_2COL_MIN,
        show_footer_keys: avail >= QUICK_2COL_MIN,
    }
}

/// 滚动滑块在视口中的归一化几何。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollThumb {
    /// 滑块顶部在轨道内的位置，0 = 顶部，1 = 底部。
    pub top: f32,
    /// 滑块长度占轨道比例。
    pub len: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScrollMetrics {
    max_offset: f32,
    len: f32,
}

/// 内容比视口高出 1px 以上才算可滚动，避免像素取整让滑块忽隐忽现。
fn scroll_metrics(content_h: f32, viewport_h: f32) -> Option<ScrollMetrics> {
    if viewport_h <= 0.0 || content_h <= viewport_h + 1.0 {
        return None;
    }
    let min_len = (SCROLL_BAR_MIN_H / viewport_h).min(1.0);
    let len = (viewport_h / content_h).max(min_len).clamp(0.0, 1.0);
    Some(ScrollMetrics {
        max_offset: content_h - viewport_h,
        len,
    })
}

/// `None` = 不滚动，此时开始页不画滚动条与渐隐。
pub fn scroll_thumb(content_h: f32, viewport_h: f32, offset_y: f32) -> Option<ScrollThumb> {
    let m = scroll_metrics(content_h, viewport_h)?;
    let t = (offset_y / m.max_offset).clamp(0.0, 1.0);
    Some(ScrollThumb {
        top: t * (1.0 - m.len),
        len: m.len,
    })
}

/// 拖动滑块后反算滚动偏移；几何不可滚动时回 0。
pub fn offset_for_thumb_top(top: f32, content_h: f32, viewport_h: f32) -> f32 {
    let Some(m) = scroll_metrics(content_h, viewport_h) else {
        return 0.0;
    };
    let travel = 1.0 - m.len;
    if travel <= f32::EPSILON {
        return 0.0;
    }
    (top.clamp(0.0, 1.0) / travel).clamp(0.0, 1.0) * m.max_offset
}

/// 键盘焦点变化后，让焦点矩形完整可见所需的最小滚动偏移。
///
/// 返回 `None` 表示当前偏移已经够用（四周留 [`FOCUS_SCROLL_MARGIN`]），
/// 调用方不要设置滚动，避免一次 Tab 让整页跳动。只做最小移动，不居中：
/// 浏览器与原生控件的 Tab 导航都是这个手感。
pub fn focus_scroll_target(
    focus: Rect,
    viewport: Rect,
    offset_y: f32,
    content_h: f32,
) -> Option<f32> {
    let max_offset = (content_h - viewport.height()).max(0.0);
    if viewport.height() <= 0.0 || max_offset <= 1.0 {
        return None;
    }
    let offset = offset_y.clamp(0.0, max_offset);
    let top = focus.top() - FOCUS_SCROLL_MARGIN;
    let bottom = focus.bottom() + FOCUS_SCROLL_MARGIN;
    let next = if focus.height() + FOCUS_SCROLL_MARGIN * 2.0 > viewport.height() {
        // 卡片比视口还高时优先对齐顶边：这时底边永远露不全。
        let want_top = viewport.top() + FOCUS_SCROLL_MARGIN;
        if (want_top..=viewport.bottom()).contains(&focus.top()) {
            return None;
        }
        offset + (focus.top() - want_top)
    } else if top < viewport.top() {
        offset - (viewport.top() - top)
    } else if bottom > viewport.bottom() {
        offset + (bottom - viewport.bottom())
    } else {
        return None;
    };
    let next = next.clamp(0.0, max_offset);
    ((next - offset).abs() > 0.5).then_some(next)
}

/// 开始页上的一个意图。绘制循环只收集，出了循环再落地。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// 新建一张空白思维导图。
    NewBlank,
    /// 新建一张空白概念图（`LayoutSettings.kind = "conceptMap"`）。
    NewConcept,
    OpenFile,
    /// 回到当前打开的文档（只有从文档进入开始页时才有）。
    Return,
    OpenRecent(PathBuf),
    Template(usize),
    CardLibrary,
    /// 只清 `%APPDATA%\织思\recent.json`，不碰任何文档。
    ClearRecents,
}

/// 模板的起手结构：树形或概念图。
enum Shape {
    /// 树：根 + 一级分支（分支可再挂一层）。
    Tree {
        root: &'static str,
        branches: &'static [(&'static str, &'static [&'static str])],
    },
    /// 概念图：中心概念 + 周围自由摆放的概念 + 有向连接。
    Concept {
        root: &'static str,
        /// 周围概念：(文字, 盒左上角 x, 盒左上角 y)。
        concepts: &'static [(&'static str, f32, f32)],
        /// 有向连接：(起点概念文字, 终点概念文字, 连线文字)。
        links: &'static [(&'static str, &'static str, &'static str)],
    },
}

/// 一套模板：布局 + 起手内容。`hint` 是卡片上那一行说明。
pub struct Template {
    pub name: &'static str,
    pub hint: &'static str,
    pub layout: &'static str,
    /// 模板卡上那一枚结构图标（单色矢量，28px）。
    pub(crate) icon: Icon,
    shape: Shape,
}

pub const TEMPLATES: [Template; 10] = [
    Template {
        name: "思维导图",
        hint: "放射式 · 头脑风暴",
        layout: "mindmap",
        icon: Icon::Mindmap,
        shape: Shape::Tree {
            root: "中心主题",
            branches: &[("子主题 1", &[]), ("子主题 2", &[]), ("子主题 3", &[])],
        },
    },
    Template {
        name: "逻辑图",
        hint: "向右展开 · 拆解结构",
        layout: "logicRight",
        icon: Icon::Logic,
        shape: Shape::Tree {
            root: "核心议题",
            branches: &[("要点 1", &[]), ("要点 2", &[]), ("要点 3", &[])],
        },
    },
    Template {
        name: "组织架构",
        hint: "自上而下 · 团队分工",
        layout: "orgChart",
        icon: Icon::Org,
        shape: Shape::Tree {
            root: "组织名称",
            branches: &[("部门 A", &["小组 A1"]), ("部门 B", &[]), ("部门 C", &[])],
        },
    },
    Template {
        name: "鱼骨图",
        hint: "左右因果 · 复盘分析",
        layout: "fishbone",
        icon: Icon::Fishbone,
        shape: Shape::Tree {
            root: "问题是什么",
            branches: &[
                ("原因 1", &[]),
                ("原因 2", &[]),
                ("原因 3", &[]),
                ("原因 4", &[]),
            ],
        },
    },
    Template {
        name: "时间轴",
        hint: "线性推进 · 里程碑",
        layout: "timeline",
        icon: Icon::Timeline,
        shape: Shape::Tree {
            root: "项目名称",
            branches: &[
                ("阶段 1", &[]),
                ("阶段 2", &[]),
                ("阶段 3", &[]),
                ("阶段 4", &[]),
            ],
        },
    },
    Template {
        name: "概念图",
        hint: "概念互连 · 因果网络",
        layout: "conceptMap",
        icon: Icon::ConceptMap,
        shape: Shape::Concept {
            root: "产品定位",
            concepts: &[
                ("用户痛点", -330.0, -170.0),
                ("竞品对比", 250.0, -230.0),
                ("技术方案", 280.0, 40.0),
                ("增长策略", -300.0, 110.0),
                ("风险与依赖", -40.0, 200.0),
            ],
            links: &[
                ("产品定位", "用户痛点", "解决"),
                ("产品定位", "技术方案", "支撑"),
                ("产品定位", "竞品对比", "参照"),
                ("用户痛点", "增长策略", "转化"),
                ("技术方案", "风险与依赖", "带来"),
            ],
        },
    },
    // 内容模板（`docs/02` §1 的 P1 缺口）：起手树按「写什么」分类，布局统一 mindmap。
    // 四张卡都走同一条 `session()` 路径，内容全是静态字面量，不读墙钟/随机数。
    Template {
        name: "空白",
        hint: "干净画布 · 从零开始",
        layout: "mindmap",
        icon: Icon::New,
        // 真正的空文档：根节点不带占位文字，也没有任何子级。
        shape: Shape::Tree {
            root: "",
            branches: &[],
        },
    },
    Template {
        name: "读书笔记",
        hint: "书目 · 观点 · 摘录",
        layout: "mindmap",
        icon: Icon::Note,
        shape: Shape::Tree {
            root: "书名",
            branches: &[
                ("书目信息", &["作者：", "出版社：", "读于："]),
                ("核心观点", &["一句话总结", "支撑论据"]),
                ("章节笔记", &["第 1 章", "第 2 章", "第 3 章"]),
                ("摘录与金句", &["原文与页码"]),
                ("行动", &["读完做什么"]),
            ],
        },
    },
    Template {
        name: "会议记录",
        hint: "与会人 · 议题 · 待办",
        layout: "mindmap",
        icon: Icon::Levels,
        shape: Shape::Tree {
            root: "会议主题",
            branches: &[
                ("与会人", &["主持人：", "记录人：", "参会者："]),
                ("议题", &["议题 1", "议题 2"]),
                ("决议", &["决议 1", "负责人："]),
                ("待办", &["待办 1 · 负责人：", "截止时间："]),
            ],
        },
    },
    Template {
        name: "产品规划",
        hint: "目标 · 场景 · 里程碑",
        layout: "mindmap",
        icon: Icon::Focus,
        shape: Shape::Tree {
            root: "产品名称",
            branches: &[
                ("目标", &["一句话目标", "成功指标："]),
                ("用户与场景", &["目标用户：", "核心场景："]),
                ("里程碑", &["阶段 1", "阶段 2", "阶段 3"]),
                ("风险", &["风险 1", "应对策略："]),
            ],
        },
    },
];

/// 开始页数字键直选：`1`–`9` → 前九张，`0` → 第十张（`docs/30` §3.5）。
///
/// 返回 `None` 表示这张卡没有数字键位（模板超过 10 张时多出来的那些）。
pub fn template_hotkey(idx: usize) -> Option<egui::Key> {
    const KEYS: [egui::Key; 10] = [
        egui::Key::Num1,
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::Num4,
        egui::Key::Num5,
        egui::Key::Num6,
        egui::Key::Num7,
        egui::Key::Num8,
        egui::Key::Num9,
        egui::Key::Num0,
    ];
    KEYS.get(idx).copied()
}

impl Template {
    pub fn at(idx: usize) -> Option<&'static Template> {
        TEMPLATES.get(idx)
    }

    /// 生成一份**干净**的会话：内容还没落盘，但按“未修改”对待
    /// （与空白新建一致，Ctrl+S 才弹另存为）。
    pub fn session(&self) -> Session {
        let mut doc = Document::new(self.name);
        let root = doc.root_id.clone();
        let _ = doc.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: self.layout.to_string(),
                ..Default::default()
            },
        });
        match &self.shape {
            Shape::Tree {
                root: text,
                branches,
            } => {
                let _ = doc.apply(&Command::SetText {
                    id: root.clone(),
                    text: (*text).to_string(),
                });
                let mut nodes = Vec::new();
                for (label, kids) in *branches {
                    let id = mw_core::new_id();
                    let mut top = Node::new(id.clone(), Some(root.clone()), *label);
                    for kid in *kids {
                        let kid_id = mw_core::new_id();
                        top.children.push(kid_id.clone());
                        nodes.push(Node::new(kid_id, Some(id.clone()), *kid));
                    }
                    nodes.push(top);
                }
                if !nodes.is_empty() {
                    let _ = doc.apply(&Command::InsertSubtree {
                        parent: root,
                        index: None,
                        nodes,
                    });
                }
            }
            Shape::Concept {
                root: text,
                concepts,
                links,
            } => {
                let _ = doc.apply(&Command::SetText {
                    id: root.clone(),
                    text: (*text).to_string(),
                });
                for (label, x, y) in *concepts {
                    let _ = doc.apply(&Command::AddFloatingTopic {
                        text: (*label).to_string(),
                        x: *x,
                        y: *y,
                    });
                }
                for (from, to, label) in *links {
                    let (Some(f), Some(t)) = (concept_id(&doc, from), concept_id(&doc, to)) else {
                        continue;
                    };
                    let _ = doc.apply(&Command::AddRelationship {
                        from: f,
                        to: t,
                        label: Some((*label).to_string()),
                    });
                }
            }
        }
        Session::fresh_named(doc, self.name)
    }
}

/// 按文字找概念 id：概念图里每个概念文字唯一，浮在主树之外。
fn concept_id(doc: &Document, text: &str) -> Option<String> {
    // 根概念是「中心概念」，它在词表里但没有 `pos`，必须单独比对**文字**——
    // 拿 `root_id` 去和文字比会永远不相等，根发出的连线会被静默丢掉。
    if doc.nodes.get(&doc.root_id).is_some_and(|n| n.text == text) {
        return Some(doc.root_id.clone());
    }
    doc.nodes
        .values()
        .find(|n| n.text == text && n.parent_id.is_none() && n.pos.is_some())
        .map(|n| n.id.clone())
}

/// 开始页唯一需要保留的状态：从菜单进入/首次出现时把焦点送到主操作卡。
#[derive(Default)]
pub struct StartPage {
    focus_pending: bool,
    /// 下一帧要落到的滚动偏移（拖动滑块、焦点滚入视野）。
    scroll_to: Option<f32>,
    /// 按下滑块时指针相对滑块顶部的偏移，保证拖动跟手不跳。
    grab_dy: Option<f32>,
    /// 上一帧的焦点 id：只在焦点变化时把卡片滚进视口，滚轮不会被抢回。
    focus_seen: Option<egui::Id>,
    /// 页面刚出现时自动聚焦主操作卡，这一次焦点变化也要保证卡片可见。
    reveal_focus: bool,
    /// 上一帧量到的内容总高（已含摊分出去的余量）。
    content_h_seen: f32,
    /// 上一帧摊出去的余量，用来反算「不含余量的自然高」。
    slack_seen: f32,
}

impl StartPage {
    /// 页面刚出现或从菜单返回时调用；下一帧绘制结束前把焦点移回「新建空白图」。
    pub fn request_new_focus(&mut self) {
        self.focus_pending = true;
        self.reveal_focus = true;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        recents: &[recent::Entry],
        recents_pending: bool,
        can_return: bool,
        now_ms: i64,
        tone: Tone,
    ) -> Option<Action> {
        let full = ui.max_rect();
        let p = ui.painter();
        p.rect_filled(full, 0, tone.bg);
        // 页面底只铺一层纯色。曾经这里画两团 9% 的主题色光晕，实测在浅色主题上
        // 像抹了一层脏灰——`docs/30` §3.7 禁装饰性大留白，同样不该留装饰性大色斑。

        let mut action = None;
        let mut new_id = None;
        // 纵向余量按上一帧量到的自然高反算：内容比视口矮就把空白还给段间距，
        // 页面因此「装满」而不是漂在上半屏（`docs/30` §1.1 的 S1）。
        // 用上一帧的量测值而不是本帧预估值：控件行高受字体影响，算不准就只能量。
        let view_h = ui.available_height();
        let base_h = (self.content_h_seen - self.slack_seen).max(0.0);
        let slack = if base_h > 1.0 {
            (view_h - base_h).clamp(0.0, SLACK_MAX)
        } else {
            0.0
        };
        let gaps = slack_gaps(slack);
        // 自绘滚动条接管滚动条命中；滚轮仍走 ScrollArea 原生路径。
        let mut area = egui::ScrollArea::vertical()
            .id_salt("start-page")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .scroll_source(egui::scroll_area::ScrollSource {
                scroll_bar: false,
                ..Default::default()
            });
        if let Some(y) = self.scroll_to.take() {
            area = area.vertical_scroll_offset(y);
        }
        let out = area.show(ui, |ui| {
            // 两侧留白要从可用宽里扣：窗口窄于 MAX_W 时若先取整宽再补 pad，
            // 内容会整体右移 18pt，右侧被窗口裁掉。
            let lay = layout(ui.available_width());
            let width = lay.page_w;
            let pad = ((ui.available_width() - width) * 0.5).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.allocate_ui_with_layout(
                    Vec2::new(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        ui.add_space(PAGE_PAD_TOP);
                        self.header(ui, can_return, tone, &mut action);
                        ui.add_space(gaps[0]);

                        new_id = quick_start(ui, lay, tone, &mut action);
                        ui.add_space(gaps[1]);

                        recents_section(
                            ui,
                            lay,
                            recents,
                            recents_pending,
                            now_ms,
                            tone,
                            &mut action,
                        );
                        ui.add_space(gaps[2]);

                        section(
                            ui,
                            "模板",
                            lay.show_template_hint.then_some("挑一套开始"),
                            lay.show_template_hint.then_some("按 1–9/0 快选"),
                            tone,
                        );
                        ui.add_space(12.0);
                        // 10 张卡仍用同一张网格（同尺寸、同间距、同断点），只加两行
                        // 组标题把「结构」与「内容」分开：3 列时 6 张结构正好两整行，
                        // 4 张内容收在下一组，最后那一格按既有占位分支留空。
                        template_group_label(ui, "结构模板", tone);
                        ui.add_space(TPL_GROUP_GAP);
                        template_grid(ui, lay, tone, &mut action, 0..STRUCTURE_TEMPLATES);
                        ui.add_space(TPL_GROUP_SPACE);
                        template_group_label(ui, "内容模板", tone);
                        ui.add_space(TPL_GROUP_GAP);
                        template_grid(
                            ui,
                            lay,
                            tone,
                            &mut action,
                            STRUCTURE_TEMPLATES..TEMPLATES.len(),
                        );
                        ui.add_space(gaps[3]);

                        footer_strip(ui, lay, tone);
                        ui.add_space(PAGE_PAD_BOTTOM);
                    },
                );
            });
        });

        // 本帧量到的内容高留到下一帧反算自然高；两者相减就与余量无关。
        self.content_h_seen = out.content_size.y;
        self.slack_seen = slack;

        if let Some(y) = paint_scroll_affordance(
            ui,
            out.inner_rect,
            out.content_size.y,
            out.state.clamped_offset().y,
            &mut self.grab_dy,
            tone,
        ) {
            self.scroll_to = Some(y);
        }

        // 键盘焦点换了控件时，把首屏外的卡片滚进视野——否则 Tab 到模板卡只有
        // 屏幕外的焦点环。只认 Tab/Shift+Tab 与页面出现时的自动聚焦：点标题栏
        // 也会移动焦点，但那是壳层控件，不属于开始页的滚动内容。
        let focused = ui.ctx().memory(|m| m.focused());
        if focused != self.focus_seen {
            self.focus_seen = focused;
            let tab_nav = ui.input(|i| i.key_pressed(egui::Key::Tab));
            let reveal = tab_nav || self.reveal_focus;
            self.reveal_focus = false;
            if reveal && self.scroll_to.is_none() {
                if let Some(id) = focused {
                    if let Some(resp) = ui.ctx().read_response(id) {
                        let offset = out.state.clamped_offset().y;
                        if let Some(y) = focus_scroll_target(
                            resp.rect,
                            out.inner_rect,
                            offset,
                            out.content_size.y,
                        ) {
                            trace::event(&format!("start_focus_scroll from={offset:.1} to={y:.1}"));
                            self.scroll_to = Some(y);
                        }
                    }
                }
            }
        }

        // 没有任何控件握着焦点时自动落到主操作；从菜单返回时用 pending 强制一次。
        if let Some(id) = new_id {
            let wanted = self.focus_pending || ui.ctx().memory(|m| m.focused()).is_none();
            if wanted {
                ui.ctx().memory_mut(|m| m.request_focus(id));
                self.focus_pending = false;
            }
        }
        action
    }

    fn header(
        &mut self,
        ui: &mut egui::Ui,
        can_return: bool,
        tone: Tone,
        action: &mut Option<Action>,
    ) {
        // 固定 48 高：品牌头是页面的第一段，行高跟着内容涨会让下面所有段落
        // 在对齐上抖 1–2px（`docs/30` §3.2）。
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), BRAND_H),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let (mark, _) = ui.allocate_exact_size(Vec2::splat(BRAND_ICON), Sense::hover());
                paint_logo(ui.painter(), mark, tone);
                ui.add_space(11.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    // `.strong()` 在 egui 0.37 只改颜色不改字重；品牌名要的是真 semibold，
                    // 所以直接指定粗体族。
                    ui.label(
                        RichText::new("织思")
                            .size(BRAND_NAME_SIZE)
                            .family(FontFamily::Name(crate::fonts::BOLD.into()))
                            .color(tone.ink),
                    );
                    ui.label(
                        RichText::new("MindWeaver · 把想法织成结构")
                            .size(size::SUBTITLE)
                            .color(tone.ch.dim),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if can_return {
                        let resp = ui_kit::outline_button(ui, "返回文档", tone);
                        trace::name(ui.ctx(), resp.id, "开始页 · 返回文档");
                        ui_kit::paint_focus_ring(
                            ui.painter(),
                            tone.ch,
                            resp.rect,
                            design::component::OUTLINE_RADIUS,
                            &resp,
                        );
                        if ui_kit::activated(ui, &resp) {
                            *action = Some(Action::Return);
                        }
                    }
                });
            },
        );
    }
}

/// 滚动条与上下渐隐。返回下一帧要应用的滚动偏移（拖动滑块时）。
fn paint_scroll_affordance(
    ui: &mut egui::Ui,
    viewport: Rect,
    content_h: f32,
    offset: f32,
    grab_dy: &mut Option<f32>,
    tone: Tone,
) -> Option<f32> {
    let Some(thumb) = scroll_thumb(content_h, viewport.height(), offset) else {
        *grab_dy = None;
        return None;
    };

    let below = content_h - viewport.height() - offset;
    if below > 0.5 {
        let strength = (below / SCROLL_FADE_H).clamp(0.0, 1.0) * 0.9;
        paint_scroll_fade(ui.painter(), viewport, false, strength, tone);
    }
    if offset > 0.5 {
        let strength = (offset / SCROLL_FADE_H).clamp(0.0, 1.0) * 0.9;
        paint_scroll_fade(ui.painter(), viewport, true, strength, tone);
    }

    let track = Rect::from_min_max(
        Pos2::new(
            viewport.right() - SCROLL_BAR_INSET - SCROLL_BAR_W,
            viewport.top() + SCROLL_BAR_PAD,
        ),
        Pos2::new(
            viewport.right() - SCROLL_BAR_INSET,
            viewport.bottom() - SCROLL_BAR_PAD,
        ),
    );
    if track.height() < SCROLL_BAR_MIN_H {
        return None;
    }
    let thumb_h = (thumb.len * track.height()).clamp(SCROLL_BAR_MIN_H, track.height());
    let travel = track.height() - thumb_h;
    let mut rect = Rect::from_min_size(
        Pos2::new(track.left(), track.top() + thumb.top * travel),
        Vec2::new(SCROLL_BAR_W, thumb_h),
    );

    // 命中区比 4px 视觉宽：左右各 6、上下各 4。指针在滑块上时 egui 只让
    // 这个（最后登记的）控件拿到拖拽，内置滚动条已在本页关闭。
    let resp = ui.interact(
        rect.expand2(Vec2::new(6.0, 4.0)),
        ui.scope_id().with("start-scrollbar"),
        // 不进 Tab 顺序：X18 的焦点闭环只属于卡片与按钮。
        Sense::DRAG,
    );
    let mut request = None;
    if resp.is_pointer_button_down_on() {
        if let Some(p) = resp.interact_pointer_pos() {
            let grab = *grab_dy.get_or_insert((p.y - rect.top()).clamp(0.0, thumb_h));
            if resp.dragged() {
                let top = (p.y - grab).clamp(track.top(), track.top() + travel);
                rect = Rect::from_min_size(
                    Pos2::new(track.left(), top),
                    Vec2::new(SCROLL_BAR_W, thumb_h),
                );
                let frac = if travel > 0.0 {
                    (top - track.top()) / travel
                } else {
                    0.0
                };
                request = Some(offset_for_thumb_top(frac, content_h, viewport.height()));
            }
        }
    } else {
        *grab_dy = None;
    }

    let alpha = if resp.dragged() {
        SCROLL_ALPHA_DRAG
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        SCROLL_ALPHA_HOT
    } else {
        SCROLL_ALPHA_IDLE
    };
    let dim = tone.ch.dim;
    ui.painter().rect_filled(
        rect,
        2.0,
        Color32::from_rgba_unmultiplied(dim.r(), dim.g(), dim.b(), alpha),
    );
    request
}

fn paint_scroll_fade(
    painter: &egui::Painter,
    viewport: Rect,
    at_top: bool,
    strength: f32,
    tone: Tone,
) {
    let bg = tone.bg;
    let clear = Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 0);
    let solid = Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), (255.0 * strength) as u8);
    let (rect, colors) = if at_top {
        (
            Rect::from_min_size(
                viewport.left_top(),
                Vec2::new(viewport.width(), SCROLL_FADE_H),
            ),
            [solid, clear],
        )
    } else {
        (
            Rect::from_min_size(
                Pos2::new(viewport.left(), viewport.bottom() - SCROLL_FADE_H),
                Vec2::new(viewport.width(), SCROLL_FADE_H),
            ),
            [clear, solid],
        )
    };
    painter.add(egui::Shape::gradient_rect(
        rect,
        egui::epaint::Direction::TopDown,
        colors,
    ));
}

fn section(ui: &mut egui::Ui, title: &str, hint: Option<&str>, right: Option<&str>, tone: Tone) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .size(Type::HEADING)
                .family(FontFamily::Name(crate::fonts::BOLD.into()))
                .color(tone.ink),
        );
        if let Some(hint) = hint {
            ui.add_space(6.0);
            ui.label(RichText::new(hint).size(Type::SMALL).color(tone.ch.dim));
        }
        if let Some(right) = right {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(right).size(Type::SMALL).color(tone.ch.dim));
            });
        }
    });
}

fn paint_logo(p: &egui::Painter, rect: Rect, tone: Tone) {
    p.rect_filled(rect, 12.0, tone.ch.accent);
    let c = rect.center();
    let k = rect.width() / 26.0;
    let ink = tone.on_accent;
    let s = Stroke::new(1.9 * k, ink);
    let leaves = [
        Pos2::new(c.x + 5.6 * k, c.y - 5.2 * k),
        Pos2::new(c.x + 6.0 * k, c.y + 4.8 * k),
        Pos2::new(c.x - 5.8 * k, c.y + 5.2 * k),
    ];
    for leaf in leaves {
        p.line_segment([c, leaf], s);
        p.circle_filled(leaf, 1.9 * k, ink);
    }
    p.circle_filled(c, 2.3 * k, ink);
}

fn quick_start(
    ui: &mut egui::Ui,
    lay: Layout,
    tone: Tone,
    action: &mut Option<Action>,
) -> Option<egui::Id> {
    let cols = lay.quick_cols;
    let card_w = ((lay.page_w - GAP * (cols as f32 - 1.0)) / cols as f32).max(160.0);
    let items = [
        (
            Quick::Mindmap,
            "新建思维导图",
            if lay.compact_hints {
                "空白画布"
            } else {
                "空白画布 · Enter"
            },
        ),
        (
            Quick::Concept,
            "新建概念图",
            if lay.compact_hints {
                "概念互连"
            } else {
                "概念互连 · 命题网"
            },
        ),
        (
            Quick::Open,
            "打开文件…",
            if lay.compact_hints {
                "Ctrl+O"
            } else {
                "Ctrl+O · .zmind 等"
            },
        ),
        (
            Quick::Library,
            "卡片笔记库",
            if lay.compact_hints {
                "卡片与标签"
            } else {
                "浏览全部卡片与标签"
            },
        ),
    ];
    let mut new_id = None;
    for row in 0..lay.quick_rows {
        if row > 0 {
            ui.add_space(GAP);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for col in 0..cols {
                let idx = row * cols + col;
                let Some((kind, title, hint)) = items.get(idx) else {
                    ui.allocate_exact_size(Vec2::new(card_w, QUICK_H), Sense::hover());
                    continue;
                };
                let (id, hit) = quick_card(ui, card_w, *kind, title, hint, tone);
                if *kind == Quick::Mindmap {
                    new_id = Some(id);
                }
                if hit {
                    *action = Some(match kind {
                        Quick::Mindmap => Action::NewBlank,
                        Quick::Concept => Action::NewConcept,
                        Quick::Open => Action::OpenFile,
                        Quick::Library => Action::CardLibrary,
                    });
                }
            }
        });
    }
    new_id
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quick {
    Mindmap,
    Concept,
    Open,
    Library,
}

/// 把 `b` 按 `t` 混进 `a`（`t = 0` 不变）。只做不透明色的插值。
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

fn quick_card(
    ui: &mut egui::Ui,
    w: f32,
    kind: Quick,
    title: &str,
    hint: &str,
    tone: Tone,
) -> (egui::Id, bool) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, QUICK_H), Sense::click());
    let primary = kind == Quick::Mindmap;
    let hover = resp.hovered();
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let accent = tone.ch.accent;
    // 主作用卡不铺实底强调色：8% 强调底 + 强调色图标，其余卡保持安静。
    // hover 只加 4%（`docs/30` §3.4）——整块强调色实底是被明确禁止的那种「网页 hero」。
    let mut fill = if primary {
        mix(tone.card, accent, 0.08)
    } else {
        tone.card
    };
    if hover {
        fill = mix(fill, accent, if primary { 0.06 } else { 0.04 });
    }
    let border = Stroke::new(
        design::stroke::HAIRLINE,
        if hover { accent } else { tone.stroke },
    );
    ui.painter().rect(
        rect,
        CornerRadius::same(design::radius::PANEL as u8),
        fill,
        border,
        StrokeKind::Inside,
    );

    let icon_box = Rect::from_center_size(
        Pos2::new(rect.left() + 12.0 + 18.0, rect.center().y),
        Vec2::splat(36.0),
    );
    // 四张卡必须是同一个组件，只有主作用那张换色：chip 形状与尺寸一致，
    // 否则「主卡有底块、其余卡没有」看起来像画漏了，而不是像分工。
    //
    // 暗色下同样的百分比会消失在底色里（10% 强调色压在近黑面板上看不见），
    // 所以深色主题各多半档。
    let (chip, icon_ink) = if primary {
        (
            mix(tone.card, accent, if tone.dark { 0.22 } else { 0.10 }),
            accent,
        )
    } else {
        (
            mix(tone.card, tone.ink, if tone.dark { 0.12 } else { 0.06 }),
            tone.muted,
        )
    };
    ui.painter().rect_filled(icon_box, 9.0, chip);
    let icon = match kind {
        Quick::Mindmap => Icon::Mindmap,
        Quick::Concept => Icon::ConceptMap,
        Quick::Open => Icon::Open,
        Quick::Library => Icon::Library,
    };
    icon_paint::paint_icon_sized(ui.painter(), icon_box.center(), 20.0, icon, icon_ink);

    let tx = rect.left() + 60.0;
    let hint_font = font(size::QUICK_HINT, false);
    let hint_max = (rect.right() - 12.0 - tx).max(0.0);
    ui.painter().text(
        Pos2::new(tx, rect.center().y - 10.0),
        Align2::LEFT_CENTER,
        title,
        font(size::QUICK_TITLE, true),
        tone.ink,
    );
    ui.painter().text(
        Pos2::new(tx, rect.center().y + 11.0),
        Align2::LEFT_CENTER,
        fit_text(ui.painter(), hint, &hint_font, hint_max),
        hint_font,
        tone.ch.dim,
    );
    if primary {
        // 主作用卡：焦点环画在**卡内** 2px（`docs/30` §3.3）。卡本身已经有 8% 强调底，
        // 再在外面套一圈同色环就是「同色叠同色」，糊成一片。
        if resp.has_focus() {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                CornerRadius::same(design::radius::PANEL as u8),
                Stroke::new(2.0, accent),
                StrokeKind::Inside,
            );
        }
    } else {
        ui_kit::paint_focus_ring(ui.painter(), tone.ch, rect, design::radius::PANEL, &resp);
    }
    // 卡片矩形进轨迹：交互脚本（`MW_GOLDEN_INPUT` 的 `move/click`）按屏幕坐标写，
    // 断点一变就得重算；有这行就不用猜（C3 的 relink 探针踩过同样的坑）。
    if trace::is_enabled() {
        trace::event(&format!(
            "quick rect {} {:.0},{:.0} {:.0}x{:.0}",
            match kind {
                Quick::Mindmap => "mindmap",
                Quick::Concept => "concept",
                Quick::Open => "open",
                Quick::Library => "library",
            },
            rect.left(),
            rect.top(),
            rect.width(),
            rect.height()
        ));
    }
    let label = match kind {
        Quick::Mindmap => "开始页 · 新建思维导图",
        Quick::Concept => "开始页 · 新建概念图",
        Quick::Open => "开始页 · 打开文件",
        Quick::Library => "开始页 · 卡片笔记库",
    };
    trace::name(ui.ctx(), resp.id, label);
    (resp.id, ui_kit::activated(ui, &resp))
}

/// 模板区的小标题：「结构模板」/「内容模板」。
///
/// 它不是控件：没有命中区、不进 Tab 序，只是把同一张网格分成两组，
/// 免得 10 张卡里那 4 张内容模板像「多出来的」。
fn template_group_label(ui: &mut egui::Ui, text: &str, tone: Tone) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), TPL_GROUP_H), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font(size::TPL_HINT, true),
        tone.ch.dim,
    );
}

/// 画一组模板卡。`range` 是 `TEMPLATES` 的下标区间，卡宽仍按页宽与列数算，
/// 所以两组卡永远同尺寸；行尾不足一格时补占位，卡片不会被拉伸。
fn template_grid(
    ui: &mut egui::Ui,
    lay: Layout,
    tone: Tone,
    action: &mut Option<Action>,
    range: std::ops::Range<usize>,
) {
    let cols = lay.template_cols;
    let card_w = ((lay.page_w - GAP * (cols as f32 - 1.0)) / cols as f32).max(180.0);
    let rows = range.len().div_ceil(cols);
    for row in 0..rows {
        if row > 0 {
            ui.add_space(GAP);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for col in 0..cols {
                let idx = range.start + row * cols + col;
                if idx < range.end {
                    if template_card(ui, card_w, &TEMPLATES[idx], idx, tone) {
                        *action = Some(Action::Template(idx));
                    }
                } else {
                    // 撑住最后一行，别让卡片被拉伸。
                    ui.allocate_exact_size(Vec2::new(card_w, TPL_H), Sense::hover());
                }
            }
        });
    }
}

fn template_card(ui: &mut egui::Ui, w: f32, tpl: &Template, idx: usize, tone: Tone) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, TPL_H), Sense::click());
    let hover = resp.hovered();
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let fill = if hover {
        mix(tone.card, tone.ch.accent, 0.06)
    } else {
        tone.card
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(design::radius::PANEL as u8),
        fill,
        Stroke::new(
            design::stroke::HAIRLINE,
            if hover { tone.ch.accent } else { tone.stroke },
        ),
        StrokeKind::Inside,
    );

    // 结构图标：单色矢量，认出「这套布局长什么样」靠形状，不靠彩色示意图。
    let icon_box = Rect::from_center_size(
        Pos2::new(rect.left() + 28.0, rect.center().y),
        Vec2::splat(28.0),
    );
    icon_paint::paint_icon_sized(ui.painter(), icon_box.center(), 28.0, tpl.icon, tone.ink);

    let tx = rect.left() + 52.0;
    let hint_font = font(size::TPL_HINT, false);
    let text_max = (rect.right() - 40.0 - tx).max(0.0);
    ui.painter().text(
        Pos2::new(tx, rect.center().y - 10.0),
        Align2::LEFT_CENTER,
        tpl.name,
        font(size::TPL_NAME, true),
        tone.ink,
    );
    ui.painter().text(
        Pos2::new(tx, rect.center().y + 11.0),
        Align2::LEFT_CENTER,
        fit_text(ui.painter(), tpl.hint, &hint_font, text_max),
        hint_font,
        tone.ch.dim,
    );

    // 数字键帽：视觉上告诉用户 1–9 / 0 能直接选模板。
    let key = Rect::from_min_size(
        Pos2::new(rect.right() - 32.0, rect.top() + 10.0),
        Vec2::splat(18.0),
    );
    ui.painter().rect(
        key,
        CornerRadius::same(design::radius::XS as u8),
        tone.card,
        Stroke::new(design::stroke::HAIRLINE, tone.stroke),
        StrokeKind::Inside,
    );
    ui.painter().text(
        key.center(),
        Align2::CENTER_CENTER,
        format!("{}", idx + 1),
        font(size::KEYCAP, false),
        tone.ch.dim,
    );

    ui_kit::paint_focus_ring(ui.painter(), tone.ch, rect, design::radius::PANEL, &resp);
    if trace::is_enabled() {
        trace::name(ui.ctx(), resp.id, &format!("开始页 · 模板 {}", tpl.name));
    }
    ui_kit::activated(ui, &resp)
}

fn recents_section(
    ui: &mut egui::Ui,
    lay: Layout,
    recents: &[recent::Entry],
    pending: bool,
    now_ms: i64,
    tone: Tone,
    action: &mut Option<Action>,
) {
    // 标题行里只**占位**（灰色占位控件，不是按钮）：真正那颗按钮等最近文件行画完再注册，
    // 这样 Tab 序是「… → 最近文件行 → 清除列表 →（下一段）」——`docs/30` §3.5。
    let show_clear = !pending && !recents.is_empty();
    let clear_rect = ui
        .horizontal(|ui| {
            ui.label(
                RichText::new("最近文件")
                    .size(Type::HEADING)
                    .family(FontFamily::Name(crate::fonts::BOLD.into()))
                    .color(tone.ink),
            );
            if show_clear {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let size =
                        ui_kit::text_button_size(ui, tone.ch, "清除列表", ButtonTone::Ghost, true);
                    ui.allocate_exact_size(size, Sense::hover()).0
                })
                .inner
            } else {
                Rect::NOTHING
            }
        })
        .inner;
    ui.add_space(10.0);
    if pending {
        recents_skeleton(ui, tone);
    } else if recents.is_empty() {
        empty_recents(ui, tone);
    } else {
        recents_card(ui, lay.recent_rows, recents, now_ms, tone, action);
    }

    if show_clear && clear_rect.is_positive() {
        let resp =
            ui_kit::text_button_at(ui, clear_rect, tone.ch, "清除列表", ButtonTone::Ghost, true);
        trace::name(ui.ctx(), resp.id, "开始页 · 清除最近文件");
        ui_kit::paint_focus_ring(ui.painter(), tone.ch, resp.rect, 6.0, &resp);
        if ui_kit::activated(ui, &resp) {
            *action = Some(Action::ClearRecents);
        }
    }
}

/// 3 行加载骨架（`docs/30` §3.4）：卡片与三行真实列表**同高**，列表到货时不跳。
///
/// 骨架不吃点击：它是占位，不是「点了没反应的行」。
fn recents_skeleton(ui: &mut egui::Ui, tone: Tone) {
    const ROWS: usize = 3;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), recents_height(ROWS)),
        Sense::hover(),
    );
    ui.painter().rect(
        rect,
        design::radius::PANEL,
        tone.card,
        Stroke::new(design::stroke::HAIRLINE, tone.stroke),
        StrokeKind::Inside,
    );
    let bar = mix(tone.card, tone.ink, 0.07);
    for i in 0..ROWS {
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.top() + 6.0 + i as f32 * ROW_H),
            Vec2::new(rect.width() - 12.0, ROW_H),
        );
        let icon = Rect::from_center_size(
            Pos2::new(row.left() + 18.0, row.center().y),
            Vec2::splat(16.0),
        );
        ui.painter().rect_filled(icon, design::radius::XS, bar);
        let title = Rect::from_min_size(
            Pos2::new(row.left() + 36.0, row.center().y - 12.0),
            Vec2::new((row.width() * 0.32).max(60.0), 8.0),
        );
        let meta = Rect::from_min_size(
            Pos2::new(row.left() + 36.0, row.center().y + 4.0),
            Vec2::new((row.width() * 0.20).max(40.0), 6.0),
        );
        ui.painter().rect_filled(title, 4.0, bar);
        ui.painter().rect_filled(meta, 3.0, bar);
    }
    if trace::is_enabled() {
        trace::event(&format!("recents skeleton rows={ROWS}"));
    }
}

fn empty_recents(ui: &mut egui::Ui, tone: Tone) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), EMPTY_H), Sense::hover());
    ui.painter().rect(
        rect,
        design::radius::PANEL,
        tone.card,
        Stroke::new(design::stroke::HAIRLINE, tone.stroke),
        StrokeKind::Inside,
    );
    let icon = Rect::from_center_size(
        Pos2::new(rect.left() + 30.0, rect.center().y),
        Vec2::splat(28.0),
    );
    ui.painter().rect_filled(icon, 8.0, tone.ch.accent_soft);
    icon_paint::paint_icon_sized(
        ui.painter(),
        icon.center(),
        14.0,
        Icon::File,
        tone.ch.accent,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 56.0, rect.center().y - 10.0),
        Align2::LEFT_CENTER,
        "还没有最近文件",
        font(size::ROW_TITLE, true),
        tone.ink,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 56.0, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        "打开或新建后，会在这里快速回来",
        font(size::SUBTITLE, false),
        tone.ch.dim,
    );
}

fn recents_card(
    ui: &mut egui::Ui,
    max_rows: usize,
    recents: &[recent::Entry],
    now_ms: i64,
    tone: Tone,
    action: &mut Option<Action>,
) {
    let rows = recents.len().min(max_rows);
    let height = recents_height(rows);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter().rect(
        rect,
        design::radius::PANEL,
        tone.card,
        Stroke::new(design::stroke::HAIRLINE, tone.stroke),
        StrokeKind::Inside,
    );
    for (i, entry) in recents.iter().take(rows).enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.top() + 6.0 + i as f32 * ROW_H),
            Vec2::new(rect.width() - 12.0, ROW_H),
        );
        let resp = ui.interact(
            row,
            ui.make_persistent_id(("mw-home-recent", i)),
            Sense::click(),
        );
        if resp.hovered() || resp.has_focus() {
            ui.painter().rect_filled(row, 8.0, tone.hover);
            let c = Pos2::new(row.right() - 16.0, row.center().y);
            let s = Stroke::new(1.2, tone.ch.faint);
            ui.painter()
                .line_segment([c + Vec2::new(-2.0, -4.0), c + Vec2::new(2.0, 0.0)], s);
            ui.painter()
                .line_segment([c + Vec2::new(2.0, 0.0), c + Vec2::new(-2.0, 4.0)], s);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // 行内图标只用单色 16：最近文件的辨识靠文件名，不靠色块。
        let icon = Rect::from_center_size(
            Pos2::new(row.left() + 18.0, row.center().y),
            Vec2::splat(16.0),
        );
        icon_paint::paint_icon_sized(ui.painter(), icon.center(), 16.0, Icon::File, tone.muted);

        let title = if entry.title.trim().is_empty() {
            entry
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("未命名")
        } else {
            entry.title.as_str()
        };
        let time_w = 76.0;
        let text_w = row.width() - 36.0 - time_w - 24.0;
        // 文件被移动/删掉之后这一行**留着**（`docs/30` §3.4）：标题转 secondary，
        // 副行改说「文件已移动」。以前是启动时直接把这行摘掉——用户只会看到「少了一条」。
        let meta = if entry.missing {
            "文件已移动".to_owned()
        } else {
            folder_hint(&entry.path)
        };
        ui.painter().text(
            Pos2::new(row.left() + 36.0, row.center().y - 8.0),
            Align2::LEFT_CENTER,
            truncate(title, ((text_w / size::ROW_TITLE) as usize).clamp(6, 48)),
            font(size::ROW_TITLE, true),
            if entry.missing { tone.muted } else { tone.ink },
        );
        ui.painter().text(
            Pos2::new(row.left() + 36.0, row.center().y + 9.0),
            Align2::LEFT_CENTER,
            truncate(
                &meta,
                ((text_w / size::ROW_META * 1.55) as usize).clamp(8, 80),
            ),
            font(size::ROW_META, false),
            tone.ch.dim,
        );
        ui.painter().text(
            Pos2::new(row.right() - 28.0, row.center().y),
            Align2::RIGHT_CENTER,
            relative_time(entry.at, now_ms),
            font(size::ROW_META, false),
            tone.ch.dim,
        );
        ui_kit::paint_focus_ring(ui.painter(), tone.ch, row, 8.0, &resp);
        if trace::is_enabled() {
            trace::name(ui.ctx(), resp.id, &format!("开始页 · 最近文件 {title}"));
            if entry.missing {
                // 带矩形：验收脚本要能点它（`MW_GOLDEN_INPUT` 只认屏幕坐标）。
                trace::event(&format!(
                    "recents row missing title={title} rect={:.0},{:.0} {:.0}x{:.0}",
                    row.left(),
                    row.top(),
                    row.width(),
                    row.height()
                ));
            }
        }
        if ui_kit::activated(ui, &resp) {
            *action = Some(Action::OpenRecent(entry.path.clone()));
        }
        resp.on_hover_text(entry.path.display().to_string());
    }
}

/// 最近文件卡的高度：行高固定 44，上下各 6pt 内距。
fn recents_height(rows: usize) -> f32 {
    rows as f32 * ROW_H + 12.0
}

const FOOTER_KEYS: [(&str, &str); 3] = [("Enter", "新建"), ("Ctrl+O", "打开"), ("1–9/0", "模板")];

fn footer_strip(ui: &mut egui::Ui, lay: Layout, tone: Tone) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(lay.page_w, FOOTER_H), Sense::hover());
    ui.painter().rect(
        rect,
        design::radius::PANEL,
        tone.card,
        Stroke::new(design::stroke::HAIRLINE, tone.stroke),
        StrokeKind::Inside,
    );
    let icon_c = Pos2::new(rect.left() + 20.0, rect.center().y);
    icon_paint::paint_icon_sized(ui.painter(), icon_c, 14.0, Icon::File, tone.ch.accent);

    let text_x = rect.left() + 36.0;
    let keys_w = if lay.show_footer_keys {
        footer_keys_width(ui.painter())
    } else {
        0.0
    };
    let gap = if lay.show_footer_keys { 16.0 } else { 0.0 };
    let text_max = (rect.right() - 16.0 - keys_w - gap - text_x).max(0.0);
    let font = font(size::FOOTER, false);
    let text = fit_text(
        ui.painter(),
        "拖入 .zmind / Markdown / OPML 即可打开",
        &font,
        text_max,
    );
    ui.painter().text(
        Pos2::new(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        tone.ch.dim,
    );
    if lay.show_footer_keys {
        paint_footer_keys(ui.painter(), rect.right() - 16.0, rect.center().y, tone);
    }
}

fn footer_keys_width(painter: &egui::Painter) -> f32 {
    let key_font = font(size::KEYCAP, false);
    let label_font = font(size::KEYCAP, false);
    let mut w = 0.0;
    for (i, (key, label)) in FOOTER_KEYS.iter().enumerate() {
        if i > 0 {
            w += 14.0;
        }
        w += text_width(painter, key, &key_font) + 12.0 + 8.0;
        w += text_width(painter, label, &label_font);
    }
    w
}

fn paint_footer_keys(painter: &egui::Painter, right: f32, cy: f32, tone: Tone) {
    let key_font = font(size::KEYCAP, false);
    let label_font = font(size::KEYCAP, false);
    let mut x = right - footer_keys_width(painter);
    for (i, (key, label)) in FOOTER_KEYS.iter().enumerate() {
        if i > 0 {
            x += 14.0;
        }
        let kw = text_width(painter, key, &key_font) + 10.0;
        let key_rect = Rect::from_min_size(Pos2::new(x, cy - 9.0), Vec2::new(kw, 18.0));
        painter.rect(
            key_rect,
            CornerRadius::same(design::radius::XS as u8),
            tone.current,
            Stroke::new(design::stroke::HAIRLINE, tone.stroke),
            StrokeKind::Inside,
        );
        let galley = painter.layout_no_wrap(key.to_string(), key_font.clone(), tone.ch.dim);
        painter.galley(key_rect.center() - galley.size() * 0.5, galley, tone.ch.dim);
        x += kw + 8.0;
        painter.text(
            Pos2::new(x, cy),
            Align2::LEFT_CENTER,
            *label,
            label_font.clone(),
            tone.ch.dim,
        );
        x += text_width(painter, label, &label_font);
    }
}

fn text_width(painter: &egui::Painter, text: &str, font: &FontId) -> f32 {
    painter
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x
}

/// 按像素宽截断单行文字：底部提示条不许换行压高。
fn fit_text(painter: &egui::Painter, text: &str, font: &FontId, max_w: f32) -> String {
    if text_width(painter, text, font) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    for n in (0..chars.len()).rev() {
        let mut candidate: String = chars[..n].iter().collect();
        candidate.push('…');
        if text_width(painter, &candidate, font) <= max_w {
            return candidate;
        }
    }
    String::new()
}

/// 最近文件那一行的第二行：父目录。文件名本身已经在上行，重复没有意义。
pub fn folder_hint(path: &Path) -> String {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "此电脑".into())
}

/// 相对时间。**不用绝对日期**：本程序没有时区/日历依赖，跨过 7 天直接说
/// “N 周/个月/年前”，不会在午夜附近显示成前一天。
pub fn relative_time(at_ms: i64, now_ms: i64) -> String {
    let secs = (now_ms - at_ms).max(0) / 1000;
    match secs {
        0..=59 => "刚刚".into(),
        60..=3599 => format!("{} 分钟前", secs / 60),
        3600..=86_399 => format!("{} 小时前", secs / 3600),
        86_400..=604_799 => format!("{} 天前", secs / 86_400),
        604_800..=2_591_999 => format!("{} 周前", secs / 604_800),
        2_592_000..=31_535_999 => format!("{} 个月前", secs / 2_592_000),
        _ => format!("{} 年前", secs / 31_536_000),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_a_valid_document_with_its_layout() {
        for (i, tpl) in TEMPLATES.iter().enumerate() {
            let session = tpl.session();
            assert_eq!(
                session.doc.meta.layout.kind, tpl.layout,
                "模板 {i} 的布局种类要写进文档"
            );
            let root = &session.doc.nodes[&session.doc.root_id];
            match &tpl.shape {
                Shape::Tree { branches, .. } => {
                    assert_eq!(root.children.len(), branches.len(), "模板 {i} 的一级分支数")
                }
                // 概念图的外围概念是自由节点（`parent_id = None`），不挂在根上。
                Shape::Concept { .. } => {
                    assert!(root.children.is_empty(), "模板 {i} 根节点不挂子级")
                }
            }
            assert!(
                root.children
                    .iter()
                    .all(|id| session.doc.nodes.contains_key(id)),
                "模板 {i} 的父子 id 必须自洽"
            );
            assert!(session.doc.validate().is_ok(), "模板 {i} 结构校验");
            assert!(!session.dirty(), "模板刚建出来应当是干净文档");
            assert!(
                matches!(Template::at(i), Some(t) if t.name == tpl.name),
                "模板 {i} 要能按卡片下标取回"
            );
        }
        assert!(Template::at(TEMPLATES.len()).is_none());
    }

    #[test]
    fn templates_cover_six_structures_and_four_content_starters() {
        // `docs/02` §1：结构 6 张（1–6）+ 内容 4 张（7–10）。
        assert_eq!(TEMPLATES.len(), 10);
        assert_eq!(STRUCTURE_TEMPLATES, 6);
        let mut seen = std::collections::HashSet::new();
        for tpl in &TEMPLATES {
            assert!(seen.insert(tpl.name), "模板名重复：{}", tpl.name);
            assert!(!tpl.hint.is_empty(), "{} 缺卡片说明", tpl.name);
            assert!(!tpl.name.is_empty(), "模板名不能为空");
        }
        let content: Vec<&str> = TEMPLATES[STRUCTURE_TEMPLATES..]
            .iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(content, ["空白", "读书笔记", "会议记录", "产品规划"]);
        // 内容模板不按形状分类：四张都是 mindmap，让用户按「写什么」挑。
        assert!(TEMPLATES[STRUCTURE_TEMPLATES..]
            .iter()
            .all(|t| t.layout == "mindmap"));
    }

    /// 「空白」是真正的空文档：根没有占位文字，也没有子级。
    #[test]
    fn blank_content_template_is_a_really_empty_mindmap() {
        let tpl = TEMPLATES
            .iter()
            .find(|t| t.name == "空白")
            .expect("模板表里要有空白");
        let session = tpl.session();
        assert_eq!(session.doc.meta.layout.kind, "mindmap");
        assert_eq!(session.doc.nodes.len(), 1, "空白模板只有一个根节点");
        let root = &session.doc.nodes[&session.doc.root_id];
        assert_eq!(root.text, "", "根节点不带占位文字");
        assert!(root.children.is_empty(), "空白模板根下不挂子级");
        assert!(session.doc.validate().is_ok(), "空白模板结构要合法");
        assert!(!session.dirty());
    }

    fn branch_labels(tree: &[(String, Vec<String>)]) -> Vec<&str> {
        tree.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// 把一棵起手树摊平成（根文字，[(一级标题, [二级文字])]），断言不必摸 id。
    fn starting_tree(name: &str) -> (String, Vec<(String, Vec<String>)>) {
        let tpl = TEMPLATES.iter().find(|t| t.name == name).expect("模板存在");
        let session = tpl.session();
        let doc = &session.doc;
        let root = &doc.nodes[&doc.root_id];
        let branches = root
            .children
            .iter()
            .map(|id| {
                let branch = &doc.nodes[id];
                let kids = branch
                    .children
                    .iter()
                    .map(|k| doc.nodes[k].text.clone())
                    .collect();
                (branch.text.clone(), kids)
            })
            .collect();
        (root.text.clone(), branches)
    }

    #[test]
    fn content_templates_build_their_documented_starting_tree() {
        let (root, branches) = starting_tree("读书笔记");
        assert_eq!(root, "书名");
        assert_eq!(
            branch_labels(&branches),
            ["书目信息", "核心观点", "章节笔记", "摘录与金句", "行动"]
        );
        assert_eq!(branches[0].1, ["作者：", "出版社：", "读于："]);
        assert_eq!(branches[2].1, ["第 1 章", "第 2 章", "第 3 章"]);

        let (root, branches) = starting_tree("会议记录");
        assert_eq!(root, "会议主题");
        assert_eq!(branch_labels(&branches), ["与会人", "议题", "决议", "待办"]);
        assert!(
            branches[3].1.iter().any(|t| t.contains("负责人")),
            "待办必须带负责人占位"
        );

        let (root, branches) = starting_tree("产品规划");
        assert_eq!(root, "产品名称");
        assert_eq!(
            branch_labels(&branches),
            ["目标", "用户与场景", "里程碑", "风险"]
        );
        assert_eq!(branches[2].1.len(), 3, "里程碑留三格");

        // 三张有内容模板的每个一级分支都得有子项，不能只是空标签。
        for name in ["读书笔记", "会议记录", "产品规划"] {
            let (_, branches) = starting_tree(name);
            assert!(
                branches.iter().all(|(_, kids)| !kids.is_empty()),
                "{name} 有一级分支是空标签"
            );
        }
    }

    #[test]
    fn template_number_keys_cover_1_to_9_and_zero_for_the_tenth() {
        use egui::Key;
        assert_eq!(template_hotkey(0), Some(Key::Num1));
        assert_eq!(template_hotkey(8), Some(Key::Num9));
        assert_eq!(template_hotkey(9), Some(Key::Num0), "第 10 张用 0");
        assert_eq!(template_hotkey(TEMPLATES.len()), None, "越界没有键位");
        // 每张卡都要有键位，且互不重复——键帽上写的就是这个数。
        let keys: Vec<Key> = (0..TEMPLATES.len()).filter_map(template_hotkey).collect();
        assert_eq!(keys.len(), TEMPLATES.len());
        let unique: std::collections::HashSet<Key> = keys.iter().copied().collect();
        assert_eq!(unique.len(), keys.len(), "数字键位重复");
    }

    #[test]
    fn blank_session_is_pristine_but_templates_are_not() {
        assert!(Session::blank().is_pristine());
        assert!(!TEMPLATES[0].session().is_pristine());
    }

    #[test]
    fn layout_is_the_single_source_of_breakpoints() {
        let wide = layout(2000.0);
        assert_eq!(wide.page_w, MAX_W, "再宽也不超过 880");
        assert_eq!(wide.quick_cols, 4);
        assert_eq!(wide.quick_rows, 1);
        assert_eq!(wide.template_cols, 3);
        assert_eq!(wide.recent_rows, 8);
        // 副标题长短跟着卡片宽窄走：四列时卡片 ~211px，只能用短副标题；
        // 两列/一列卡片更宽，放得下带快捷键的长副标题。
        assert!(wide.compact_hints, "四列短副标题");
        assert!(wide.show_template_hint);
        assert!(wide.show_footer_keys);

        let three = layout(TEMPLATE_3COL_MIN);
        assert_eq!(three.template_cols, 3);
        assert_eq!(three.quick_cols, 4);
        let two = layout(TEMPLATE_3COL_MIN - 1.0);
        assert_eq!(two.template_cols, 2, "899 退回两列");
        assert_eq!(two.quick_cols, 2, "899 起快速开始改 2×2");
        assert_eq!(two.recent_rows, 5);
        assert!(!two.compact_hints, "两列卡片够宽，用完整副标题");
        let two_at = layout(TEMPLATE_2COL_MIN);
        assert_eq!(two_at.template_cols, 2);
        let one = layout(TEMPLATE_2COL_MIN - 1.0);
        assert_eq!(one.template_cols, 1, "639 只能一列");
        assert_eq!(one.quick_cols, 1);
        assert_eq!(one.quick_rows, 4);
        assert_eq!(one.recent_rows, 3);
        assert!(!one.compact_hints, "一列卡片最宽");
        assert!(!one.show_template_hint);
        assert!(!one.show_footer_keys);

        assert_eq!(layout(100.0).page_w, 240.0, "最小页宽 240");
    }

    /// 卡片宽度的下限与「不横向溢出」：任何断点下卡片都 ≥180，
    /// 且一整行（列宽 + 间距）不超页宽。窄窗里横向溢出就是「文字压边」的另一种写法
    /// （`docs/30` §3.2、§3.5 的 640 断点）。
    #[test]
    fn cards_stay_between_180_and_the_page_width() {
        let card = |lay: Layout, cols: usize| {
            ((lay.page_w - GAP * (cols as f32 - 1.0)) / cols as f32).max(180.0)
        };
        let row_w = |cols: usize, w: f32| w * cols as f32 + GAP * (cols as f32 - 1.0);
        for avail in [
            240.0_f32, 320.0, 480.0, 639.0, 640.0, 720.0, 899.0, 900.0, 1200.0, 1920.0,
        ] {
            let lay = layout(avail);
            for (name, cols, w) in [
                ("快速开始", lay.quick_cols, card(lay, lay.quick_cols)),
                ("模板", lay.template_cols, card(lay, lay.template_cols)),
            ] {
                assert!(w >= 180.0, "avail={avail} {name} 卡片 {w} 比 180 还窄");
                assert!(
                    row_w(cols, w) <= lay.page_w + 0.01,
                    "avail={avail} {name} 行宽 {} 溢出页宽 {}",
                    row_w(cols, w),
                    lay.page_w
                );
            }
        }
    }

    #[test]
    fn empty_state_and_recent_rows_have_fixed_heights() {
        assert_eq!(EMPTY_H, 76.0);
        assert_eq!(ROW_H, 44.0);
        assert_eq!(recents_height(1), 56.0);
        assert_eq!(recents_height(3), 144.0);
        assert_eq!(recents_height(8), 364.0);
    }

    #[test]
    fn slack_gaps_hand_back_exactly_the_slack() {
        // 没有余量时就是基础间距，不该偷偷加料。
        assert_eq!(slack_gaps(0.0), BASE_GAPS);
        assert_eq!(slack_gaps(-50.0), BASE_GAPS, "负余量夹到 0");

        for slack in [0.01_f32, 1.0, 37.5, 120.0, SLACK_MAX] {
            let g = slack_gaps(slack);
            let total: f32 = (0..4).map(|i| g[i] - BASE_GAPS[i]).sum();
            assert!(
                (total - slack).abs() < 0.05,
                "slack={slack} 摊出去的和是 {total}，必须等于余量本身"
            );
            for i in 0..4 {
                assert!(g[i] >= BASE_GAPS[i], "第 {i} 段只能加不能减");
            }
        }

        // 超过上限就封顶：页面再高也不该把段间距拉成一片草原。
        let capped = slack_gaps(SLACK_MAX * 5.0);
        assert_eq!(capped, slack_gaps(SLACK_MAX));

        // 大头在最后一段：提示行要落到视口近底部。
        let g = slack_gaps(SLACK_MAX);
        let last = g[3] - BASE_GAPS[3];
        let others: f32 = (0..3).map(|i| g[i] - BASE_GAPS[i]).sum();
        assert!(last > others, "末段余量 {last} 应大于前三段合计 {others}");
    }

    #[test]
    fn concept_template_builds_a_map_of_floating_concepts() {
        let tpl = TEMPLATES
            .iter()
            .find(|t| t.layout == "conceptMap")
            .expect("模板表里要有概念图");
        let session = tpl.session();
        let doc = &session.doc;
        assert_eq!(doc.meta.layout.kind, "conceptMap");
        assert!(doc.validate().is_ok(), "概念图结构要合法");
        let root = &doc.nodes[&doc.root_id];
        assert_eq!(root.text, "产品定位");
        let floats: Vec<_> = doc
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none() && n.id != doc.root_id)
            .collect();
        assert_eq!(floats.len(), 5, "中心概念外还有 5 个自由概念");
        assert!(floats.iter().all(|n| n.pos.is_some()), "概念都要有坐标");
        let rels: Vec<_> = doc
            .nodes
            .values()
            .filter(|n| !n.relationships.is_empty())
            .collect();
        let total: usize = rels.iter().map(|n| n.relationships.len()).sum();
        assert_eq!(total, 5, "5 条有向连接");
        assert!(
            rels.iter()
                .all(|n| n.relationships.iter().all(|r| r.label.is_some())),
            "每条连接都要有连接词"
        );
    }

    #[test]
    fn relative_time_buckets() {
        let now = 1_000_000_000_000i64;
        assert_eq!(relative_time(now - 5_000, now), "刚刚");
        assert_eq!(
            relative_time(now + 10_000, now),
            "刚刚",
            "时钟回拨不显示负时间"
        );
        assert_eq!(relative_time(now - 120_000, now), "2 分钟前");
        assert_eq!(relative_time(now - 7_200_000, now), "2 小时前");
        assert_eq!(relative_time(now - 172_800_000, now), "2 天前");
        assert_eq!(relative_time(now - 1_209_600_000, now), "2 周前");
        assert_eq!(relative_time(now - 7_776_000_000, now), "3 个月前");
        assert_eq!(relative_time(now - 94_608_000_000, now), "3 年前");
    }

    #[test]
    fn folder_hint_drops_the_file_name_and_never_panics() {
        assert_eq!(folder_hint(Path::new("C:/notes/a.zmind")), "C:/notes");
        assert_eq!(folder_hint(Path::new("a.zmind")), "此电脑");
    }

    #[test]
    fn long_titles_are_truncated_by_chars_not_bytes() {
        let s = "一二三四五六七八九十".repeat(6);
        let short = truncate(&s, 10);
        assert_eq!(short.chars().count(), 10);
        assert!(short.ends_with('…'));
    }

    #[test]
    fn scroll_thumb_hides_when_content_fits() {
        assert_eq!(scroll_thumb(500.0, 500.0, 0.0), None);
        assert_eq!(scroll_thumb(500.5, 500.0, 0.0), None, "1px 内算不溢出");
        assert_eq!(scroll_thumb(500.0, 0.0, 0.0), None, "视口高为 0 不画");
        assert!(scroll_thumb(501.1, 500.0, 0.0).is_some());
    }

    #[test]
    fn scroll_thumb_tracks_both_ends_and_min_length() {
        let top = scroll_thumb(1000.0, 500.0, 0.0).unwrap();
        assert_eq!(top.top, 0.0);
        assert!((top.len - 0.5).abs() < 1e-6);

        let bottom = scroll_thumb(1000.0, 500.0, 500.0).unwrap();
        assert!((bottom.top - 0.5).abs() < 1e-6, "滚到底滑块贴轨道底");

        let clamped = scroll_thumb(1000.0, 500.0, -40.0).unwrap();
        assert_eq!(clamped.top, 0.0, "越界 offset 夹回顶部");
        let overshoot = scroll_thumb(1000.0, 500.0, 900.0).unwrap();
        assert!((overshoot.top - 0.5).abs() < 1e-6, "越界 offset 夹到底部");

        // 内容越长滑块越短，但最短 28px 折算成比例。
        let long = scroll_thumb(100_000.0, 500.0, 0.0).unwrap();
        assert!((long.len - (SCROLL_BAR_MIN_H / 500.0)).abs() < 1e-6);
    }

    #[test]
    fn offset_for_thumb_top_round_trips() {
        let thumb = scroll_thumb(1400.0, 600.0, 320.0).unwrap();
        let offset = offset_for_thumb_top(thumb.top, 1400.0, 600.0);
        assert!((offset - 320.0).abs() < 1e-3, "拖动反算与当前 offset 一致");

        assert_eq!(offset_for_thumb_top(0.0, 1400.0, 600.0), 0.0);
        assert!((offset_for_thumb_top(1.0, 1400.0, 600.0) - 800.0).abs() < 1e-3);
        assert_eq!(offset_for_thumb_top(0.5, 500.0, 500.0), 0.0, "不溢出回 0");
    }

    fn viewport() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))
    }

    fn card(x: f32, y: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(200.0, h))
    }

    #[test]
    fn focus_scroll_leaves_visible_focus_alone() {
        let visible = card(20.0, 100.0, 60.0);
        assert_eq!(focus_scroll_target(visible, viewport(), 0.0, 900.0), None);
        assert_eq!(
            focus_scroll_target(visible, viewport(), 120.0, 900.0),
            None,
            "焦点已在视口内时，滚轮把页面拖到哪都不抢"
        );
    }

    #[test]
    fn focus_scroll_moves_just_enough_for_below_and_above() {
        // 卡片底边在视口下 30px：滚 30 + 8px 边距。
        let below = card(20.0, 380.0, 50.0);
        assert_eq!(
            focus_scroll_target(below, viewport(), 0.0, 900.0),
            Some(38.0)
        );
        // 上方被裁 20px：回滚 20 + 8px，不能过 0。
        let above = card(20.0, -20.0, 60.0);
        assert_eq!(
            focus_scroll_target(above, viewport(), 100.0, 900.0),
            Some(72.0)
        );
        assert_eq!(
            focus_scroll_target(above, viewport(), 20.0, 900.0),
            Some(0.0),
            "顶部空间不够时夹到 0"
        );
    }

    #[test]
    fn focus_scroll_clamps_and_handles_tall_cards() {
        // 内容 900 / 视口 400：最大偏移 500，目标不得越界。
        let below = card(20.0, 450.0, 60.0);
        assert_eq!(
            focus_scroll_target(below, viewport(), 480.0, 900.0),
            Some(500.0)
        );
        // 不溢出（1px 容差）或视口高为 0：不滚。
        assert_eq!(
            focus_scroll_target(card(20.0, 395.0, 5.0), viewport(), 0.0, 400.0),
            None
        );
        assert_eq!(
            focus_scroll_target(card(20.0, 395.0, 5.0), Rect::ZERO, 0.0, 900.0),
            None
        );
        // 卡片比视口高：顶边已经露出就不动，否则对齐到视口顶 +8。
        let tall = card(20.0, 120.0, 600.0);
        assert_eq!(
            focus_scroll_target(tall, viewport(), 0.0, 1200.0),
            None,
            "超高卡片顶边已可见"
        );
        let tall_below = card(20.0, 450.0, 600.0);
        assert_eq!(
            focus_scroll_target(tall_below, viewport(), 0.0, 1200.0),
            Some(442.0),
            "超高卡片顶边在视口外：对齐顶边"
        );
    }
}
