//! 领域模型：节点 / 样式覆盖 / 布局设置 / 文档元数据。

use serde::{Deserialize, Serialize};

/// 当前毫秒时间戳（Unix epoch）。
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 生成全局唯一、时间有序的节点 ID（UUID v7）。
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// 节点样式覆盖；全部字段可选，`None` 表示继承（级联规则见文档 05 §3）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct StyleOverride {
    pub fill: Option<String>,
    pub text_color: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size: Option<u16>,
    /// rounded / rect / capsule / underline / ellipse
    pub shape: Option<String>,
    /// 圆角半径（0–24 css px）；只对圆角矩形/矩形生效，缺键按 `Shape` 默认（SE-13）。
    ///
    /// 三个 SE-13 新键都带 `skip_serializing_if`：没设就不写键，与 `Relationship` 的
    /// 线型/线宽/注释同一套先例——**旧文档与既有字节级夹具一个字节都不变**。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    /// 文本水平对齐：left / center / right；缺键左对齐（概念图仍走居中默认）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 字重：400 / 500 / 600；缺键回落 `bold ? 600 : 400`（SE-13，`bold` 是 600 的别名）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    pub branch_color: Option<String>,
    pub apply_text_to_subtree: Option<bool>,
}

/// 关系线端点箭头（CmapTools 的有向关系：A → B 与 B → A 不是一回事）。
///
/// 默认 `Forward` = 今天的行为（终点端一个 6×4 实心箭头），而且**默认值不落盘**：
/// 旧文件缺这个键读出来就是 `Forward`，新文件只要没改过也不会写出这个键——
/// 于是「加了字段」不会让任何既有 `.zmind` 或任何 `cm-*` 基线变样（`docs/32` §3）。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArrowHead {
    /// 两端都不画（纯连接线，表示「相关」而不表示方向）。
    Off,
    /// 只在终点画（默认，等价于今天）。
    #[default]
    Forward,
    /// 只在起点画（反向：读作 B → A）。
    Backward,
    /// 两端都画（双向）。
    Both,
}

impl ArrowHead {
    pub fn as_str(self) -> &'static str {
        match self {
            ArrowHead::Off => "off",
            ArrowHead::Forward => "forward",
            ArrowHead::Backward => "backward",
            ArrowHead::Both => "both",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ArrowHead::Off => "无",
            ArrowHead::Forward => "单向",
            ArrowHead::Backward => "反向",
            ArrowHead::Both => "双向",
        }
    }

    /// 默认值：不写进文件（写出去只会让 diff 变脏）。
    pub fn is_forward(&self) -> bool {
        matches!(self, ArrowHead::Forward)
    }
}
/// 概念分组（`docs/32` §13，CM-18）：把若干概念框在一起，可命名、可整体拖动、可解组。
///
/// 组**不是父子关系**：成员仍是独立的浮点概念，解组只删框、不删概念。
/// 组框矩形不进文件——它由「成员包围盒 + 内距」在 `Scene` 里算出来（派生量，
/// 写进文件会和成员位置打架）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ConceptGroup {
    pub id: String,
    pub label: String,
    /// 成员概念 id。重建时按「成员还存在」逐条过滤，空了整组丢弃。
    pub members: Vec<String>,
    /// 组框颜色（`#rrggbb`）；`None` 跟随主题的 dim 色。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// 关系线（XMind Relationship）：由所属节点指向另一节点。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Relationship {
    pub id: String,
    pub to: String,
    pub label: Option<String>,
    /// 自定义线色（#rrggbb）；`None` 跟随主题。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 端点箭头。默认 `Forward`（= 今天的样子）；`Off` 是「两端都不画」，
    /// 不是「没设置」——没设置就是 `Forward`，所以这里不需要 Option 包一层。
    #[serde(default, skip_serializing_if = "ArrowHead::is_forward")]
    pub head: ArrowHead,
    /// 虚线（`docs/30` §5.4.3，CM-16）：`true` = 虚线，用来表达「暂定 / 推测」这类弱命题。
    /// 默认 `false` 且**不落盘**——旧文件读出来就是实线，新文件没改过也不会多一个键。
    #[serde(default, skip_serializing_if = "is_false")]
    pub dashed: bool,
    /// 线宽（css px，0.5–6.0）。`None` = 跟随主题（1.5，屏幕空间恒定）。
    /// 与 `color` / `head` 同款：缺键读成 `None`，`None` 不写回文件。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    /// 命题的补充说明（**不是**链接短语）。`None` = 没有。
    ///
    /// 短语是命题的谓词（「导致」），必须短、要进检索标题；注释是给这条命题的解释
    /// （「2026Q3 的数据」），可以长，不参与「命题」的读法。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `skip_serializing_if` 用：`false` 不写进文件。
fn is_false(v: &bool) -> bool {
    !*v
}

impl Relationship {
    /// 这条线该用多宽（css px）。`None` 回落到主题的 1.5。
    pub fn width_css(&self) -> f32 {
        self.width.unwrap_or(1.5)
    }
}

/// 边界（XMind Boundary）：圈住所属节点的若干子节点（及其子树）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Boundary {
    pub id: String,
    pub targets: Vec<String>,
    pub color: Option<String>,
}

/// 边界 / 概要这两种「注解」的类别。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationKind {
    Summary,
    Boundary,
}

/// 一条盖住某节点的注解引用（id + 所属父节点 + 显示文字）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnotationRef {
    pub parent: String,
    pub kind: AnnotationKind,
    pub id: String,
    pub label: String,
    /// true = 这条注解挂在当前节点自己身上（它盖的是子节点），false = 它盖住当前节点。
    pub owned: bool,
    /// 边界自带的颜色（概要恒为 None）。
    pub color: Option<String>,
}

/// 概要（XMind Summary）：对所属节点的若干子节点做摘要，带摘要文字。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Summary {
    pub id: String,
    pub targets: Vec<String>,
    pub text: String,
}

/// 世界坐标位置（自由主题专用，XMind 的「自由定位」）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// 思维导图节点。`children` 有序；与各子节点的 `parent_id` 双向一致（由校验器保证）。
///
/// 森林语义：`root_id` 为主根；`parent_id == None` 的其余节点为「自由主题」
/// （XMind 浮动主题），可携带 `pos` 自由摆放，整体构成一棵独立的小树。
/// 节点任务状态（内容面板的小状态机，不参与导出语义）。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    Todo,
    Doing,
    Done,
}

impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskState::Todo => "todo",
            TaskState::Doing => "doing",
            TaskState::Done => "done",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TaskState::Todo => "未开始",
            TaskState::Doing => "进行中",
            TaskState::Done => "完成",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Node {
    pub id: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub text: String,
    pub note: Option<String>,
    pub link: Option<String>,
    pub icon: Option<String>,
    /// 标签（XMind Label）：节点下方的小胶囊，最多 5 个
    pub labels: Vec<String>,
    /// 关系线（XMind Relationship，起点为本节点）
    pub relationships: Vec<Relationship>,
    /// 边界（XMind Boundary，圈住 `targets` 指定的直接子节点）
    pub boundaries: Vec<Boundary>,
    /// 概要（XMind Summary，覆盖 `targets` 指定的直接子节点）
    pub summaries: Vec<Summary>,
    pub collapsed: bool,
    pub style: Option<StyleOverride>,
    /// 自由主题的世界坐标（仅当自身为浮动分支根时有效）
    pub pos: Option<Position>,
    /// 概念图「钉住」标记：`Some(true)` = 手工摆位受保护，`None` = 未钉。
    ///
    /// 只用 `Option<bool>` 的两态（不设 `Some(false)`），和 `focus_question` /
    /// `theme_overrides` 同一套先例：`skip_serializing_if` 而不是升 `FORMAT_VERSION`——
    /// 没钉过的文档序列化出来和旧版**逐字节相同**，旧版读新文档也只是忽略这个键。
    /// 对思维导图节点无意义（钉住只对概念图的「整理布局/力导向」生效），但字段
    /// 共用 `Node`，不加文档类型判别。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    /// 链接的卡片笔记 id（0/1 张；卡片本身在 `Document.cards`）。
    pub card_id: Option<String>,
    /// 任务状态（内容面板）。
    pub task: Option<TaskState>,
    /// 图片/附件占位（P41；真正加载在 P53）。
    pub image: Option<String>,
    pub modified_at: i64,
}

impl Node {
    pub fn new(id: String, parent: Option<String>, text: impl Into<String>) -> Self {
        Self {
            id,
            parent_id: parent,
            text: text.into(),
            modified_at: now_ms(),
            ..Default::default()
        }
    }
}

/// 布局设置（随文档保存，见文档 05 §2.2）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutSettings {
    /// mindmap / logicRight / orgChart / fishbone / timeline
    pub kind: String,
    /// 思维导图展开方向：auto / right / left（其它布局忽略）。
    pub direction: String,
    /// 分支线形态：curve / elbow / straight。
    pub edge_shape: String,
    /// 分支线末端是否画箭头。
    pub edge_arrow: bool,
    pub sibling_gap: f32,
    pub level_gap: f32,
    pub max_node_width: f32,
    /// 概念图的聚焦问题（对标 CmapTools 的 Focus Question）：这张图要回答的那句话。
    ///
    /// 只给概念图用，树布局忽略。`None` = 没写；空串在命令层就归一成 `None`。
    /// 用 `skip_serializing_if` 而不是升 `FORMAT_VERSION`：没写的文档序列化出来
    /// 和旧版**逐字节相同**，旧版读新文档也只是忽略这个键（与 `theme_overrides` 同款）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_question: Option<String>,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            kind: "mindmap".to_string(),
            direction: "auto".to_string(),
            edge_shape: "curve".to_string(),
            edge_arrow: false,
            sibling_gap: 24.0,
            level_gap: 80.0,
            max_node_width: 300.0,
            focus_question: None,
        }
    }
}

/// 文档级元数据。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DocMeta {
    pub title: String,
    pub theme: String,
    /// 自定义强调色（`#rrggbb`）；`None` = 用主题自带的强调色。覆盖只取色相，明度与饱和度
    /// 由各主题自己保住（见 `theme::Theme::with_accent`）。
    pub theme_accent: Option<String>,
    /// 文档级主题微调：只覆盖少数几个颜色槽位，其余仍由主题决定。
    /// 与 `theme_accent` 一样住在 meta 里——随文档保存，撤销/重做天然跟着走。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_overrides: Option<ThemeOverrides>,
    pub layout: LayoutSettings,
}

/// 主题微调：每个字段都是 `#rrggbb`；`None` = 用主题自己的值。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeOverrides {
    pub canvas_bg: Option<String>,
    pub central_fill: Option<String>,
    pub level1_fill: Option<String>,
    /// 所有层级的文字色（老字段；分层的三项优先级更高）。
    pub text: Option<String>,
    /// 中心主题文字色。
    pub text_central: Option<String>,
    /// 一级（分支）主题文字色。
    pub text_main: Option<String>,
    /// 二级及更深主题文字色。
    pub text_sub: Option<String>,
    /// 连线粗细倍率（1.0 = 主题原值，钳在 0.4–2.5）。
    pub edge_scale: Option<f32>,
    /// 分支线统一颜色（`None` = 跟随各分支自己的调色板色）。
    pub edge_color: Option<String>,
    /// 分支线是否收细；`None` = 跟随主题。
    pub edge_taper: Option<bool>,
}

impl ThemeOverrides {
    /// 三个槽位都空等于没有微调，存盘时可以整个省掉。
    pub fn is_empty(&self) -> bool {
        self.canvas_bg.is_none()
            && self.central_fill.is_none()
            && self.level1_fill.is_none()
            && self.text.is_none()
            && self.text_central.is_none()
            && self.text_main.is_none()
            && self.text_sub.is_none()
            && self.edge_scale.is_none()
            && self.edge_color.is_none()
            && self.edge_taper.is_none()
    }
}

impl Default for DocMeta {
    fn default() -> Self {
        Self {
            title: "未命名".to_string(),
            theme: "snow".to_string(),
            theme_accent: None,
            theme_overrides: None,
            layout: LayoutSettings::default(),
        }
    }
}
