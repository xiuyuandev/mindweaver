//! 大纲面板：把文档结构竖着摊开（M3-c）。
//!
//! 三条性能纪律，直接决定 20k 节点还顺不顺：
//! 1. **行缓存只在 `doc.revision` / 主题变化时重建**（O(n) 一次），其余帧是一次 `u64` 比较；
//! 2. **只画可见行**：行矩形照常占位，像素工作（文字排版、层级线、角标）过裁剪就跳过；
//!    O08 再把实例化也收进视口；
//! 3. **不复制整棵树**：行里只存 id / 文本 / 深度 / 几个标志，没有 `Node` 的克隆。
//! 4. **筛选结果按下标缓存**：只有筛选词或行缓存变了才重扫全表；滚动帧不再对 20k 行做
//!    `to_lowercase`（那是每帧几万次堆分配，滚动会抖）。
//!
//! 全屏视图里正文是一列限宽 1040 的纸面，左右留白由 `column_margin` 统一算。

use std::collections::HashSet;

use eframe::egui::{
    self, Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2,
};
use mw_core::theme::{resolve_all, ResolvedStyle, Rgba, Theme};
use mw_core::{Command, Document, TaskState};

use crate::chrome::{c32, Chrome, Type};

/// 保留 alpha 的 Rgba→Color32：状态填充（hover 4% / selected 7%）靠 alpha 承载
/// 语义，丢掉就会变成实心强调色（CER-02 在 content_kit 里翻过同一辆车）。
pub(crate) fn c32a(c: mw_core::theme::Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

use super::focus::{self, FocusKey, FocusMemory, PanelId};

/// 大纲行 / 折叠箭头的稳定身份前缀：id 由「前缀 + 节点 id」派生，
/// 与行在滚动区里排第几无关——增删节点不会再顶走别行的焦点。
const ROW_KEY: &str = "outline/row";
const CHEV_KEY: &str = "outline/chev";

/// 面板重开时的兜底锚点。
///
/// 大纲的控件全是跟着文档走的动态行，没有一件「永远在那儿」的固定锚点，所以这里给的是
/// 一个通常不会命中的键：真正兜底的是 `FocusMemory` 的语义回落（回落到同前缀的行）。
pub fn anchor() -> FocusKey {
    FocusKey::new("outline/anchor")
}
/// 折叠箭头占的横向空间（没有子节点也留着，让标题左缘对齐）。
const CHEVRON: f32 = 22.0;
const PAD_L: f32 = 12.0;
/// docs/28 §5.6：悬停 120ms 后才露拖拽握把，避免指针扫过时满屏握把闪烁。
const GRIP_DELAY: std::time::Duration = std::time::Duration::from_millis(120);
/// 正文栏最大宽度：docs/28 §4.2 版心 800（正文宽 760），窄窗左右至少留 32。
const COL_MAX: f32 = 800.0;
const COL_MARGIN: f32 = 32.0;

/// 大纲密度。行高与字号一起变，避免「行距大了字还是小」的假宽松。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OutlineDensity {
    Compact,
    #[default]
    Standard,
    Comfortable,
}

/// 标签弹层的高光绕行：`n` 是「全部标签 + 各标签」的总格数，
/// 空列表时也必须是 1（只有「全部标签」），否则取模会炸。
fn label_hl_step(cur: usize, n: usize, up: bool) -> usize {
    let n = n.max(1);
    if up {
        (cur + n - 1) % n
    } else {
        (cur + 1) % n
    }
}
impl OutlineDensity {
    // docs/28 §5.2 固定行几何：32px 行、24px 缩进、16px 正文（depth 0 是页面标题，
    // 由 row_font_id 走 28/600）。密度三档已按 CER-05 裁决移除；枚举与
    // set_density/MW_OUTLINE_DENSITY 保留为 no-op，旧门脚本在 CER-12 新门接管前可跑。
    pub fn row_h(self) -> f32 {
        let _ = self;
        32.0
    }

    pub fn indent(self) -> f32 {
        let _ = self;
        24.0
    }

    pub fn font(self, depth: u16) -> f32 {
        let _ = (self, depth);
        16.0
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "紧凑",
            Self::Standard => "标准",
            Self::Comfortable => "宽松",
        }
    }

    /// 落盘用序号：旧存档读到未知值时回落标准，而不是让整份窗口状态解析失败。
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Compact => 0,
            Self::Standard => 1,
            Self::Comfortable => 2,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Compact,
            2 => Self::Comfortable,
            _ => Self::Standard,
        }
    }
}

/// 行尾角标：任务方框、备注/卡片点、标签计数，右对齐成一列。
/// 文字裁剪与角标绘制共用 [`marker_reserve`]，长标题不会钻到角标底下。
/// docs/28 §5.2：任务框 14×14 行内；右侧指示单个 5px 圆点。
const MARK_DOT: f32 = 5.0;
const MARK_TASK: f32 = 14.0;
/// 任务框与文字的间距（框 14 + 4）。
const TASK_INLINE_STEP: f32 = 18.0;
/// 折叠子项数与文字尾部的间隙（含数字宽度的保守预留）。
const CHIP_STEP: f32 = 22.0;
/// 行尾「更多」动作道（docs/28 §5.2：16px `⋯`，距列右 8）。
/// 24px 始终占位、悬停/选中才画：否则指针一进行，长标题会当场重排。
const MORE_LANE: f32 = 24.0;
/// `⋯` 三颗点：半径 1.4、步距 6，总宽约 16px。
const MORE_DOT_R: f32 = 1.4;
const MORE_DOT_STEP: f32 = 6.0;

/// 中央留白：宽屏居中，窄屏至少留 28px，标题栏与正文用同一口径对齐。
pub(crate) fn column_margin(width: f32) -> f32 {
    ((width - COL_MAX) * 0.5).max(COL_MARGIN)
}

/// 大纲行握着键盘时锁住的键：Tab 是缩进、方向键是走行、Esc 是退出视图。
/// egui 默认把这些键当「移动焦点」，而判定发生在本帧读键盘之前，必须锁在行上。
/// 与画布的 `canvas_key_lock` 同一口径。
fn outline_key_lock() -> egui::EventFilter {
    egui::EventFilter {
        tab: true,
        escape: true,
        horizontal_arrows: true,
        vertical_arrows: true,
    }
}

/// 本帧是否有会被 egui 当成「移动焦点」的按键：Tab（含 Shift+Tab）与无修饰的方向键。
/// Ctrl+Shift+↑/↓（重排）不在此列——带修饰的方向键不触发焦点巡游。
fn outline_nav_pressed(i: &egui::InputState) -> bool {
    i.events.iter().any(|e| match e {
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => match key {
            Key::Tab => true,
            Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight => !modifiers.any(),
            _ => false,
        },
        _ => false,
    })
}

/// 行表是否拥有本帧键盘：前台浮层（模态/菜单）、行内编辑、搜索框任一在场，
/// 大纲都只画不收键（docs/24 门后维护 ⑪）。
fn row_keys_owned(blocked: bool, editing: bool, filter_owned: bool) -> bool {
    !blocked && !editing && !filter_owned
}

/// 一行大纲。字段越少越好——5000 行时每个多余字段都是 5000 份。
/// `branch`/`guide_mask`/`labels` 在 CER-05 行绘制 v2 后暂无绘制消费者
/// （分支色/树线/标签胶囊已按 docs/28 §5.2/§5.9 移除）；数据保留，
/// 供 CER-06 状态底色与检查器路径使用。
#[allow(dead_code)]
struct Row {
    id: String,
    text: String,
    depth: u16,
    has_children: bool,
    collapsed: bool,
    /// 直接子节点数：折叠时当计数徽标用，比一个「…」有信息量。
    kids: u16,
    /// 父节点 id（根与自由主题为 None）。拖拽重排要靠它算「插到谁的后面」。
    parent: Option<String>,
    /// 在兄弟里的下标（根为 0）。拖拽落点的 index 由它推出来。
    index: usize,
    /// 所属分支色：箭头/叶点按它染色，让大纲与画布的色系对齐。
    branch: Rgba,
    /// 解析后的文字色（节点覆盖 → 分支/层级继承）。
    text_color: Rgba,
    /// 解析后的字重/斜体。
    bold: bool,
    italic: bool,
    /// 节点显式字号覆盖；`None` = 跟随大纲层级字号。
    font_override: Option<f32>,
    /// 是否为本层最后一个孩子：决定父级竖线在行中心收笔还是继续向下。
    last: bool,
    /// 哪些祖先层还有后续兄弟（bit = 深度）。只画这些竖线，行尾不再无尽延伸。
    guide_mask: u64,
    /// 有纯文本备注（检查器里写的备注；只有空白不算）。
    has_note: bool,
    /// 备注首行预览：说明行 13/20 画在正文下方（docs/28 §5.2，行高 +20）。
    note_preview: String,
    /// 挂了卡片笔记（卡片本身在 `Document.cards`）。
    has_card: bool,
    /// 标签数：行尾显示 `#n`。
    labels: u8,
    /// 任务状态：行尾方框（未开始 / 进行中 / 完成）。
    task: Option<TaskState>,
    /// 概念图专用：进来那条连线的**链接短语**（CM-07）。行文字仍是概念本身，
    /// 短语只画成淡色前缀，改名只改概念、不碰短语。
    via: Option<String>,
    /// 非编辑态正文实测的显示行数（docs/28 §5.2 换行 +24）；1 = 不换行。
    /// `measure_wraps` 在行表或列宽变化后重算，虚拟化前缀高度与绘制共用这一份。
    text_rows: u16,
    /// 链接短语前缀的实测宽度（px）；`via` 为空时是 0。测量与绘制共用，
    /// 免得「量的宽度」和「画的宽度」差几像素、长概念名在边界上多折一行。
    via_w: f32,
    /// 进来那条边的 id（CM-09）：断边、改短语都只能按 id 点名。
    via_id: Option<String>,
    /// 进来那条边的起点概念 id：`DeleteRelationship{from, id}` 要它。
    via_from: Option<String>,
}

/// 行尾角标占用的横向宽度。折叠计数徽标另算（见 `CHIP_STEP`）。
fn marker_reserve(row: &Row) -> f32 {
    // docs/28 §5.2：行尾只剩单个 5px 圆点（卡片/备注）；任务框在行内。
    if row.has_note || row.has_card {
        MARK_DOT + 10.0
    } else {
        0.0
    }
}

/// 一行的显示行数（docs/28 §5.2）：编辑态按草稿行数，非编辑态按实测换行行数。
fn row_lines(row: &Row, edit_lines: Option<usize>) -> usize {
    edit_lines
        .unwrap_or_else(|| row.text_rows.max(1) as usize)
        .max(1)
}

/// 一行的占位高度（docs/28 §5.2/§5.4）：根 46、说明 +22、每多一行 +26。
/// 虚拟化按同一把尺子算前缀高度，保证「滚到哪、内容就在哪」。
fn row_layout_height(row: &Row, row_h: f32, edit_lines: Option<usize>) -> f32 {
    let base = if row.depth == 0 {
        46.0
    } else if row.has_note {
        row_h + 22.0
    } else {
        row_h
    };
    base + 26.0 * row_lines(row, edit_lines).saturating_sub(1) as f32
}

/// 正文左内距（docs/28 §5.2）：握把/圆点的缩进之后，正文从圆点右侧 12px 起。
/// 与绘制里的 `text_x` 同源；`via` 前缀与任务框另加（见 [`text_left`]）。
fn text_inset(row: &Row, indent: f32) -> f32 {
    PAD_L + row.depth as f32 * indent + 16.0
}

/// 正文起笔的横向内距：任务框 18、概念图短语前缀 +8 间隔。
fn text_left(row: &Row, indent: f32) -> f32 {
    text_inset(row, indent)
        + if row.task.is_some() {
            TASK_INLINE_STEP
        } else {
            0.0
        }
        + if row.via.is_some() {
            row.via_w + 8.0
        } else {
            0.0
        }
}

/// 正文可用宽度：列宽减去左边内距、行尾角标/折叠计数与「更多」动作道。
/// 换行测量与绘制共用同一把尺子，否则边界上的长行会「量 2 行、画 3 行」。
fn text_avail(row: &Row, col_w: f32, indent: f32) -> f32 {
    let chip = if row.collapsed && row.has_children {
        CHIP_STEP
    } else {
        0.0
    };
    (col_w - 8.0 - MORE_LANE - chip - marker_reserve(row) - text_left(row, indent)).max(1.0)
}

/// 行尾 `⋯` 的点击域：24×28，距列右 8，垂直对齐首行文字带。
/// 与绘制同源；测试直接钉几何，避免「画的」和「点的」差几像素。
fn row_more_rect(rect: Rect, row_cy: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(rect.right() - 8.0 - MORE_LANE, row_cy - 14.0),
        Pos2::new(rect.right() - 8.0, row_cy + 14.0),
    )
}

/// 行菜单作用于谁：目标已在一组多选里就整组，否则只作用这一行。
pub(crate) fn row_menu_multi(selected: &[String], target: &str) -> bool {
    selected.len() > 1 && selected.iter().any(|id| id == target)
}

/// 非编辑态正文的排版任务（docs/28 §5.2）：换行、不截断、悬挂缩进。
///
/// `line_height` 只在真的换行时给 26——单行仍用字体行高，既有基线零漂移；
/// 换行行按合同的「每行 +26」走，多行才不会比行框高出 2px/行。
fn row_text_job(
    label: String,
    font: egui::FontId,
    color: Color32,
    width: f32,
    italic: bool,
    lines: u16,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(label, font, color, width);
    job.wrap.max_rows = usize::MAX;
    job.wrap.break_anywhere = true;
    job.sections[0].format.italics = italic;
    if lines > 1 {
        job.sections[0].format.line_height = Some(26.0);
    }
    job
}

/// 命中区间在已排版文本上的底带（docs/28 §5.7，12% 强调色）。
/// 坐标系是 galley 局部：换行行按行拆段，调用方再加上 `(text_x, text_top)`。
fn hit_bands(galley: &egui::Galley, start: usize, end: usize) -> Vec<Rect> {
    let mut out = Vec::new();
    if start >= end {
        return out;
    }
    let mut seen = 0usize;
    for row in &galley.rows {
        // 段落换行符不进 glyphs，但算一个字符，否则下标会越走越偏。
        let chars = row.glyphs.len() + usize::from(row.ends_with_newline);
        let row_start = seen;
        let row_end = seen + chars;
        seen = row_end;
        // 空段落（只有换行符）的字符已经记进 `seen`，但没有字可量。
        if row.glyphs.is_empty() {
            continue;
        }
        if row_end <= start {
            continue;
        }
        if row_start >= end {
            break;
        }
        let x0 = if start <= row_start {
            0.0
        } else {
            let g = &row.glyphs[(start - row_start).min(row.glyphs.len() - 1)];
            g.pos.x
        };
        let x1 = if end >= row_end {
            row.size.x
        } else {
            let g = &row.glyphs[(end - row_start - 1).min(row.glyphs.len() - 1)];
            g.pos.x + g.advance_width
        };
        let top = row.pos.y + 2.0;
        let bottom = row.pos.y + row.size.y - 4.0;
        if x1 > x0 {
            out.push(Rect::from_min_max(
                Pos2::new(x0, top),
                Pos2::new(x1, bottom.max(top + 1.0)),
            ));
        }
    }
    out
}

/// 行表展平的显式栈项：(id, 深度, 兄弟下标, 兄弟数, 祖先竖线掩码)。
type RowStack = Vec<(String, u16, usize, usize, u64)>;

/// 把 `stack` 里的节点按前序摊成行。`rebuild` 与「展开时补行」共用这一份，
/// 保证增量补出来的行与全量重建逐字段一致。
#[allow(clippy::too_many_arguments)]
fn collect_rows(
    styles: &std::collections::HashMap<String, ResolvedStyle>,
    doc: &Document,
    theme: &Theme,
    focus: Option<&str>,
    rows_all: bool,
    stack: &mut RowStack,
    floats: &[String],
    out: &mut Vec<Row>,
) {
    while let Some((id, depth, index, siblings, guide_mask)) = stack.pop() {
        let Some(n) = doc.node(&id) else {
            continue;
        };
        let is_focus_root = focus == Some(n.id.as_str());
        let resolved = styles.get(&n.id);
        let branch = resolved.map(|s| s.branch_color).unwrap_or(theme.ui.accent);
        // 视图根（文档根 / 聚焦根 / 自由主题）没有同级：竖线不向下延伸。
        let last = depth == 0 || index + 1 >= siblings;
        let font_override = n.style.as_ref().and_then(|s| s.font_size).map(f32::from);
        let want = resolved.map(|s| s.text).unwrap_or(theme.ui.text);
        let text_color = if contrast32(c32(want), c32(theme.ui.panel)) < 3.0 {
            theme.ui.text
        } else {
            want
        };
        out.push(Row {
            id: n.id.clone(),
            text: n.text.clone(),
            depth,
            has_children: !n.children.is_empty(),
            collapsed: n.collapsed,
            kids: n.children.len().min(u16::MAX as usize) as u16,
            // 聚焦根在这个视图里没有同级：拖拽落在它前/后都按「挂成子节点」处理。
            parent: if is_focus_root {
                None
            } else {
                n.parent_id.clone()
            },
            index: if is_focus_root { 0 } else { index },
            branch,
            text_color,
            bold: resolved.is_some_and(|s| s.bold),
            italic: resolved.is_some_and(|s| s.italic),
            font_override,
            last,
            guide_mask,
            has_note: n.note.as_deref().is_some_and(|s| !s.trim().is_empty()),
            note_preview: n
                .note
                .as_deref()
                .and_then(|s| s.lines().map(str::trim).find(|l| !l.is_empty()))
                .unwrap_or_default()
                .to_owned(),
            has_card: n.card_id.is_some(),
            labels: n.labels.len().min(u8::MAX as usize) as u8,
            task: n.task,
            via: None,
            text_rows: 1,
            via_w: 0.0,
            via_id: None,
            via_from: None,
        });
        if n.collapsed && !rows_all {
            continue;
        }
        // 这一行还有后续兄弟 → 它所在的父级中轴要穿过它的整棵子树，直到下一个兄弟。
        // 中轴画在父级项目符号的 x 上，也就是 depth-1 层；根没有上一层可续。
        let child_guide = guide_mask
            | if !last && depth > 0 && depth <= 64 {
                1u64 << (depth - 1)
            } else {
                0
            };
        // 栈是后进先出：先把自由主题压底，再把子节点倒序压在上面，
        // 出栈顺序就与 `doc_order` 一致（根子树在前、自由主题在后）。
        if depth == 0 && id == doc.root_id {
            // 自由主题各自是一棵独立的树：不作为彼此的兄弟画竖线。
            for (i, f) in floats.iter().enumerate().rev() {
                stack.push((f.clone(), 0, i, 1, 0));
            }
        }
        for (i, child) in n.children.iter().enumerate().rev() {
            stack.push((child.clone(), depth + 1, i, n.children.len(), child_guide));
        }
    }
}

pub enum Action {
    /// 点了一行：选中并把画布移过去。
    Select(String),
    /// Shift/Ctrl 点行：加选/减选。
    ToggleSelect(String),
    /// 区间选择（Shift+点击 / Shift+↑↓ / Ctrl+A / Esc 收敛）：
    /// `ids` 按可见顺序排列，`primary` 是键盘当前行。
    SetSelection {
        ids: Vec<String>,
        primary: String,
    },
    /// 点了折叠箭头。
    Toggle(String),
    /// 双击改名提交（空文字也允许，画布会显示「未命名」）。
    Rename {
        id: String,
        text: String,
    },
    /// 概念图（CM-07）：Enter 新建一个概念（`AddFloatingTopic`）。
    /// 命题网里没有「子主题」——加边必须先有链接短语（`docs/30` §5.9），
    /// 那要画布上的短语编辑态，大纲只负责加「点」。
    NewConcept(String),
    /// 概念图 `Tab`：新建概念**并连一条边**（docs/30 §5.6.1）。
    LinkConcept(String),
    /// 概念图 `Shift+Delete`：只断这行「进来那条边」，概念留成孤立概念。
    UnlinkConcept {
        from: String,
        rel: String,
    },
    /// 概念图：请壳层在这一行立起短语编辑框（点行首前缀 / `Shift+F2`）。
    /// 走 Action 而不是直接改面板状态：绘制与按键两处都握着 `&Row`，
    /// 就地改 `self.via_edit` 会和行借用打架。
    EditVia(String),
    /// 概念图短语编辑提交：`None` = 清掉短语（边保留）。
    SetVia {
        from: String,
        rel: String,
        label: Option<String>,
    },
    /// Enter 在文字末尾：先提交文字，再新建同级/子级并立即编辑新行。
    /// 两条命令分开记历史：撤销先退文字，再退空行。`changed=false` 时不写 SetText。
    CommitAndAdd {
        id: String,
        text: String,
        changed: bool,
        child: bool,
    },
    /// Enter 在文字中间：拆成两个同级；一条 Batch 一次撤销。
    SplitSibling {
        id: String,
        head: String,
        tail: String,
    },
    /// Ctrl+Enter 在文字中间：后半段拆成子级；一条 Batch 一次撤销。
    SplitChild {
        id: String,
        head: String,
        tail: String,
    },
    /// Backspace 在文字开头：并入上一同级末尾；一条 Batch 一次撤销。
    MergePrev {
        id: String,
        text: String,
    },
    /// 空行按 Backspace/Delete：删掉节点，焦点落到上一可见行。
    DeleteEmpty {
        id: String,
        prev: Option<String>,
    },
    /// 拖拽重排：把 id 挂到 new_parent 下、第 index 位（None = 追加到末尾）。
    Move {
        id: String,
        new_parent: String,
        index: Option<usize>,
    },
    /// 多选拖拽：整组按可见顺序挂到同一落点；一条 Batch 一次撤销。
    MoveMany {
        ids: Vec<String>,
        new_parent: String,
        index: Option<usize>,
    },
    /// 多选缩进：从下往上，每个节点挂到上一个兄弟下；一次撤销。
    IndentMany(Vec<String>),
    /// 多选升级：从上往下，每个节点升到父级后面；一次撤销。
    OutdentMany(Vec<String>),
    /// 多选上/下移：按方向从边缘向内处理；一次撤销。
    MoveStepMany {
        ids: Vec<String>,
        delta: i32,
    },
    /// 多选删除：只删结构根（祖先已选时不重复删子树）；删完焦点落到删除区之后。
    DeleteMany(Vec<String>),
    /// 多选任务：同一状态一次写给整组；一次撤销。
    SetTaskMany {
        ids: Vec<String>,
        task: Option<TaskState>,
    },
    /// Ctrl+D：原地复制多选子树，副本插在原节点之后。
    Duplicate,
    /// Enter：新建同级（根节点由画布退化为子节点）。
    AddSibling(String),
    /// Ctrl+Enter：新建子节点。
    AddChild(String),
    /// 编辑态多行粘贴：本行文本 + 余行拆成同级；一条 Batch 一次撤销。
    PasteLines {
        id: String,
        text: String,
        siblings: Vec<String>,
    },
    /// Tab / Shift+Tab：降级 / 升级。
    Indent(String),
    Outdent(String),
    /// Delete：删除节点及其子树。
    Delete(String),
    /// Ctrl+Shift+↑/↓：在兄弟里重排。
    MoveStep {
        id: String,
        delta: i32,
    },
    /// 点行尾任务方框：按 待办 → 进行中 → 完成 → 取消标记 循环。
    SetTask {
        id: String,
        task: Option<TaskState>,
    },
    /// 点行尾卡片点：打开这个节点挂的卡片笔记。
    OpenCard(String),
    /// 点行尾备注点：画布选中该主题，切到检查器的备注页并聚焦输入框。
    OpenNote(String),
    /// Alt+Enter：聚焦这一节点，只显示它和它的后代。
    Focus(String),
    /// 面包屑首级「大纲」/ 聚焦态按 Esc：回到全图。
    Unfocus,
    /// Esc / 返回：退出大纲视图。
    Close,
    /// Ctrl+Shift+F / 页头搜索：展开或收起搜索行。
    ToggleSearch,
    /// 页头折叠菜单：展开全部 / 折叠全部（根节点由核心保证保持展开）。
    SetCollapsedAll {
        collapsed: bool,
    },
    /// 页头折叠菜单：只展开到第 N 层（根为第 0 层，始终展开）。
    SetCollapsedLevel(u8),
    /// 页头密度菜单：紧凑 / 标准 / 宽松；壳层负责落盘。
    SetDensity(OutlineDensity),
    /// 页脚 `?` / `…` 菜单：打开大纲快捷键表。
    OpenHelp,
    /// `…` 菜单：文档信息。
    ShowInfo,
    /// 行尾 `⋯` / 行右键：请壳层在这一行弹出菜单。
    RowMenu {
        id: String,
        pos: egui::Pos2,
    },
}

/// 行菜单动作（docs/06 §6「大纲行」）。
/// 只收大纲这一面看得见结果的动作：画布专属项（关系线/卡片/概要/边界/样式）不进来。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowAction {
    AddChild,
    AddSibling,
    EditText,
    Cut,
    Copy,
    Paste,
    Delete,
    ToggleCollapse,
    Focus,
    /// 思维导图：把这一行提为中心主题（M31）。
    SetRoot,
    CollapseSelected,
    MoveSameParent,
    /// 概念图（C20）：新建概念并连一条进来的边（与 `Tab` 同一动作）。
    LinkConcept,
    /// 概念图：只加概念、不加边（与 `Enter` 同一动作）。
    NewConcept,
    /// 概念图：就地改「进来那条边」的短语（与 `Shift+F2` 同一动作）。
    EditVia,
    /// 概念图：只断「进来那条边」（与 `Shift+Delete` 同一动作）。
    Unlink,
}

/// 概念图大纲行菜单的条目集（C20）。
///
/// 只收命题网语义：建边 / 加概念 / 改短语 / 断边 / 重命名 / 复制 / 删除。
/// 树级项（新建子主题或同级、折叠、聚焦、升级/降级）一条都不进来——
/// `docs/30` §5.2/§5.7 禁止把 `children` 语义漏进概念图。
///
/// 多选时只留「复制 / 删除概念」两项：建边、改短语、断边、重命名都只对**一行**成立，
/// 在多选里列出来会让人以为对整组生效（`multi` 的删除本来就是整组，见
/// `run_outline_row_action` 的 `R::Delete` 分支）。
pub(crate) fn concept_row_items(
    has_edge: bool,
    can_delete: bool,
    multi: bool,
) -> Vec<crate::menu::Entry<RowAction>> {
    use crate::menu::Entry;
    if multi {
        return vec![
            Entry::action("复制", "Ctrl+C", RowAction::Copy, true),
            Entry::Separator,
            Entry::action("删除概念", "Delete", RowAction::Delete, can_delete),
        ];
    }
    vec![
        Entry::action("新建概念并连线", "Tab", RowAction::LinkConcept, true),
        Entry::action("新建概念", "Enter", RowAction::NewConcept, true),
        Entry::Separator,
        Entry::action("编辑连接短语", "Shift+F2", RowAction::EditVia, has_edge),
        Entry::action("断开进入的边", "Shift+Delete", RowAction::Unlink, has_edge),
        Entry::Separator,
        Entry::action("重命名概念", "F2", RowAction::EditText, true),
        Entry::action("复制", "Ctrl+C", RowAction::Copy, true),
        Entry::action("删除概念", "Delete", RowAction::Delete, can_delete),
    ]
}

/// 挂起的行菜单：目标行、是否作用于整组，以及菜单引擎本身。
pub(crate) struct RowMenu {
    pub(crate) target: String,
    pub(crate) multi: bool,
    pub(crate) menu: crate::menu::Menu<RowAction>,
}

/// 整份文档的静态统计。随行缓存一起重建，页脚和「文档信息」不必每帧扫全树。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutlineStats {
    pub total: usize,
    pub done: usize,
    pub notes: usize,
    pub cards: usize,
    pub labels: usize,
}

/// 到大纲视图根（聚焦根或文档根）的层数；根自己为 0。不在根下面时返回 `None`。
fn depth_below(doc: &Document, base: &str, id: &str) -> Option<usize> {
    let mut depth = 0;
    let mut cur = id;
    loop {
        if cur == base {
            return Some(depth);
        }
        let node = doc.node(cur)?;
        cur = node.parent_id.as_deref()?;
        depth += 1;
        if depth > 4096 {
            return None;
        }
    }
}

/// 「只显示到第 N 层」：第 N 层继续展开，第 N 层以下有孩子的节点全部折叠。
///
/// 根（或聚焦根）始终展开，所以 N=1 的实际可见范围是根 + 第一层子节点——这是
/// 「根保持展开」这条硬约束下唯一自洽的解释。所有改动装进一个 `Batch`，一次撤销。
pub(crate) fn collapse_level_command(
    doc: &Document,
    base: Option<&str>,
    level: u8,
) -> Option<Command> {
    let base = base.unwrap_or(doc.root_id.as_str());
    let level = level.clamp(1, 32) as usize;
    let mut commands = Vec::new();
    for (id, node) in &doc.nodes {
        if node.children.is_empty() {
            continue;
        }
        let Some(depth) = depth_below(doc, base, id) else {
            continue;
        };
        if depth == 0 {
            continue;
        }
        let collapsed = depth >= level;
        if node.collapsed != collapsed {
            commands.push(Command::SetCollapsed {
                id: id.clone(),
                collapsed,
            });
        }
    }
    if commands.is_empty() {
        None
    } else {
        Some(Command::Batch { commands })
    }
}

/// 大纲过滤：忽略大小写、去首尾空白的子串匹配。空查询 = 全显示。
fn matches_filter(text: &str, needle: &str) -> bool {
    needle.is_empty() || text.to_lowercase().contains(needle)
}

/// 筛选条上的条件胶囊（22×22 热区）：选中给 12% 强调底，悬停给 hover 底。
fn filter_chip(
    ui: &mut egui::Ui,
    cx: f32,
    cy: f32,
    id: &'static str,
    active: bool,
    focused: bool,
    ch: Chrome,
) -> (egui::Response, Rect) {
    let fill = Color32::from_rgba_unmultiplied(ch.accent.r(), ch.accent.g(), ch.accent.b(), 30);
    let r = Rect::from_center_size(Pos2::new(cx, cy), Vec2::splat(22.0));
    let hit = ui.interact(
        r,
        egui::Id::unique(("mw-outline-filter", id)),
        Sense::click(),
    );
    if active {
        ui.painter().rect_filled(r, 5.0, fill);
    } else if focused || hit.hovered() {
        ui.painter().rect_filled(r, 5.0, ch.hover);
    }
    if focused {
        ui.painter().rect_stroke(
            r,
            5.0,
            Stroke::new(1.5, ch.accent),
            egui::StrokeKind::Inside,
        );
    }
    if hit.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    (hit, r)
}

/// 任务方框的单击循环：待办 → 进行中 → 完成 → 取消标记（再点回待办）。
/// 第四态必须存在——否则标记一旦加上，只能去检查器里才能摘掉。
fn next_task(task: Option<TaskState>) -> Option<TaskState> {
    match task {
        None => Some(TaskState::Todo),
        Some(TaskState::Todo) => Some(TaskState::Doing),
        Some(TaskState::Doing) => Some(TaskState::Done),
        Some(TaskState::Done) => None,
    }
}

/// 行尾角标。热区比图形大，相邻两点会叠在一起，命中要靠中心距离裁决。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Task,
    Note,
    Card,
}

/// 指针落在哪个角标上：热区重叠时取中心最近的那个。
/// 悬停提示与单击分派共用这一套判定，避免「看到卡片提示、点下去却开了备注」。
fn marker_at(
    pos: Option<Pos2>,
    task_hit: Option<Rect>,
    note_hit: Option<Rect>,
    card_hit: Option<Rect>,
) -> Option<Mark> {
    let p = pos?;
    let mut best: Option<(f32, Mark)> = None;
    for (mark, hit) in [
        (Mark::Task, task_hit),
        (Mark::Note, note_hit),
        (Mark::Card, card_hit),
    ] {
        let Some(hit) = hit else { continue };
        if !hit.contains(p) {
            continue;
        }
        let d = hit.center().distance(p);
        if best.is_none_or(|(best_d, _)| d < best_d) {
            best = Some((d, mark));
        }
    }
    best.map(|(_, mark)| mark)
}

/// 角标悬停文案。任务点随四态给出下一步动作，备注/卡片说明单击后果。
fn marker_tip(mark: Mark, task: Option<TaskState>) -> Option<&'static str> {
    match mark {
        Mark::Task => Some(match task {
            Some(TaskState::Todo) => "任务：待办 · 单击切换",
            Some(TaskState::Doing) => "任务：进行中 · 单击切换",
            Some(TaskState::Done) => "任务：完成 · 单击清除",
            None => return None,
        }),
        Mark::Note => Some("备注 · 单击在检查器中编辑"),
        Mark::Card => Some("卡片笔记 · 单击打开"),
    }
}

/// 行内点击分派：角标热区优先于整行选择；指针不在任何热区里返回 `None`。
/// 角标与行响应共用一次点击，没命中的部分才归整行。
fn marker_click(
    pos: Option<Pos2>,
    task: Option<TaskState>,
    task_hit: Option<Rect>,
    note_hit: Option<Rect>,
    card_hit: Option<Rect>,
    id: &str,
) -> Option<Action> {
    match marker_at(pos, task_hit, note_hit, card_hit)? {
        Mark::Task => task.map(|task| Action::SetTask {
            id: id.to_string(),
            task: next_task(Some(task)),
        }),
        Mark::Note => Some(Action::OpenNote(id.to_string())),
        Mark::Card => Some(Action::OpenCard(id.to_string())),
    }
}

/// 大纲行的键位语义。抽成枚举是为了让「回车 / Tab / 方向键」的优先级
/// 能被单测钉住，而不是散在一串 `else if` 里靠人眼复核。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowKey {
    MoveUp,
    MoveDown,
    AddSibling,
    Outdent,
    Indent,
    Delete,
    Rename,
    /// Shift+Delete：只断「进来那条边」（概念图，docs/30 §5.6.1）。
    Unlink,
    /// Shift+F2：就地改链接短语（概念图）。
    RenameVia,
    Prev,
    Next,
    Collapse,
    Expand,
    Toggle,
    /// Ctrl+T：切换任务状态（没有标记就新建待办）。
    Task,
    /// Alt+Enter：聚焦这一节点的子树。
    Focus,
    /// Ctrl+[：退出聚焦（非聚焦态空操作）。
    UnfocusHere,
    /// Alt+Shift+.：全部折叠/展开（按当前是否有展开行取反）。
    ExpandAllToggle,
    /// Shift+↑：以锚点为界向上扩展多选。
    SelectPrev,
    /// Shift+↓：以锚点为界向下扩展多选。
    SelectNext,
    /// Ctrl+A：全选当前可见行（不跨折叠）。
    SelectAllVisible,
    Close,
}

/// 单个按键 → 大纲语义。修饰键按**精确匹配**判定：
/// · Alt+Enter 在大纲里是「聚焦子树」（与画布同一手势）；其余 Alt 组合一律不认领；
/// · Shift+Tab 是升级，绝不能被「忽略多余 Shift」的逻辑吃成缩进；
/// · Shift+方向键、Ctrl+Delete 等未定义组合一律不认领，留给别处。
fn row_key(mods: Modifiers, key: Key) -> Option<RowKey> {
    if mods.mac_cmd {
        return None;
    }
    if mods.alt {
        // docs/28 §5.5：Alt+. 折叠/展开当前；Alt+Shift+. 全部折叠/展开；
        // Alt+Enter 聚焦子树（与画布同一手势）。其余 Alt 组合不认领。
        if mods.ctrl {
            return None;
        }
        return match (mods.shift, key) {
            (false, Key::Enter) => Some(RowKey::Focus),
            (false, Key::Period) => Some(RowKey::Toggle),
            (true, Key::Period) => Some(RowKey::ExpandAllToggle),
            _ => None,
        };
    }
    match (mods.ctrl, mods.shift, key) {
        (true, true, Key::ArrowUp) => Some(RowKey::MoveUp),
        (true, true, Key::ArrowDown) => Some(RowKey::MoveDown),
        // docs/28 §5.5：Ctrl+Enter 切换任务完成（旧「加子级」作废，走 Tab 后输入）。
        (true, false, Key::Enter) => Some(RowKey::Task),
        (true, false, Key::T) => Some(RowKey::Task),
        (true, false, Key::CloseBracket) => Some(RowKey::Focus),
        (true, false, Key::OpenBracket) => Some(RowKey::UnfocusHere),
        (true, false, Key::A) => Some(RowKey::SelectAllVisible),
        (false, false, Key::Enter) => Some(RowKey::AddSibling),
        (false, true, Key::Tab) => Some(RowKey::Outdent),
        (false, false, Key::Tab) => Some(RowKey::Indent),
        (false, false, Key::Delete | Key::Backspace) => Some(RowKey::Delete),
        // docs/30 §5.6.1：Shift+Delete 只断边、Shift+F2 只改短语。树文档里这两条
        // 没有定义，处理侧按「非概念图直接忽略」放行（不会误删东西）。
        (false, true, Key::Delete) => Some(RowKey::Unlink),
        (false, false, Key::F2) => Some(RowKey::Rename),
        (false, true, Key::F2) => Some(RowKey::RenameVia),
        (false, false, Key::ArrowUp) => Some(RowKey::Prev),
        (false, false, Key::ArrowDown) => Some(RowKey::Next),
        // docs/28 §5.5：Shift+↑/↓ 扩展多选（锚点保持）。
        (false, true, Key::ArrowUp) => Some(RowKey::SelectPrev),
        (false, true, Key::ArrowDown) => Some(RowKey::SelectNext),
        (false, false, Key::ArrowLeft) => Some(RowKey::Collapse),
        (false, false, Key::ArrowRight) => Some(RowKey::Expand),
        (false, false, Key::Space) => Some(RowKey::Toggle),
        (false, false, Key::Escape) => Some(RowKey::Close),
        _ => None,
    }
}

/// 取本帧第一条属于大纲的按键，并从事件队列里摘掉。
///
/// 只摘自己认领的那一条：若先用 `consume_key(Enter)` 清队列，同一帧里更早的
/// Alt+Enter 会被 egui 的「逻辑匹配」顺手吃掉，画布的聚焦快捷键就丢了。
fn take_row_key(ui: &egui::Ui) -> Option<RowKey> {
    ui.input_mut(|i| {
        let idx = i.events.iter().position(|e| match e {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => row_key(*modifiers, *key).is_some(),
            _ => false,
        })?;
        match i.events.remove(idx) {
            egui::Event::Key { key, modifiers, .. } => row_key(modifiers, key),
            _ => None,
        }
    })
}

/// Esc 的退路：清筛选词 → 关搜索行 → 退出聚焦 → 退出大纲。
/// 抽成纯函数单测——顺序反一次，「在搜索框里按 Esc」就会直接把人踢回画布。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EscStep {
    ClearFilter,
    CloseSearch,
    Unfocus,
    Close,
}

fn esc_step(filter_empty: bool, search_open: bool, focused: bool) -> EscStep {
    if !filter_empty {
        EscStep::ClearFilter
    } else if search_open {
        EscStep::CloseSearch
    } else if focused {
        EscStep::Unfocus
    } else {
        EscStep::Close
    }
}

/// WCAG 相对亮度 / 对比度。画布节点可能带白色文字（根节点就是白字压实色卡），
/// 大纲不画卡片，照搬会把标题画没；低于 3:1 时退回界面正文色。
fn contrast32(a: egui::Color32, b: egui::Color32) -> f32 {
    fn lum(c: egui::Color32) -> f32 {
        let ch = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }
    let (a, b) = (lum(a), lum(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// 行文字字体：节点显式粗体走单独的 BOLD 家族；字号先取节点覆盖，再按深度/密度回落。
fn row_font_id(density: OutlineDensity, depth: u16, row: Option<&Row>) -> egui::FontId {
    // docs/28 §5.1：depth 0 是页面标题（根/自由主题），28/38/600；
    // 600 由 zhisi-bold（700 面）承载（CER-02 偏差①，font_for 同口径）。
    let size = row
        .and_then(|r| r.font_override)
        .unwrap_or(if depth == 0 {
            28.0
        } else {
            density.font(depth)
        })
        .clamp(12.0, 28.0);
    let family = if depth == 0 || row.is_some_and(|r| r.bold) {
        egui::FontFamily::Name(crate::fonts::BOLD.into())
    } else {
        egui::FontFamily::Proportional
    };
    egui::FontId::new(size, family)
}

/// 行内编辑会话。`draft` 是正在输入的文字；`original` 用来判断「没改就不写历史」。
///
/// `just_started` 只在第一帧把焦点与光标要过来；`caret_hint` 是进入编辑时希望落点
/// （双击、拆分、合并各给一个），拿不到就落在末尾。`composing` 记录输入法组合态：
/// 组合期间 Enter/Tab/Esc 归输入法，绝不能拿去提交行。
struct Edit {
    id: String,
    draft: String,
    original: String,
    just_started: bool,
    select_all: bool,
    caret_hint: Option<usize>,
    composing: bool,
    /// 多行粘贴的余行：粘贴帧拆出，提交时与 SetText 走一条 Batch 拆成同级。
    paste_tail: Vec<String>,
}

impl Edit {
    fn start(
        id: &str,
        draft: String,
        original: String,
        select_all: bool,
        caret_hint: Option<usize>,
    ) -> Self {
        Self {
            id: id.to_string(),
            draft,
            original,
            just_started: true,
            select_all,
            caret_hint,
            composing: false,
            paste_tail: Vec::new(),
        }
    }
}

/// 下一帧要打开的行内编辑。行缓存重建之后才能按 id 找到行，所以请求先在这里排队。
struct PendingEdit {
    id: String,
    select_all: bool,
    caret_hint: Option<usize>,
}

/// 概念图短语的就地编辑会话（docs/30 §5.6.1）。
///
/// 与正文编辑**分开一套状态**：短语属于边不属于节点，提交走 `SetRelationshipLabel`，
/// 行高、换行、选区、多行粘贴一律不参与——硬塞进 `Edit` 只会让那套逻辑多四个分支。
struct ViaEdit {
    /// 正在编辑的行（节点 id）。
    id: String,
    /// 这条边从谁出发。
    from: String,
    /// 这条边的 id。
    rel: String,
    draft: String,
    original: String,
    just_started: bool,
    composing: bool,
    /// 这条边是「刚按 Tab 建出来的」：留空提交或取消都要把整条线撤掉（§5.9）。
    fresh: bool,
}

impl ViaEdit {
    fn start(row: &Row, fresh: bool) -> Option<Self> {
        Some(Self {
            id: row.id.clone(),
            from: row.via_from.clone()?,
            rel: row.via_id.clone()?,
            draft: row.via.clone().unwrap_or_default(),
            original: row.via.clone().unwrap_or_default(),
            just_started: true,
            composing: false,
            fresh,
        })
    }
}

/// 下一帧要立起来的短语编辑（行缓存重建之后才能按 id 找到行）。
struct PendingVia {
    id: String,
    fresh: bool,
}

/// 非编辑态的行上敲字：直接进入编辑。IME 预编辑没有可显示的文字，先开空编辑框，
/// 让下一帧的 TextEdit 接管组合串。
enum TypeStart {
    Replace(String),
    Compose,
}

/// 可打印、且不是 Space 的单字符才触发 type-to-edit。Space 仍归折叠/展开。
fn type_char(text: &str) -> bool {
    !text.is_empty()
        && text != " "
        && !text.chars().any(|c| c.is_control())
        && !text.contains('\n')
        && !text.contains('\t')
}

/// 从事件队列里摘一条「精确修饰键」的按键。
///
/// `consume_key` 会忽略多余的 Shift/Alt：Shift+Enter 因此会被当成 Enter 提交，
/// 与「O08 之前软换行不响应」冲突。编辑态的按键全部走这里，修饰键必须一模一样。
fn take_exact_key(ui: &egui::Ui, mods: Modifiers, key: Key) -> bool {
    ui.input_mut(|i| {
        let idx = i.events.iter().position(|e| match e {
            egui::Event::Key {
                key: k,
                pressed: true,
                modifiers,
                ..
            } => *k == key && modifiers.matches_exact(mods),
            _ => false,
        });
        idx.is_some_and(|idx| {
            i.events.remove(idx);
            true
        })
    })
}

/// 本帧的 type-to-edit 事件。IME 提交优先于 Text（两者可能同帧到）；预编辑不消费，
/// 留给下一帧的 TextEdit 继续组合。
fn take_type_start(ui: &egui::Ui) -> Option<TypeStart> {
    let mods = ui.input(|i| i.modifiers);
    if mods.ctrl || mods.alt || mods.command || mods.mac_cmd {
        return None;
    }
    ui.input_mut(|i| {
        if let Some(idx) = i.events.iter().position(|e| match e {
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => !text.is_empty(),
            _ => false,
        }) {
            if let egui::Event::Ime(egui::ImeEvent::Commit(text)) = i.events.remove(idx) {
                return Some(TypeStart::Replace(text));
            }
        }
        if let Some(idx) = i.events.iter().position(|e| match e {
            egui::Event::Text(text) => type_char(text),
            _ => false,
        }) {
            if let egui::Event::Text(text) = i.events.remove(idx) {
                return Some(TypeStart::Replace(text));
            }
        }
        if i.events.iter().any(|e| match e {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => !text.is_empty(),
            _ => false,
        }) {
            return Some(TypeStart::Compose);
        }
        None
    })
}

/// 按字符位把文字切成两段。越过末尾就落在末尾，不会切进 UTF-8 中间。
fn split_text(text: &str, caret: usize) -> (String, String) {
    let byte = text
        .char_indices()
        .nth(caret)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    (text[..byte].to_string(), text[byte..].to_string())
}

/// 双击落点 → 字符下标。单独排一次版只在双击那一帧发生；拿不到就交给末尾/全选兜底。
fn caret_at_pointer(
    ui: &egui::Ui,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    text_x: f32,
    pointer: Pos2,
) -> usize {
    if text.is_empty() {
        return 0;
    }
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, color));
    let rel = Vec2::new((pointer.x - text_x).max(0.0), galley.size().y * 0.5);
    galley
        .cursor_from_pos(rel)
        .index
        .0
        .min(text.chars().count())
}

/// 拖拽落点的三种语义：插到目标前面、插到目标后面、变成目标的子节点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Zone {
    Before,
    After,
    Child,
}

/// 本帧的拖拽落点。
#[derive(Clone, Debug)]
struct DropSpot {
    row: String,
    zone: Zone,
}

/// docs/28 §5.6：上 1/4 插入前、下 1/4 插入后、中 1/2 成子级。
/// 抽成纯函数，边界值（0.25 / 0.75）可以直接断言，不用靠截图猜。
fn drop_zone(rel: f32) -> Zone {
    if rel < 0.25 {
        Zone::Before
    } else if rel > 0.75 {
        Zone::After
    } else {
        Zone::Child
    }
}

/// docs/28 §5.6：握把在悬停满 120ms 后出现；选中行代表明确意图，即时出现。
fn grip_ready(selected: bool, hovered_for: Option<std::time::Duration>) -> bool {
    selected || hovered_for.is_some_and(|d| d >= GRIP_DELAY)
}

/// 大纲拖拽时的自动滚动步长（CSS px）：贴上下缘各推一步，中间不动。
///
/// 抽成纯函数单测：符号反了会朝反方向滚，而拖拽截图里几乎看不出来。
fn outline_autoscroll(y: f32, panel: egui::Rect) -> f32 {
    const EDGE: f32 = 24.0; // docs/28 §5.6：视口上/下 24px 自动滚动
    const SLOW: f32 = 4.0;
    const FAST: f32 = 18.0;
    let ramp = |depth: f32| SLOW + (FAST - SLOW) * (depth / EDGE).clamp(0.0, 1.0);
    if y < panel.top() + EDGE {
        -ramp(panel.top() + EDGE - y)
    } else if y > panel.bottom() - EDGE {
        ramp(y - (panel.bottom() - EDGE))
    } else {
        0.0
    }
}

/// 落点 → (新父级, 插入下标)。根节点没有同级，落在它前/后都按「挂成子节点」处理。
fn move_target(row: &Row, zone: Zone) -> (String, Option<usize>) {
    match zone {
        Zone::Child => (row.id.clone(), None),
        Zone::Before => match &row.parent {
            Some(p) => (p.clone(), Some(row.index)),
            None => (row.id.clone(), None),
        },
        Zone::After => match &row.parent {
            Some(p) => (p.clone(), Some(row.index + 1)),
            None => (row.id.clone(), None),
        },
    }
}

/// 从拖拽根向上找，target 是不是它自己或它的后代。
/// 只靠行表里的 parent 链就够，不需要把整份文档再借一遍。
fn is_self_or_descendant(
    parent_of: &std::collections::HashMap<String, Option<String>>,
    dragged: &str,
    target: &str,
) -> bool {
    let mut cur = Some(target.to_string());
    let mut guard = 0;
    while let Some(id) = cur {
        if id == dragged {
            return true;
        }
        guard += 1;
        if guard > 4096 {
            return true;
        }
        cur = parent_of.get(&id).cloned().flatten();
    }
    false
}

/// 提交时清理用户输入：去掉首尾空白（全空等于清空标题）。
fn commit_text(draft: &str) -> String {
    draft.trim().to_string()
}

/// 粘贴文本按行拆（docs/28 §5.4）：多行返回 `Some((首行, 余行))`，
/// 单行/空返回 `None`（原样交给 TextEdit）；行尾空白与末尾空行忽略。
fn split_paste(text: &str) -> Option<(String, Vec<String>)> {
    let mut lines: Vec<String> = text.lines().map(|l| l.trim_end().to_owned()).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if lines.len() > 1 {
        Some((lines[0].clone(), lines[1..].to_vec()))
    } else {
        None
    }
}

/// 在字符下标区间 `[start, end)` 处插入 `insert`（越界夹取、删选区语义）。
/// Shift+Enter 软换行与多行粘贴共用：docs/28 §5.4 行内换行是本地草稿操作。
fn insert_at_chars(text: &str, start: usize, end: usize, insert: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let start = start.min(chars.len());
    let end = end.min(chars.len()).max(start);
    let mut out: String = chars[..start].iter().collect();
    out.push_str(insert);
    out.extend(chars[end..].iter());
    out
}

/// 可见表上的闭区间（含两端），顺序永远按可见表从上到下。
/// 锚点或目标不在可见表里时只回目标一行，调用方不至于丢选择。
fn range_between(visible: &[String], anchor: &str, target: &str) -> Vec<String> {
    let a = visible.iter().position(|id| id == anchor);
    let b = visible.iter().position(|id| id == target);
    match (a, b) {
        (Some(a), Some(b)) if a <= b => visible[a..=b].to_vec(),
        (Some(a), Some(b)) => visible[b..=a].to_vec(),
        _ => vec![target.to_string()],
    }
}

/// 把区间并进当前选择：先来后到，已存在的 id 不重复加。
fn union_ids(current: &[String], extra: &[String]) -> Vec<String> {
    let mut out = current.to_vec();
    for id in extra {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

pub struct Outline {
    rows: Vec<Row>,
    /// 正在行内改名的行；`None` = 没有编辑。
    editing: Option<Edit>,
    /// 正在就地改写的**链接短语**（概念图，docs/30 §5.6.1）；与 `editing` 互斥。
    via_edit: Option<ViaEdit>,
    /// 下一帧要在这一行的行首立起短语编辑框（行缓存重建之后才找得到行）。
    pending_via: Option<PendingVia>,
    /// 正在被拖拽的行 id；`None` = 没在拖。
    dragging: Option<String>,
    /// 过滤框（按文字包含，忽略大小写）。
    filter: String,
    /// 任务状态筛选（docs/28 §5.7）；`None` = 不限。
    filter_task: Option<TaskState>,
    /// 条件胶囊的键盘焦点（0 任务 / 1 卡片 / 2 标签）；`None` = 焦点在搜索框。
    chip_focus: Option<u8>,
    /// 本帧由键盘触发的胶囊切换（与鼠标单击等价，取走即清）。
    chip_act: Option<u8>,
    /// 标签弹层里的键盘高亮与选中（`0` = 全部标签，其余是 `label_options` 的下标 + 1）。
    label_hl: Option<usize>,
    label_pick: Option<Option<String>>,
    /// 只看有卡片笔记的行。
    filter_card: bool,
    /// 标签筛选；`None` = 不限。
    filter_label: Option<String>,
    /// 文档里出现过的标签（筛选条弹层用，随行缓存重建）。
    label_options: Vec<String>,
    /// 标签弹层是否展开。
    label_popup: bool,
    /// 筛选命中行下标（不含只为上下文保留的祖先）。计数与前后跳只认它。
    hits: Vec<usize>,
    /// 命中行下标集合：非命中但可见的行 = 祖先上下文，按 48% 淡显。
    hit_set: HashSet<usize>,
    /// 每行文本命中的字符区间 `(start, len)`，只对命中行有值。
    hit_ranges: std::collections::HashMap<usize, (usize, usize)>,
    /// 行表是否已按「忽略折叠」展开：筛选态必须穿透折叠分支，清空后回到正常行表。
    rows_all: bool,
    /// 解析后的节点样式缓存（随行缓存重建）。折叠/展开增量补行要靠它，
    /// 否则每展开一次就要再跑一遍全表的 `resolve_all`（20k 下 13ms）。
    styles: std::collections::HashMap<String, ResolvedStyle>,
    /// 缓存对应的文档修订号。
    rev: u64,
    /// 缓存对应的主题 id：换主题要重取分支色。
    theme_id: String,
    /// 当前筛选词命中的行下标。筛选词或行缓存变了才重扫，滚动帧零分配。
    visible: Vec<usize>,
    /// `visible` 对应的筛选词（已 trim + 小写）；`None` = 还没算过。
    filter_key: Option<String>,
    /// 测试用：行缓存重建次数。
    #[cfg(test)]
    rebuilds: u64,
    /// 测试用：全表筛选扫描次数。
    #[cfg(test)]
    filter_scans: u64,
    /// 测试用：上一帧实际排进布局的行数（虚拟化证明：应 ≈ 视口行数 + 20）。
    #[cfg(test)]
    laid_rows: usize,
    /// 下一帧要滚到视野中央的行（键盘上下移动 / 从画布跳进来 / 新建节点）。
    scroll_to: Option<String>,
    /// 新建节点后要在下一帧进入行内编辑（此刻行缓存还没重建）。
    pending_edit: Option<PendingEdit>,
    /// 上一帧只容得下一个动作时暂存的后续动作（Enter 提交 + Tab 缩进撞在同一帧）。
    deferred: Option<Action>,
    /// 本帧处理完按键后要把焦点钉回的行；由壳层在大纲绘制完之后执行。
    focus_request: Option<String>,
    /// 聚焦子树：只显示这一节点及其后代；`None` = 全图。
    focus: Option<String>,
    /// 这份文档是不是概念图（CM-07）：行表改走生成树，树语义的按键与拖拽一律不接。
    concept_map: bool,
    /// 面包屑：从文档根到聚焦点的祖先链（含聚焦点），`(id, text)`。
    crumbs: Vec<(String, String)>,
    /// 行高与字号密度；只影响绘制，不需要重建行缓存。
    density: OutlineDensity,
    /// 换行行数缓存需要重算：行表重建/增量补行/列宽变化时置位（docs/28 §5.2）。
    wrap_dirty: bool,
    /// 上次量换行用的正文列宽（px）；跨帧比较，列宽一变就整表重量。
    wrap_col: f32,
    /// 上一帧行矩形实测宽度：ScrollArea 的滚动条会吃掉几像素，
    /// 用实测值而不是版心宽，才能「量多少、画多少」。
    row_w_seen: f32,
    /// 当前焦点是否由键盘（Tab / 方向键 / 新建节点）带进来的：鼠标点击不画焦点环。
    keyboard_focus: bool,
    /// 搜索行是否展开。默认收起，正文直接顶到页头下。
    search_open: bool,
    /// 下一帧把焦点交给搜索框（Ctrl+Shift+F / 页头搜索按钮之后）。
    search_focus: bool,
    /// 搜索行关闭后把焦点送回当前行；在 `show` 末尾统一执行。
    refocus_rows: bool,
    /// 「文档信息」弹窗是否打开。
    info_open: bool,
    /// 整份文档的静态统计（随行缓存重建）。
    stats: OutlineStats,
    /// 区间选择的锚点：Shift+点击 / Shift+↑↓ 从这里扩到目标行。
    selection_anchor: Option<String>,
    /// 验收钩子钉住的行（`MW_OUTLINE_SELECT`）：绘制时覆盖「主选中」，
    /// 让键位落在这行上。正常运行时恒为 `None`。
    pin: Option<String>,
    /// 多选拖拽根（按可见顺序）。空 = 单行拖拽，`dragging` 就是那一行。
    drag_ids: Vec<String>,
    /// 拖拽悬停在折叠行上的起点（egui 时间，秒）：满 500ms 自动展开。
    /// 用输入时钟而不是 `Instant::now()`：金样按帧数截帧，真实帧耗时不能让状态漂。
    drag_hover: Option<(String, f64)>,
    /// 指针悬停在同一行上的起点（egui 时间，秒）：满 120ms 才画握把（docs/28 §5.6）。
    grip_since: Option<(String, f64)>,
}

impl Default for Outline {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            editing: None,
            via_edit: None,
            pending_via: None,
            dragging: None,
            filter: String::new(),
            filter_task: None,
            chip_focus: None,
            chip_act: None,
            label_hl: None,
            label_pick: None,
            filter_card: false,
            filter_label: None,
            label_options: Vec::new(),
            label_popup: false,
            hits: Vec::new(),
            hit_set: HashSet::new(),
            hit_ranges: std::collections::HashMap::new(),
            rows_all: false,
            styles: std::collections::HashMap::new(),
            rev: u64::MAX,
            theme_id: String::new(),
            visible: Vec::new(),
            filter_key: None,
            #[cfg(test)]
            rebuilds: 0,
            #[cfg(test)]
            filter_scans: 0,
            #[cfg(test)]
            laid_rows: 0,
            scroll_to: None,
            pending_edit: None,
            deferred: None,
            focus_request: None,
            focus: None,
            concept_map: false,
            crumbs: Vec::new(),
            density: OutlineDensity::default(),
            wrap_dirty: true,
            wrap_col: 0.0,
            row_w_seen: 0.0,
            keyboard_focus: false,
            search_open: false,
            search_focus: false,
            refocus_rows: false,
            info_open: false,
            stats: OutlineStats::default(),
            selection_anchor: None,
            pin: None,
            drag_ids: Vec::new(),
            drag_hover: None,
            grip_since: None,
        }
    }
}

/// 大纲行的稳定控件 id：焦点钉回、轨迹登记共用一处派生。
pub(crate) fn row_id(id: &str) -> egui::Id {
    FocusKey::dynamic(ROW_KEY, id).id()
}

impl Outline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn set_density(&mut self, density: OutlineDensity) {
        self.density = density;
        // 字号变了，换行行数要重量（docs/28 §5.2）。
        self.wrap_dirty = true;
    }

    pub fn density(&self) -> OutlineDensity {
        self.density
    }

    pub fn search_open(&self) -> bool {
        self.search_open
    }

    /// 打开搜索行并把键盘交给输入框；已经开着时只重新聚焦。
    pub fn open_search(&mut self) {
        self.search_open = true;
        self.search_focus = true;
        self.refocus_rows = false;
    }

    /// 关闭搜索行并清空筛选；焦点在 `show` 末尾回到当前行。
    pub fn close_search(&mut self) {
        self.search_open = false;
        self.search_focus = false;
        self.clear_filter();
        self.refocus_rows = true;
    }

    /// 清空全部筛选条件（文本 / 任务 / 卡片 / 标签），不改折叠位。
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filter_task = None;
        self.filter_card = false;
        self.filter_label = None;
        self.filter_key = None;
    }

    /// 关闭搜索行时是否还有任何筛选条件。
    pub fn filter_active(&self) -> bool {
        !self.filter.trim().is_empty()
            || self.filter_task.is_some()
            || self.filter_card
            || self.filter_label.is_some()
    }

    pub fn toggle_search(&mut self) {
        if self.search_open {
            self.close_search();
        } else {
            self.open_search();
        }
    }

    /// 实拍钩子：打开搜索行时预填筛选词（界面里由输入框直接改 `filter`）。
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_string();
        self.filter_key = None;
    }

    /// 实拍/测试钩子：直接设定条件（`None` 清空）。界面里由筛选条的胶囊改。
    pub fn set_filter_conditions(
        &mut self,
        task: Option<TaskState>,
        card: bool,
        label: Option<&str>,
    ) {
        self.filter_task = task;
        self.filter_card = card;
        self.filter_label = label.map(str::to_string);
        self.filter_key = None;
    }

    /// 实拍钩子：把标签弹层直接打开（截「文档里有哪些标签」）。
    pub fn open_label_popup(&mut self) {
        self.label_popup = true;
    }

    /// 当前筛选命中数（不含祖先上下文）。没有条件时等于可见行数。
    pub fn search_hits(&self) -> usize {
        if self.filter_active() {
            self.hits.len()
        } else {
            self.visible.len()
        }
    }

    /// 从当前行沿命中表前后跳一格；没有命中时返回 `None`。到头回绕，不卡在末尾。
    fn step_match(&self, primary: Option<&str>, dir: i32) -> Option<String> {
        if self.hits.is_empty() {
            return None;
        }
        let at = primary.and_then(|id| self.hits.iter().position(|&i| self.rows[i].id == id));
        let next = match (at, dir) {
            (Some(0), -1) => self.hits.len() - 1,
            (Some(p), -1) => p - 1,
            (Some(p), _) => (p + 1) % self.hits.len(),
            (None, -1) => self.hits.len() - 1,
            (None, _) => 0,
        };
        self.hits
            .get(next)
            .and_then(|&i| self.rows.get(i))
            .map(|r| r.id.clone())
    }

    /// CER-06 选择/焦点状态表与 CER-07 编辑接线的前瞻 API。
    #[allow(dead_code)]
    /// 短语框开着没有。钩子每帧钉行时要靠它让位——否则刚立起来的短语框
    /// 会被下一帧的「重选 + 刷新」连人带框一起抹掉。
    pub fn is_via_editing(&self) -> bool {
        self.via_edit.is_some()
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// CER-06/09 拖拽接线的前瞻 API。
    #[allow(dead_code)]
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    pub fn stats(&self) -> OutlineStats {
        self.stats
    }

    pub fn info_open(&self) -> bool {
        self.info_open
    }

    pub fn toggle_info(&mut self) {
        self.info_open = !self.info_open;
    }

    pub fn close_info(&mut self) {
        self.info_open = false;
    }

    /// 进入视图 / 从画布跳过来：下一帧把这一行滚到视野中央。
    pub fn reveal(&mut self, id: &str) {
        self.scroll_to = Some(id.to_string());
    }

    /// 聚焦这一节点的子树。缓存里没有「聚焦点」这一维，改 rev 强制下帧重建。
    pub fn focus_on(&mut self, id: &str) {
        self.focus = Some(id.to_string());
        self.rev = u64::MAX;
        self.scroll_to = Some(id.to_string());
        self.editing = None;
        self.dragging = None;
        self.grip_since = None;
    }

    /// 退出聚焦，返回原来的聚焦点（仍在文档里，可作为「回到哪一行」的锚）。
    pub fn unfocus(&mut self) -> Option<String> {
        let old = self.focus.take();
        if old.is_some() {
            self.rev = u64::MAX;
            self.crumbs.clear();
            self.scroll_to.clone_from(&old);
        }
        old
    }

    pub fn focus_id(&self) -> Option<&str> {
        self.focus.as_deref()
    }

    /// CER-11：单节点折叠/展开的行表增量更新。
    ///
    /// 折叠 = 删掉子树行；展开 = 用缓存的样式补回子树行。两条路径都不跑全量
    /// `resolve_all`（20k 下约 13ms）与全表行重建；筛选态与概念图退回全量重建。
    /// 返回 true 表示行表已更新、`sync` 这一帧不必再重建。
    pub(crate) fn apply_collapse(
        &mut self,
        doc: &Document,
        theme: &Theme,
        id: &str,
        collapsed: bool,
    ) -> bool {
        if self.concept_map || self.rows_all {
            return false;
        }
        let Some(r) = self.rows.iter().position(|row| row.id == id) else {
            return false;
        };
        if self.rows[r].collapsed == collapsed {
            return true;
        }
        self.rows[r].collapsed = collapsed;
        if collapsed {
            let depth = self.rows[r].depth;
            let mut end = r + 1;
            while end < self.rows.len() && self.rows[end].depth > depth {
                end += 1;
            }
            self.rows.drain(r + 1..end);
        } else {
            let Some(node) = doc.node(id) else {
                return false;
            };
            let depth = self.rows[r].depth;
            let last = self.rows[r].last;
            let guide_mask = self.rows[r].guide_mask;
            let child_guide = guide_mask
                | if !last && depth > 0 && depth <= 64 {
                    1u64 << (depth - 1)
                } else {
                    0
                };
            let mut stack: RowStack = Vec::with_capacity(node.children.len());
            for (i, child) in node.children.iter().enumerate().rev() {
                stack.push((
                    child.clone(),
                    depth + 1,
                    i,
                    node.children.len(),
                    child_guide,
                ));
            }
            let mut new_rows = Vec::new();
            collect_rows(
                &self.styles,
                doc,
                theme,
                None,
                false,
                &mut stack,
                &[],
                &mut new_rows,
            );
            self.rows.splice(r + 1..r + 1, new_rows);
        }
        // 补进来/删掉的行要重新量换行，否则展开后长行只占一行高（docs/28 §5.2）。
        self.wrap_dirty = true;
        self.visible.clear();
        self.visible.extend(0..self.rows.len());
        self.rev = doc.revision;
        true
    }

    /// 面包屑（根 → 聚焦点）；未聚焦时为空。
    pub fn crumbs(&self) -> &[(String, String)] {
        &self.crumbs
    }

    /// 新建节点后调用：等行缓存重建完，再就地打开这一行的输入框。
    pub fn begin_edit(&mut self, id: &str) {
        self.begin_edit_at(id, None, false);
    }

    /// 某一行的「进来那条边」：`(from, rel)`。概念图行菜单的禁用态、断边与改短语
    /// 共用这一份判定（C20）；树文档行没有进来的边，恒为 `None`。
    pub(crate) fn row_edge(&self, id: &str) -> Option<(String, String)> {
        self.rows
            .iter()
            .find(|r| r.id == id)
            .and_then(|r| Some((r.via_from.clone()?, r.via_id.clone()?)))
    }

    /// 概念图：在这一行的行首立起**短语**编辑框（docs/30 §5.6.1）。
    ///
    /// `fresh` = 这条边是刚按 Tab 建出来的：留空提交/取消要整条撤掉，而不是留一条
    /// 没有短语的暗线（§5.9「未填不给连线落盘」）。
    pub fn begin_via_edit(&mut self, id: &str, fresh: bool) {
        // 行焦点请求会把下一帧的光标抢回行上，和正文编辑同一个坑。
        self.focus_request = None;
        self.pending_via = Some(PendingVia {
            id: id.to_string(),
            fresh,
        });
        self.scroll_to = Some(id.to_string());
    }

    /// 这一行还在不在行表里（改名留空会删行，删了就不该再立短语框）。
    pub fn has_row(&self, id: &str) -> bool {
        self.rows.iter().any(|r| r.id == id)
    }

    /// 进入编辑并指定光标：拆分后落在新行开头，合并后落在拼接处。
    pub fn begin_edit_at(&mut self, id: &str, caret: Option<usize>, select_all: bool) {
        // 行焦点请求在绘制之后才执行，会把下一帧立起来的 TextEdit 光标抢回行上。
        self.focus_request = None;
        self.pending_edit = Some(PendingEdit {
            id: id.to_string(),
            select_all,
            caret_hint: caret,
        });
        self.scroll_to = Some(id.to_string());
    }

    /// 壳层在大纲画完（滚动条也已建好）之后取走焦点请求：滚动条是 `ScrollArea`
    /// 收尾时才创建的，面板内部先请求会被它用 `give_to_next` 抢走（实测焦点落在
    /// 一条看不见的滚动条上）。
    pub(crate) fn take_focus_request(&mut self) -> Option<String> {
        let id = self.focus_request.take();
        if let Some(id) = &id {
            crate::trace::event(&format!("outline_focus_take {id}"));
        }
        id
    }

    /// 窗口级粘贴拦截（docs/28 §5.4 多行拆节点）：编辑态把 Paste 摘走——
    /// 多行首行按选区替换进草稿、余行进 `paste_tail`（提交时一条 Batch 拆成
    /// 同级）；单行转成 Text 事件还给 TextEdit。返回 true 表示粘贴已被
    /// 编辑器消费，窗口剪贴板路径不再处理；非编辑态原样还原事件。
    pub(crate) fn paste_into_edit(&mut self, ctx: &egui::Context) -> bool {
        let Some(text) = ctx.input_mut(|i| {
            i.events
                .iter()
                .position(|e| matches!(e, egui::Event::Paste(_)))
                .and_then(|idx| match i.events.remove(idx) {
                    egui::Event::Paste(t) => Some(t),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(ed) = self.editing.as_mut() else {
            ctx.input_mut(|i| i.events.insert(0, egui::Event::Paste(text)));
            return false;
        };
        match split_paste(&text) {
            Some((first, tail)) => {
                let edit_id = egui::Id::unique(("outline-row-edit", ed.id.as_str()));
                let chars = ed.draft.chars().count();
                let (start, end) = egui::text_edit::TextEditState::load(ctx, edit_id)
                    .and_then(|st| st.cursor.char_range())
                    .map(|r| {
                        let r = r.as_sorted_char_range();
                        (r.start.0.min(chars), r.end.0.min(chars))
                    })
                    .unwrap_or((chars, chars));
                crate::trace::event(&format!(
                    "outline_paste_window caret={start} tail={}",
                    tail.len()
                ));
                ed.draft = insert_at_chars(&ed.draft, start, end, &first);
                ed.paste_tail = tail;
                if let Some(mut state) = egui::text_edit::TextEditState::load(ctx, edit_id) {
                    let at = (start + first.chars().count()).min(ed.draft.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(at),
                        )));
                    state.store(ctx, edit_id);
                }
                true
            }
            None => {
                // 单行：按选区交给 TextEdit（Text 事件在编辑 overlay 绘制时消费）。
                ctx.input_mut(|i| i.events.push(egui::Event::Text(text)));
                true
            }
        }
    }

    /// 结构变化（例如删除）之后，把焦点交给新的当前行。
    pub(crate) fn request_row_focus(&mut self, id: impl Into<String>) {
        let id = id.into();
        crate::trace::event(&format!("outline_focus_pin {id}"));
        self.focus_request = Some(id);
        // 故意不置 keyboard_focus（docs/28 §4.6/§5.3）：pin 是命令/程序化路径
        // （剪贴板、结构动作、演示种子），只选中不画键盘焦点环；环只跟
        // 方向键/Tab 等真实键盘导航走（nav_key 与行内键处理各自置位）。
    }

    /// 只给离屏场景用：把按键焦点真正交给这一行（`focus_request` + 键盘焦点环）。
    ///
    /// 与 [`Self::request_row_focus`] 的差别只有一件事：那个方法**故意**不点亮键盘焦点环
    /// （程序化 pin 不该画环，docs/28 §4.6）。但离屏场景要让 `Tab` / `Shift+Delete` /
    /// `Shift+F2` 走大纲的键位分支，就必须同时满足 `resp.has_focus()` 与
    /// `keyboard_focus`——少一个，按键会被别处吃掉，场景拍出来是一张「什么都没发生」的图。
    /// 摘掉验收钩子钉的行：行内编辑/短语框立起来时必须摘，
    /// 否则面板每帧按「活动行」把焦点从文本框抢回行上。
    pub(crate) fn clear_pin(&mut self) {
        self.pin = None;
    }

    pub(crate) fn force_row_focus(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.keyboard_focus = true;
        self.pin = Some(id.clone());
        self.request_row_focus(id);
    }

    /// 可见行 id，从上到下。选择与批量操作一次取一份。
    fn visible_ids(&self) -> Vec<String> {
        self.visible
            .iter()
            .map(|&i| self.rows[i].id.clone())
            .collect()
    }

    /// 选择集合按可见顺序排列（批量命令要按文档顺序处理）。
    fn selected_visible(&self, sel: &HashSet<&str>) -> Vec<String> {
        self.visible
            .iter()
            .filter_map(|&i| {
                let id = &self.rows[i].id;
                sel.contains(id.as_str()).then(|| id.clone())
            })
            .collect()
    }

    /// 批量删除后焦点落到哪：选中区之后的第一可见行，没有就取之前的一行。
    pub(crate) fn next_visible_after_set(&self, ids: &[String]) -> Option<String> {
        let mut positions: Vec<usize> = ids
            .iter()
            .filter_map(|id| self.visible.iter().position(|&i| &self.rows[i].id == id))
            .collect();
        if positions.is_empty() {
            return None;
        }
        positions.sort_unstable();
        let last = *positions.last()?;
        self.visible
            .get(last + 1)
            .map(|&i| self.rows[i].id.clone())
            .or_else(|| {
                positions
                    .first()
                    .and_then(|&first| first.checked_sub(1))
                    .and_then(|p| self.visible.get(p))
                    .map(|&i| self.rows[i].id.clone())
            })
    }

    /// 同步缓存。**每帧都会调**，所以这里必须便宜：一次整数比较 + 一次字符串比较，不等才重建。
    ///
    /// 折叠也会推进 `revision`（`SetCollapsed` 照样走 ChangeSet），不需要再对全树做指纹。
    pub fn sync(&mut self, doc: &Document, theme: &Theme) {
        // 筛选态要能搜到折叠分支里的命中：行表必须包含全部节点。
        // 条件从无到有/从有到无时强制重建一次，其余帧仍只比 revision。
        if self.filter_active() != self.rows_all {
            self.rev = u64::MAX;
        }
        if self.rev == doc.revision && self.theme_id == theme.id {
            return;
        }
        self.rev = doc.revision;
        self.theme_id = theme.id.to_string();
        self.rebuild(doc, theme);
    }

    /// docs/28 §5.2：非编辑态长文换行。按**正文可用宽**把每行的显示行数量出来缓存，
    /// 虚拟化的前缀高度与绘制共用这一份结果。
    ///
    /// 20k 文档里绝大多数行是短标签：先用「字符数 × 字号」当上界，装得下就直接记 1 行，
    /// 不进字体引擎；只有可能超宽或带硬换行（Shift+Enter）的行才真排版。
    fn measure_wraps(&mut self, ctx: &egui::Context, col_w: f32) {
        if !self.wrap_dirty && (self.wrap_col - col_w).abs() < 0.5 {
            return;
        }
        self.wrap_dirty = false;
        self.wrap_col = col_w;
        let started = std::time::Instant::now();
        let density = self.density;
        let indent = density.indent();
        for row in &mut self.rows {
            let font = row_font_id(density, row.depth, Some(row));
            if let Some(via) = row.via.clone() {
                let label = format!("{via} →");
                row.via_w = ctx
                    .fonts_mut(|f| f.layout_no_wrap(label, font.clone(), Color32::PLACEHOLDER))
                    .size()
                    .x;
            } else {
                row.via_w = 0.0;
            }
            let avail = text_avail(row, col_w, indent);
            let chars = row.text.chars().count();
            // 上界：每个字符最多占一个 em（CJK 全角）。装得下就不可能换行。
            if !row.text.contains('\n') && chars as f32 * font.size <= avail {
                row.text_rows = 1;
                continue;
            }
            let job = row_text_job(
                row.text.clone(),
                font,
                Color32::PLACEHOLDER,
                avail,
                row.italic,
                1,
            );
            let galley = ctx.fonts_mut(|f| f.layout_job(job));
            row.text_rows = galley.rows.len().clamp(1, u16::MAX as usize) as u16;
            if row.text_rows > 1 && crate::trace::is_enabled() {
                crate::trace::event(&format!(
                    "outline_wrap depth={} rows={} avail={:.0} id={}",
                    row.depth, row.text_rows, avail, row.id
                ));
            }
        }
        if std::env::var_os("MW_OUTLINE_PROFILE").is_some() {
            eprintln!(
                "outline wrap: {} rows in {:?}",
                self.rows.len(),
                started.elapsed()
            );
        }
    }

    /// 重算可见行下标（docs/28 §5.7）。只在筛选条件变化时跑全表；
    /// 命中行 + 其祖先进 `visible`，其余分支隐藏；祖先行靠 `hit_set` 淡显。
    pub(crate) fn prepare_filter(&mut self, doc: &Document) {
        let needle = self.filter.trim().to_lowercase();
        let key = format!(
            "{needle}\u{1}{:?}\u{1}{}\u{1}{:?}",
            self.filter_task, self.filter_card, self.filter_label
        );
        if self.filter_key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.filter_key = Some(key);
        #[cfg(test)]
        {
            self.filter_scans += 1;
        }
        self.hits.clear();
        self.hit_set.clear();
        self.hit_ranges.clear();
        self.visible.clear();

        // 没有条件：整棵可见行原样放行。
        if needle.is_empty()
            && self.filter_task.is_none()
            && !self.filter_card
            && self.filter_label.is_none()
        {
            self.visible.extend(0..self.rows.len());
            return;
        }

        let by_id: std::collections::HashMap<&str, usize> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.as_str(), i))
            .collect();
        let mut keep = vec![false; self.rows.len()];
        for i in 0..self.rows.len() {
            let row = &self.rows[i];
            if !needle.is_empty() && !matches_filter(&row.text, &needle) {
                continue;
            }
            if let Some(task) = self.filter_task {
                if row.task != Some(task) {
                    continue;
                }
            }
            if self.filter_card && !row.has_card {
                continue;
            }
            if let Some(label) = &self.filter_label {
                let Some(node) = doc.node(&row.id) else {
                    continue;
                };
                if !node.labels.iter().any(|l| l == label) {
                    continue;
                }
            }
            self.hits.push(i);
            self.hit_set.insert(i);
            if !needle.is_empty() {
                let lower = row.text.to_lowercase();
                if let Some(pos) = lower.find(&needle) {
                    let start = lower[..pos].chars().count();
                    self.hit_ranges.insert(i, (start, needle.chars().count()));
                }
            }
            keep[i] = true;
            // 祖先上下文：沿行表父链向上，全部保留（48% 淡显）。
            let mut cur = row.parent.as_deref();
            let mut guard = 0;
            while let Some(pid) = cur {
                guard += 1;
                if guard > 4096 {
                    break;
                }
                let Some(&p) = by_id.get(pid) else { break };
                keep[p] = true;
                cur = self.rows[p].parent.as_deref();
            }
        }
        self.visible
            .extend(keep.iter().enumerate().filter_map(|(i, k)| k.then_some(i)));
    }

    /// 概念图的行表：把命题网按**生成树**摊开（CM-07，`mw_core::concept_tree`）。
    ///
    /// 与树分支的三处不同：
    /// 1. 父子关系来自 `relationships`（有向、带链接短语），不是 `children`；
    /// 2. 父链不能走 `node.parent_id`（概念图里全是 None），面包屑要沿生成树的 `parent`；
    /// 3. `kids`/`has_children` 按整棵生成树算，折叠只影响可见性——折叠态仍是
    ///    `Node.collapsed`（显示态），不往文档里写树语义。
    fn rebuild_concept(&mut self, doc: &Document, theme: &Theme) {
        self.crumbs.clear();
        self.rows.clear();
        self.wrap_dirty = true;
        self.visible.clear();
        self.filter_key = None;
        let rows = mw_core::concept_rows(doc);
        if rows.is_empty() {
            self.focus = None;
            return;
        }
        // 聚焦点：生成树里找不到（刚被删）就退回全图；聚焦到主概念等于没聚焦。
        let focus_idx = self
            .focus
            .as_deref()
            .and_then(|f| rows.iter().position(|r| r.id == f))
            .filter(|&i| i > 0);
        if self.focus.is_some() && focus_idx.is_none() {
            self.focus = None;
        }
        if let Some(i) = focus_idx {
            let mut chain = Vec::new();
            let mut cur = Some(i);
            let mut guard = 0;
            while let Some(k) = cur {
                guard += 1;
                if guard > 4096 {
                    break;
                }
                let r = &rows[k];
                let text = doc.node(&r.id).map(|n| n.text.clone()).unwrap_or_default();
                chain.push((r.id.clone(), text));
                cur = r
                    .parent
                    .as_ref()
                    .and_then(|p| rows.iter().position(|x| &x.id == p));
            }
            chain.reverse();
            self.crumbs = chain;
        }
        // 视口里的 (生成树行下标, 相对层级)。生成树是前序，聚焦点的子树就是它后面
        // 连续的那些「更深」的行——一个 break 就切干净，不用再建反向表。
        let mut view: Vec<(usize, u16)> = Vec::new();
        match focus_idx {
            Some(fi) => {
                let base = rows[fi].depth;
                view.push((fi, 0));
                for (i, r) in rows.iter().enumerate().skip(fi + 1) {
                    if r.depth <= base {
                        break;
                    }
                    view.push((i, r.depth - base));
                }
            }
            None => view.extend(rows.iter().enumerate().map(|(i, r)| (i, r.depth))),
        }
        let mut kids: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for r in &rows {
            if let Some(p) = r.parent.as_deref() {
                *kids.entry(p).or_insert(0) += 1;
            }
        }
        self.styles = resolve_all(doc, theme);
        let mut seen: std::collections::HashMap<Option<String>, usize> =
            std::collections::HashMap::new();
        // 折叠行以下整棵子树跳过；遇到同层或更浅的行就把闸门放开。
        let mut skip_below: Option<u16> = None;
        for (i, depth) in view {
            if skip_below.is_some_and(|d| depth > d) {
                continue;
            }
            skip_below = None;
            let r = &rows[i];
            let Some(n) = doc.node(&r.id) else {
                continue;
            };
            let parent = if depth == 0 { None } else { r.parent.clone() };
            let index = {
                let slot = seen.entry(parent.clone()).or_insert(0);
                let at = *slot;
                *slot += 1;
                at
            };
            let siblings = parent
                .as_deref()
                .and_then(|p| kids.get(p).copied())
                .unwrap_or(1);
            let resolved = self.styles.get(&n.id);
            let want = resolved.map(|s| s.text).unwrap_or(theme.ui.text);
            let text_color = if contrast32(c32(want), c32(theme.ui.panel)) < 3.0 {
                theme.ui.text
            } else {
                want
            };
            self.rows.push(Row {
                id: n.id.clone(),
                text: n.text.clone(),
                depth,
                has_children: kids.get(n.id.as_str()).is_some_and(|&k| k > 0),
                collapsed: n.collapsed,
                kids: kids
                    .get(n.id.as_str())
                    .copied()
                    .unwrap_or(0)
                    .min(u16::MAX as usize) as u16,
                // 视图根（主概念 / 孤立概念）没有父级：拖拽落点按「挂成子节点」处理。
                parent,
                index: if depth == 0 { 0 } else { index },
                // 命题网没有分支色：大纲里一律走强调色，不做「一线一色」。
                branch: theme.ui.accent,
                text_color,
                bold: resolved.is_some_and(|s| s.bold),
                italic: resolved.is_some_and(|s| s.italic),
                font_override: n.style.as_ref().and_then(|s| s.font_size).map(f32::from),
                last: depth == 0 || index + 1 >= siblings,
                guide_mask: 0,
                has_note: n.note.as_deref().is_some_and(|s| !s.trim().is_empty()),
                note_preview: n
                    .note
                    .as_deref()
                    .and_then(|s| s.lines().map(str::trim).find(|l| !l.is_empty()))
                    .unwrap_or_default()
                    .to_owned(),
                has_card: n.card_id.is_some(),
                labels: n.labels.len().min(u8::MAX as usize) as u8,
                task: n.task,
                via: r.via.clone(),
                text_rows: 1,
                via_w: 0.0,
                via_id: r.via_id.clone(),
                via_from: r.parent.clone(),
            });
            if n.collapsed && !self.rows_all {
                skip_below = Some(depth);
            }
        }
    }

    fn rebuild(&mut self, doc: &Document, theme: &Theme) {
        let profile_start = std::time::Instant::now();
        self.rows.clear();
        self.wrap_dirty = true;
        self.visible.clear();
        self.hits.clear();
        self.hit_set.clear();
        self.hit_ranges.clear();
        self.filter_key = None;
        // 筛选态穿透折叠：否则折叠分支里的命中永远搜不到（docs/28 §5.7）。
        self.rows_all = self.filter_active();
        let stats_start = std::time::Instant::now();
        self.stats = OutlineStats {
            total: doc.node_count(),
            done: doc
                .nodes
                .values()
                .filter(|n| n.task == Some(TaskState::Done))
                .count(),
            notes: doc
                .nodes
                .values()
                .filter(|n| n.note.as_deref().is_some_and(|s| !s.trim().is_empty()))
                .count(),
            cards: doc.cards.len(),
            labels: doc
                .nodes
                .values()
                .flat_map(|n| n.labels.iter().map(String::as_str))
                .collect::<HashSet<_>>()
                .len(),
        };
        self.label_options = doc
            .nodes
            .values()
            .flat_map(|n| n.labels.iter().cloned())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        self.label_options.sort();
        let stats_done = stats_start.elapsed();
        #[cfg(test)]
        {
            self.rebuilds += 1;
        }
        // 概念图（CM-07）：层级在 `relationships` 里，`children` 恒空。走生成树那条分支，
        // 否则整张命题网会被摊成一列 depth 0 的自由主题（审计时的实际表现）。
        self.concept_map = doc.is_concept_map();
        if self.concept_map {
            self.rebuild_concept(doc, theme);
            if std::env::var_os("MW_OUTLINE_PROFILE").is_some() {
                eprintln!(
                    "outline rebuild concept: stats={stats_done:?} total={:?} n={}",
                    profile_start.elapsed(),
                    self.rows.len()
                );
            }
            if std::env::var_os("MW_TRACE").is_some() {
                for r in self.rows.iter().take(8) {
                    crate::trace::event(&format!(
                        "outline_row id={} depth={} via={:?}",
                        r.id, r.depth, r.via
                    ));
                }
            }
            return;
        }
        self.crumbs.clear();
        // 聚焦点可能已被删除；聚焦到根等于没聚焦——都退回全图，免得剩一行孤零零的根。
        if self
            .focus
            .as_deref()
            .is_some_and(|f| f == doc.root_id || doc.node(f).is_none())
        {
            self.focus = None;
        }
        if let Some(focus) = self.focus.clone() {
            // 面包屑从根顺着 parent 链走到聚焦点：只借用文档，不靠行表。
            let mut chain = Vec::new();
            let mut cur = Some(focus.clone());
            let mut guard = 0;
            while let Some(id) = cur {
                guard += 1;
                if guard > 4096 {
                    break;
                }
                let Some(n) = doc.node(&id) else { break };
                chain.push((n.id.clone(), n.text.clone()));
                cur = n.parent_id.clone();
            }
            chain.reverse();
            self.crumbs = chain;
        }
        let focus = self.focus.clone();
        let styles_start = std::time::Instant::now();
        self.styles = resolve_all(doc, theme);
        let styles_done = styles_start.elapsed();
        // 显式栈而不是递归：一份 10k 层的链式文档会把递归打爆。
        // (id, depth, 兄弟下标, 兄弟数, 祖先继续线掩码)。`last` 决定竖线在哪收笔，
        // `guide_mask` 决定这一行要画哪几条祖先竖线——两者都在展平时算好，绘制时零推导。
        let mut stack: RowStack = vec![match &focus {
            Some(f) => (f.clone(), 0, 0, 1, 0),
            None => (doc.root_id.clone(), 0, 0, 1, 0),
        }];
        // 自由主题根：与 `doc_order` 保持一致（根之后、按 id 排序）。聚焦时不出全图别的根。
        let mut floats: Vec<String> = if focus.is_some() {
            Vec::new()
        } else {
            doc.nodes
                .values()
                .filter(|n| n.parent_id.is_none() && n.id != doc.root_id)
                .map(|n| n.id.clone())
                .collect()
        };
        floats.sort();
        collect_rows(
            &self.styles,
            doc,
            theme,
            focus.as_deref(),
            self.rows_all,
            &mut stack,
            &floats,
            &mut self.rows,
        );
        if std::env::var_os("MW_OUTLINE_PROFILE").is_some() {
            eprintln!(
                "outline rebuild: stats={stats_done:?} styles={styles_done:?} rows={:?} total={:?} n={}",
                profile_start.elapsed() - stats_done - styles_done,
                profile_start.elapsed(),
                self.rows.len()
            );
        }
    }

    /// 全屏大纲。返回本帧用户做的动作（最多一个）。
    ///
    /// 内容栏居中、最大 920px：整块屏幕只读一列，行距 38px，眼睛不用在缩进里找路。
    /// 键盘语义与 XMind 大纲一致：Enter 同级、Ctrl+Enter 子级、Tab 降级、Shift+Tab 升级。
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        ch: Chrome,
        selected: &[String],
        primary: Option<&str>,
        hovered: Option<&str>,
        blocked: bool,
        focus: &mut FocusMemory,
    ) -> Option<Action> {
        focus.begin_frame(PanelId::Outline);
        // 金样截图要求像素稳定：编辑态光标闪烁会让同一份构建截出两张不同的图。
        if std::env::var_os("MW_OUTLINE_EDIT").is_some() {
            ui.style_mut().visuals.text_cursor.blink = false;
        }
        let accent = ch.accent;
        // docs/28 §5.3 状态表：着色一律走 design::content 令牌（亮/暗自动），
        // 不再拿 chrome 混色凑数。圆角对齐 content::radius::ROW（6）。
        let content_colors =
            mw_core::design::content::content_metrics(theme, ui.ctx().pixels_per_point()).colors;
        let sel_fill = c32a(content_colors.selected); // 强调色 7%
        let hover_bg = c32a(content_colors.hover); // 主色 4%
        let edit_fill = c32a(content_colors.editing); // 强调色 3%
                                                      // 多选非主选 5%（§5.3 多选行）：令牌表未单列，按 §4.3 selected 口径取
                                                      // 强调色 alpha 13/255，代码即出处。
        let multi_fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 13);
        let row_radius = mw_core::design::content::radius::ROW;
        let text_faint = ch.dim;
        let marker = ch.faint;
        let density = self.density;
        let row_h = density.row_h();
        let indent = density.indent();
        let sel: HashSet<&str> = selected.iter().map(|s| s.as_str()).collect();
        // 聚焦根在视图里按「根」对待：没有同级、不能缩进/重排，也不能被拖走。
        let focus_root = self.focus.clone();
        let focused = focus_root.is_some();

        // 新建节点的行缓存要到下一帧才存在；把编辑请求暂存到这里落地。
        if let Some(p) = self.pending_edit.take() {
            if let Some(r) = self.rows.iter().find(|r| r.id == p.id) {
                crate::trace::event(&format!(
                    "outline_edit_start id={} all={} caret={:?} source=pending",
                    r.id, p.select_all, p.caret_hint
                ));
                self.editing = Some(Edit::start(
                    &r.id,
                    r.text.clone(),
                    r.text.clone(),
                    p.select_all,
                    p.caret_hint,
                ));
            }
        }

        // 短语编辑框同理：行表重建之后才按 id 找得到行、才拿得到 from/rel。
        if let Some(p) = self.pending_via.take() {
            if let Some(r) = self.rows.iter().find(|r| r.id == p.id) {
                if r.via_id.is_some() {
                    crate::trace::event(&format!(
                        "outline_via_start id={} fresh={}",
                        r.id, p.fresh
                    ));
                    self.via_edit = ViaEdit::start(r, p.fresh);
                }
            }
        }

        // 上一帧提交时没来得及返回的动作（同一帧里 Enter 提交、Tab 缩进只容得下一个）。
        let mut action = self.deferred.take();
        let full_w = ui.available_width();
        let margin = column_margin(full_w);
        let col_w = (full_w - margin * 2.0).max(240.0);
        // docs/28 §5.2：非编辑态长文换行——先量行数再排版。滚动区会吃掉几像素，
        // 所以第二帧起用上一帧行矩形的实测宽；宽一变就整表重量。
        let measure_w = if self.row_w_seen > 0.0 {
            self.row_w_seen
        } else {
            col_w
        };
        self.measure_wraps(ui.ctx(), measure_w);
        let filter_id = egui::Id::unique("mw-outline-filter");
        if crate::trace::is_enabled() {
            crate::trace::name(ui.ctx(), filter_id, "outline-filter");
        }
        // 键盘归属：行内编辑 / 搜索框正在输入时，大纲快捷键让位；其余时候整列归大纲。
        let search_open = self.search_open;
        let filter_focused = search_open && ui.memory(|m| m.has_focus(filter_id));
        // Esc 不锁在 TextEdit 里，egui 在帧首就把焦点摘掉了；这一帧仍要认「上一帧在搜索框」，
        // 否则第一次 Esc 会跳过「清空筛选 / 关搜索」直接退出大纲。
        let filter_owned =
            search_open && (filter_focused || ui.memory(|m| m.had_focus_last_frame(filter_id)));
        // 短语编辑框也算「正在输入」：它虽然是行内另一个控件，但 Ctrl+A/Delete 这些键
        // 若漏到大纲列表，会在改短语的同时把可见行全选中（C16 金样实拍：选中 6）。
        let editing_active = self.editing.is_some() || self.via_edit.is_some();
        let rows_own_keys = row_keys_owned(blocked, editing_active, filter_owned);
        // egui 的焦点巡游发生在本帧读键盘之前：Tab / 方向键一到，聚焦行会在创建控件时
        // 把焦点交出去（实测落到顶栏第一个按钮上，下一次回车直接打开菜单）。这些帧把
        // 行降级成 hover 命中，焦点留在原处，按键统一由面板末尾处理。
        let nav_key = rows_own_keys && ui.input(outline_nav_pressed);
        if nav_key {
            self.keyboard_focus = true;
        }
        // 过滤先算：搜索行的命中计数与上下跳转都要用本帧的 `visible`。
        let needle = self.filter.trim().to_lowercase();
        let match_count = self.search_hits();
        let row_total = self.rows.len();
        // 编辑态 ↑/↓ 已交给 TextEdit 移动光标（docs/28 §5.5）；这里只保留
        // 「退格删空行后焦点落哪」所需的上一行。
        let edit_prev = match self.editing.as_ref() {
            Some(e) => match self.visible.iter().position(|&i| self.rows[i].id == e.id) {
                Some(p) => p
                    .checked_sub(1)
                    .map(|q| self.rows[self.visible[q]].id.clone()),
                None => None,
            },
            None => None,
        };
        // 搜索行：默认收起（O02），Ctrl+F / 页头按钮展开后才占 40px。
        // 过滤只按文字包含，不改变树深/父链；拖拽落点仍以完整行表为准。
        // 条件胶囊的键盘路径（docs/28 §5.7）：搜索框里 ↓ 进胶囊行，←/→ 换格，
        // Space/Enter 切换，↑/Esc 回搜索框。鼠标路径一行没动——胶囊仍是点一下走一格。
        if search_open {
            if self.label_popup {
                // 弹层开着：↑/↓ 在「全部标签 + 各标签」里绕，Enter/Space 选中，Esc 收起。
                let n = self.label_options.len() + 1;
                let cur = self.label_hl.unwrap_or(0);
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    self.label_hl = Some(label_hl_step(cur, n, false));
                    crate::trace::event(&format!(
                        "outline_label_popup_hl idx={}",
                        label_hl_step(cur, n, false)
                    ));
                }
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    self.label_hl = Some(label_hl_step(cur, n, true));
                    crate::trace::event(&format!(
                        "outline_label_popup_hl idx={}",
                        (cur + n - 1) % n
                    ));
                }
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::Enter)
                    || take_exact_key(ui, egui::Modifiers::NONE, egui::Key::Space)
                {
                    self.label_pick = Some(if cur == 0 {
                        None
                    } else {
                        self.label_options.get(cur - 1).cloned()
                    });
                    crate::trace::event(&format!("outline_label_popup_pick idx={cur}"));
                }
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::Escape) {
                    self.label_popup = false;
                    self.label_hl = None;
                    crate::trace::event("outline_label_popup_close esc=true");
                }
            } else if self.chip_focus.is_some() {
                let cur = self.chip_focus.unwrap_or(0);
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                    self.chip_focus = Some((cur + 2) % 3);
                    crate::trace::event(&format!("outline_filter_chip_focus idx={}", cur + 2));
                }
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::ArrowRight) {
                    self.chip_focus = Some((cur + 1) % 3);
                    crate::trace::event(&format!(
                        "outline_filter_chip_focus idx={}",
                        (cur + 1) % 3
                    ));
                }
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::Space)
                    || take_exact_key(ui, egui::Modifiers::NONE, egui::Key::Enter)
                {
                    self.chip_act = Some(cur);
                    crate::trace::event(&format!("outline_filter_chip_act idx={cur} source=key"));
                }
                if take_exact_key(ui, egui::Modifiers::NONE, egui::Key::ArrowUp)
                    || take_exact_key(ui, egui::Modifiers::NONE, egui::Key::Escape)
                {
                    self.chip_focus = None;
                    self.search_focus = true;
                }
            } else if filter_focused
                && take_exact_key(ui, egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                // 焦点离开搜索框才能让 ←/→ 归胶囊；回来时再还给它。
                self.chip_focus = Some(0);
                ui.memory_mut(|m| m.surrender_focus(filter_id));
                crate::trace::event("outline_filter_chip_focus idx=0 source=down");
            }
        }
        if search_open {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(margin);
                let (row_rect, _) = ui.allocate_exact_size(Vec2::new(col_w, 40.0), Sense::hover());
                let rect = Rect::from_center_size(row_rect.center(), Vec2::new(col_w, 32.0));
                ui.painter().rect_filled(rect, 8.0, ch.panel2);
                let focused = filter_focused;
                let border = if focused { ch.accent } else { ch.border };
                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                // 放大镜：两条线，比字体图标在 150% 缩放下干净。
                let lens = Pos2::new(rect.left() + 17.0, rect.center().y - 1.0);
                let icon = if focused { ch.accent } else { ch.faint };
                ui.painter()
                    .circle_stroke(lens, 5.0, Stroke::new(1.4, icon));
                ui.painter().line_segment(
                    [
                        Pos2::new(lens.x + 3.6, lens.y + 3.6),
                        Pos2::new(lens.x + 8.0, lens.y + 8.0),
                    ],
                    Stroke::new(1.4, icon),
                );
                // 右缘依次留出计数、条件胶囊、上/下、关闭：文字不钻到按钮底下。
                let text_right = rect.right() - 204.0;
                let edit_rect = Rect::from_min_max(
                    Pos2::new(rect.left() + 32.0, rect.top() + 5.0),
                    Pos2::new(text_right, rect.bottom() - 5.0),
                );
                let resp = ui.put(
                    edit_rect,
                    egui::TextEdit::singleline(&mut self.filter)
                        .id(filter_id)
                        .hint_text("搜索大纲…")
                        // Tab 交给面板处理（跳回行表），否则 egui 会把焦点巡游到顶栏。
                        .event_filter(egui::EventFilter {
                            tab: true,
                            ..Default::default()
                        })
                        .frame(egui::Frame::NONE)
                        .desired_width(edit_rect.width()),
                );
                if self.search_focus {
                    resp.request_focus();
                    self.search_focus = false;
                }
                // 命中计数：有条件时给「命中 / 总行」，否则留白，不让 0 先吓人。
                if self.filter_active() {
                    ui.painter().text(
                        Pos2::new(rect.right() - 196.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        format!("{match_count} / {row_total}"),
                        FontId::proportional(Type::SMALL),
                        ch.faint,
                    );
                }
                // 条件胶囊（docs/28 §5.7）：任务状态 / 有卡片 / 标签，与文本 AND。
                {
                    let chip_act = self.chip_act.take();
                    // 任务：待办 → 进行中 → 完成 → 不限，点一次走一格。
                    let (hit, r) = filter_chip(
                        ui,
                        rect.right() - 168.0,
                        rect.center().y,
                        "task",
                        self.filter_task.is_some(),
                        self.chip_focus == Some(0),
                        ch,
                    );
                    let tc = if self.filter_task.is_some() {
                        accent
                    } else if hit.hovered() {
                        ch.text
                    } else {
                        ch.dim
                    };
                    let box_r = Rect::from_center_size(r.center(), Vec2::splat(13.0));
                    ui.painter().rect_stroke(
                        box_r,
                        3.0,
                        Stroke::new(1.4, tc),
                        egui::StrokeKind::Inside,
                    );
                    if self.filter_task.is_some() {
                        let c = box_r.center();
                        ui.painter().line_segment(
                            [Pos2::new(c.x - 3.0, c.y), Pos2::new(c.x - 0.5, c.y + 3.0)],
                            Stroke::new(1.6, tc),
                        );
                        ui.painter().line_segment(
                            [
                                Pos2::new(c.x - 0.5, c.y + 3.0),
                                Pos2::new(c.x + 3.5, c.y - 3.0),
                            ],
                            Stroke::new(1.6, tc),
                        );
                    }
                    let task_tip = match self.filter_task {
                        Some(TaskState::Todo) => "任务：待办",
                        Some(TaskState::Doing) => "任务：进行中",
                        Some(TaskState::Done) => "任务：完成",
                        None => "按任务状态筛选",
                    };
                    if hit.clicked() || chip_act == Some(0) {
                        self.filter_task = match self.filter_task {
                            None => Some(TaskState::Todo),
                            Some(TaskState::Todo) => Some(TaskState::Doing),
                            Some(TaskState::Doing) => Some(TaskState::Done),
                            Some(TaskState::Done) => None,
                        };
                        self.filter_key = None;
                    }
                    hit.on_hover_text(task_tip);

                    // 卡片：只看挂卡片笔记的行。
                    let (hit, r) = filter_chip(
                        ui,
                        rect.right() - 142.0,
                        rect.center().y,
                        "card",
                        self.filter_card,
                        self.chip_focus == Some(1),
                        ch,
                    );
                    let cc = if self.filter_card {
                        accent
                    } else if hit.hovered() {
                        ch.text
                    } else {
                        ch.dim
                    };
                    let card_r = Rect::from_center_size(
                        r.center() + Vec2::new(0.0, -1.0),
                        Vec2::new(14.0, 11.0),
                    );
                    ui.painter().rect_stroke(
                        card_r,
                        2.5,
                        Stroke::new(1.4, cc),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().line_segment(
                        [
                            Pos2::new(card_r.left() + 3.0, card_r.top() + 3.5),
                            Pos2::new(card_r.right() - 3.0, card_r.top() + 3.5),
                        ],
                        Stroke::new(1.2, cc),
                    );
                    if hit.clicked() || chip_act == Some(1) {
                        self.filter_card = !self.filter_card;
                        self.filter_key = None;
                    }
                    hit.on_hover_text("只看有卡片笔记");

                    // 标签：点开文档里出现过的标签列表。
                    let (hit, r) = filter_chip(
                        ui,
                        rect.right() - 116.0,
                        rect.center().y,
                        "label",
                        self.filter_label.is_some(),
                        self.chip_focus == Some(2),
                        ch,
                    );
                    let lc = if self.filter_label.is_some() || self.label_popup {
                        accent
                    } else if hit.hovered() {
                        ch.text
                    } else {
                        ch.dim
                    };
                    let c = r.center();
                    let tag = vec![
                        Pos2::new(c.x - 6.0, c.y - 5.0),
                        Pos2::new(c.x + 1.0, c.y - 5.0),
                        Pos2::new(c.x + 6.0, c.y),
                        Pos2::new(c.x + 1.0, c.y + 5.0),
                        Pos2::new(c.x - 6.0, c.y + 5.0),
                    ];
                    ui.painter()
                        .add(egui::Shape::closed_line(tag, Stroke::new(1.3, lc)));
                    ui.painter()
                        .circle_filled(Pos2::new(c.x - 2.0, c.y), 1.2, lc);
                    if hit.clicked() || chip_act == Some(2) {
                        self.label_popup = !self.label_popup;
                        self.label_hl = if self.label_popup { Some(0) } else { None };
                        crate::trace::event(&format!(
                            "outline_label_popup_toggle open={}",
                            self.label_popup
                        ));
                    }
                    let label_tip = match &self.filter_label {
                        Some(l) => format!("标签：{l}"),
                        None => "按标签筛选".to_string(),
                    };
                    hit.on_hover_text(label_tip);

                    if self.label_popup {
                        let anchor = Pos2::new(r.left() - 20.0, r.bottom() + 6.0);
                        let mut picked: Option<Option<String>> = self.label_pick.take();
                        let screen = ui.ctx().content_rect();
                        // 封顶 + 滚动：条目多时长出窗口底部是原来最明显的一处越界（docs/30 §5.11）。
                        // `Frame::popup` 自己还有上下内边距（实测 14，取 16 留 2px 余量）；它不在滚动区里，
                        // 不扣掉就会让弹层底边越过窗口 6px 被裁（实测 bottom=806 / 窗口 800）。
                        let max_h = popup_max_height(screen.bottom(), anchor.y, 8.0 + POPUP_FRAME_PAD, 120.0);
                        let area = egui::Area::new(egui::Id::unique("mw-outline-label-popup"))
                            .order(egui::Order::Foreground)
                            .fixed_pos(anchor)
                            // 位置也夹进窗口：宁可压在胶囊上方，也不许被窗口边缘切掉。
                            .constrain_to(screen.shrink(8.0))
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    ui.set_min_width(150.0);
                                    // `Area` 的内层 `max_rect` 是**上一帧**的矩形，`ScrollArea` 又按「当前可用高度」封顶，
                                    // 两者一凑就会自锁在一个偏小的尺寸上（实测 400，而真正可用是 655）。
                                    // 这里先把最大高度顶到算好的 `max_h`，滚动区的视口才等于「窗口下方那一段」。
                                    ui.set_max_height(max_h);
                                    egui::ScrollArea::vertical()
                                        .max_height(max_h)
                                        .auto_shrink([false, true])
                                        .show(ui, |ui| {
                                            // 键盘高亮画成焦点环：与鼠标 hover 区分，避免「看着选中其实没选」。
                                            let all = ui.selectable_label(
                                                self.filter_label.is_none(),
                                                "全部标签",
                                            );
                                            let all_clicked = all.clicked();
                                            if self.label_hl == Some(0) {
                                                all.highlight();
                                            }
                                            if all_clicked {
                                                picked = Some(None);
                                            }
                                            for (i, l) in self.label_options.iter().enumerate() {
                                                let on = self.filter_label.as_deref()
                                                    == Some(l.as_str());
                                                let row = ui.selectable_label(on, l);
                                                let row_clicked = row.clicked();
                                                if self.label_hl == Some(i + 1) {
                                                    row.highlight();
                                                }
                                                if row_clicked {
                                                    picked = Some(Some(l.clone()));
                                                }
                                            }
                                        });
                                });
                            });
                        crate::trace::event(&format!(
                            "outline_label_popup_geom rows={} max_h={:.1} h={:.1} top={:.1} bottom={:.1} screen={:.0}x{:.0}",
                            self.label_options.len() + 1,
                            max_h,
                            area.response.rect.height(),
                            area.response.rect.top(),
                            area.response.rect.bottom(),
                            screen.width(),
                            screen.height()
                        ));
                        if let Some(choice) = picked {
                            self.filter_label = choice;
                            self.filter_key = None;
                            self.label_popup = false;
                        } else if ui.input(|i| i.pointer.any_click()) {
                            // 点弹层与胶囊以外的地方收起（Esc 另有逐级链）。
                            let popup_rect = area.response.rect;
                            let outside = ui
                                .ctx()
                                .pointer_latest_pos()
                                .is_none_or(|p| !popup_rect.contains(p) && !r.contains(p));
                            if outside {
                                self.label_popup = false;
                            }
                        }
                    }
                }
                let nav_enabled = match_count > 0 && self.filter_active();
                let nav_color = if nav_enabled { ch.dim } else { ch.icon_off };
                for (dir, cx, up) in [
                    (-1, rect.right() - 84.0, true),
                    (1, rect.right() - 58.0, false),
                ] {
                    let hit_rect =
                        Rect::from_center_size(Pos2::new(cx, rect.center().y), Vec2::splat(22.0));
                    let hit = ui.interact(
                        hit_rect,
                        egui::Id::unique(("mw-outline-search-step", dir)),
                        Sense::click(),
                    );
                    if hit.hovered() && nav_enabled {
                        ui.painter().rect_filled(hit_rect, 5.0, ch.hover);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let c = hit_rect.center();
                    let dy = if up { 4.0 } else { -4.0 };
                    let color = if hit.hovered() && nav_enabled {
                        ch.text
                    } else {
                        nav_color
                    };
                    ui.painter().line_segment(
                        [Pos2::new(c.x - 4.0, c.y + dy), Pos2::new(c.x, c.y - dy)],
                        Stroke::new(1.3, color),
                    );
                    ui.painter().line_segment(
                        [Pos2::new(c.x, c.y - dy), Pos2::new(c.x + 4.0, c.y + dy)],
                        Stroke::new(1.3, color),
                    );
                    if hit.clicked() && nav_enabled {
                        if let Some(target) = self.step_match(primary, dir) {
                            self.scroll_to = Some(target.clone());
                            action = Some(Action::Select(target));
                        }
                    }
                }
                // 关闭：清词、收起搜索行，焦点回当前行（下一帧执行）。
                let close = Rect::from_center_size(
                    Pos2::new(rect.right() - 18.0, rect.center().y),
                    Vec2::splat(22.0),
                );
                let hit = ui.interact(
                    close,
                    egui::Id::unique("mw-outline-search-close"),
                    Sense::click(),
                );
                if hit.hovered() {
                    ui.painter().rect_filled(close, 5.0, ch.hover);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let c = hit.rect.center();
                let x = Stroke::new(1.3, if hit.hovered() { ch.text } else { ch.faint });
                ui.painter().line_segment(
                    [
                        Pos2::new(c.x - 4.0, c.y - 4.0),
                        Pos2::new(c.x + 4.0, c.y + 4.0),
                    ],
                    x,
                );
                ui.painter().line_segment(
                    [
                        Pos2::new(c.x + 4.0, c.y - 4.0),
                        Pos2::new(c.x - 4.0, c.y + 4.0),
                    ],
                    x,
                );
                if hit.clicked() {
                    self.close_search();
                }
                // Esc 在搜索框里只清词、不丢焦点。egui 的焦点系统在帧首看到 Esc 就把
                // TextEdit 的焦点摘了（它默认不锁 Esc），这里补回来——否则清完筛选光标
                // 一起消失，想接着输入还得再点一次输入框。
                if !blocked
                    && filter_owned
                    && !self.filter.is_empty()
                    && ui.input(|i| i.key_pressed(Key::Escape))
                {
                    resp.request_focus();
                }
            });
            ui.add_space(6.0);
        } else {
            // 没有搜索行时正文直接顶到页头下，只留一点呼吸。
            ui.add_space(4.0);
        }
        let scroll_to = self.scroll_to.clone();
        let mut scroll_hit = false;
        // `rows` 在闭包里被不可变借用；编辑/拖拽状态先搬出来，循环里改局部，最后再放回去。
        let mut editing = self.editing.take();
        // 短语编辑框同样「取出—处理—放回」：它要在行循环里改草稿，而 `self.rows` 这时
        // 正被不可变借用。与正文编辑同一个理由，只是状态分开存（短语属于边）。
        let mut via_edit = self.via_edit.take();
        let mut dragging = self.dragging.take();
        let mut drag_ids = std::mem::take(&mut self.drag_ids);
        // 提交同时按下的 Tab/Shift+Tab：本帧先交文字，缩进/升级放到下一帧做。
        let mut deferred: Option<Action> = None;
        // 本帧所有可见行的矩形：松手时按指针落点找目标行。
        let mut spots: Vec<(String, egui::Rect)> = Vec::new();
        // 滚动条跟着纸面走：默认深灰在浅色大纲里像一根黑棍。
        ui.scope(|ui| {
            let v = ui.visuals_mut();
            v.widgets.inactive.bg_fill = ch.border;
            v.widgets.hovered.bg_fill = ch.faint;
            v.widgets.active.bg_fill = ch.dim;
            // CER-11：目标行在视口外时，先按内容高度把 ScrollArea 直接定位过去。
            // 内容闭包里排队 `scroll_with_delta` 这一帧取不到；20k 的远距离跳转
            // 必须用 offset 才会真的动（只影响「跳到远处」这一种路径）。
            let forced_offset = self.scroll_to.as_deref().and_then(|target| {
                let pos = self
                    .visible
                    .iter()
                    .position(|&i| self.rows[i].id == target)?;
                let height = |i: usize| {
                    let lines = self
                        .editing
                        .as_ref()
                        .filter(|e| e.id == self.rows[i].id)
                        .map(|e| e.draft.split('\n').count().max(1));
                    row_layout_height(&self.rows[i], row_h, lines)
                };
                let mut y = 0.0;
                for i in 0..pos {
                    y += height(i);
                }
                let view_h = (ui.available_height() - 30.0).max(160.0);
                Some((y + height(pos) * 0.5 - view_h * 0.5).max(0.0))
            });
            let mut area = egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height((ui.available_height() - 30.0).max(160.0));
            if let Some(off) = forced_offset {
                area = area.vertical_scroll_offset(off);
            }
            area.show_viewport(ui, |ui, viewport| {
                    ui.horizontal_top(|ui| {
                        ui.add_space(margin);
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            let vis = &self.visible;
                            // docs/28 §5.8：只排视口 ±10 行。行高用同一把尺子累加，
                            // 上下各垫一段空白，滚动范围仍按全表高度算。
                            let line_count = |i: usize| {
                                editing
                                    .as_ref()
                                    .filter(|e| e.id == self.rows[vis[i]].id)
                                    .map(|e| e.draft.split('\n').count().max(1))
                            };
                            let row_height = |i: usize| {
                                row_layout_height(&self.rows[vis[i]], row_h, line_count(i))
                            };
                            const OVERSCAN_PX: f32 = 10.0 * 32.0;
                            let mut first = 0usize;
                            let mut pre = 0.0f32;
                            let top = viewport.top() - OVERSCAN_PX;
                            while first < vis.len() && pre + row_height(first) <= top {
                                pre += row_height(first);
                                first += 1;
                            }
                            let anchor = first;
                            let back = first.min(10);
                            for i in (anchor - back)..anchor {
                                pre -= row_height(i);
                            }
                            first -= back;
                            let mut last = first;
                            let mut y = pre;
                            let bottom = viewport.bottom() + OVERSCAN_PX;
                            while last < vis.len() && (last < anchor + 10 || y < bottom) {
                                y += row_height(last);
                                last += 1;
                            }
                            let mut total = y;
                            for i in last..vis.len() {
                                total += row_height(i);
                            }
                            #[cfg(test)]
                            {
                                self.laid_rows = last - first;
                            }
                            if pre > 0.0 {
                                ui.add_space(pre);
                            }
                            if let Some(target) = scroll_to.as_deref() {
                                if let Some(pos) =
                                    vis.iter().position(|&i| self.rows[i].id == target)
                                {
                                    if pos < first || pos >= last {
                                        crate::trace::event(&format!(
                                            "outline_virt first={first} last={last} pos={pos} vp={:.0}..{:.0}",
                                            viewport.top(),
                                            viewport.bottom()
                                        ));
                                    }
                                }
                            }
                            // 本帧指针真正停住的那一行（不含选中行）：握把计时只认它。
                            let mut hot_row: Option<String> = None;
                            for &row_index in &vis[first..last] {
                                let row = &self.rows[row_index];
                                let width = ui.available_width();
                                // 换行测量用上一帧的实测行宽：滚动条宽度由 ScrollArea 定，
                                // 版心宽减去它才是真正能排字的宽度（下一帧生效即可）。
                                self.row_w_seen = width;
                                // 行占位与点击登记拆开：id 由节点身份派生，不看它在可视区里排第几。
                                let key = FocusKey::dynamic(ROW_KEY, row.id.as_str());
                                // docs/28 §5.1：根是页面标题（28/40），行位 46 给字形留呼吸。
                                // docs/28 §5.2：32px 行；有说明行 +22；编辑中的行按草稿
                                // 行数长高（32 + 26*(n-1)），软换行的行在同框内可见。
                                let edit_lines = editing
                                    .as_ref()
                                    .filter(|e| e.id == row.id)
                                    .map(|e| e.draft.split('\n').count().max(1));
                                let row_top = row_layout_height(row, row_h, edit_lines);
                                let (_, rect) = ui.allocate_space(Vec2::new(width, row_top));
                                // 圆点/chevron/任务框/角标一律对齐**首行**文字带；
                                // 说明行占底部 20px 带。单行行与旧口径逐像素相同。
                                let first_band = if row.depth == 0 { row_h + 12.0 } else { row_h };
                                let row_cy = rect.top() + first_band * 0.5;
                                let sense = if nav_key {
                                    Sense::hover()
                                } else {
                                    Sense::click_and_drag()
                                };
                                let resp = ui.interact(rect, focus::widget_id(ui, &key), sense);
                                // 行拿到焦点后把 Tab/方向键/Esc 锁在行里；上一帧有焦点才生效，
                                // 所以每次获得焦点都顺手请求一次重绘，让锁在下一键之前落地。
                                if resp.has_focus() {
                                    ui.memory_mut(|m| {
                                        m.set_focus_lock_filter(resp.id, outline_key_lock())
                                    });
                                }
                                focus.note(PanelId::Outline, ui.ctx(), &key);
                                if scroll_to.as_deref() == Some(row.id.as_str()) {
                                    resp.scroll_to_me(Some(egui::Align::Center));
                                    scroll_hit = true;
                                }
                                if resp.drag_started()
                                    && !self.concept_map
                                    && focus_root.as_deref() != Some(row.id.as_str())
                                    && editing.as_ref().is_none_or(|e| e.id != row.id)
                                {
                                    // 抓住多选里的任意一行 = 拖整组；顺序按可见表。
                                    drag_ids = if sel.len() > 1 && sel.contains(row.id.as_str()) {
                                        self.selected_visible(&sel)
                                    } else {
                                        vec![row.id.clone()]
                                    };
                                    dragging = Some(row.id.clone());
                                    self.drag_hover = None;
                                    crate::trace::event(&format!("outline_drag_start id={}", row.id));
                                }
                                spots.push((row.id.clone(), rect));
                                if !ui.is_rect_visible(rect) {
                                    continue;
                                }
                                let is_sel = sel.contains(row.id.as_str());
                                let is_primary = primary == Some(row.id.as_str());
                                // 悬停双源：画布同步（X20 口径）+ 本行 egui 指针悬停。
                                // outline_screen 对全屏大纲传 None，此前行悬停恒为假——
                                // hover 态只能靠画布侧反衬，脚本注入的指针到不了行上。
                                let is_hot = hovered == Some(row.id.as_str()) || resp.hovered();
                                if is_hot && !is_sel {
                                    hot_row = Some(row.id.clone());
                                }
                                let is_drag_source = dragging.as_deref() == Some(row.id.as_str())
                                    || drag_ids.iter().any(|d| d == &row.id);
                                // docs/28 §5.7：非命中的可见行是祖先上下文，整行 48% 淡显。
                                let is_ancestor_ctx =
                                    !self.hit_set.is_empty() && !self.hit_set.contains(&row_index);
                                // docs/28 §5.3：拖拽源整行降到 40% 不透明度；幽灵另画。
                                let mut painter = ui.painter_at(rect);
                                if is_drag_source {
                                    painter.set_opacity(0.4);
                                } else if is_ancestor_ctx {
                                    painter.set_opacity(0.48);
                                }
                                let mut editing_this =
                                    editing.as_ref().is_some_and(|e| e.id == row.id);
                                // 行尾角标与折叠计数占住右缘：文字在这里收笔，
                                // 否则长标题会从角标底下穿过去。编辑态这一行归输入框，
                                // 角标全部让位，文字占满整行。
                                let chip_reserve = if !editing_this
                                    && row.collapsed
                                    && row.has_children
                                {
                                    CHIP_STEP
                                } else {
                                    0.0
                                };
                                let text_right = if editing_this {
                                    rect.right() - 8.0
                                } else {
                                    rect.right()
                                        - 8.0
                                        - MORE_LANE
                                        - chip_reserve
                                        - marker_reserve(row)
                                };

                                let row_rect = rect.shrink2(Vec2::new(0.0, 1.0));
                                // 页面标题行无底、无边框、无悬停底（docs/28 §5.1/§5.9）。
                                // 状态表（docs/28 §5.3）：编辑 3% > 主选 7% > 多选 5% >
                                // 悬停 4%；一律无边框、圆角 6（§3.1：选中行不许像输入框）。
                                if row.depth > 0 {
                                    if editing_this {
                                        painter.rect_filled(row_rect, row_radius, edit_fill);
                                    } else if is_sel && is_primary {
                                        painter.rect_filled(row_rect, row_radius, sel_fill);
                                    } else if is_sel {
                                        painter.rect_filled(row_rect, row_radius, multi_fill);
                                    } else if is_hot {
                                        painter.rect_filled(row_rect, row_radius, hover_bg);
                                    }
                                }
                                // 左侧握把：悬停满 120ms 或选中时露出 6 个点，提示这行可以拖。
                                // docs/28 §5.6：握把延迟是为了不让指针扫过时满屏闪烁。
                                let now = ui.input(|i| i.time);
                                let hovered_for = self.grip_since.as_ref().and_then(|(gid, since)| {
                                    (gid == &row.id).then(|| {
                                        std::time::Duration::from_secs_f64((now - since).max(0.0))
                                    })
                                });
                                if !editing_this && row.depth > 0 && grip_ready(is_sel, hovered_for) {
                                    let gx = rect.left() + 5.0;
                                    for row_i in 0..3 {
                                        let y = row_cy - 4.0 + row_i as f32 * 4.0;
                                        for col in 0..2 {
                                            painter.circle_filled(
                                                Pos2::new(gx + col as f32 * 4.0, y),
                                                1.1,
                                                marker,
                                            );
                                        }
                                    }
                                }
                                // 键盘焦点另给一圈细环；鼠标点击不画，Tab / 方向键才把它带回来。
                                if resp.has_focus() && self.keyboard_focus {
                                    painter.rect_stroke(
                                        rect.shrink(1.0),
                                        row_radius,
                                        Stroke::new(1.5, accent),
                                        egui::StrokeKind::Inside,
                                    );
                                }

                                let base = rect.left() + PAD_L + row.depth as f32 * indent;
                                // docs/28 §5.2：4px 圆点贴缩进线，距文本 12；折叠时换 12px chevron。
                                let bullet_x = base + 2.0;
                                let cc = Pos2::new(bullet_x, row_cy);
                                // docs/28 §5.9：树形竖线/连接线/引导线全部不画。
                                // 圆点 4px 中性色；折叠或悬停/选中时换成 12px chevron。
                                let tint = ch.dim;
                                let chev_hover = row.has_children
                                    && row.depth > 0
                                    && ui.ctx().pointer_hover_pos().is_some_and(|p| {
                                        Rect::from_min_size(
                                            Pos2::new(base, rect.top()),
                                            Vec2::new(20.0, rect.height()),
                                        )
                                        .contains(p)
                                    });
                                if row.has_children
                                    && row.depth > 0
                                    && (row.collapsed || chev_hover || is_sel)
                                {
                                    let color = if chev_hover { ch.text } else { ch.dim };
                                    let stroke = Stroke::new(1.4, color);
                                    if row.collapsed {
                                        painter.line_segment(
                                            [Pos2::new(cc.x - 2.0, cc.y - 4.0), Pos2::new(cc.x + 2.0, cc.y)],
                                            stroke,
                                        );
                                        painter.line_segment(
                                            [Pos2::new(cc.x + 2.0, cc.y), Pos2::new(cc.x - 2.0, cc.y + 4.0)],
                                            stroke,
                                        );
                                    } else {
                                        painter.line_segment(
                                            [Pos2::new(cc.x - 4.0, cc.y - 2.0), Pos2::new(cc.x, cc.y + 2.0)],
                                            stroke,
                                        );
                                        painter.line_segment(
                                            [Pos2::new(cc.x, cc.y + 2.0), Pos2::new(cc.x + 4.0, cc.y - 2.0)],
                                            stroke,
                                        );
                                    }
                                } else if row.depth > 0 {
                                    painter.circle_filled(cc, 2.0, tint);
                                }

                                // 折叠 chevron 的点击热区：20px 宽，与圆点/文本互不重叠。
                                if row.has_children && row.depth > 0 {
                                    let chev = Rect::from_min_size(
                                        Pos2::new(base, rect.top()),
                                        Vec2::new(20.0, rect.height()),
                                    );
                                    let hit = ui.interact(
                                        chev,
                                        focus::widget_id(
                                            ui,
                                            &FocusKey::dynamic(CHEV_KEY, row.id.as_str()),
                                        ),
                                        Sense::click(),
                                    );
                                    if hit.clicked() {
                                        action = Some(Action::Toggle(row.id.clone()));
                                    }
                                    if hit.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                }

                                let mut text_x = base + 16.0;
                                // 这一行是否正被短语编辑框占着行首（CM-09）。后面所有
                                // 「进入正文编辑」的入口都要让位，否则同帧会立起两个输入框。
                                let via_this = via_edit.as_ref().is_some_and(|v| v.id == row.id);
                                // 行首短语前缀的命中区：点它 = 改这条边的短语（§5.6.1）。
                                let mut via_prefix_rect: Option<Rect> = None;
                                // docs/28 §5.2：任务框行内 14×14，在文本之前；先定位再画字，
                                // 否则框会盖住首字（实拍翻过车）。
                                let task_box = row.task.filter(|_| !editing_this).map(|_| {
                                    Rect::from_min_size(
                                        Pos2::new(text_x, row_cy - 7.0),
                                        Vec2::splat(MARK_TASK),
                                    )
                                });
                                if task_box.is_some() {
                                    text_x += TASK_INLINE_STEP;
                                }
                                // CM-07：概念图的行是**命题的一半**——链接短语画成淡色前缀
                                // （`因为 →`），文字仍是概念本身：改名只改概念，不碰短语。
                                // 前缀占位靠实测字宽，`text_x` 之后的选区、编辑框、省略
                                // 全部自动跟着走，不会出现「前缀被输入框盖住」。
                                if via_this {
                                    // 短语编辑框就长在行首：同一行高、同一字体、无边框，
                                    // 底下一条 2px 强调色——「这段字正在编辑」，不是弹窗。
                                    let font = row_font_id(density, row.depth, Some(row));
                                    let h = (rect.height() - 8.0).max(18.0);
                                    let w = 160.0_f32
                                        .min((text_right - text_x - 12.0).max(60.0));
                                    let box_rect = Rect::from_min_size(
                                        Pos2::new(text_x, rect.top() + 4.0),
                                        Vec2::new(w, h),
                                    );
                                    let via_id =
                                        egui::Id::unique(("outline-via-edit", row.id.as_str()));
                                    if crate::trace::is_enabled() {
                                        crate::trace::name(ui.ctx(), via_id, "outline-via-edit");
                                    }
                                    let (done, close): (bool, Option<Action>) = {
                                        let mut close: Option<Action> = None;
                                        let v = via_edit.as_mut().expect("短语编辑行");
                                        if v.just_started {
                                            // 立起来的下一帧再要焦点：同帧要焦点会被行焦点覆盖。
                                            ui.memory_mut(|m| m.request_focus(via_id));
                                            v.just_started = false;
                                        }
                                        if let Some(c) = crate::canvas::ime_transition(
                                            &ui.input(|i| i.events.clone()),
                                        ) {
                                            if v.composing != c {
                                                crate::trace::event(&format!(
                                                    "outline_via_ime composing={c}"
                                                ));
                                            }
                                            v.composing = c;
                                        }
                                        let ime_commit = ui.input(|i| {
                                            i.events.iter().any(|e| {
                                                matches!(
                                                    e,
                                                    egui::Event::Ime(egui::ImeEvent::Commit(_))
                                                )
                                            })
                                        });
                                        // 组合态里 Structure 键归输入法（与行编辑同一条纪律）。
                                        let keys_free = !v.composing && !ime_commit;
                                        let esc = keys_free
                                            && take_exact_key(ui, Modifiers::NONE, Key::Escape);
                                        let enter = keys_free
                                            && (take_exact_key(ui, Modifiers::NONE, Key::Enter)
                                                || take_exact_key(ui, Modifiers::NONE, Key::Tab));
                                        let resp = ui.put(
                                            box_rect,
                                            egui::TextEdit::singleline(&mut v.draft)
                                                .id(via_id)
                                                .font(font)
                                                .text_color(text_faint)
                                                .frame(egui::Frame::NONE)
                                                .margin(egui::Margin::ZERO)
                                                .desired_width(box_rect.width()),
                                        );
                                        // 下划线只画在文字实际宽度上：像「正在编辑这段字」。
                                        let uw = ui
                                            .fonts_mut(|f| {
                                                f.layout_no_wrap(
                                                    v.draft.clone(),
                                                    row_font_id(density, row.depth, Some(row)),
                                                    text_faint,
                                                )
                                            })
                                            .size()
                                            .x
                                            .max(24.0);
                                        painter.line_segment(
                                            [
                                                Pos2::new(box_rect.left(), box_rect.bottom()),
                                                Pos2::new(
                                                    box_rect.left() + uw.min(w),
                                                    box_rect.bottom(),
                                                ),
                                            ],
                                            Stroke::new(2.0, accent),
                                        );
                                        let blur_commit = resp.lost_focus() && !esc;
                                        let finished = esc || enter || blur_commit;
                                        if finished {
                                            let text = v.draft.trim().to_string();
                                            let blank = text.is_empty();
                                            // 一字未动就别发命令：改短语会压一层撤销栈，
                                            // 也会把文档标脏——「点开又点走」不该算一次编辑。
                                            let unchanged = text == v.original.trim();
                                            // 留空但原本就有短语：仍发 SetVia(None)，由概念图口径
                                            // 拒绝并提示（C16）——大纲与画布不能各说各话。
                                            let clear = blank && !v.original.trim().is_empty();
                                            crate::trace::event(&format!(
                                                "outline_via_close id={} esc={esc} blank={blank} unchanged={unchanged} fresh={}",
                                                row.id, v.fresh
                                            ));
                                            close = if blank && v.fresh {
                                                // 空短语 + 刚建的边：整条撤掉，概念留着（§5.9）。
                                                Some(Action::UnlinkConcept {
                                                    from: v.from.clone(),
                                                    rel: v.rel.clone(),
                                                })
                                            } else if clear {
                                                Some(Action::SetVia {
                                                    from: v.from.clone(),
                                                    rel: v.rel.clone(),
                                                    label: None,
                                                })
                                            } else if blank || unchanged {
                                                None
                                            } else {
                                                Some(Action::SetVia {
                                                    from: v.from.clone(),
                                                    rel: v.rel.clone(),
                                                    label: Some(text),
                                                })
                                            };
                                        }
                                        (finished, close)
                                    };
                                    if done {
                                        self.keyboard_focus = true;
                                        via_edit = None;
                                        if let Some(act) = close {
                                            if action.is_none() {
                                                action = Some(act);
                                            } else {
                                                deferred = Some(act);
                                            }
                                        }
                                    }
                                    text_x += w + 8.0;
                                } else if let Some(via) = row.via.as_deref() {
                                    let font = row_font_id(density, row.depth, Some(row));
                                    let label = format!("{via} →");
                                    // 宽度由 `measure_wraps` 量好：换行测量与绘制共用同一个数。
                                    let w = if row.via_w > 0.0 {
                                        row.via_w
                                    } else {
                                        ui.fonts_mut(|f| {
                                            f.layout_no_wrap(label.clone(), font.clone(), text_faint)
                                        })
                                        .size()
                                        .x
                                    };
                                    painter.text(
                                        Pos2::new(text_x, row_cy),
                                        Align2::LEFT_CENTER,
                                        &label,
                                        font,
                                        text_faint,
                                    );
                                    via_prefix_rect = Some(Rect::from_min_max(
                                        Pos2::new(text_x, rect.top() + 2.0),
                                        Pos2::new(text_x + w + 8.0, rect.bottom() - 2.0),
                                    ));
                                    text_x += w + 8.0;
                                }
                                let edit_id =
                                    egui::Id::unique(("outline-row-edit", row.id.as_str()));
                                if crate::trace::is_enabled() {
                                    crate::trace::name(ui.ctx(), edit_id, "outline-row-edit");
                                }
                                // ---- 进入编辑：F2 全选、双击按指针落点、落字直接改写 ----
                                // 三种入口都在正文绘制之前判定：本帧就能立起 TextEdit，
                                // IME 的第一帧预编辑串才不会丢。
                                if !editing_this && !via_this {
                                    let font = row_font_id(density, row.depth, Some(row));
                                    // docs/28 §5.2：完成任务文字 text.faint，不划删除线。
                                    let color = if row.task == Some(TaskState::Done) {
                                        text_faint
                                    } else {
                                        c32(row.text_color)
                                    };
                                    if resp.has_focus()
                                        && take_exact_key(ui, Modifiers::NONE, Key::F2)
                                    {
                                        crate::trace::event(&format!(
                                            "outline_edit_enter id={} source=f2",
                                            row.id
                                        ));
                                        editing = Some(Edit::start(
                                            &row.id,
                                            row.text.clone(),
                                            row.text.clone(),
                                            true,
                                            None,
                                        ));
                                        self.keyboard_focus = true;
                                        self.scroll_to = Some(row.id.clone());
                                        editing_this = true;
                                    } else if resp.double_clicked() {
                                        let caret = resp.interact_pointer_pos().map(|p| {
                                            caret_at_pointer(
                                                ui,
                                                &row.text,
                                                font,
                                                color,
                                                text_x,
                                                p,
                                            )
                                        });
                                        crate::trace::event(&format!(
                                            "outline_edit_enter id={} source=double caret={caret:?}",
                                            row.id
                                        ));
                                        editing = Some(Edit::start(
                                            &row.id,
                                            row.text.clone(),
                                            row.text.clone(),
                                            false,
                                            caret,
                                        ));
                                        self.scroll_to = Some(row.id.clone());
                                        editing_this = true;
                                    } else if resp.has_focus() && !nav_key {
                                        if let Some(start) = take_type_start(ui) {
                                            let (draft, kind) = match start {
                                                TypeStart::Replace(text) => (text, "text"),
                                                TypeStart::Compose => (String::new(), "ime"),
                                            };
                                            crate::trace::event(&format!(
                                                "outline_edit_enter id={} source={kind}",
                                                row.id
                                            ));
                                            editing = Some(Edit::start(
                                                &row.id,
                                                draft,
                                                row.text.clone(),
                                                false,
                                                None,
                                            ));
                                            editing_this = true;
                                        }
                                    }
                                }
                                let text_rect = Rect::from_min_max(
                                    Pos2::new(text_x, rect.top() + 3.0),
                                    Pos2::new(text_right, rect.bottom() - 3.0),
                                );
                                let mut commit_now: Option<String> = None;
                                // Some(child)：回车落在末尾，先提交再新建并接着编辑新行。
                                let mut commit_add: Option<bool> = None;
                                // 提交之后要跟着做的结构动作（Tab 缩进 / ↑↓ 走行）。
                                let mut commit_follow: Option<Action> = None;
                                // 键盘提交把焦点钉回哪一行；失焦提交留 `None`，不抢焦点。
                                let mut commit_focus: Option<String> = None;
                                let mut cancel_now = false;
                                // 结构动作要把这一行从编辑态里摘出去，但不能在还借着
                                // `ed` 的时候动 `editing`：先记标记，画完 TextEdit 再落地。
                                let mut leave_split: Option<Action> = None;
                                let mut leave_merge: Option<String> = None;
                                let mut leave_empty = false;
                                // 折叠计数徽标跟着文字尾部走；编辑时按输入框右缘算。
                                let mut text_end = text_right;
                                if editing_this {
                                    let ed = editing.as_mut().expect("editing row");
                                    let original = ed.original.clone();
                                    // IME 组合态：组合期间 Enter/Tab/Esc/方向键归输入法。
                                    if let Some(v) = crate::canvas::ime_transition(
                                        &ui.input(|i| i.events.clone()),
                                    ) {
                                        if ed.composing != v {
                                            crate::trace::event(&format!(
                                                "outline_ime composing={v}"
                                            ));
                                        }
                                        ed.composing = v;
                                    }
                                    let ime_commit = ui.input(|i| {
                                        i.events.iter().any(|e| {
                                            matches!(
                                                e,
                                                egui::Event::Ime(egui::ImeEvent::Commit(_))
                                            )
                                        })
                                    });
                                    if ime_commit {
                                        crate::trace::event("outline_ime commit");
                                    }
                                    let keys_free = !ed.composing && !ime_commit;
                                    // 组合态（以及确认组合的那一帧）里，结构键归输入法。
                                    // 正常输入法在系统层就截住了它们；万一被转发进来，
                                    // 也不能让 TextEdit 拿去当提交/失焦——预编辑串会被
                                    // 提前落盘。这里只吃掉，不执行任何大纲动作。
                                    if !keys_free {
                                        for (mods, key) in [
                                            (Modifiers::NONE, Key::Enter),
                                            (
                                                Modifiers {
                                                    shift: true,
                                                    ..Modifiers::NONE
                                                },
                                                Key::Enter,
                                            ),
                                            (
                                                Modifiers {
                                                    ctrl: true,
                                                    ..Modifiers::NONE
                                                },
                                                Key::Enter,
                                            ),
                                            (Modifiers::NONE, Key::Tab),
                                            (
                                                Modifiers {
                                                    shift: true,
                                                    ..Modifiers::NONE
                                                },
                                                Key::Tab,
                                            ),
                                            (Modifiers::NONE, Key::Escape),
                                            (Modifiers::NONE, Key::ArrowLeft),
                                            (Modifiers::NONE, Key::ArrowRight),
                                            (Modifiers::NONE, Key::ArrowUp),
                                            (Modifiers::NONE, Key::ArrowDown),
                                            (Modifiers::NONE, Key::Backspace),
                                            (Modifiers::NONE, Key::Delete),
                                        ] {
                                            take_exact_key(ui, mods, key);
                                        }
                                    }
                                    let ctrl_enter = keys_free
                                        && take_exact_key(
                                            ui,
                                            Modifiers {
                                                ctrl: true,
                                                ..Modifiers::NONE
                                            },
                                            Key::Enter,
                                        );
                                    let enter = keys_free
                                        && !ctrl_enter
                                        && take_exact_key(ui, Modifiers::NONE, Key::Enter);
                                    let esc = keys_free
                                        && take_exact_key(ui, Modifiers::NONE, Key::Escape);
                                    let shift_tab = keys_free
                                        && take_exact_key(
                                            ui,
                                            Modifiers {
                                                shift: true,
                                                ..Modifiers::NONE
                                            },
                                            Key::Tab,
                                        );
                                    let tab = keys_free
                                        && !shift_tab
                                        && take_exact_key(ui, Modifiers::NONE, Key::Tab);
                                    // caret 读上一帧的 TextEdit 状态：本帧按键在 TextEdit 之前
                                    // 判定，必须知道光标在哪，才能决定「拆分还是新建」。
                                    let chars = ed.draft.chars().count();
                                    let (sel_start, sel_end) =
                                        egui::text_edit::TextEditState::load(ui.ctx(), edit_id)
                                            .and_then(|s| s.cursor.char_range())
                                            .map(|r| {
                                                let r = r.as_sorted_char_range();
                                                (r.start.0.min(chars), r.end.0.min(chars))
                                            })
                                            .unwrap_or((chars, chars));
                                    // docs/28 §5.4：Shift+Enter 行内换行——写入本地草稿，
                                    // 不产生命令；整份多行文本提交时仍是一条 SetText。
                                    if keys_free
                                        && take_exact_key(
                                            ui,
                                            Modifiers {
                                                shift: true,
                                                ..Modifiers::NONE
                                            },
                                            Key::Enter,
                                        )
                                    {
                                        crate::trace::event(&format!(
                                            "outline_edit_soft_break caret={}",
                                            sel_start
                                        ));
                                        ed.draft =
                                            insert_at_chars(&ed.draft, sel_start, sel_end, "\n");
                                    }
                                    // docs/28 §5.4：Shift+Enter 行内换行——写本地草稿，
                                    // 不产生命令；整份多行文本提交时仍是一条 SetText。
                                    if keys_free
                                        && take_exact_key(
                                            ui,
                                            Modifiers {
                                                shift: true,
                                                ..Modifiers::NONE
                                            },
                                            Key::Enter,
                                        )
                                    {
                                        crate::trace::event(&format!(
                                            "outline_edit_soft_break caret={}",
                                            sel_start
                                        ));
                                        ed.draft =
                                            insert_at_chars(&ed.draft, sel_start, sel_end, "\n");
                                        // 光标跟到换行后的行首：程序化改草稿不会动
                                        // TextEdit 的内部光标，不推它就停在原行行尾。
                                        if let Some(mut state) =
                                            egui::text_edit::TextEditState::load(ui.ctx(), edit_id)
                                        {
                                            let at = (sel_start + 1).min(ed.draft.chars().count());
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::one(
                                                    egui::text::CCursor::new(at),
                                                ),
                                            ));
                                            state.store(ui.ctx(), edit_id);
                                        }
                                    }
                                    let empty = ed.draft.trim().is_empty();
                                    // 空行 Backspace/Delete：删掉这一行，焦点回上一可见行。
                                    if keys_free
                                        && empty
                                        && (take_exact_key(
                                            ui,
                                            Modifiers::NONE,
                                            Key::Backspace,
                                        ) || take_exact_key(
                                            ui,
                                            Modifiers::NONE,
                                            Key::Delete,
                                        ))
                                    {
                                        crate::trace::event(&format!(
                                            "outline_delete_empty id={}",
                                            row.id
                                        ));
                                        leave_empty = true;
                                    } else if keys_free
                                        && sel_start == 0
                                        && sel_end == 0
                                        && take_exact_key(ui, Modifiers::NONE, Key::Backspace)
                                    {
                                        // 行首退格：并入上一同级；改字和删除装进一条 Batch。
                                        leave_merge = Some(commit_text(&ed.draft));
                                    } else if enter || ctrl_enter {
                                        // 有选区就先删掉选区再拆，像文本编辑器一样。
                                        let (source, caret) = if sel_start != sel_end {
                                            let (head, rest) = split_text(&ed.draft, sel_start);
                                            let (_, tail) =
                                                split_text(&rest, sel_end - sel_start);
                                            (format!("{head}{tail}"), sel_start)
                                        } else {
                                            (ed.draft.clone(), sel_start)
                                        };
                                        let (head, tail) = split_text(&source, caret);
                                        let head = head.trim_end().to_string();
                                        let tail = tail.trim_start().to_string();
                                        let child = ctrl_enter || row.parent.is_none();
                                        if tail.is_empty() {
                                            // 末尾回车：提交 + 新建同级/子级，随后编辑新行。
                                            commit_now = Some(commit_text(&source));
                                            commit_add = Some(child);
                                        } else {
                                            crate::trace::event(&format!(
                                                "outline_split id={} child={child} head={head:?} tail={tail:?}",
                                                row.id
                                            ));
                                            leave_split = Some(if child {
                                                Action::SplitChild {
                                                    id: row.id.clone(),
                                                    head,
                                                    tail,
                                                }
                                            } else {
                                                Action::SplitSibling {
                                                    id: row.id.clone(),
                                                    head,
                                                    tail,
                                                }
                                            });
                                        }
                                    } else if esc {
                                        cancel_now = true;
                                    } else if shift_tab || tab {
                                        commit_now = Some(commit_text(&ed.draft));
                                        commit_focus = Some(row.id.clone());
                                        commit_follow = Some(if shift_tab {
                                            Action::Outdent(row.id.clone())
                                        } else {
                                            Action::Indent(row.id.clone())
                                        });
                                    }
                                    // 编辑态就是这一行：透明编辑框盖在原文上，同字体同色，
                                    // 底部一条 2px 强调色下划线；没有白底、没有外框。
                                    let font = row_font_id(density, row.depth, Some(row));
                                    let ink = c32(row.text_color);
                                    let resp = ui.put(
                                        text_rect,
                                        // 多行编辑框：软换行的 \n 在草稿里，行高随行数
                                        // 增长（见 row_top）；Enter 在前置键处理里已被消费。
                                        // 默认 desired_rows=4：单行草稿会被 4 行最小高度
                                        // 加垂直居中推到下一行，2px 下划线就跑到文字上方。
                                        // 这里锁 1 行；多行由草稿行数与 row_top 接管。
                                        egui::TextEdit::multiline(&mut ed.draft)
                                            .id(edit_id)
                                            .font(font.clone())
                                            .text_color(ink)
                                            .frame(egui::Frame::NONE)
                                            .margin(egui::Margin::ZERO)
                                            .desired_rows(1)
                                            .align(egui::Align2::LEFT_CENTER)
                                            .desired_width(text_rect.width()),
                                    );
                                    // 下划线跟着文字走：像「这段字正在编辑」，而不是一个输入框。
                                    let underline = ui
                                        .fonts_mut(|f| {
                                            f.layout_no_wrap(ed.draft.clone(), font, ink)
                                        })
                                        .size()
                                        .x
                                        .max(40.0)
                                        .min(text_rect.width());
                                    painter.hline(
                                        egui::Rangef::new(text_x, text_x + underline),
                                        text_rect.bottom() - 1.0,
                                        Stroke::new(2.0, accent),
                                    );
                                    if ed.just_started {
                                        resp.request_focus();
                                        let mut state =
                                            egui::text_edit::TextEditState::load(
                                                ui.ctx(),
                                                edit_id,
                                            )
                                            .unwrap_or_default();
                                        let range = if ed.select_all {
                                            egui::text::CCursorRange::select_all_str(&ed.draft)
                                        } else {
                                            let at = ed.caret_hint.unwrap_or_else(|| {
                                                ed.draft.chars().count()
                                            });
                                            egui::text::CCursorRange::one(
                                                egui::text::CCursor::new(
                                                    at.min(ed.draft.chars().count()),
                                                ),
                                            )
                                        };
                                        state.cursor.set_char_range(Some(range));
                                        // 新行没有自己的撤销栈：Ctrl+Z 不该回放上一行的输入。
                                        state.clear_undoer();
                                        state.store(ui.ctx(), edit_id);
                                        ed.just_started = false;
                                    }
                                    // 失焦即提交（点别处）；IME 用回车确认组合的那一帧不算。
                                    if !ime_commit
                                        && resp.lost_focus()
                                        && !cancel_now
                                        && commit_now.is_none()
                                    {
                                        commit_now = Some(commit_text(&ed.draft));
                                    }
                                    let id = row.id.clone();
                                    // 到这里 `ed` 不再使用，可以安全地放下编辑态。
                                    if let Some(act) = leave_split.take() {
                                        crate::trace::event(&format!(
                                            "outline_commit f={} id={id:?} split=true",
                                            ui.ctx().cumulative_pass_nr()
                                        ));
                                        editing = None;
                                        if action.is_none() {
                                            action = Some(act);
                                        } else {
                                            deferred = Some(act);
                                        }
                                    } else if let Some(text) = leave_merge.take() {
                                        crate::trace::event(&format!(
                                            "outline_commit f={} id={id:?} merge={text:?}",
                                            ui.ctx().cumulative_pass_nr()
                                        ));
                                        editing = None;
                                        if action.is_none() {
                                            action = Some(Action::MergePrev { id, text });
                                        } else {
                                            deferred = Some(Action::MergePrev { id, text });
                                        }
                                    } else if leave_empty {
                                        crate::trace::event(&format!(
                                            "outline_commit f={} id={id:?} delete_empty=true",
                                            ui.ctx().cumulative_pass_nr()
                                        ));
                                        editing = None;
                                        let act = Action::DeleteEmpty {
                                            id,
                                            prev: edit_prev.clone(),
                                        };
                                        if action.is_none() {
                                            action = Some(act);
                                        } else {
                                            deferred = Some(act);
                                        }
                                    } else if cancel_now {
                                        crate::trace::event(&format!(
                                            "outline_commit f={} id={id:?} cancel=true",
                                            ui.ctx().cumulative_pass_nr()
                                        ));
                                        editing = None;
                                        self.focus_request = Some(id);
                                        ui.ctx().request_repaint();
                                    } else if let Some(text) = commit_now.take() {
                                        let paste_tail = editing
                                            .as_ref()
                                            .map(|e| e.paste_tail.clone())
                                            .unwrap_or_default();
                                        crate::trace::event(&format!(
                                            "outline_commit f={} id={id:?} text={text:?} add={commit_add:?}",
                                            ui.ctx().cumulative_pass_nr()
                                        ));
                                        editing = None;
                                        // docs/28 §5.4：余行拆成同级优先于"提交+新建"，
                                        // 否则 Enter 在行尾时 CommitAndAdd 会丢掉 paste_tail。
                                        if !paste_tail.is_empty() {
                                            crate::trace::event(&format!(
                                                "outline_commit f={} id={id:?} paste_tail={}",
                                                ui.ctx().cumulative_pass_nr(),
                                                paste_tail.len()
                                            ));
                                            let act = Action::PasteLines {
                                                id,
                                                text,
                                                siblings: paste_tail,
                                            };
                                            if action.is_none() {
                                                action = Some(act);
                                            } else {
                                                deferred = Some(act);
                                            }
                                        } else if let Some(child) = commit_add.take() {
                                            let changed = text != original;
                                            let act = Action::CommitAndAdd {
                                                id,
                                                text,
                                                changed,
                                                child,
                                            };
                                            if action.is_none() {
                                                action = Some(act);
                                            } else {
                                                deferred = Some(act);
                                            }
                                        } else {
                                            if text != original {
                                                let act = Action::Rename {
                                                    id: id.clone(),
                                                    text,
                                                };
                                                if action.is_none() {
                                                    action = Some(act);
                                                } else {
                                                    deferred = Some(act);
                                                }
                                            }
                                            if let Some(follow) = commit_follow.take() {
                                                deferred = Some(follow);
                                            }
                                            if let Some(focus) = commit_focus.take() {
                                                self.focus_request = Some(focus);
                                            }
                                        }
                                        ui.ctx().request_repaint();
                                    }
                                } else {
                                    let empty = row.text.trim().is_empty();
                                    let label = if empty {
                                        "（未命名）".to_string()
                                    } else {
                                        // docs/28 §5.2：文本不截断——硬换行（Shift+Enter）与
                                        // 超宽自动换行都按显示行排版，悬挂缩进由 `text_x` 保证。
                                        row.text.clone()
                                    };
                                    let color = if empty {
                                        text_faint
                                    } else {
                                        c32(row.text_color)
                                    };
                                    // 文字右缘让给行尾角标与折叠计数：与量换行同一把尺子。
                                    let width = (text_right - text_x).max(1.0);
                                    let font = row_font_id(density, row.depth, Some(row));
                                    let lines = row.text_rows.max(1);
                                    let job =
                                        row_text_job(label, font, color, width, row.italic, lines);
                                    let galley = painter.fonts_mut(|f| f.layout_job(job));
                                    // 首行按自身行高居中在文字带上，后续行按 24 步下落。
                                    let first_h = galley
                                        .rows
                                        .first()
                                        .map_or(galley.size().y, |r| r.size.y);
                                    let text_top = row_cy - first_h * 0.5;
                                    // docs/28 §5.7：命中字符 12% 强调底，不加粗；换行行按行拆段。
                                    if let Some(&(start, len)) = self.hit_ranges.get(&row_index) {
                                        let hit_fill = Color32::from_rgba_unmultiplied(
                                            accent.r(),
                                            accent.g(),
                                            accent.b(),
                                            30,
                                        );
                                        // 单行仍用旧的「行框 ±3」竖向范围，既有金样零漂移；
                                        // 只有真换行的行才按各自文字行拆开画底。
                                        let one_line = galley.rows.len() <= 1;
                                        for band in hit_bands(&galley, start, start + len) {
                                            let band =
                                                band.translate(egui::vec2(text_x, text_top));
                                            if band.left() >= text_right {
                                                continue;
                                            }
                                            let band = if one_line {
                                                Rect::from_min_max(
                                                    Pos2::new(band.left(), rect.top() + 3.0),
                                                    Pos2::new(band.right(), rect.bottom() - 3.0),
                                                )
                                            } else {
                                                band
                                            };
                                            painter.rect_filled(
                                                Rect::from_min_max(
                                                    band.min,
                                                    Pos2::new(
                                                        band.right().min(text_right),
                                                        band.bottom(),
                                                    ),
                                                ),
                                                2.0,
                                                hit_fill,
                                            );
                                        }
                                    }
                                    text_end = (text_x + galley.size().x).min(text_right);
                                    let clip = Rect::from_min_max(
                                        Pos2::new(text_x, rect.top()),
                                        Pos2::new(text_right, rect.bottom()),
                                    );
                                    painter.with_clip_rect(clip).galley(
                                        Pos2::new(text_x, text_top),
                                        galley,
                                        color,
                                    );
                                }
                                // 说明行（docs/28 §5.2）：备注首行 13/20，text.secondary，
                                // 与正文同 x；行高已按 +20 计入。编辑态让位给输入框。
                                if row.has_note
                                    && !editing_this
                                    && !row.note_preview.is_empty()
                                {
                                    painter.text(
                                        Pos2::new(text_x, rect.bottom() - 10.0),
                                        Align2::LEFT_CENTER,
                                        &row.note_preview,
                                        egui::FontId::new(13.0, egui::FontFamily::Proportional),
                                        ch.dim,
                                    );
                                }
                                // 行尾角标（docs/28 §5.2）：单个 5px 圆点（卡片/备注）。
                                // 任务框在行内（text_x 之前），不再占行尾。
                                // 角标点击不能用嵌套 `ui.interact`：整行的 `click_and_drag`
                                // 先占了指针，统一在行响应里按指针落点判定。
                                let hover_pos = ui.ctx().pointer_hover_pos();
                                // 编辑态这一行不给角标留位置：文字占满整行，角标整体隐身。
                                let markers_on = !editing_this;
                                let end_dot = (markers_on && (row.has_note || row.has_card))
                                    .then(|| {
                                        Pos2::new(
                                            rect.right() - 8.0 - MORE_LANE - MARK_DOT * 0.5,
                                            row_cy,
                                        )
                                    });
                                let note_dot = end_dot.filter(|_| row.has_note);
                                let card_dot = end_dot.filter(|_| row.has_card);
                                // 方框本体只有 12pt，热区放大到 20pt；编辑态这一行归输入框。
                                // 备注点与卡片点只隔 10pt，热区必然重叠，统一按最近中心裁决。
                                let (task_hit, note_hit, card_hit) = if editing_this {
                                    (None, None, None)
                                } else {
                                    let hit = |c: Pos2| {
                                        Rect::from_center_size(c, Vec2::splat(MARK_TASK + 8.0))
                                    };
                                    (
                                        task_box.map(|b| hit(b.center())),
                                        note_dot.map(hit),
                                        card_dot.map(hit),
                                    )
                                };
                                let hover_mark = marker_at(hover_pos, task_hit, note_hit, card_hit);
                                if let (Some(mark), Some(p)) = (hover_mark, hover_pos) {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    if let Some(text) = marker_tip(mark, row.task) {
                                        // 行内 painter 会被滚动区裁剪，提示改画在独立浮层上。
                                        // 角标贴着右缘：右侧放不下时翻到指针左边，避免文案折行。
                                        let screen = ui.ctx().content_rect();
                                        let tip_pos = if p.x + 220.0 > screen.right() {
                                            p + egui::vec2(-208.0, 16.0)
                                        } else {
                                            p + egui::vec2(12.0, 16.0)
                                        };
                                        egui::Area::new(egui::Id::unique((
                                            "outline_mark_tip",
                                            row.id.as_str(),
                                        )))
                                        .order(egui::Order::Tooltip)
                                        .fixed_pos(tip_pos)
                                        .interactable(false)
                                        .show(
                                            ui.ctx(),
                                            |ui| {
                                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                                    ui.label(
                                                        egui::RichText::new(text)
                                                            .size(Type::SMALL)
                                                            .color(ch.text),
                                                    );
                                                });
                                            },
                                        );
                                    }
                                }
                                // 正文不再截断（docs/28 §5.2 长文换行），省略号悬停提示随之退役。
                                if let Some(box_) = task_box {
                                    let task = row.task.unwrap_or(TaskState::Todo);
                                    if hover_mark == Some(Mark::Task) {
                                        painter.rect_filled(box_.expand(3.0), 4.0, hover_bg);
                                    }
                                    match task {
                                        TaskState::Todo => {
                                            painter.rect_stroke(
                                                box_,
                                                3.0,
                                                Stroke::new(1.5, text_faint),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        TaskState::Doing => {
                                            painter.rect_stroke(
                                                box_,
                                                3.0,
                                                Stroke::new(1.5, accent),
                                                egui::StrokeKind::Inside,
                                            );
                                            painter.circle_filled(box_.center(), 2.0, accent);
                                        }
                                        TaskState::Done => {
                                            painter.rect_filled(box_, 3.0, accent);
                                            let c = box_.center();
                                            let check = Stroke::new(1.4, ch.panel);
                                            painter.line_segment(
                                                [
                                                    Pos2::new(c.x - 3.0, c.y),
                                                    Pos2::new(c.x - 1.0, c.y + 2.0),
                                                ],
                                                check,
                                            );
                                            painter.line_segment(
                                                [
                                                    Pos2::new(c.x - 1.0, c.y + 2.0),
                                                    Pos2::new(c.x + 3.2, c.y - 2.2),
                                                ],
                                                check,
                                            );
                                        }
                                    }
                                }
                                if let Some(dot) = note_dot {
                                    if hover_mark == Some(Mark::Note) {
                                        painter.circle_filled(dot, MARK_DOT * 0.5 + 3.0, hover_bg);
                                    }
                                    painter.circle_filled(dot, MARK_DOT * 0.5, tint);
                                }
                                if let Some(dot) = card_dot {
                                    if hover_mark == Some(Mark::Card) {
                                        painter.circle_filled(dot, MARK_DOT * 0.5 + 3.0, hover_bg);
                                    }
                                    painter.circle_filled(dot, MARK_DOT * 0.5, accent);
                                }
                                // 行尾「更多」（docs/28 §5.2）：16px `⋯` 距列右 8，
                                // 悬停/选中出现、编辑与浮层开着时隐身。页面标题行（根）
                                // 没有行铬件，它的动作在页头 `⋯` 里。
                                let more_on = !blocked
                                    && row.depth > 0
                                    && !editing_this
                                    && dragging.is_none()
                                    && (is_sel || is_hot);
                                let more_hot = more_on
                                    && hover_pos
                                        .is_some_and(|p| row_more_rect(rect, row_cy).contains(p));
                                if more_on {
                                    let more = row_more_rect(rect, row_cy);
                                    if more_hot {
                                        painter.rect_filled(more, 4.0, hover_bg);
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    let tint = if more_hot { ch.text } else { ch.dim };
                                    for i in 0..3 {
                                        let x = rect.right() - 8.0 - MORE_LANE + 10.0
                                            + MORE_DOT_STEP * i as f32;
                                        painter.circle_filled(
                                            Pos2::new(x, row_cy),
                                            MORE_DOT_R,
                                            tint,
                                        );
                                    }
                                }
                                // 折叠子项数：13 辅助色纯文字，贴文字尾部（docs/28 §5.2）。
                                if row.collapsed && row.has_children && markers_on {
                                    painter.text(
                                        Pos2::new(text_end + 8.0, row_cy),
                                        Align2::LEFT_CENTER,
                                        format!("{}", row.kids),
                                        egui::FontId::proportional(Type::SMALL),
                                        text_faint,
                                    );
                                }

                                // 行上右键：与 `⋯` 同一张菜单，锚在指针处（docs/06 §6）。
                                // 概念图也走这张入口（C20）：动作集在壳层按文档类型切换，
                                // 它只有「建边 / 断边 / 改短语」这类命题网动作，没有树语义。
                                if !editing_this && resp.secondary_clicked() && action.is_none() {
                                    let pos =
                                        resp.interact_pointer_pos().unwrap_or_else(|| rect.center());
                                    crate::trace::event(&format!(
                                        "outline_row_context id={} pos={pos:?}",
                                        row.id
                                    ));
                                    self.keyboard_focus = false;
                                    self.selection_anchor = Some(row.id.clone());
                                    action = Some(Action::RowMenu {
                                        id: row.id.clone(),
                                        pos,
                                    });
                                }
                                // 编辑中这一行的单击归输入框，别顺手把选择改了。
                                // 角标优先于整行：指针落在热区里就分派给角标。
                                if !editing_this && resp.clicked() && action.is_none() {
                                    self.keyboard_focus = false;
                                    let pos = resp.interact_pointer_pos();
                                    crate::trace::event(&format!(
                                        "outline_row_click id={} pos={pos:?} more_on={more_on}",
                                        row.id
                                    ));
                                    action = marker_click(
                                        pos, row.task, task_hit, note_hit, card_hit, &row.id,
                                    );
                                    // `⋯` 在角标右侧的动作道里：角标没压中才轮到它。
                                    if action.is_none() && more_on {
                                        if let Some(p) =
                                            pos.filter(|p| row_more_rect(rect, row_cy).contains(*p))
                                        {
                                            self.selection_anchor = Some(row.id.clone());
                                            action = Some(Action::RowMenu {
                                                id: row.id.clone(),
                                                pos: p,
                                            });
                                        }
                                    }
                                    // 点行首的短语前缀 = 改这条边的短语（§5.6.1）。
                                    // 只有单点、且这一行真有一条进来的边时才成立：前缀是
                                    // 「轻量入口」，不该抢 Shift/Ctrl 的选择手势。
                                    if action.is_none() && row.via_id.is_some() {
                                        let mods = ui.input(|i| i.modifiers);
                                        let plain =
                                            !mods.shift && !mods.command && !mods.ctrl;
                                        if plain
                                            && pos.is_some_and(|p| {
                                                via_prefix_rect.is_some_and(|r| r.contains(p))
                                            })
                                        {
                                            crate::trace::event(&format!(
                                                "outline_concept_via_edit id={} source=click",
                                                row.id
                                            ));
                                            action = Some(Action::EditVia(row.id.clone()));
                                        }
                                    }
                                    if action.is_none() {
                                        let mods = ui.input(|i| i.modifiers);
                                        let ctrl = mods.command || mods.ctrl;
                                        if mods.shift {
                                            // Shift：从锚点扩到这一行；Ctrl+Shift 并入当前选择。
                                            let anchor = self
                                                .selection_anchor
                                                .clone()
                                                .or_else(|| primary.map(str::to_string))
                                                .unwrap_or_else(|| row.id.clone());
                                            let vis = self.visible_ids();
                                            let extra = range_between(&vis, &anchor, &row.id);
                                            let ids = if ctrl {
                                                union_ids(selected, &extra)
                                            } else {
                                                extra
                                            };
                                            if self.selection_anchor.is_none() || !ctrl {
                                                self.selection_anchor = Some(anchor);
                                            }
                                            self.focus_request = Some(row.id.clone());
                                            action = Some(Action::SetSelection {
                                                ids,
                                                primary: row.id.clone(),
                                            });
                                        } else if ctrl {
                                            self.selection_anchor = Some(row.id.clone());
                                            action = Some(Action::ToggleSelect(row.id.clone()));
                                        } else {
                                            // 点行即把键盘焦点交给这一行：接着敲 F2 / 方向键
                                            // 都该作用在刚点的行上，而不是上一行。
                                            self.selection_anchor = Some(row.id.clone());
                                            self.focus_request = Some(row.id.clone());
                                            action = Some(Action::Select(row.id.clone()));
                                        }
                                    }
                                }
                                if resp.hovered() && dragging.is_none() && !editing_this {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                            }
                            // 底部空白把剩余行的高度补上，滚动条范围才与全表一致。
                            let tail = total - y;
                            if tail > 0.0 {
                                ui.add_space(tail);
                            }

                            // 握把计时：同一行继续悬停就保留起点，换行/离开重新计时。
                            self.grip_since = match hot_row {
                                Some(id) => match self.grip_since.take() {
                                    Some((cur, since)) if cur == id => Some((cur, since)),
                                    _ => Some((id, ui.input(|i| i.time))),
                                },
                                None => None,
                            };

                            // 筛选没有命中：给一块安静的说明，而不是一整页空白。
                            if self.filter_active() && match_count == 0 {
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::new(col_w, 168.0), Sense::hover());
                                let painter = ui.painter();
                                let (icon, icon_r) =
                                    (Pos2::new(rect.center().x, rect.top() + 34.0), 9.0);
                                painter.circle_stroke(icon, icon_r, Stroke::new(1.6, ch.border));
                                painter.line_segment(
                                    [
                                        Pos2::new(icon.x + icon_r * 0.72, icon.y + icon_r * 0.72),
                                        Pos2::new(icon.x + icon_r * 1.5, icon.y + icon_r * 1.5),
                                    ],
                                    Stroke::new(1.6, ch.border),
                                );
                                let what = if needle.is_empty() {
                                    "没有符合条件的内容".to_string()
                                } else {
                                    format!("没有找到「{}」", self.filter.trim())
                                };
                                painter.text(
                                    Pos2::new(rect.center().x, rect.top() + 70.0),
                                    Align2::CENTER_CENTER,
                                    what,
                                    FontId::proportional(Type::LABEL),
                                    ch.dim,
                                );
                                painter.text(
                                    Pos2::new(rect.center().x, rect.top() + 94.0),
                                    Align2::CENTER_CENTER,
                                    "Esc 清空筛选 · 换个关键词试试",
                                    FontId::proportional(Type::SMALL),
                                    ch.dim,
                                );
                            }

                            // ---- 拖拽落点：只在指针盖住某个可见行时才算 ----
                            // docs/28 §5.6：Esc 取消拖拽，文档不变；按键在这里被吃掉，
                            // 不再往「逐级退出」的 Esc 链上传。
                            if dragging.is_some()
                                && ui.input_mut(|i| {
                                    i.consume_key(Modifiers::NONE, Key::Escape)
                                })
                            {
                                crate::trace::event("outline_drag_cancel");
                                dragging = None;
                                drag_ids.clear();
                                self.drag_hover = None;
                            }
                            if let Some(id) = dragging.clone() {
                                let (pointer, released) = ui
                                    .ctx()
                                    .input(|i| (i.pointer.latest_pos(), i.pointer.any_released()));
                                let mut spot: Option<DropSpot> = None;
                                if let Some(p) = pointer {
                                    for (rid, rect) in &spots {
                                        if rect.contains(p) {
                                            let rel = (p.y - rect.top()) / rect.height().max(1.0);
                                            // docs/28 §5.6：上 1/4 插入前、下 1/4 插入后、中 1/2 成子级。
                                            spot = Some(DropSpot {
                                                row: rid.clone(),
                                                zone: drop_zone(rel),
                                            });
                                            break;
                                        }
                                    }
                                }
                                // 拖到面板上下缘时自动滚：越贴边越快（4→18px/帧），边带 24px。
                                if let Some(p) = pointer {
                                    let dy = outline_autoscroll(p.y, ui.max_rect());
                                    if dy != 0.0 {
                                        ui.scroll_with_delta(Vec2::new(0.0, dy));
                                        crate::trace::event(&format!(
                                            "outline_autoscroll dy={dy:.1}"
                                        ));
                                    }
                                }
                                if let Some(s) = &spot {
                                    let text = self
                                        .rows
                                        .iter()
                                        .find(|r| r.id == s.row)
                                        .map(|r| r.text.as_str())
                                        .unwrap_or("?");
                                    crate::trace::event(&format!(
                                        "outline_drag_spot row={text:?} zone={:?}",
                                        s.zone
                                    ));
                                }
                                // 悬停在折叠行上 500ms：自动展开，露出可落子节点的位置。
                                let collapsed_hover = spot.as_ref().and_then(|s| {
                                    self.rows
                                        .iter()
                                        .find(|r| r.id == s.row && r.collapsed && r.has_children)
                                        .map(|r| r.id.clone())
                                });
                                match (collapsed_hover, &self.drag_hover) {
                                    (Some(rid), Some((hovered, since))) if &rid == hovered => {
                                        // docs/28 §5.6：满 500ms 自动展开。
                                        if ui.input(|i| i.time) - *since >= 0.5 {
                                            crate::trace::event("outline_drag_autofold");
                                            action = Some(Action::Toggle(rid));
                                            self.drag_hover = None;
                                        }
                                    }
                                    (Some(rid), _) => {
                                        self.drag_hover =
                                            Some((rid, ui.input(|i| i.time)));
                                    }
                                    (None, _) => self.drag_hover = None,
                                }
                                // 指针 x 落在哪个缩进档，before/after 就预览/落到哪一级。
                                let drop = spot.as_ref().and_then(|s| {
                                    let (_, rect) =
                                        spots.iter().find(|(rid, _)| rid == &s.row)?;
                                    let target = self.rows.iter().find(|r| r.id == s.row)?;
                                    let depth = match s.zone {
                                        Zone::Child => target.depth,
                                        _ => {
                                            let x = pointer.map(|p| p.x).unwrap_or(f32::NAN);
                                            if x.is_finite() {
                                                let base = rect.left() + PAD_L + CHEVRON;
                                                ((x - base) / indent)
                                                    .round()
                                                    .clamp(0.0, target.depth as f32)
                                                    as u16
                                            } else {
                                                target.depth
                                            }
                                        }
                                    };
                                    let mut effective = target;
                                    while effective.depth > depth {
                                        let parent = effective.parent.as_deref()?;
                                        effective = self.rows.iter().find(|r| r.id == parent)?;
                                    }
                                    Some((effective, depth))
                                });
                                // docs/28 §5.6：落点在任一被拖节点上/其子树里 → 非法，
                                // 预览给红 1px 线 + 禁止光标，松开不执行。
                                let drag_illegal = match (&spot, &drop) {
                                    (Some(s), Some((target, _))) => {
                                        let parent_of: std::collections::HashMap<
                                            String,
                                            Option<String>,
                                        > = self
                                            .rows
                                            .iter()
                                            .map(|r| (r.id.clone(), r.parent.clone()))
                                            .collect();
                                        let ids = if drag_ids.is_empty() {
                                            vec![id.clone()]
                                        } else {
                                            drag_ids.clone()
                                        };
                                        ids.iter().any(|dragged| {
                                            is_self_or_descendant(&parent_of, dragged, &s.row)
                                                || is_self_or_descendant(
                                                    &parent_of,
                                                    dragged,
                                                    &target.id,
                                                )
                                        })
                                    }
                                    _ => false,
                                };
                                if drag_illegal {
                                    crate::trace::event("outline_drag_illegal");
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::NoDrop);
                                }
                                if let Some((target, depth)) = &drop {
                                    if let Some((_, rect)) =
                                        spots.iter().find(|(rid, _)| rid == &target.id)
                                    {
                                        // 非法落点红 1px；合法落点强调色 2px。
                                        let line = if drag_illegal {
                                            Stroke::new(1.0, ch.warn)
                                        } else {
                                            Stroke::new(2.0, accent)
                                        };
                                        match spot.as_ref().map(|s| s.zone) {
                                            Some(Zone::Before) | Some(Zone::After) => {
                                                let before = spot
                                                    .as_ref()
                                                    .is_some_and(|s| s.zone == Zone::Before);
                                                let x = rect.left()
                                                    + PAD_L
                                                    + *depth as f32 * indent
                                                    + CHEVRON / 2.0;
                                                let y =
                                                    if before { rect.top() } else { rect.bottom() };
                                                ui.painter().hline(
                                                    x..=(rect.right() - 8.0),
                                                    y,
                                                    line,
                                                );
                                                ui.painter().circle_filled(
                                                    Pos2::new(x, y),
                                                    3.0,
                                                    if drag_illegal { ch.warn } else { accent },
                                                );
                                            }
                                            Some(Zone::Child) => {
                                                // docs/28 §5.3：成为子级时整行 6% 强调底，无描边。
                                                // docs/28 §5.6：非法落点不画目标底，改红 1px 线。
                                                if drag_illegal {
                                                    ui.painter().rect_stroke(
                                                        *rect,
                                                        6.0,
                                                        Stroke::new(1.0, ch.warn),
                                                        egui::StrokeKind::Inside,
                                                    );
                                                } else {
                                                    let wash =
                                                        egui::Color32::from_rgba_unmultiplied(
                                                            accent.r(),
                                                            accent.g(),
                                                            accent.b(),
                                                            15,
                                                        );
                                                    ui.painter().rect_filled(*rect, 6.0, wash);
                                                }
                                            }
                                            None => {}
                                        }
                                    }
                                }
                                if released {
                                    if let (Some(s), Some((target, _depth))) = (spot, drop) {
                                        let ids = if drag_ids.is_empty() {
                                            vec![id.clone()]
                                        } else {
                                            drag_ids.clone()
                                        };
                                        // 非法落点（自身/后代）已在预览期判定：松开直接放弃。
                                        if !drag_illegal {
                                            let (new_parent, index) =
                                                move_target(target, s.zone);
                                            if !ids.iter().any(|dragged| dragged == &new_parent)
                                            {
                                                action = Some(if ids.len() > 1 {
                                                    Action::MoveMany {
                                                        ids,
                                                        new_parent,
                                                        index,
                                                    }
                                                } else {
                                                    Action::Move {
                                                        id: id.clone(),
                                                        new_parent,
                                                        index,
                                                    }
                                                });
                                            }
                                        }
                                    }
                                    dragging = None;
                                    drag_ids.clear();
                                    self.drag_hover = None;
                                }
                            }
                        });
                    });
                });
        });
        // 拖拽幽灵：半透明行卡跟随指针；多选时第二行给数量和首行标题。
        if let Some(first) = dragging.as_deref() {
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                let row = self.rows.iter().find(|r| r.id == first);
                let full = row.map(|r| r.text.clone()).unwrap_or_default();
                let mut title: String = full.chars().take(24).collect();
                if full.chars().count() > 24 {
                    title.push('…');
                }
                let count = drag_ids.len().max(1);
                let screen = ui.ctx().content_rect();
                crate::trace::event(&format!(
                    "outline_drag_ghost id={first} text={title:?} ptr=({:.1},{:.1})",
                    p.x, p.y
                ));
                let size = Vec2::new(220.0, if count > 1 { 50.0 } else { 32.0 });
                let pos = Pos2::new(
                    (p.x + 18.0).min(screen.right() - size.x - 8.0),
                    (p.y + 14.0).min(screen.bottom() - size.y - 8.0),
                );
                egui::Area::new(egui::Id::unique("mw-outline-drag-ghost"))
                    .order(egui::Order::Tooltip)
                    .fixed_pos(pos)
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::NONE
                            .fill(ch.panel2.gamma_multiply(0.9)) // docs/28 §5.6：行幽灵 90% 不透明度
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.set_max_width(size.x - 20.0);
                                ui.label(
                                    egui::RichText::new(if count > 1 {
                                        format!("{count} 项")
                                    } else {
                                        title.clone()
                                    })
                                    .size(Type::SMALL)
                                    .family(egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()))
                                    .color(ch.text),
                                );
                                if count > 1 {
                                    ui.label(
                                        egui::RichText::new(title)
                                            .size(Type::CAPTION)
                                            .color(ch.dim),
                                    );
                                }
                            });
                    });
            }
        }
        if scroll_hit {
            self.scroll_to = None;
        }
        // 编辑/拖拽状态放回面板；下一帧接着画输入框与落点。
        self.editing = editing;
        self.via_edit = via_edit;
        self.dragging = dragging;
        self.drag_ids = drag_ids;
        self.deferred = deferred;

        // ---- 大纲键盘：整列是一张表，方向键在行间走，Tab 改层级 ----
        // 可见行下标已由 `prepare_filter` 缓存，这里只做一次线性查找定位当前行。
        // 验收钩子钉住的行优先：概念节点没有 `parent_id`，画布的 `select_many`
        // 会把它从选择里筛掉，于是每帧都回落到第 0 行（根），键位永远打在根上。
        let primary = self.pin.as_deref().or(primary);
        let active = {
            let vis = &self.visible;
            primary
                .and_then(|id| vis.iter().position(|&i| self.rows[i].id == id))
                .map(|p| (p, self.rows[vis[p]].id.clone()))
                .or_else(|| vis.first().map(|&i| (0, self.rows[i].id.clone())))
        };
        // 上一/下一行的 id 先取出来：键盘分支里还要改 `self.editing` 等字段。
        let (prev_id, next_id) = {
            let vis = &self.visible;
            let at = |p: usize| vis.get(p).map(|&i| self.rows[i].id.clone());
            let pos = active.as_ref().map(|(p, _)| *p);
            (
                pos.and_then(|p| p.checked_sub(1)).and_then(at),
                pos.and_then(|p| at(p + 1)),
            )
        };
        // 键盘事件处理完后把焦点钉回这一行：方向键/Tab 的巡游已经发生，不钉回去
        // 焦点会落到顶栏（实测下一次回车直接打开菜单）。
        let mut focus_target: Option<String> = None;
        let active_id = active.as_ref().map(|(_, id)| id.clone());
        if blocked {
            // 前台浮层是唯一键盘主人：大纲只画不收键，也不改选择、缩进或结构。
            let keys: Vec<String> = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Key {
                            key, pressed: true, ..
                        } => Some(format!("{key:?}")),
                        _ => None,
                    })
                    .collect()
            });
            for key in keys {
                crate::trace::event(&format!("outline_key_blocked key={key}"));
            }
        } else if let Some((pos, id)) = active {
            if filter_owned {
                // 搜索框里：Tab 跳回行表，↑↓ 在命中之间走，Esc 逐级退。
                if ui.input(|i| i.key_pressed(Key::Tab)) {
                    focus_target = Some(id);
                } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                    match esc_step(self.filter.is_empty(), search_open, focused) {
                        EscStep::ClearFilter => self.clear_filter(),
                        EscStep::CloseSearch => {
                            self.close_search();
                            focus_target = Some(id.clone());
                            self.keyboard_focus = true;
                            ui.ctx().request_repaint();
                        }
                        EscStep::Unfocus => action = Some(Action::Unfocus),
                        EscStep::Close => action = Some(Action::Close),
                    }
                } else {
                    let dir = if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                        Some(1)
                    } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                        Some(-1)
                    } else {
                        None
                    };
                    if let Some(dir) = dir {
                        if let Some(target) = self.step_match(primary, dir) {
                            self.scroll_to = Some(target.clone());
                            if action.is_none() {
                                action = Some(Action::Select(target));
                            }
                        }
                    }
                }
            } else if !editing_active && self.editing.is_none() {
                let (row_text, has_children, collapsed, parent, task, is_focus_root) = {
                    let r = &self.rows[self.visible[pos]];
                    (
                        r.text.clone(),
                        r.has_children,
                        r.collapsed,
                        r.parent.clone(),
                        r.task,
                        focus_root.as_deref() == Some(r.id.as_str()),
                    )
                };
                let mut act = None;
                let multi = sel.len() > 1;
                let selected_now: Vec<String> = if multi {
                    self.selected_visible(&sel)
                } else {
                    Vec::new()
                };
                // 批量结构操作只保留能合法移动的行：聚焦根不能缩进/删除；
                // 聚焦根的直接子节点不能升级（升出去就离开视野）。
                let mut indent_ids = selected_now.clone();
                let mut outdent_ids = selected_now.clone();
                let mut delete_ids = selected_now.clone();
                if multi {
                    indent_ids.retain(|x| Some(x.as_str()) != focus_root.as_deref());
                    delete_ids.retain(|x| Some(x.as_str()) != focus_root.as_deref());
                    outdent_ids.retain(|x| {
                        if Some(x.as_str()) == focus_root.as_deref() {
                            return false;
                        }
                        self.rows.iter().find(|r| &r.id == x).is_some_and(|r| {
                            !(focused && r.parent.as_deref() == focus_root.as_deref())
                        })
                    });
                }
                let ctrl = Modifiers {
                    ctrl: true,
                    ..Modifiers::NONE
                };
                let shift = Modifiers {
                    shift: true,
                    ..Modifiers::NONE
                };
                // Shift+↑/↓：从锚点扩一行；普通 ↑/↓ 仍在 `take_row_key` 里。
                let extend_dir = if take_exact_key(ui, shift, Key::ArrowUp) {
                    Some(-1)
                } else if take_exact_key(ui, shift, Key::ArrowDown) {
                    Some(1)
                } else {
                    None
                };
                if take_exact_key(ui, ctrl, Key::A) {
                    // Ctrl+A：当前可见范围全选（筛选/聚焦后就是眼前这些行）。
                    let vis = self.visible_ids();
                    if !vis.is_empty() {
                        let p = primary
                            .and_then(|p| vis.iter().find(|id| id.as_str() == p).cloned())
                            .unwrap_or_else(|| vis[0].clone());
                        self.selection_anchor = Some(p.clone());
                        self.keyboard_focus = true;
                        focus_target = Some(p.clone());
                        act = Some(Action::SetSelection {
                            ids: vis,
                            primary: p,
                        });
                    }
                } else if let Some(dir) = extend_dir {
                    let anchor = self
                        .selection_anchor
                        .clone()
                        .or_else(|| primary.map(str::to_string))
                        .unwrap_or_else(|| id.clone());
                    let vlen = self.visible.len();
                    if vlen > 0 {
                        let target_pos =
                            (pos as isize + dir as isize).clamp(0, vlen as isize - 1) as usize;
                        let target = self.rows[self.visible[target_pos]].id.clone();
                        let vis = self.visible_ids();
                        let ids = range_between(&vis, &anchor, &target);
                        self.selection_anchor = Some(anchor);
                        self.keyboard_focus = true;
                        self.scroll_to = Some(target.clone());
                        focus_target = Some(target.clone());
                        act = Some(Action::SetSelection {
                            ids,
                            primary: target,
                        });
                    }
                } else if take_exact_key(ui, ctrl, Key::D) {
                    act = Some(Action::Duplicate);
                } else if let Some(key) = take_row_key(ui) {
                    match key {
                        // 概念图（CM-07）：命题网没有「兄弟顺序」也没有「父子层级」，
                        // 树语义的按键一律不接——接了就等于把 `children` 写进图里
                        // （`docs/30` §5.2 明令禁止）。留一条 trace，按了没反应能查到原因。
                        RowKey::MoveUp | RowKey::MoveDown | RowKey::Outdent if self.concept_map => {
                            crate::trace::event(&format!(
                                "outline_tree_key_ignored id={id} key=outdent_or_move"
                            ));
                        }
                        // Tab 在概念图里不是「缩进」——命题网没有缩进，层级只由边表达。
                        // 它等于「新建概念 + 连一条边」（docs/30 §5.6.1）。
                        RowKey::Indent if self.concept_map => {
                            crate::trace::event(&format!("outline_concept_link from={id}"));
                            act = Some(Action::LinkConcept(id.clone()));
                        }
                        // Shift+Delete：只断「进来那条边」，概念留成孤立根。
                        RowKey::Unlink if self.concept_map => {
                            let (via_from, via_id) = self
                                .rows
                                .iter()
                                .find(|r| r.id == id)
                                .map(|r| (r.via_from.clone(), r.via_id.clone()))
                                .unwrap_or((None, None));
                            match (via_from, via_id) {
                                (Some(from), Some(rel)) => {
                                    crate::trace::event(&format!(
                                        "outline_concept_unlink id={id} from={from} rel={rel}"
                                    ));
                                    act = Some(Action::UnlinkConcept { from, rel });
                                }
                                // 根行（含主概念与孤立概念）没有进来的边：明确记一笔，
                                // 免得用户以为「按了没反应」是 bug。
                                _ => {
                                    let (d, v) = self
                                        .rows
                                        .iter()
                                        .find(|r| r.id == id)
                                        .map(|r| (r.depth, r.via.clone()))
                                        .unwrap_or((0, None));
                                    crate::trace::event(&format!(
                                        "outline_concept_unlink_skipped id={id} depth={d} via={v:?} reason=no_incoming_edge"
                                    ))
                                }
                            }
                        }
                        // Shift+F2：就地改这条边的短语（行文字仍是概念名）。
                        RowKey::RenameVia if self.concept_map => {
                            let has_edge = self
                                .rows
                                .iter()
                                .find(|r| r.id == id)
                                .is_some_and(|r| r.via_id.is_some());
                            if has_edge {
                                crate::trace::event(&format!("outline_concept_via_edit id={id}"));
                                act = Some(Action::EditVia(id.clone()));
                            } else {
                                let (d, v) = self
                                    .rows
                                    .iter()
                                    .find(|r| r.id == id)
                                    .map(|r| (r.depth, r.via.clone()))
                                    .unwrap_or((0, None));
                                crate::trace::event(&format!(
                                    "outline_concept_via_edit_skipped id={id} depth={d} via={v:?} reason=no_incoming_edge"
                                ));
                            }
                        }
                        // 树文档里这两个键没有语义：不做事，也不落 trace（正常按键）。
                        RowKey::Unlink | RowKey::RenameVia => {}
                        // Enter 在概念图里 = 新建一个概念（`AddFloatingTopic`），不是加子主题。
                        RowKey::AddSibling if self.concept_map => {
                            act = Some(Action::NewConcept(id.clone()));
                        }
                        RowKey::MoveUp => {
                            if multi {
                                act = Some(Action::MoveStepMany {
                                    ids: selected_now.clone(),
                                    delta: -1,
                                });
                            } else if !is_focus_root {
                                act = Some(Action::MoveStep {
                                    id: id.clone(),
                                    delta: -1,
                                });
                            }
                        }
                        RowKey::MoveDown => {
                            if multi {
                                act = Some(Action::MoveStepMany {
                                    ids: selected_now.clone(),
                                    delta: 1,
                                });
                            } else if !is_focus_root {
                                act = Some(Action::MoveStep {
                                    id: id.clone(),
                                    delta: 1,
                                });
                            }
                        }
                        // 聚焦根在视图里没有同级：Enter 落成子节点，避免新行跑到视野外面。
                        RowKey::AddSibling => {
                            act = Some(if is_focus_root {
                                Action::AddChild(id.clone())
                            } else {
                                Action::AddSibling(id.clone())
                            });
                        }
                        RowKey::Outdent => {
                            if multi {
                                if !outdent_ids.is_empty() {
                                    act = Some(Action::OutdentMany(outdent_ids.clone()));
                                }
                            } else {
                                // 聚焦根没有上一级；它的直接子节点升出去就离开了视野，都不接。
                                let leaves_scope =
                                    focused && parent.as_deref() == focus_root.as_deref();
                                if !is_focus_root && !leaves_scope {
                                    act = Some(Action::Outdent(id.clone()));
                                }
                            }
                        }
                        RowKey::Indent => {
                            if multi {
                                if !indent_ids.is_empty() {
                                    act = Some(Action::IndentMany(indent_ids.clone()));
                                }
                            } else if !is_focus_root {
                                act = Some(Action::Indent(id.clone()));
                            }
                        }
                        RowKey::Delete if self.via_edit.is_some() => {
                            // 短语框立着时 Backspace/Delete 归它：删字，不许断边。
                            crate::trace::event("outline_delete_blocked_by_via_edit");
                        }
                        RowKey::Delete => {
                            if multi {
                                if !delete_ids.is_empty() {
                                    act = Some(Action::DeleteMany(delete_ids.clone()));
                                }
                            } else {
                                act = Some(Action::Delete(id.clone()));
                            }
                        }
                        RowKey::Rename => {
                            self.editing =
                                Some(Edit::start(&id, row_text.clone(), row_text, true, None));
                            self.keyboard_focus = true;
                            self.scroll_to = Some(id.clone());
                        }
                        RowKey::Prev => {
                            if let Some(target) = prev_id.clone() {
                                self.selection_anchor = Some(target.clone());
                                self.scroll_to = Some(target.clone());
                                act = Some(Action::Select(target));
                            }
                        }
                        RowKey::Next => {
                            if let Some(target) = next_id.clone() {
                                self.selection_anchor = Some(target.clone());
                                self.scroll_to = Some(target.clone());
                                act = Some(Action::Select(target));
                            }
                        }
                        RowKey::Collapse => {
                            if has_children && !collapsed {
                                act = Some(Action::Toggle(id.clone()));
                            } else if let Some(parent) = parent.clone() {
                                self.scroll_to = Some(parent.clone());
                                act = Some(Action::Select(parent));
                            }
                        }
                        RowKey::Expand => {
                            if collapsed && has_children {
                                act = Some(Action::Toggle(id.clone()));
                            } else if let Some(child) = self
                                .rows
                                .iter()
                                .find(|r| r.parent.as_deref() == Some(id.as_str()))
                                .map(|r| r.id.clone())
                            {
                                self.scroll_to = Some(child.clone());
                                act = Some(Action::Select(child));
                            }
                        }
                        RowKey::Task => {
                            if multi {
                                act = Some(Action::SetTaskMany {
                                    ids: selected_now.clone(),
                                    task: next_task(task),
                                });
                            } else {
                                act = Some(Action::SetTask {
                                    id: id.clone(),
                                    task: next_task(task),
                                });
                            }
                        }
                        RowKey::Toggle => {
                            if has_children {
                                act = Some(Action::Toggle(id.clone()));
                            }
                        }
                        RowKey::Focus => {
                            if !is_focus_root {
                                act = Some(Action::Focus(id.clone()));
                            }
                        }
                        RowKey::UnfocusHere => {
                            // docs/28 §5.5：Ctrl+[ 返回上一级；非聚焦态无可返回，空操作。
                            if focused {
                                act = Some(Action::Unfocus);
                            }
                        }
                        RowKey::ExpandAllToggle => {
                            // docs/28 §5.5：Alt+Shift+. 全部折叠/展开——按当前是否有
                            // 展开的行取反，一次 SetCollapsedAll 一条撤销。
                            let any_open = self.rows.iter().any(|r| r.has_children && !r.collapsed);
                            act = Some(Action::SetCollapsedAll {
                                collapsed: any_open,
                            });
                        }
                        RowKey::SelectAllVisible => {
                            // docs/28 §5.5：Ctrl+A 全选当前可见行（不跨折叠）。
                            let ids: Vec<String> = self
                                .visible
                                .iter()
                                .map(|&i| self.rows[i].id.clone())
                                .collect();
                            if let Some(last) = ids.last().cloned() {
                                self.selection_anchor = None;
                                act = Some(Action::SetSelection { ids, primary: last });
                            }
                        }
                        sel_key @ (RowKey::SelectPrev | RowKey::SelectNext) => {
                            // docs/28 §5.5：Shift+↑/↓ 以锚点为界扩展多选；
                            // 锚点保持不动，继续按方向扩展/收缩。
                            let anchor = self
                                .selection_anchor
                                .clone()
                                .or_else(|| primary.map(str::to_string))
                                .unwrap_or_else(|| id.clone());
                            let target = if matches!(sel_key, RowKey::SelectPrev) {
                                prev_id.clone()
                            } else {
                                next_id.clone()
                            };
                            if let Some(t) = target {
                                let ids = range_between(&self.visible_ids(), &anchor, &t);
                                act = Some(Action::SetSelection { ids, primary: t });
                            }
                        }
                        RowKey::Close => {
                            if multi {
                                // 多选时 Esc 先收敛到主选行，不直接退出大纲。
                                if let Some(p) = primary.map(str::to_string) {
                                    self.selection_anchor = Some(p.clone());
                                    self.keyboard_focus = true;
                                    focus_target = Some(p.clone());
                                    act = Some(Action::SetSelection {
                                        ids: vec![p.clone()],
                                        primary: p,
                                    });
                                }
                            } else {
                                match esc_step(!self.filter_active(), search_open, focused) {
                                    EscStep::ClearFilter => self.clear_filter(),
                                    EscStep::CloseSearch => {
                                        self.close_search();
                                        focus_target = Some(id.clone());
                                        self.keyboard_focus = true;
                                        ui.ctx().request_repaint();
                                    }
                                    EscStep::Unfocus => act = Some(Action::Unfocus),
                                    EscStep::Close => act = Some(Action::Close),
                                }
                            }
                        }
                    }
                }
                // 同一帧里「改名提交」与「Tab 缩进」可能一起到：提交先走，
                // 缩进排到下一帧；否则刚提交的文字会被覆盖（实拍：Tab 一按，标题变未命名）。
                if let Some(act) = act {
                    if action.is_none() {
                        action = Some(act);
                    } else {
                        self.deferred = Some(act);
                    }
                }
                if nav_key && focus_target.is_none() {
                    crate::trace::event(&format!("outline_focus_target {id}"));
                    focus_target = Some(id.clone());
                }
            }
        } else if !editing_active && self.editing.is_none() && filter_owned {
            // docs/28 §5.7：Enter 下一个命中，Shift+Enter 上一个；焦点留在筛选框里。
            let step = if take_exact_key(ui, Modifiers::NONE, Key::Enter) {
                Some(1)
            } else if take_exact_key(ui, Modifiers::SHIFT, Key::Enter) {
                Some(-1)
            } else {
                None
            };
            if let Some(dir) = step {
                if let Some(target) = self.step_match(primary, dir) {
                    crate::trace::event(&format!("outline_filter_step dir={dir} id={target}"));
                    self.scroll_to = Some(target.clone());
                    action = Some(Action::Select(target));
                    self.keyboard_focus = true;
                    self.search_focus = true;
                }
            }
            // 搜索没有命中：Esc 按同一层级退——清条件、关搜索、退聚焦、退出大纲。
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                match esc_step(!self.filter_active(), search_open, focused) {
                    EscStep::ClearFilter => self.clear_filter(),
                    EscStep::CloseSearch => self.close_search(),
                    EscStep::Unfocus => action = Some(Action::Unfocus),
                    EscStep::Close => action = Some(Action::Close),
                }
            }
        } else if !editing_active
            && self.editing.is_none()
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            match esc_step(true, search_open, focused) {
                EscStep::CloseSearch => self.close_search(),
                EscStep::Unfocus => action = Some(Action::Unfocus),
                _ => action = Some(Action::Close),
            }
        }
        // 搜索行关闭后把焦点送回当前行；`active` 已被上面的分支消费，用提前留下的 id。
        if self.refocus_rows {
            if let Some(id) = active_id {
                self.keyboard_focus = true;
                focus_target = Some(id);
            }
            self.refocus_rows = false;
        }
        // 键盘路径算出的焦点目标优先；行点击/提交在行循环里已经排好的请求不能在这里被清掉。
        if focus_target.is_some() && self.via_edit.is_none() && !self.is_editing() {
            self.focus_request = focus_target;
        }
        action
    }
}

/// `Frame::popup` 的上下内边距之和（实测 14，取 16 留余量）。
///
/// 它不属于滚动区视口，算「弹层总高」时要单独扣掉，否则底边会越过窗口。
pub(crate) const POPUP_FRAME_PAD: f32 = 16.0;

/// 弹层最大高度：锚点下方到窗口底边（各留 `margin`），且不小于 `floor`。
///
/// 标签弹层原来没有高度上限：条目一多就长出窗口底部，既看不见最后几条、也没有滚动条。
/// 这条把它压成「封顶」，配合 `ScrollArea` 就能滚（docs/30 §5.11）。
pub(crate) fn popup_max_height(screen_bottom: f32, anchor_y: f32, margin: f32, floor: f32) -> f32 {
    (screen_bottom - margin - anchor_y).max(floor)
}
// 大纲是纯绘制 + 读文档，逻辑很薄；真正的性能约束（缓存命中）靠"不改就不重建"这条。
#[cfg(test)]
mod tests {
    use super::label_hl_step;

    /// 弹层封顶：贴底边时只剩 floor，锚点在上方时就是下方那一段（docs/30 §5.11）。
    #[test]
    fn popup_max_height_caps_to_window_bottom() {
        assert_eq!(super::popup_max_height(800.0, 700.0, 8.0, 120.0), 120.0);
        assert_eq!(super::popup_max_height(800.0, 300.0, 8.0, 120.0), 492.0);
        assert_eq!(super::popup_max_height(100.0, 90.0, 8.0, 120.0), 120.0);
    }
    use super::{
        esc_step, matches_filter, range_between, row_key, row_keys_owned, split_text, type_char,
        union_ids, EscStep, Outline, RowKey, Theme,
    };
    use eframe::egui;
    use eframe::egui::{Key, Modifiers};
    use mw_core::{get_theme, Command, Document, NoteCard, TaskState};

    fn theme() -> Theme {
        get_theme("snow")
    }

    fn doc_with(children: &[&str]) -> Document {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        for c in children {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: (*c).to_string(),
                style: None,
            })
            .unwrap();
        }
        doc
    }

    fn vis(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn row_keys_are_owned_only_without_foreground_or_field() {
        assert!(row_keys_owned(false, false, false));
        assert!(!row_keys_owned(true, false, false), "前台浮层时大纲不收键");
        assert!(!row_keys_owned(false, true, false), "行内编辑时行表不收键");
        assert!(!row_keys_owned(false, false, true), "搜索框持有键盘");
        assert!(!row_keys_owned(true, true, true));
    }

    #[test]
    fn range_selection_follows_visible_order_in_both_directions() {
        let visible = vis(&["a", "b", "c", "d"]);
        assert_eq!(range_between(&visible, "b", "d"), vis(&["b", "c", "d"]));
        assert_eq!(
            range_between(&visible, "d", "b"),
            vis(&["b", "c", "d"]),
            "向上扩也按可见顺序"
        );
        assert_eq!(range_between(&visible, "x", "c"), vis(&["c"]));
    }

    #[test]
    fn union_selection_keeps_first_time_order_and_dedupes() {
        let current = vis(&["a", "c"]);
        let extra = vis(&["b", "c", "d"]);
        assert_eq!(union_ids(&current, &extra), vis(&["a", "c", "b", "d"]));
    }

    #[test]
    fn outline_filter_is_case_insensitive_and_empty_matches_all() {
        assert!(matches_filter("分支 A", ""));
        assert!(matches_filter("分支 A", "分支"));
        assert!(matches_filter("Branch", "branch"));
        assert!(!matches_filter("分支 A", "叶"));
    }

    /// docs/28 §5.7：条件 AND、祖先上下文、命中区间、折叠分支穿透。
    #[test]
    fn filter_conditions_and_ancestor_context_with_hit_ranges() {
        let mut doc = doc_with(&["甲组", "乙组"]);
        let jia = doc
            .nodes
            .values()
            .find(|n| n.text == "甲组")
            .unwrap()
            .id
            .clone();
        for text in ["命中一", "其他"] {
            doc.apply(&Command::AddNode {
                parent: jia.clone(),
                index: None,
                text: text.into(),
                style: None,
            })
            .unwrap();
        }
        let hit = doc
            .nodes
            .values()
            .find(|n| n.text == "命中一")
            .unwrap()
            .id
            .clone();
        doc.apply(&Command::SetTask {
            id: hit.clone(),
            task: Some(TaskState::Todo),
        })
        .unwrap();
        doc.apply(&Command::AddLabel {
            ids: vec![hit.clone()],
            label: "验收".into(),
        })
        .unwrap();

        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        o.set_filter("命中");
        o.prepare_filter(&doc);
        assert_eq!(o.hits.len(), 1);
        assert_eq!(o.visible.len(), 3, "根 + 甲组 + 命中一（祖先上下文）");
        assert_eq!(o.hit_ranges.get(&o.hits[0]), Some(&(0, 2)));

        // 文本 AND 任务：同一条仍在；换一个不存在的状态就归零。
        o.set_filter_conditions(Some(TaskState::Todo), false, None);
        o.prepare_filter(&doc);
        assert_eq!(o.hits.len(), 1);
        o.set_filter_conditions(Some(TaskState::Done), false, None);
        o.prepare_filter(&doc);
        assert_eq!(o.hits.len(), 0);
        assert!(o.visible.is_empty());

        // 只按标签；标签再与文本 AND。
        o.set_filter("");
        o.set_filter_conditions(None, false, Some("验收"));
        o.prepare_filter(&doc);
        assert_eq!(o.hits.len(), 1);
        o.set_filter("命中");
        o.prepare_filter(&doc);
        assert_eq!(o.hits.len(), 1);
        o.set_filter("其他");
        o.prepare_filter(&doc);
        assert_eq!(o.hits.len(), 0, "标签 AND 文本：其他行没有该标签");

        // 折叠分支里的命中也要搜得到（筛选态穿透折叠，不改文档折叠位）。
        doc.apply(&Command::SetCollapsed {
            id: jia.clone(),
            collapsed: true,
        })
        .unwrap();
        let mut o2 = Outline::new();
        o2.set_filter("命中");
        o2.sync(&doc, &t);
        o2.prepare_filter(&doc);
        assert_eq!(o2.hits.len(), 1, "折叠不应让命中消失");
        assert!(doc.node(&jia).unwrap().collapsed, "文档折叠位不变");
    }

    /// docs/28 §5.7：Enter/Shift+Enter 只在命中行之间走，到头回绕；
    /// 祖先上下文不进命中表，否则「下一个」会停在灰行上。
    #[test]
    fn filter_steps_only_through_hits_and_wraps() {
        let doc = doc_with(&["甲", "乙", "丙"]);
        let a = doc
            .nodes
            .values()
            .find(|n| n.text == "甲")
            .unwrap()
            .id
            .clone();
        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        o.set_filter("甲");
        o.prepare_filter(&doc);
        assert_eq!(o.search_hits(), 1);
        assert_eq!(o.step_match(None, 1).as_deref(), Some(a.as_str()));
        assert_eq!(o.step_match(None, -1).as_deref(), Some(a.as_str()));
        assert_eq!(
            o.step_match(Some(doc.root_id.as_str()), 1).as_deref(),
            Some(a.as_str()),
            "从祖先（根）出发也落在第一个命中"
        );
    }

    /// 大纲键位（W51/S09）：修饰键精确匹配，Shift+Tab 是升级、Ctrl+Enter 建子节点，
    /// CER-11：增量折叠/展开必须与全量重建逐字段一致（行表形状、父链、竖线、
    /// 折叠位、可见表），否则 20k 下省下的 29ms 会换成错行。
    #[test]
    fn collapse_and_expand_update_rows_incrementally_like_a_full_rebuild() {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        let mut branches = Vec::new();
        for b in 0..3 {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: format!("分支 {b}"),
                style: None,
            })
            .unwrap();
            let branch = doc.node(&root).unwrap().children[b].clone();
            for leaf in 0..3 {
                doc.apply(&Command::AddNode {
                    parent: branch.clone(),
                    index: None,
                    text: format!("叶 {b}-{leaf}"),
                    style: None,
                })
                .unwrap();
            }
            branches.push(branch);
        }
        // 第一个分支再挂一个孙节点，验证 depth/竖线掩码在补行时也对。
        let grandparent = doc.node(&branches[0]).unwrap().children[1].clone();
        doc.apply(&Command::AddNode {
            parent: grandparent,
            index: None,
            text: "孙".into(),
            style: None,
        })
        .unwrap();
        let leaf_with_child = doc.node(&branches[0]).unwrap().children[1].clone();

        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        let sig = |o: &Outline| -> Vec<(String, u16, usize, Option<String>, bool, u64)> {
            o.rows
                .iter()
                .map(|r| {
                    (
                        r.id.clone(),
                        r.depth,
                        r.index,
                        r.parent.clone(),
                        r.last,
                        r.guide_mask,
                    )
                })
                .collect()
        };
        let full = sig(&o);

        doc.apply(&Command::SetCollapsed {
            id: branches[0].clone(),
            collapsed: true,
        })
        .unwrap();
        assert!(
            o.apply_collapse(&doc, &t, &branches[0], true),
            "非筛选态应走增量"
        );
        let mut fresh = Outline::new();
        fresh.sync(&doc, &t);
        assert_eq!(sig(&o), sig(&fresh), "折叠后的行表要与全量重建一致");
        assert_eq!(o.rev, doc.revision, "增量已同步修订号");
        assert_eq!(o.visible.len(), o.rows.len(), "无筛选时可见表 = 全表");

        doc.apply(&Command::SetCollapsed {
            id: branches[0].clone(),
            collapsed: false,
        })
        .unwrap();
        assert!(o.apply_collapse(&doc, &t, &branches[0], false));
        let mut fresh = Outline::new();
        fresh.sync(&doc, &t);
        assert_eq!(sig(&o), sig(&fresh), "展开后的行表要与全量重建一致");
        assert_eq!(sig(&o), full, "展开回到原始行表");

        // 深层节点单独展开/折叠也走增量。
        doc.apply(&Command::SetCollapsed {
            id: leaf_with_child.clone(),
            collapsed: true,
        })
        .unwrap();
        assert!(o.apply_collapse(&doc, &t, &leaf_with_child, true));
        assert!(
            !o.rows.iter().any(|r| r.text == "孙"),
            "孙节点随叶子折叠消失"
        );
        assert!(o.apply_collapse(&doc, &t, &leaf_with_child, false));
        assert!(o.rows.iter().any(|r| r.text == "孙"), "展开后回来");
    }

    /// Alt+Enter 在大纲里是「聚焦子树」——只认这一个 Alt 组合。
    #[test]
    fn outline_keys_are_exact_and_shift_tab_outdents() {
        let none = Modifiers::NONE;
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };

        assert_eq!(row_key(none, Key::Enter), Some(RowKey::AddSibling));
        // docs/28 §5.5：Ctrl+Enter 切换任务完成（docs/06 旧「加子级」组合作废）。
        assert_eq!(row_key(ctrl, Key::Enter), Some(RowKey::Task));
        // 先判 Shift+Tab：若先问 Tab，egui 的「忽略多余 Shift」会把升级吃成缩进。
        assert_eq!(row_key(shift, Key::Tab), Some(RowKey::Outdent));
        assert_eq!(row_key(none, Key::Tab), Some(RowKey::Indent));
        assert_eq!(row_key(ctrl, Key::T), Some(RowKey::Task));
        assert_eq!(row_key(ctrl_shift, Key::ArrowUp), Some(RowKey::MoveUp));
        assert_eq!(row_key(ctrl_shift, Key::ArrowDown), Some(RowKey::MoveDown));
        assert_eq!(row_key(none, Key::ArrowUp), Some(RowKey::Prev));
        assert_eq!(row_key(none, Key::ArrowDown), Some(RowKey::Next));
        assert_eq!(row_key(none, Key::ArrowLeft), Some(RowKey::Collapse));
        assert_eq!(row_key(none, Key::ArrowRight), Some(RowKey::Expand));
        assert_eq!(row_key(none, Key::Delete), Some(RowKey::Delete));
        assert_eq!(row_key(none, Key::Backspace), Some(RowKey::Delete));

        // 标签弹层高光绕行（纯函数，压住 n=1 与两端回绕）
        assert_eq!(label_hl_step(0, 1, false), 0);
        assert_eq!(label_hl_step(0, 1, true), 0);
        assert_eq!(label_hl_step(0, 3, false), 1);
        assert_eq!(label_hl_step(2, 3, false), 0);
        assert_eq!(label_hl_step(0, 3, true), 2);
        assert_eq!(label_hl_step(2, 3, true), 1);
        assert_eq!(label_hl_step(0, 0, false), 0);
        assert_eq!(row_key(none, Key::F2), Some(RowKey::Rename));
        assert_eq!(row_key(none, Key::Space), Some(RowKey::Toggle));
        assert_eq!(row_key(none, Key::Escape), Some(RowKey::Close));

        // 画布同款手势：Alt+Enter 聚焦子树；其余 Alt 组合与未定义组合不认领。
        assert_eq!(row_key(alt, Key::Enter), Some(RowKey::Focus));
        assert_eq!(row_key(alt, Key::Tab), None);
        assert_eq!(
            row_key(
                Modifiers {
                    alt: true,
                    ctrl: true,
                    ..Modifiers::NONE
                },
                Key::Enter
            ),
            None
        );
        // docs/28 §5.5：Shift+↑/↓ 扩展多选（X20 未定义）。
        assert_eq!(row_key(shift, Key::ArrowUp), Some(RowKey::SelectPrev));
        assert_eq!(row_key(ctrl, Key::Delete), None);
        assert_eq!(row_key(none, Key::A), None);
        assert_eq!(row_key(ctrl_shift, Key::T), None);
        assert_eq!(row_key(alt, Key::T), None);
    }

    /// Esc 四级退路（O02）：清筛选 → 关搜索 → 退聚焦 → 退视图，顺序不能反。
    #[test]
    fn escape_unwinds_filter_then_search_then_focus_then_the_view() {
        assert_eq!(esc_step(false, false, false), EscStep::ClearFilter);
        assert_eq!(
            esc_step(false, true, true),
            EscStep::ClearFilter,
            "有筛选词时，哪怕聚焦且搜索开着，也先清词"
        );
        assert_eq!(
            esc_step(true, true, true),
            EscStep::CloseSearch,
            "搜索行先关，聚焦下一轮再退"
        );
        assert_eq!(esc_step(true, false, true), EscStep::Unfocus);
        assert_eq!(esc_step(true, false, false), EscStep::Close);
    }

    fn nested_doc() -> (Document, String, String, String, String) {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        let add = |doc: &mut Document, parent: &str, text: &str| {
            doc.apply(&Command::AddNode {
                parent: parent.into(),
                index: None,
                text: text.into(),
                style: None,
            })
            .unwrap();
            doc.node(parent).unwrap().children.last().unwrap().clone()
        };
        let a = add(&mut doc, &root, "甲");
        let a1 = add(&mut doc, &a, "甲一");
        let a1x = add(&mut doc, &a1, "甲一甲");
        let b = add(&mut doc, &root, "乙");
        let _b1 = add(&mut doc, &b, "乙一");
        (doc, a, a1, a1x, b)
    }

    /// 「只显示到第 N 级」：根始终展开，N=1 是根 + 第一层；一条 Batch 一次撤销。
    #[test]
    fn collapse_level_is_one_batch_and_keeps_the_root_open() {
        let (mut doc, a, a1, _a1x, b) = nested_doc();
        let cmd = super::collapse_level_command(&doc, None, 1).expect("有可折叠的节点");
        assert!(matches!(&cmd, Command::Batch { .. }), "必须合成一次撤销");
        doc.apply(&cmd).unwrap();
        assert!(!doc.node(&doc.root_id).unwrap().collapsed, "根永远展开");
        assert!(doc.node(&a).unwrap().collapsed);
        assert!(doc.node(&b).unwrap().collapsed);
        assert!(doc.node(&a1).unwrap().collapsed, "更深的节点一并折起");

        doc.undo().expect("一次撤销");
        assert!(!doc.node(&a).unwrap().collapsed);
        assert!(!doc.node(&b).unwrap().collapsed);
        assert!(!doc.node(&a1).unwrap().collapsed);
    }

    /// 聚焦态下「折叠全部」不能把聚焦根折成一行；只显示 1 级时聚焦根仍是根。
    #[test]
    fn collapse_level_respects_the_focus_root() {
        let (mut doc, a, a1, _a1x, b) = nested_doc();
        let cmd = super::collapse_level_command(&doc, Some(a.as_str()), 1).unwrap();
        doc.apply(&cmd).unwrap();
        assert!(!doc.node(&a).unwrap().collapsed, "聚焦根展开");
        assert!(doc.node(&a1).unwrap().collapsed, "聚焦根下面第一层折起");
        assert!(
            !doc.node(&b).unwrap().collapsed,
            "聚焦之外的分支不该被这个命令动到"
        );

        let cmd = super::collapse_level_command(&doc, Some(a.as_str()), 2).unwrap();
        doc.apply(&cmd).unwrap();
        assert!(!doc.node(&a).unwrap().collapsed);
        assert!(!doc.node(&a1).unwrap().collapsed, "2 级时第二层展开");
    }

    #[test]
    fn split_text_lands_on_character_boundaries() {
        assert_eq!(split_text("中文abc", 2), ("中文".into(), "abc".into()));
        assert_eq!(split_text("中文", 0), (String::new(), "中文".into()));
        assert_eq!(split_text("中文", 9), ("中文".into(), String::new()));
        assert_eq!(split_text("", 0), (String::new(), String::new()));
    }

    #[test]
    fn type_to_edit_ignores_space_and_control_keys() {
        assert!(type_char("a"));
        assert!(type_char("中"));
        assert!(!type_char(" "));
        assert!(!type_char(""));
        assert!(!type_char("\n"));
        assert!(!type_char("\t"));
        assert!(!type_char("\u{7}"));
    }

    /// 搜索行的开关与关闭后的焦点回收；密度序号可持久化。
    #[test]
    fn search_toggle_and_density_round_trip() {
        let mut o = Outline::new();
        assert!(!o.search_open());
        o.open_search();
        assert!(o.search_open());
        o.open_search();
        assert!(o.search_open(), "重复打开只是重新聚焦，不来回切");
        o.toggle_search();
        assert!(!o.search_open());
        assert!(o.refocus_rows, "关闭后把焦点送回当前行");
        o.open_search();
        o.close_search();
        assert!(!o.search_open());
        assert!(o.filter.is_empty());

        use super::OutlineDensity;
        let d = OutlineDensity::Comfortable;
        o.set_density(d);
        assert_eq!(o.density(), d);
        assert_eq!(d.as_u8(), 2);
        assert_eq!(OutlineDensity::from_u8(0), OutlineDensity::Compact);
        assert_eq!(OutlineDensity::from_u8(2), OutlineDensity::Comfortable);
        assert_eq!(OutlineDensity::from_u8(9), OutlineDensity::Standard);
    }

    /// 页脚 / 文档信息用的静态统计：任务、备注、卡片、去重标签。
    #[test]
    fn stats_count_tasks_notes_cards_and_labels() {
        let (mut doc, a, _a1, a1x, b) = nested_doc();
        doc.apply(&Command::SetTask {
            id: a.clone(),
            task: Some(TaskState::Done),
        })
        .unwrap();
        doc.apply(&Command::SetNote {
            id: b.clone(),
            note: Some("有备注".into()),
        })
        .unwrap();
        for label in ["季度", "重点", "季度"] {
            doc.apply(&Command::AddLabel {
                ids: vec![a.clone()],
                label: label.into(),
            })
            .unwrap();
        }
        let card = NoteCard::new(doc.id.clone(), None, "卡片", "", 1);
        doc.apply(&Command::AttachNewCard {
            node: a1x.clone(),
            card,
        })
        .unwrap();

        let mut o = Outline::new();
        o.sync(&doc, &theme());
        let s = o.stats();
        assert_eq!(s.total, doc.node_count());
        assert_eq!(s.done, 1);
        assert_eq!(s.notes, 1);
        assert_eq!(s.cards, 1);
        assert_eq!(s.labels, 2, "同一个标签在多个节点上只算一次");
    }

    /// docs/28 §5.6：上 1/4 / 中 1/2 / 下 1/4 的分带边界。
    #[test]
    fn drop_zone_splits_the_row_at_quarter_lines() {
        assert_eq!(super::drop_zone(0.0), super::Zone::Before);
        assert_eq!(super::drop_zone(0.249), super::Zone::Before);
        assert_eq!(super::drop_zone(0.25), super::Zone::Child, "边界归中间带");
        assert_eq!(super::drop_zone(0.5), super::Zone::Child);
        assert_eq!(super::drop_zone(0.75), super::Zone::Child, "边界归中间带");
        assert_eq!(super::drop_zone(0.751), super::Zone::After);
        assert_eq!(super::drop_zone(1.0), super::Zone::After);
    }

    /// docs/28 §5.6：握把悬停满 120ms 才出现，选中即时。
    #[test]
    fn grip_appears_after_120ms_of_hover_but_instantly_when_selected() {
        use std::time::Duration;
        assert!(super::grip_ready(true, None), "选中行立即露出握把");
        assert!(!super::grip_ready(false, None), "没悬停不露出");
        assert!(
            !super::grip_ready(false, Some(Duration::from_millis(119))),
            "119ms 还不够"
        );
        assert!(
            super::grip_ready(false, Some(Duration::from_millis(120))),
            "120ms 到点"
        );
    }

    /// 大纲自动滚动：上缘往上、下缘往下、中间不动。
    #[test]
    fn outline_autoscroll_ramps_from_slow_to_fast_and_never_overshoots() {
        // docs/28 §5.6：上/下 24px 边带。
        let panel =
            egui::Rect::from_min_size(egui::Pos2::new(0.0, 100.0), egui::Vec2::new(232.0, 600.0));
        // 刚进边带 4px/帧，贴死边缘 18px/帧；方向与位置相反。
        let gentle = super::outline_autoscroll(123.0, panel);
        let hard = super::outline_autoscroll(102.0, panel);
        assert!(gentle < 0.0 && hard < 0.0);
        assert!(gentle.abs() < hard.abs(), "{gentle} vs {hard}");
        assert!(hard.abs() <= 18.0);
        assert_eq!(super::outline_autoscroll(400.0, panel), 0.0);
        assert!(super::outline_autoscroll(698.0, panel) > 0.0);
    }

    /// docs/28 §5.5 结构命令表 → `row_key` 全映射（每条键一条断言）。
    #[test]
    fn row_key_maps_the_docs28_5_5_table() {
        use egui::{Key, Modifiers};
        let m = |ctrl: bool, shift: bool, alt: bool| Modifiers {
            ctrl,
            shift,
            alt,
            ..Modifiers::default()
        };
        // 基础结构键
        assert_eq!(
            row_key(m(false, false, false), Key::Enter),
            Some(RowKey::AddSibling)
        );
        assert_eq!(
            row_key(m(false, false, false), Key::Tab),
            Some(RowKey::Indent)
        );
        assert_eq!(
            row_key(m(false, true, false), Key::Tab),
            Some(RowKey::Outdent)
        );
        assert_eq!(
            row_key(m(false, false, false), Key::F2),
            Some(RowKey::Rename)
        );
        assert_eq!(
            row_key(m(false, false, false), Key::Delete),
            Some(RowKey::Delete)
        );
        // docs/30 §5.6.1：Shift+Delete 只断「进来那条边」、Shift+F2 只改短语。
        // 这两条在树文档里没有语义（处理侧直接忽略），所以必须键位分开、别撞上
        // 普通的 Delete/F2——撞了就是「按 Shift 把行删了」。
        assert_eq!(
            row_key(m(false, true, false), Key::Delete),
            Some(RowKey::Unlink)
        );
        assert_eq!(
            row_key(m(false, true, false), Key::F2),
            Some(RowKey::RenameVia)
        );
        // Ctrl+Shift+Delete 不属于大纲：别让一个手抖的键组合删掉边。
        assert_eq!(row_key(m(true, true, false), Key::Delete), None);
        assert_eq!(
            row_key(m(false, false, false), Key::ArrowUp),
            Some(RowKey::Prev)
        );
        assert_eq!(
            row_key(m(false, false, false), Key::ArrowDown),
            Some(RowKey::Next)
        );
        assert_eq!(
            row_key(m(false, false, false), Key::Escape),
            Some(RowKey::Close)
        );
        // §5.5：Ctrl+Enter 切换任务（旧「加子级」作废）
        assert_eq!(
            row_key(m(true, false, false), Key::Enter),
            Some(RowKey::Task)
        );
        assert_eq!(row_key(m(true, false, false), Key::T), Some(RowKey::Task));
        // Ctrl+Shift+↑/↓ 移动
        assert_eq!(
            row_key(m(true, true, false), Key::ArrowUp),
            Some(RowKey::MoveUp)
        );
        assert_eq!(
            row_key(m(true, true, false), Key::ArrowDown),
            Some(RowKey::MoveDown)
        );
        // Alt+. 折叠/展开；Alt+Shift+. 全部；Alt+Enter 聚焦
        assert_eq!(
            row_key(m(false, false, true), Key::Period),
            Some(RowKey::Toggle)
        );
        assert_eq!(
            row_key(m(false, true, true), Key::Period),
            Some(RowKey::ExpandAllToggle)
        );
        assert_eq!(
            row_key(m(false, false, true), Key::Enter),
            Some(RowKey::Focus)
        );
        // Ctrl+] / Ctrl+[
        assert_eq!(
            row_key(m(true, false, false), Key::CloseBracket),
            Some(RowKey::Focus)
        );
        assert_eq!(
            row_key(m(true, false, false), Key::OpenBracket),
            Some(RowKey::UnfocusHere)
        );
        // Ctrl+A 全选
        assert_eq!(
            row_key(m(true, false, false), Key::A),
            Some(RowKey::SelectAllVisible)
        );
        // Shift+↑/↓ 扩展多选
        assert_eq!(
            row_key(m(false, true, false), Key::ArrowUp),
            Some(RowKey::SelectPrev)
        );
        assert_eq!(
            row_key(m(false, true, false), Key::ArrowDown),
            Some(RowKey::SelectNext)
        );
        // 未定义组合不认领
        assert_eq!(
            row_key(m(true, false, true), Key::Enter),
            None,
            "Ctrl+Alt 不认领"
        );
        assert_eq!(
            row_key(m(false, false, false), Key::A),
            None,
            "裸字母留给编辑"
        );
    }

    /// 粘贴按行拆：多行才拆；行尾空白与末尾空行忽略；CRLF 由 str::lines 承接。
    #[test]
    fn split_paste_only_splits_multiline() {
        assert_eq!(super::split_paste("单行"), None);
        assert_eq!(super::split_paste(""), None);
        assert_eq!(super::split_paste("a\n"), None, "末尾空行不算新节点");
        let (first, tail) = super::split_paste("a\nb\nc").expect("三行");
        assert_eq!(first, "a");
        assert_eq!(tail, vec!["b".to_owned(), "c".to_owned()]);
        let (first, tail) = super::split_paste("a\r\nb \n\n\n").expect("CRLF 与尾随空行");
        assert_eq!(first, "a");
        assert_eq!(tail, vec!["b".to_owned()]);
    }

    /// 软换行的插入原语：区间删选区 + 插入 \n；越界夹取。
    #[test]
    fn insert_at_chars_covers_caret_and_selection() {
        assert_eq!(super::insert_at_chars("abc", 1, 1, "\n"), "a\nbc");
        assert_eq!(super::insert_at_chars("abc", 1, 2, "\n"), "a\nc");
        assert_eq!(super::insert_at_chars("abc", 0, 3, "\n"), "\n");
        assert_eq!(super::insert_at_chars("abc", 9, 9, "\n"), "abc\n");
        assert_eq!(super::insert_at_chars("中文abc", 2, 2, "x"), "中文xabc");
        // 软换行后的草稿按 \n 计行（尾部换行也算一行）。
        let draft = super::insert_at_chars("第一行", 3, 3, "\n");
        assert_eq!(draft.split('\n').count(), 2);
        assert_eq!(super::commit_text(&draft), "第一行", "提交仍走单条 SetText");
    }

    /// 落点语义：前/后落在目标的兄弟位，中间挂成子节点；根没有同级。
    #[test]
    fn drop_zones_map_to_the_right_parent_and_index() {
        let row = super::Row {
            id: "b".into(),
            text: "乙".into(),
            depth: 1,
            has_children: false,
            kids: 0,
            collapsed: false,
            parent: Some("r".into()),
            index: 2,
            branch: mw_core::theme::Rgba::rgb(0, 0, 0),
            text_color: mw_core::theme::Rgba::rgb(0, 0, 0),
            bold: false,
            italic: false,
            font_override: None,
            last: false,
            guide_mask: 0,
            has_note: false,
            note_preview: String::new(),
            has_card: false,
            labels: 0,
            task: None,
            via: None,
            text_rows: 1,
            via_w: 0.0,
            via_id: None,
            via_from: None,
        };
        assert_eq!(
            super::move_target(&row, super::Zone::Before),
            ("r".to_string(), Some(2))
        );
        assert_eq!(
            super::move_target(&row, super::Zone::After),
            ("r".to_string(), Some(3))
        );
        assert_eq!(
            super::move_target(&row, super::Zone::Child),
            ("b".to_string(), None)
        );
        let root = super::Row {
            id: "r".into(),
            parent: None,
            index: 0,
            ..row
        };
        assert_eq!(
            super::move_target(&root, super::Zone::After),
            ("r".to_string(), None),
            "根没有同级，落在后面等于挂成子节点"
        );
    }

    /// 不能把节点拖进自己的子树。
    #[test]
    fn a_row_cannot_be_dropped_into_its_own_subtree() {
        use std::collections::HashMap;
        let mut parent: HashMap<String, Option<String>> = HashMap::new();
        parent.insert("r".into(), None);
        parent.insert("a".into(), Some("r".into()));
        parent.insert("a1".into(), Some("a".into()));
        assert!(super::is_self_or_descendant(&parent, "a", "a1"));
        assert!(super::is_self_or_descendant(&parent, "a", "a"));
        assert!(!super::is_self_or_descendant(&parent, "a1", "a"));
        assert!(!super::is_self_or_descendant(&parent, "a", "r"));
    }

    #[test]
    fn rename_trims_and_keeps_the_placeholder_semantics() {
        assert_eq!(super::commit_text("  季度计划  "), "季度计划");
        assert_eq!(super::commit_text(""), "");
        assert_eq!(super::commit_text("   "), "");
    }

    #[test]
    fn rows_follow_the_tree_and_skip_collapsed_subtrees() {
        let mut doc = doc_with(&["甲", "乙"]);
        let jia = doc
            .doc_order()
            .into_iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "甲"))
            .unwrap();
        doc.apply(&Command::AddNode {
            parent: jia.clone(),
            index: None,
            text: "甲一".into(),
            style: None,
        })
        .unwrap();

        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        assert_eq!(o.len(), 4, "根 + 甲 + 甲一 + 乙");

        doc.apply(&Command::SetCollapsed {
            id: jia,
            collapsed: true,
        })
        .unwrap();
        o.sync(&doc, &t);
        assert_eq!(o.len(), 3, "折叠后子树不再出行");
    }

    /// pin 与键盘焦点分离（docs/28 §4.6/§5.3）：命令路径只选中，不画环。
    #[test]
    fn pin_keeps_keyboard_focus_separate() {
        let mut o = Outline::new();
        assert!(!o.keyboard_focus);
        o.request_row_focus("a");
        assert_eq!(o.focus_request.as_deref(), Some("a"));
        assert!(!o.keyboard_focus, "程序化 pin 不得点亮键盘焦点环");
    }

    /// docs/28 §5.2 固定行几何：密度三档移除后，任何密度值都返回同一组规格。
    #[test]
    fn density_is_a_noop_and_geometry_is_fixed() {
        use super::OutlineDensity::{Comfortable, Compact, Standard};
        for d in [Compact, Standard, Comfortable] {
            assert_eq!(d.row_h(), 32.0, "行高固定 32");
            assert_eq!(d.indent(), 24.0, "缩进固定 24");
            assert_eq!(d.font(0), 16.0, "正文固定 16（depth 0 标题走 28）");
            assert_eq!(d.font(2), 16.0);
        }
        assert_eq!(Outline::new().density, Standard);
    }

    /// 造一行只用于几何断言的 Row：只关心深度/说明/换行行数，别的都给中性值。
    fn probe_row(depth: u16, text_rows: u16) -> super::Row {
        super::Row {
            id: format!("r{depth}"),
            text: "文本".into(),
            depth,
            has_children: false,
            collapsed: false,
            kids: 0,
            parent: None,
            index: 0,
            branch: mw_core::theme::Rgba::rgb(0, 0, 0),
            text_color: mw_core::theme::Rgba::rgb(0, 0, 0),
            bold: false,
            italic: false,
            font_override: None,
            last: true,
            guide_mask: 0,
            has_note: false,
            note_preview: String::new(),
            has_card: false,
            labels: 0,
            task: None,
            via: None,
            text_rows,
            via_w: 0.0,
            via_id: None,
            via_from: None,
        }
    }

    /// 装真字体的 egui Context：换行测量要走字体引擎，默认字体没有 CJK。
    fn ctx_with_fonts() -> egui::Context {
        let ctx = egui::Context::default();
        crate::fonts::install(&ctx);
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    /// docs/28 §5.2「换行每行 +26」：行高按显示行数长，说明行/根标题的底子另算。
    #[test]
    fn wrapped_text_adds_26_per_line() {
        let mut r = probe_row(1, 1);
        assert_eq!(super::row_layout_height(&r, 32.0, None), 32.0);
        r.text_rows = 3;
        assert_eq!(super::row_layout_height(&r, 32.0, None), 32.0 + 52.0);
        // 编辑中的行按草稿行数走：缓存不能压过正在输入的内容（§5.4）。
        assert_eq!(
            super::row_layout_height(&r, 32.0, Some(5)),
            32.0 + 4.0 * 26.0
        );
        // 说明行 +22 与换行 +26 叠加，不是二选一。
        r.has_note = true;
        assert_eq!(super::row_layout_height(&r, 32.0, None), 54.0 + 52.0);
        // 根标题 46 的底子随 16/26 标题走。
        let root = probe_row(0, 2);
        assert_eq!(super::row_layout_height(&root, 32.0, None), 46.0 + 26.0);
    }

    /// 量换行用的可用宽必须与绘制同一把尺子：左内距 + 行尾角标/折叠计数 + 更多动作道。
    #[test]
    fn text_avail_matches_the_row_inset() {
        let r = probe_row(2, 1);
        // 800 − 8(右边距) − 24(更多动作道) − (12 + 2×24 + 16)(左内距) = 692
        assert_eq!(super::text_avail(&r, 800.0, 24.0), 692.0);
        let mut note = probe_row(2, 1);
        note.has_note = true; // 行尾 5px 圆点 + 10 间距
        assert_eq!(super::text_avail(&note, 800.0, 24.0), 692.0 - 15.0);
        let mut fold = probe_row(2, 1);
        fold.collapsed = true;
        fold.has_children = true; // 折叠计数徽标 22
        assert_eq!(super::text_avail(&fold, 800.0, 24.0), 692.0 - 22.0);
        let mut task = probe_row(2, 1);
        task.task = Some(TaskState::Todo); // 行内任务框 18
        assert_eq!(super::text_avail(&task, 800.0, 24.0), 692.0 - 18.0);
    }

    /// `⋯` 的点击域与绘制同源：24×28、距列右 8、垂直对齐首行文字带。
    #[test]
    fn row_more_rect_sits_in_the_action_lane() {
        let rect =
            egui::Rect::from_min_max(egui::Pos2::new(200.0, 100.0), egui::Pos2::new(800.0, 132.0));
        let more = super::row_more_rect(rect, 116.0);
        assert_eq!(
            more,
            egui::Rect::from_min_max(egui::Pos2::new(768.0, 102.0), egui::Pos2::new(792.0, 130.0))
        );
        // 动作道与文字/角标互不重叠：文字右界在 768 左侧 15px（有圆点时）。
        assert!(more.left() > 800.0 - 8.0 - super::MORE_LANE - 15.0);
    }

    /// C20：概念图行菜单只有命题网动作，一条树语义都不许混进来；
    /// 没有进来边的行（根/孤立概念）禁用「改短语」「断边」，但其它项照常可用。
    #[test]
    fn concept_row_menu_is_proposition_only() {
        use super::RowAction;
        use crate::menu::Entry;
        fn actions(has_edge: bool) -> Vec<RowAction> {
            super::concept_row_items(has_edge, true, false)
                .into_iter()
                .filter_map(|e| match e {
                    Entry::Action { action, .. } => Some(action),
                    _ => None,
                })
                .collect()
        }
        /// 多选版：同样的过滤器，`has_edge` 无意义（单行项都不列）。
        fn actions_multi() -> Vec<RowAction> {
            super::concept_row_items(false, true, true)
                .into_iter()
                .filter_map(|e| match e {
                    Entry::Action { action, .. } => Some(action),
                    _ => None,
                })
                .collect()
        }
        let with_edge = actions(true);
        assert_eq!(
            with_edge,
            vec![
                RowAction::LinkConcept,
                RowAction::NewConcept,
                RowAction::EditVia,
                RowAction::Unlink,
                RowAction::EditText,
                RowAction::Copy,
                RowAction::Delete,
            ]
        );
        // 多选：只留「复制 / 删除概念」，单行动作不列。
        assert_eq!(actions_multi(), vec![RowAction::Copy, RowAction::Delete]);
        for tree in [
            RowAction::AddChild,
            RowAction::AddSibling,
            RowAction::ToggleCollapse,
            RowAction::Focus,
            RowAction::CollapseSelected,
            RowAction::MoveSameParent,
        ] {
            assert!(!with_edge.contains(&tree), "概念图菜单混进树语义：{tree:?}");
        }
        // 没有进来边：仍列出七项，但两个依赖边的项置灰。
        let items = super::concept_row_items(false, false, false);
        let disabled: Vec<&str> = items
            .iter()
            .filter_map(|e| match e {
                Entry::Action { label, enabled, .. } if !enabled => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(disabled, vec!["编辑连接短语", "断开进入的边", "删除概念"]);
    }

    /// 行菜单的作用域：目标在 n>1 的多选里才整组，否则只这一行。
    #[test]
    fn row_menu_scope_keeps_a_multi_selection_only_for_its_members() {
        let sel = vec!["a".to_string(), "b".to_string()];
        assert!(super::row_menu_multi(&sel, "a"));
        assert!(super::row_menu_multi(&sel, "b"));
        assert!(!super::row_menu_multi(&sel, "c"), "组外的行只作用自己");
        assert!(!super::row_menu_multi(&sel[..1], "a"), "单选不是多选");
        assert!(!super::row_menu_multi(&[], "a"), "空选择不成立");
    }

    /// 短行不进字体引擎（20k 全靠这条），超宽行按可用宽折行，列宽变了要重量。
    #[test]
    fn measure_wraps_counts_long_rows_and_respects_the_column_width() {
        let ctx = ctx_with_fonts();
        let long = "思维导图长文换行验收：这一段刻意写得足够长，好让它在正文栏里折成两行以上，\
                    用来钉住 §5.2 的「文本不截断、换行每行 +26、悬挂缩进」三条契约。";
        let doc = doc_with(&["短标题", long]);
        let mut o = Outline::new();
        o.sync(&doc, &theme());
        o.measure_wraps(&ctx, 800.0);
        let short = o.rows.iter().find(|r| r.text == "短标题").expect("短行");
        assert_eq!(short.text_rows, 1, "装得下的短行必须是 1 行");
        let wide = o.rows.iter().find(|r| r.text == long).expect("长行");
        let wide_rows = wide.text_rows;
        assert!(wide_rows >= 2, "超宽行要折行，实际 {wide_rows}");
        assert_eq!(
            super::row_layout_height(wide, 32.0, None),
            32.0 + 26.0 * (wide_rows as f32 - 1.0),
            "行高按 +26/行"
        );
        // 列变窄只会折得更多：缓存必须跟着列宽重算，而不是沿用上一帧。
        o.measure_wraps(&ctx, 360.0);
        let narrow = o.rows.iter().find(|r| r.text == long).expect("长行");
        assert!(narrow.text_rows > wide_rows, "窄列要重测换行");
        // 同一宽度重复调用不再重算（脏标记已清）。
        o.measure_wraps(&ctx, 360.0);
        assert!(!o.wrap_dirty);
    }

    /// 命中底带（§5.7）：换行行按行拆段，不再假设「命中永远在第一行」。
    #[test]
    fn hit_bands_split_across_wrapped_lines() {
        let ctx = ctx_with_fonts();
        let text = "abcdefghijklmnopqrstuvwxyz0123456789";
        let job = super::row_text_job(
            text.into(),
            // 走产品同一条字体派生：裸字号会被 chrome 的防回退测试拦下。
            super::row_font_id(super::OutlineDensity::Standard, 1, None),
            egui::Color32::WHITE,
            120.0,
            false,
            2,
        );
        let galley = ctx.fonts_mut(|f| f.layout_job(job));
        assert!(galley.rows.len() >= 2, "120px 装不下这串字");
        let bands = super::hit_bands(&galley, 0, text.chars().count());
        assert_eq!(bands.len(), galley.rows.len(), "整段命中要每行一段");
        assert!(bands[0].left() <= 1.0, "首段从左缘起");
        assert!(super::hit_bands(&galley, 4, 4).is_empty(), "空区间不画带");
    }

    /// 折叠/展开会补行，补进来的行也要重量换行（行表变了就置脏）。
    #[test]
    fn collapse_marks_the_wrap_cache_dirty() {
        let ctx = ctx_with_fonts();
        let (doc, a, ..) = nested_doc();
        let mut o = Outline::new();
        o.sync(&doc, &theme());
        o.measure_wraps(&ctx, 800.0);
        assert!(!o.wrap_dirty);
        assert!(o.apply_collapse(&doc, &theme(), &a, true));
        assert!(o.wrap_dirty, "折叠删行后要重量");
        o.measure_wraps(&ctx, 800.0);
        assert!(o.apply_collapse(&doc, &theme(), &a, false));
        assert!(o.wrap_dirty, "展开补行后也要重量");
    }

    /// 肘线的两个事实：最后的孩子在本行收笔；祖先还有后续兄弟时，竖线要穿过整棵子树。
    #[test]
    fn guide_mask_and_last_follow_the_branch() {
        let mut doc = doc_with(&["甲", "乙"]);
        let jia = doc
            .doc_order()
            .into_iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "甲"))
            .unwrap();
        doc.apply(&Command::AddNode {
            parent: jia.clone(),
            index: None,
            text: "甲一".into(),
            style: None,
        })
        .unwrap();
        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        let row = |text: &str| o.rows.iter().find(|r| r.text == text).unwrap();

        let root = &o.rows[0];
        assert_eq!(root.id, doc.root_id);
        assert!(root.last, "视图根没有同级");
        assert_eq!(root.guide_mask, 0);

        let a = row("甲");
        assert!(!a.last, "甲后面还有乙");
        assert_eq!(a.guide_mask, 0, "甲的祖先层都不需要继续线");

        let a1 = row("甲一");
        assert!(a1.last, "甲一是甲的最后一个孩子");
        assert_eq!(a1.guide_mask, 1, "甲还有兄弟，所以第 0 层竖线要穿过甲一");

        let b = row("乙");
        assert!(b.last);
        assert_eq!(b.guide_mask, 0);
    }

    /// 节点文字样式生效；极端字号在行里被夹到 12–28，不把行撑坏。
    #[test]
    fn node_text_style_is_applied_and_font_size_is_clamped() {
        let mut doc = doc_with(&["甲"]);
        let id = doc.doc_order()[1].clone();
        doc.apply(&Command::SetStyle {
            ids: vec![id.clone()],
            style: Some(mw_core::model::StyleOverride {
                bold: Some(true),
                italic: Some(true),
                font_size: Some(40),
                ..Default::default()
            }),
        })
        .unwrap();
        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        let row = o.rows.iter().find(|r| r.id == id).unwrap();
        assert!(row.bold && row.italic);
        assert_eq!(row.font_override, Some(40.0));
        let font = super::row_font_id(super::OutlineDensity::Standard, row.depth, Some(row));
        assert_eq!(font.size, 28.0, "40 → 28");
        assert!(matches!(font.family, egui::FontFamily::Name(ref n) if n.as_ref() == "zhisi-bold"));
    }

    /// 根节点在画布上是白字压实色卡；大纲没有卡片，必须退回正文色，否则标题直接消失。
    #[test]
    fn the_root_keeps_readable_text_when_the_canvas_style_is_white_on_color() {
        let doc = Document::new("根");
        let t = theme();
        assert_eq!(
            t.level(0).text,
            mw_core::theme::Rgba::rgb(0xff, 0xff, 0xff),
            "前提：雪花主题的根是白字"
        );
        let mut o = Outline::new();
        o.sync(&doc, &t);
        assert_eq!(o.rows[0].text_color, t.ui.text);
        assert!(super::contrast32(super::c32(t.ui.text), super::c32(t.ui.panel)) > 3.0);
    }

    #[test]
    fn the_cache_only_rebuilds_when_the_document_changes() {
        let mut doc = doc_with(&["甲"]);
        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        assert_eq!(o.len(), 2);

        // 文档没变：这次 sync 不能重建（重建会得到同样的行数，所以要换个角度验：
        // 直接改 doc 但不改 revision 是不可能的，因此这里验证"改了就会变"）。
        let id = doc.doc_order()[1].clone();
        doc.apply(&Command::SetText {
            id,
            text: "甲改".into(),
        })
        .unwrap();
        o.sync(&doc, &t);
        assert_eq!(o.len(), 2);
    }

    #[test]
    fn an_empty_root_still_gets_a_row() {
        let doc = Document::new("中心主题");
        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        assert_eq!(o.len(), 1);
    }

    /// 聚焦（S09）：只留目标子树；目标行按「根」对待（depth 0、没有同级），
    /// 面包屑从文档根一路走到目标，兄弟节点一行都不许出现。
    #[test]
    fn focusing_shows_only_the_subtree_with_a_root_to_leaf_crumb() {
        let mut doc = doc_with(&["甲", "乙"]);
        let ids = doc.doc_order();
        let jia = ids
            .iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "甲"))
            .cloned()
            .unwrap();
        doc.apply(&Command::AddNode {
            parent: jia.clone(),
            index: None,
            text: "甲一".into(),
            style: None,
        })
        .unwrap();

        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        assert_eq!(o.len(), 4);

        o.focus_on(&jia);
        o.sync(&doc, &t);
        assert_eq!(o.focus_id(), Some(jia.as_str()));
        assert_eq!(o.len(), 2, "聚焦后只剩甲 + 甲一");
        assert_eq!(o.rows[0].id, jia);
        assert_eq!(o.rows[0].depth, 0, "聚焦根在最上层");
        assert_eq!(o.rows[0].parent, None, "聚焦根在视图里没有同级");
        assert!(
            o.rows.iter().all(|r| r.text != "乙"),
            "兄弟节点不应进聚焦视图"
        );
        let crumbs = o.crumbs();
        assert_eq!(crumbs.len(), 2, "根 → 甲");
        assert_eq!(crumbs[0].0, doc.root_id);
        assert_eq!(crumbs[1], (jia.clone(), "甲".into()));

        let old = o.unfocus().unwrap();
        assert_eq!(old, jia);
        o.sync(&doc, &t);
        assert_eq!(o.len(), 4, "退出聚焦恢复全图");
        assert!(o.crumbs().is_empty());
    }

    /// 聚焦点被删掉、或聚焦到根：都退回全图，不留下「只剩一行」的假视图。
    #[test]
    fn missing_or_root_focus_falls_back_to_the_full_tree() {
        let mut doc = doc_with(&["甲", "乙"]);
        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);

        o.focus_on("does-not-exist");
        o.sync(&doc, &t);
        assert_eq!(o.focus_id(), None);
        assert_eq!(o.len(), 3);

        o.focus_on(&doc.root_id.clone());
        o.sync(&doc, &t);
        assert_eq!(o.focus_id(), None, "聚焦根等于全图");
        assert_eq!(o.len(), 3);

        let jia = doc.doc_order()[1].clone();
        o.focus_on(&jia);
        o.sync(&doc, &t);
        doc.apply(&Command::DeleteNodes { ids: vec![jia] }).unwrap();
        o.sync(&doc, &t);
        assert_eq!(o.focus_id(), None, "聚焦点没了要退回全图");
        assert_eq!(o.len(), 2);
    }

    /// 大纲的分支色必须与画布同源：一级按调色板循环，孙辈继承父色。
    #[test]
    fn rows_carry_the_same_branch_colors_as_the_canvas() {
        let mut doc = doc_with(&["甲", "乙"]);
        let order = doc.doc_order();
        let jia = order
            .iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "甲"))
            .cloned()
            .unwrap();
        doc.apply(&Command::AddNode {
            parent: jia,
            index: None,
            text: "甲一".into(),
            style: None,
        })
        .unwrap();

        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        let s = mw_core::theme::resolve_all(&doc, &t);
        for row in &o.rows {
            assert_eq!(
                Some(row.branch),
                s.get(&row.id).map(|st| st.branch_color),
                "行 {} 的分支色与级联结果不一致",
                row.text
            );
        }
        assert_eq!(o.rows[1].branch, t.palette[0]);
        assert_eq!(o.rows[2].branch, t.palette[0], "孙辈继承父分支色");
        assert_eq!(o.rows[3].branch, t.palette[1]);
    }

    /// 换主题不改文档，所以 revision 不变——分支色得靠主题 id 触发重建。
    #[test]
    fn switching_theme_repaints_the_branch_colors() {
        let doc = doc_with(&["甲"]);
        let mut o = Outline::new();
        o.sync(&doc, &get_theme("snow"));
        let snow = o.rows[1].branch;
        o.sync(&doc, &get_theme("ink"));
        assert_ne!(o.rows[1].branch, snow, "暗色主题的调色板应不同");
    }

    /// 行尾角标（S08）：备注 / 卡片 / 标签 / 任务从文档同步到行缓存。
    /// 纯备注与卡片分开记；只有空白的备注不算——否则敲个空格就冒出一个点。
    #[test]
    fn rows_carry_note_card_label_and_task_marks() {
        let mut doc = doc_with(&["甲", "乙"]);
        let order = doc.doc_order();
        let jia = order
            .iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "甲"))
            .cloned()
            .unwrap();
        let yi = order
            .iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == "乙"))
            .cloned()
            .unwrap();

        doc.apply(&Command::SetNote {
            id: jia.clone(),
            note: Some(
                "
  第一行 
第二行"
                    .into(),
            ),
        })
        .unwrap();
        doc.apply(&Command::SetTask {
            id: jia.clone(),
            task: Some(TaskState::Doing),
        })
        .unwrap();
        for label in ["季度", "重点"] {
            doc.apply(&Command::AddLabel {
                ids: vec![jia.clone()],
                label: label.into(),
            })
            .unwrap();
        }
        let card = NoteCard::new(doc.id.clone(), None, "乙的卡片", "", 1);
        doc.apply(&Command::AttachNewCard {
            node: yi.clone(),
            card,
        })
        .unwrap();
        doc.apply(&Command::SetTask {
            id: yi.clone(),
            task: Some(TaskState::Done),
        })
        .unwrap();

        let t = theme();
        let mut o = Outline::new();
        o.sync(&doc, &t);
        let a = o.rows.iter().find(|r| r.id == jia).unwrap();
        assert!(a.has_note);
        assert_eq!(
            a.note_preview, "第一行",
            "说明行取备注首个非空行并去掉首尾空白"
        );
        assert!(!a.has_card, "纯备注不是卡片");
        assert_eq!(a.labels, 2);
        assert_eq!(a.task, Some(TaskState::Doing));
        assert_eq!(
            super::marker_reserve(a),
            15.0,
            "备注/卡片单圆点 5 + 10 间隙"
        );

        let b = o.rows.iter().find(|r| r.id == yi).unwrap();
        assert!(!b.has_note, "挂卡片不等于写了备注");
        assert!(b.has_card);
        assert_eq!(b.labels, 0);
        assert_eq!(b.task, Some(TaskState::Done));
        assert_eq!(super::marker_reserve(b), 15.0, "卡片单圆点 5 + 10 间隙");

        doc.apply(&Command::SetNote {
            id: jia.clone(),
            note: Some("   ".into()),
        })
        .unwrap();
        o.sync(&doc, &t);
        let a = o.rows.iter().find(|r| r.id == jia).unwrap();
        assert!(!a.has_note, "空白备注不该留下角标");
    }

    /// 行尾任务方框（S12）：单击按 待办 → 进行中 → 完成 → 取消标记 循环。
    /// 第四态保证「加了标记」不会变成只能去检查器里才摘得掉。
    #[test]
    fn task_box_click_cycles_todo_doing_done_and_clear() {
        let mut doc = doc_with(&["甲"]);
        let id = doc.root_id.clone();
        let mut seen = Vec::new();
        for _ in 0..4 {
            let next = super::next_task(doc.node(&id).and_then(|n| n.task));
            doc.apply(&Command::SetTask {
                id: id.clone(),
                task: next,
            })
            .unwrap();
            seen.push(doc.node(&id).and_then(|n| n.task));
        }
        assert_eq!(
            seen,
            vec![
                Some(TaskState::Todo),
                Some(TaskState::Doing),
                Some(TaskState::Done),
                None,
            ]
        );
        // 每次都要推进修订号，大纲的行缓存才会跟着重建成新角标。
        let rev = doc.revision;
        doc.apply(&Command::SetTask {
            id: id.clone(),
            task: super::next_task(None),
        })
        .unwrap();
        assert!(doc.revision > rev);
    }

    /// 行内点击分派（S14/S15）：任务/备注/卡片各回各的动作；
    /// 备注点与卡片点热区重叠时按最近中心裁决，空白处不抢整行。
    #[test]
    fn marker_click_prefers_marker_hit_zones_and_nearest_center() {
        use super::{Action, Mark};
        use egui::{Pos2, Rect, Vec2};
        let task_hit = Rect::from_center_size(Pos2::new(100.0, 50.0), Vec2::splat(20.0));
        let note_hit = Rect::from_center_size(Pos2::new(130.0, 50.0), Vec2::splat(20.0));
        let card_hit = Rect::from_center_size(Pos2::new(140.0, 50.0), Vec2::splat(20.0));

        let on_task = super::marker_click(
            Some(Pos2::new(102.0, 52.0)),
            Some(TaskState::Doing),
            Some(task_hit),
            Some(note_hit),
            Some(card_hit),
            "n1",
        );
        assert!(matches!(
            on_task,
            Some(Action::SetTask { ref id, task: Some(TaskState::Done) }) if id == "n1"
        ));

        let on_note = super::marker_click(
            Some(Pos2::new(128.0, 50.0)),
            Some(TaskState::Doing),
            Some(task_hit),
            Some(note_hit),
            Some(card_hit),
            "n1",
        );
        assert!(matches!(
            on_note,
            Some(Action::OpenNote(ref id)) if id == "n1"
        ));

        // 139 同时落在两个热区里，但离卡片中心 1pt、离备注中心 9pt。
        let on_card = super::marker_click(
            Some(Pos2::new(139.0, 50.0)),
            Some(TaskState::Doing),
            Some(task_hit),
            Some(note_hit),
            Some(card_hit),
            "n1",
        );
        assert!(matches!(
            on_card,
            Some(Action::OpenCard(ref id)) if id == "n1"
        ));
        assert_eq!(
            super::marker_at(
                Some(Pos2::new(139.0, 50.0)),
                Some(task_hit),
                Some(note_hit),
                Some(card_hit),
            ),
            Some(Mark::Card)
        );

        assert!(
            super::marker_click(
                Some(Pos2::new(300.0, 50.0)),
                Some(TaskState::Doing),
                Some(task_hit),
                Some(note_hit),
                Some(card_hit),
                "n1"
            )
            .is_none(),
            "空白处不抢行点击"
        );
        assert!(
            super::marker_click(None, None, None, None, None, "n1").is_none(),
            "没有指针位置就不分派"
        );
    }

    /// 悬停文案（S15）：任务点随四态给下一步，备注/卡片说明单击后果；
    /// 没有任务时不会凭空空出一条任务提示。
    #[test]
    fn marker_tips_match_the_click_result() {
        use super::{marker_tip, Mark};
        assert_eq!(
            marker_tip(Mark::Task, Some(TaskState::Todo)),
            Some("任务：待办 · 单击切换")
        );
        assert_eq!(
            marker_tip(Mark::Task, Some(TaskState::Done)),
            Some("任务：完成 · 单击清除")
        );
        assert_eq!(marker_tip(Mark::Task, None), None);
        assert_eq!(
            marker_tip(Mark::Note, None),
            Some("备注 · 单击在检查器中编辑")
        );
        assert_eq!(marker_tip(Mark::Card, None), Some("卡片笔记 · 单击打开"));
    }

    /// 20k 行性能门（S10）：行缓存只在修订号变化时重建；筛选词不变时滚动帧不重扫全表。
    #[test]
    fn twenty_thousand_rows_rebuild_once_and_filter_once() {
        let t = theme();
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        // 4000 分支 × 4 叶 + 根 = 20001 行；新 id 从父节点的 children 末尾取，
        // 避免每步一次 O(n) 的 doc_order。
        for b in 0..4_000 {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: format!("分支 {b:04}"),
                style: None,
            })
            .unwrap();
            let branch = doc.node(&root).unwrap().children.last().unwrap().clone();
            for leaf in 0..4 {
                doc.apply(&Command::AddNode {
                    parent: branch.clone(),
                    index: None,
                    text: format!("叶 {b:04}-{leaf}"),
                    style: None,
                })
                .unwrap();
            }
        }

        let mut o = Outline::new();
        let start = std::time::Instant::now();
        o.sync(&doc, &t);
        let cold = start.elapsed();
        assert_eq!(o.len(), 20_001);
        assert_eq!(o.rebuilds, 1);

        // 修订号没变的 200 帧：一次 `u64` + 一次字符串比较，不许重建。
        let start = std::time::Instant::now();
        for _ in 0..200 {
            o.sync(&doc, &t);
        }
        let warm = start.elapsed();
        assert_eq!(o.rebuilds, 1, "文档没变，行缓存不许重建");

        // 首次筛选扫一遍全表；同一个词再走 200 帧只比较字符串。
        let start = std::time::Instant::now();
        o.set_filter("分支 19");
        o.prepare_filter(&doc);
        let scan = start.elapsed();
        assert_eq!(o.filter_scans, 1);
        assert_eq!(o.hits.len(), 100, "分支 1900–1999");
        assert_eq!(o.visible.len(), 101, "命中分支 + 根（祖先上下文）");

        let start = std::time::Instant::now();
        for _ in 0..200 {
            o.prepare_filter(&doc);
        }
        let reuse = start.elapsed();
        assert_eq!(o.filter_scans, 1, "筛选词没变，不许重扫");
        assert_eq!(o.visible.len(), 101);

        o.set_filter("叶 1999");
        o.prepare_filter(&doc);
        assert_eq!(o.filter_scans, 2, "换词才重扫");
        assert_eq!(o.hits.len(), 4, "叶 1999-0..3");
        assert_eq!(o.visible.len(), 6, "4 叶 + 分支 + 根");

        eprintln!(
            "outline 20k: cold={cold:?} warm200={warm:?} filter_scan={scan:?} filter_reuse200={reuse:?}"
        );
        // 阈值给足余量（debug 构建、CI 机器慢），只拦「每帧 O(n)」这类回退。
        assert!(
            cold < std::time::Duration::from_secs(10),
            "冷重建太慢: {cold:?}"
        );
        assert!(
            warm < std::time::Duration::from_secs(1),
            "200 次空 sync 太慢: {warm:?}"
        );
        assert!(
            scan < std::time::Duration::from_secs(2),
            "全表筛选太慢: {scan:?}"
        );
        assert!(
            reuse < std::time::Duration::from_secs(1),
            "筛选缓存没命中: {reuse:?}"
        );
    }

    /// 编辑 overlay 必须锁 1 行：`multiline` 的默认 `desired_rows` 是 4，单行草稿会被
    /// 最小高度 + 垂直居中推到下一行，2px 下划线就跑到文字上方（用户实拍的「文字上方
    /// 有条线」）。行高由 `32 + 26*(行数-1)` 自己算，不靠 widget 撑。
    #[test]
    fn edit_overlay_pins_desired_rows_to_one() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/panels/outline.rs"
        ))
        .expect("读 outline.rs 源码");
        let at = src
            .find("TextEdit::multiline(&mut ed.draft)")
            .expect("编辑 overlay 还在吗");
        let builder = &src[at..src.len().min(at + 600)];
        assert!(
            builder.contains(".desired_rows(1)"),
            "编辑 overlay 没锁 desired_rows(1)：单行草稿会被默认 4 行撑高，下划线跑到文字上方"
        );
    }

    /// CM-09：三张「大纲改结构」场景用的是**模板本体**（`start::TEMPLATES` 的概念图），
    /// 所以模板自身也得能被展开成树：主概念写进 `root_id`、五个概念各有一条进来的边、
    /// 一级行带短语与边 id。import 进来或用户新建的概念图缺 `root_id` 时，下面这三条
    /// 断言就是「`Shift+Delete` / `Shift+F2` 按了没反应」的预警。
    #[test]
    fn concept_template_expands_into_a_tree() {
        let tpl = crate::start::TEMPLATES
            .iter()
            .find(|t| t.layout == "conceptMap")
            .expect("概念图模板还在吗");
        let session = tpl.session();
        let doc = &session.doc;
        assert!(
            doc.node(&doc.root_id).is_some(),
            "模板没写 root_id：生成树会退化成「所有概念各自成根」，大纲里没有一条边"
        );
        let mut o = Outline::new();
        o.sync(doc, &theme());
        let row = |t: &'static str| o.rows.iter().find(|r| r.text == t).expect("行还在吗");
        let pain = row("用户痛点");
        assert_eq!(pain.depth, 1, "用户痛点挂在主概念下面");
        assert_eq!(pain.via.as_deref(), Some("解决"), "短语要写进行里");
        assert!(
            pain.via_id.is_some(),
            "行要能指回那条边（Shift+Delete 靠它断）"
        );
        let growth = row("增长策略");
        assert_eq!(growth.depth, 2, "增长策略是二级概念");
        assert_eq!(growth.via.as_deref(), Some("转化"));
        assert!(growth.via_id.is_some());
    }

    /// CM-07：概念图的行表按 `relationships` 生成树展开。
    /// 反例：拿掉 `rebuild` 里的概念图分叉，主概念以外全是 depth 0 的自由主题，
    /// 这条断言的深度序列立刻塌成一列 0。
    #[test]
    fn concept_rows_follow_the_relationship_tree() {
        use mw_core::LayoutSettings;
        let mut doc = Document::new("概念图");
        doc.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = doc.root_id.clone();
        doc.apply(&Command::SetText {
            id: root.clone(),
            text: "用户痛点".into(),
        })
        .unwrap();
        let mut ids = vec![root];
        for (i, t) in ["竞品访谈", "留存低"].iter().enumerate() {
            doc.apply(&Command::AddFloatingTopic {
                text: (*t).into(),
                x: 200.0 * (i as f32 + 1.0),
                y: 0.0,
            })
            .unwrap();
            let id = doc
                .nodes
                .values()
                .find(|n| &n.text == t)
                .map(|n| n.id.clone())
                .unwrap();
            ids.push(id);
        }
        for (from, to, label) in [(0, 1, "因为"), (1, 2, "导致")] {
            doc.apply(&Command::AddRelationship {
                from: ids[from].clone(),
                to: ids[to].clone(),
                label: Some(label.into()),
            })
            .unwrap();
        }

        let mut o = Outline::new();
        o.sync(&doc, &theme());
        // `sync` 只建行表；`visible` 要等 `show` 里的筛选扫描，所以这里直接看 `rows`。
        let shape: Vec<(String, u16)> = o.rows.iter().map(|r| (r.text.clone(), r.depth)).collect();
        assert_eq!(
            shape,
            vec![
                ("用户痛点".to_string(), 0),
                ("竞品访谈".to_string(), 1),
                ("留存低".to_string(), 2),
            ]
        );
        assert!(
            o.rows[0].has_children && o.rows[0].kids == 1,
            "生成树的子数要进行表"
        );
        assert!(!o.rows[2].has_children, "叶子概念没有 chevron");
    }

    /// CER-11 性能探针：20k 行连续滚动的单帧 UI 耗时（release 跑）：
    /// `cargo test --release -p mw-shell --bin mw-shell outline_20k_frame -- --ignored --nocapture`
    ///
    /// 不截图、不进阶段门；只给出「布局 + 绘制命令生成」的 CPU 帧时间。GPU 上屏时间
    /// 由 `MW_STATS` 的 wall 帧率负责（`scripts/x100-outline-20k.ps1`）。
    #[test]
    #[ignore = "性能探针：release + --ignored 手动跑"]
    fn outline_20k_frame_budget_probe() {
        use egui::{Event, MouseWheelSource, MouseWheelUnit, TouchPhase};

        let t = theme();
        let chrome = crate::chrome::chrome_of(&t);
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        for b in 0..4_000 {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: format!("分支 {b:04}"),
                style: None,
            })
            .unwrap();
            let branch = doc.node(&root).unwrap().children.last().unwrap().clone();
            for leaf in 0..4 {
                doc.apply(&Command::AddNode {
                    parent: branch.clone(),
                    index: None,
                    text: format!("叶 {b:04}-{leaf}"),
                    style: None,
                })
                .unwrap();
            }
        }
        let mut outline = Outline::new();
        let first_start = std::time::Instant::now();
        outline.sync(&doc, &t);
        outline.prepare_filter(&doc);
        let first_screen = first_start.elapsed().as_secs_f32() * 1000.0;
        assert!(
            first_screen < 100.0,
            "20k 首屏（行表重建 + 可见表）预算 100ms，实测 {first_screen:.1}ms"
        );

        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.0);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1080.0, 720.0));
        let mut focus = crate::panels::focus::FocusMemory::new();
        let mut times = Vec::new();
        for frame in 0..80 {
            let mut input = egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            };
            input
                .events
                .push(Event::PointerMoved(egui::pos2(400.0, 300.0)));
            if frame >= 10 {
                input.events.push(Event::MouseWheel {
                    unit: MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -96.0),
                    phase: TouchPhase::Move,
                    source: MouseWheelSource::Wheel,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let start = std::time::Instant::now();
            let _ = ctx.run_ui(input, |ui| {
                let _ = outline.show(ui, &t, chrome, &[], None, None, false, &mut focus);
            });
            if frame >= 10 {
                times.push(start.elapsed().as_secs_f32() * 1000.0);
            }
        }
        times.sort_by(|a, b| a.total_cmp(b));
        let p50 = times[times.len() / 2];
        let p95 = times[(times.len() * 95 / 100).min(times.len() - 1)];
        let max = *times.last().unwrap();
        // 单节点折叠/展开：行表增量，不跑全量 resolve_all。
        let branch = doc.node(&doc.root_id).unwrap().children[0].clone();
        doc.apply(&Command::SetCollapsed {
            id: branch.clone(),
            collapsed: true,
        })
        .unwrap();
        let t0 = std::time::Instant::now();
        assert!(outline.apply_collapse(&doc, &t, &branch, true));
        let collapse_ms = t0.elapsed().as_secs_f32() * 1000.0;
        doc.apply(&Command::SetCollapsed {
            id: branch.clone(),
            collapsed: false,
        })
        .unwrap();
        let t0 = std::time::Instant::now();
        assert!(outline.apply_collapse(&doc, &t, &branch, false));
        let expand_ms = t0.elapsed().as_secs_f32() * 1000.0;
        eprintln!(
            "outline 20k: first_screen={first_screen:.1}ms scroll p50={p50:.2}ms p95={p95:.2}ms max={max:.2}ms collapse={collapse_ms:.2}ms expand={expand_ms:.2}ms laid_rows={}",
            outline.laid_rows
        );
        assert!(
            outline.laid_rows <= 80,
            "虚拟化：只排视口 ±10 行，实测 {} 行",
            outline.laid_rows
        );
        assert!(p95 < 16.0, "单帧 UI 预算 16ms，实测 p95={p95:.2}ms");
        assert!(collapse_ms < 16.0, "折叠预算 16ms，实测 {collapse_ms:.2}ms");
        assert!(expand_ms < 16.0, "展开预算 16ms，实测 {expand_ms:.2}ms");
    }
}
