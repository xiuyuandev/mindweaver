//! 场景构建（自 ui/src/lib/scene.ts 移植）：布局结果 + 样式级联 + 高级元素 → 渲染就绪数据。
//!
//! 画布、导出、小地图共用这一份 [`Scene`]，所以任何「看起来怎样」的决定都在这里定完：
//! 渲染层只做变换与实例化，不再算几何、不再解析样式（docs/13 §9：导出与画布同源）。
//!
//! 索引约定：`ids[i]` 是第 i 个节点的文档 id，`nodes[i]` 是它的渲染数据，
//! `edges`/`boundaries`/`summaries`/`relationships` 用同一套 `u32` 索引互相引用。

use std::collections::{HashMap, HashSet};

use crate::concept_layout::proposition_label_offsets;
use crate::geom::{clip_toward, Enclosing, Path, PathSeg, Point, Rect};
use crate::icons::{icon_def, icon_width};
use crate::layout::{
    compute_layout, compute_layout_memo_with_reach, BoxSpec, Edge, EdgeShape, Editing, Layout,
    LayoutKind, LayoutMemo, NodeBox, NodeMeasure, Orientation, BOUND_PAD,
};
use crate::model::{ArrowHead, LayoutSettings, Node};
use crate::text::{TextMeasurer, NO_WRAP};
use crate::theme::{resolve_all, ResolvedStyle, Rgba, Shape, Theme, WEIGHT_REGULAR};
use crate::Document;

/// 边界外扩（TS scene.ts 的 `pad = 14`）。
pub const BOUNDARY_PAD: f32 = 14.0;
/// 未指定坐标的自由主题：主图右侧 140 起，逐个下堆 60。
const FLOATER_GAP_X: f32 = 140.0;
const FLOATER_GAP_Y: f32 = 60.0;
/// 概要摘要文字：13px、最宽 220。渲染层按同一字号排版，所以公开。
pub const SUMMARY_FONT: f32 = 14.0;
const SUMMARY_MAX_W: f32 = 220.0;
/// 关系线标签字号（渲染层同源）。
pub const REL_LABEL_FONT: f32 = 13.0;
/// 虚线的节奏：`[实长, 空长]`，**世界单位**（`docs/30` §5.4.3，CM-16）。
///
/// 为什么是世界单位而不是屏幕像素：`Scene` 必须与缩放无关（§5.4 不变量），
/// 而虚线是几何——SVG / PDF 导出直接照这个节奏画，屏幕与导出才逐段对得上。
/// 代价是缩到 20% 时 6 世界单位 ≈ 1.2 css px，看起来接近实线；那个缩放档位
/// 本来也读不出线型，可以接受。
pub const REL_DASH: [f32; 2] = [6.0, 4.0];
/// 分组框的内距：成员包围盒往外扩这么多（世界单位）。`docs/32` §13。
pub const GROUP_PAD: f32 = 24.0;
/// 分组标签的行高（世界单位）：框的上沿往上让出这么高再画字。
pub const GROUP_LABEL_H: f32 = 14.0;
/// 节点盒最小宽度（TS scene.ts 的 `max(..., 60)`）。
const MIN_NODE_W: f32 = 88.0;

// ---- 概念图节点（`docs/30` §5.3）----
/// 概念盒左右内距：宽 = 文本宽 + 28。
const CONCEPT_PAD_X: f32 = 14.0;
/// 概念盒最小宽。
pub const CONCEPT_MIN_W: f32 = 96.0;
/// 概念盒最大宽；文本按「240 - 28」换行。
pub const CONCEPT_MAX_W: f32 = 240.0;
/// 单行概念盒高。
pub const CONCEPT_H1: f32 = 36.0;
/// 双行概念盒高。
pub const CONCEPT_H2: f32 = 52.0;
/// 概念文字上限宽（盒最大宽减两侧内距）。
const CONCEPT_TEXT_MAX: f32 = CONCEPT_MAX_W - 2.0 * CONCEPT_PAD_X;

// ---- 椭圆概念盒（`docs/30` §5.3 / §12 第 9 条）----
/// 椭圆横向外扩比例：以矩形内接盒为基准按比例放大。
const ELLIPSE_EXTRA_X: f32 = 0.22;
/// 椭圆纵向外扩比例。
const ELLIPSE_EXTRA_Y: f32 = 0.16;
/// 横向外扩封顶（px）：长短语不该被拉成一颗大饼。
const ELLIPSE_EXTRA_MAX_X: f32 = 24.0;
/// 纵向外扩封顶（px）。
const ELLIPSE_EXTRA_MAX_Y: f32 = 8.0;

/// 节点下方的小标签胶囊（**节点局部坐标**，原点为节点盒左上角）。
#[derive(Clone, Debug, PartialEq)]
pub struct LabelPill {
    pub rect: Rect,
    pub text: String,
    pub font_size: f32,
}

/// 图标标记（字形 + 颜色 + 占位宽度）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconMark {
    pub glyph: &'static str,
    pub color: &'static str,
    pub width: f32,
}

/// 概要 / 花括号的伸展方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Right,
    Left,
}

impl Side {
    pub fn sign(self) -> f32 {
        match self {
            Self::Right => 1.0,
            Self::Left => -1.0,
        }
    }
}

/// 可渲染节点。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    pub geom: NodeBox,
    pub style: ResolvedStyle,
    pub has_children: bool,
    pub collapsed: bool,
    pub has_note: bool,
    pub has_link: bool,
    /// 概念文字被截断（概念图最多两行）。真截了才给 tooltip（docs/30 §5.3.1）。
    pub truncated: bool,
    /// 挂了卡片笔记：概念图右下角画 5px 圆点 + hover 提示（docs/30 §5.3.1）。
    pub has_card: bool,
    /// 图片引用（`asset:<hash>` 或外部路径）；无图为 `None`。
    pub image: Option<String>,
    pub parent: Option<u32>,
    pub icon: Option<IconMark>,
    /// 文字起始 x（有图标时右移；节点局部坐标）
    pub text_x: f32,
    /// 文字水平居中（概念图：概念是短语，居中读起来才像节点而不是卡片内容）。
    pub center_text: bool,
    pub labels: Vec<LabelPill>,
    /// 所属自由主题的根（主树内节点为 `None`）
    pub float_root: Option<u32>,
    /// 过渡透明度（1 = 正常）。只有 `blend_scenes` 会写非 1 值。
    pub alpha: f32,
}

/// 父子连线（含颜色与线宽）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneEdge {
    pub from: u32,
    pub to: u32,
    pub start: Point,
    pub end: Point,
    pub orientation: Orientation,
    pub color: Rgba,
    pub width: f32,
    /// 子端线宽（CSS px）。锥形分支线在一条边上从 `width` 收到 `width_end`。
    pub width_end: f32,
    /// 线型：曲线/直角/直线。
    pub shape: EdgeShape,
    /// 子端是否带箭头。
    pub arrow: bool,
    /// 过渡透明度（1 = 正常）。
    pub alpha: f32,
}

/// 装饰性轴线（鱼骨脊线 / 时间轴）：纯视觉，不参与命中测试。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSpine {
    pub path: Path,
    pub color: Rgba,
    pub width: f32,
    /// 过渡透明度（1 = 正常）。
    pub alpha: f32,
}

/// 边界：圈住若干直接子节点及其可见子树。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneBoundary {
    pub id: String,
    pub owner: u32,
    pub rect: Rect,
    pub color: Rgba,
    /// 过渡透明度（1 = 正常）。
    pub alpha: f32,
}

/// 概念分组框（CM-18，`docs/32` §13）：圈住若干概念的圆角虚线框。
///
/// 矩形是**派生量**（成员包围盒 + 内距），不进文件；成员一动框就跟着变。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneGroup {
    pub id: String,
    pub label: String,
    /// 组框（世界坐标，已含内距）。
    pub rect: Rect,
    /// 组框与标签的颜色（自定义色 > 主题 dim）。
    pub color: Rgba,
    /// 标签文字的**左上角**锚点（世界坐标），压在框的上沿外侧。
    pub label_at: Point,
}

/// 概要：花括号 + 摘要主题盒 + 短连接线。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSummary {
    pub id: String,
    pub owner: u32,
    pub text: String,
    pub side: Side,
    pub brace: Path,
    pub topic: Rect,
    pub connector: Path,
    pub color: Rgba,
    /// 过渡透明度（1 = 正常）。
    pub alpha: f32,
}

/// 关系线：任意两节点间的曲线 + 箭头 + 中部标签。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneRelationship {
    pub id: String,
    pub from: u32,
    pub to: u32,
    pub label: Option<String>,
    pub path: Path,
    /// 终点端箭头三角形（世界坐标）。`None` = 这一端不画（CM-10 ⑦ 的「无箭头」/「反向」）。
    pub arrow: Option<[Point; 3]>,
    /// 起点端箭头三角形（世界坐标）。`None` = 这一端不画（默认就是 `None`）。
    pub arrow_back: Option<[Point; 3]>,
    pub start: Point,
    pub label_at: Point,
    pub label_w: f32,
    pub color: Rgba,
    /// 过渡透明度（1 = 正常）。
    pub alpha: f32,
    /// 虚线（`docs/30` §5.4.3，CM-16）。`true` = 按 [`REL_DASH`] 的节奏断开。
    pub dashed: bool,
    /// 线宽（css px）。屏幕空间恒定，渲染层再换成世界单位。
    pub width: f32,
}

/// 一帧渲染所需的全部内容。
#[derive(Clone, Debug)]
pub struct Scene {
    pub ids: std::sync::Arc<Vec<String>>,
    pub index: std::sync::Arc<HashMap<String, u32>>,
    /// 节点表。`Arc`：场景与「下一轮的账」共享同一份，重建时不再克隆两万个 `SceneNode`
    /// （M12f）；只有过渡帧真的改节点时才 `Arc::make_mut` 复制一次。
    pub nodes: std::sync::Arc<Vec<SceneNode>>,
    /// 绘制顺序：深度浅的先画，使父盒与其连线压在子节点之下
    pub draw_order: Vec<u32>,
    pub edges: Vec<SceneEdge>,
    pub spines: Vec<SceneSpine>,
    pub boundaries: Vec<SceneBoundary>,
    /// 概念分组框（CM-18）。只有概念图会有；思维导图恒为空。
    pub groups: Vec<SceneGroup>,
    pub summaries: Vec<SceneSummary>,
    pub relationships: Vec<SceneRelationship>,
    pub bounds: Rect,
    pub kind: LayoutKind,
    /// 导出专用的「聚焦问题」条（CM-10 ⑥ 的导出补齐）。
    ///
    /// **画布永远是 `None`**：画布的问题带是贴视口顶的 pinned 层，滚动时不跟着内容走；
    /// 导出没有视口，只能锚在内容包围盒之上。两边共用的是文字排版，不是几何（docs/32 §6）。
    pub focus_note: Option<SceneNote>,
}

/// 画面顶部的一条自由文字（目前只有概念图的「聚焦问题」用它）。
#[derive(Clone, Debug, PartialEq)]
pub struct SceneNote {
    pub rect: Rect,
    pub text: String,
    pub font_size: f32,
    pub color: Rgba,
}

impl Scene {
    pub fn find(&self, id: &str) -> Option<u32> {
        self.index.get(id).copied()
    }

    pub fn node(&self, idx: u32) -> &SceneNode {
        &self.nodes[idx as usize]
    }

    pub fn rect(&self, idx: u32) -> Rect {
        self.nodes[idx as usize].geom.rect
    }

    /// 把「聚焦问题」挂到画面顶部，并把 `bounds` 向上扩一条带子的高度。
    ///
    /// 不移动任何既有几何——所以导出画面除了顶上多一条，其余逐像素不变。
    /// 只给导出调用；画布调用它会让问题带跟着内容滚走，那不是设计（docs/32 §6）。
    pub fn attach_focus_note(&mut self, text: &str, font_size: f32, color: Rgba) {
        // 行高走设计令牌的倍数、上下各留 `FOCUS_NOTE_PAD`——与画布那条带子同一套观感
        // （画布那边是 galley 高 + 2×7，16px 落在 36 上下，这里算出来 36.1）。
        // 取整：带子高带小数会让整幅内容落到半个像素上，导出图与「没带子」那张就不再逐像素同源，
        // 没法用「位移 N 行后完全相等」来证明只多了顶上一条。
        let h = (font_size * crate::design::text::line_height(font_size) + 2.0 * FOCUS_NOTE_PAD)
            .round();
        let rect = Rect::new(self.bounds.x, self.bounds.y - h, self.bounds.w, h);
        self.focus_note = Some(SceneNote {
            rect,
            text: text.to_string(),
            font_size,
            color,
        });
        self.bounds.y -= h;
        self.bounds.h += h;
    }
}

/// 导出时「聚焦问题」条上下各留的白（世界坐标）。
/// 7 与画布那条带子的 `pad.y` 一致——两边留白不一样，导出图里的问题就会比屏幕上更「松」。
pub const FOCUS_NOTE_PAD: f32 = 7.0;

/// 两帧场景之间的过渡。`t` 是**已缓动**的进度（0 = `from`，1 = `to`）。
///
/// 只插值几何与透明度：共同节点的样式/文字直接取目标（移动中文字保持可读），
/// 新增节点从父盒中心滑出并淡入，消失节点滑向父盒中心并淡出；连线端点与
/// 边界、概要、关系线、轴线跟着走。两端直接返回入参副本，保证终态逐字节一致。
pub fn blend_scenes(from: &Scene, to: &Scene, t: f32) -> Scene {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return to.clone();
    }
    let mut scene = to.clone();
    // 过渡帧真的要改节点：只在这里复制一次，重建路径不需要。
    let nodes = std::sync::Arc::make_mut(&mut scene.nodes);

    // ---- 共同节点：位置与文字起点插值；新节点先定「从父盒出发」的起点 ----
    for i in 0..to.nodes.len() {
        if let Some(&j) = from.index.get(to.ids[i].as_str()) {
            let a = from.nodes[j as usize].geom.rect;
            let b = to.nodes[i].geom.rect;
            nodes[i].geom.rect = lerp_rect(a, b, t);
            nodes[i].text_x = lerp(from.nodes[j as usize].text_x, to.nodes[i].text_x, t);
            nodes[i].alpha = lerp(from.nodes[j as usize].alpha, to.nodes[i].alpha, t);
            continue;
        }
        let target = to.nodes[i].geom.rect;
        nodes[i].alpha = t;
        let start = to.nodes[i]
            .parent
            .and_then(|p| to.ids.get(p as usize))
            .and_then(|pid| from.index.get(pid.as_str()).copied())
            .map(|j| centered_at(from.nodes[j as usize].geom.rect.center(), target))
            .unwrap_or(target);
        nodes[i].geom.rect = lerp_rect(start, target, t);
    }

    // ---- 消失节点：追加在目标节点之后，父/自由主题根按 id 重映射 ----
    let mut appended: Vec<(usize, SceneNode, Option<String>, Option<String>)> = Vec::new();
    for j in 0..from.nodes.len() {
        if to.index.contains_key(from.ids[j].as_str()) {
            continue;
        }
        let mut node = from.nodes[j].clone();
        let parent_id = node.parent.and_then(|p| from.ids.get(p as usize)).cloned();
        let float_id = node
            .float_root
            .and_then(|p| from.ids.get(p as usize))
            .cloned();
        let r = node.geom.rect;
        let end = parent_id
            .as_deref()
            .and_then(|pid| to.index.get(pid).copied())
            .map(|p| centered_at(to.nodes[p as usize].geom.rect.center(), r))
            .unwrap_or(r);
        node.geom.rect = lerp_rect(r, end, t);
        node.alpha = 1.0 - t;
        node.parent = None;
        node.float_root = None;
        appended.push((j, node, parent_id, float_id));
    }
    let base = nodes.len() as u32;
    let mut remap = Vec::with_capacity(appended.len());
    for (k, (j, node, parent_id, float_id)) in appended.into_iter().enumerate() {
        let idx = base + k as u32;
        std::sync::Arc::make_mut(&mut scene.ids).push(from.ids[j].clone());
        std::sync::Arc::make_mut(&mut scene.index).insert(from.ids[j].clone(), idx);
        scene.draw_order.push(idx);
        nodes.push(node);
        remap.push((idx, parent_id, float_id));
    }
    for (idx, parent_id, float_id) in remap {
        let parent = parent_id.and_then(|id| scene.index.get(id.as_str()).copied());
        let float_root = float_id.and_then(|id| scene.index.get(id.as_str()).copied());
        nodes[idx as usize].parent = parent;
        nodes[idx as usize].float_root = float_root;
    }

    // ---- 连线：共有端点插值；只有一端存在的淡入/淡出 ----
    let from_edges: HashMap<(&str, &str), usize> = from
        .edges
        .iter()
        .enumerate()
        .map(|(k, e)| {
            (
                (
                    from.ids[e.from as usize].as_str(),
                    from.ids[e.to as usize].as_str(),
                ),
                k,
            )
        })
        .collect();
    let target_keys: HashSet<(&str, &str)> = to
        .edges
        .iter()
        .map(|e| {
            (
                to.ids[e.from as usize].as_str(),
                to.ids[e.to as usize].as_str(),
            )
        })
        .collect();
    for e in scene.edges.iter_mut() {
        let key = (
            to.ids[e.from as usize].as_str(),
            to.ids[e.to as usize].as_str(),
        );
        match from_edges.get(&key) {
            Some(&k) => {
                let old = from.edges[k];
                e.start = lerp_point(old.start, e.start, t);
                e.end = lerp_point(old.end, e.end, t);
                // 半透明的连线再起一段过渡（动画中途又改结构）时，透明度要接着走，
                // 不能直接跳到目标值——不然「固化此刻那一帧」只固化了位置，淡入会当场断掉。
                e.alpha = lerp(old.alpha, e.alpha, t);
            }
            None => {
                e.alpha = t;
                // 新连线从父侧端点「长」到子节点；否则子节点还在半路，
                // 线已经先连到终点了。
                e.end = lerp_point(e.start, e.end, t);
            }
        }
    }
    for old in from.edges.iter() {
        let key = (
            from.ids[old.from as usize].as_str(),
            from.ids[old.to as usize].as_str(),
        );
        if target_keys.contains(&key) {
            continue;
        }
        let (Some(&fi), Some(&ti)) = (
            scene.index.get(from.ids[old.from as usize].as_str()),
            scene.index.get(from.ids[old.to as usize].as_str()),
        ) else {
            continue;
        };
        let mut e = *old;
        e.from = fi;
        e.to = ti;
        e.start = lerp_point(old.start, scene.nodes[fi as usize].geom.rect.center(), t);
        e.end = lerp_point(old.end, scene.nodes[ti as usize].geom.rect.center(), t);
        e.alpha = 1.0 - t;
        scene.edges.push(e);
    }

    // ---- 边界：同 id 框体插值，整块消失/出现的淡出/淡入 ----
    let from_bounds: HashMap<&str, usize> = from
        .boundaries
        .iter()
        .enumerate()
        .map(|(k, b)| (b.id.as_str(), k))
        .collect();
    let target_bounds: HashSet<&str> = to.boundaries.iter().map(|b| b.id.as_str()).collect();
    for b in scene.boundaries.iter_mut() {
        if let Some(&k) = from_bounds.get(b.id.as_str()) {
            b.rect = lerp_rect(from.boundaries[k].rect, b.rect, t);
            b.alpha = lerp(from.boundaries[k].alpha, b.alpha, t);
        } else {
            b.alpha = t;
        }
    }
    for old in from
        .boundaries
        .iter()
        .filter(|b| !target_bounds.contains(b.id.as_str()))
    {
        let mut b = old.clone();
        if let Some(&owner) = scene.index.get(from.ids[old.owner as usize].as_str()) {
            b.owner = owner;
        }
        b.alpha = 1.0 - t;
        scene.boundaries.push(b);
    }

    // ---- 概要：id 相同则花括号/连接线/主题盒一起插值 ----
    let from_summaries: HashMap<&str, usize> = from
        .summaries
        .iter()
        .enumerate()
        .map(|(k, s)| (s.id.as_str(), k))
        .collect();
    let target_summaries: HashSet<&str> = to.summaries.iter().map(|s| s.id.as_str()).collect();
    for s in scene.summaries.iter_mut() {
        if let Some(&k) = from_summaries.get(s.id.as_str()) {
            let old = &from.summaries[k];
            s.topic = lerp_rect(old.topic, s.topic, t);
            if let Some(p) = lerp_path(&old.brace, &s.brace, t) {
                s.brace = p;
            }
            if let Some(p) = lerp_path(&old.connector, &s.connector, t) {
                s.connector = p;
            }
            s.alpha = lerp(old.alpha, s.alpha, t);
        } else {
            s.alpha = t;
        }
    }
    for old in from
        .summaries
        .iter()
        .filter(|s| !target_summaries.contains(s.id.as_str()))
    {
        let mut s = old.clone();
        if let Some(&owner) = scene.index.get(from.ids[old.owner as usize].as_str()) {
            s.owner = owner;
        }
        s.alpha = 1.0 - t;
        scene.summaries.push(s);
    }

    // ---- 关系线：id 相同则曲线/箭头/标签位置插值 ----
    let from_rels: HashMap<&str, usize> = from
        .relationships
        .iter()
        .enumerate()
        .map(|(k, r)| (r.id.as_str(), k))
        .collect();
    let target_rels: HashSet<&str> = to.relationships.iter().map(|r| r.id.as_str()).collect();
    for r in scene.relationships.iter_mut() {
        if let Some(&k) = from_rels.get(r.id.as_str()) {
            let old = &from.relationships[k];
            if let Some(p) = lerp_path(&old.path, &r.path, t) {
                r.path = p;
            }
            r.start = lerp_point(old.start, r.start, t);
            r.label_at = lerp_point(old.label_at, r.label_at, t);
            r.label_w = lerp(old.label_w, r.label_w, t);
            r.arrow = lerp_tri(old.arrow, r.arrow, t);
            r.arrow_back = lerp_tri(old.arrow_back, r.arrow_back, t);
            // 线宽可以插值；线型（实/虚）是离散的，取目标那一帧的值，不插。
            r.width = lerp(old.width, r.width, t);
            // 同连线：正在淡入/淡出的关系线，过渡中途再来一条，透明度从当前值接着走。
            // 少了这一行，「新命题线淡入」会在第二帧就被重建抹成满色。
            r.alpha = lerp(old.alpha, r.alpha, t);
        } else {
            r.alpha = t;
        }
    }
    for old in from
        .relationships
        .iter()
        .filter(|r| !target_rels.contains(r.id.as_str()))
    {
        let mut r = old.clone();
        if let Some(&f) = scene.index.get(from.ids[old.from as usize].as_str()) {
            r.from = f;
        }
        if let Some(&to_i) = scene.index.get(from.ids[old.to as usize].as_str()) {
            r.to = to_i;
        }
        r.alpha = 1.0 - t;
        scene.relationships.push(r);
    }

    // ---- 轴线：同布局按序插值；换布局时旧轴淡出、新轴淡入 ----
    if from.kind != to.kind {
        for s in scene.spines.iter_mut() {
            s.alpha = t;
        }
        for s in from.spines.iter() {
            let mut old = s.clone();
            old.alpha = 1.0 - t;
            scene.spines.push(old);
        }
    } else if from.spines.len() == to.spines.len() {
        for (old, new) in from.spines.iter().zip(scene.spines.iter_mut()) {
            if let Some(p) = lerp_path(&old.path, &new.path, t) {
                new.path = p;
            }
        }
    }

    scene.bounds = to.bounds;
    scene.kind = to.kind;
    scene
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
/// 三角形绕 `m` 点旋转 180°：终点端的箭头镜像过去就是起点端的箭头。
///
/// 用镜像而不是重算，是为了让「反向」与「双向」的箭头跟正向箭头**完全同形**——
/// 弧线两端的切线方向不同，重算会得到两个不一样大的箭头。
fn mirror_tri(tri: [Point; 3], m: Point) -> [Point; 3] {
    tri.map(|p| Point::new(2.0 * m.x - p.x, 2.0 * m.y - p.y))
}

/// 两个可能缺失的箭头三角形之间插值：一端有一端没有时按透明度淡入淡出，
/// 两端都有才逐点插值。
fn lerp_tri(from: Option<[Point; 3]>, to: Option<[Point; 3]>, t: f32) -> Option<[Point; 3]> {
    match (from, to) {
        (Some(a), Some(b)) => Some([
            lerp_point(a[0], b[0], t),
            lerp_point(a[1], b[1], t),
            lerp_point(a[2], b[2], t),
        ]),
        (None, Some(b)) => (t >= 0.999).then_some(b),
        (Some(a), None) => (t <= 0.001).then_some(a),
        (None, None) => None,
    }
}
fn lerp_point(a: Point, b: Point, t: f32) -> Point {
    Point::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t))
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(
        lerp(a.x, b.x, t),
        lerp(a.y, b.y, t),
        lerp(a.w, b.w, t),
        lerp(a.h, b.h, t),
    )
}

/// 保持 `r` 的尺寸，把盒心放到 `c`。
fn centered_at(c: Point, r: Rect) -> Rect {
    Rect::new(c.x - r.w / 2.0, c.y - r.h / 2.0, r.w, r.h)
}

/// 分段结构一致时才逐点插值；结构变了（换布局、概要重排）返回 `None`，由调用方保留目标。
fn lerp_path(a: &Path, b: &Path, t: f32) -> Option<Path> {
    if a.segs.len() != b.segs.len() {
        return None;
    }
    let mut out = Path::new(lerp_point(a.start, b.start, t));
    for (sa, sb) in a.segs.iter().zip(b.segs.iter()) {
        match (sa, sb) {
            (PathSeg::Line { to: a1 }, PathSeg::Line { to: b1 }) => {
                out.line(lerp_point(*a1, *b1, t));
            }
            (PathSeg::Quad { ctrl: ca, to: a1 }, PathSeg::Quad { ctrl: cb, to: b1 }) => {
                out.quad(lerp_point(*ca, *cb, t), lerp_point(*a1, *b1, t));
            }
            (
                PathSeg::Cubic {
                    c1: a1,
                    c2: a2,
                    to: at,
                },
                PathSeg::Cubic {
                    c1: b1,
                    c2: b2,
                    to: bt,
                },
            ) => {
                out.cubic(
                    lerp_point(*a1, *b1, t),
                    lerp_point(*a2, *b2, t),
                    lerp_point(*at, *bt, t),
                );
            }
            _ => return None,
        }
    }
    Some(out)
}

/// 文字区高度：行数 × 行高 + 上下内距，下限 32。
///
/// 内距默认随字号分档（见 `design::card_padding`），主题层级可用 `LevelStyle.pad` 覆盖；
/// 中心卡舒展、深层卡紧凑。
fn text_height(lines: usize, line_h: f32, pad_y: f32) -> f32 {
    (lines as f32 * line_h + 2.0 * pad_y).max(32.0)
}

/// 标签胶囊字号：节点字号的 78%，夹在 10–12。
fn pill_font_size(font_size: f32) -> f32 {
    (font_size * 0.78).round().clamp(10.0, 12.0)
}

/// 花括号（概要括号）：竖直跨度 `y0..y1`，脊柱在 `x`，朝 `side` 凸出。
pub fn brace_path(x: f32, y0: f32, y1: f32, side: Side) -> Path {
    let y1 = if y1 - y0 < 6.0 { y0 + 6.0 } else { y1 };
    let dir = side.sign();
    let r = ((y1 - y0) / 4.0).clamp(3.0, 9.0);
    let mid = (y0 + y1) / 2.0;
    let s = x + dir * 7.0;
    let t = x + dir * 13.0;
    let mut p = Path::new(Point::new(x, y0));
    p.cubic(Point::new(s, y0), Point::new(s, y0), Point::new(s, y0 + r));
    p.line(Point::new(s, mid - r));
    p.cubic(
        Point::new(s, mid - r * 0.5),
        Point::new(t, mid),
        Point::new(t, mid),
    );
    p.cubic(
        Point::new(t, mid),
        Point::new(s, mid + r * 0.5),
        Point::new(s, mid + r),
    );
    p.line(Point::new(s, y1 - r));
    p.cubic(Point::new(s, y1), Point::new(s, y1), Point::new(x, y1));
    p
}

/// 关系线弧线：垂直偏置形成柔和弯曲，终点切线定箭头朝向，`t = 0.5` 放标签。
/// 概念图的连线：**直线段 + 6×4 实心箭头**（`docs/30` §5.4）。
///
/// 思维导图的关系线用二次曲线是为了在密集树里绕开兄弟节点；概念图的节点是
/// 手动摆的，曲线只会让「谁是主语」变模糊，而且弧线中点会把链接短语推离视线。
fn straight_between(p1: Point, p2: Point) -> (Path, [Point; 3], Point) {
    let mut path = Path::new(p1);
    path.line(p2);
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let ux = dx / len;
    let uy = dy / len;
    let base = Point::new(p2.x - ux * 6.0, p2.y - uy * 6.0);
    let perp = Point::new(-uy, ux);
    let arrow = [
        p2,
        Point::new(base.x + perp.x * 2.0, base.y + perp.y * 2.0),
        Point::new(base.x - perp.x * 2.0, base.y - perp.y * 2.0),
    ];
    let mid = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    (path, arrow, mid)
}

/// 同一对概念之间有**多条命题**时的车道间距上限（世界单位，`docs/30` §5.4）。
///
/// 短语胶囊高 20，两条线的中点必须分开到这个量级，标签才不会叠在一起。
const CONCEPT_LANE_GAP: f32 = 24.0;

/// 把直线按法向 `off` 微微拱起（`off` 为零时逐字节等同 `straight_between`）。
///
/// 用途：概念图里 A 与 B 之间可以有多条命题（多对多关系），它们原先画成**同一条直线**，
/// 完全重合——既看不出有几条，也没法单独点选。现在第 k 条走第 k 条车道。
fn bowed_straight(p1: Point, p2: Point, off: Point) -> (Path, [Point; 3], Point) {
    if off.x.abs() < 0.01 && off.y.abs() < 0.01 {
        return straight_between(p1, p2);
    }
    let mid = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    // 二次贝塞尔 t=0.5 落在 (p1 + 2C + p2)/4：控制点要偏两倍才让中点落在 `off`。
    let ctrl = Point::new(mid.x + off.x * 2.0, mid.y + off.y * 2.0);
    let mut path = Path::new(p1);
    path.quad(ctrl, p2);
    // 箭头按终点切线重算，否则会指向旧直线方向。
    let (tx, ty) = (p2.x - ctrl.x, p2.y - ctrl.y);
    let tl = (tx * tx + ty * ty).sqrt().max(1.0);
    let (ux, uy) = (tx / tl, ty / tl);
    let base = Point::new(p2.x - ux * 6.0, p2.y - uy * 6.0);
    let perp = Point::new(-uy, ux);
    let arrow = [
        p2,
        Point::new(base.x + perp.x * 2.0, base.y + perp.y * 2.0),
        Point::new(base.x - perp.x * 2.0, base.y - perp.y * 2.0),
    ];
    let curve_mid = Point::new(
        0.25 * p1.x + 0.5 * ctrl.x + 0.25 * p2.x,
        0.25 * p1.y + 0.5 * ctrl.y + 0.25 * p2.y,
    );
    (path, arrow, curve_mid)
}

/// 连线颜色：显式色优先 > 跨分支取来源分支色 > 默认 dim（docs/30 §5.4.1）。
fn concept_rel_color(
    branches: &HashMap<String, u32>,
    ids: &[String],
    from: u32,
    to: u32,
    theme: &crate::theme::Theme,
) -> Rgba {
    if branches.is_empty() {
        return theme.ui.dim;
    }
    let branch_of = |i: u32| ids.get(i as usize).and_then(|id| branches.get(id)).copied();
    match (branch_of(from), branch_of(to)) {
        (Some(a), Some(b)) if a != b => theme.palette_at(a as usize).unwrap_or(theme.ui.dim),
        _ => theme.ui.dim,
    }
}

/// 交叉的一对线里，**后声明的那条**弯一次：中点垂偏 `min(6, len/12)` 的二次曲线。
///
/// 只弯一次是刻意的：让每条线都去避让，密集图会变成一团弧线，反而看不出主次。
fn bow_crossings(rels: &mut [SceneRelationship]) {
    if rels.len() < 2 || rels.len() > 512 {
        return;
    }
    let ends: Vec<(Point, Point)> = rels.iter().map(|r| (r.start, r.path.end())).collect();
    let mut bowed = vec![false; rels.len()];
    for j in 1..ends.len() {
        for i in 0..j {
            if segments_cross(ends[i], ends[j]) {
                bowed[j] = true;
                break;
            }
        }
    }
    for (j, r) in rels.iter_mut().enumerate() {
        if !bowed[j] {
            continue;
        }
        // 车道曲线（C14）已经偏离弦线：再按弦线重弯一次会把车道偏移抹掉，
        // 两条平行命题又叠回一条。避让只改直线。
        if matches!(r.path.segs.as_slice(), [PathSeg::Quad { .. }]) {
            continue;
        }
        let (p1, p2) = ends[j];
        let (path, arrow, mid) = bowed_between(p1, p2);
        r.path = path;
        r.label_at = mid;
        // 换个弯法后两端的箭头都要重新钉：`arrow` 是终点端，镜像就是起点端。
        let m = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
        if r.arrow.is_some() {
            r.arrow = Some(arrow);
        }
        if r.arrow_back.is_some() {
            r.arrow_back = Some(mirror_tri(arrow, m));
        }
    }
}

/// 两段直线是否**真相交**：端点相接不算，夹角 <12° 的近平行不算（§5.4.1）。
fn segments_cross(a: (Point, Point), b: (Point, Point)) -> bool {
    let dir = |p: (Point, Point)| (p.1.x - p.0.x, p.1.y - p.0.y);
    let (ax, ay) = dir(a);
    let (bx, by) = dir(b);
    let la = (ax * ax + ay * ay).sqrt();
    let lb = (bx * bx + by * by).sqrt();
    if la < 1.0 || lb < 1.0 {
        return false;
    }
    let mut deg = ((ax * bx + ay * by) / (la * lb))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();
    if deg > 90.0 {
        deg = 180.0 - deg;
    }
    if deg < 12.0 {
        return false;
    }
    let cross =
        |o: Point, p: Point, q: Point| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let d1 = cross(b.0, b.1, a.0);
    let d2 = cross(b.0, b.1, a.1);
    let d3 = cross(a.0, a.1, b.0);
    let d4 = cross(a.0, a.1, b.1);
    // 0.5 世界单位的死区：贴着端点/共线的两段不当作交叉，否则「箭头正好抵着线」也会弯。
    const EPS: f32 = 0.5;
    if d1.abs() < EPS || d2.abs() < EPS || d3.abs() < EPS || d4.abs() < EPS {
        return false;
    }
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// 微弧：偏移方向由「from 在 to 的哪一侧」定死，同一份文档两次渲染必然一致。
fn bowed_between(p1: Point, p2: Point) -> (Path, [Point; 3], Point) {
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let off = (len / 12.0).min(6.0);
    let far = if dx >= 0.0 { 1.0 } else { -1.0 };
    let nx = dy / len * far;
    let ny = -dx / len * far;
    let mid = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    // 二次贝塞尔 t=0.5 落在 (p1 + 2C + p2)/4：控制点要偏两倍才得到 `off` 的垂距。
    let ctrl = Point::new(mid.x + nx * off * 2.0, mid.y + ny * off * 2.0);
    let mut path = Path::new(p1);
    path.quad(ctrl, p2);
    // 箭头按终点切线重算，方向才不会指向旧直线。
    let (tx, ty) = (p2.x - ctrl.x, p2.y - ctrl.y);
    let tl = (tx * tx + ty * ty).sqrt().max(1.0);
    let (ux, uy) = (tx / tl, ty / tl);
    let base = Point::new(p2.x - ux * 9.0, p2.y - uy * 9.0);
    let perp = Point::new(-uy, ux);
    let arrow = [
        p2,
        Point::new(base.x + perp.x * 4.2, base.y + perp.y * 4.2),
        Point::new(base.x - perp.x * 4.2, base.y - perp.y * 4.2),
    ];
    let label = Point::new(
        0.25 * p1.x + 0.5 * ctrl.x + 0.25 * p2.x,
        0.25 * p1.y + 0.5 * ctrl.y + 0.25 * p2.y,
    );
    (path, arrow, label)
}

fn curve_between(p1: Point, p2: Point) -> (Path, [Point; 3], Point) {
    let mx = (p1.x + p2.x) / 2.0;
    let my = (p1.y + p2.y) / 2.0;
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let k = (len * 0.22).min(56.0);
    let ctrl = Point::new(mx - dy / len * k, my + dx / len * k);

    let mut path = Path::new(p1);
    path.quad(ctrl, p2);

    let tx = p2.x - ctrl.x;
    let ty = p2.y - ctrl.y;
    let tl = (tx * tx + ty * ty).sqrt().max(1.0);
    let ux = tx / tl;
    let uy = ty / tl;
    let base = Point::new(p2.x - ux * 9.0, p2.y - uy * 9.0);
    let perp = Point::new(-uy, ux);
    let arrow = [
        p2,
        Point::new(base.x + perp.x * 4.2, base.y + perp.y * 4.2),
        Point::new(base.x - perp.x * 4.2, base.y - perp.y * 4.2),
    ];
    let mid = Point::new(
        0.25 * p1.x + 0.5 * ctrl.x + 0.25 * p2.x,
        0.25 * p1.y + 0.5 * ctrl.y + 0.25 * p2.y,
    );
    (path, arrow, mid)
}

/// 节点度量：文字 + 图标 + 标签胶囊 → 盒子尺寸（TS buildScene 里的 `measure`）。
use std::cell::RefCell;

/// 节点测量缓存：文本/样式/图标/标签/图片都没变的节点直接复用上次 `BoxSpec`。
/// 键里包含所有会影响盒尺寸的字段；主题只换颜色时不会误用旧尺寸。
///
/// 键以前是 `format!` 拼出来的字符串：每个节点每轮重建都要一次格式化 + 一次分配，
/// 20k 文档光这一项就吃掉十几毫秒（打磨轮 6 实测 25ms 布局里的大头）。改成结构体比较后，
/// 命中时一次分配都没有，只有真重测的那几个节点才克隆一份键。
#[derive(Clone, Debug, PartialEq)]
struct BoxKey {
    text: String,
    font_px: f32,
    bold: bool,
    /// 生效字重（SE-13）。进键的理由同 `bold`：400 → 600 换的是字面，
    /// 字宽也跟着换，不重测就会留下按旧字面量的盒宽。
    weight: u16,
    icon: Option<String>,
    labels: Vec<String>,
    image: Option<String>,
    max_width: f32,
    /// 生效内距（主题层级覆盖或按字号分档）。内距改盒宽/盒高，必须进键。
    pad: (f32, f32),
    concept: bool,
    /// 形状进键是因为**概念盒的几何跟着形状变**（椭圆要外扩，`docs/30` §5.3）：
    /// 不进键的话，用户从椭圆切回圆角矩形时缓存会把外扩后的盒子留下——
    /// 文字没变、字号没变，什么都不变，只有形状变了。
    shape: Shape,
}

impl BoxKey {
    /// 与当前输入逐项比对——命中就不必先造一个键。
    fn matches(
        &self,
        node: &Node,
        text: &str,
        st: ResolvedStyle,
        max_width: f32,
        concept: bool,
    ) -> bool {
        self.concept == concept
            && self.font_px == st.font_size
            && self.bold == st.bold
            && self.weight == st.weight
            && self.max_width == max_width
            && self.pad == st.card_pad()
            && self.text == text
            && self.shape == st.shape
            && self.icon.as_deref() == node.icon.as_deref()
            && self.image.as_deref() == node.image.as_deref()
            && self.labels == node.labels
    }
}

struct CachedBox {
    key: BoxKey,
    spec: BoxSpec,
}

/// 上一次重建留下的账（打磨轮 11）：主题 + 样式输入指纹 + 解出的样式表 + 布局账。
///
/// 只在「同一个根、同一套主题、样式输入一模一样」时复用；任何一项对不上就整份作废。
/// 复用的最坏后果是慢（退化成全量），不是画错——画错由金样兜底。
struct SceneMemo {
    theme: Theme,
    root: String,
    /// 样式/结构内容修订号（不含文字）。文字编辑时直接命中，不必再遍历 20k 节点算指纹。
    content_rev: u64,
    /// 只影响盒宽的两项设置（kind / max_width）的轻量指纹。
    settings_fp: u64,
    style_fp: u64,
    styles: std::sync::Arc<HashMap<String, ResolvedStyle>>,
    layout: LayoutMemo,
    /// 上一轮组装好的场景节点。盒子没变的那颗直接克隆（`SceneNode` 除了行数据是 Arc，
    /// 其余都是 POD），比重查一遍文档、样式表、父索引再拼一遍便宜一个数量级。
    nodes: std::sync::Arc<Vec<SceneNode>>,
}

#[derive(Default)]
pub struct SceneCache {
    boxes: RefCell<HashMap<String, CachedBox>>,
    /// 当前这一代的账。
    memo: RefCell<Option<SceneMemo>>,
    /// 上一代的账（打磨轮 15）：撤销/重做会把文档**退回**到上一轮走过的那份内容，
    /// 只留一代的话每次都整份作废、20k 上白付 ~25ms。两代足够覆盖「撤销一步」与「撤销再重做」。
    prev_memo: RefCell<Option<SceneMemo>>,
    /// 下一次重建优先试上一代（撤销/重做刚把内容退回去了）。
    prefer_prev: std::cell::Cell<bool>,
    hits: std::cell::Cell<usize>,
    misses: std::cell::Cell<usize>,
}

impl SceneCache {
    fn take_memo(&self) -> Option<SceneMemo> {
        self.memo.borrow_mut().take()
    }

    fn take_prev_memo(&self) -> Option<SceneMemo> {
        self.prev_memo.borrow_mut().take()
    }

    /// 下一次重建优先试「上一代」的账。
    ///
    /// 撤销/重做会把内容退回到上一代走过的那份；不打招呼的话场景会先按当前代去试，
    /// 白白跑一遍布局才发现「复用不全」（20k 上多付 3–4ms）。
    pub fn prefer_previous(&self) {
        self.prefer_prev.set(true);
    }

    fn take_prefer(&self) -> bool {
        self.prefer_prev.replace(false)
    }

    /// 收新一代的账；`spare` 是这次没用上的那一代，退成「上一代」。
    fn put_memo(&self, memo: SceneMemo, spare: Option<SceneMemo>) {
        *self.memo.borrow_mut() = Some(memo);
        *self.prev_memo.borrow_mut() = spare;
    }
}

impl SceneCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 本进程内累计的 `(命中, 未命中)`。只给性能证据用（`perf_scene`）：
    /// 「缓存全命中也要 25ms」这个结论得能被复跑验证（打磨轮 10）。
    pub fn stats(&self) -> (usize, usize) {
        (self.hits.get(), self.misses.get())
    }

    pub fn len(&self) -> usize {
        self.boxes.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.boxes.borrow().is_empty()
    }

    /// 主题/字体整体变化时清空；平时让键自然失效。
    pub fn clear(&self) {
        self.boxes.borrow_mut().clear();
    }
}

struct NodeSizer<'a> {
    styles: &'a HashMap<String, ResolvedStyle>,
    fallback: ResolvedStyle,
    measure: &'a dyn TextMeasurer,
    max_width: f32,
    assets: &'a crate::assets::Assets,
    cache: &'a SceneCache,
    /// 概念图：盒宽 = 文本 + 28，夹 96–240；高 36（双行 52），文字居中。
    concept: bool,
}

impl NodeSizer<'_> {
    fn style(&self, node: &Node) -> ResolvedStyle {
        self.styles.get(&node.id).copied().unwrap_or(self.fallback)
    }

    fn icon_w(&self, node: &Node) -> f32 {
        if icon_def(node.icon.as_deref()).is_some() {
            icon_width(self.style(node).font_size)
        } else {
            0.0
        }
    }

    /// 节点图片的宽高比：`asset:<hash>` 从资源表嗅探；坏/缺资源回退 16:9。
    fn image_ratio(&self, node: &Node) -> f32 {
        node.image
            .as_deref()
            .and_then(|v| v.strip_prefix("asset:"))
            .and_then(|h| self.assets.get(h))
            .and_then(crate::image::ratio)
            .unwrap_or(16.0 / 9.0)
    }

    /// 标签胶囊：`(文字, 宽度)` + 总宽 + 胶囊高；无标签时 `None`。
    fn pills(&self, node: &Node) -> Option<(Vec<(String, f32)>, f32, f32)> {
        if node.labels.is_empty() {
            return None;
        }
        let pf = pill_font_size(self.style(node).font_size);
        let pill_h = pf + 8.0;
        let mut pills = Vec::with_capacity(node.labels.len());
        let mut total = 0.0f32;
        for l in node.labels.iter() {
            let w = self.measure.measure(l, pf, WEIGHT_REGULAR, NO_WRAP).width + 12.0;
            pills.push((l.clone(), w));
            total += w + 6.0;
        }
        Some((pills, total - 6.0, pill_h))
    }
}

impl NodeSizer<'_> {
    /// 概念盒：宽 = 文本 + 28、夹 96–240，高 36（双行 52），最多两行。
    ///
    /// 不套卡片的「内距随字号比例」公式——概念是短语，盒子跟着短语走，
    /// 不跟着层级走（`docs/30` §5.3「层级由位置与连线表达，不靠字号递减」）。
    fn measure_concept(&self, node: &Node, text: &str, st: ResolvedStyle) -> BoxSpec {
        if let Some(cb) = self.cache.boxes.borrow().get(&node.id) {
            if cb.key.matches(node, text, st, self.max_width, true) {
                return cb.spec.clone();
            }
        }
        let m = self
            .measure
            .measure(text, st.font_size, st.weight, CONCEPT_TEXT_MAX);
        let mut lines: Vec<String> = m.lines.iter().take(2).cloned().collect();
        if m.lines.len() > 2 {
            if let Some(last) = lines.last_mut() {
                last.push('…');
            }
        }
        let inner_w = (m.width + 2.0 * CONCEPT_PAD_X).clamp(CONCEPT_MIN_W, CONCEPT_MAX_W);
        let inner_h = if lines.len() >= 2 {
            CONCEPT_H2
        } else {
            CONCEPT_H1
        };
        // 椭圆盒要把内接矩形外面的那两块弧兜进来（`docs/30` §5.3）：
        // 文字那一行的高度 `y` 上，椭圆的半宽只有 `a·√(1−(y/b)²)`——单行时 y/b = 10/22，
        // 系数 0.894；双行时 y/b = 20/30，系数 0.8165。谱系外扩量按比例给、再各自封顶，
        // 免得一句长短语在椭圆里被拉成一颗大饼。
        let (w, h) = if st.shape.is_ellipse() {
            (
                inner_w + (ELLIPSE_EXTRA_X * inner_w).min(ELLIPSE_EXTRA_MAX_X),
                inner_h + (ELLIPSE_EXTRA_Y * inner_h).min(ELLIPSE_EXTRA_MAX_Y),
            )
        } else {
            (inner_w, inner_h)
        };
        let spec = BoxSpec {
            w,
            h,
            lines: std::sync::Arc::from(lines),
            font_size: st.font_size,
            line_height: m.line_height,
            pad: st.card_pad(),
            bold: st.bold,
            label_h: 0.0,
            image_h: 0.0,
            image_ratio: 16.0 / 9.0,
        };
        self.cache.misses.set(self.cache.misses.get() + 1);
        self.cache.boxes.borrow_mut().insert(
            node.id.clone(),
            CachedBox {
                key: BoxKey {
                    text: text.to_string(),
                    font_px: st.font_size,
                    bold: st.bold,
                    weight: st.weight,
                    icon: node.icon.clone(),
                    labels: node.labels.clone(),
                    image: node.image.clone(),
                    max_width: self.max_width,
                    pad: st.card_pad(),
                    concept: true,
                    shape: st.shape,
                },
                spec: spec.clone(),
            },
        );
        spec
    }
}

impl NodeMeasure for NodeSizer<'_> {
    fn measure(&self, node: &Node, _depth: u32, text: &str) -> BoxSpec {
        let st = self.style(node);
        if self.concept {
            return self.measure_concept(node, text, st);
        }
        // 影响盒尺寸的所有输入都在键里：换文字/字号/粗体/图标/标签/图片/最大宽才重测。
        if let Some(cb) = self.cache.boxes.borrow().get(&node.id) {
            if cb.key.matches(node, text, st, self.max_width, false) {
                self.cache.hits.set(self.cache.hits.get() + 1);
                return cb.spec.clone();
            }
        }
        let m = self
            .measure
            .measure(text, st.font_size, st.weight, self.max_width);
        let input_pad = st.card_pad();
        let (pad_x, pad_y) = input_pad;
        let th = text_height(m.lines.len(), m.line_height, pad_y);
        let (label_h, labels_w) = match self.pills(node) {
            // 胶囊高度 + 上下各 4px 呼吸（落在 4px 网格上，和 design::space::XS 对齐）
            Some((_, total_w, ph)) => (ph + 8.0, total_w + 2.0 * pad_x),
            None => (0.0, 0.0),
        };
        let has_image = node.image.as_deref().is_some_and(|s| !s.trim().is_empty());
        let image_ratio = if has_image {
            self.image_ratio(node)
        } else {
            16.0 / 9.0
        };
        let w = (m.width + 2.0 * pad_x + self.icon_w(node))
            .max(labels_w)
            .max(if has_image { 180.0 } else { MIN_NODE_W });
        // 图片占位：宽随卡片，高按宽高比夹在 56–180，再留 8px 与文字分隔。
        let image_h = if has_image {
            crate::image::image_box(image_ratio, w, pad_x, 56.0, 180.0).2
        } else {
            0.0
        };
        let base_h = image_h + th + label_h;
        // 思维导图节点的椭圆盒：文字盒尺寸不变，盒子按比例外扩（X23 G-3）。
        // 内距同步加上外扩的一半，文字/标签在盒内仍然居中。
        let (w, h, pad) = if st.shape.is_ellipse() {
            let extra_w = (ELLIPSE_EXTRA_X * w).min(ELLIPSE_EXTRA_MAX_X);
            let extra_h = (ELLIPSE_EXTRA_Y * base_h).min(ELLIPSE_EXTRA_MAX_Y);
            (
                w + extra_w,
                base_h + extra_h,
                (pad_x + extra_w * 0.5, pad_y + extra_h * 0.5),
            )
        } else {
            (w, base_h, input_pad)
        };
        let spec = BoxSpec {
            w,
            h,
            lines: std::sync::Arc::from(m.lines),
            font_size: st.font_size,
            line_height: m.line_height,
            pad,
            bold: st.bold,
            label_h,
            image_h,
            image_ratio,
        };
        self.cache.boxes.borrow_mut().insert(
            node.id.clone(),
            CachedBox {
                key: BoxKey {
                    text: text.to_string(),
                    font_px: st.font_size,
                    bold: st.bold,
                    weight: st.weight,
                    icon: node.icon.clone(),
                    labels: node.labels.clone(),
                    image: node.image.clone(),
                    max_width: self.max_width,
                    pad: input_pad,
                    concept: false,
                    shape: st.shape,
                },
                spec: spec.clone(),
            },
        );
        spec
    }
}

/// 只覆盖影响盒宽的设置项：布局种类与最大节点宽。两者变了一定要重算样式指纹。
fn settings_fingerprint(settings: &LayoutSettings) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    fn mix(mut h: u64, bytes: &[u8]) -> u64 {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(FNV_PRIME);
        }
        h
    }
    let mut h = FNV_OFFSET;
    h = mix(h, settings.kind.as_bytes());
    mix(
        h,
        &settings.max_node_width.max(80.0).to_bits().to_le_bytes(),
    )
}

/// 场景装配的**输入指纹**（打磨轮 11）。
///
/// 覆盖「除文字以外、会影响某颗节点长什么样」的一切：层级与兄弟序号（决定主题级联）、
/// 样式覆盖、备注/链接/卡片、图标、标签、图片。打字改的是文字，不在这儿，
/// 所以指纹不变 → 上一轮的样式表与场景节点可以直接搬过来用。
///
/// 与 `resolve_all` 走同一条遍历（主根 + 按 id 排序的自由主题，逆序入栈即文档顺序），
/// 但不解析任何颜色。刻意不用 `HashSet`：20k 节点建一张访问表要 5ms，
/// 比它想省下的 `resolve_all` 还贵。
/// `settings` 只取**影响盒子尺寸**的那几项（目前只有 `max_node_width`）。
///
/// 以前这里只吃 `doc`：检查器把「最大节点宽」从 300 调到 200，文档内容一个字没变，
/// `style_fp` 也不变 → `SceneMemo` 整代被复用、`LayoutMemo.boxes` 逐颗照搬，
/// 画布继续按 300 画；而导出路径新建 `SceneCache`，按 200 量出的盒子，
/// 于是画布与 PNG/SVG/PDF 不一致，直到改一个字或重开文件才对齐（BUG-2）。
/// 间距（`sibling_gap` / `level_gap`）不进这里：它们只影响摆位，位置每轮都重算。
fn content_fingerprint(doc: &Document, settings: &LayoutSettings) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    fn mix(mut h: u64, bytes: &[u8]) -> u64 {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(FNV_PRIME);
        }
        h
    }
    fn mix_opt(h: u64, v: Option<&str>) -> u64 {
        match v {
            Some(s) => mix(mix(h, &[1]), s.as_bytes()),
            None => mix(h, &[0]),
        }
    }

    let mut h = FNV_OFFSET;
    let mut roots: Vec<&str> = vec![doc.root_id.as_str()];
    let mut floats: Vec<&str> = doc
        .nodes
        .values()
        .filter(|n| n.parent_id.is_none() && n.id != doc.root_id)
        .map(|n| n.id.as_str())
        .collect();
    floats.sort_unstable();
    roots.extend(floats);

    let cap = doc.nodes.len() * 4 + 8;
    let mut visited = 0usize;
    let mut stack: Vec<(&str, u32, usize)> = Vec::new();
    for id in roots.into_iter().rev() {
        stack.push((id, 0, 0));
    }
    while let Some((id, depth, sibling)) = stack.pop() {
        visited += 1;
        if visited > cap {
            break; // 万一有环也不死循环
        }
        let Some(n) = doc.node(id) else { continue };
        // id 也要进指纹：两颗**内容完全相同**的一级兄弟交换顺序时，兄弟序号的字节序列
        // 与内容序列都会原样重复一遍，只有 id 能分出「谁排在第几位」。少了这一项，
        // 交换后指纹不变 → 样式表整份复用 → 调色板颜色还留在旧 id 上（BUG-6）。
        h = mix(h, id.as_bytes());
        h = mix(h, &depth.to_le_bytes());
        h = mix(h, &sibling.to_le_bytes());
        match n.style.as_ref() {
            Some(st) => {
                h = mix(h, &[1]);
                h = mix_opt(h, st.fill.as_deref());
                h = mix_opt(h, st.text_color.as_deref());
                h = mix_opt(h, st.stroke.as_deref());
                h = mix(h, &st.stroke_width.unwrap_or(0.0).to_bits().to_le_bytes());
                h = mix(
                    h,
                    &[
                        st.bold.unwrap_or(false) as u8,
                        st.italic.unwrap_or(false) as u8,
                    ],
                );
                h = mix(h, &st.font_size.unwrap_or(0).to_le_bytes());
                h = mix_opt(h, st.shape.as_deref());
                h = mix(h, &st.radius.unwrap_or(-1.0).to_bits().to_le_bytes());
                h = mix_opt(h, st.align.as_deref());
                h = mix(h, &st.weight.unwrap_or(0).to_le_bytes());
                h = mix_opt(h, st.branch_color.as_deref());
                h = mix(h, &[st.apply_text_to_subtree.unwrap_or(false) as u8]);
            }
            None => h = mix(h, &[0]),
        }
        h = mix_opt(h, n.note.as_deref());
        h = mix_opt(h, n.link.as_deref());
        h = mix_opt(h, n.card_id.as_deref());
        h = mix_opt(h, n.icon.as_deref());
        h = mix_opt(h, n.image.as_deref());
        for l in n.labels.iter() {
            h = mix(mix(h, &[0x1f]), l.as_bytes());
        }
        h = mix(h, &[0xfe]);
        // 真正的兄弟序号：`resolve_all` 按它取一级调色板，指纹必须同源（以前恒传 0）。
        for (i, c) in n.children.iter().enumerate().rev() {
            stack.push((c.as_str(), depth + 1, i));
        }
    }
    // 概念图的默认样式还取决于图结构，这里一律按「每次都算」处理（见调用点）。
    h = mix(h, doc.meta.layout.kind.as_bytes());
    // 盒子尺寸的另一半输入：与 `NodeSizer.max_width` / `BoxKey.max_width` 同一口径，
    // 用了 `.max(80.0)` 之后的实际值，免得 20 与 79 在这种「反正都会被抬到 80」的区间里
    // 白丢一代缓存。
    mix(
        h,
        &settings.max_node_width.max(80.0).to_bits().to_le_bytes(),
    )
}

/// 构建场景。`layout_root` 通常是主根；传某个子节点即「聚焦该子树」，
/// 此时不摆自由主题（与旧版一致）。
pub fn build_scene(
    doc: &Document,
    layout_root: &str,
    settings: &LayoutSettings,
    theme: &Theme,
    measure: &dyn TextMeasurer,
    editing: Editing<'_>,
) -> Scene {
    build_scene_cached(
        doc,
        layout_root,
        settings,
        theme,
        measure,
        editing,
        &SceneCache::new(),
    )
}

/// 带测量缓存：同一文档只改一个节点时，其余节点的文字测量直接命中缓存。
pub fn build_scene_cached(
    doc: &Document,
    layout_root: &str,
    settings: &LayoutSettings,
    theme: &Theme,
    measure: &dyn TextMeasurer,
    editing: Editing<'_>,
    cache: &SceneCache,
) -> Scene {
    // 分阶段计时（打磨轮 10）：`MW_LAYOUT_LOG=1` 时打到 stderr。
    let perf = std::env::var_os("MW_LAYOUT_LOG").is_some();
    let mut marks: Vec<(&'static str, f64)> = Vec::new();
    let mut mark_at = std::time::Instant::now();
    macro_rules! mark {
        ($label:expr) => {
            if perf {
                marks.push(($label, mark_at.elapsed().as_secs_f64() * 1000.0));
                mark_at = std::time::Instant::now();
            }
        };
    }
    // 上一轮的账：主题、布局根、样式输入指纹三项都相同才复用样式表与布局账。
    // 概念图的默认样式还取决于概念图结构（关系线/卡片），指纹没覆盖到那一层，
    // 所以概念图一律重算——保守，但概念图的节点数远小于 20k 思维导图。
    let kind = LayoutKind::from_kind(&settings.kind);
    let settings_fp = settings_fingerprint(settings);
    let mut taken: Vec<SceneMemo> = Vec::new();
    let mut fresh = [cache.take_memo(), cache.take_prev_memo()];
    if cache.take_prefer() {
        fresh.swap(0, 1);
    }
    taken.extend(fresh.into_iter().flatten());
    // 快速路径（M12h）：`content_rev` 没变说明样式/结构/设置都没变，上一轮的样式指纹
    // 可以整份沿用——只改字时省掉一次 20k 节点的 `content_fingerprint` 遍历。
    // 设置也单独算一个轻量指纹，防止测试/调用方绕过命令直接改 `LayoutSettings`。
    let fast_fp = taken
        .iter()
        .find(|m| {
            m.theme == *theme
                && m.root == layout_root
                && m.content_rev == doc.content_rev
                && m.settings_fp == settings_fp
        })
        .map(|m| m.style_fp);
    let style_fp = fast_fp.unwrap_or_else(|| content_fingerprint(doc, settings));
    // 两代账：内容指纹相同（主题、布局根、样式输入）的**都**留作候选，布局跑完再挑
    // 「复用得更全」的那一代——撤销/重做会把内容退回上一代，而内容指纹里不含文字，
    // 光比对指纹分不出哪一代才是「改之前」，得看复用了多少颗节点的盒子（打磨轮 15）。
    // 注意：这里**不能**再比 `content_rev`——撤销/重做会让它 +1，但内容（除文字外）
    // 正好退回上一代，必须让上一代的账继续当候选（M12e 的两代账语义）。
    let same = |m: &SceneMemo| m.theme == *theme && m.root == layout_root && m.style_fp == style_fp;
    // 候选是「指纹对得上」的那几代；对不上的也不能扔——结构一变指纹就变，
    // 可它记着的正是「改之前」那份内容，撤销要退回去的账就在里面（打磨轮 15 第二步）。
    let candidates: Vec<usize> = (0..taken.len()).filter(|&i| same(&taken[i])).collect();
    if perf && candidates.is_empty() {
        eprintln!("[memo] 场景账对不上（两代都没有可用的指纹）");
    }
    // 样式表与内容指纹一一对应：哪一代的都一样，取第一份即可。
    let styles: std::sync::Arc<HashMap<String, ResolvedStyle>> = match candidates.first() {
        Some(&i) if !kind.is_concept_map() => taken[i].styles.clone(),
        _ => std::sync::Arc::new(resolve_all(doc, theme)),
    };
    mark!("resolve");
    let base = theme.level(usize::MAX);
    let fallback = ResolvedStyle {
        fill: base.fill,
        text: base.text,
        bold: base.bold,
        italic: false,
        font_size: base.font_size,
        shape: base.shape,
        stroke: Rgba::TRANSPARENT,
        stroke_width: 1.5,
        branch_color: theme.ui.accent,
        radius: None,
        align: None,
        pad: base.pad,
        // 回退样式也读层级 `weight`：500 档在只认布尔的地方会落成 400，
        // 但测量/绘制拿到的是真实字重（X25 G-25-08）。
        weight: base.weight,
    };
    let sizer = NodeSizer {
        styles: styles.as_ref(),
        fallback,
        measure,
        max_width: settings.max_node_width.max(80.0),
        assets: &doc.assets,
        cache,
        concept: kind.is_concept_map(),
    };
    let show_floaters = layout_root == doc.root_id;

    // 每一代各跑一次布局，取「复用得更全」的那份；有一代全命中就直接停。
    let mut best: Option<(usize, Layout, LayoutMemo, usize)> = None;
    for &ci in candidates.iter() {
        let c = &taken[ci];
        let (layout, memo, reused) = compute_layout_memo_with_reach(
            doc,
            layout_root,
            settings,
            &sizer,
            editing,
            Some(&c.layout),
        );
        if best.as_ref().is_none_or(|(br, ..)| reused > *br) {
            best = Some((reused, layout, memo, ci));
        }
        // 只差一颗（正是刚改/刚撤销的那颗）就认这一代，不再试另一代——
        // 另一边几乎必然更差，多跑一遍布局是白付（20k 上 3–4ms）。
        if reused + 1 >= c.layout.n {
            break;
        }
    }
    let (main, layout_memo, pick) = match best {
        Some((_reused, layout, memo, i)) => (layout, memo, i),
        None => {
            let (layout, memo, _) =
                compute_layout_memo_with_reach(doc, layout_root, settings, &sizer, editing, None);
            (layout, memo, usize::MAX)
        }
    };
    // 概念图里主概念也是「可摆放的浮动概念」：它的位置同样来自 `pos`（CM-04）。
    // 树布局把它摆在原点附近，这里把整块布局平移到 `pos`，语义与其它概念一致。
    let main = if show_floaters && kind.is_concept_map() {
        concept_root_shift(doc, main)
    } else {
        main
    };

    // ---- 合并自由主题：各自独立布局后整体平移 ----
    // 布局结果整块搬走而不是克隆：20k 时 `ids` / `boxes` / `edges` 各两万个条目，
    // 三份克隆就是几万次分配（打磨轮 6 实测约 3 ms）。
    //
    // `Layout::index` 只在**真的并进了自由主题**时作废：那次索引会变。一颗浮动主题都没有
    // （20k 树、20k 打字都是这种）时它就是最终索引，直接搬过来——重建一次是两万次 id 克隆
    // 加两万次插入，实测 1.1–1.3 ms（打磨轮 14）。
    let Layout {
        mut ids,
        mut boxes,
        mut edges,
        mut spines,
        bounds: main_bounds,
        index: main_index,
    } = main;
    let main_len = ids.len();
    let mut float_of: Vec<Option<String>> = vec![None; boxes.len()];
    if show_floaters {
        // 按 id 排序：HashMap 的哈希顺序每轮进程都可能变，不排序则摆位不可复现
        let mut floats: Vec<&str> = doc
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none() && n.id != doc.root_id)
            .map(|n| n.id.as_str())
            .collect();
        floats.sort_unstable();
        let mut cursor_y = main_bounds.y;
        let main_right = main_bounds.right();
        for f in floats {
            let fl = compute_layout(doc, f, settings, &sizer, editing);
            let Some(base_box) = fl.find(f).map(|i| fl.boxes[i as usize].rect) else {
                continue;
            };
            let pos = doc.node(f).and_then(|n| n.pos.clone());
            let (dx, dy) = match pos.as_ref() {
                Some(p) => (p.x - base_box.x, p.y - base_box.y),
                None => (
                    main_right + FLOATER_GAP_X - base_box.x,
                    cursor_y - base_box.y,
                ),
            };
            let shift = ids.len() as u32;
            for b in fl.boxes.iter() {
                let mut nb = b.clone();
                nb.rect.x += dx;
                nb.rect.y += dy;
                boxes.push(nb);
                float_of.push(Some(f.to_string()));
            }
            for e in fl.edges.iter() {
                edges.push(Edge {
                    from: e.from + shift,
                    to: e.to + shift,
                    start: Point::new(e.start.x + dx, e.start.y + dy),
                    end: Point::new(e.end.x + dx, e.end.y + dy),
                    orientation: e.orientation,
                });
            }
            for p in fl.spines.iter() {
                spines.push(shift_path(p, dx, dy));
            }
            std::sync::Arc::make_mut(&mut ids).extend(fl.ids.iter().cloned());
            if pos.is_none() {
                cursor_y += fl.bounds.h + FLOATER_GAP_Y;
            }
        }
    }
    mark!("layout");
    let index: std::sync::Arc<HashMap<String, u32>> = if ids.len() == main_len {
        mark!("index");
        main_index
    } else {
        let index: HashMap<String, u32> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i as u32))
            .collect();
        mark!("index");
        std::sync::Arc::new(index)
    };

    // ---- 节点 ----
    let reuse_nodes = taken
        .get(pick)
        .map(|m| &m.nodes)
        .filter(|ns| ns.len() == ids.len());
    let mut nodes: Vec<SceneNode> = Vec::with_capacity(ids.len());
    for (i, geom) in boxes.into_iter().enumerate() {
        // 盒子与上一轮一模一样（同一颗节点、同样文字、同样样式）→ 直接搬上一轮的场景节点，
        // 只把位置换成本轮摆出来的。改字的那一颗盒子会变，走下面的完整装配。
        let untouched = reuse_nodes.and_then(|ns| {
            let old = &ns[i];
            (old.geom.rect.w == geom.rect.w
                && old.geom.rect.h == geom.rect.h
                && old.geom.lines.len() == geom.lines.len()
                && old.geom.font_size == geom.font_size)
                .then_some(old)
        });
        if let Some(old) = untouched {
            let mut n = old.clone();
            n.geom = geom;
            nodes.push(n);
            continue;
        }
        // ids 全部来自布局，而布局只从文档取节点：这里必然命中
        let n = doc.node(&ids[i]).expect("场景 id 必须来自文档");
        let st = styles.get(&n.id).copied().unwrap_or(fallback);
        let icon = icon_def(n.icon.as_deref()).map(|d| IconMark {
            glyph: d.glyph,
            color: d.color,
            width: icon_width(st.font_size),
        });
        let (pad_x, pad_y) = geom.pad;
        let mut labels = Vec::new();
        if let Some((pills, _, pill_h)) = sizer.pills(n) {
            let th = text_height(geom.lines.len(), geom.line_height, pad_y);
            let mut x = pad_x;
            for (text, w) in pills {
                labels.push(LabelPill {
                    rect: Rect::new(x, th + 3.0, w, pill_h),
                    text,
                    font_size: pill_font_size(st.font_size),
                });
                x += w + 6.0;
            }
        }
        // 截断的判据就是「量出来的末行带省略号」：measure_concept 只在真超两行时
        // 才补 '…'，所以这里不会出现「没截也给 tooltip」的假阳性（§5.3.1）。
        let truncated =
            kind.is_concept_map() && geom.lines.last().is_some_and(|l| l.ends_with('…'));
        nodes.push(SceneNode {
            geom,
            style: st,
            has_children: !n.children.is_empty(),
            collapsed: n.collapsed,
            has_note: n.note.as_deref().is_some_and(|s| !s.is_empty()),
            has_link: n.link.as_deref().is_some_and(|s| !s.is_empty()),
            truncated,
            has_card: n.card_id.as_deref().is_some_and(|c| !c.trim().is_empty()),
            image: n.image.clone().filter(|s| !s.trim().is_empty()),
            parent: n.parent_id.as_deref().and_then(|p| index.get(p).copied()),
            text_x: pad_x + icon.map(|i| i.width).unwrap_or(0.0),
            center_text: kind.is_concept_map(),
            icon,
            labels,
            float_root: float_of[i].as_deref().and_then(|f| index.get(f).copied()),
            alpha: 1.0,
        });
    }

    mark!("nodes");
    let mut draw_order: Vec<u32> = (0..nodes.len() as u32).collect();
    draw_order.sort_by(|&a, &b| {
        let x = &nodes[a as usize].geom;
        let y = &nodes[b as usize].geom;
        x.depth
            .cmp(&y.depth)
            .then(x.rect.y.total_cmp(&y.rect.y))
            .then(x.rect.x.total_cmp(&y.rect.x))
    });

    mark!("order");
    // ---- 连线：颜色取子节点分支色，深度 ≤ 1 用粗线 ----
    let scene_edges: Vec<SceneEdge> = edges
        .iter()
        .map(|e| {
            let child = &nodes[e.to as usize];
            let (w0, w1) = theme.edge_widths(child.geom.depth);
            SceneEdge {
                from: e.from,
                to: e.to,
                start: e.start,
                end: e.end,
                orientation: e.orientation,
                width: w0,
                width_end: w1,
                shape: EdgeShape::from_name(&settings.edge_shape),
                arrow: settings.edge_arrow,
                color: child.style.branch_color,
                alpha: 1.0,
            }
        })
        .collect();

    // ---- 轴线：主题强调色，与一级分支同粗 ----
    let scene_spines: Vec<SceneSpine> = spines
        .iter()
        .map(|p| SceneSpine {
            path: p.clone(),
            color: theme.ui.accent,
            width: theme.edge_width_root,
            alpha: 1.0,
        })
        .collect();

    mark!("edges");
    // ---- 高级元素：按文档顺序遍历，输出与 HashMap 顺序无关 ----
    let mut boundaries = Vec::new();
    let mut summaries = Vec::new();
    let mut relationships = Vec::new();
    // ---- 概念分组框（CM-18）----
    //
    // 矩形由成员包围盒 + 内距派生，不进文件：成员一动，框就跟着变。
    // 只有概念图有分组；思维导图即使文件里带着 `groups` 也不画（换布局后残留的）。
    let mut groups = Vec::new();
    if doc.is_concept_map() {
        for g in &doc.groups {
            let Some(rect) = union_of(doc, &nodes, &index, &g.members) else {
                continue;
            };
            let rect = rect.expanded(GROUP_PAD);
            let label = if g.label.trim().is_empty() {
                "分组".to_string()
            } else {
                g.label.trim().to_string()
            };
            groups.push(SceneGroup {
                id: g.id.clone(),
                label,
                rect,
                color: g
                    .color
                    .as_deref()
                    .and_then(Rgba::parse)
                    .unwrap_or(theme.ui.dim),
                // 标签压在框的上沿外侧：框里是概念，字放进去会压到盒子。
                label_at: Point::new(rect.x + 8.0, rect.y - GROUP_LABEL_H - 4.0),
            });
        }
    }
    // 20k 的文档里绝大多数节点没有任何高级元素。先扫一遍（不分配、命中即停），
    // 全空就整段跳过：`doc_order()` 自己就要克隆两万个 id，这一跳省得最值。
    let has_advanced = doc.nodes.values().any(|n| {
        !n.boundaries.is_empty() || !n.summaries.is_empty() || !n.relationships.is_empty()
    });
    // 跨分支取色（docs/30 §5.4.1）：分支号只有一份来源——`concept_rows` 的生成树，
    // 与大纲、导出用的是同一份。在这里另算一遍，画布与大纲就会各说各话。
    let branches: HashMap<String, u32> = if kind.is_concept_map()
        && has_advanced
        && doc.nodes.values().any(|n| !n.relationships.is_empty())
    {
        crate::concept_tree::concept_rows(doc)
            .into_iter()
            .map(|r| (r.id, r.branch))
            .collect()
    } else {
        HashMap::new()
    };
    if has_advanced {
        // 车道分组必须**跨源节点**收集（`docs/30` §5.4）：反向命题 B→A 挂在 B 的
        // `relationships` 里。按源节点各分各的组，A→B 与 B→A 就各自成「组内一条」，
        // 两条都走零偏移直线、画成完全重合的一条——C14 当年就是栽在这里。
        // 只取一次 `doc_order()`：两次收集共用同一份 id 表，免得大图多克隆一遍。
        let owner_ids = doc.doc_order();
        let concept_lanes: HashMap<(u32, u32), Vec<(&str, f32)>> = if kind.is_concept_map() {
            let mut lanes: HashMap<(u32, u32), Vec<(&str, f32)>> = HashMap::new();
            for owner_id in &owner_ids {
                let Some(owner) = doc.node(owner_id) else {
                    continue;
                };
                let Some(from) = index.get(owner_id.as_str()).copied() else {
                    continue;
                };
                for r in owner.relationships.iter() {
                    let Some(to) = index.get(&r.to).copied() else {
                        continue;
                    };
                    let pair = if from <= to { (from, to) } else { (to, from) };
                    // C21：短语宽现在就量（车道带的自适应分配要按实测盒长算），
                    // 主循环直接取这份宽度，不再量第二遍。
                    let label_w = r
                        .label
                        .as_deref()
                        .map(|l| {
                            measure
                                .measure(l, REL_LABEL_FONT, WEIGHT_REGULAR, NO_WRAP)
                                .width
                                + 12.0
                        })
                        .unwrap_or(0.0);
                    lanes
                        .entry(pair)
                        .or_default()
                        .push((r.id.as_str(), label_w));
                }
            }
            lanes
        } else {
            HashMap::new()
        };
        // C21：按无向对算一次短语切线偏移。≥3 条车道时按实测短语宽自适应净空；
        // ≤2 条仍走 C15 的旧公式（金样 `cm-parallel-select-a/b` 逐字节不动）。
        let concept_lane_steps: HashMap<(u32, u32), Vec<f32>> = concept_lanes
            .iter()
            .map(|(&pair, lanes)| {
                let ca = nodes[pair.0 as usize].geom.rect.center();
                let cb = nodes[pair.1 as usize].geom.rect.center();
                let (dx, dy) = (cb.x - ca.x, cb.y - ca.y);
                let l = (dx * dx + dy * dy).sqrt().max(1.0);
                let widths: Vec<f32> = lanes.iter().map(|(_, w)| *w).collect();
                (pair, proposition_label_offsets(&widths, (dx / l, dy / l)))
            })
            .collect();
        for owner_id in &owner_ids {
            let Some(owner) = doc.node(owner_id) else {
                continue;
            };
            let owner_idx = index.get(owner_id.as_str()).copied();
            let owner_style = styles.get(owner_id.as_str()).copied().unwrap_or(fallback);

            for b in owner.boundaries.iter() {
                let Some(rect) = union_of(doc, &nodes, &index, &b.targets) else {
                    continue;
                };
                boundaries.push(SceneBoundary {
                    id: b.id.clone(),
                    owner: owner_idx.unwrap_or_default(),
                    rect: rect.expanded(BOUNDARY_PAD),
                    color: Rgba::parse_opt(&b.color).unwrap_or(owner_style.branch_color),
                    alpha: 1.0,
                });
            }

            for s in owner.summaries.iter() {
                let Some(rect) = union_of(doc, &nodes, &index, &s.targets) else {
                    continue;
                };
                let Some(owner_rect) = owner_idx.map(|i| nodes[i as usize].geom.rect) else {
                    continue;
                };
                let side = if rect.x + rect.w / 2.0 >= owner_rect.center().x {
                    Side::Right
                } else {
                    Side::Left
                };
                let dir = side.sign();
                let text = if s.text.trim().is_empty() {
                    "概要".to_string()
                } else {
                    s.text.trim().to_string()
                };
                let m = measure.measure(&text, SUMMARY_FONT, WEIGHT_REGULAR, SUMMARY_MAX_W);
                let tw = (m.width + 20.0).max(44.0);
                let th = (m.lines.len() as f32 * m.line_height + 12.0).max(26.0);
                let brace_x = if side == Side::Right {
                    rect.right() + 10.0
                } else {
                    rect.x - 10.0
                };
                let mid = rect.y + rect.h / 2.0;
                let tip_x = brace_x + dir * 13.0;
                let tx = if side == Side::Right {
                    tip_x + 6.0
                } else {
                    tip_x - 6.0 - tw
                };
                let topic = Rect::new(tx, mid - th / 2.0, tw, th);
                let mut connector = Path::new(Point::new(tip_x, mid));
                connector.line(Point::new(
                    if side == Side::Right { tx } else { tx + tw },
                    mid,
                ));
                summaries.push(SceneSummary {
                    id: s.id.clone(),
                    owner: owner_idx.unwrap_or_default(),
                    text,
                    side,
                    brace: brace_path(brace_x, rect.y, rect.bottom(), side),
                    topic,
                    connector,
                    color: owner_style.branch_color,
                    alpha: 1.0,
                });
            }

            // 同一对概念之间的多条命题：组来自上面的 `concept_lanes`（跨源节点），
            // 这里只按本节点的每条边查它在组内的序号。
            for r in owner.relationships.iter() {
                let (Some(from), Some(to)) = (owner_idx, index.get(&r.to).copied()) else {
                    continue;
                };
                let fb = nodes[from as usize].geom.rect;
                let tb = nodes[to as usize].geom.rect;
                let p1 = clip_toward(fb, tb.center());
                let p2 = clip_toward(tb, fb.center());
                // 车道偏移：0 条/1 条时严格为零（黄金基线逐字节不动），多条时按序扇开。
                let pair = if from <= to { (from, to) } else { (to, from) };
                let (lk, ln, lane_w) = concept_lanes
                    .get(&pair)
                    .and_then(|lanes| {
                        let k = lanes.iter().position(|(id, _)| *id == r.id.as_str())?;
                        Some((k, lanes.len(), Some(lanes[k].1)))
                    })
                    .unwrap_or((0, 1, None));
                // 规范方向（小索引 → 大索引）的切线与法向：反向命题也用同一套，
                // 否则 A→B 与 B→A 各自把正方向算反，偏移互相抵消。
                let (ca, cb) = if from <= to {
                    (fb.center(), tb.center())
                } else {
                    (tb.center(), fb.center())
                };
                let (cdx, cdy) = (cb.x - ca.x, cb.y - ca.y);
                let cl = (cdx * cdx + cdy * cdy).sqrt().max(1.0);
                let (ux, uy) = (cdx / cl, cdy / cl);
                let lane_off = if ln > 1 {
                    let dx = p2.x - p1.x;
                    let dy = p2.y - p1.y;
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    // 短边长不了那么多：间距随长度收窄，免得短边上拱成大弧。
                    let gap = (len / 6.0).min(CONCEPT_LANE_GAP);
                    let shift = (lk as f32 - (ln as f32 - 1.0) * 0.5) * gap;
                    Point::new(-uy * shift, ux * shift)
                } else {
                    Point::new(0.0, 0.0)
                };
                let (path, arrow, mid) = if kind.is_concept_map() {
                    bowed_straight(p1, p2, lane_off)
                } else {
                    curve_between(p1, p2)
                };
                // 短语沿切线再错开：竖边的法向是水平的，只靠车道扇开时两条 36 宽的
                // 短语会横向压在一起（车道组内序号越靠后，短语越往终点方向挪）。
                // C21：≥3 条车道时偏移按短语实测盒长自适应（相邻盒净空 ≥ 24）；
                // ≤2 条仍走 C15 的旧公式，金样逐字节不动。
                let label_at = if ln > 1 {
                    let step = concept_lane_steps
                        .get(&pair)
                        .and_then(|steps| steps.get(lk))
                        .copied()
                        .unwrap_or(0.0);
                    Point::new(mid.x + ux * step, mid.y + uy * step)
                } else {
                    mid
                };
                let label_w = lane_w
                    .or_else(|| {
                        r.label.as_deref().map(|l| {
                            measure
                                .measure(l, REL_LABEL_FONT, WEIGHT_REGULAR, NO_WRAP)
                                .width
                                + 12.0
                        })
                    })
                    .unwrap_or(0.0);
                // ⑦ 端点箭头：`arrow` 恒指终点端，`arrow_back` 是它的镜像（起点端）。
                let mirror = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
                let back = mirror_tri(arrow, mirror);
                let (arrow, arrow_back) = match r.head {
                    ArrowHead::Off => (None, None),
                    ArrowHead::Forward => (Some(arrow), None),
                    ArrowHead::Backward => (None, Some(back)),
                    ArrowHead::Both => (Some(arrow), Some(back)),
                };
                relationships.push(SceneRelationship {
                    id: r.id.clone(),
                    from,
                    to,
                    label: r.label.clone(),
                    path,
                    arrow,
                    arrow_back,
                    start: p1,
                    label_at,
                    label_w,
                    // 显式色 > 跨分支取来源分支色 > 默认 dim（§5.4.1）。颜色只在绘制层
                    // 推导，不写回 relationships[].color：落盘就成了用户数据。
                    color: r
                        .color
                        .as_deref()
                        .and_then(Rgba::parse)
                        .unwrap_or_else(|| concept_rel_color(&branches, &ids, from, to, theme)),
                    alpha: 1.0,
                    dashed: r.dashed,
                    width: r.width_css(),
                });
            }
        }
    }

    // 交叉微弧（docs/30 §5.4.1）：两条直线在屏幕上相交时，后来者弯一次、
    // 先来的保持直线——画成纯 X 就看不出谁压谁，也看不出还有第二条线。
    if kind.is_concept_map() {
        bow_crossings(&mut relationships);
    }

    // ---- 全局包围盒 ----
    let mut enc = Enclosing::new();
    for nd in nodes.iter() {
        enc.add(nd.geom.rect);
    }
    for s in summaries.iter() {
        enc.add(s.topic);
    }
    let bounds = enc
        .rect()
        .unwrap_or(Rect::new(-10.0, -10.0, 20.0, 20.0))
        .expanded(BOUND_PAD);

    mark!("advanced");
    // 同 `compute_layout`：最后一个里程碑之后不再读 `mark_at`。
    let _ = &mark_at;
    // 场景与「下一轮的账」共享同一份节点表：以前每帧 `nodes.clone()` 是两万个
    // `SceneNode` 的整份复制（M12f），现在只加一次引用计数。
    let memo_nodes = std::sync::Arc::new(nodes);
    let scene = Scene {
        ids,
        index,
        nodes: memo_nodes.clone(),
        draw_order,
        edges: scene_edges,
        spines: scene_spines,
        boundaries,
        groups,
        summaries,
        relationships,
        bounds,
        kind,
        focus_note: None,
    };
    // 存账：下一轮只改一个字时，`resolve_all` 与两万次测量查表都能跳过。
    cache.put_memo(
        SceneMemo {
            theme: theme.clone(),
            root: layout_root.to_string(),
            content_rev: doc.content_rev,
            settings_fp,
            style_fp,
            styles: styles.clone(),
            layout: layout_memo,
            nodes: memo_nodes,
        },
        // 上一代留给「撤销」：两代都在时取没被选中的那份；只有一代时就是**这一轮建账时用的**
        // 那份——内容指纹不含文字，「改了一个字」时它照样算「匹配」，可它代表的正是改之前的
        // 内容，撤销要退回的就是它（打磨轮 15 当场测出来：不留它，撤销永远命中不了）。
        {
            // 先留「没被选中」的那一代（它代表这份内容的**另一个**真实状态）；
            // 没有别的可留时，留**最近**的一代——哪怕它这轮指纹对不上（结构刚变过），
            // 撤销退回到它那份内容时正好用得上。
            let mut slots: Vec<Option<SceneMemo>> = taken.into_iter().map(Some).collect();
            // 还有别的代可留时，退役被选中的那份（它已被新账取代）；
            // 只剩它自己时留着——它就是「改之前」那份内容（打磨轮 15 第一版丢过它，单测当场红）。
            if slots.len() > 1 {
                if let Some(slot) = slots.get_mut(pick) {
                    *slot = None;
                }
            }
            slots.into_iter().flatten().next()
        },
    );
    if perf {
        let total: f64 = marks.iter().map(|m| m.1).sum();
        let parts: Vec<String> = marks.iter().map(|(l, ms)| format!("{l}={ms:.2}")).collect();
        eprintln!(
            "[scene] n={} nodes={} edges={} total={total:.2} {}",
            scene.ids.len(),
            scene.nodes.len(),
            scene.edges.len(),
            parts.join(" ")
        );
    }
    scene
}

/// 自由主题整体平移：轴线也要跟着走。
/// 概念图：把主概念的布局整体平移到它的 `pos`（没有 `pos` 就原样返回）。
///
/// 概念图的主概念不是「树根」，而是第一个概念——它和其它概念一样可以被拖。
fn concept_root_shift(doc: &Document, main: Layout) -> Layout {
    let Some(p) = doc.node(&doc.root_id).and_then(|n| n.pos.as_ref()) else {
        return main;
    };
    let Some(i) = main.find(&doc.root_id) else {
        return main;
    };
    let base = main.boxes[i as usize].rect;
    let (dx, dy) = (p.x - base.x, p.y - base.y);
    if dx.abs() < 0.01 && dy.abs() < 0.01 {
        return main;
    }
    let mut boxes = main.boxes;
    for b in boxes.iter_mut() {
        b.rect.x += dx;
        b.rect.y += dy;
    }
    let mut edges = main.edges;
    for e in edges.iter_mut() {
        e.start.x += dx;
        e.start.y += dy;
        e.end.x += dx;
        e.end.y += dy;
    }
    let spines = main.spines.iter().map(|s| shift_path(s, dx, dy)).collect();
    let bounds = Rect::new(
        main.bounds.x + dx,
        main.bounds.y + dy,
        main.bounds.w,
        main.bounds.h,
    );
    Layout {
        ids: main.ids,
        index: main.index,
        boxes,
        edges,
        spines,
        bounds,
    }
}

fn shift_path(p: &Path, dx: f32, dy: f32) -> Path {
    let mut out = Path::new(Point::new(p.start.x + dx, p.start.y + dy));
    let at = |q: Point| Point::new(q.x + dx, q.y + dy);
    for seg in p.segs.iter() {
        match *seg {
            PathSeg::Line { to } => out.line(at(to)),
            PathSeg::Quad { ctrl, to } => out.quad(at(ctrl), at(to)),
            PathSeg::Cubic { c1, c2, to } => out.cubic(at(c1), at(c2), at(to)),
        }
    }
    out
}

/// 目标节点「自身 + 可见子孙」的并集。折叠节点的子树不计入（画布上看不见）。
/// 目标全部落在布局之外（例如整支被折叠）时返回 `None`。
fn union_of(
    doc: &Document,
    nodes: &[SceneNode],
    index: &HashMap<String, u32>,
    targets: &[String],
) -> Option<Rect> {
    let mut enc = Enclosing::new();
    for t in targets {
        let mut stack: Vec<&str> = vec![t.as_str()];
        while let Some(id) = stack.pop() {
            if let Some(i) = index.get(id) {
                enc.add(nodes[*i as usize].geom.rect);
            }
            let Some(n) = doc.node(id) else { continue };
            if n.collapsed {
                continue;
            }
            for c in n.children.iter().rev() {
                stack.push(c.as_str());
            }
        }
    }
    enc.rect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::PathSeg;
    use crate::model::StyleOverride;
    use crate::text::MonoMeasurer;
    use crate::tfix::{simple, Builder};
    use crate::theme::{self, Shape};

    fn snow() -> Theme {
        theme::get_theme("snow")
    }

    fn settings(kind: &str) -> LayoutSettings {
        LayoutSettings {
            kind: kind.to_string(),
            direction: "auto".to_string(),
            edge_shape: "curve".to_string(),
            edge_arrow: false,
            sibling_gap: 24.0,
            level_gap: 80.0,
            max_node_width: 300.0,
            focus_question: None,
        }
    }

    fn scene(doc: &Document, theme: &Theme, kind: &str) -> Scene {
        build_scene(
            doc,
            &doc.root_id.clone(),
            &settings(kind),
            theme,
            &MonoMeasurer,
            None,
        )
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    /// 概念图：同一对概念之间的多条命题各走一条车道，不叠成一条线。
    ///
    /// 多对多关系是概念图的基本能力（`docs/30` §5.4）。模型层放开平行命题之后，
    /// 若绘制层还画同一条直线，两条线完全重合：用户看不出有几条，也点不中第二条。
    #[test]
    fn parallel_propositions_between_one_pair_take_separate_lanes() {
        let mut doc = Builder::root("产品定位")
            .float("a", "甲概念", Some((-220.0, 0.0)))
            .float("b", "乙概念", Some((220.0, 0.0)))
            .relationship("a", "r1", "b", Some("导致"))
            .relationship("a", "r2", "b", Some("支持"))
            .build();
        doc.meta.layout.kind = "conceptMap".into();
        let s = scene(&doc, &snow(), "conceptMap");
        let rels: Vec<_> = s.relationships.iter().collect();
        assert_eq!(rels.len(), 2, "两条命题都要进场景");
        let d = ((rels[0].label_at.x - rels[1].label_at.x).powi(2)
            + (rels[0].label_at.y - rels[1].label_at.y).powi(2))
        .sqrt();
        assert!(
            d >= 8.0,
            "平行命题的短语锚点必须分开（实际相距 {d:.2}），否则两条线重合、第二条点不中"
        );
        // 两条都必须是曲线段：还是 `Line` 就等于还是同一条直线。
        for r in rels.iter() {
            assert!(
                r.path
                    .segs
                    .iter()
                    .any(|s| matches!(s, PathSeg::Quad { .. })),
                "平行命题必须走各自的车道曲线"
            );
        }
    }

    /// 跨线避让（CM-03）不能把车道曲线重弯回弦线。
    ///
    /// `bow_crossings` 只看端点弦线：一条已经拱起的车道线如果和别的线相交，
    /// 它会按弦线重新生成一条曲线，把 C14 的车道偏移整个抹掉。
    #[test]
    fn crossing_bow_leaves_a_lane_curve_alone() {
        let dim = snow().ui.dim;
        let mut straight = Path::new(Point::new(-200.0, 0.0));
        straight.line(Point::new(200.0, 0.0));
        let mut lane = Path::new(Point::new(0.0, -200.0));
        lane.quad(Point::new(80.0, 0.0), Point::new(0.0, 200.0));
        let rel = |id: &str, path: Path| {
            let start = path.start;
            let end = path.end();
            SceneRelationship {
                id: id.into(),
                from: 0,
                to: 1,
                label: Some(id.into()),
                path,
                arrow: None,
                arrow_back: None,
                start,
                label_at: Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0),
                label_w: 20.0,
                color: dim,
                alpha: 1.0,
                dashed: false,
                width: 1.5,
            }
        };
        let before = lane.clone();
        let mut rels = vec![rel("straight", straight), rel("lane", lane)];
        bow_crossings(&mut rels);
        assert_eq!(
            rels[1].path, before,
            "车道曲线不能被跨线避让按弦线重弯（会抹掉车道偏移）"
        );
    }

    /// 反向命题也属于同一对概念：A→B 与 B→A 必须同样分车道。
    ///
    /// C14 的注释写着「否则 A→B 与 B→A … 会叠回一条线」，但当初的车道分组是
    /// **按源节点**做的——反向命题挂在另一个节点的 `relationships` 里，各成一组、
    /// 组内都只有一条，于是两条都走零偏移直线，完全重合。
    #[test]
    fn opposite_propositions_between_the_same_pair_also_take_lanes() {
        let mut doc = Builder::root("产品定位")
            .float("a", "甲概念", Some((-220.0, 0.0)))
            .float("b", "乙概念", Some((220.0, 0.0)))
            .relationship("a", "r1", "b", Some("导致"))
            .relationship("b", "r2", "a", Some("支持"))
            .build();
        doc.meta.layout.kind = "conceptMap".into();
        let s = scene(&doc, &snow(), "conceptMap");
        let rels: Vec<_> = s.relationships.iter().collect();
        assert_eq!(rels.len(), 2, "两条命题都要进场景");
        let d = ((rels[0].label_at.x - rels[1].label_at.x).powi(2)
            + (rels[0].label_at.y - rels[1].label_at.y).powi(2))
        .sqrt();
        assert!(
            d >= 8.0,
            "反向命题的短语锚点必须分开（实际相距 {d:.2}），否则两条线重合、第二条点不中"
        );
        for r in rels.iter() {
            assert!(
                r.path
                    .segs
                    .iter()
                    .any(|s| matches!(s, PathSeg::Quad { .. })),
                "反向命题也必须走各自的车道曲线"
            );
        }
    }

    /// 竖边上的平行命题：只靠车道法向扇开不够。
    ///
    /// 竖边（两个概念的连线接近竖直）的法向是**水平**的，两条车道中心只差 24，
    /// 而短语胶囊宽三十几个世界单位——两个标签会横向压在一起。车道 ≥2 时短语
    /// 还要沿切线错开，让它们在竖直方向也分开。
    #[test]
    fn parallel_proposition_labels_do_not_overlap_on_a_vertical_edge() {
        let mut doc = Builder::root("产品定位")
            .float("a", "甲概念", Some((0.0, -220.0)))
            .float("b", "乙概念", Some((0.0, 220.0)))
            .relationship("a", "r1", "b", Some("导致"))
            .relationship("a", "r2", "b", Some("支持"))
            .build();
        doc.meta.layout.kind = "conceptMap".into();
        let s = scene(&doc, &snow(), "conceptMap");
        let rels: Vec<_> = s.relationships.iter().collect();
        assert_eq!(rels.len(), 2, "两条命题都要进场景");
        // 短语胶囊：中心 label_at、宽 label_w、高 20（与 `mw-render` 的 REL_LABEL_H 一致）。
        let pill = |r: &SceneRelationship| {
            Rect::new(
                r.label_at.x - r.label_w * 0.5,
                r.label_at.y - 10.0,
                r.label_w,
                20.0,
            )
        };
        let (a, b) = (pill(rels[0]), pill(rels[1]));
        let overlap = a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom();
        assert!(
            !overlap,
            "平行命题的短语胶囊叠在一起：{a:?} 与 {b:?}（label_at 相距 ({:.1},{:.1})）",
            (a.center().x - b.center().x).abs(),
            (a.center().y - b.center().y).abs()
        );
    }

    /// 单条命题仍然画直线（基线不许因为车道逻辑漂移）。
    #[test]
    fn a_lone_proposition_still_runs_straight_between_the_boxes() {
        let mut doc = Builder::root("产品定位")
            .float("a", "甲概念", Some((-220.0, 0.0)))
            .float("b", "乙概念", Some((220.0, 0.0)))
            .relationship("a", "r1", "b", Some("导致"))
            .build();
        doc.meta.layout.kind = "conceptMap".into();
        let s = scene(&doc, &snow(), "conceptMap");
        let r = s.relationships.first().expect("一条命题");
        assert!(
            matches!(r.path.segs.as_slice(), [PathSeg::Line { .. }]),
            "单条命题必须仍是单段直线（`--tol 0` 基线靠这一条不动）"
        );
        let PathSeg::Line { to } = r.path.segs[0] else {
            unreachable!()
        };
        let mid = Point::new((r.path.start.x + to.x) / 2.0, (r.path.start.y + to.y) / 2.0);
        assert!(
            (r.label_at.x - mid.x).abs() < 0.01 && (r.label_at.y - mid.y).abs() < 0.01,
            "单条命题的短语锚点必须落在直线中点上"
        );
        assert!(
            (r.path.start.y - to.y).abs() < 0.01,
            "同一水平线上的两个概念之间仍是水平直线"
        );
    }

    /// 概念图：盒子按 `docs/30` §5.3 的规格走，不套思维导图的卡片内距。
    #[test]
    fn concept_boxes_follow_the_node_contract() {
        let mut doc = Builder::root("产品定位")
            .float("a", "用户痛点", Some((-200.0, -100.0)))
            .float("b", "一个相当长的概念名称用来检查换行", Some((220.0, 0.0)))
            .relationship("a", "r1", "b", Some("导致"))
            .build();
        doc.meta.layout.kind = "conceptMap".into();
        let s = scene(&doc, &snow(), "conceptMap");
        assert_eq!(s.kind, LayoutKind::ConceptMap, "场景要记住自己是概念图");
        assert!(s.nodes.len() >= 3);
        // 椭圆盒 = 矩形内接盒 + 外扩（`docs/30` §5.3）。内接盒仍受 96–240 / 36–52 约束，
        // 外扩是比例式的、各有封顶，所以盒子的合法区间要按同一条式子放出来。
        let extra_w = |inner: f32| (ELLIPSE_EXTRA_X * inner).min(ELLIPSE_EXTRA_MAX_X);
        let extra_h = |inner: f32| (ELLIPSE_EXTRA_Y * inner).min(ELLIPSE_EXTRA_MAX_Y);
        for n in s.nodes.iter() {
            let w = n.geom.rect.w;
            assert!(
                w >= CONCEPT_MIN_W - 0.01 && w <= CONCEPT_MAX_W + extra_w(CONCEPT_MAX_W) + 0.01,
                "概念盒宽必须在 96–{} 之间，实际 {w}",
                CONCEPT_MAX_W + extra_w(CONCEPT_MAX_W)
            );
            let h1 = CONCEPT_H1 + extra_h(CONCEPT_H1);
            let h2 = CONCEPT_H2 + extra_h(CONCEPT_H2);
            assert!(
                near(n.geom.rect.h, h1) || near(n.geom.rect.h, h2),
                "概念盒高只能是 {h1}（单行）或 {h2}（双行），实际 {}",
                n.geom.rect.h
            );
            assert!(n.center_text, "概念文字居中");
            assert!(
                near(n.geom.font_size, 14.0) || near(n.geom.font_size, 15.0),
                "概念 14/主概念 15，实际 {}",
                n.geom.font_size
            );
            assert!(n.geom.lines.len() <= 2, "最多两行");
        }
        // 主概念比其余概念大一号。
        let root = s.find(&doc.root_id).expect("主概念要在场景里");
        assert!(near(s.nodes[root as usize].geom.font_size, 15.0));

        // 同一张概念图里，把某颗概念显式改回圆角矩形，它就不该带椭圆外扩。
        // 这条同时守住「缓存键要带形状」：两颗概念的 `concept` / 文字 / 字号全一样，
        // 只有形状不同——`BoxKey` 忘了比形状的话，后测的那颗会直接命中前一颗的尺寸。
        let a = doc
            .nodes
            .iter()
            .find(|(_, n)| n.text == "用户痛点")
            .map(|(id, _)| id.clone())
            .expect("a 要在");
        doc.nodes.get_mut(&a).unwrap().style = Some(StyleOverride {
            shape: Some("rounded".into()),
            ..Default::default()
        });
        let s2 = scene(&doc, &snow(), "conceptMap");
        let na = s2.find(&a).expect("a 要在场景里");
        let wa = s2.nodes[na as usize].geom.rect.w;
        let ha = s2.nodes[na as usize].geom.rect.h;
        assert!(
            wa <= CONCEPT_MAX_W + 0.01 && (near(ha, CONCEPT_H1) || near(ha, CONCEPT_H2)),
            "圆角矩形概念不外扩，实际 {wa}×{ha}"
        );
        let inflated = s2
            .nodes
            .iter()
            .filter(|n| n.geom.rect.w > CONCEPT_MAX_W)
            .count();
        assert!(
            inflated >= 1,
            "同图里椭圆概念要明显比矩形概念宽（外扩要看得见）"
        );
    }

    /// CM-05：删掉主概念换根之后，每个概念的盒子仍落在自己的 `pos` 上。
    /// 换根要是让整块布局再平移一次，图会被甩到视野外——这条断言钉住「只平移一次」。
    #[test]
    fn scene_keeps_each_concept_at_its_pos_after_the_root_swap() {
        let mut doc = Builder::root("甲")
            .float("b", "乙", Some((200.0, 0.0)))
            .float("c", "丙", Some((0.0, 200.0)))
            .build();
        doc.meta.layout.kind = "conceptMap".into();
        let root = doc.root_id.clone();
        doc.apply(&crate::command::Command::MoveConcept {
            id: root.clone(),
            x: 0.0,
            y: 0.0,
        })
        .expect("主概念摆位");
        doc.apply(&crate::command::Command::DeleteNodes { ids: vec![root] })
            .expect("概念图主概念可删");

        let s = scene(&doc, &snow(), "conceptMap");
        for (id, (x, y)) in [("b", (200.0, 0.0)), ("c", (0.0, 200.0))] {
            let i = s.find(id).expect("概念要在场景里");
            let r = s.rect(i);
            assert!(
                (r.x - x).abs() < 0.5 && (r.y - y).abs() < 0.5,
                "{id} 应落在 ({x}, {y})，实际 ({}, {})",
                r.x,
                r.y
            );
        }
    }

    /// 概念图的关系线是直线，思维导图仍是二次曲线——同一份文档、只换 `kind`。
    #[test]
    fn concept_relationships_are_straight_while_mindmap_keeps_curves() {
        let build = || {
            let mut d = Builder::root("概念甲")
                .float("b", "概念乙", Some((240.0, 0.0)))
                .build();
            let root = d.root_id.clone();
            d.apply(&crate::command::Command::AddRelationship {
                from: root,
                to: "b".into(),
                label: Some("导致".into()),
            })
            .expect("关系线要能加上");
            d
        };

        let mut concept = build();
        concept.meta.layout.kind = "conceptMap".into();
        let cs = scene(&concept, &snow(), "conceptMap");
        let r = cs.relationships.first().expect("概念图要有关系线");
        assert!(
            matches!(r.path.segs.as_slice(), [PathSeg::Line { .. }]),
            "概念图只画直线，实际 {:?}",
            r.path.segs
        );
        let end = r.arrow.expect("默认前向箭头")[0];
        assert!(
            near(r.label_at.x, (r.start.x + end.x) / 2.0),
            "短语落在线段中点"
        );
        assert!(near(r.label_at.y, (r.start.y + end.y) / 2.0));

        let mut mind = build();
        mind.meta.layout.kind = "mindmap".into();
        let ms = scene(&mind, &snow(), "mindmap");
        let mr = ms.relationships.first().expect("思维导图也要有关系线");
        assert!(
            matches!(mr.path.segs.as_slice(), [PathSeg::Quad { .. }]),
            "思维导图保持曲线，实际 {:?}",
            mr.path.segs
        );
    }

    #[test]
    fn cached_rebuild_only_measures_the_changed_node() {
        use std::cell::Cell;
        struct Count<'a> {
            inner: &'a dyn TextMeasurer,
            n: Cell<usize>,
        }
        impl TextMeasurer for Count<'_> {
            fn measure(
                &self,
                text: &str,
                fs: f32,
                weight: u16,
                max_w: f32,
            ) -> crate::text::Measured {
                self.n.set(self.n.get() + 1);
                self.inner.measure(text, fs, weight, max_w)
            }
        }
        let mut d = Document::new("根");
        let root = d.root_id.clone();
        for i in 0..300 {
            d.apply(&crate::Command::AddNode {
                parent: root.clone(),
                index: None,
                text: format!("节点 {i}"),
                style: None,
            })
            .unwrap();
        }
        let inner = MonoMeasurer;
        let count = Count {
            inner: &inner,
            n: Cell::new(0),
        };
        let cache = SceneCache::new();
        let theme = snow();
        let settings = settings("logicRight");
        build_scene_cached(&d, &root, &settings, &theme, &count, None, &cache);
        let first = count.n.get();
        assert!(first >= 300, "首帧要测全部节点，实际 {first}");

        let changed = d
            .doc_order()
            .into_iter()
            .find(|id| id != &root)
            .expect("至少一个子节点");
        d.nodes.get_mut(&changed).unwrap().text = "只改这一条".into();
        count.n.set(0);
        build_scene_cached(&d, &root, &settings, &theme, &count, None, &cache);
        let second = count.n.get();
        assert!(
            second * 5 <= first,
            "改一个节点应至少少测 80%：首帧 {first}，第二帧 {second}"
        );
    }

    /// 测量缓存的键要「该失效时失效、不该失效时别失效」：影响盒尺寸的输入
    /// （文字/图标……）一变就得重测，只改颜色则必须原样命中。打磨轮 6 把
    /// `format!` 字符串键换成结构体比较，这条测试是那次改写的护栏——
    /// 键少放一项会让盒子错位，多放一项（比如颜色）会让平移/换色都白重测一遍。
    #[test]
    fn the_box_cache_remeasures_only_when_a_size_input_changes() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        // 从五个字起步：一两个字的盒子会被 `MIN_NODE_W`（88）托底，图标塞进去也不会变宽，
        // 那样测不出键有没有失效。
        let mut doc = Builder::root("中心").child("r", "a", "甲乙丙丁戊").build();
        let width = |doc: &Document, cache: &SceneCache| -> f32 {
            let s = build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                cache,
            );
            let i = s.find("a").expect("孩子必须在场景里");
            s.nodes[i as usize].geom.rect.w
        };
        let base = width(&doc, &cache);
        assert!(near(width(&doc, &cache), base), "同一份输入必须原样命中");

        doc.apply(&crate::Command::SetText {
            id: "a".into(),
            text: "甲".into(),
        })
        .unwrap();
        let narrow = width(&doc, &cache);
        assert!(narrow < base, "换了更短的字必须重测（{base} → {narrow}）");

        doc.apply(&crate::Command::SetText {
            id: "a".into(),
            text: "甲乙丙丁戊".into(),
        })
        .unwrap();
        assert!(near(width(&doc, &cache), base), "改回去要回到原来的宽");

        doc.apply(&crate::Command::SetIcon {
            ids: vec!["a".into()],
            icon: Some("priority-1".into()),
        })
        .unwrap();
        let with_icon = width(&doc, &cache);
        assert!(with_icon > base, "加图标要重测（{base} → {with_icon}）");
        doc.apply(&crate::Command::SetStyle {
            ids: vec!["a".into()],
            style: Some(StyleOverride {
                fill: Some("#FF0000".into()),
                ..Default::default()
            }),
        })
        .unwrap();
        assert!(
            near(width(&doc, &cache), with_icon),
            "只改颜色不许改盒尺寸：{with_icon} → {}",
            width(&doc, &cache)
        );
    }

    /// SE-13：字重进盒键。400 → 600 换的是字面，**字宽也跟着换**；
    /// 不重测就会留下按旧字面量的盒宽，长标题会在 600 下被裁掉一截。
    /// 用一台按字重改宽度的假度量器来钉这条（`MonoMeasurer` 不看字重）。
    #[test]
    fn the_box_cache_remeasures_when_only_the_weight_changes() {
        struct ByWeight;
        impl TextMeasurer for ByWeight {
            fn measure(
                &self,
                text: &str,
                font_size: f32,
                weight: u16,
                _max_width: f32,
            ) -> crate::text::Measured {
                let mut m = MonoMeasurer.measure(text, font_size, weight, NO_WRAP);
                if weight >= crate::theme::WEIGHT_SEMIBOLD {
                    m.width *= 1.25;
                }
                m
            }
        }
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let mut doc = Builder::root("中心").child("r", "a", "甲乙丙丁戊").build();
        let width = |doc: &Document, cache: &SceneCache| -> f32 {
            let s =
                build_scene_cached(doc, &doc.root_id, &settings, &theme, &ByWeight, None, cache);
            let i = s.find("a").expect("孩子必须在场景里");
            s.nodes[i as usize].geom.rect.w
        };
        let regular = width(&doc, &cache);
        doc.apply(&crate::Command::SetStyle {
            ids: vec!["a".into()],
            style: Some(StyleOverride {
                weight: Some(crate::theme::WEIGHT_SEMIBOLD),
                ..Default::default()
            }),
        })
        .unwrap();
        let semibold = width(&doc, &cache);
        assert!(
            semibold > regular,
            "换字重必须重测：{regular} → {semibold}（键里漏了 weight 就会相等）"
        );
    }

    /// BUG-6：内容**完全相同**的两颗一级兄弟交换顺序后，样式指纹必须变。
    ///
    /// 一级主题的颜色来自 `palette_at(兄弟序号)`（`theme.rs` 的 `stack.push((…, i))`），
    /// 而指纹以前把兄弟序号恒传 0、也不混 id。两颗兄弟文字/样式一字不差时，
    /// 交换前后喂进哈希的字节序列一模一样 → 指纹不变 → `SceneMemo.styles` 整份复用
    /// → 调色板颜色留在旧 id 上，与屏幕上新的左右次序对不上（BUG-6）。
    #[test]
    fn swapping_identical_siblings_changes_the_style_fingerprint() {
        let settings = settings("mindmap");
        let mut doc = Builder::root("中心")
            .child("r", "a", "同名")
            .child("r", "b", "同名")
            .build();
        let before = content_fingerprint(&doc, &settings);
        // 把 b 挪到第一位：内容一个字没变，只有次序变了。
        doc.apply(&crate::Command::MoveNode {
            id: "b".into(),
            new_parent: "r".into(),
            index: Some(0),
        })
        .unwrap();
        assert_eq!(
            doc.nodes["r"].children,
            vec!["b".to_string(), "a".to_string()],
            "前提：b 真的排到了第一位"
        );
        let after = content_fingerprint(&doc, &settings);
        assert_ne!(
            before, after,
            "交换同属性兄弟必须换指纹（否则调色板颜色留在旧 id 上）"
        );
    }

    /// BUG-2：只改「最大节点宽」也必须让盒子重算。
    ///
    /// 检查器把 `max_node_width` 从 600 调到 120 时，文档里一个字都没变：`style_fp`
    /// （修复前只吃 `doc`）相同 → `SceneMemo` 与 `LayoutMemo.boxes` 整代复用 → 画布还按
    /// 600 摆，导出路径新建 `SceneCache` 却按 120 量 → 画布与 PNG/SVG/PDF 不一致，
    /// 直到改一个字或重开文件。这条钉住「settings 的盒尺寸输入进指纹」。
    #[test]
    fn changing_max_node_width_alone_invalidates_the_box_cache() {
        let theme = snow();
        let cache = SceneCache::new();
        let doc = Builder::root("中心")
            .child("r", "a", "甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申酉")
            .build();
        let mut wide = settings("mindmap");
        wide.max_node_width = 600.0;
        let mut narrow = wide.clone();
        narrow.max_node_width = 120.0;
        let probe = |settings: &LayoutSettings| -> (f32, usize) {
            let s = build_scene_cached(
                &doc,
                &doc.root_id,
                settings,
                &theme,
                &MonoMeasurer,
                None,
                &cache,
            );
            let i = s.find("a").expect("孩子必须在场景里");
            (
                s.nodes[i as usize].geom.rect.w,
                s.nodes[i as usize].geom.lines.len(),
            )
        };
        let (wide_w, wide_lines) = probe(&wide);
        let (narrow_w, narrow_lines) = probe(&narrow);
        assert!(
            narrow_lines > wide_lines && narrow_w < wide_w,
            "收窄最大节点宽必须重折行并重测盒宽：{wide_lines} 行/{wide_w} → {narrow_lines} 行/{narrow_w}"
        );
        // 调回去要回到原样（不是被窄档的账卡住）。
        let (back_w, back_lines) = probe(&wide);
        assert!(
            near(back_w, wide_w) && back_lines == wide_lines,
            "调回宽档要回到 {wide_lines} 行/{wide_w}，实际 {back_lines} 行/{back_w}"
        );
    }

    /// 命中缓存时行文本共用同一份 `Arc`，不再逐节点克隆——20k 文档每帧两万次
    /// 字符串分配就是打磨轮 10 干掉的那笔钱，也是「改一个字重建」从 46ms 掉到 30ms 的来源。
    #[test]
    fn a_cache_hit_shares_the_line_storage_instead_of_cloning_it() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let mut doc = Builder::root("中心")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .build();
        let lines_of = |doc: &Document, cache: &SceneCache, id: &str| {
            let s = build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                cache,
            );
            let i = s.find(id).expect("节点必须在场景里");
            s.nodes[i as usize].geom.lines.clone()
        };
        let first = lines_of(&doc, &cache, "b");
        // 改另一颗的字：b 没变，必须原样交出上一次那份行数据（指针相同，零分配）
        doc.apply(&crate::Command::SetText {
            id: "a".into(),
            text: "甲甲".into(),
        })
        .unwrap();
        let second = lines_of(&doc, &cache, "b");
        assert!(
            std::sync::Arc::ptr_eq(&first, &second),
            "未变的节点必须复用同一份行数据"
        );
        // 自己变了才换一份新的，内容跟着更新
        doc.apply(&crate::Command::SetText {
            id: "b".into(),
            text: "乙乙".into(),
        })
        .unwrap();
        let third = lines_of(&doc, &cache, "b");
        assert!(!std::sync::Arc::ptr_eq(&second, &third));
        assert_eq!(&*third, &["乙乙".to_string()]);
    }

    /// 结构没变时，id 序列与 id→下标表整份沿用上一轮（只加引用计数）。
    ///
    /// 20k 打字的 collect / bounds 两段都在这里：两万个 String 克隆与两万次哈希插入
    /// 以前每帧白付一次（M12 残账的 Arc 化）。
    #[test]
    fn an_unchanged_structure_reuses_the_scene_ids_and_index() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let mut doc = Builder::root("中心")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .build();
        let build = |doc: &Document, cache: &SceneCache| {
            build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                cache,
            )
        };
        let first = build(&doc, &cache);
        let ids = first.ids.clone();
        let index = first.index.clone();

        doc.apply(&crate::Command::SetText {
            id: "a".into(),
            text: "甲甲".into(),
        })
        .unwrap();
        let second = build(&doc, &cache);
        assert!(
            std::sync::Arc::ptr_eq(&ids, &second.ids),
            "只改字时 id 序列必须整份沿用"
        );
        assert!(
            std::sync::Arc::ptr_eq(&index, &second.index),
            "只改字时 id→下标表必须整份沿用"
        );

        doc.apply(&crate::Command::AddNode {
            parent: "r".into(),
            index: None,
            text: "丙".into(),
            style: None,
        })
        .unwrap();
        let third = build(&doc, &cache);
        assert!(
            !std::sync::Arc::ptr_eq(&ids, &third.ids),
            "结构变了必须换一份新的 id 序列"
        );
    }

    /// 账本复用的前提是「除了文字什么都没变」：给一颗节点加备注必须让下一轮重新装配它，    /// 账本复用的前提是「除了文字什么都没变」：给一颗节点加备注必须让下一轮重新装配它，
    /// 否则会出现「点了备注、画布上不显示」——这类字段全在 `SceneNode` 里，不在盒子里。
    #[test]
    fn the_scene_memo_rebuilds_a_node_whose_flags_changed() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let mut doc = Builder::root("中心").child("r", "a", "甲").build();
        let has_note = |doc: &Document, cache: &SceneCache| {
            let s = build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                cache,
            );
            let i = s.find("a").expect("节点必须在场景里");
            s.nodes[i as usize].has_note
        };
        assert!(!has_note(&doc, &cache), "初始没有备注");
        doc.apply(&crate::Command::SetNote {
            id: "a".into(),
            note: Some("备注".into()),
        })
        .unwrap();
        assert!(has_note(&doc, &cache), "加了备注必须看得见");
    }

    /// 结构一变（多了颗子节点）账就整份作废：新节点必须出现在场景里，位置由本轮重新摆。
    #[test]
    fn a_structure_change_drops_the_layout_memo() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let mut doc = Builder::root("中心").child("r", "a", "甲").build();
        let build = |doc: &Document| {
            build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                &cache,
            )
        };
        let before = build(&doc);
        doc.apply(&crate::Command::AddNode {
            parent: "a".into(),
            index: None,
            text: "乙".into(),
            style: None,
        })
        .unwrap();
        let after = build(&doc);
        assert_eq!(after.nodes.len(), before.nodes.len() + 1);
        assert!(
            after
                .nodes
                .iter()
                .any(|n| n.geom.lines.iter().any(|l| l.contains('乙'))),
            "新节点必须进场景"
        );
    }

    /// 撤销要把内容退回**上一代**，账本只留一代就永远对不上指纹、整份作废
    /// （打磨轮 15：20k 上这是 43ms 与 16ms 的差别）。这条钉住「改字 → 撤销」后沿用上一代的账。
    #[test]
    fn undo_hits_the_previous_generation_of_the_memo() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let mut doc = Builder::root("中心").child("r", "a", "甲").build();
        let build = |doc: &Document| {
            build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                &cache,
            )
        };
        let lines = |s: &Scene, id: &str| {
            let i = s.find(id).expect("节点必须在场景里");
            s.nodes[i as usize].geom.lines.clone()
        };
        let before = build(&doc);
        doc.apply(&crate::Command::SetText {
            id: "a".into(),
            text: "甲乙丙".into(),
        })
        .expect("改字");
        let _changed = build(&doc);
        doc.undo().expect("撤销");
        // 和真实路径一致：`Canvas::undo` 在重建前打招呼「先试上一代」。
        cache.prefer_previous();
        let back = build(&doc);
        assert!(
            std::sync::Arc::ptr_eq(&lines(&before, "a"), &lines(&back, "a")),
            "撤销回到上一代的内容，行数据必须是同一次测量（账本命中），不是重新分配"
        );
        let text = lines(&back, "a").join("");
        assert_eq!(text, "甲", "内容也要真的退回去");
    }

    /// 结构撤销（加一颗再撤掉）也要能复用**上一代**的账：内容指纹里含结构，
    /// 加完节点当前代就作废了，账只留一代的话这一步是全量重建（20k 上 40ms 量级）。
    #[test]
    fn a_structural_undo_reuses_the_previous_generation() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let mut doc = Builder::root("中心").child("r", "a", "甲").build();
        let build = |doc: &Document| {
            build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                &cache,
            )
        };
        let lines = |s: &Scene, id: &str| {
            let i = s.find(id).expect("节点必须在场景里");
            s.nodes[i as usize].geom.lines.clone()
        };
        let before = build(&doc);
        doc.apply(&crate::Command::AddNode {
            parent: "r".into(),
            index: None,
            text: "新的".into(),
            style: None,
        })
        .expect("加一颗");
        let grown = build(&doc);
        assert_eq!(grown.ids.len(), before.ids.len() + 1, "新节点要真的进场景");
        doc.undo().expect("撤销");
        cache.prefer_previous();
        let back = build(&doc);
        assert_eq!(back.ids.len(), before.ids.len(), "撤销后回到原来的节点数");
        assert!(
            std::sync::Arc::ptr_eq(&lines(&before, "a"), &lines(&back, "a")),
            "结构撤销也要沿用上一代的测量（否则这一步就是全量重建）"
        );
    }

    /// 索引只建一次：没有并进自由主题就沿用布局那份（打磨轮 14 把 1.2ms 砍成 0），
    /// 并进了自由主题就必须重建——两条路都得让 `index` 与 `ids` 一一对应。
    /// 这条测试挡的是「无条件沿用」：那样并进来的概念查不到下标，`parent` 会静默变成 `None`。
    #[test]
    fn the_scene_index_agrees_with_ids_whether_floaters_are_merged_or_not() {
        let theme = snow();
        let check = |doc: &Document, kind: &str| {
            let settings = settings(kind);
            let s = build_scene_cached(
                doc,
                &doc.root_id,
                &settings,
                &theme,
                &MonoMeasurer,
                None,
                &SceneCache::new(),
            );
            assert_eq!(
                s.ids.len(),
                s.index.len(),
                "索引与节点表必须一样长（{kind}）"
            );
            for (i, id) in s.ids.iter().enumerate() {
                assert_eq!(
                    s.index.get(id.as_str()).copied(),
                    Some(i as u32),
                    "id「{id}」的下标必须指回自己（{kind}）"
                );
            }
        };
        // 树：没有自由主题 → 直接沿用布局那份索引
        check(
            &Builder::root("中心").child("r", "a", "甲").build(),
            "mindmap",
        );
        // 概念图：浮动概念会被并进 `ids` → 索引必须重建
        let mut doc = Builder::root("中心").child("r", "a", "甲").build();
        doc.apply(&crate::Command::SetLayout {
            layout: crate::LayoutSettings {
                kind: "conceptMap".into(),
                ..crate::LayoutSettings::default()
            },
        })
        .expect("切成概念图");
        doc.apply(&crate::Command::AddFloatingTopic {
            text: "浮动".into(),
            x: 400.0,
            y: 120.0,
        })
        .expect("加浮动概念");
        check(&doc, "conceptMap");
    }

    /// 同一份文档连画两遍：每颗节点的行数据都是同一份 `Arc`（不是逐节点重新测量/分配）。
    #[test]
    fn painting_the_same_document_twice_reuses_every_box() {
        let theme = snow();
        let settings = settings("mindmap");
        let cache = SceneCache::new();
        let doc = Builder::root("中心")
            .child("r", "a", "甲乙")
            .child("r", "b", "丙丁")
            .build();
        let first = build_scene_cached(
            &doc,
            &doc.root_id,
            &settings,
            &theme,
            &MonoMeasurer,
            None,
            &cache,
        );
        let second = build_scene_cached(
            &doc,
            &doc.root_id,
            &settings,
            &theme,
            &MonoMeasurer,
            None,
            &cache,
        );
        assert_eq!(first.nodes.len(), second.nodes.len());
        for (a, b) in first.nodes.iter().zip(second.nodes.iter()) {
            assert!(
                std::sync::Arc::ptr_eq(&a.geom.lines, &b.geom.lines),
                "同一份文档的两轮场景必须共用行数据"
            );
        }
    }

    #[test]
    fn scene_covers_every_visible_node() {
        let s = scene(&simple(), &snow(), "mindmap");
        assert_eq!(s.nodes.len(), 7);
        assert_eq!(s.ids.len(), s.nodes.len());
        for id in ["r", "a", "b", "c", "a1", "a2", "b1"] {
            assert!(s.find(id).is_some(), "{id} 应进入场景");
        }
    }

    #[test]
    fn root_and_branches_are_rounded_cards() {
        let s = scene(&simple(), &snow(), "mindmap");
        assert_eq!(s.node(s.find("r").unwrap()).style.shape, Shape::Rounded);
        assert_eq!(s.node(s.find("a").unwrap()).style.shape, Shape::Rounded);
    }

    #[test]
    fn edges_are_colored_by_the_child_branch() {
        let theme = snow();
        let s = scene(&simple(), &theme, "logicRight");
        for e in s.edges.iter() {
            let child = s.node(e.to);
            assert_eq!(e.color, child.style.branch_color);
            let (w0, w1) = theme.edge_widths(child.geom.depth);
            assert_eq!((e.width, e.width_end), (w0, w1));
            assert!(e.width_end <= e.width, "锥形分支线只能由父端向子端收细");
        }
    }

    #[test]
    fn draw_order_puts_parents_before_children() {
        let s = scene(&simple(), &snow(), "mindmap");
        let pos: HashMap<u32, usize> = s
            .draw_order
            .iter()
            .enumerate()
            .map(|(p, i)| (*i, p))
            .collect();
        for e in s.edges.iter() {
            assert!(pos[&e.from] <= pos[&e.to], "父节点应先于子节点绘制");
        }
    }

    #[test]
    fn collapsed_children_leave_the_scene_but_keep_the_badge() {
        let d = Builder::root("根")
            .collapsed("r", "a", "甲")
            .child("a", "a1", "乙")
            .child("r", "b", "丙")
            .build();
        let s = scene(&d, &snow(), "logicRight");
        assert_eq!(s.nodes.len(), 3);
        let a = s.node(s.find("a").unwrap());
        assert!(a.collapsed && a.has_children);
    }

    #[test]
    fn floating_topics_are_placed_to_the_right_of_the_main_tree() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .float("f", "自由主题", None)
            .under("f", "f1", "子")
            .build();
        let s = scene(&d, &snow(), "logicRight");
        let main_right = s.rect(s.find("a").unwrap()).right();
        let f = s.node(s.find("f").unwrap());
        assert!(f.geom.rect.x > main_right);
        assert_eq!(f.float_root, s.find("f"));
        assert_eq!(s.node(s.find("f1").unwrap()).float_root, s.find("f"));
        assert_eq!(s.node(s.find("a").unwrap()).float_root, None);
        assert!(s.bounds.right() >= f.geom.rect.right());
    }

    #[test]
    fn floating_topic_honours_its_stored_position() {
        let d = Builder::root("根")
            .float("f", "自由", Some((900.0, -300.0)))
            .build();
        let s = scene(&d, &snow(), "logicRight");
        let r = s.rect(s.find("f").unwrap());
        assert_eq!((r.x, r.y), (900.0, -300.0));
    }

    #[test]
    fn a_group_box_wraps_its_members_with_padding() {
        let mut d = Builder::root("根")
            .float("a", "甲", Some((-120.0, 0.0)))
            .float("b", "乙", Some((140.0, 60.0)))
            .build();
        d.meta.layout.kind = "conceptMap".into();
        d.groups.push(crate::model::ConceptGroup {
            id: "g1".into(),
            label: "  ".into(),
            members: vec!["a".into(), "b".into()],
            color: None,
        });
        let s = build_scene(
            &d,
            &d.root_id.clone(),
            &settings("conceptMap"),
            &snow(),
            &MonoMeasurer,
            None,
        );
        assert_eq!(s.groups.len(), 1);
        let g = &s.groups[0];
        assert_eq!(g.label, "分组", "空标签回落到「分组」");
        for m in ["a", "b"] {
            let idx = s.find(m).expect("成员要在场景里");
            let r = s.node(idx).geom.rect;
            assert!(
                g.rect.x <= r.x + 0.01
                    && g.rect.y <= r.y + 0.01
                    && g.rect.right() >= r.right() - 0.01
                    && g.rect.bottom() >= r.bottom() - 0.01,
                "组框要包住 {m}"
            );
        }
        // 标签压在框的上沿外侧，不进框里。
        assert!(g.label_at.y < g.rect.y);
    }

    #[test]
    fn floaters_are_hidden_when_focused_on_a_subtree() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("a", "a1", "乙")
            .float("f", "自由", Some((500.0, 0.0)))
            .build();
        let s = build_scene(
            &d,
            "a",
            &settings("logicRight"),
            &snow(),
            &MonoMeasurer,
            None,
        );
        assert_eq!(s.find("f"), None);
        assert!(s.find("a1").is_some());
    }

    #[test]
    fn axis_layouts_expose_theme_colored_spines() {
        let theme = snow();
        let s = scene(&simple(), &theme, "fishbone");
        assert_eq!(s.spines.len(), 1);
        assert_eq!(s.spines[0].color, theme.ui.accent);
        assert_eq!(s.spines[0].width, theme.edge_width_root);
        assert!(s.spines[0].path.end().x < s.rect(s.find("r").unwrap()).x);
        assert!(scene(&simple(), &theme, "logicRight").spines.is_empty());
    }

    #[test]
    fn floating_topic_spines_move_with_the_topic() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .float("f", "自由", Some((900.0, -300.0)))
            .under("f", "f1", "子")
            .build();
        let s = scene(&d, &snow(), "fishbone");
        assert_eq!(s.spines.len(), 2);
        let f = s.rect(s.find("f").unwrap());
        assert!(
            s.spines
                .iter()
                .any(|sp| near(sp.path.start.x, f.x) && near(sp.path.start.y, f.center().y)),
            "自由主题的脊线应随主题一起平移"
        );
    }

    #[test]
    fn boundary_wraps_union_of_targets() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .child("r", "c", "丙")
            .child("a", "a1", "丁")
            .boundary("r", "bnd1", &["a", "b"])
            .build();
        let s = scene(&d, &snow(), "logicRight");
        assert_eq!(s.boundaries.len(), 1);
        let b = &s.boundaries[0];
        let mut enc = Enclosing::new();
        for id in ["a", "a1", "b"] {
            enc.add(s.rect(s.find(id).unwrap()));
        }
        assert_eq!(b.rect, enc.rect().unwrap().expanded(BOUNDARY_PAD));
        assert_eq!(b.owner, s.find("r").unwrap());
        assert_eq!(b.color, s.node(s.find("r").unwrap()).style.branch_color);
    }

    #[test]
    fn boundary_skips_collapsed_subtrees() {
        let d = Builder::root("根")
            .collapsed("r", "a", "甲")
            .child("a", "a1", "乙")
            .child("r", "b", "乙")
            .boundary("r", "bnd", &["a"])
            .build();
        let s = scene(&d, &snow(), "logicRight");
        let b = &s.boundaries[0];
        assert_eq!(b.rect, s.rect(s.find("a").unwrap()).expanded(BOUNDARY_PAD));
    }

    #[test]
    fn summary_brace_sits_outside_the_targets() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .summary("r", "sum1", &["a", "b"], "小结")
            .build();
        let s = scene(&d, &snow(), "logicRight");
        assert_eq!(s.summaries.len(), 1);
        let sum = &s.summaries[0];
        assert_eq!(sum.side, Side::Right);
        let mut enc = Enclosing::new();
        for id in ["a", "b"] {
            enc.add(s.rect(s.find(id).unwrap()));
        }
        let targets = enc.rect().unwrap();
        assert!(near(sum.brace.start.x, targets.right() + 10.0));
        assert!(sum.topic.x > sum.brace.start.x);
        assert!(near(
            sum.topic.y + sum.topic.h / 2.0,
            targets.y + targets.h / 2.0
        ));
        assert_eq!(sum.text, "小结");
    }

    #[test]
    fn summary_on_the_left_flips_direction() {
        // 放射式贪心分配：a 去左列、b 去右列（两个等高兄弟，顺序确定）
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .summary("r", "s", &["a"], "左")
            .build();
        let s = scene(&d, &snow(), "mindmap");
        assert!(s.rect(s.find("a").unwrap()).x < s.rect(s.find("r").unwrap()).x);
        let sum = &s.summaries[0];
        assert_eq!(sum.side, Side::Left);
        assert!(
            sum.topic.right() < sum.brace.start.x,
            "摘要主题应在括号左侧"
        );
    }

    #[test]
    fn empty_summary_text_falls_back_to_placeholder() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .summary("r", "s", &["a"], "   ")
            .build();
        let s = scene(&d, &snow(), "logicRight");
        assert_eq!(s.summaries[0].text, "概要");
    }

    #[test]
    fn relationship_connects_box_edges_not_centers() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .relationship("a", "rel1", "b", Some("关联"))
            .build();
        let s = scene(&d, &snow(), "logicRight");
        assert_eq!(s.relationships.len(), 1);
        let rel = &s.relationships[0];
        let fa = s.rect(s.find("a").unwrap());
        let tb = s.rect(s.find("b").unwrap());
        assert!(fa.expanded(0.5).contains(rel.start));
        assert!(tb
            .expanded(0.5)
            .contains(rel.arrow.expect("默认前向箭头")[0]));
        assert!(rel.start.dist(fa.center()) > 1.0, "端点应落在盒边而非盒心");
        let PathSeg::Quad { ctrl, to } = rel.path.segs[0] else {
            panic!("关系线应为二次曲线");
        };
        let expect = Point::new(
            0.25 * rel.start.x + 0.5 * ctrl.x + 0.25 * to.x,
            0.25 * rel.start.y + 0.5 * ctrl.y + 0.25 * to.y,
        );
        assert_eq!(rel.label_at, expect);
        assert!(rel.label_w > 0.0);
    }

    #[test]
    fn relationship_to_a_hidden_target_is_dropped() {
        let d = Builder::root("根")
            .collapsed("r", "a", "甲")
            .child("a", "a1", "乙")
            .relationship("r", "rel", "a1", None)
            .build();
        // a1 被折叠隐藏：关系线没有端点，整条不画
        let s = scene(&d, &snow(), "logicRight");
        assert!(s.relationships.is_empty());
    }

    /// X25 G-25-01：主题级内距要同时改盒子与标签落位，两处不能各算一套。
    #[test]
    fn a_theme_level_padding_widens_the_box_and_moves_labels() {
        let d = Builder::root("根")
            .child("r", "a", "甲甲甲甲")
            .labels("a", &["标签"])
            .build();
        let base = scene(&d, &snow(), "logicRight");
        let mut padded = snow();
        padded.levels[1].pad = Some((30.0, 15.0));
        let wide = scene(&d, &padded, "logicRight");
        let a = base.node(base.find("a").unwrap());
        let b = wide.node(wide.find("a").unwrap());
        assert_eq!(b.geom.pad, (30.0, 15.0));
        let dw = b.geom.rect.w - a.geom.rect.w;
        let dh = b.geom.rect.h - a.geom.rect.h;
        assert!((dw - 24.0).abs() < 0.01, "dw={dw}");
        assert!((dh - 10.0).abs() < 0.01, "dh={dh}");
        assert!((b.labels[0].rect.x - 30.0).abs() < 0.01);
    }

    #[test]
    fn label_pills_extend_the_box_and_use_local_coords() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .labels("a", &["标签一", "标签二"])
            .build();
        let s = scene(&d, &snow(), "logicRight");
        let a = s.node(s.find("a").unwrap());
        assert_eq!(a.labels.len(), 2);
        assert!(a.geom.label_h > 0.0);
        let (pad_x, pad_y) = a.geom.pad;
        let text_h = text_height(a.geom.lines.len(), a.geom.line_height, pad_y);
        assert!(near(a.geom.rect.h, text_h + a.geom.label_h));
        assert!(near(a.labels[0].rect.x, pad_x));
        assert!(near(a.labels[1].rect.x, a.labels[0].rect.right() + 6.0));
        assert!(a.geom.rect.w >= a.labels[1].rect.right() + pad_x);
    }

    /// X23 G-3：思维导图的椭圆节点盒要围着同一个文字盒外扩，文字起点同步内缩。
    #[test]
    fn a_mindmap_ellipse_box_expands_around_the_same_text_box() {
        let text = "甲甲甲甲";
        let rounded = Builder::root("中心").child("r", "a", text).build();
        let ellipse = Builder::root("中心")
            .styled(
                "r",
                "a",
                text,
                StyleOverride {
                    shape: Some("ellipse".into()),
                    ..Default::default()
                },
            )
            .build();
        let a = scene(&rounded, &snow(), "mindmap");
        let b = scene(&ellipse, &snow(), "mindmap");
        let ra = a.node(a.find("a").unwrap());
        let rb = b.node(b.find("a").unwrap());
        let extra_w = (ELLIPSE_EXTRA_X * ra.geom.rect.w).min(ELLIPSE_EXTRA_MAX_X);
        let extra_h = (ELLIPSE_EXTRA_Y * ra.geom.rect.h).min(ELLIPSE_EXTRA_MAX_Y);
        assert!((rb.geom.rect.w - ra.geom.rect.w - extra_w).abs() < 0.01);
        assert!((rb.geom.rect.h - ra.geom.rect.h - extra_h).abs() < 0.01);
        assert!((rb.text_x - ra.text_x - extra_w * 0.5).abs() < 0.01);
    }

    /// 分支线两端都要离卡片有点缝：起点 0.5、终点 1.5（锥形末端最细，贴边会像粘住）。
    #[test]
    fn branch_lines_keep_a_small_gap_from_the_cards() {
        let d = Builder::root("根").child("r", "a", "甲").build();
        let s = scene(&d, &snow(), "mindmap");
        let ri = s.find("r").unwrap();
        let ai = s.find("a").unwrap();
        let root = s.node(ri);
        let a = s.node(ai);
        let e = s
            .edges
            .iter()
            .find(|e| e.from == ri && e.to == ai)
            .expect("应有父子连线");
        let right_side = a.geom.rect.x >= root.geom.rect.x;
        let (parent_edge, child_edge) = if right_side {
            (root.geom.rect.right(), a.geom.rect.x)
        } else {
            (root.geom.rect.x, a.geom.rect.right())
        };
        // 起点在父卡外侧一点点
        let start_gap = if right_side {
            e.start.x - parent_edge
        } else {
            parent_edge - e.start.x
        };
        assert!(start_gap > 0.0, "起点不该压在父卡里（gap={start_gap}）");
        assert!(
            start_gap <= 1.0,
            "起点离父卡太远会露出空隙（gap={start_gap}）"
        );
        // 终点在子卡外侧一点点
        let end_gap = if right_side {
            child_edge - e.end.x
        } else {
            e.end.x - child_edge
        };
        assert!(end_gap > 0.0, "终点不该顶到子卡边缘（gap={end_gap}）");
        assert!(end_gap <= 3.0, "终点离子卡太远会像断线（gap={end_gap}）");
        // 纵向都对齐卡片中线
        assert!((e.start.y - (root.geom.rect.y + root.geom.rect.h / 2.0)).abs() < 0.01);
        assert!((e.end.y - (a.geom.rect.y + a.geom.rect.h / 2.0)).abs() < 0.01);
    }

    #[test]
    fn icon_widens_the_box_and_shifts_the_text() {
        let long = "这是一个较长的分支标题";
        let with = Builder::root("根")
            .child("r", "a", long)
            .icon("a", "priority-1")
            .build();
        let plain = Builder::root("根").child("r", "a", long).build();
        let s = scene(&with, &snow(), "logicRight");
        let p = scene(&plain, &snow(), "logicRight");
        let a = s.node(s.find("a").unwrap());
        let b = p.node(p.find("a").unwrap());
        assert_eq!(a.icon.map(|i| i.glyph), Some("①"));
        assert!(b.geom.rect.w > MIN_NODE_W, "样本应宽到不受最小宽约束");
        assert!(near(a.geom.rect.w, b.geom.rect.w + a.icon.unwrap().width));
        let (pad_x, _) = a.geom.pad;
        assert!(near(a.text_x, pad_x + a.icon.unwrap().width));
        assert!(near(b.text_x, pad_x));
    }

    #[test]
    fn note_and_link_flags_are_exposed_for_badges() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .note("a", "备注")
            .link("a", "https://example.com")
            .build();
        let s = scene(&d, &snow(), "logicRight");
        let a = s.node(s.find("a").unwrap());
        assert!(a.has_note && a.has_link);
    }

    #[test]
    fn editing_node_uses_the_in_progress_text_for_size() {
        let d = Builder::root("根").child("r", "a", "短").build();
        let st = settings("logicRight");
        let before = build_scene(&d, "r", &st, &snow(), &MonoMeasurer, None);
        let during = build_scene(
            &d,
            "r",
            &st,
            &snow(),
            &MonoMeasurer,
            Some(("a", "很长很长的一行中文文字")),
        );
        let w0 = before.rect(before.find("a").unwrap()).w;
        let w1 = during.rect(during.find("a").unwrap()).w;
        assert!(w1 > w0, "输入中的文字应立刻影响盒宽：{w0} → {w1}");
    }

    #[test]
    fn wide_text_is_wrapped_at_the_configured_limit() {
        let long = "思维导图的节点文字在超过设定宽度时必须折行而不是无限变宽";
        let d = Builder::root("根").child("r", "a", long).build();
        let s = scene(&d, &snow(), "logicRight");
        let a = s.node(s.find("a").unwrap());
        assert!(a.geom.lines.len() > 1);
        assert!(a.geom.rect.w <= 300.0 + 2.0 * a.geom.pad.0 + 1.0);
    }

    #[test]
    fn narrow_nodes_are_clamped_to_the_minimum_width() {
        let d = Builder::root("根").child("r", "a", "一").build();
        let s = scene(&d, &snow(), "logicRight");
        assert!(near(s.rect(s.find("a").unwrap()).w, MIN_NODE_W));
    }

    #[test]
    fn theme_switch_changes_every_resolved_color() {
        let d = simple();
        let a = scene(&d, &snow(), "mindmap");
        let b = scene(&d, &theme::get_theme("ink"), "mindmap");
        assert_ne!(
            a.node(a.find("r").unwrap()).style.fill,
            b.node(b.find("r").unwrap()).style.fill
        );
        assert_eq!(a.nodes.len(), b.nodes.len());
    }

    #[test]
    fn scene_is_deterministic_across_runs() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .float("f2", "乙", None)
            .float("f1", "丙", None)
            .build();
        let x = scene(&d, &snow(), "mindmap");
        let y = scene(&d, &snow(), "mindmap");
        assert_eq!(x.ids, y.ids);
        // 排序后 f1 先摆，f2 在其下方
        assert!(x.rect(x.find("f1").unwrap()).y < x.rect(x.find("f2").unwrap()).y);
        for i in 0..x.nodes.len() {
            assert_eq!(x.nodes[i].geom.rect, y.nodes[i].geom.rect);
        }
    }

    #[test]
    fn bounds_contain_all_nodes_and_summary_topics() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .summary("r", "s", &["a", "b"], "小结")
            .build();
        let s = scene(&d, &snow(), "logicRight");
        for n in s.nodes.iter() {
            assert!(s.bounds.contains(n.geom.rect.center()));
        }
        let t = s.summaries[0].topic;
        assert!(s.bounds.right() >= t.right());
    }

    #[test]
    fn custom_style_override_reaches_the_scene() {
        let d = Builder::root("根")
            .styled(
                "r",
                "a",
                "甲",
                StyleOverride {
                    fill: Some("#ff0000".into()),
                    shape: Some("rect".into()),
                    bold: Some(true),
                    font_size: Some(22),
                    ..Default::default()
                },
            )
            .build();
        let s = scene(&d, &snow(), "logicRight");
        let a = s.node(s.find("a").unwrap());
        assert_eq!(a.style.fill, Rgba::rgb(255, 0, 0));
        assert_eq!(a.style.shape, Shape::Rect);
        assert!(a.style.bold);
        assert!(near(a.style.font_size, 22.0));
    }

    #[test]
    fn brace_path_flips_with_direction() {
        let r = brace_path(100.0, 0.0, 60.0, Side::Right);
        let l = brace_path(100.0, 0.0, 60.0, Side::Left);
        assert_eq!(r.start, Point::new(100.0, 0.0));
        assert_eq!(r.end(), Point::new(100.0, 60.0));
        match r.segs[1] {
            PathSeg::Line { to } => assert!(to.x > 100.0),
            _ => panic!("第二段应为直线"),
        }
        match l.segs[1] {
            PathSeg::Line { to } => assert!(to.x < 100.0),
            _ => panic!("镜像后应朝左"),
        }
    }

    #[test]
    fn brace_path_clamps_degenerate_spans() {
        let p = brace_path(0.0, 0.0, 1.0, Side::Right);
        match p.segs.last().unwrap() {
            PathSeg::Cubic { to, .. } => assert_eq!(*to, Point::new(0.0, 6.0)),
            _ => panic!("花括号以曲线收尾"),
        }
    }

    #[test]
    fn curve_between_keeps_the_arrow_on_the_end_side() {
        let (path, arrow, mid) = curve_between(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        assert_eq!(path.start, Point::new(0.0, 0.0));
        assert_eq!(arrow[0], Point::new(100.0, 0.0));
        assert!(arrow[1].x < 100.0 && arrow[2].x < 100.0);
        assert!(near(mid.x, 50.0), "二次曲线中点在控制点一侧，此处为 x = 50");
        assert!(mid.y > 0.0, "弧线应向垂直方向偏置");
    }

    fn point_near(a: Point, b: Point) -> bool {
        near(a.x, b.x) && near(a.y, b.y)
    }

    #[test]
    fn blend_moves_shared_nodes_to_the_midpoint() {
        let from = scene(&simple(), &snow(), "mindmap");
        let mut to = from.clone();
        let idx = to.find("a").unwrap();
        std::sync::Arc::make_mut(&mut to.nodes)[idx as usize]
            .geom
            .rect
            .x += 120.0;
        let mid = blend_scenes(&from, &to, 0.5);
        let a = mid.find("a").unwrap();
        assert!(near(mid.rect(a).x, from.rect(a).x + 60.0));
        assert_eq!(mid.nodes.len(), to.nodes.len());
        assert!(mid.nodes.iter().all(|n| near(n.alpha, 1.0)));
    }

    #[test]
    fn image_placeholder_adds_height_and_shifts_text_down() {
        let mut plain_doc = Builder::root("根").child("r", "a", "分支").build();
        let id = "a".to_string();
        let plain = scene(&plain_doc, &snow(), "logicRight");
        let pi = plain.find(&id).unwrap();
        let plain_h = plain.rect(pi).h;

        plain_doc
            .apply(&crate::Command::SetImage {
                id: id.clone(),
                image: Some("images/a.png".into()),
            })
            .unwrap();
        let with_image = scene(&plain_doc, &snow(), "logicRight");
        let ni = with_image.find(&id).unwrap();
        let n = with_image.node(ni);
        assert!(n.geom.image_h > 0.0);
        assert!(with_image.rect(ni).h > plain_h);
        let base = crate::layout::line_base_y(&n.geom, 0);
        assert!(base > n.geom.image_h);
    }

    #[test]
    fn blend_collapse_slides_children_into_parent_and_edges_follow() {
        let expanded = Builder::root("根")
            .child("r", "a", "分支")
            .child("a", "a1", "叶")
            .build();
        let collapsed = Builder::root("根").collapsed("r", "a", "分支").build();
        let from = scene(&expanded, &snow(), "logicRight");
        let to = scene(&collapsed, &snow(), "logicRight");
        let old = from.rect(from.find("a1").unwrap()).center();
        let a_to = to.rect(to.find("a").unwrap()).center();
        let mid = blend_scenes(&from, &to, 0.5);
        let a1 = mid.find("a1").unwrap();
        let n = mid.node(a1);
        assert!(near(n.alpha, 0.5));
        assert_eq!(n.parent, Some(mid.find("a").unwrap()));
        assert!(point_near(
            n.geom.rect.center(),
            Point::new((old.x + a_to.x) / 2.0, (old.y + a_to.y) / 2.0)
        ));
        // 连线也跟到中间帧的位置，而不是留在旧坐标。
        let edge = mid
            .edges
            .iter()
            .find(|e| e.to == a1)
            .expect("折叠中的子节点应保留一条入边");
        assert!(near(edge.alpha, 0.5));
        assert!(
            n.geom.rect.expanded(2.0).contains(edge.end),
            "折叠中连线端点应落在节点附近：{:?} / {:?}",
            edge.end,
            n.geom.rect
        );

        // 结束帧与目标场景一致：没有残留的消失节点。
        let end = blend_scenes(&from, &to, 1.0);
        assert_eq!(end.nodes.len(), to.nodes.len());
        assert!(end.find("a1").is_none());
    }

    #[test]
    fn blend_slides_removed_nodes_toward_the_parent_and_appends_them() {
        let from_doc = Builder::root("根")
            .child("r", "a", "分支")
            .child("a", "a1", "叶")
            .build();
        let to_doc = Builder::root("根").child("r", "a", "分支").build();
        let from = scene(&from_doc, &snow(), "logicRight");
        let to = scene(&to_doc, &snow(), "logicRight");
        let a1_from = from.rect(from.find("a1").unwrap()).center();
        let a_to = to.rect(to.find("a").unwrap()).center();
        let mid = blend_scenes(&from, &to, 0.5);
        let idx = mid.find("a1").unwrap();
        assert!(
            idx as usize >= to.nodes.len(),
            "消失节点应追加在目标节点之后"
        );
        let n = mid.node(idx);
        assert!(near(n.alpha, 0.5));
        let expect = Point::new((a1_from.x + a_to.x) / 2.0, (a1_from.y + a_to.y) / 2.0);
        assert!(point_near(n.geom.rect.center(), expect));
        assert_eq!(n.parent, Some(mid.find("a").unwrap()));
    }

    #[test]
    fn blend_fades_in_new_nodes_from_the_parent() {
        let from_doc = Builder::root("根").child("r", "a", "分支").build();
        let to_doc = Builder::root("根")
            .child("r", "a", "分支")
            .child("a", "a1", "叶")
            .build();
        let from = scene(&from_doc, &snow(), "logicRight");
        let to = scene(&to_doc, &snow(), "logicRight");
        let a_center = from.rect(from.find("a").unwrap()).center();
        let a1_to = to.rect(to.find("a1").unwrap()).center();
        let mid = blend_scenes(&from, &to, 0.5);
        let idx = mid.find("a1").unwrap();
        assert!(near(mid.node(idx).alpha, 0.5));
        let expect = Point::new((a_center.x + a1_to.x) / 2.0, (a_center.y + a1_to.y) / 2.0);
        assert!(point_near(mid.node(idx).geom.rect.center(), expect));
    }

    #[test]
    fn blend_remaps_parent_of_appended_subtrees() {
        let from_doc = Builder::root("根")
            .child("r", "a", "分支")
            .child("a", "a1", "叶")
            .child("a1", "a1x", "孙")
            .build();
        let to_doc = Builder::root("根").child("r", "a", "分支").build();
        let from = scene(&from_doc, &snow(), "logicRight");
        let to = scene(&to_doc, &snow(), "logicRight");
        let mid = blend_scenes(&from, &to, 0.5);
        let a1 = mid.find("a1").unwrap();
        let a1x = mid.find("a1x").unwrap();
        assert_eq!(mid.node(a1).parent, Some(mid.find("a").unwrap()));
        assert_eq!(mid.node(a1x).parent, Some(a1));
        assert!(near(mid.node(a1).alpha, 0.5));
        assert!(near(mid.node(a1x).alpha, 0.5));
    }

    #[test]
    fn blend_interpolates_shared_edges() {
        let d = simple();
        let from = scene(&d, &snow(), "mindmap");
        let to = scene(&d, &snow(), "logicRight");
        let mid = blend_scenes(&from, &to, 0.5);
        let mut checked = false;
        for e in to.edges.iter() {
            let key = (
                to.ids[e.from as usize].as_str(),
                to.ids[e.to as usize].as_str(),
            );
            let Some(old) = from.edges.iter().find(|o| {
                (
                    from.ids[o.from as usize].as_str(),
                    from.ids[o.to as usize].as_str(),
                ) == key
            }) else {
                continue;
            };
            if point_near(old.start, e.start) && point_near(old.end, e.end) {
                continue;
            }
            let got = mid
                .edges
                .iter()
                .find(|m| {
                    (
                        mid.ids[m.from as usize].as_str(),
                        mid.ids[m.to as usize].as_str(),
                    ) == key
                })
                .expect("中点场景必须保留这条连线");
            assert!(point_near(
                got.start,
                Point::new(
                    (old.start.x + e.start.x) / 2.0,
                    (old.start.y + e.start.y) / 2.0
                )
            ));
            checked = true;
            break;
        }
        assert!(checked, "两份布局至少应有一条连线端点不同");
    }

    #[test]
    fn blend_endpoints_return_exact_inputs() {
        let from = scene(&simple(), &snow(), "mindmap");
        let to = scene(&simple(), &snow(), "logicRight");
        // t = 0 保留目标索引（选择环/命中按目标算），几何必须与 from 一致。
        let a = blend_scenes(&from, &to, 0.0);
        for (j, id) in from.ids.iter().enumerate() {
            let i = a.find(id).expect("from 的节点必须还在");
            assert_eq!(a.rect(i), from.rect(j as u32));
        }
        let b = blend_scenes(&from, &to, 1.0);
        assert_eq!(b.ids, to.ids);
        assert!(b
            .nodes
            .iter()
            .zip(to.nodes.iter())
            .all(|(x, y)| x.geom.rect == y.geom.rect));
    }

    /// §5.4.1：同分支走 dim，跨分支取**来源**分支的调色板色；没有分支表时回落 dim。
    ///
    /// 「取色只在绘制层」是硬约束：颜色绝不写回 `relationships[].color`，
    /// 否则切主题、撤销、round-trip 都会把一次算出来的颜色固化成文档内容。
    #[test]
    fn cross_branch_links_borrow_the_source_branch_colour() {
        let theme = snow();
        let ids: Vec<String> = ["r", "a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let mut branches: HashMap<String, u32> = HashMap::new();
        for (id, b) in [("r", 0u32), ("a", 1), ("b", 1), ("c", 2)] {
            branches.insert(id.to_string(), b);
        }
        // a→b 同分支：默认暗色。
        assert_eq!(
            concept_rel_color(&branches, &ids, 1, 2, &theme),
            theme.ui.dim
        );
        // b→c 跨分支：用来源 b 所在分支（1）的色。
        assert_eq!(
            concept_rel_color(&branches, &ids, 2, 3, &theme),
            theme.palette_at(1).expect("分支 1 有颜色")
        );
        // 反向 c→b 用的是 c 的分支（2）：方向不同，颜色跟着来源变。
        assert_eq!(
            concept_rel_color(&branches, &ids, 3, 2, &theme),
            theme.palette_at(2).expect("分支 2 有颜色")
        );
        // 没有分支表（树文档/还没投影）：不 panic，回落 dim。
        assert_eq!(
            concept_rel_color(&HashMap::new(), &ids, 1, 2, &theme),
            theme.ui.dim
        );
    }

    /// §5.4.1：只有真相交才算交叉——端点相接、近平行、共线都不算。
    #[test]
    fn only_true_crossings_count() {
        let p = |x: f32, y: f32| Point::new(x, y);
        // 十字：算。
        assert!(segments_cross(
            (p(0.0, 0.0), p(100.0, 100.0)),
            (p(0.0, 100.0), p(100.0, 0.0))
        ));
        // 共享端点（都从 (0,0) 出发）：不算，否则箭头抵着线也会被弯。
        assert!(!segments_cross(
            (p(0.0, 0.0), p(100.0, 0.0)),
            (p(0.0, 0.0), p(100.0, 60.0))
        ));
        // 近平行（夹角约 1.1°，真交于 x=50）：不算。
        assert!(!segments_cross(
            (p(0.0, 0.0), p(100.0, 0.0)),
            (p(0.0, -1.0), p(100.0, 1.0))
        ));
        // 平行不相交：不算。
        assert!(!segments_cross(
            (p(0.0, 0.0), p(10.0, 0.0)),
            (p(0.0, 50.0), p(10.0, 50.0))
        ));
        // 退化线段（长度为 0）：不算，免得除零。
        assert!(!segments_cross(
            (p(5.0, 5.0), p(5.0, 5.0)),
            (p(0.0, 0.0), p(10.0, 10.0))
        ));
    }

    /// 微弧：垂距封顶 6、方向由 from→to 的水平朝向决定，同一份文档两次渲染必然一致。
    #[test]
    fn bow_offset_is_capped_and_deterministic() {
        let a = Point::new(0.0, 0.0);
        let far = Point::new(600.0, 0.0); // 600/12 = 50 → 封顶 6
        let (path, arrow, label) = bowed_between(a, far);
        assert_eq!(path.end(), far, "弧的终点必须正好落在目标点");
        assert_eq!(arrow[0], far, "箭头尖仍钉在终点");
        let mid = Point::new(300.0, 0.0);
        // 控制点偏 2×6 = 12 → t=0.5 处偏 6（世界单位）。
        // 偏移方向由 from→to 的水平朝向定死（向右 → 往上弯），大小封顶 6。
        assert!(
            near((label.y - mid.y).abs(), 6.0),
            "垂距该封顶 6，实际 {}",
            label.y - mid.y
        );
        // 确定性：同参数两次调用逐字段相同。
        let again = bowed_between(a, far);
        assert_eq!(again.2, label);
        // 短线的偏移按 len/12 缩，不会超过 6。
        let (_, _, short) = bowed_between(a, Point::new(24.0, 0.0));
        assert!(short.y.abs() <= 6.0 + 0.01, "短线的偏移得小于封顶");
    }

    /// CM-09：`truncated` 表示「真被截了」（两行放不下多了省略号），不是「文字长」；
    /// 卡片圆点看 `card_id` 有没有实际内容。
    #[test]
    fn concept_nodes_report_truncation_and_card_dot() {
        let mut doc = Builder::root("产品定位")
            .float(
                "a",
                "一个相当长的概念名称用来检查换行与省略号是否真的会出现以及第三行到底会不会被吃掉",
                Some((-200.0, -100.0)),
            )
            .float("b", "短概念", Some((220.0, 0.0)))
            .build();
        doc.meta.layout.kind = "conceptMap".into();
        let id_of = |d: &Document, prefix: &str| {
            d.nodes
                .values()
                .find(|n| n.text.starts_with(prefix))
                .map(|n| n.id.clone())
                .expect("概念在文档里")
        };
        let long_id = id_of(&doc, "一个相当长");
        let short_id = id_of(&doc, "短概念");
        doc.nodes.get_mut(&short_id).unwrap().card_id = Some("c1".into());
        // 空串不算「挂了卡片」：导出的 `.zmind` 里见过这种脏数据。
        let empty_card = doc
            .nodes
            .values()
            .find(|n| n.text == "产品定位")
            .map(|n| n.id.clone())
            .unwrap();
        doc.nodes.get_mut(&empty_card).unwrap().card_id = Some("  ".into());

        let s = scene(&doc, &snow(), "conceptMap");
        let idx = |id: &str| s.ids.iter().position(|x| x == id).expect("节点在场景里");
        assert!(
            s.nodes[idx(&long_id)].truncated,
            "两行放不下必须标 truncated"
        );
        assert!(!s.nodes[idx(&short_id)].truncated, "短概念不该有 tooltip");
        assert!(s.nodes[idx(&short_id)].has_card, "有卡片才画圆点");
        assert!(!s.nodes[idx(&long_id)].has_card);
        assert!(!s.nodes[idx(&empty_card)].has_card, "空白 card_id 不算挂卡");
    }

    #[test]
    fn blend_cross_fades_axes_when_layout_kind_changes() {
        let d = simple();
        let from = scene(&d, &snow(), "mindmap");
        let to = scene(&d, &snow(), "fishbone");
        assert!(from.spines.is_empty() && !to.spines.is_empty());
        let mid = blend_scenes(&from, &to, 0.5);
        assert!(mid.spines.len() >= to.spines.len());
        assert!(mid.spines.iter().all(|s| near(s.alpha, 0.5)));
    }
}
