//! 画布面板（M1b-3 / M2a）：`mw-core::Scene` → wgpu 四层合成 → egui 原生纹理。
//!
//! 这一层只做「穿针引线」：文档/场景、相机、命中索引、纹理注册。平移缩放与选择
//! 是交互语义，不写进 render 层——render 只接收 `Camera` 与 `selected/hovered/editing`。
//!
//! 每帧顺序固定：输入 → 相机 → `build_layers` → `TextEngine::paint` → 图集增量上传
//! → `render_texture` → 首次/尺寸变化时注册 egui 纹理 → `Painter::image`。
//! 纹理与 egui 共用同一个 wgpu device，渲染命令在 update 阶段提交，egui 本帧采样，
//! 队列顺序保证看到的是这一帧的内容。
//!
//! M2a 的行内编辑走 **egui TextEdit 覆盖层**而不是自绘 caret：
//! · IME 组合、候选窗定位（egui-winit 用 caret rect 调 `set_ime_cursor_area`）、
//!   输入法自带的光标跟随都由 egui 处理，不用碰 winit 事件循环；
//! · 编辑中的文本每帧喂给 `build_scene` 的 editing 槽，节点盒随输入实时重排；
//! · 合成器跳过该节点的文字与强调环（`Frame::editing`），避免与覆盖层重影；
//! · 撤销以「一次提交一条 `SetText`」为粒度，编辑期间的 Ctrl+Z 由 TextEdit 自己处理。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use eframe::egui::{
    self, Color32, CursorIcon, Event, FontFamily, FontId, Galley, Key, Modifiers, Pos2, Sense,
    Stroke, TextBuffer, Vec2,
};
use eframe::egui_wgpu;
use mw_core::design;
use mw_core::document::MAX_TEXT_LEN;
use mw_core::layout::{line_baseline, LayoutKind};
use mw_core::model::AnnotationKind;
use mw_core::text::{MonoMeasurer, TextMeasurer};
use mw_core::theme::{theme_for, Rgba, Shape, Theme};
use mw_core::{
    build_scene, geom::PathSeg, new_id, now_ms, Command, Node, NoteCard, Point, Rect, Scene,
    SceneCache, StyleOverride,
};
use mw_render::{
    build_layers, owner_node, Camera, Compositor, DWrite, DWriteMeasurer, FrameBudget, HitIndex,
    LayerGeom, Target, TextEngine, LAYERS,
};
use mw_ui::arrange::{self, Align, Distribute};
use mw_ui::drill_click::{ToggleClick, ToggleClickMemory};
use mw_ui::{marquee, Selection};

use crate::demo::Demo;
use crate::session::Session;

mod zorder;

mod consts;
pub(crate) use consts::*;

mod hover;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DragKind {
    Pan,
    Marquee,
    /// Alt+拖空白：自由套索。
    Lasso,
    /// 按住节点拖：幽灵跟手、三分区落点、松手改结构（M2b-3）。
    Move,
    /// 按住四边锚点拖：拉出一条关系线，松手连到目标节点（M2b-4）。
    Link,
    /// 按住注解跨度手柄拖：上下移动概要/边界覆盖的兄弟区间（WP9）。
    Span,
    /// 按住关系线终点拖：换目标节点（P03）。
    Relink,
}

/// 一次按下到松手之间的手势状态。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Drag {
    kind: DragKind,
    /// 按下点（屏幕坐标，含面板原点）。
    origin: Pos2,
    /// 当前指针位置。松手那一帧 `interact_pointer_pos` 仍可用，但 `latest_pos` 更稳。
    current: Pos2,
    /// 平移已经结算到的位置——用它做增量，画布从第一个像素起就 1:1 跟手。
    applied: Pos2,
    /// 框选是否已越过阈值、真正在画橡皮筋。
    drawing: bool,
    /// 按下时按着 Ctrl：这次拖拽是**复制**而不是移动（XMind 的 Ctrl+拖拽）。
    copy: bool,
    /// 框选按下时的 Shift/Ctrl 加选。
    additive: bool,
    /// 框选按下时的 Alt 减选。
    subtract: bool,
}

/// 拖出连线的进行态。
pub(crate) struct Link {
    from: u32,
    side: mw_ui::Side,
    /// 指针位置（世界坐标）。
    at: Point,
    /// 松手会连到谁。
    to: Option<u32>,
}

/// 悬停工具条：节点右侧（空间不够时镜像到左侧）的两枚 24×24 圆角按钮。
///
/// 全部是**屏幕空间**矩形，画在 egui 层而不是合成器里：按钮大小不该随缩放变，
/// 而且它只在指针下出现，属于「交互反馈」而不是文档内容（离屏导出不该带上它）。
#[derive(Clone, Copy, Debug)]
pub(crate) struct HoverBar {
    /// 工具条属于哪个场景节点。
    node: u32,
    /// 节点盒的屏幕矩形。用来把「节点 → 按钮」之间的缝隙也算进悬停区，
    /// 否则指针跨过那 8px 时节点先失去悬停，按钮会在指尖前消失。
    node_rect: egui::Rect,
    /// 「＋」：加子主题。
    plus: egui::Rect,
    /// 「⋯」：点开节点菜单。
    more: egui::Rect,
}

impl HoverBar {
    fn buttons(self) -> egui::Rect {
        self.plus.union(self.more)
    }

    /// 悬停保持区：节点盒 + 缝隙 + 两枚按钮。只有「保持悬停」用它，命中仍按按钮算。
    fn hover_zone(self) -> egui::Rect {
        self.node_rect.union(self.buttons())
    }
}

/// 最小平移的纯计算：目标盒已在视口内时相机不动；否则只挪到刚好露出。
///
/// 四周留 48 CSS px 安全边；目标本身比视口还大时直接居中——此时「露出全部」
/// 做不到，让它的中心对准屏幕中心是唯一稳定的读法。
pub(crate) fn reveal_center(center: Point, zoom: f32, viewport: [f32; 2], r: Rect) -> Point {
    let pad = 48.0 / zoom;
    let half_w = viewport[0] / (2.0 * zoom);
    let half_h = viewport[1] / (2.0 * zoom);
    let mut c = center;
    if r.w + pad * 2.0 > half_w * 2.0 {
        c.x = r.x + r.w * 0.5;
    } else {
        if r.x < c.x - half_w + pad {
            c.x = r.x + half_w - pad;
        }
        if r.right() > c.x + half_w - pad {
            c.x = r.right() - half_w + pad;
        }
    }
    if r.h + pad * 2.0 > half_h * 2.0 {
        c.y = r.y + r.h * 0.5;
    } else {
        if r.y < c.y - half_h + pad {
            c.y = r.y + half_h - pad;
        }
        if r.bottom() > c.y + half_h - pad {
            c.y = r.bottom() - half_h + pad;
        }
    }
    c
}

/// 工具条几何：纯函数，方便单测直接钉住「不压节点、不压折叠点、不出画布」。
///
/// 横向默认贴节点右侧 `HOVER_GAP` 处；右边放不下就镜像到左侧；两侧都放不下
/// （节点比画布还宽）就不画——宁可没有，也不要把命中区叠到节点盒上。
///
/// 纵向**不居中**：折叠圆环就在节点右侧中线，居中的按钮会把它整个盖住，
/// 用户连「折叠」都点不到。改为贴节点右上角之上 `HOVER_GAP_V`，上面放不下
/// 才落到节点下方；两边都放不下就不画。
fn hover_bar_rects(
    node: egui::Rect,
    canvas: egui::Rect,
    has_card: bool,
) -> Option<(egui::Rect, egui::Rect)> {
    if node.width() < 1.0 || node.height() < 1.0 {
        return None;
    }
    let total = HOVER_BTN * 2.0 + HOVER_BTN_GAP;
    let right_x = node.right() + HOVER_GAP;
    let left_x = node.left() - HOVER_GAP - total;
    let x = if right_x + total <= canvas.right() {
        right_x
    } else if left_x >= canvas.left() {
        left_x
    } else {
        return None;
    };
    // 有卡片角标时再抬高一个按钮高度：角标在节点右上角外侧，工具条不能压住它。
    let lift = if has_card {
        HOVER_BTN + HOVER_GAP_V
    } else {
        0.0
    };
    let above = node.top() - HOVER_BTN - HOVER_GAP_V - lift;
    let below = node.bottom() + HOVER_GAP_V;
    let y = if above >= canvas.top() {
        above
    } else if below + HOVER_BTN <= canvas.bottom() {
        below
    } else {
        // 节点上顶天、下立地（或画布比按钮还矮）：不画，也不把命中区压到折叠点上。
        return None;
    };
    let plus = egui::Rect::from_min_size(Pos2::new(x, y), Vec2::splat(HOVER_BTN));
    let more = plus.translate(Vec2::new(HOVER_BTN + HOVER_BTN_GAP, 0.0));
    Some((plus, more))
}

/// 右键落点。壳层据此决定弹哪张菜单；具体条目由画布当前选择与剪贴板决定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextTarget {
    /// 落在节点上。选区大小由 [`Canvas::selected_count`] 决定（>1 即多选菜单）。
    Node,
    /// 落在选中的注解（概要/边界）上：菜单只给这条注解的动作。
    Annotation,
    /// 落在选中的关系线上：菜单只给这条关系线的动作。
    Relationship,
    /// 落在画布空白处。
    Blank,
    /// 落在概念分组框的描边上（CM-18）：菜单只给这个分组的动作。
    Group,
}

/// 一次右键请求。`pos` 是窗口逻辑像素坐标（含面板原点），菜单直接按它定位。
#[derive(Clone, Copy, Debug)]
pub struct ContextRequest {
    pub pos: egui::Pos2,
    pub target: ContextTarget,
}

/// 概念图验收钩子（CM-08）：`MW_CONCEPT_EDIT` / `MW_CONCEPT_LINK` / `MW_CONCEPT_REL_EDIT`
/// 各自要跑的一次性动作。
///
/// 三者都只转发到生产的入口函数——`start_edit`（双击/F2）、`commit_link`（鼠标松手）、
/// `start_rel_edit`（双击连线中点）。**跳过的是手势，不是逻辑**：省掉的只有「先把指针
/// 移到那个像素上」，因为相机 fit 之后的屏幕坐标会随窗口与内容漂，脚本里写死就是赌。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConceptHook {
    /// 按概念文字进入就地改名。
    Edit(String),
    /// 按概念文字连一条线（源，目标）。
    Link(String, String),
    /// 按链接短语打开已有连线的短语编辑框。
    Phrase(String),
    /// 从概念拖到空白处（源概念、世界坐标偏移 dx/dy）：就地新建概念并连线。
    LinkBlank(String, i32, i32),
    /// 只**停在**拖拽中（不松手），用来拍落空占位预览这一帧。
    LinkDrag(String, i32, i32),
    /// 把某个短语的线改到空白处（短语、dx、dy）：落空即新建概念并改线。
    RelinkBlank(String, i32, i32),
    /// 把某个短语的线**停在拖终点途中**（短语、dx、dy）：选中它、指针停在目标端偏移处，不提交。
    RelinkDrag(String, i32, i32),
    /// 只**选中**某条关系（按短语），不进入拖终点——用来给「强调」做对照。
    SelectRel(String),
    /// 给当前文档加一条概要并选中它（按概要文字）：让「选中环 + 跨度手柄」进像素。
    SummarySelect(String),
    /// 给当前文档加一条边界并选中它：补「边界选中环」，与概要共用同一段绘制。
    BoundarySelect,
    /// 加一条概要并停在「跨度拖拽中」的预览态（按概要文字）：补跨度预览框的画面证据。
    SpanDrag(String),
    /// 概要 + 边界**同时选中**（按概要文字）：additive 选择只有这个形态能被看见。
    MultiSelect(String),
    /// 概要 + 边界同时选中后**取消边界**（按概要文字）：取消后应与单选基线逐像素一致。
    MultiSelectOff(String),
    /// 跨度拖拽**松手提交**（按概要文字）：补「提交发命令 + 只撤一次 + 画面停在变宽之后」。
    SpanCommit(String),
    /// CM-14：选中一颗概念并进入连线态（按概念文字）。`cm-link-mode` 用它。
    LinkMode(String),
    /// CM-14：选中两颗概念后直连（按两颗概念的文字）。`cm-link-two` 用它。
    LinkTwo(String, String),
    /// CM-14：选中某条关系并反向（按短语）。`cm-rel-reverse` 用它。
    Reverse(String),
    /// CM-14：打开某条关系的短语框并预置输入（短语、输入）。`cm-phrase-suggest` 用它。
    Suggest(String, String),
    /// CM-16：把某条关系设成「虚线 + 粗线 + 有注释」并选中它（按短语）。`cm-rel-style` 用它。
    RelStyle(String),
    /// CM-17：按短语加选多条命题（`cm-rel-multi` 用它）。
    MultiRel(Vec<String>),
    /// CM-18：按概念文字建一个分组（`cm-group` 用它）。
    Group(Vec<String>),
}

/// 演示控制条要显示的一帧状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemoInfo {
    /// 当前节点在可见前序里的下标（0 起）。
    pub index: usize,
    pub total: usize,
    /// 当前节点文字（控制条上截断显示）。
    pub title: String,
    /// 演讲者备注：节点备注优先，否则卡片正文。
    pub note: Option<String>,
    /// 当前节点卡片 id（有卡片才能打开卡片页）。
    pub card: Option<String>,
    pub bar_hidden: bool,
}

/// 拖拽改结构的附加状态。`Tree` 必须在**按下那一刻**拍快照：拖拽期间 `Scene` 不重排，
/// 落点索引才有稳定的参照系。
pub(crate) struct Moving {
    tree: mw_ui::Tree,
    /// 参与拖拽的根（选中集合里祖先未被一起选中的那些）。
    roots: Vec<u32>,
    /// 会被挪走的全部节点（根 + 后代，按 draw_order 排好给渲染层用）。
    nodes: Vec<u32>,
    /// 落在这些节点上算非法（就是 `nodes` 的集合形式）。
    banned: HashSet<u32>,
    /// 屏幕像素偏移。幽灵跟手用它，世界坐标偏移再除以 zoom。
    offset: Vec2,
    /// 当前落点；`None` = 指针不在任何节点上。
    drop: Option<mw_ui::DropTarget>,
    /// 幽灵的并集盒（**原始**世界坐标，不含偏移）——吸附拿它去对齐别人。
    union: mw_core::Rect,
    /// 本帧的吸附量（已计入 `offset`）。
    snap: mw_ui::Snap,
    /// 吸附候选第一次出现的时间（egui 的 `input.time`），用于 docs/06:125 的 60ms 延迟。
    snap_since: Option<f64>,
    /// 拖拽过程中指针是否还在画布里。松手那一帧指针状态可能已经被清掉，
    /// 所以每帧在拖动里记一次，提交时读这个快照。
    inside: bool,
    /// 这次拖拽是复制（Ctrl+拖拽）。按下那一刻定，拖动中松开 Ctrl 不改判。
    copy: bool,
}

impl Moving {
    fn start(scene: &Scene, sel: &[u32]) -> Self {
        let tree = mw_ui::Tree::build(scene);
        let roots = mw_ui::drag::drag_roots(&tree, sel);
        let banned = mw_ui::drag::subtree(&tree, &roots);
        let nodes: Vec<u32> = scene
            .draw_order
            .iter()
            .copied()
            .filter(|i| banned.contains(i))
            .collect();
        let mut union = mw_core::Rect::new(0.0, 0.0, 0.0, 0.0);
        for (k, &i) in nodes.iter().enumerate() {
            let r = scene.rect(i);
            union = if k == 0 { r } else { union.union(r) };
        }
        Self {
            tree,
            roots,
            nodes,
            banned,
            offset: Vec2::ZERO,
            drop: None,
            union,
            snap: mw_ui::Snap::default(),
            snap_since: None,
            inside: true,
            copy: false,
        }
    }

    fn illegal(&self) -> bool {
        self.drop.is_some_and(|d| d.illegal)
    }

    /// 幽灵的世界坐标偏移。
    fn world_offset(&self, zoom: f32) -> Point {
        let z = zoom.max(1e-4);
        Point::new(self.offset.x / z, self.offset.y / z)
    }
}

/// 性能浮层的一帧读数（docs/08 §8）。只在 `MW_STATS=1` 时计算，默认零开销。
#[derive(Clone, Copy, Debug, Default)]
pub struct PerfSnapshot {
    pub nodes: usize,
    pub visible: usize,
    pub zoom: f32,
    pub low_render: bool,
    pub motion: bool,
}

/// 注解命中（纯函数，便于单测）：概要在上、边界在下；点进任何节点盒子就判给节点。
fn annotation_hit(
    scene: &Scene,
    camera: &mw_render::Camera,
    local: Point,
    viewport: [f32; 2],
) -> Option<AnnotationSel> {
    let world = camera.screen_to_world(local, viewport);
    if scene.nodes.iter().any(|n| n.geom.rect.contains(world)) {
        return None;
    }
    for s in &scene.summaries {
        if s.topic.expanded(4.0).contains(world) {
            return Some(AnnotationSel {
                parent: scene.ids.get(s.owner as usize)?.clone(),
                kind: AnnotationKind::Summary,
                id: s.id.clone(),
            });
        }
    }
    for b in &scene.boundaries {
        if b.rect.contains(world) {
            return Some(AnnotationSel {
                parent: scene.ids.get(b.owner as usize)?.clone(),
                kind: AnnotationKind::Boundary,
                id: b.id.clone(),
            });
        }
    }
    None
}

/// 注解多选的点击语义：普通点击替换整组，Shift/Ctrl 点击切换单条进/出。
///
/// 抽成纯函数是为了能在无 GPU 的测试里钉住：重复 Shift 点击同一条必须回到原状态，
/// 普通点击必须把多选收成单选。
fn annotation_click(list: &mut Vec<AnnotationSel>, ann: AnnotationSel, additive: bool) {
    if additive {
        if let Some(pos) = list.iter().position(|a| *a == ann) {
            list.remove(pos);
        } else {
            list.push(ann);
        }
    } else {
        list.clear();
        list.push(ann);
    }
}

/// 当前 targets 在父级 children 里的最小/最大下标。任何 target 都找不到时为空。
///
/// targets 来自文件，顺序不保证与 children 一致；跨度拖拽只认「首尾」，所以先归一成
/// 闭区间，拖动时再用 `span_range` 保证非空。
fn span_bounds(targets: &[String], children: &[String]) -> Option<(usize, usize)> {
    let mut lo = usize::MAX;
    let mut hi = 0usize;
    let mut found = false;
    for t in targets {
        if let Some(p) = children.iter().position(|c| c == t) {
            lo = lo.min(p);
            hi = hi.max(p);
            found = true;
        }
    }
    found.then_some((lo, hi))
}

/// 跨度拖拽：指针 y 落在同侧兄弟里最近的那一个（返回父级 children 下标）。
///
/// 只看拉开后的**纵向距离**，不看 x：同侧兄弟在放射式布局里可能左右两列交错，
/// 按文档顺序硬推会一跳两行。`centers` 在按下时拍好，拖动期间不再碰场景。
fn span_candidate(centers: &[(usize, f32)], y: f32) -> Option<usize> {
    centers
        .iter()
        .min_by(|a, b| {
            let da = (a.1 - y).abs();
            let db = (b.1 - y).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(pos, _)| *pos)
}

/// 拖动一端后的新区间：上端只能往回收/往外扩到终点之前，下端反之。区间永远非空。
fn span_range(start: usize, end: usize, end_kind: SpanEnd, candidate: usize) -> (usize, usize) {
    match end_kind {
        SpanEnd::Top => (candidate.min(end), end),
        SpanEnd::Bottom => (start, candidate.max(start)),
    }
}

/// 画布上被选中的注解（边界或概要）。`parent` 是持有它的节点。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnotationSel {
    pub parent: String,
    pub kind: AnnotationKind,
    pub id: String,
}

impl AnnotationSel {
    pub fn label(&self) -> &'static str {
        match self.kind {
            AnnotationKind::Summary => "概要",
            AnnotationKind::Boundary => "边界",
        }
    }
}

/// 跨度手柄在哪一端。拖动上端改区间起点，拖动下端改区间终点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpanEnd {
    Top,
    Bottom,
}

/// 注解跨度手柄的世界坐标（概要在花括号两端，边界在上下缘中点）。
#[derive(Clone, Debug)]
pub(crate) struct SpanHandles {
    ann: AnnotationSel,
    top: Point,
    bottom: Point,
}

/// 画布上选中的关系线（from = 起点节点，id = 关系线 id）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationshipSel {
    pub from: String,
    pub id: String,
    pub to: String,
}

/// 关系线选中集（CM-17）：命题可以多选，批量删 / 改箭头 / 改线型 / 改线宽。
///
/// 约定：
/// - **顺序 = 选中顺序**，最后一个是「主选」——检查器显示它的值，右键菜单的勾也按它打。
/// - 单选用 [`Self::select_only`]，多选用 [`Self::toggle`]（加选/减选），框选走 [`Self::replace`]。
/// - 空集 = 没有选中关系线（节点/注解的选中态与它互斥，见 `clear_foreign_selection`）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelationshipSelection {
    items: Vec<RelationshipSel>,
}

impl RelationshipSelection {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 主选（最后选中的那条）。检查器、右键菜单都读它。
    pub fn primary(&self) -> Option<&RelationshipSel> {
        self.items.last()
    }

    pub fn as_slice(&self) -> &[RelationshipSel] {
        &self.items
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// 单选：清掉别的，只留这一条。
    pub fn select_only(&mut self, sel: RelationshipSel) {
        self.items.clear();
        self.items.push(sel);
    }

    /// 加选 / 减选（Ctrl / Shift + 点击）。已选中的再点一次就移除。
    pub fn toggle(&mut self, sel: RelationshipSel) {
        if let Some(i) = self
            .items
            .iter()
            .position(|r| r.id == sel.id && r.from == sel.from)
        {
            self.items.remove(i);
        } else {
            self.items.push(sel);
        }
    }

    pub fn contains(&self, from: &str, id: &str) -> bool {
        self.items.iter().any(|r| r.from == from && r.id == id)
    }

    /// 场景重建后清掉已经不存在的线（反向连接、撤销、删节点都会让 `(from, id)` 失效）。
    pub fn retain_alive(&mut self, alive: impl Fn(&RelationshipSel) -> bool) {
        self.items.retain(|r| alive(r));
    }
}

/// 关系线标签的画布内联编辑态。
#[derive(Clone, Debug)]
struct RelEdit {
    sel: RelationshipSel,
    text: String,
    opening: String,
    focus: bool,
    rect: egui::Rect,
    /// 输入法正在组合：这期间 Enter 归 IME（选字），不能当成提交。
    ime: bool,
    /// 这条关系线是「刚拖出来、还没写短语」的：留空取消时要把线一起撤掉，
    /// 否则 `docs/30` §5.9 说的「未填不给连线落盘」就被绕过去了。
    blank_new: bool,
    /// 短语建议下拉里当前高亮的那一行（`docs/30` §5.4.2）。只在建议可见时有意义。
    pick: usize,
}

/// 概念图里 `Ctrl+L` / 「连接」入口按选中数量分岔（`docs/30` §5.4.2）。
///
/// 命题只连两端：选中两颗就是「把它们连起来」，选中一颗才需要「再点一个」。
/// 抽成纯函数是为了把「三颗以上不许静默变成两两相连」钉在单测里。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ConceptLinkIntent {
    /// 选中两颗：直接建一条命题。
    Direct,
    /// 选中一颗：进连线态，等第二次点击。
    Arm,
    /// 一颗都没选：提示先选。
    NeedSelection,
    /// 三颗以上：提示只连两个。
    TooMany,
}

pub(crate) fn concept_link_intent(selected: usize) -> ConceptLinkIntent {
    match selected {
        2 => ConceptLinkIntent::Direct,
        1 => ConceptLinkIntent::Arm,
        0 => ConceptLinkIntent::NeedSelection,
        _ => ConceptLinkIntent::TooMany,
    }
}

/// 短语建议下拉的矩形（`docs/30` §5.4.2）。
///
/// 贴在编辑框正下方 4px；下方放不下就翻到上方；两侧夹进画布，四周留 8px。
/// 行高 24、上下内距 4；宽度取编辑框宽与 180 的较大者。
/// 抽成纯函数是为了能测：下拉跑出画布就等于「建议在窗口外」，用户根本点不到。
pub(crate) fn phrase_suggest_rect(
    anchor: egui::Rect,
    rows: usize,
    canvas: egui::Rect,
) -> egui::Rect {
    const ROW_H: f32 = 24.0;
    const PAD: f32 = 4.0;
    const GAP: f32 = 4.0;
    const EDGE: f32 = 8.0;
    let h = rows as f32 * ROW_H + PAD * 2.0;
    let w = anchor.width().max(180.0);
    let mut x = anchor.left();
    if x + w > canvas.right() - EDGE {
        x = canvas.right() - EDGE - w;
    }
    if x < canvas.left() + EDGE {
        x = canvas.left() + EDGE;
    }
    let below = anchor.bottom() + GAP;
    let y = if below + h <= canvas.bottom() - EDGE {
        below
    } else {
        let above = anchor.top() - GAP - h;
        if above >= canvas.top() + EDGE {
            above
        } else {
            canvas.bottom() - EDGE - h
        }
    };
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
}

/// 画布顶部「聚焦问题」带子的矩形（CM-10 ⑥）。
///
/// 纯几何，单独抽出来是为了能测：横向居中、贴顶 10px、宽度不超过画布减 32px
/// （长问题不能压到画布边缘外），高度按文字行高加上下 7px 内边距。
pub(crate) fn focus_band_rect(origin: egui::Rect, text: egui::Vec2) -> egui::Rect {
    let pad = egui::vec2(16.0, 7.0);
    let w = (text.x + pad.x * 2.0).min(origin.width() - 32.0);
    egui::Rect::from_center_size(
        egui::Pos2::new(
            origin.center().x,
            origin.min.y + 10.0 + (text.y + pad.y * 2.0) * 0.5,
        ),
        egui::vec2(w, text.y + pad.y * 2.0),
    )
}
/// 问题带换行时的**行宽上限**（第 25 轮）。
///
/// 画布宽 − 32（`focus_band_rect` 对带子宽的夹法）− 32（带子左右各 16 内距）。
/// 不换行的话，137 个字的一行会被窗口左右直接切掉——实测 ink 从 x=1 铺到 x=1918，
/// 连侧栏都压过去；而问题恰恰可能问得很长（`docs/32` §9）。
pub(crate) fn focus_wrap_width(origin: egui::Rect) -> f32 {
    (origin.width() - 64.0).max(120.0)
}
/// 画布顶部「聚焦问题」的就地编辑态（CM-10 ⑥，`docs/32` §2）。
///
/// 与节点编辑器分开：那个编辑器住在 `Scene` 里、由 wgpu 排版，问题带是壳层的
/// 屏幕空间文字，两者的生命周期没有交集。
///
/// 导出那条路**不是**这里：导出没有视口，带子由 `Scene::focus_note` 锚在内容顶部
/// （`export::attach_focus_note`），画布永远不填那个字段——所以滚动时带子不走、
/// 导出图里也照样有（docs/32 §6）。
#[derive(Clone, Debug)]
pub(crate) struct FocusEdit {
    pub(crate) text: String,
    /// 打开时的原文：没改就退出不发命令，撤销栈里不留空记录。
    pub(crate) opening: String,
    pub(crate) focus: bool,
    pub(crate) rect: egui::Rect,
    /// 输入法正在组合：这期间 Enter 归 IME（选字），不能当成提交。
    pub(crate) ime: bool,
}

/// 一次「拖关系线终点换目标」的状态。
#[derive(Clone, Debug)]
pub(crate) struct RelinkDrag {
    sel: RelationshipSel,
    /// 指针当前位置（世界坐标）。
    at: Point,
    /// 松手会连到谁；`None` = 空白。
    target: Option<u32>,
}

/// 概要文字的画布内联编辑态（WP10/P01）。与节点编辑器分开：概要不是 `SceneNode`，
/// 硬塞进节点编辑器会把场景索引、布局测量和 commit 路径全部带歪。
#[derive(Clone, Debug)]
struct SummaryEdit {
    parent: String,
    id: String,
    text: String,
    /// 打开时的文字：提交时没变就不发命令。
    opening: String,
    /// 第一帧要抢焦点；之后失焦即提交。
    focus: bool,
    /// 当前屏幕矩形（光标判断与点击穿透用）。
    rect: egui::Rect,
}

/// 画布上的三条选择道。任何时刻只允许一条非空（打磨轮 49 / M26）。
///
/// 节点 / 注解 / 关系线不是三层可以叠加的高亮：检查器显示哪一组、`Delete` 删什么、
/// `Esc` 退哪一层，都按「当前是哪一道」决定。两条道同时非空时，一次 `Delete`
/// 会先删关系线、再给节点立删除确认——用户只按了一次键，却同时失去了两样东西。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionLane {
    Node,
    Annotation,
    Relationship,
}

/// 除 `keep` 之外、当前非空、必须清掉的道（顺序：关系线、注解、节点）。
pub(crate) fn foreign_lanes(
    keep: SelectionLane,
    relationship: bool,
    annotations: usize,
    nodes: usize,
) -> [bool; 3] {
    [
        relationship && keep != SelectionLane::Relationship,
        annotations > 0 && keep != SelectionLane::Annotation,
        nodes > 0 && keep != SelectionLane::Node,
    ]
}

/// 一次 `Delete` 落在哪条道上（优先级：关系线 → 注解 → 节点）。
///
/// 判据只看「哪条道非空」，因此即使上游漏清、两条道同时非空，
/// 一次按键也绝不会跨道删两类东西。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeleteLane {
    Relationship,
    Annotation,
    Nodes,
    Nothing,
}

pub(crate) fn delete_lane(relationship: bool, annotations: usize, nodes: usize) -> DeleteLane {
    if relationship {
        DeleteLane::Relationship
    } else if annotations > 0 {
        DeleteLane::Annotation
    } else if nodes > 0 {
        DeleteLane::Nodes
    } else {
        DeleteLane::Nothing
    }
}

/// 一次跨度拖拽的完整状态。
///
/// `centers` 在按下瞬间拍好：拖动期间只做「指针 y → 最近的同侧兄弟」，不再碰场景，
/// 避免每帧重算一遍可见性，也避免拖动中文档变化把基准悄悄换掉。
#[derive(Clone, Debug)]
pub(crate) struct SpanDrag {
    ann: AnnotationSel,
    end: SpanEnd,
    /// 父节点的直接子节点（文档顺序，也是 `targets` 的合法取值域）。
    children: Vec<String>,
    /// 当前区间 [start, end] 在 `children` 里的下标。
    start: usize,
    end_idx: usize,
    /// (children 下标, 屏幕外/同侧可见子节点的世界 y 中心)
    centers: Vec<(usize, f32)>,
    /// 按下时的 targets，松手时与 `preview` 比较，没变就不发命令。
    origin: Vec<String>,
    /// 拖动中的预览 targets（始终是 `children[start..=end_idx]`）。
    preview: Vec<String>,
}

pub struct Canvas {
    /// 文档会话（文档 / 布局 / 主题 / 路径 / 脏标记）。画布只借用它，不拥有语义。
    session: Session,
    scene: Scene,
    /// 测量缓存：只改一个节点时其余节点不重新排版文字。
    scene_cache: mw_core::SceneCache,
    hit: HitIndex,
    measure: Box<dyn TextMeasurer>,
    text: Option<TextEngine>,
    compositor: Compositor,
    camera: Camera,
    /// 有序多选，**末尾即主选**（Tab/Enter/F2/Delete 的目标）。语义在 `mw_ui::Selection`。
    sel: Selection,
    /// 被选中的边界/概要（末位为主选）。与节点选择互斥：选中注解时节点选择会清空。
    annotations: Vec<AnnotationSel>,
    /// 正在拖的注解跨度手柄（WP9）。与 `drag.kind == Span` 同生同灭。
    span_drag: Option<SpanDrag>,
    /// 正在画布内编辑的概要文字（P01）。
    summary_edit: Option<SummaryEdit>,
    /// 画布上选中的关系线（P03）。
    relationship: RelationshipSelection,
    /// 右键命中的分组 id（CM-18）：分组菜单的动作都作用于它。
    context_group: Option<String>,
    /// 插入关系线的进行态：源节点 id。`None` = 不在插入态。
    /// 只是一枚视图态——不碰文档，落点时才发一条 `AddRelationship`。
    rel_link: Option<String>,
    /// 正在拖关系线终点（P03）。
    relink_drag: Option<RelinkDrag>,
    /// 正在画布内编辑关系线标签（P03）。
    rel_edit: Option<RelEdit>,
    /// 画布顶部「聚焦问题」的就地编辑态；`None` = 没在编辑。
    focus_edit: Option<FocusEdit>,
    hovered: Option<u32>,
    /// 框选进行中「松手会选中谁」的预览（打磨轮 9）。只在拖拽画橡皮筋时非空；
    /// 用来给命中的节点实时画选中环，并在集合变化时写一行轨迹。
    marquee_preview: Option<Vec<u32>>,
    /// 指针悬停的关系线：`(起点节点 id, 关系 id)`。
    ///
    /// 用 id 而不是下标——重建场景会重排 `scene.relationships` 的下标，下标一漂就会
    /// 强调到另一条线上（而且只在改文档之后才错，最难查）。
    hovered_rel: Option<(String, String)>,
    /// 概念图悬停提示的当前目标与起始时刻：(节点, 是否停在卡片圆点上)。
    /// 换目标就重置计时，所以「划过整张图」不会一路闪提示。
    tip: Option<(u32, bool)>,
    tip_at: f64,
    /// 离屏 golden 关掉 hover，避免真实鼠标位置让基线在“有/无悬停条”之间抖动。
    hover_enabled: bool,
    /// 悬停环透明度（120ms 过渡）。
    hover_alpha: f32,
    /// 悬停的折叠圆点/概要/边界（打磨轮 37）。与 `hovered` 并存：指着折叠点时两者都非空。
    hover_detail: Option<hover::HoverDetail>,
    /// 注解/折叠点悬停的淡入透明度（120ms 过渡）。
    hover_detail_alpha: f32,
    /// 选中环透明度（180ms 过渡）。
    select_alpha: f32,
    /// 画布上点了卡片角标：壳层下一帧打开阅读页。
    card_open: Option<String>,
    /// 画布想对用户说的一句话（文案, 是否告警）。壳层下一帧取走并走底部提示条。
    ///
    /// 画布没有自己的 toast——提示条是壳层的铬件。用一次性字段而不是回调，
    /// 与 `card_open` 同一种「画布置、壳层取」的通道。
    notice: Option<(String, bool)>,
    texture: Option<egui::TextureId>,
    fitted: bool,
    /// Alt+拖空白的套索路径（窗口坐标）。`None` = 没在套索。
    lasso: Option<Vec<Pos2>>,
    /// 网格与标尺：默认关，开了也克制。
    show_grid: bool,
    grid_step: f32,
    show_rulers: bool,
    /// 阅读模式：隐藏卡片角标与悬浮工具条，只留结构、选择与连线。
    reading: bool,
    /// 上一帧的画布视口（逻辑像素）。命令面板的"适应画布"要用它。
    last_viewport: [f32; 2],
    /// 上一帧画布在窗口里的矩形。右键菜单给的是窗口坐标，新建自由主题要换成画布局部坐标。
    last_rect: egui::Rect,
    /// 上一帧画布在**窗口**里的中心（逻辑 px）。大纲/检查器/标签条开合会挪动它，
    /// 相机必须反向补偿，否则整张图会跟着平移（13 §4.20 的缺口）。
    last_view_center: Option<egui::Pos2>,
    /// 上一帧的整窗尺寸。只有它没变、画布中心却变了，才认定是面板开合而不是窗口缩放：
    /// 窗口缩放本来就该重新居中，不能把画面钉死在屏幕上。
    last_window_size: Option<egui::Vec2>,
    budget: FrameBudget,
    editor: Option<Editor>,
    /// 上一帧行内编辑器的实际盒子（全局像素）。按下点落在它里面时，那一下归编辑器，
    /// 画布不能抢去平移——见 `editor_owns_press`。
    edit_rect: Option<egui::Rect>,
    /// 提交/取消后把键盘焦点还给画布，快捷键才能立刻继续用。
    refocus_canvas: bool,
    /// 画布是否握着键盘（`F6` 在画布与面板之间交接）。为假时画布连焦点都不要：
    /// 一个不能被聚焦的区域才会从 Tab 环里消失，否则 `Tab` 一按又回到画布，面板
    /// 永远走不到——检查器的强调色色板就是这么变成鼠标专属的。
    wants_keys: bool,
    /// 按下到松手之间的手势；`None` 表示没有进行中的拖拽。
    drag: Option<Drag>,
    /// 拖拽改结构的附加状态（树快照、幽灵偏移、落点）。压过阈值才建。
    moving: Option<Moving>,
    /// 拖出连线的进行态。
    link: Option<Link>,
    /// 被浮层（命令面板）挡住时不收键盘与指针——否则一边打字一边把选中的节点删了。
    input_blocked: bool,
    /// 画布内部浮层（缩放控件/聚焦面包屑）的屏幕矩形。指针落在里面时画布不收手势：
    /// 否则在浮层上拖一下，底下的画布会当成「点空白」把选中清掉。
    overlay: Vec<egui::Rect>,
    /// 画布自绘控件（选中节点的「＋」）的屏幕矩形；与 `overlay` 同样做指针门禁。
    chrome_gate: Option<egui::Rect>,
    /// 聚焦问题带的屏幕矩形（CM-10 ⑥）。带子是画布上的覆盖层，指针落在里面就不能
    /// 再当成「点空白」——否则双击带子会在它背后顺手新建一个浮动主题（2026-10-27 修）。
    focus_band: Option<egui::Rect>,
    /// 框选已提交：本帧的 `clicked` 必须吞掉（阈值 4px 小于 egui 的 6px 判定窗）。
    swallow_click: bool,
    /// 进程内节点剪贴板：复制时保存整棵子树快照，粘贴时重映射 ID。
    clipboard: Vec<Node>,
    /// 样式剪贴板。外层 `None` = 还没复制过；内层 `None` = 复制的是「无覆盖」。
    style_clipboard: Option<Option<StyleOverride>>,
    /// Alt+Enter 聚焦栈：末位是当前布局根，空表示全图。
    focus: Vec<String>,
    /// 搜索命中的呼吸外环；选择一变就自然失效。
    glow: Option<Glow>,
    /// 系统「减少动画」：只保留静态反馈，不做脉冲/过渡。
    reduce_motion: bool,
    /// 本帧的右键请求。壳层取走后构建菜单；不取就丢，不会跨帧残留。
    context: Option<ContextRequest>,
    /// 结构/布局过渡：`from` 是变化前的**可见**场景，目标永远是 `self.scene`。
    motion: Option<Motion>,
    /// 演示模式（F5）。`Some` 时渲染它的高亮场景副本与深色主题，输入由壳层锁死。
    demo: Option<Demo>,
    /// 本帧时间（egui 秒）。真机是墙钟，金样是 M16 的帧计数虚拟时钟
    /// （`main.rs` 每帧把 `raw_input.time` 设成它），所以按时间老化的反馈也能拍得住。
    now: f64,
    /// 概念图落点与别人重叠时的 1px 告警描边：`(概念 id, 起亮时刻)`。
    /// 只活 `flash_window`（缺省 0.25s，`docs/30` §5.5），过期即丢，不参与文档、不进导出。
    overlap_flash: Option<(Vec<String>, f64)>,
    /// 告警边的存活窗口（秒）。见 [`overlap_flash_window`]。
    flash_window: f64,
    /// 整理布局那次提交之后栈深。撤销踩到这一条时，位置撤回来了，取景也得跟着 `fit` 回去，
    /// 否则用户看到的是「原来的散布 + 整理后的取景」。
    arrange_undo_depth: Option<usize>,
    /// 刚刚撤销掉的就是整理布局：下一次重做要再取一次景。
    arrange_redo_fit: bool,
    /// 上一次取景用的视口。启动阶段壳层拿它判断「窗口尺寸落定了没有」：
    /// 视口和这份不一样才重取景（落定在第几帧都出同一张图）。
    fit_viewport: Option<[f32; 2]>,
    /// 待确认的删除。`Some` 时画布上立着确认框，键盘与指针都不归别的路径。
    confirm_delete: Option<DeleteConfirm>,
    /// 折叠圆点的双击记忆（M32）：第二击沿用第一击之前的状态下钻一层。
    /// 判定只看「同节点 + 500ms + 4 css px + 撤销深度没变」，纯逻辑在 `mw_ui::drill_click`。
    toggle_clicks: ToggleClickMemory,
}

/// 搜索命中的高亮：前 700ms 呼吸两次，之后常亮到选择变化。
struct Glow {
    id: String,
    start: Instant,
}

/// 节点增删、折叠展开与布局切换的过渡层。
///
/// `self.scene` 始终是目标场景（选择/命中/面板都按它算），渲染时才把
/// [`mw_core::blend_scenes`] 的结果喂给合成器。动画中途再来一条命令时，
/// `rebuild` 会先把「此刻屏幕上的那一帧」固化下来当新的起点，避免位置回跳。
struct Motion {
    from: Scene,
    start: Instant,
    duration: f32,
}

impl Motion {
    fn new(from: Scene, to: &Scene) -> Option<Self> {
        let span = motion_span(&from, to)?;
        Some(Self {
            from,
            start: Instant::now(),
            duration: motion_duration(span),
        })
    }

    fn progress(&self) -> f32 {
        (self.start.elapsed().as_secs_f32() / self.duration).clamp(0.0, 1.0)
    }

    fn active(&self) -> bool {
        match motion_pin() {
            Some(t) => t < 1.0,
            None => self.progress() < 1.0,
        }
    }

    fn scene(&self, to: &Scene) -> Scene {
        let t = match motion_pin() {
            Some(t) => t,
            None => ease_out_cubic(self.progress()),
        };
        mw_core::blend_scenes(&self.from, to, t)
    }
}

/// 证据用的「定格」：`MW_MOTION_T=0.5` 把过渡的混合系数钉死在中途。
///
/// 过渡只有 0.2–0.4s，截图靠墙上的钟永远追不上「刚好一半」的那一帧——
/// CM-10 §3 第 10 条（强调淡入的中间帧）就是卡在这里只能拍稳态。钉死之后
/// 那一帧会一直停着，金样才能把它锁住。不设这个变量时一次原子读都不多花
/// （与 `MW_STATS` / `MW_CARD_PERF` 同一套做法）。
pub(crate) fn motion_pin() -> Option<f32> {
    static PIN: OnceLock<Option<f32>> = OnceLock::new();
    *PIN.get_or_init(|| {
        std::env::var("MW_MOTION_T")
            .ok()
            .as_deref()
            .and_then(parse_pin)
    })
}

/// `MW_MOTION_T` 怎么读：空白、非数、`NaN` 一律当「没设」，其余夹到 `0..=1`。
///
/// `NaN` 单独挡一道不是洁癖：`f32::clamp` 遇 `NaN` 会返回 `NaN`，
/// 混合系数变成 `NaN` 之后整张场景的几何全是 `NaN`，画出来是一片空白。
fn parse_pin(raw: &str) -> Option<f32> {
    raw.trim()
        .parse::<f32>()
        .ok()
        .filter(|t| t.is_finite())
        .map(|t| t.clamp(0.0, 1.0))
}

/// `MW_CONTEXT_POS='x,y'`：右键菜单**请求**落在哪（CSS px）。
///
/// 菜单场景以前都写死在 `(360,120)`——贴着左上角开，永远碰不到底边，
/// 于是「子菜单贴底被夹住」那一档只有单测、没有图（CM-10 §3 第 8 条）。
/// 这个变量只挪请求点，夹取逻辑一行没动。
pub(crate) fn context_pos() -> Option<egui::Pos2> {
    std::env::var("MW_CONTEXT_POS")
        .ok()
        .and_then(|s| parse_pos(&s))
}

/// `'x,y'` 怎么读：两个都能解析成有限数才算数，否则当「没设」（退回默认点）。
fn parse_pos(raw: &str) -> Option<egui::Pos2> {
    let (x, y) = raw.split_once(',')?;
    let x: f32 = x.trim().parse().ok()?;
    let y: f32 = y.trim().parse().ok()?;
    (x.is_finite() && y.is_finite()).then(|| egui::pos2(x, y))
}

mod motion;
pub(crate) use motion::*;
/// 行内编辑状态。`laid_out` 是上次喂给 `build_scene` 的快照——只有它变了才重排。
struct Editor {
    id: String,
    original: String,
    text: String,
    laid_out: String,
    focused: bool,
    /// 输入法正在组合（preedit）。组合期间 Enter/Esc 归 IME，不能用来提交/取消。
    ime: bool,
    /// 这枚节点是**这次编辑**顺手建出来的（`Tab` / `Enter` / 悬停「＋」）。`Esc` 取消时
    /// 它该跟着编辑一起消失——否则树上留一个点不到的空白盒子。
    fresh: bool,
    /// `fresh` 那枚节点建完之后的撤销栈深。取消时拿它和当前栈深比对：一样说明栈顶仍是
    /// 「建这枚节点」那条命令，一次 `undo` 就能把整个动作收回，不必再压一条
    /// `DeleteNodes`（那会让用户按两下 `Ctrl+Z` 才回到建之前）。
    fresh_depth: Option<usize>,
}

/// 等确认的删除。只有「待删集合里还有节点带着子节点」时才会出现。
#[derive(Clone)]
struct DeleteConfirm {
    ids: Vec<String>,
    /// 真要删掉几个节点（含整片子树，已去重）。
    count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AddKind {
    Child,
    Sibling,
}

#[derive(Default)]
struct EditorAction {
    changed: bool,
    lost_focus: bool,
    /// 编辑器这一帧真正占用的盒子；画布下一帧据此判断左键该不该让给它。
    rect: Option<egui::Rect>,
}

/// 画布在焦点环里的成员资格。`F6` 把键盘交给面板时，画布必须连「可聚焦」都放弃：
/// 只交出手里的焦点没用，`Tab` 一转又转回画布，焦点锁再把它锁死——检查器的强调色
/// 色板就是这么变成鼠标专属的。
pub(crate) fn canvas_sense(wants_keys: bool) -> egui::Sense {
    if wants_keys {
        egui::Sense::click_and_drag()
    } else {
        egui::Sense::CLICK | egui::Sense::DRAG
    }
}

/// 画布那块区域的固定 id。用它换掉自动 id 是必需的，不是优化：
/// `CentralPanel` 的 `Ui` 由「这一帧注册了哪几个面板」决定身份——开合大纲（`F8`）、
/// 检查器（`F9`）或再加一个标签页，都会让上层 `Ui` 的 auto-id 盐整体漂移，画布的
/// widget id 跟着变。egui 的焦点有个死人开关：本帧 `used_ids` 里查不到就清空焦点，
/// 于是 id 一漂，画布握着的那个焦点无声消失，而画布的裸键快捷键只认 `has_focus`——
/// Tab 加子节点、方向键选节点全体失灵，直到用户重新点一下画布。
pub(crate) fn canvas_id() -> egui::Id {
    egui::Id::unique("mw-canvas-area")
}

/// 节点就地编辑框的 egui id。
///
/// 软换行要按 `TextEdit` 里的光标位置插字，而光标存在这个 id 上——两处必须用同一个
/// 算式，否则按 `Shift+Enter` 会从末尾插而不是从光标插。抽成函数就是为了不出现第二份
/// `"mw-node-edit"` 盐。
pub(crate) fn node_edit_id(id: &str) -> egui::Id {
    egui::Id::unique(("mw-node-edit", id))
}

/// 占满剩余空间并拿到画布的交互 `Response`。和 `allocate_exact_size` 走的是同一套
/// 布局（同样消耗一个 auto-id 盐），只有 id 换成固定的——像素位置不该有任何变化。
pub(crate) fn canvas_area(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    wants_keys: bool,
) -> (egui::Rect, egui::Response) {
    let (_salt, rect) = ui.allocate_space(size);
    let response = ui.interact(rect, canvas_id(), canvas_sense(wants_keys));
    (rect, response)
}

/// 画布握着键盘时锁住的键。`Tab`/方向键/`Esc` 在 egui 里默认是「移动焦点」，而这个
/// 判定发生在 `canvas_area` 的 `interact` 里，比本帧后面读键盘更早；画布是这些键的语义主人
/// （加子节点、选节点、取消选择），必须先把它们锁在自己身上。
pub(crate) fn canvas_key_lock() -> egui::EventFilter {
    egui::EventFilter {
        tab: true,
        escape: true,
        horizontal_arrows: true,
        vertical_arrows: true,
    }
}

/// 文档级剪贴板和弦。它们和撤销/重做同一条纪律：**主人是窗口，不是画布**
/// （`App::window_shortcuts`），因为画布那条路要求画布握着 egui 焦点——鼠标点一下
/// 工具条就把焦点交出去了，之后 Ctrl+C 会静默失灵。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardChord {
    Copy,
    Cut,
    Paste,
    StyleCopy,
    StylePaste,
}

/// 一帧里到过的和弦该执行哪一条。判定和摘事件分开做，是因为「一条都不该执行」的时候
/// 必须一个事件都没被摘走——`Copy/Cut/Paste` 是 `events` 里的普通条目，谁先 `retain`
/// 谁赢，而面板的 `TextEdit` 要到本帧后面才画。
///
/// 带 Shift 的先判：egui-winit 只在**纯 Command**（`modifiers.command_only()`）时才把
/// `Ctrl+C` 翻译成 `Event::Copy`，所以 `Ctrl+Shift+C` 走的是普通 `Event::Key`，一次按键
/// 不会同时点亮两条；但同一帧里先后按过两次时必须有确定顺序，否则一次「复制样式」会
/// 顺带把选中子树也压进剪贴板，撤销栈多出一条看不见的记录。
///
/// `Cut` 排在 `Copy` 前：剪切本身就是「复制 + 删除」，两条同时到过就必须落到带删除的那条，
/// 这是画布时代 `if copy_ev || cut_ev { … }` 的语义，一条不能丢。
pub fn clipboard_chord(
    style_copy: bool,
    style_paste: bool,
    copy: bool,
    cut: bool,
    paste: bool,
) -> Option<ClipboardChord> {
    if style_copy {
        Some(ClipboardChord::StyleCopy)
    } else if style_paste {
        Some(ClipboardChord::StylePaste)
    } else if cut {
        Some(ClipboardChord::Cut)
    } else if copy {
        Some(ClipboardChord::Copy)
    } else if paste {
        Some(ClipboardChord::Paste)
    } else {
        None
    }
}

mod canvas_methods_1;
mod canvas_methods_2;
mod canvas_methods_3;

/// 按下瞬间的手势判定。抽成纯函数只为一件事：这四条规则改错了会立刻毁掉手感，
/// 而它们能脱离窗口、脱离 GPU 断言（`mw-ui` 的同一套纪律）。
fn classify(on_empty: bool, on_anchor: bool, middle: bool, space: bool, editing: bool) -> DragKind {
    if middle || space {
        DragKind::Pan
    } else if editing {
        // 编辑期不框选、不搬节点、也不拉线：能走到这里的按下，要么提交编辑、要么平移。
        // 落在编辑器盒里的那一下左键更早就被 `editor_owns_press` 让给编辑器了。
        DragKind::Pan
    } else if on_anchor {
        // 锚点优先于节点本体：它就在节点边上，先判边界那一圈才不会「想拉线变成搬节点」。
        DragKind::Link
    } else if on_empty {
        DragKind::Marquee
    } else {
        DragKind::Move
    }
}

/// 插入关系线时，这一击该干什么。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RelLinkStep {
    /// 不在插入态：按正常点击处理。
    Idle,
    /// 落在另一个主题上：建线。
    Commit(String),
    /// 点回源主题/空白/注解/关系线：退出插入态，这一击仍按正常点击处理。
    Cancel,
}

/// 插入态下把「点到了谁」翻译成动作。抽成纯函数：落点误判会安静地多出一条线，
/// 或者让用户以为「点了没反应」，两种都得能脱离窗口断言。
///
/// `hit` 只认**节点盒**（调用方传 `Target::Node` 的 id），折叠点/注解/关系线都当空白。
pub(crate) fn rel_link_step(source: Option<&str>, hit: Option<&str>) -> RelLinkStep {
    let Some(source) = source else {
        return RelLinkStep::Idle;
    };
    match hit {
        Some(target) if target != source => RelLinkStep::Commit(target.to_string()),
        _ => RelLinkStep::Cancel,
    }
}

/// 概要 / 边界的键盘入口（`Ctrl+]` / `Ctrl+B`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnnotationAdd {
    Boundary,
    Summary,
}

/// 把按键翻译成注解动作。只认**恰好** `COMMAND` 修饰的 `B` / `]`：无修饰的
/// `B` / `]` 是普通落字（选中态直接开始编辑），不能被这里抢走；带 Shift/Alt 的
/// 变体也不是这个语义。调用方用 `take_exact_key`（精确修饰键）取键。
pub(crate) fn annotation_add_for_key(modifiers: Modifiers, key: Key) -> Option<AnnotationAdd> {
    if modifiers != Modifiers::COMMAND {
        return None;
    }
    match key {
        Key::B => Some(AnnotationAdd::Boundary),
        Key::CloseBracket => Some(AnnotationAdd::Summary),
        _ => None,
    }
}

/// 快捷键失败的原因。菜单靠禁用态表达「现在不能加」，键盘没有禁用态，必须说清楚。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnnotationShortcutError {
    /// 什么都没选中。
    NoSelection,
    /// 选了根主题（或主选不在任何父级下）：没有「同一父级」可圈。
    NoParent,
}

/// 先判原因再发命令：`annotation_targets` 的 `None` 同时盖住「没选」与「选了根」，
/// 两种提示不能混。
pub(crate) fn annotation_shortcut_error(
    doc: &mw_core::Document,
    ids: &[String],
    primary: Option<&str>,
) -> Option<AnnotationShortcutError> {
    let Some(primary) = primary else {
        return Some(AnnotationShortcutError::NoSelection);
    };
    if doc.node(primary).is_none() {
        return Some(AnnotationShortcutError::NoSelection);
    }
    annotation_targets(doc, ids, primary)
        .is_none()
        .then_some(AnnotationShortcutError::NoParent)
}

/// 编辑期哪一次按下归行内编辑器：只有「左键 + 没按住空格 + 点在自己正在编辑的盒里」。
///
/// 划选文字和画布平移抢的是同一个左键。`classify` 在编辑期一律给 `Pan`，于是「拖黑
/// 一个字」会变成「把整张图挪走」——按下点落在编辑器盒里时必须让给编辑器。
/// 中键和空格仍算平移：编辑长文字时把视野挪开是刚需，盒外（卡片空白处、别的节点）
/// 也仍按老语义平移/提交，不动。
fn editor_owns_press(
    edit_rect: Option<egui::Rect>,
    primary: bool,
    middle: bool,
    space: bool,
    at: egui::Pos2,
) -> bool {
    primary && !middle && !space && edit_rect.is_some_and(|r| r.contains(at))
}

/// 把屏幕上的橡皮筋换成世界矩形，交给 `mw_ui::marquee` 做相交判定。
/// 框选结果并入当前选择：默认替换；Shift/Ctrl 并入；Alt 只减。
fn apply_marquee(current: &[u32], picked: Vec<u32>, additive: bool, subtract: bool) -> Vec<u32> {
    if subtract {
        return current
            .iter()
            .copied()
            .filter(|id| !picked.contains(id))
            .collect();
    }
    if additive {
        let mut out = current.to_vec();
        for id in picked {
            if !out.contains(&id) {
                out.push(id);
            }
        }
        return out;
    }
    picked
}

/// 套索松手的结算：`None` 表示「这一下不算」，选择原样保留。
///
/// 与矩形框选只差一处：**Alt（减选）什么都没圈中时不清空选择**。按住 Alt 是想
/// 「从组里去掉一点」，套索擦过去没圈住任何一颗，按 `apply_marquee` 的替换口径结算
/// 会把整组选掉——减选手势最怕这个（打磨轮 62 / M7b）。
///
/// 注意：`DragKind::Lasso` 只可能在按下时带 Alt（`handle_pointer` 用
/// `modifiers.alt` 把空白框选改判成套索），所以真实手势里 `subtract` 恒为真，
/// 套索因此**只减不选**。这条不变量由 `mm-lasso` / `mm-marquee-alt` 的轨迹与
/// `lasso_subtract_only_removes_looped_nodes` 单测钉住。
fn apply_lasso(
    current: &[u32],
    picked: Vec<u32>,
    additive: bool,
    subtract: bool,
) -> Option<Vec<u32>> {
    if subtract && picked.is_empty() {
        return None;
    }
    Some(apply_marquee(current, picked, additive, subtract))
}

/// 射线法：点是否在多边形内（套索）。边界点归属不重要，套索允许差一像素。
fn point_in_polygon(p: Pos2, poly: &[Pos2]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) {
            let x = (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x;
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn lasso_pick(
    camera: &Camera,
    scene: &Scene,
    rect: egui::Rect,
    points: &[Pos2],
    viewport: [f32; 2],
) -> Vec<u32> {
    if points.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for idx in 0..scene.nodes.len() as u32 {
        let c = scene.rect(idx).center();
        let s = camera.world_to_screen(c, viewport);
        let p = Pos2::new(rect.min.x + s.x, rect.min.y + s.y);
        if point_in_polygon(p, points) {
            out.push(idx);
        }
    }
    out
}

fn marquee_pick(
    camera: &Camera,
    scene: &Scene,
    rect: egui::Rect,
    drag: &Drag,
    viewport: [f32; 2],
) -> Vec<u32> {
    let to_local = |p: Pos2| Point::new(p.x - rect.min.x, p.y - rect.min.y);
    let a = camera.screen_to_world(to_local(drag.origin), viewport);
    let b = camera.screen_to_world(to_local(drag.current), viewport);
    let world = mw_core::Rect::new(
        a.x.min(b.x),
        a.y.min(b.y),
        (b.x - a.x).abs(),
        (b.y - a.y).abs(),
    );
    marquee::intersecting(scene, world)
}

/// 卡片角标中心：节点屏幕右上角再往外挪，保证不压节点文字。
fn card_badge_center(top_right: egui::Pos2, radius: f32) -> egui::Pos2 {
    top_right + Vec2::new(radius + 3.0, -(radius + 3.0))
}

/// 概念图落点重叠的判定容差：贴边不算重叠（`docs/30` §5.5 的「重叠」是实打实压住）。
const OVERLAP_EPS: f32 = 0.5;
/// 重叠告警描边亮多久（秒）。
pub(crate) const OVERLAP_FLASH: f32 = 0.25;

/// 重叠告警边是否还亮着：`started` 起亮时刻、`now` 当前时刻（都是 egui 秒）。
///
/// 老化只能读注入的时钟，不能读 `Instant::now()`——金样按固定帧数截帧，
/// 墙钟会让同一份脚本在两遍跑出不同的画面（M16 口径）。
fn overlap_flash_live(started: f64, now: f64, window: f64) -> bool {
    now - started < window
}

/// 告警边的存活窗口（秒）：缺省 [`OVERLAP_FLASH`]。
///
/// `MW_OVERLAP_FLASH_MS` 是**验收旋钮**，只给金样用：金样要等脚本跑完再空跑
/// `脚本帧数 + 4` 帧才截帧（`app_methods_1.rs` 的 `extend_warmup`），等到那一刻
/// 虚拟时钟已经走过 ~0.6s，0.25s 的窗口必然早就灭了——不拉长窗口，基线只能是
/// 「什么都没有」，那就不叫在拍告警边。真机不设这个变量，行为仍是 0.25s。
fn overlap_flash_window() -> f64 {
    std::env::var("MW_OVERLAP_FLASH_MS")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|ms| ms / 1000.0)
        .unwrap_or(OVERLAP_FLASH as f64)
}

/// 两个世界矩形是否真的压在一起。
fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.x + a.w > b.x + OVERLAP_EPS
        && b.x + b.w > a.x + OVERLAP_EPS
        && a.y + a.h > b.y + OVERLAP_EPS
        && b.y + b.h > a.y + OVERLAP_EPS
}

/// 四边锚点小圆点。屏幕空间恒定大小，所以画在 egui 层。
fn paint_anchors(
    ui: &egui::Ui,
    origin: egui::Rect,
    camera: Camera,
    scene: &Scene,
    theme: &Theme,
    viewport: [f32; 2],
    node: u32,
) {
    let r = scene.rect(node);
    if r.w <= 0.0 || r.h <= 0.0 {
        return;
    }
    let painter = ui.painter_at(origin);
    for side in mw_ui::Side::ALL {
        let p = mw_ui::link::anchor_point(r, side);
        let s = camera.world_to_screen(p, viewport);
        let c = Pos2::new(origin.min.x + s.x, origin.min.y + s.y);
        if !origin.contains(c) {
            continue;
        }
        // 先垫一圈底色，锚点压在连线上时才看得清。
        painter.circle_filled(c, ANCHOR_R + 1.5, with_alpha(theme.ui.canvas_bg, 210));
        painter.circle_filled(c, ANCHOR_R, with_alpha(theme.ui.accent, 190));
    }
}

/// 悬停工具条：两枚同尺寸圆角按钮，图标用线段画（不依赖字体，缩放/DPI 下都清晰）。
///
/// 底色取主题的 `panel`，只在悬停时换成 `accent_soft` + 强调色描边：画布上已经
/// 有卡片、锚点、连线，工具条再抢眼就变成噪音。它只是「节点旁边的一件小工具」。
fn paint_hover_bar(
    ui: &egui::Ui,
    theme: &Theme,
    bar: HoverBar,
    plus_hover: bool,
    more_hover: bool,
) {
    let u = &theme.ui;
    let painter = ui.painter();
    for (r, hover, more) in [(bar.plus, plus_hover, false), (bar.more, more_hover, true)] {
        let fill = if hover {
            with_alpha(u.accent_soft, 255)
        } else {
            with_alpha(u.panel, 252)
        };
        let border = if hover {
            with_alpha(u.accent, 170)
        } else {
            with_alpha(u.border, 255)
        };
        let ink = if hover {
            with_alpha(u.accent, 255)
        } else {
            with_alpha(u.dim, 255)
        };
        // 和面包屑/缩放控件同一套悬浮语法：一层极浅的投影把按钮从纸上托起来。
        painter.add(crate::canvas_chrome::float_shadow().as_shape(r, design::radius::SM));
        painter.rect_filled(r, egui::CornerRadius::same(design::radius::SM as u8), fill);
        painter.rect_stroke(
            r,
            egui::CornerRadius::same(design::radius::SM as u8),
            Stroke::new(design::stroke::HAIRLINE, border),
            egui::StrokeKind::Inside,
        );
        // 「＋ / ⋯」与菜单、工具条共用同一套矢量几何：不随字体基线漂移。
        crate::icon_paint::paint_icon_sized(
            painter,
            r.center(),
            design::icon::SIZE,
            if more {
                crate::icons::Icon::More
            } else {
                crate::icons::Icon::Plus
            },
            ink,
        );
    }
    if plus_hover || more_hover {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
}

/// 拖出连线时的预览曲线 + 目标高亮。
///
/// 参数多是因为它要同时握「相机 + 视口 + 场景 + 主题」四件绘制上下文，
/// 再加被拖的那条线与「是不是概念图」两种模式；拆成结构体只是把 `>` 换成 `.`，
/// 所以这里显式放行。
#[allow(clippy::too_many_arguments)]
fn paint_link(
    ui: &egui::Ui,
    origin: egui::Rect,
    camera: Camera,
    scene: &Scene,
    theme: &Theme,
    viewport: [f32; 2],
    l: &Link,
    concept: bool,
) {
    let from = mw_ui::link::anchor_point(scene.rect(l.from), l.side);
    let a = camera.world_to_screen(from, viewport);
    let b = camera.world_to_screen(l.at, viewport);
    let p0 = Pos2::new(origin.min.x + a.x, origin.min.y + a.y);
    let p3 = Pos2::new(origin.min.x + b.x, origin.min.y + b.y);
    // 起点按锚点法线出一段控制柄，终点再镜像一个：拉出来是一条顺滑的 S 形，
    // 而不是一根贴着节点边缘的斜线。
    let k = 44.0;
    let (dx, dy) = match l.side {
        mw_ui::Side::Left => (-k, 0.0),
        mw_ui::Side::Right => (k, 0.0),
        mw_ui::Side::Top => (0.0, -k),
        mw_ui::Side::Bottom => (0.0, k),
    };
    let p1 = Pos2::new(p0.x + dx, p0.y + dy);
    let p2 = Pos2::new(p3.x - dx, p3.y - dy);
    let accent = color32(theme.ui.accent);
    let painter = ui.painter_at(origin);
    if let Some(to) = l.to {
        let r = scene.rect(to);
        let s0 = camera.world_to_screen(Point::new(r.x, r.y), viewport);
        let s1 = camera.world_to_screen(Point::new(r.right(), r.bottom()), viewport);
        let box_ = egui::Rect::from_min_max(
            Pos2::new(origin.min.x + s0.x, origin.min.y + s0.y),
            Pos2::new(origin.min.x + s1.x, origin.min.y + s1.y),
        );
        painter.rect_stroke(
            box_.expand(2.0),
            egui::CornerRadius::same(6),
            egui::Stroke::new(2.0, accent),
            egui::StrokeKind::Inside,
        );
    }
    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
        [p0, p1, p2, p3],
        false,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.6, accent),
    ));
    painter.circle_filled(p3, 3.5, accent);
    // CM-10 ①：落空松手会**长出一个新概念**（CmapTools 的手势）。
    if l.to.is_none() && concept {
        paint_blank_concept_hint(&painter, p3, camera.zoom, accent, "松手新建概念");
    }
}

/// 插入关系线的预览：源主题描 2px 强调环，虚线从源盒中心跟到指针；
/// 指针压在合法目标上时，目标盒也描一圈（告诉用户「松手会连到它」）。
///
/// 指针出画布时整条不画（与框选同一取舍）：画到裁剪边会像「线断了」。
#[allow(clippy::too_many_arguments)]
fn paint_rel_link_preview(
    ui: &egui::Ui,
    origin: egui::Rect,
    camera: Camera,
    scene: &Scene,
    theme: &Theme,
    viewport: [f32; 2],
    from: u32,
    pointer: Pos2,
    target: Option<u32>,
) {
    let painter = ui.painter_at(origin);
    let accent = color32(theme.ui.accent);
    for n in std::iter::once(from).chain(target.filter(|t| *t != from)) {
        let r = scene.rect(n);
        let a = camera.world_to_screen(Point::new(r.x, r.y), viewport);
        let b = camera.world_to_screen(Point::new(r.right(), r.bottom()), viewport);
        let box_ = egui::Rect::from_min_max(
            Pos2::new(origin.min.x + a.x, origin.min.y + a.y),
            Pos2::new(origin.min.x + b.x, origin.min.y + b.y),
        );
        painter.rect_stroke(
            box_.expand(2.0),
            egui::CornerRadius::same(6),
            Stroke::new(2.0, accent),
            egui::StrokeKind::Inside,
        );
    }
    let c = scene.rect(from).center();
    let s = camera.world_to_screen(c, viewport);
    let p0 = Pos2::new(origin.min.x + s.x, origin.min.y + s.y);
    painter.extend(egui::Shape::dashed_line(
        &[p0, pointer],
        Stroke::new(1.6, accent),
        5.0,
        4.0,
    ));
    painter.circle_filled(pointer, 3.5, accent);
}

/// 「松手会在这里长出一个概念」的占位框（CM-10 ①）。
///
/// 96×36 是概念盒的最小尺寸；**左上角对齐指针**——pos 的语义就是盒子左上角
/// （mw-core 的 floating_topic_honours_its_stored_position 断言 rect.x == pos.x），
/// 框要是按中心画，就等于向用户承诺了一个偏半个盒子的位置。
///
/// 两处共用：从概念拉新线（paint_link）、把已有线改到空白（paint_relink_preview）。
pub(crate) fn paint_blank_concept_hint(
    painter: &egui::Painter,
    at: Pos2,
    zoom: f32,
    accent: egui::Color32,
    hint: &str,
) {
    const CONCEPT_MIN_W: f32 = 96.0;
    const CONCEPT_MIN_H: f32 = 36.0;
    {
        let (w, h) = (CONCEPT_MIN_W * zoom, CONCEPT_MIN_H * zoom);
        // 对齐口径必须和 `AddFloatingTopic` / `AddLinkedConcept` 一致：`pos` 是盒子
        // **左上角**（`mw-core` `floating_topic_honours_its_stored_position` 断言
        // `rect.x == pos.x`）。框如果以指针为中心画，松手后概念会往右下偏半个盒子。
        let box_ = egui::Rect::from_min_size(at, egui::Vec2::new(w, h));
        painter.rect_filled(box_, 8.0, accent.gamma_multiply(0.10));
        painter.rect_stroke(
            box_,
            8.0,
            egui::Stroke::new(1.5, accent.gamma_multiply(0.65)),
            egui::StrokeKind::Middle,
        );
        painter.text(
            box_.center_bottom() + egui::Vec2::new(0.0, 14.0),
            egui::Align2::CENTER_TOP,
            hint,
            // 字号走令牌：字面量会被 `chrome` 的字号守卫测试判为违规（它拦的就是这种）。
            egui::FontId::proportional(design::content::META.size),
            accent.gamma_multiply(0.85),
        );
    }
}

/// 三分区落点指示（docs/06 §5：上缘蓝条 = 插到它之前、整框高亮 = 成为子级、
/// 下缘蓝条 = 插到它之后；非法目标整块转红）。
///
/// 画在 egui 层而不是合成器里：条宽 3px、描边 2px 都是**屏幕空间常量**，
/// 走 egui 天然不会随缩放变粗，也不用为几个矩形加一套实例。
fn paint_drop_target(
    ui: &egui::Ui,
    origin: egui::Rect,
    camera: Camera,
    scene: &Scene,
    theme: &Theme,
    viewport: [f32; 2],
    dt: mw_ui::DropTarget,
) {
    let Some(n) = scene.nodes.get(dt.node as usize) else {
        return;
    };
    let r = n.geom.rect;
    let a = camera.world_to_screen(Point::new(r.x, r.y), viewport);
    let b = camera.world_to_screen(Point::new(r.right(), r.bottom()), viewport);
    let target = egui::Rect::from_min_max(
        Pos2::new(origin.min.x + a.x, origin.min.y + a.y),
        Pos2::new(origin.min.x + b.x, origin.min.y + b.y),
    );
    if target.width() < 1.0 || target.height() < 1.0 {
        return;
    }
    let src = if dt.illegal {
        mw_render::DROP_BAD
    } else {
        theme.ui.accent
    };
    let painter = ui.painter_at(origin);
    let line = with_alpha(src, 255);
    match dt.zone {
        mw_ui::Zone::Child => {
            painter.rect_filled(
                target.expand(1.0),
                egui::CornerRadius::same(6),
                with_alpha(src, 30),
            );
            painter.rect_stroke(
                target.expand(1.0),
                egui::CornerRadius::same(6),
                egui::Stroke::new(2.0, line),
                egui::StrokeKind::Inside,
            );
        }
        mw_ui::Zone::Before | mw_ui::Zone::After => {
            // 条子压在半边外侧：正好压在节点边缘上会被节点描边吃掉。
            let y = if dt.zone == mw_ui::Zone::Before {
                target.top() - 2.5
            } else {
                target.bottom() + 2.5
            };
            let bar = egui::Rect::from_min_max(
                Pos2::new(target.left() - 3.0, y - 1.5),
                Pos2::new(target.right() + 3.0, y + 1.5),
            );
            painter.rect_filled(bar, egui::CornerRadius::same(2), line);
        }
    }
}

/// 60ms 吸附延迟的判定。
///
/// 抽成纯函数只为一件事：这段逻辑的可见效果是「拖过去时不要立刻粘住」，
/// 而 60ms 内的两帧既截不到图也数不出来——只能靠断言。
fn snap_ready(since: Option<f64>, now: f64) -> bool {
    since.is_some_and(|t| now - t >= SNAP_DELAY)
}

/// 矩形按世界单位平移（吸附与幽灵共用同一套算法）。
fn shift_rect(r: mw_core::Rect, dx: f32, dy: f32) -> mw_core::Rect {
    mw_core::Rect::new(r.x + dx, r.y + dy, r.w, r.h)
}

/// 面板开合补偿：画布中心在窗口里移动了 `shift`（逻辑 px），相机中心要同向移动的
/// **世界**距离。推导：`屏幕 = (世界 - 相机中心) * zoom + 画布中心`，要让世界点的
/// 屏幕位置不变，相机中心就得跟着画布中心同向走 `shift / zoom`。
///
/// 小于 1/10 px 的抖动直接忽略：egui 的面板动画每帧给的宽度是浮点，末帧会剩零点几
/// 像素，累计起来会让相机慢慢漂走。缩放非法（0/NaN）时也不动，宁可画面跳一帧。
fn camera_shift_for_viewport(shift: Vec2, zoom: f32) -> (f32, f32) {
    // 阈值取 1/1000 px：面板动画有十几帧，若按 0.1px 丢弃，来回开合会累计出 1px 漂移。
    if !(zoom.is_finite() && zoom > 0.0) || shift.length_sq() < 1e-6 {
        return (0.0, 0.0);
    }
    (shift.x / zoom, shift.y / zoom)
}

/// 方向键导航：在 `(dx, dy)` 方向（其中一个为 ±1）上找最近的节点。
///
/// 评分 = 主轴距离 + 1.8 × 侧向偏移；侧向偏移超过主轴 1.6 倍的直接排除，
/// 免得「按右键跳到斜上方很远的节点」。这条规则只用几何，不看树结构——
/// 思维导图左右分叉时，结构上的「兄弟」在屏幕上可能根本不在同一侧。
fn directional_neighbor(scene: &Scene, from: u32, dx: f32, dy: f32) -> Option<u32> {
    let a = scene.rect(from);
    let (ax, ay) = (a.x + a.w * 0.5, a.y + a.h * 0.5);
    let mut best: Option<(f32, u32)> = None;
    for i in 0..scene.nodes.len() as u32 {
        if i == from {
            continue;
        }
        let b = scene.rect(i);
        let (bx, by) = (b.x + b.w * 0.5, b.y + b.h * 0.5);
        let (along, across) = if dx != 0.0 {
            ((bx - ax) * dx, (by - ay).abs())
        } else {
            ((by - ay) * dy, (bx - ax).abs())
        };
        if along <= 1.0 || across > along * 1.6 {
            continue;
        }
        let score = along + across * 1.8;
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, i));
        }
    }
    best.map(|(_, i)| i)
}

fn with_alpha(c: Rgba, a: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], a)
}

/// 虚线框 + 半透明填充（docs/06:80）。画在 egui 层，不进合成器。
fn paint_marquee(ui: &egui::Ui, clip: egui::Rect, theme: &Theme, a: Pos2, b: Pos2) {
    let r = egui::Rect::from_two_pos(a, b).intersect(clip);
    if r.width() < 1.0 || r.height() < 1.0 {
        return;
    }
    let accent = theme.ui.accent;
    let painter = ui.painter_at(clip);
    painter.rect_filled(
        r,
        egui::CornerRadius::ZERO,
        color32(Rgba::rgba(accent.0[0], accent.0[1], accent.0[2], 26)),
    );
    let ring = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    painter.extend(egui::Shape::dashed_line(
        &ring,
        egui::Stroke::new(1.0, color32(accent)),
        4.0,
        3.0,
    ));
}

/// 旧选中集合（按 id）落到重排后的新场景上。
///
/// 能找回的按原顺序保留（末尾仍是主选），找不回的丢掉；**一个都找不回**时回落到根
/// ——「撤销把选中节点删掉」不能让选择悬空，但也不能无缘无故清空。
fn restore_selection(old_ids: &[String], scene: &Scene, root: &str) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(old_ids.len());
    for id in old_ids {
        if let Some(i) = scene.find(id) {
            if !out.contains(&i) {
                out.push(i);
            }
        }
    }
    if out.is_empty() && !old_ids.is_empty() {
        if let Some(i) = scene.find(root) {
            out.push(i);
        }
    }
    out
}

/// 搜索跳转前把挡路的折叠祖先展开成一组命令（**根 → 近**）。
///
/// 检索遍历的是文档前序，折叠子树里的节点照样是命中；可画布只排可见节点。
/// 展开必须走既有的 `SetCollapsed` 并包成一条 `Command::Batch`，
/// 撤销栈里就只多一条，`Ctrl+Z` 一次收回全部祖先。
pub(crate) fn reveal_commands(doc: &mw_core::Document, id: &str) -> Vec<Command> {
    let mut ids = doc.collapsed_ancestors(id);
    ids.reverse();
    ids.into_iter()
        .map(|id| Command::SetCollapsed {
            id,
            collapsed: false,
        })
        .collect()
}

/// 跳转目标在聚焦视图之外、要退几层才能排出来。
///
/// 聚焦是用户显式进入的视图态，但**点搜索结果也是显式动作**：目标不在当前
/// 聚焦子树、又确实存在于文档里时，跳转先退出全部聚焦（`focus_to(0)`），
/// 再走既有的展开/选中。返回 `Some(levels)` 表示要退出这么多层；
/// `None` 表示不用退（没聚焦 / 目标本来就看得到 / 目标不在文档里）。
pub(crate) fn focus_exit_levels(
    doc: &mw_core::Document,
    focus: &[String],
    id: &str,
) -> Option<usize> {
    let root = focus.last()?;
    if doc.node(id).is_none() || in_layout_subtree(doc, root, id) {
        return None;
    }
    Some(focus.len())
}

/// `id` 是否落在布局根之下（**不看折叠**）。
///
/// 折叠之外的不可见是聚焦模式造成的：那种情况下把祖先展开也排不出来，
/// 只会白改一次文档、白占一条撤销。跳转前先问这里，再决定要不要动文档。
pub(crate) fn in_layout_subtree(doc: &mw_core::Document, root: &str, id: &str) -> bool {
    let mut seen = HashSet::new();
    let mut cur = Some(id);
    while let Some(cur_id) = cur {
        if cur_id == root {
            return true;
        }
        if !seen.insert(cur_id) {
            return false;
        }
        cur = doc.node(cur_id).and_then(|n| n.parent_id.as_deref());
    }
    false
}

/// 编辑器覆盖层这一帧落在屏幕的哪里：`rect` 是过渡中的插值盒，`None` 表示用场景终点。
struct EditorPlacement {
    viewport: [f32; 2],
    rect: Option<Rect>,
}

/// IME 组合状态只看本帧最后一个 IME 事件：预编辑进入组合，空预编辑或提交退出。
/// `None` 表示这一帧没有 IME 事件，调用方沿用上一帧状态。
pub(crate) fn ime_transition(events: &[Event]) -> Option<bool> {
    events.iter().rev().find_map(|e| match e {
        Event::Ime(egui::ImeEvent::Preedit { text, .. }) => Some(!text.is_empty()),
        Event::Ime(egui::ImeEvent::Commit(_)) => Some(false),
        _ => None,
    })
}

/// 编辑期回车/取消归不归画布：IME 组合中两个键都属于输入法。
fn edit_keys_owned(editing: bool, ime: bool) -> bool {
    editing && !ime
}

/// 编辑态按下的提交键，提交之后还要不要顺手建一个节点。
///
/// `docs/07` §3.2 的原话是「`Enter` 提交编辑 → 触发语义」「`Tab` 提交编辑 → 新建子节点」：
/// 建树时手不离键盘，一下一个。概念图没有「同级」这个结构意义（概念是浮的，顺序不决定
/// 层级），所以那边两个键都只提交——和 CmapTools 就地改名的手感一致。
fn commit_follow_up(enter: bool, tab: bool, concept_map: bool) -> Option<AddKind> {
    if concept_map {
        return None;
    }
    if enter {
        Some(AddKind::Sibling)
    } else if tab {
        Some(AddKind::Child)
    } else {
        None
    }
}

/// 行内编辑器覆盖层。返回本帧是否改了文本、是否因失焦需要提交。
fn editor_ui(
    ui: &mut egui::Ui,
    origin: Pos2,
    camera: Camera,
    scene: &Scene,
    theme: &Theme,
    ed: &mut Editor,
    place: EditorPlacement,
) -> EditorAction {
    let mut action = EditorAction::default();

    // IME 组合状态只在有 IME 事件的帧里更新；没有事件就沿用上一帧。
    let composing = ui.input(|i| ime_transition(&i.events));
    if let Some(v) = composing {
        ed.ime = v;
    }

    let Some(idx) = scene.find(&ed.id) else {
        action.lost_focus = true;
        return action;
    };
    let n = scene.node(idx);
    let zoom = camera.zoom;
    let r = place.rect.unwrap_or(n.geom.rect);
    let p0 = camera.world_to_screen(Point::new(r.x, r.y), place.viewport);
    let p1 = camera.world_to_screen(Point::new(r.x + r.w, r.y + r.h), place.viewport);
    let node_rect = egui::Rect::from_min_max(
        Pos2::new(origin.x + p0.x, origin.y + p0.y),
        Pos2::new(origin.x + p1.x, origin.y + p1.y),
    );
    if node_rect.width() < 2.0 || node_rect.height() < 2.0 {
        action.lost_focus = true;
        return action;
    }

    // 编辑态的强调环由 egui 画（合成器已对 editing 节点跳过自己的环）。
    // 环画在节点盒**外侧**并留空隙，与合成器的选中环共用几何常量：画在盒内会被
    // 根节点那种饱和填充吃掉，等于没有编辑态反馈。
    let ring_w = mw_render::SELECT_RING_W;
    let ring_off = mw_render::SELECT_RING_GAP + ring_w * 0.5;
    let ring_rect = node_rect.expand(ring_off);
    let ring_stroke = egui::Stroke::new(ring_w, color32(theme.ui.accent));
    if n.style.shape == Shape::Ellipse {
        // 椭圆节点的环必须也是椭圆：用圆角矩形画会在四个角上和盒形错开一截，
        // 看着像「环没套住节点」。
        ui.painter()
            .add(egui::Shape::Ellipse(egui::epaint::EllipseShape {
                center: ring_rect.center(),
                radius: egui::vec2(ring_rect.width() * 0.5, ring_rect.height() * 0.5),
                fill: egui::Color32::TRANSPARENT,
                stroke: ring_stroke,
                angle: 0.0,
            }));
    } else {
        // 半径走**同一个出口** `mw_render::corner_style`（SE-13 复审修的）：以前这里自己
        // 又 match 了一遍 shape，于是 `radius` 覆盖后节点是圆角、环还是直角；`radius=0`
        // 的圆角矩形则反过来。盒子已经缩放过，半径按 `zoom` 换算回屏幕空间。
        let base = mw_render::corner_style(&n.style, r);
        let radius = if base <= mw_render::ELLIPSE_CODE + 0.5 {
            0.0 // 上面已拦掉椭圆，这里只留一个不会 panic 的兜底值。
        } else {
            (base * zoom).min(ring_rect.height() * 0.5)
        };
        ui.painter().rect_stroke(
            ring_rect,
            radius + ring_off,
            ring_stroke,
            egui::StrokeKind::Middle,
        );
    }

    // 字族按**字重**选，不再看 `geom.bold` 二值：`bold` 只是 600 的别名，而新控件能把
    // 字重设成 500/400。否则把 600 改成 400（或反过来）一按 F2 文字就跳字重。
    let weight = n.style.weight;
    let family = if weight >= mw_core::theme::WEIGHT_SEMIBOLD {
        FontFamily::Name(crate::fonts::BOLD.into())
    } else {
        FontFamily::Proportional
    };
    // 对齐也跟着节点走：`align=right` 的节点一进编辑就跳到左边同样是「两段字」。
    let h_align = match n.style.align {
        Some(mw_core::theme::TextAlign::Center) => egui::Align::Center,
        Some(mw_core::theme::TextAlign::Right) => egui::Align::RIGHT,
        _ => egui::Align::LEFT,
    };
    let font_size = (n.geom.font_size * zoom).max(6.0);
    let font_id = FontId::new(font_size, family);
    let ink = color32(n.style.text);
    let (pad_x, pad_y) = mw_core::design::card_padding(n.geom.font_size);
    let inner_w = (r.w - n.text_x - pad_x).max(8.0) * zoom;
    let rows = ed.text.split('\n').count().max(1);
    let lines = n.geom.lines.len().max(1);
    // 行高锁到画布那一份（`字号 × 1.45`）。egui 默认按雅黑自身的行高（约 1.17 倍）排行，
    // 不锁住的话多行节点一按 F2 文字就**收紧**——编辑器与画布必须像同一段字。
    let line_h = (n.geom.line_height * zoom).max(font_size);
    let mut layouter = |ui: &egui::Ui, buf: &dyn TextBuffer, wrap_width: f32| {
        edit_galley(ui.ctx(), buf.as_str(), &font_id, ink, wrap_width, line_h)
    };

    // 首行基线对齐：画布那一行由 `line_baseline` 定（过渡中的盒高用 `r`），再减去 egui
    // 「行顶→基线」的距离。后者随字号与字体回退变，算不准，只能用同一条排版路径量一次。
    let probe = edit_galley(
        ui.ctx(),
        BASELINE_PROBE,
        &font_id,
        ink,
        f32::INFINITY,
        line_h,
    );
    let dy = first_base_offset(r.h, n.geom.label_h, n.geom.line_height, lines, rows) * zoom
        - first_baseline(&probe, line_h);
    let dx = n.text_x * zoom;
    // `Margin` 存 i8：整像素走内边距，剩下不到 1pt 的小数压在矩形位移上。
    // 注意 `.margin()` 在同时给了 `.frame()` 时会被 egui 丢掉，内边距必须挂在 frame 上。
    let margin = egui::Margin {
        left: dx.floor().clamp(0.0, 127.0) as i8,
        right: (pad_x * zoom).round().clamp(0.0, 127.0) as i8,
        top: dy.floor().clamp(0.0, 127.0) as i8,
        bottom: (pad_y * zoom).round().clamp(0.0, 127.0) as i8,
    };
    let edit_rect = node_rect.translate(Vec2::new(dx - margin.left as f32, dy - margin.top as f32));

    // caret 和输入法下划线跟着节点字色走：根节点那种饱和底色上，全局深色 caret 等于隐形，
    // 而中文输入时那根下划线是「IME 收到键了没」的唯一反馈。
    let saved_cursor = ui.visuals().text_cursor.stroke;
    let saved_ime_on = ui.visuals().ime_composition.active_underline_stroke;
    let saved_ime_off = ui.visuals().ime_composition.inactive_underline_stroke;
    let saved_selection = ui.visuals().selection;
    {
        let v = ui.visuals_mut();
        v.text_cursor.stroke = Stroke::new(1.5, ink);
        v.ime_composition.active_underline_stroke = Stroke::new(1.5, ink);
        v.ime_composition.inactive_underline_stroke = Stroke::new(1.5, ink.linear_multiply(0.5));
        v.selection.bg_fill = ink.linear_multiply(0.25);
        v.selection.stroke = Stroke::new(1.0, ink);
    }

    let edit_id = node_edit_id(&ed.id);
    let response = ui.put(
        edit_rect,
        egui::TextEdit::multiline(&mut ed.text)
            .id(edit_id)
            .font(font_id.clone())
            .text_color(ink)
            .frame(egui::Frame::NONE.inner_margin(margin))
            .layouter(&mut layouter)
            .align(egui::Align2::new(h_align, egui::Align::TOP))
            .desired_width(inner_w)
            .desired_rows(rows)
            .char_limit(MAX_TEXT_LEN),
    );

    let v = ui.visuals_mut();
    v.text_cursor.stroke = saved_cursor;
    v.ime_composition.active_underline_stroke = saved_ime_on;
    v.ime_composition.inactive_underline_stroke = saved_ime_off;
    v.selection = saved_selection;

    if !ed.focused {
        response.request_focus();
        ed.focused = true;
        // 进入编辑时光标落在末尾；双击不按点击位置拆字（M2a 的取舍，见 docs/13 §4.12）。
        let mut state =
            egui::widgets::text_edit::TextEditState::load(ui.ctx(), edit_id).unwrap_or_default();
        let end = ed.text.chars().count();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(end),
            )));
        state.store(ui.ctx(), edit_id);
    }

    action.changed = response.changed();
    action.lost_focus = ed.focused && response.lost_focus();
    action.rect = Some(response.rect);
    if response.has_focus() {
        ui.ctx().request_repaint();
    }
    action
}

fn color32(c: Rgba) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

/// 量基线用的探针字：中西文在雅黑里同一个 face，取一个能代表正文的字形就够。
const BASELINE_PROBE: &str = "中";

/// 编辑器排版：字号、字色、**行高**全部来自节点几何，与画布同一套。
///
/// 首行基线的探针也走这个函数——两条路径必须同源，否则量到的和画出来的对不上。
fn edit_galley(
    ctx: &egui::Context,
    text: &str,
    font_id: &FontId,
    color: Color32,
    wrap_width: f32,
    line_h: f32,
) -> Arc<Galley> {
    let mut job =
        egui::text::LayoutJob::simple(text.to_owned(), font_id.clone(), color, wrap_width);
    // 行尾空格要留着：吞掉它会让 caret 看起来跳到上一个字前面。
    job.keep_trailing_whitespace = true;
    for section in &mut job.sections {
        section.format.line_height = Some(line_h);
    }
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// 首行基线相对节点盒顶的高度（世界单位）。
///
/// 打字时编辑器的行数可能暂时少于几何（场景下一帧才重排），差额按居中补回去；行数多出
/// 来的部分只能往下长——两边都不能让已经落好的文字跳位。
fn first_base_offset(box_h: f32, label_h: f32, line_h: f32, lines: usize, rows: usize) -> f32 {
    line_baseline(box_h, label_h, lines, line_h, 0)
        + lines.saturating_sub(rows) as f32 * line_h * 0.5
}

/// `galley 顶 → 首行基线`。egui 会把显式行高上下均分，这个数随字号与字体回退变，只能量。
fn first_baseline(galley: &Galley, line_h: f32) -> f32 {
    galley
        .rows
        .first()
        .and_then(|row| row.row.glyphs.first().map(|g| row.pos.y + g.pos.y))
        .unwrap_or(line_h * mw_core::layout::BASELINE_RATIO)
}

/// 复制/剪切的顶层节点：去掉「祖先也在选区里」的重复项，保持文档前序。
///
/// 不这么剪枝，框选父节点和它的孩子再复制，孩子会被粘贴两遍。
fn selected_roots(doc: &mw_core::Document, ids: &[String]) -> Vec<String> {
    let set: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut roots = Vec::new();
    for id in doc.doc_order() {
        if !set.contains(id.as_str()) {
            continue;
        }
        let mut covered = false;
        let mut at = doc.node(&id).and_then(|n| n.parent_id.as_deref());
        while let Some(parent) = at {
            if set.contains(parent) {
                covered = true;
                break;
            }
            at = doc.node(parent).and_then(|n| n.parent_id.as_deref());
        }
        if !covered {
            roots.push(id);
        }
    }
    roots
}

/// 概念图复制：只取选中的**概念本身**，不取子树；关系线只保留两端都在选中集内的。
///
/// 概念图的边是命题网，不是父子边。把它交给 `copy_subtrees` 会连 `children` 一起带走，
/// 粘贴时再经 `InsertSubtree` 造出父子关系——那正是 `docs/30` §5.2 禁止的树语义。
fn concept_snapshot(doc: &mw_core::Document, roots: &[String]) -> Vec<Node> {
    let picked: HashSet<&str> = roots.iter().map(String::as_str).collect();
    let mut out = Vec::with_capacity(roots.len());
    for id in roots {
        let Some(node) = doc.node(id) else { continue };
        let mut copy = node.clone();
        copy.parent_id = None;
        copy.children.clear();
        copy.boundaries.clear();
        copy.summaries.clear();
        // 卡是 1:1 绑定，副本不能抢原件那张卡；留成未绑定比落盘后判脏好。
        copy.card_id = None;
        copy.relationships
            .retain(|r| r.to != copy.id && picked.contains(r.to.as_str()));
        out.push(copy);
    }
    out
}

/// 把一批概念复制成新的独立概念：换新 id、保留内部关系、整体偏移，返回一条命令。
///
/// `fallback` 只服务「从思维导图复制、粘到概念图」这种没有 `pos` 的来源；
/// 概念图自己的节点都带坐标，永远走 `pos + offset`。
fn concept_clone_command(
    nodes: &[Node],
    offset: (f32, f32),
    fallback: (f32, f32),
) -> Option<(Command, Vec<String>)> {
    if nodes.is_empty() {
        return None;
    }
    let ids: HashMap<String, String> = nodes.iter().map(|n| (n.id.clone(), new_id())).collect();
    let mut out = Vec::with_capacity(nodes.len());
    for (i, node) in nodes.iter().enumerate() {
        let id = ids.get(&node.id)?.clone();
        let mut copy = node.clone();
        copy.id = id.clone();
        copy.parent_id = None;
        copy.children.clear();
        copy.boundaries.clear();
        copy.summaries.clear();
        copy.card_id = None;
        let base = node.pos.as_ref().map(|p| (p.x, p.y)).unwrap_or_else(|| {
            // 没有坐标的来源按 4 列排开，至少不会全叠在原点。
            (
                fallback.0 + (i % 4) as f32 * 180.0,
                fallback.1 + (i / 4) as f32 * 80.0,
            )
        });
        copy.pos = Some(mw_core::model::Position {
            x: base.0 + offset.0,
            y: base.1 + offset.1,
        });
        copy.relationships.retain_mut(|r| {
            let Some(to) = ids.get(&r.to) else {
                return false;
            };
            r.id = new_id();
            r.to = to.clone();
            true
        });
        out.push(copy);
    }
    let tops: Vec<String> = out.iter().map(|n| n.id.clone()).collect();
    Some((Command::InsertConcepts { nodes: out }, tops))
}

/// 前序快照若干子树（含祖先隐藏的后代）。
fn copy_subtrees(doc: &mw_core::Document, roots: &[String]) -> Vec<Node> {
    let mut out = Vec::new();
    for root in roots {
        for id in doc.subtree_ids(root) {
            if let Some(node) = doc.node(&id) {
                out.push(node.clone());
            }
        }
    }
    out
}

/// 把快照重挂到 `parent` 下：节点/关系/边界/概要全部换新 ID，并清掉自由坐标。
///
/// 关系线只保留两端都在这批节点里的；指向原文档其它节点的外部关系在粘贴后会悬空，
/// 这里直接丢弃（`InsertSubtree` 也会再剪一次，双保险）。
fn remap_subtrees(nodes: &[Node], parent: &str) -> (Vec<Node>, Vec<String>) {
    if nodes.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let ids: HashMap<&str, String> = nodes.iter().map(|n| (n.id.as_str(), new_id())).collect();
    let mut out = Vec::with_capacity(nodes.len());
    let mut tops = Vec::new();
    for node in nodes {
        let is_top = match node.parent_id.as_deref() {
            Some(p) => !ids.contains_key(p),
            None => true,
        };
        let mut copy = node.clone();
        copy.id = ids[node.id.as_str()].clone();
        copy.parent_id = if is_top {
            Some(parent.to_string())
        } else {
            node.parent_id.as_deref().and_then(|p| ids.get(p).cloned())
        };
        copy.children = node
            .children
            .iter()
            .filter_map(|c| ids.get(c.as_str()).cloned())
            .collect();
        copy.relationships.retain_mut(|r| {
            let Some(to) = ids.get(r.to.as_str()) else {
                return false;
            };
            r.id = new_id();
            r.to = to.clone();
            true
        });
        let child_set: HashSet<String> = copy.children.iter().cloned().collect();
        copy.boundaries.retain_mut(|b| {
            b.id = new_id();
            b.targets = b
                .targets
                .iter()
                .filter_map(|t| ids.get(t.as_str()).cloned())
                .filter(|t| child_set.contains(t))
                .collect();
            !b.targets.is_empty()
        });
        copy.summaries.retain_mut(|s| {
            s.id = new_id();
            s.targets = s
                .targets
                .iter()
                .filter_map(|t| ids.get(t.as_str()).cloned())
                .filter(|t| child_set.contains(t))
                .collect();
            !s.targets.is_empty()
        });
        copy.pos = None;
        copy.modified_at = now_ms();
        if is_top {
            tops.push(copy.id.clone());
        }
        out.push(copy);
    }
    (out, tops)
}

/// 粘贴命令：目标折叠时先展开（否则粘完看不见），整批一次撤销。
fn paste_command(
    doc: &mw_core::Document,
    clipboard: &[Node],
    parent: &str,
) -> Option<(Command, Vec<String>)> {
    let (nodes, tops) = remap_subtrees(clipboard, parent);
    if nodes.is_empty() {
        return None;
    }
    let mut commands = Vec::new();
    if doc.node(parent).is_some_and(|n| n.collapsed) {
        commands.push(Command::SetCollapsed {
            id: parent.to_string(),
            collapsed: false,
        });
    }
    commands.push(Command::InsertSubtree {
        parent: parent.to_string(),
        index: None,
        nodes,
    });
    let command = if commands.len() > 1 {
        Command::Batch { commands }
    } else {
        commands.pop().expect("至少一条命令")
    };
    Some((command, tops))
}

/// Ctrl+D 的命令：每个复制根紧跟原节点插入；同一父级下的多个根要补偿前面的插入位移。
fn duplicate_command(doc: &mw_core::Document, roots: &[String]) -> Option<(Command, Vec<String>)> {
    let mut commands = Vec::new();
    let mut tops = Vec::new();
    let mut inserted: HashMap<String, usize> = HashMap::new();
    for root in roots {
        let Some(node) = doc.node(root) else {
            continue;
        };
        let parent = node
            .parent_id
            .clone()
            .unwrap_or_else(|| doc.root_id.clone());
        let pos = doc
            .node(&parent)
            .and_then(|p| p.children.iter().position(|c| c == root));
        let snapshot = copy_subtrees(doc, std::slice::from_ref(root));
        let (nodes, new_tops) = remap_subtrees(&snapshot, &parent);
        if nodes.is_empty() {
            continue;
        }
        let count = inserted.entry(parent.clone()).or_insert(0);
        // 前面的副本已经把下标往后推了 `count` 格；没有父级（主根/自由主题）就追加。
        let index = pos.map(|p| p + 1 + *count);
        commands.push(Command::InsertSubtree {
            parent,
            index,
            nodes,
        });
        *count += 1;
        tops.extend(new_tops);
    }
    if commands.is_empty() {
        return None;
    }
    Some((Command::Batch { commands }, tops))
}

/// 多选「移入同一父级」：以主选的父级为目标，把散落的选中根追加到末尾。
///
/// 已经在目标下的节点不动，避免无意义的撤销记录；主选自己永远不动（它定义目标）。
fn same_parent_command(doc: &mw_core::Document, ids: &[String], primary: &str) -> Option<Command> {
    let target = doc.node(primary)?.parent_id.clone()?;
    let mut commands = Vec::new();
    for id in selected_roots(doc, ids) {
        if id == target {
            continue;
        }
        let already = doc.node(&id).and_then(|n| n.parent_id.as_deref()) == Some(target.as_str());
        if already {
            continue;
        }
        commands.push(Command::MoveNode {
            id,
            new_parent: target.clone(),
            index: None,
        });
    }
    if commands.is_empty() {
        return None;
    }
    Some(Command::Batch { commands })
}

/// 概要 / 边界的共同目标：以主选的父节点为容器，只取**同一父级**下的选中项，
/// 顺序按文档里的兄弟顺序（概要括号的左右端取决于这个顺序）。
///
/// 跨父级的选中会被丢掉——`AddSummary` / `AddBoundary` 只接受直接子节点，
/// 与其让整条命令失败，不如按「能圈的都圈上」处理；一个都没剩就返回 `None`。
fn annotation_targets(
    doc: &mw_core::Document,
    ids: &[String],
    primary: &str,
) -> Option<(String, Vec<String>)> {
    let parent = doc.node(primary)?.parent_id.clone()?;
    let picked: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
    let targets: Vec<String> = doc
        .node(&parent)?
        .children
        .iter()
        .filter(|c| picked.contains(c.as_str()))
        .cloned()
        .collect();
    if targets.is_empty() {
        None
    } else {
        Some((parent, targets))
    }
}

/// 松手到空白处的命令：树内节点**分离为自由主题**（保留落点），本身就是自由主题的
/// 只更新位置（`SetNodePos`）；主根不参与——它没有父级可脱离，也不该被拖走。
///
/// `dropped` 是 `(id, 当前世界矩形)`，`(dx, dy)` 是本次拖拽的世界位移。
/// 多选时合成一条 `Batch`，一次撤销就能全部退回。
fn detach_commands(
    doc: &mw_core::Document,
    dropped: &[(String, mw_core::Rect)],
    dx: f32,
    dy: f32,
) -> Option<Command> {
    let mut commands = Vec::new();
    for (id, rect) in dropped {
        let Some(node) = doc.node(id) else {
            continue;
        };
        let (x, y) = (rect.x + dx, rect.y + dy);
        if node.parent_id.is_some() {
            commands.push(Command::DetachNode {
                id: id.clone(),
                x,
                y,
            });
        } else if id != &doc.root_id {
            commands.push(Command::SetNodePos {
                id: id.clone(),
                x,
                y,
            });
        }
    }
    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(Command::Batch { commands }),
    }
}

/// 空白落点的完整判定：指针必须还在画布里，才允许把节点摘下来。
/// 把节点拖到面板上松手是一次放弃（XMind 同样不动结构），不该把节点从树上拔走。
fn detach_on_blank_command(
    doc: &mw_core::Document,
    dropped: &[(String, mw_core::Rect)],
    pointed_inside: bool,
    dx: f32,
    dy: f32,
) -> Option<Command> {
    if !pointed_inside {
        return None;
    }
    detach_commands(doc, dropped, dx, dy)
}

/// Ctrl+拖拽：把每棵被拖子树的**副本**插到落点，原件留在原地。
///
/// `roots` 与 `targets` 一一对应（`targets` 来自 `plan_moves`，顺序与 roots 相同）。
/// 同一个父级下插入多次时，后面的下标要补偿前面已经插进去的条数——
/// 否则多选复制会被反着摞起来，和 `duplicate_command` 是同一个道理。
fn copy_into_command(
    doc: &mw_core::Document,
    roots: &[String],
    targets: &[(String, Option<usize>)],
) -> Option<(Command, Vec<String>)> {
    let mut commands = Vec::new();
    let mut tops = Vec::new();
    let mut inserted: HashMap<String, usize> = HashMap::new();
    for (root, (parent, index)) in roots.iter().zip(targets) {
        let snapshot = copy_subtrees(doc, std::slice::from_ref(root));
        let (nodes, new_tops) = remap_subtrees(&snapshot, parent);
        if nodes.is_empty() {
            continue;
        }
        let count = inserted.entry(parent.clone()).or_insert(0);
        let index = index.map(|i| i + *count);
        commands.push(Command::InsertSubtree {
            parent: parent.clone(),
            index,
            nodes,
        });
        *count += 1;
        tops.extend(new_tops);
    }
    if commands.is_empty() {
        return None;
    }
    Some((Command::Batch { commands }, tops))
}

/// Ctrl+拖到空白：把子树**复制**成自由主题，落在拖到的位置。
///
/// 没有为它新增命令：先 `InsertSubtree` 挂到主根下、再 `DetachNode` 摘下来并给坐标，
/// 两条包成一条 `Batch`——撤销一次就全回去。这样命令层不必为「浮动子树」再开一条
/// 需要独立校验的路径（`InsertSubtree` 的剪枝/校验逻辑继续复用它自己那份）。
fn copy_floating_command(
    doc: &mw_core::Document,
    dropped: &[(String, mw_core::Rect)],
    dx: f32,
    dy: f32,
) -> Option<(Command, Vec<String>)> {
    let mut commands = Vec::new();
    let mut tops = Vec::new();
    for (id, rect) in dropped {
        let snapshot = copy_subtrees(doc, std::slice::from_ref(id));
        let (nodes, new_tops) = remap_subtrees(&snapshot, &doc.root_id);
        if nodes.is_empty() {
            continue;
        }
        commands.push(Command::InsertSubtree {
            parent: doc.root_id.clone(),
            index: None,
            nodes,
        });
        for top in &new_tops {
            commands.push(Command::DetachNode {
                id: top.clone(),
                x: rect.x + dx,
                y: rect.y + dy,
            });
            tops.push(top.clone());
        }
    }
    if tops.is_empty() {
        return None;
    }
    Some((Command::Batch { commands }, tops))
}

/// 节点路径（不含自己）：从主根一路走到父节点，用「›」连接。
///
/// 太长时**从左往右截断**并加省略号——路径靠后的部分（离自己最近的父级）更有用。
/// 拖拽时指针贴边该往哪边推（CSS 像素；0 = 不滚）。
///
/// 抽出来是为了能单测：滚动的**符号**搞反了，画布会朝反方向跑，
/// 而拖动时的截图很难分辨「跑反了」和「跑对了」。
fn autoscroll_step(cur: egui::Pos2, rect: egui::Rect, edge: f32, step: f32) -> Vec2 {
    let mut pan = Vec2::ZERO;
    if cur.x - rect.left() < edge {
        pan.x -= step;
    } else if rect.right() - cur.x < edge {
        pan.x += step;
    }
    if cur.y - rect.top() < edge {
        pan.y -= step;
    } else if rect.bottom() - cur.y < edge {
        pan.y += step;
    }
    pan
}

fn path_of(doc: &mw_core::Document, id: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut cur = doc.node(id).and_then(|n| n.parent_id.clone());
    let mut guard = 0;
    while let Some(pid) = cur {
        let Some(parent) = doc.node(&pid) else {
            break;
        };
        // 空标题的祖先是真实存在的一层，用占位符保住"在第几层"这个信息。
        parts.push(if parent.text.trim().is_empty() {
            "（未命名）"
        } else {
            parent.text.trim()
        });
        cur = parent.parent_id.clone();
        guard += 1;
        if guard > 4096 {
            break;
        }
    }
    parts.reverse();
    let joined = parts.join(" › ");
    const MAX: usize = 42;
    let chars: Vec<char> = joined.chars().collect();
    if chars.len() <= MAX {
        joined
    } else {
        let tail: String = chars[chars.len() - (MAX - 2)..].iter().collect();
        format!("… {tail}")
    }
}

/// 把一棵子树摘成一份**结构合法**的独立文档：根就是它自己，标题取根的文字。
///
/// 文本序列化器（Markdown / 大纲 / OPML / FreeMind）只认 `Document`，所以「只导选中子树」
/// 必须造一份临时文档，而不是给四个格式各加一个变体。三件事必须做干净，否则
/// `Document::validate` 会直接拒绝：
/// 1. 根的 `parent_id` 清空（新文档里它就是主根）；
/// 2. 所有节点的 `pos` 清掉（只有自由主题能带坐标，而这份文档里没有自由主题）；
/// 3. 指向子树外部的关系线剪掉（外部目标在这份文档里不存在）。
fn subtree_document(doc: &mw_core::Document, root: &str) -> Option<mw_core::Document> {
    let ids = doc.subtree_ids(root);
    if ids.is_empty() {
        return None;
    }
    let keep: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut nodes: HashMap<String, Node> = HashMap::with_capacity(ids.len());
    for id in &ids {
        let Some(node) = doc.node(id) else {
            continue;
        };
        let mut copy = node.clone();
        copy.pos = None;
        copy.relationships.retain(|r| keep.contains(r.to.as_str()));
        if id == root {
            copy.parent_id = None;
        }
        nodes.insert(id.clone(), copy);
    }
    let root_node = nodes.get(root)?;
    let mut meta = doc.meta.clone();
    meta.title = root_node.text.clone();
    mw_core::Document::from_parts(None, meta, root.to_string(), nodes).ok()
}

/// Alt+↑/↓ 的单步移动命令；`MoveNode` 的 index 以移动前的兄弟数组为参照。
fn move_command(doc: &mw_core::Document, id: &str, delta: i32) -> Option<Command> {
    let node = doc.node(id)?;
    let parent = node.parent_id.clone()?;
    let siblings = doc.node(&parent)?.children.clone();
    let pos = siblings.iter().position(|c| c == id)?;
    let index = if delta < 0 {
        if pos == 0 {
            return None;
        }
        pos - 1
    } else {
        if pos + 1 >= siblings.len() {
            return None;
        }
        // 目标位在原数组里是 pos+1；`MoveNode` 会在 want > old_pos 时再减一。
        pos + 2
    };
    Some(Command::MoveNode {
        id: id.to_string(),
        new_parent: parent,
        index: Some(index),
    })
}

/// 单颗主题无法再上移/下移的原因（M6b）。只读文档，提示文案与轨迹共用同一份判定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoveBlock {
    Root,
    First,
    Last,
    None,
}

pub(crate) fn move_block_reason(doc: &mw_core::Document, id: &str, delta: i32) -> MoveBlock {
    let Some(node) = doc.node(id) else {
        return MoveBlock::None;
    };
    let Some(parent) = node.parent_id.clone() else {
        return MoveBlock::Root;
    };
    let Some(siblings) = doc.node(&parent).map(|n| &n.children) else {
        return MoveBlock::None;
    };
    let Some(pos) = siblings.iter().position(|c| c == id) else {
        return MoveBlock::None;
    };
    if delta < 0 {
        if pos == 0 {
            MoveBlock::First
        } else {
            MoveBlock::None
        }
    } else if pos + 1 >= siblings.len() {
        MoveBlock::Last
    } else {
        MoveBlock::None
    }
}

/// 多选批量结构操作。顺序规则与单行版一致，但按整组规划：
/// - 缩进从下往上；升级从上往下，同一父级的兄弟按原顺序插到父级后面；
/// - 上移时若上一个兄弟也在选区，跟随组头走；下移反之——组内顺序不翻。
/// - 每条子命令都对着「已应用前几条」的副本计算，index 不会互相踩踏。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchOp {
    Indent,
    Outdent,
    MoveUp,
    MoveDown,
}

/// 规划批量移动时用的轻量树形：只保留 parent/children，镜像 `Command::MoveNode`
/// 的 index 语义。`Document` 不能克隆（带历史），而批量命令又必须按「前一条已生效」
/// 的状态计算下一条的 index——用这层影子树就够了。
struct PlanTree {
    root: String,
    parent: HashMap<String, Option<String>>,
    children: HashMap<String, Vec<String>>,
}

impl PlanTree {
    fn new(doc: &mw_core::Document) -> Self {
        let mut parent = HashMap::new();
        let mut children = HashMap::new();
        for id in doc.doc_order() {
            if let Some(n) = doc.node(&id) {
                parent.insert(id.clone(), n.parent_id.clone());
                children.insert(id.clone(), n.children.clone());
            }
        }
        Self {
            root: doc.root_id.clone(),
            parent,
            children,
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.parent.contains_key(id)
    }

    fn subtree_contains(&self, root: &str, target: &str) -> bool {
        let mut cur = Some(target);
        let mut guard = 0usize;
        while let Some(id) = cur {
            if id == root {
                return true;
            }
            guard += 1;
            if guard > 100_000 {
                return true;
            }
            cur = self.parent.get(id).and_then(|p| p.as_deref());
        }
        false
    }

    /// 与 `Document::apply(MoveNode)` 同一套下标语义；返回落位后的下标。
    fn move_node(&mut self, id: &str, new_parent: &str, index: Option<usize>) -> Option<usize> {
        if id == self.root || !self.contains(id) || !self.children.contains_key(new_parent) {
            return None;
        }
        if id == new_parent || self.subtree_contains(id, new_parent) {
            return None;
        }
        let old_parent = self.parent.get(id).cloned().flatten();
        let old_pos = old_parent.as_ref().and_then(|p| {
            self.children
                .get(p)
                .and_then(|v| v.iter().position(|c| c == id))
        });
        if let (Some(p), Some(pos)) = (&old_parent, old_pos) {
            if let Some(v) = self.children.get_mut(p) {
                v.remove(pos);
            }
        }
        let idx = if old_parent.as_deref() == Some(new_parent) {
            let pre_len = self.children.get(new_parent).map_or(0, Vec::len) + 1;
            let want = index.unwrap_or(pre_len).min(pre_len);
            match old_pos {
                Some(pos) if want > pos => want - 1,
                _ => want,
            }
        } else {
            let target_len = self.children.get(new_parent).map_or(0, Vec::len);
            index.unwrap_or(target_len).min(target_len)
        };
        let idx = idx.min(self.children.get(new_parent).map_or(0, Vec::len));
        if let Some(v) = self.children.get_mut(new_parent) {
            v.insert(idx, id.to_string());
        }
        self.parent
            .insert(id.to_string(), Some(new_parent.to_string()));
        Some(idx)
    }
}

fn batch_structure_command(
    doc: &mw_core::Document,
    ids: &[String],
    op: BatchOp,
) -> Option<Command> {
    let roots = selected_roots(doc, ids);
    if roots.is_empty() {
        return None;
    }
    let selected: HashSet<&str> = roots.iter().map(String::as_str).collect();
    let mut shape = PlanTree::new(doc);
    let mut flat: Vec<Command> = Vec::new();
    let order: Vec<String> = match op {
        BatchOp::Indent | BatchOp::MoveDown => roots.iter().rev().cloned().collect(),
        BatchOp::Outdent | BatchOp::MoveUp => roots.clone(),
    };
    let mut outdent_cursor: HashMap<String, usize> = HashMap::new();
    for id in order {
        let Some(parent) = shape.parent.get(&id).cloned().flatten() else {
            continue;
        };
        let Some(siblings) = shape.children.get(&parent).cloned() else {
            continue;
        };
        let Some(pos) = siblings.iter().position(|c| c == &id) else {
            continue;
        };
        let mut parts = Vec::new();
        match op {
            BatchOp::Indent => {
                if pos == 0 {
                    continue;
                }
                let prev = siblings[pos - 1].clone();
                if doc.node(&prev).is_some_and(|n| n.collapsed) {
                    parts.push(Command::SetCollapsed {
                        id: prev.clone(),
                        collapsed: false,
                    });
                }
                if shape.move_node(&id, &prev, None).is_none() {
                    continue;
                }
                parts.push(Command::MoveNode {
                    id,
                    new_parent: prev,
                    index: None,
                });
            }
            BatchOp::Outdent => {
                if parent == shape.root {
                    continue;
                }
                let Some(grand) = shape.parent.get(&parent).cloned().flatten() else {
                    continue;
                };
                let Some(pidx) = shape
                    .children
                    .get(&grand)
                    .and_then(|v| v.iter().position(|c| c == &parent))
                else {
                    continue;
                };
                let extra = outdent_cursor.entry(parent.clone()).or_insert(0);
                let index = pidx + 1 + *extra;
                if shape.move_node(&id, &grand, Some(index)).is_none() {
                    continue;
                }
                *extra += 1;
                parts.push(Command::MoveNode {
                    id,
                    new_parent: grand,
                    index: Some(index),
                });
            }
            BatchOp::MoveUp => {
                if pos == 0 || selected.contains(siblings[pos - 1].as_str()) {
                    continue;
                }
                let index = pos - 1;
                if shape.move_node(&id, &parent, Some(index)).is_none() {
                    continue;
                }
                parts.push(Command::MoveNode {
                    id,
                    new_parent: parent,
                    index: Some(index),
                });
            }
            BatchOp::MoveDown => {
                if pos + 1 >= siblings.len() || selected.contains(siblings[pos + 1].as_str()) {
                    continue;
                }
                let index = pos + 2;
                if shape.move_node(&id, &parent, Some(index)).is_none() {
                    continue;
                }
                parts.push(Command::MoveNode {
                    id,
                    new_parent: parent,
                    index: Some(index),
                });
            }
        }
        flat.extend(parts);
    }
    if flat.is_empty() {
        None
    } else {
        Some(Command::Batch { commands: flat })
    }
}

/// 多选拖拽：结构根按可见顺序挂到同一落点；每次插入后游标 +1，组内顺序不翻。
/// 落点在任一被拖子树里就整组拒绝（和单行拖拽同一条规则）。
fn move_many_command(
    doc: &mw_core::Document,
    ids: &[String],
    new_parent: &str,
    index: Option<usize>,
) -> Option<Command> {
    let roots = selected_roots(doc, ids);
    if roots.is_empty() || doc.node(new_parent).is_none() {
        return None;
    }
    let mut shape = PlanTree::new(doc);
    let mut flat = Vec::new();
    let mut cursor = index;
    for id in roots {
        let placed = shape.move_node(&id, new_parent, cursor)?;
        flat.push(Command::MoveNode {
            id,
            new_parent: new_parent.to_string(),
            index: cursor,
        });
        // 定向下标插入后，后续节点接在刚落下的那一个后面；追加（None）则继续追加。
        if cursor.is_some() {
            cursor = Some(placed + 1);
        }
    }
    Some(Command::Batch { commands: flat })
}

/// 纯文本粘贴：按行首缩进还原成树（Tab=4 空格），顶层挂到 `parent` 下。
/// 常见列表前缀（`- ` / `* ` / `+ ` / `1. `）会剥掉；空行跳过。
fn parse_plain_outline(text: &str, parent: &str) -> Option<Vec<Node>> {
    let mut rows: Vec<(usize, String)> = Vec::new();
    for line in text.lines() {
        let mut width = 0usize;
        for c in line.chars() {
            match c {
                ' ' => width += 1,
                '\t' => width += 4,
                _ => break,
            }
        }
        let rest = line.trim_start_matches([' ', '\t']);
        let content = strip_list_marker(rest).trim_end().to_string();
        if content.trim().is_empty() {
            continue;
        }
        rows.push((width, content));
    }
    if rows.is_empty() {
        return None;
    }
    let base = rows[0].0;
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for (width, content) in rows {
        let width = width.saturating_sub(base);
        while stack.last().is_some_and(|(w, _)| *w >= width) {
            stack.pop();
        }
        let id = new_id();
        let parent_id = stack
            .last()
            .map(|(_, idx)| nodes[*idx].id.clone())
            .unwrap_or_else(|| parent.to_string());
        let idx = nodes.len();
        nodes.push(Node::new(id.clone(), Some(parent_id), content));
        if let Some((_, parent_idx)) = stack.last() {
            nodes[*parent_idx].children.push(id);
        }
        stack.push((width, idx));
    }
    Some(nodes)
}

/// 列表前缀只剥常见的一层，正文里的 `-` 不动（`-5℃` 不是列表）。
fn strip_list_marker(line: &str) -> &str {
    if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return rest;
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let rest = &line[digits..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return r;
        }
    }
    line
}

/// Ctrl+Shift+←：插到父节点之后；根的孩子、自由主题都不能升级。
fn promote_command(doc: &mw_core::Document, id: &str) -> Option<Command> {
    let parent = doc.node(id)?.parent_id.clone()?;
    if parent == doc.root_id {
        return None;
    }
    let grand = doc.node(&parent)?.parent_id.clone()?;
    let pos = doc
        .node(&grand)?
        .children
        .iter()
        .position(|c| c == &parent)?;
    Some(Command::MoveNode {
        id: id.to_string(),
        new_parent: grand,
        index: Some(pos + 1),
    })
}

/// Ctrl+Shift+→：成为前一个兄弟的末子；没有前兄弟就没有落点。
fn demote_command(doc: &mw_core::Document, id: &str) -> Option<Command> {
    let parent = doc.node(id)?.parent_id.clone()?;
    let siblings = doc.node(&parent)?.children.clone();
    let pos = siblings.iter().position(|c| c == id)?;
    if pos == 0 {
        return None;
    }
    let prev = siblings[pos - 1].clone();
    let mut commands = Vec::new();
    if doc.node(&prev).is_some_and(|n| n.collapsed) {
        commands.push(Command::SetCollapsed {
            id: prev.clone(),
            collapsed: false,
        });
    }
    commands.push(Command::MoveNode {
        id: id.to_string(),
        new_parent: prev,
        index: None,
    });
    if commands.len() > 1 {
        Some(Command::Batch { commands })
    } else {
        Some(commands.pop().expect("至少一条命令"))
    }
}

/// 大纲行内 Enter（光标在中间）：拆成两个同级。
///
/// `SetText` 与 `AddNode` 装进一条 `Batch`：用户只按了一次回车，撤销就该一步复原。
/// 新节点文字直接随 `AddNode` 落地，避免「先建空行、再改文字」两条历史。
fn split_sibling_command(
    doc: &mw_core::Document,
    id: &str,
    head: &str,
    tail: &str,
) -> Option<Command> {
    let node = doc.node(id)?;
    let parent = node.parent_id.clone()?;
    let pos = doc.node(&parent)?.children.iter().position(|c| c == id)?;
    Some(Command::Batch {
        commands: vec![
            Command::SetText {
                id: id.to_string(),
                text: head.to_string(),
            },
            Command::AddNode {
                parent,
                index: Some(pos + 1),
                text: tail.to_string(),
                style: None,
            },
        ],
    })
}

/// 大纲行内 Ctrl+Enter（光标在中间）：前半段留在原节点，后半段成为它的子级。
fn split_child_command(
    doc: &mw_core::Document,
    id: &str,
    head: &str,
    tail: &str,
) -> Option<Command> {
    let collapsed = doc.node(id)?.collapsed;
    let mut commands = Vec::new();
    if collapsed {
        commands.push(Command::SetCollapsed {
            id: id.to_string(),
            collapsed: false,
        });
    }
    commands.push(Command::SetText {
        id: id.to_string(),
        text: head.to_string(),
    });
    commands.push(Command::AddNode {
        parent: id.to_string(),
        index: None,
        text: tail.to_string(),
        style: None,
    });
    Some(Command::Batch { commands })
}

/// 大纲行内 Backspace（光标在开头）：把当前文字并到上一同级末尾。
///
/// 被并入的节点若带着子节点，先把孩子过继给上一兄弟再删除——合并文字不能连带
/// 吞掉一整棵子树。返回 (命令, 上一兄弟 id, 光标应在的字符位)。
fn merge_prev_command(
    doc: &mw_core::Document,
    id: &str,
    text: &str,
) -> Option<(Command, String, usize)> {
    let node = doc.node(id)?;
    let parent = node.parent_id.clone()?;
    let siblings = doc.node(&parent)?.children.clone();
    let pos = siblings.iter().position(|c| c == id)?;
    if pos == 0 {
        return None;
    }
    let prev = siblings[pos - 1].clone();
    let prev_text = doc.node(&prev)?.text.clone();
    let mut commands = Vec::with_capacity(node.children.len() + 2);
    for child in node.children.clone() {
        commands.push(Command::MoveNode {
            id: child,
            new_parent: prev.clone(),
            index: None,
        });
    }
    commands.push(Command::SetText {
        id: prev.clone(),
        text: format!("{prev_text}{text}"),
    });
    commands.push(Command::DeleteNodes {
        ids: vec![id.to_string()],
    });
    Some((Command::Batch { commands }, prev, prev_text.chars().count()))
}

/// 场景里的节点是否有祖先在选区内（Ctrl+2 适应选中时把子树算进来）。
fn has_selected_ancestor(scene: &Scene, idx: u32, selected: &[u32]) -> bool {
    let mut at = scene.nodes.get(idx as usize).and_then(|n| n.parent);
    while let Some(parent) = at {
        if selected.contains(&parent) {
            return true;
        }
        at = scene.nodes.get(parent as usize).and_then(|n| n.parent);
    }
    false
}

#[cfg(test)]
mod tests;
