//! 布局引擎：放射式 / 逻辑图向右 / 组织结构图（自 ui/src/lib/layout.ts 移植，文档 02 §5）。
//!
//! 纯函数：树 + 测量结果 → 盒子坐标 + 连线端点。文本度量经 [`NodeMeasure`] 注入，
//! 所以这里既不认识字体也不认识 egui（docs/04 §1 铁律 2）。
//!
//! 节点身份：返回 [`Layout`] 内的**密集索引**（`u32`），id → 索引经 `index` 查询。
//! 渲染层要的是可以当数组下标用的句柄，不是每帧哈希 UUID。

use std::collections::HashMap;

use crate::geom::{Enclosing, Path, Point, Rect};
use crate::model::{LayoutSettings, Node};
use crate::Document;

// 节点内距**不在这里**：文字盒的内距由 `design::card_padding(font_size)` 按字号分档给出
// （中心 26/12、一级 18/10、二级 14/8、深层 12/7）。这里以前挂着 PAD_X = 18 / PAD_Y = 9
// 两个常量，全仓没有一处读它们——却让读代码的人以为内距是一个全局常数，
// 于是「间距为什么不是令牌」这类问题被反复提出来。删掉，只留这一行指向真正的出口。
/// 折叠圆点半径与外偏量（屏幕像素，渲染层需除以缩放）。
pub const TOGGLE_OFFSET: f32 = 11.0;
/// 鱼骨 / 时间轴：分支近边与轴线的间距。
const AXIS_GAP: f32 = 34.0;
/// 鱼骨肋在脊线上的落点相对分支中心的水平偏移（朝向鱼头）。
const BONE_SLANT: f32 = 28.0;
/// 轴线在最后一个连接点之外再伸出的长度。
const SPINE_TAIL: f32 = 16.0;

/// 布局种类（`LayoutSettings.kind` 的字符串取值见 model）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    /// 放射式：根左右两侧
    Mindmap,
    /// 逻辑图：全部子树向右
    LogicRight,
    /// 组织结构图：自上而下
    OrgChart,
    /// 鱼骨图：脊线向左，分支上下交替、各自向左生长
    Fishbone,
    /// 时间轴：轴线向右，分支上下交替、各自向右生长
    Timeline,
    /// 概念图：命题网，节点按 `pos` 自由摆放，边是有向带短语的关系线。
    /// 它**不是**另一种树布局——`compute_layout` 只负责把概念各自量出盒子，
    /// 位置由 `pos` 决定（见 `build_scene` 的自由主题合并段）。
    ConceptMap,
}

impl LayoutKind {
    pub fn from_kind(s: &str) -> Self {
        match s {
            "logicRight" => Self::LogicRight,
            "orgChart" => Self::OrgChart,
            "fishbone" => Self::Fishbone,
            "timeline" => Self::Timeline,
            "conceptMap" => Self::ConceptMap,
            _ => Self::Mindmap,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mindmap => "mindmap",
            Self::LogicRight => "logicRight",
            Self::OrgChart => "orgChart",
            Self::Fishbone => "fishbone",
            Self::Timeline => "timeline",
            Self::ConceptMap => "conceptMap",
        }
    }

    /// 概念图不吃树布局：节点位置来自 `pos`，父子关系不表达层级。
    pub fn is_concept_map(self) -> bool {
        self == Self::ConceptMap
    }
}

/// 一个节点的测量结果（布局的唯一外部依赖）。
#[derive(Clone, Debug, PartialEq)]
pub struct BoxSpec {
    pub w: f32,
    pub h: f32,
    /// 折好的行。用 `Arc<[String]>` 是为了**命中缓存时不复制字符串**：
    /// 20k 文档每次重建要取两万个盒子，逐个克隆行文本就是每帧几万次分配（打磨轮 10）。
    pub lines: std::sync::Arc<[String]>,
    pub font_size: f32,
    pub line_height: f32,
    pub bold: bool,
    /// 底部留给标签胶囊的高度；文字在其上方的区域里垂直居中
    pub label_h: f32,
    /// 顶部图片占位高度（0 = 无图）。
    pub image_h: f32,
    /// 图片宽高比（宽/高），无图时为 16/9。
    pub image_ratio: f32,
}

/// 测量器。由 `scene.rs` 实现（综合样式、图标与标签宽度）。
pub trait NodeMeasure {
    fn measure(&self, node: &Node, depth: u32, text: &str) -> BoxSpec;
}

/// 已定位置的节点盒。
#[derive(Clone, Debug, PartialEq)]
pub struct NodeBox {
    pub rect: Rect,
    pub depth: u32,
    pub lines: std::sync::Arc<[String]>,
    pub font_size: f32,
    pub line_height: f32,
    pub bold: bool,
    pub label_h: f32,
    pub image_h: f32,
    pub image_ratio: f32,
}

/// 连线方向：水平出边 / 垂直出边 / 直线（鱼骨肋、时间轴刻度）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
    Straight,
}

/// 分支线起点离父卡的距离（世界单位）。0.5px 只是把线从描边里"拔"出来。
pub const EDGE_GAP_START: f32 = 0.5;
/// 分支线终点离子卡的距离：末端最细，留一点空才不像粘在边上。
pub const EDGE_GAP_END: f32 = 1.5;

/// 父子连线端点（世界坐标）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub start: Point,
    pub end: Point,
    pub orientation: Orientation,
}

/// 一次布局的结果。
#[derive(Clone, Debug)]
pub struct Layout {
    /// 密集索引 → 节点 id（前序，主根在 0）
    pub ids: Vec<String>,
    pub index: HashMap<String, u32>,
    pub boxes: Vec<NodeBox>,
    pub edges: Vec<Edge>,
    /// 装饰性轴线：鱼骨脊线 / 时间轴（不参与点击判定）
    pub spines: Vec<Path>,
    /// 全部盒子的包围盒（已外扩 16）
    pub bounds: Rect,
}

impl Layout {
    pub fn find(&self, id: &str) -> Option<u32> {
        self.index.get(id).copied()
    }

    pub fn rect(&self, idx: u32) -> Rect {
        self.boxes[idx as usize].rect
    }

    fn empty() -> Self {
        Self {
            ids: Vec::new(),
            index: HashMap::new(),
            boxes: Vec::new(),
            edges: Vec::new(),
            spines: Vec::new(),
            bounds: Rect::new(-10.0, -10.0, 20.0, 20.0),
        }
    }
}

/// 正在编辑的节点：id + 输入中的文字。编辑中的节点即使折叠也要展开布局
/// （否则用户打字时整个子树会消失）。
pub type Editing<'a> = Option<(&'a str, &'a str)>;

/// 上一次布局的账（打磨轮 11）。
///
/// 打字时每个节点每轮都要重新「查样式表 → 查测量缓存 → 组装盒子」，20k 一次 5ms 上下；
/// 但真正变了文字的通常只有一颗。这里记下上一轮每颗节点的**文字指纹**与**盒子**，
/// 下一次逐项比对：指纹相同就直接复用盒子，只有真变了的节点才走测量。
///
/// 结构（可见节点序列 / 深度）用一条 64 位指纹 + 深度数组双重闸门：改父子、改顺序、
/// 折叠展开都会让它变，变了就整份作废走全量路。碰撞概率 2^-64，且碰撞也只有「画错」这一种后果，
/// 所以这里不追求密码学强度，只求快。
#[derive(Default, Clone, Debug)]
pub struct LayoutMemo {
    pub n: usize,
    ids_fp: u64,
    depth: Vec<u32>,
    text_fp: Vec<u64>,
    boxes: Vec<NodeBox>,
}

impl LayoutMemo {
    /// 上一轮第 `i` 颗节点的盒子（没有账就是 `None`）。
    pub fn box_at(&self, i: usize) -> Option<&NodeBox> {
        self.boxes.get(i)
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a：给「结构 / 文字」这类内部数据算指纹用，不参与任何对外哈希表。
fn fnv1a(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// 一颗节点的「盒子输入」指纹：**影响尺寸的每一项**都必须进这里，否则改图标/改标签
/// 会命中上一轮的盒子、画出旧尺寸（单测 `the_box_cache_remeasures_only_when_a_size_input_changes` 守着）。
/// 字号/粗细/最大宽不在这里——它们由场景那一侧的主题与样式指纹把关，账不匹配时整份作废。
fn box_inputs_fingerprint(node: &Node, text: &str) -> u64 {
    let mut h = fnv1a(FNV_OFFSET, text.as_bytes());
    h = fnv1a(h, &[0]);
    for opt in [node.icon.as_deref(), node.image.as_deref()] {
        h = match opt {
            Some(s) => fnv1a(fnv1a(h, &[1]), s.as_bytes()),
            None => fnv1a(h, &[0]),
        };
    }
    h = fnv1a(h, &[0]);
    for l in node.labels.iter() {
        h = fnv1a(fnv1a(h, l.as_bytes()), &[0x1f]);
    }
    h
}

/// 计算 `root_id` 这棵树（或子树）的布局。
pub fn compute_layout(
    doc: &Document,
    root_id: &str,
    settings: &LayoutSettings,
    measure: &dyn NodeMeasure,
    editing: Editing<'_>,
) -> Layout {
    compute_layout_memo(doc, root_id, settings, measure, editing, None).0
}

/// 带「上一次的账」的布局：`memo` 对得上就复用盒子，只重测变了的节点。
/// 返回布局与**本轮的新账**（调用方存起来给下一轮）。
pub fn compute_layout_memo(
    doc: &Document,
    root_id: &str,
    settings: &LayoutSettings,
    measure: &dyn NodeMeasure,
    editing: Editing<'_>,
    memo: Option<&LayoutMemo>,
) -> (Layout, LayoutMemo) {
    let (layout, memo, _reused) =
        compute_layout_memo_with_reach(doc, root_id, settings, measure, editing, memo);
    (layout, memo)
}

/// 和 [`compute_layout_memo`] 一样，另外告诉调用方「这一轮复用了几颗节点的盒子」——
/// 场景层要靠它决定「两代账用哪一代」（打磨轮 15）。
pub fn compute_layout_memo_with_reach(
    doc: &Document,
    root_id: &str,
    settings: &LayoutSettings,
    measure: &dyn NodeMeasure,
    editing: Editing<'_>,
    memo: Option<&LayoutMemo>,
) -> (Layout, LayoutMemo, usize) {
    // 分阶段计时（打磨轮 10）：`MW_LAYOUT_LOG=1` 时打到 stderr，平时一个分支都不多走。
    // 「哪一段值得做增量」必须先量出来——凭直觉猜的话，最贵的那段往往不是测量。
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
    let gap = settings.sibling_gap.max(4.0);
    let level_gap = settings.level_gap.max(20.0);
    let kind = LayoutKind::from_kind(&settings.kind);
    let editing_id = editing.map(|(i, _)| i);

    // ---- 1) 收集可见节点（前序，折叠节点不下探） ----
    let mut ids: Vec<String> = Vec::new();
    let mut refs: Vec<&Node> = Vec::new();
    let mut depth: Vec<u32> = Vec::new();
    let mut parent: Vec<Option<usize>> = Vec::new();
    // 栈里放 `&Node` 而不是 `(String, …)`：以前每下探一层就把孩子的 id **克隆一份**进栈，
    // 20k 树等于两万次本可避免的字符串分配（打磨轮 14 实测这一段占 collect 的一半）。
    let mut stack: Vec<(&Node, u32, Option<usize>)> = doc
        .node(root_id)
        .map(|r| (r, 0u32, None))
        .into_iter()
        .collect();
    // 可见节点序列 + 深度：一次遍历顺手算成指纹，用来判断「上一轮的账还对得上吗」。
    let mut ids_fp = FNV_OFFSET;
    while let Some((n, d, p)) = stack.pop() {
        ids_fp = fnv1a(ids_fp, n.id.as_bytes());
        ids_fp = fnv1a(ids_fp, &d.to_le_bytes());
        let idx = ids.len();
        ids.push(n.id.clone());
        refs.push(n);
        depth.push(d);
        parent.push(p);
        let hidden = n.collapsed && Some(ids[idx].as_str()) != editing_id;
        if !hidden {
            // 逆序入栈 → 弹出即文档顺序
            for c in n.children.iter().rev() {
                if let Some(child) = doc.node(c) {
                    stack.push((child, d + 1, Some(idx)));
                }
            }
        }
    }
    let n = ids.len();
    if n == 0 {
        return (Layout::empty(), LayoutMemo::default(), 0);
    }
    mark!("collect");

    // ---- 2) CSR 子表：前序扫描使同一父节点的孩子按文档顺序排列 ----
    let mut kid_count = vec![0usize; n];
    for i in 1..n {
        if let Some(p) = parent[i] {
            kid_count[p] += 1;
        }
    }
    let mut offsets = vec![0usize; n + 1];
    for i in 0..n {
        offsets[i + 1] = offsets[i] + kid_count[i];
    }
    let mut cursor = offsets[..n].to_vec();
    let mut flat = vec![0usize; offsets[n]];
    for i in 1..n {
        if let Some(p) = parent[i] {
            flat[cursor[p]] = i;
            cursor[p] += 1;
        }
    }

    mark!("csr");
    // ---- 3) 测量 ----
    // 上一轮的账只有在「同一批节点、同样深度」时才可用；对不上就整份丢掉走全量。
    let reuse = match memo {
        Some(m) if m.n == n && m.ids_fp == ids_fp && m.depth == depth => Some(m),
        other => {
            if std::env::var_os("MW_LAYOUT_LOG").is_some() {
                match other {
                    None => eprintln!("[memo] 没有上一轮的账"),
                    Some(m) => eprintln!(
                        "[memo] 对不上 n={} vs {} fp={:x} vs {:x} depth_eq={}",
                        m.n,
                        n,
                        m.ids_fp,
                        ids_fp,
                        m.depth == depth
                    ),
                }
            }
            None
        }
    };
    let mut boxes: Vec<NodeBox> = Vec::with_capacity(n);
    let mut text_fp: Vec<u64> = Vec::with_capacity(n);
    let mut reused = 0usize;
    for i in 0..n {
        let node = refs[i];
        // 不改字的时候不必克隆文本：`text` 只在「正在编辑的那一枚」上来自入参。
        let raw: &str = match editing {
            Some((eid, etext)) if eid == ids[i] => etext,
            _ => &node.text,
        };
        let text = if raw.trim().is_empty() { " " } else { raw };
        let fp = box_inputs_fingerprint(node, text);
        // 文字没变 → 上一轮的盒子直接搬过来（`lines` 是 Arc，搬移只有引用计数）
        if let Some(m) = reuse.filter(|m| m.text_fp[i] == fp) {
            let mut b = m.boxes[i].clone();
            b.depth = depth[i];
            b.rect = Rect::new(0.0, 0.0, b.rect.w, b.rect.h);
            boxes.push(b);
            text_fp.push(fp);
            reused += 1;
            continue;
        }
        let spec = measure.measure(node, depth[i], text);
        boxes.push(NodeBox {
            rect: Rect::new(0.0, 0.0, spec.w, spec.h),
            depth: depth[i],
            lines: spec.lines,
            font_size: spec.font_size,
            line_height: spec.line_height,
            bold: spec.bold,
            label_h: spec.label_h,
            image_h: spec.image_h,
            image_ratio: spec.image_ratio,
        });
        text_fp.push(fp);
    }

    mark!("measure");
    let max_depth = depth.iter().copied().max().unwrap_or(0) as usize;

    // 鱼骨肋 / 时间轴刻度：直接子节点用一条从轴线出发的直线代替父子曲线
    let mut bones: Vec<Option<(Point, Point)>> = vec![None; n];
    let mut spines: Vec<Path> = Vec::new();

    // ---- 4) 分支带高度（垂直堆叠方向）：前序逆扫即后序 ----
    let mut band_h = vec![0.0f32; n];
    for i in (0..n).rev() {
        let (a, b) = (offsets[i], offsets[i + 1]);
        if a == b {
            band_h[i] = boxes[i].rect.h;
            continue;
        }
        let mut total = 0.0f32;
        for j in a..b {
            total += band_h[flat[j]];
        }
        total += gap * (b - a - 1) as f32;
        band_h[i] = boxes[i].rect.h.max(total);
    }

    mark!("bands");
    if kind == LayoutKind::OrgChart {
        // ---- 5a) 组织结构图：行高 + 子树宽度，节点在带内水平居中 ----
        let mut row_h = vec![0.0f32; max_depth + 1];
        for b in boxes.iter() {
            let d = b.depth as usize;
            row_h[d] = row_h[d].max(b.rect.h);
        }
        let mut row_y = vec![0.0f32; max_depth + 1];
        let mut y = 0.0f32;
        for d in 0..=max_depth {
            row_y[d] = y;
            y += row_h[d] + level_gap;
        }

        let mut sub_w = vec![0.0f32; n];
        for i in (0..n).rev() {
            let (a, b) = (offsets[i], offsets[i + 1]);
            if a == b {
                sub_w[i] = boxes[i].rect.w;
                continue;
            }
            let mut total = 0.0f32;
            for j in a..b {
                total += sub_w[flat[j]];
            }
            total += gap * (b - a - 1) as f32;
            sub_w[i] = boxes[i].rect.w.max(total);
        }

        let mut stack: Vec<(usize, f32)> = vec![(0, -sub_w[0] / 2.0)];
        while let Some((i, left)) = stack.pop() {
            let sw = sub_w[i];
            boxes[i].rect.y = row_y[boxes[i].depth as usize];
            boxes[i].rect.x = left + (sw - boxes[i].rect.w) / 2.0;
            let (a, b) = (offsets[i], offsets[i + 1]);
            let mut total = 0.0f32;
            for j in a..b {
                total += sub_w[flat[j]];
            }
            if b > a {
                total += gap * (b - a - 1) as f32;
            }
            let mut cx = left + (sw - total) / 2.0;
            for j in a..b {
                let k = flat[j];
                stack.push((k, cx));
                cx += sub_w[k] + gap;
            }
        }
    } else if matches!(kind, LayoutKind::Fishbone | LayoutKind::Timeline) {
        // ---- 5c) 鱼骨 / 时间轴：轴线居中，分支上下交替、各自沿水平方向生长 ----
        let fish = kind == LayoutKind::Fishbone;
        boxes[0].rect.x = -boxes[0].rect.w / 2.0;
        boxes[0].rect.y = -boxes[0].rect.h / 2.0;
        let root = boxes[0].rect;
        let root_kids: Vec<usize> = flat[offsets[0]..offsets[1]].to_vec();

        // 鱼骨：上下两侧各自向左分配槽位，兄弟分支不共享 x 区间（否则肋线会穿过邻盒）；
        // 时间轴：所有分支共用一条向右推进的游标
        let mut above_slot = root.x - level_gap;
        let mut below_slot = root.x - level_gap;
        let mut timeline_slot = root.right() + level_gap;
        let mut spine_left = f32::INFINITY;
        let mut above_cursor = -AXIS_GAP;
        let mut below_cursor = AXIS_GAP;
        let mut ticks: Vec<f32> = Vec::new();

        for (nth, &br) in root_kids.iter().enumerate() {
            let above = nth % 2 == 0;
            let slot = if fish {
                if above {
                    above_slot
                } else {
                    below_slot
                }
            } else {
                timeline_slot
            };

            // 该分支在各层的最大盒宽（按绝对深度索引）
            let mut layer_w = vec![0.0f32; max_depth + 1];
            let mut seen = vec![false; max_depth + 1];
            let mut walk: Vec<usize> = vec![br];
            while let Some(i) = walk.pop() {
                let d = boxes[i].depth as usize;
                layer_w[d] = layer_w[d].max(boxes[i].rect.w);
                seen[d] = true;
                walk.extend_from_slice(&flat[offsets[i]..offsets[i + 1]]);
            }

            let d0 = boxes[br].depth as usize;
            let mut col = vec![0.0f32; max_depth + 1];
            if fish {
                let mut x = slot;
                for d in d0..=max_depth {
                    if !seen[d] {
                        continue;
                    }
                    col[d] = x - layer_w[d];
                    x -= layer_w[d] + level_gap;
                }
                let left = x + level_gap;
                spine_left = spine_left.min(left);
                let next = left - gap;
                if above {
                    above_slot = next;
                } else {
                    below_slot = next;
                }
            } else {
                let mut x = slot;
                for d in d0..=max_depth {
                    if !seen[d] {
                        continue;
                    }
                    col[d] = x;
                    x += layer_w[d] + level_gap;
                }
                timeline_slot = x - level_gap + gap;
            }

            // 上下两条游标各自向外堆叠整条分支带
            let top = if above {
                let t = above_cursor - band_h[br];
                above_cursor = t - gap;
                t
            } else {
                let t = below_cursor;
                below_cursor += band_h[br] + gap;
                t
            };

            let mut pending: Vec<(usize, f32)> = vec![(br, top)];
            while let Some((i, top)) = pending.pop() {
                boxes[i].rect.x = col[boxes[i].depth as usize];
                boxes[i].rect.y = top + (band_h[i] - boxes[i].rect.h) / 2.0;
                let (a, b) = (offsets[i], offsets[i + 1]);
                let mut kids_total = 0.0f32;
                for &k in &flat[a..b] {
                    kids_total += band_h[k];
                }
                if b > a {
                    kids_total += gap * (b - a - 1) as f32;
                }
                let mut cy = top + (band_h[i] - kids_total) / 2.0;
                for &k in &flat[a..b] {
                    pending.push((k, cy));
                    cy += band_h[k] + gap;
                }
            }

            let b = boxes[br].rect;
            let cx = b.x + b.w / 2.0;
            let near = if above {
                Point::new(cx, b.bottom())
            } else {
                Point::new(cx, b.y)
            };
            let join_x = if fish {
                (cx + BONE_SLANT).min(root.x - 4.0)
            } else {
                cx.max(root.right() + 4.0)
            };
            bones[br] = Some((Point::new(join_x, 0.0), near));
            if !fish {
                ticks.push(join_x);
            }
        }

        if !root_kids.is_empty() {
            let mut axis = Path::new(Point::new(if fish { root.x } else { root.right() }, 0.0));
            if fish {
                let left = spine_left - SPINE_TAIL;
                axis.line(Point::new(left.min(root.x), 0.0));
            } else {
                let right = ticks.iter().copied().fold(root.right(), f32::max) + SPINE_TAIL;
                axis.line(Point::new(right.max(root.right()), 0.0));
            }
            spines.push(axis);
        }
    } else {
        // ---- 5b) 放射式 / 逻辑图：根居中，横向按层对齐 ----
        boxes[0].rect.x = -boxes[0].rect.w / 2.0;
        boxes[0].rect.y = -boxes[0].rect.h / 2.0;

        let root_kids: Vec<usize> = flat[offsets[0]..offsets[1]].to_vec();
        // 放射式把根的孩子分到两侧（右侧先排），逻辑图全部在右
        let sides: Vec<Vec<usize>> = if kind == LayoutKind::Mindmap {
            match settings.direction.as_str() {
                "right" => vec![root_kids],
                "left" => vec![Vec::new(), root_kids],
                _ => {
                    let mut left: Vec<usize> = Vec::new();
                    let mut right: Vec<usize> = Vec::new();
                    let mut lh = 0.0f32;
                    let mut rh = 0.0f32;
                    for k in root_kids {
                        let h = band_h[k] + gap;
                        if lh <= rh {
                            left.push(k);
                            lh += h;
                        } else {
                            right.push(k);
                            rh += h;
                        }
                    }
                    vec![right, left]
                }
            }
        } else {
            vec![root_kids]
        };

        for (si, side) in sides.iter().enumerate() {
            if side.is_empty() {
                continue;
            }
            let dir: f32 = if kind == LayoutKind::Mindmap && si == 1 {
                -1.0
            } else {
                1.0
            };

            // 每层最宽盒子 → 列位置
            let mut layer_w = vec![0.0f32; max_depth + 1];
            let mut seen = vec![false; max_depth + 1];
            let mut walk: Vec<usize> = side.clone();
            while let Some(i) = walk.pop() {
                let d = boxes[i].depth as usize;
                layer_w[d] = layer_w[d].max(boxes[i].rect.w);
                seen[d] = true;
                for j in offsets[i]..offsets[i + 1] {
                    walk.push(flat[j]);
                }
            }

            let mut col = vec![0.0f32; max_depth + 1];
            if dir > 0.0 {
                let mut x = boxes[0].rect.right() + level_gap;
                for d in 1..=max_depth {
                    if !seen[d] {
                        continue;
                    }
                    col[d] = x;
                    x += layer_w[d] + level_gap;
                }
            } else {
                let mut x = boxes[0].rect.x - level_gap;
                for d in 1..=max_depth {
                    if !seen[d] {
                        continue;
                    }
                    col[d] = x - layer_w[d];
                    x -= layer_w[d] + level_gap;
                }
            }

            let mut walk: Vec<usize> = side.clone();
            while let Some(i) = walk.pop() {
                boxes[i].rect.x = col[boxes[i].depth as usize];
                for j in offsets[i]..offsets[i + 1] {
                    walk.push(flat[j]);
                }
            }

            let mut total = 0.0f32;
            for &r in side {
                total += band_h[r];
            }
            total += gap * side.len().saturating_sub(1) as f32;
            let mut y_top = -total / 2.0;
            let mut pending: Vec<(usize, f32)> = Vec::with_capacity(side.len());
            for &r in side {
                pending.push((r, y_top));
                y_top += band_h[r] + gap;
            }
            while let Some((i, top)) = pending.pop() {
                let hi = band_h[i];
                boxes[i].rect.y = top + (hi - boxes[i].rect.h) / 2.0;
                let (a, b) = (offsets[i], offsets[i + 1]);
                let mut kids_total = 0.0f32;
                for j in a..b {
                    kids_total += band_h[flat[j]];
                }
                if b > a {
                    kids_total += gap * (b - a - 1) as f32;
                }
                let mut cy = top + (hi - kids_total) / 2.0;
                for j in a..b {
                    let k = flat[j];
                    pending.push((k, cy));
                    cy += band_h[k] + gap;
                }
            }
        }
    }

    mark!("place");
    // ---- 6) 连线端点 ----
    let mut edges: Vec<Edge> = Vec::with_capacity(n - 1);
    for i in 0..n {
        let p = boxes[i].rect;
        for j in offsets[i]..offsets[i + 1] {
            let k = flat[j];
            let c = boxes[k].rect;
            let e = if let Some((start, end)) = bones[k] {
                Edge {
                    from: i as u32,
                    to: k as u32,
                    start,
                    end,
                    orientation: Orientation::Straight,
                }
            } else if kind == LayoutKind::OrgChart {
                Edge {
                    from: i as u32,
                    to: k as u32,
                    start: Point::new(p.x + p.w / 2.0, p.bottom()),
                    end: Point::new(c.x + c.w / 2.0, c.y),
                    orientation: Orientation::Vertical,
                }
            } else if c.x >= p.x {
                Edge {
                    from: i as u32,
                    to: k as u32,
                    // 起点离卡片 0.5px、终点离卡片 1.5px：锥形线在末端已经最细，
                    // 直接顶到卡片边缘会像"粘"在边上（边缘还会吃掉一半线宽）。
                    start: Point::new(p.right() + EDGE_GAP_START, p.y + p.h / 2.0),
                    end: Point::new(c.x - EDGE_GAP_END, c.y + c.h / 2.0),
                    orientation: Orientation::Horizontal,
                }
            } else {
                Edge {
                    from: i as u32,
                    to: k as u32,
                    start: Point::new(p.x - EDGE_GAP_START, p.y + p.h / 2.0),
                    end: Point::new(c.right() + EDGE_GAP_END, c.y + c.h / 2.0),
                    orientation: Orientation::Horizontal,
                }
            };
            edges.push(e);
        }
    }

    mark!("edges");
    // ---- 7) 包围盒 ----
    let mut enc = Enclosing::new();
    for b in boxes.iter() {
        enc.add(b.rect);
    }
    let index: HashMap<String, u32> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i as u32))
        .collect();
    let bounds = enc
        .rect()
        .unwrap_or(Rect::new(-10.0, -10.0, 20.0, 20.0))
        .expanded(BOUND_PAD);

    mark!("bounds");
    // 最后一个里程碑之后 `mark_at` 不再被读；这一行只是告诉编译器「我知道」，
    // 比给整个函数挂 `#[allow(unused_assignments)]` 更窄。
    let _ = &mark_at;
    if perf {
        let total: f64 = marks.iter().map(|(_, ms)| ms).sum();
        let parts: Vec<String> = marks
            .iter()
            .map(|(label, ms)| format!("{label}={ms:.2}"))
            .collect();
        eprintln!(
            "[layout] n={n} total={total:.2} reused={reused} {}",
            parts.join(" ")
        );
    }
    // 新一轮的账：结构指纹 + 每节点文字指纹 + 盒子（`NodeBox` 里只有 `lines` 是 Arc，
    // 克隆两万个盒子是几次 memcpy，20k 实测 0.2–0.3ms——比重新查表装配便宜一个数量级）。
    let account = LayoutMemo {
        n,
        ids_fp,
        depth: depth.clone(),
        text_fp,
        boxes: boxes.clone(),
    };
    (
        Layout {
            ids,
            index,
            boxes,
            edges,
            spines,
            bounds,
        },
        account,
        reused,
    )
}

/// 包围盒外扩（留给折叠圆点、线宽与选中环）。
pub const BOUND_PAD: f32 = 16.0;

/// 旧 SVG 的基线系数：基线落在行盒顶往下 `line_height × 0.72`。
pub const BASELINE_RATIO: f32 = 0.72;

/// 第 `i` 行基线的 y（节点盒顶为原点）。
///
/// 文字块在「盒高 − 标签高」里垂直居中。画布（`mw_render`）与行内编辑器（`mw_shell`）
/// 都调这里：公式一旦有两份，按 F2 的瞬间文字就会在两处读数之间跳几像素。
/// `box_h` 允许传过渡中的插值盒高——居中区随盒高上下平移半个差值。
pub fn line_baseline(box_h: f32, label_h: f32, lines: usize, line_height: f32, i: usize) -> f32 {
    line_baseline_with_image(box_h, label_h, 0.0, lines, line_height, i)
}

/// 同上，但文字区从图片占位下方开始。
pub fn line_baseline_with_image(
    box_h: f32,
    label_h: f32,
    image_h: f32,
    lines: usize,
    line_height: f32,
    i: usize,
) -> f32 {
    let start = image_h + ((box_h - image_h - label_h) - lines as f32 * line_height) / 2.0;
    start + i as f32 * line_height + line_height * BASELINE_RATIO
}

/// 第 `i` 行文字的基线 y（节点盒顶部为原点；标签区已扣减）。
pub fn line_base_y(b: &NodeBox, i: usize) -> f32 {
    line_baseline_with_image(
        b.rect.h,
        b.label_h,
        b.image_h,
        b.lines.len(),
        b.line_height,
        i,
    )
}

/// 连线的贝塞尔路径：水平连线在水平方向拉出控制点，垂直连线在垂直方向。
/// 分支线形态。布局只决定端点，形态由文档设置决定。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EdgeShape {
    #[default]
    Curve,
    Elbow,
    Straight,
}

impl EdgeShape {
    pub fn from_name(s: &str) -> Self {
        match s {
            "elbow" => Self::Elbow,
            "straight" => Self::Straight,
            _ => Self::Curve,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Curve => "curve",
            Self::Elbow => "elbow",
            Self::Straight => "straight",
        }
    }
}

pub fn edge_path(e: &Edge) -> Path {
    edge_path_with(e, EdgeShape::Curve)
}

/// 带线型的分支线：曲线 / 直角折线 / 直线。
pub fn edge_path_with(e: &Edge, shape: EdgeShape) -> Path {
    let mut p = Path::new(e.start);
    match shape {
        EdgeShape::Curve => match e.orientation {
            Orientation::Vertical => {
                let dy = (e.end.y - e.start.y) / 2.0;
                p.cubic(
                    Point::new(e.start.x, e.start.y + dy),
                    Point::new(e.end.x, e.end.y - dy),
                    e.end,
                );
            }
            Orientation::Horizontal => {
                let dx = (e.end.x - e.start.x) / 2.0;
                p.cubic(
                    Point::new(e.start.x + dx, e.start.y),
                    Point::new(e.end.x - dx, e.end.y),
                    e.end,
                );
            }
            Orientation::Straight => p.line(e.end),
        },
        EdgeShape::Elbow => match e.orientation {
            Orientation::Horizontal => {
                let mx = (e.start.x + e.end.x) / 2.0;
                p.line(Point::new(mx, e.start.y));
                p.line(Point::new(mx, e.end.y));
                p.line(e.end);
            }
            Orientation::Vertical => {
                let my = (e.start.y + e.end.y) / 2.0;
                p.line(Point::new(e.start.x, my));
                p.line(Point::new(e.end.x, my));
                p.line(e.end);
            }
            Orientation::Straight => p.line(e.end),
        },
        EdgeShape::Straight => p.line(e.end),
    }
    p
}

/// 折叠圆点位置（**节点局部坐标**）与所在侧。`parent_x` 为父盒左边界 x；
/// 根节点传 `None`（默认朝右）。
pub fn toggle_pos(rect: Rect, kind: LayoutKind, parent_x: Option<f32>) -> Point {
    if kind == LayoutKind::OrgChart {
        return Point::new(rect.w / 2.0, rect.h + TOGGLE_OFFSET);
    }
    if kind == LayoutKind::Fishbone {
        return Point::new(-TOGGLE_OFFSET, rect.h / 2.0);
    }
    let dir = parent_x.map_or(1.0, |px| if rect.x >= px { 1.0 } else { -1.0 });
    Point::new(
        if dir > 0.0 {
            rect.w + TOGGLE_OFFSET
        } else {
            -TOGGLE_OFFSET
        },
        rect.h / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{MonoMeasurer, TextMeasurer};
    use crate::tfix::{simple, Builder};
    use crate::theme::WEIGHT_REGULAR;

    /// 测试用测量器：字号 14，每字符 8px，行高 20，内边距与生产一致。
    struct Fix;
    impl NodeMeasure for Fix {
        fn measure(&self, _n: &Node, _d: u32, text: &str) -> BoxSpec {
            let chars = text.chars().count() as f32;
            BoxSpec {
                w: chars * 8.0 + 20.0,
                h: 32.0,
                lines: std::sync::Arc::from(vec![text.to_string()]),
                font_size: 14.0,
                line_height: 20.0,
                bold: false,
                label_h: 0.0,
                image_h: 0.0,
                image_ratio: 16.0 / 9.0,
            }
        }
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

    fn lay(doc: &Document, kind: &str) -> Layout {
        compute_layout(doc, &doc.root_id.clone(), &settings(kind), &Fix, None)
    }

    /// 按 id 取盒子矩形（断言读起来像坐标表，而不是索引查表）。
    fn rect(l: &Layout, id: &str) -> Rect {
        l.boxes[l.find(id).unwrap() as usize].rect
    }

    #[test]
    fn mindmap_direction_can_be_forced_right_or_left() {
        let doc = simple();
        let mut s = settings("mindmap");
        s.direction = "right".into();
        let right = compute_layout(&doc, &doc.root_id, &s, &Fix, None);
        let root = rect(&right, "r");
        for id in ["a", "b", "c"] {
            assert!(rect(&right, id).x > root.right(), "{id} 应在右侧");
        }
        s.direction = "left".into();
        let left = compute_layout(&doc, &doc.root_id, &s, &Fix, None);
        let root = rect(&left, "r");
        for id in ["a", "b", "c"] {
            assert!(rect(&left, id).right() < root.x, "{id} 应在左侧");
        }
    }

    #[test]
    fn root_is_centered_on_origin() {
        let l = lay(&simple(), "logicRight");
        let r = rect(&l, "r");
        assert_eq!(r.x, -r.w / 2.0);
        assert_eq!(r.y, -r.h / 2.0);
    }

    #[test]
    fn logic_right_places_columns_at_level_gap() {
        let l = lay(&simple(), "logicRight");
        let root = rect(&l, "r");
        let a = rect(&l, "a");
        assert_eq!(a.x, root.right() + 80.0);
        // 第 1 层列宽 = 该层最宽节点（a/b/c 同层），第 2 层列位在此之后
        let w1 = l
            .boxes
            .iter()
            .filter(|b| b.depth == 1)
            .fold(0.0f32, |m, b| m.max(b.rect.w));
        let a1 = rect(&l, "a1");
        assert_eq!(a1.x, a.x + w1 + 80.0);
        // 同层节点左边界对齐（列对齐是放射式观感的关键）
        let b = rect(&l, "b");
        let c = rect(&l, "c");
        assert_eq!(a.x, b.x);
        assert_eq!(b.x, c.x);
    }

    #[test]
    fn leaf_siblings_stack_with_sibling_gap() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .child("r", "c", "丙")
            .build();
        let l = lay(&d, "logicRight");
        let mut d1: Vec<&NodeBox> = l.boxes.iter().filter(|b| b.depth == 1).collect();
        d1.sort_by(|x, y| x.rect.y.partial_cmp(&y.rect.y).unwrap());
        for w in d1.windows(2) {
            let gap = w[1].rect.y - w[0].rect.bottom();
            assert!((gap - 24.0).abs() < 0.01, "兄弟间距应为 24，实为 {gap}");
        }
        // 文档顺序 a → b → c 与纵向顺序一致
        assert_eq!(d1[0].lines[0], "甲");
        assert_eq!(d1[2].lines[0], "丙");
    }

    #[test]
    fn parent_centers_on_its_child_band() {
        let l = lay(&simple(), "logicRight");
        let a = rect(&l, "a");
        let a1 = rect(&l, "a1");
        let a2 = rect(&l, "a2");
        let band = Rect::new(0.0, a1.y, 0.0, a2.bottom() - a1.y);
        let expect = band.y + (band.h - a.h) / 2.0;
        assert!((a.y - expect).abs() < 0.01);
    }

    #[test]
    fn mindmap_splits_children_across_two_sides() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .child("r", "c", "丙")
            .child("r", "e", "丁")
            .build();
        let l = lay(&d, "mindmap");
        let root_x = rect(&l, "r").x;
        let mut right = 0;
        let mut left = 0;
        for id in ["a", "b", "c", "e"] {
            let r = rect(&l, id);
            if r.x > root_x {
                right += 1;
            } else {
                left += 1;
                assert!(r.right() <= root_x, "左列应整体在根左侧");
            }
        }
        assert!(
            right >= 2 && left >= 2,
            "四兄弟应分成两侧，实为 {right}/{left}"
        );
    }

    #[test]
    fn org_chart_lays_rows_top_down() {
        let l = lay(&simple(), "orgChart");
        let root = rect(&l, "r");
        let a = rect(&l, "a");
        assert!(a.y > root.bottom(), "子节点应在父节点下方");
        assert_eq!(a.y, rect(&l, "b").y);
        // 同一层所有节点等高（行高由该层最高盒子决定）
        let e = l.edges.first().unwrap();
        assert_eq!(e.orientation, Orientation::Vertical);
        assert_eq!(e.start.x, root.x + root.w / 2.0);
    }

    #[test]
    fn fishbone_alternates_branches_around_a_leftward_spine() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .child("r", "c", "丙")
            .child("r", "e", "丁")
            .build();
        let l = lay(&d, "fishbone");
        let root = rect(&l, "r");
        let a = rect(&l, "a");
        let b = rect(&l, "b");
        let c = rect(&l, "c");
        let e = rect(&l, "e");
        // 叶子分支的带高等于盒高：近边恰好落在轴线外缘
        assert!((a.bottom() + AXIS_GAP).abs() < 0.01, "首分支应在上方");
        assert!((b.y - AXIS_GAP).abs() < 0.01, "次分支应在下方");
        assert!(c.bottom() < a.y, "第三分支继续向上堆叠");
        assert!(e.y > b.bottom(), "第四分支继续向下堆叠");
        for id in ["a", "b", "c", "e"] {
            let r = rect(&l, id);
            assert!(r.right() <= root.x - 80.0 + 0.01, "{id} 应整体在鱼头左侧");
        }
        assert_eq!(l.spines.len(), 1);
        let spine = &l.spines[0];
        assert_eq!(spine.start, Point::new(root.x, 0.0));
        assert!(spine.end().x < root.x);
        let edge = l
            .edges
            .iter()
            .find(|e| e.to == l.find("a").unwrap())
            .unwrap();
        assert_eq!(edge.orientation, Orientation::Straight);
        assert_eq!(edge.start.y, 0.0);
        assert_eq!(edge.end.y, a.bottom());
    }

    #[test]
    fn timeline_runs_branches_left_to_right_around_a_rightward_axis() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .child("r", "c", "丙")
            .child("r", "e", "丁")
            .build();
        let l = lay(&d, "timeline");
        let root = rect(&l, "r");
        let a = rect(&l, "a");
        let b = rect(&l, "b");
        let c = rect(&l, "c");
        let e = rect(&l, "e");
        assert!((a.bottom() + AXIS_GAP).abs() < 0.01);
        assert!((b.y - AXIS_GAP).abs() < 0.01);
        for id in ["a", "b", "c", "e"] {
            let r = rect(&l, id);
            assert!(r.x >= root.right() + 80.0 - 0.01, "{id} 应整体在轴线右侧");
        }
        // 文档顺序即从左到右的时间顺序
        assert!(a.x < b.x && b.x < c.x && c.x < e.x);
        assert!(b.x >= a.right() + 24.0 - 0.01, "相邻分支至少隔一个同级间距");
        assert_eq!(l.spines.len(), 1);
        assert_eq!(l.spines[0].start, Point::new(root.right(), 0.0));
        assert!(l.spines[0].end().x > root.right());
        let edge = l
            .edges
            .iter()
            .find(|e| e.to == l.find("b").unwrap())
            .unwrap();
        assert_eq!(edge.orientation, Orientation::Straight);
        assert_eq!(edge.start, Point::new(edge.end.x, 0.0));
    }

    #[test]
    fn fishbone_and_timeline_grow_descendants_along_the_axis() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("a", "a1", "乙")
            .child("a1", "a11", "丙")
            .build();
        let fish = lay(&d, "fishbone");
        let a = rect(&fish, "a");
        let a1 = rect(&fish, "a1");
        let a11 = rect(&fish, "a11");
        assert!(a1.x < a.x && a11.x < a1.x, "鱼骨后代应继续向左");

        let time = lay(&d, "timeline");
        let a = rect(&time, "a");
        let a1 = rect(&time, "a1");
        let a11 = rect(&time, "a11");
        assert!(a1.x > a.x && a11.x > a1.x, "时间轴后代应继续向右");
        assert_eq!(time.spines.len(), 1);
    }

    #[test]
    fn only_axis_layouts_emit_spines() {
        for kind in ["mindmap", "logicRight", "orgChart"] {
            assert!(lay(&simple(), kind).spines.is_empty(), "{kind}");
        }
        assert_eq!(lay(&simple(), "fishbone").spines.len(), 1);
        assert_eq!(lay(&simple(), "timeline").spines.len(), 1);
    }

    #[test]
    fn straight_edges_are_single_line_segments() {
        let e = Edge {
            from: 0,
            to: 1,
            start: Point::new(10.0, 0.0),
            end: Point::new(30.0, -60.0),
            orientation: Orientation::Straight,
        };
        let p = edge_path(&e);
        assert_eq!(p.segs.len(), 1);
        match p.segs[0] {
            crate::geom::PathSeg::Line { to } => assert_eq!(to, e.end),
            _ => panic!("直线连线不应有控制点"),
        }
    }

    #[test]
    fn fishbone_toggle_sits_on_the_left() {
        let r = Rect::new(100.0, 250.0, 60.0, 30.0);
        assert_eq!(
            toggle_pos(r, LayoutKind::Fishbone, None),
            Point::new(-11.0, 15.0)
        );
        assert_eq!(
            toggle_pos(r, LayoutKind::Fishbone, Some(200.0)),
            Point::new(-11.0, 15.0)
        );
    }

    #[test]
    fn collapsed_subtree_is_not_laid_out() {
        let d = Builder::root("根")
            .collapsed("r", "a", "甲")
            .child("a", "a1", "乙")
            .child("r", "b", "丙")
            .build();
        let l = lay(&d, "logicRight");
        assert_eq!(l.find("a1"), None);
        assert!(l.find("a").is_some());
        assert_eq!(l.boxes.len(), 3);
    }

    #[test]
    fn editing_node_keeps_its_children_visible() {
        let d = Builder::root("根")
            .collapsed("r", "a", "甲")
            .child("a", "a1", "乙")
            .build();
        let l = compute_layout(
            &d,
            &d.root_id.clone(),
            &settings("logicRight"),
            &Fix,
            Some(("a", "输入中")),
        );
        assert!(l.find("a1").is_some());
    }

    #[test]
    fn edges_connect_facing_edges_in_horizontal_layouts() {
        let l = lay(&simple(), "logicRight");
        let e = &l.edges[0];
        let p = l.boxes[e.from as usize].rect;
        let c = l.boxes[e.to as usize].rect;
        // 端点与卡片留缝：起点 +0.5、终点 -1.5（见 EDGE_GAP_*）
        assert_eq!(
            e.start,
            Point::new(p.right() + EDGE_GAP_START, p.y + p.h / 2.0)
        );
        assert_eq!(e.end, Point::new(c.x - EDGE_GAP_END, c.y + c.h / 2.0));
        assert_eq!(l.edges.len(), l.boxes.len() - 1);
    }

    #[test]
    fn bounds_pad_every_side() {
        let l = lay(&simple(), "logicRight");
        let mut enc = Enclosing::new();
        for b in l.boxes.iter() {
            enc.add(b.rect);
        }
        let inner = enc.rect().unwrap();
        assert_eq!(l.bounds, inner.expanded(BOUND_PAD));
    }

    #[test]
    fn missing_root_yields_empty_layout() {
        let d = simple();
        let l = compute_layout(&d, "no-such-id", &settings("mindmap"), &Fix, None);
        assert!(l.boxes.is_empty());
        assert_eq!(l.bounds, Rect::new(-10.0, -10.0, 20.0, 20.0));
    }

    #[test]
    fn gaps_are_clamped_to_sane_minimums() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("r", "b", "乙")
            .build();
        let s = LayoutSettings {
            kind: "logicRight".into(),
            direction: "auto".into(),
            edge_shape: "curve".into(),
            edge_arrow: false,
            sibling_gap: 0.0,
            level_gap: 0.0,
            max_node_width: 300.0,
            focus_question: None,
        };
        let l = compute_layout(&d, &d.root_id.clone(), &s, &Fix, None);
        let a = rect(&l, "a");
        let b = rect(&l, "b");
        assert!(
            (b.y - a.bottom() - 4.0).abs() < 0.01,
            "兄弟间距下限 4，实为 {}",
            b.y - a.bottom()
        );
        let root = rect(&l, "r");
        assert!((a.x - root.right() - 20.0).abs() < 0.01, "层间距下限 20");
    }

    #[test]
    fn elbow_and_straight_edge_shapes_are_distinct() {
        let e = Edge {
            from: 0,
            to: 1,
            start: Point::new(0.0, 10.0),
            end: Point::new(100.0, 30.0),
            orientation: Orientation::Horizontal,
        };
        let elbow = edge_path_with(&e, EdgeShape::Elbow);
        assert_eq!(elbow.segs.len(), 3, "直角线应是三段折线");
        assert_eq!(elbow.end(), e.end);
        let straight = edge_path_with(&e, EdgeShape::Straight);
        assert_eq!(straight.segs.len(), 1);
        let curve = edge_path_with(&e, EdgeShape::Curve);
        assert!(matches!(curve.segs[0], crate::geom::PathSeg::Cubic { .. }));
    }

    #[test]
    fn edge_path_control_points_lie_on_the_axis() {
        let e = Edge {
            from: 0,
            to: 1,
            start: Point::new(0.0, 10.0),
            end: Point::new(100.0, 30.0),
            orientation: Orientation::Horizontal,
        };
        let p = edge_path(&e);
        match p.segs[0] {
            crate::geom::PathSeg::Cubic { c1, c2, to } => {
                assert_eq!(c1, Point::new(50.0, 10.0));
                assert_eq!(c2, Point::new(50.0, 30.0));
                assert_eq!(to, Point::new(100.0, 30.0));
            }
            _ => panic!("水平连线应为三次曲线"),
        }
        assert_eq!(p.end(), e.end);
    }

    #[test]
    fn toggle_pos_follows_side_and_layout_kind() {
        // 返回值是**节点局部坐标**：矩形摆在哪里都不该影响它
        let r = Rect::new(100.0, 250.0, 60.0, 30.0);
        assert_eq!(
            toggle_pos(r, LayoutKind::Mindmap, Some(0.0)),
            Point::new(71.0, 15.0)
        );
        assert_eq!(
            toggle_pos(r, LayoutKind::Mindmap, Some(200.0)),
            Point::new(-11.0, 15.0)
        );
        assert_eq!(
            toggle_pos(r, LayoutKind::OrgChart, None),
            Point::new(30.0, 41.0)
        );
    }

    #[test]
    fn image_reserves_space_above_the_text() {
        let b = NodeBox {
            rect: Rect::new(0.0, 0.0, 180.0, 110.0),
            depth: 0,
            lines: std::sync::Arc::from(vec!["一".to_string(), "二".to_string()]),
            font_size: 14.0,
            line_height: 20.0,
            bold: false,
            label_h: 0.0,
            image_h: 40.0,
            image_ratio: 16.0 / 9.0,
        };
        // 文字区 110-40=70，两行 40 → 起始 40+15，首行基线再 +14.4。
        assert!((line_base_y(&b, 0) - (55.0 + 14.4)).abs() < 0.01);
        assert!((line_base_y(&b, 1) - (75.0 + 14.4)).abs() < 0.01);
    }

    #[test]
    fn line_base_y_is_measured_from_the_box_top() {
        let b = NodeBox {
            rect: Rect::new(0.0, 0.0, 100.0, 60.0),
            depth: 0,
            lines: std::sync::Arc::from(vec!["一".to_string(), "二".to_string()]),
            font_size: 14.0,
            line_height: 20.0,
            bold: false,
            label_h: 10.0,
            image_h: 0.0,
            image_ratio: 16.0 / 9.0,
        };
        // 可用高 50，两行 40 → 起始 5
        assert!((line_base_y(&b, 0) - (5.0 + 14.4)).abs() < 0.01);
        assert!((line_base_y(&b, 1) - (25.0 + 14.4)).abs() < 0.01);
    }

    #[test]
    fn layout_is_stable_across_runs() {
        let a = lay(&simple(), "mindmap");
        let b = lay(&simple(), "mindmap");
        for id in ["r", "a", "b", "c", "a1", "a2", "b1"] {
            assert_eq!(
                a.rect(a.find(id).unwrap()),
                b.rect(b.find(id).unwrap()),
                "{id}"
            );
        }
    }

    #[test]
    fn real_measurer_feeds_the_layout() {
        // 走真实度量端口（等宽近似），确认 BoxSpec 与 TextMeasurer 接线正确
        let m = MonoMeasurer.measure("思维导图", 14.0, WEIGHT_REGULAR, 300.0);
        assert_eq!(m.lines.len(), 1);
        let d = simple();
        struct Real(MonoMeasurer);
        impl NodeMeasure for Real {
            fn measure(&self, _n: &Node, _d: u32, text: &str) -> BoxSpec {
                let m = self.0.measure(text, 14.0, WEIGHT_REGULAR, 300.0);
                BoxSpec {
                    w: m.width + 28.0,
                    h: m.lines.len() as f32 * m.line_height + 18.0,
                    lines: std::sync::Arc::from(m.lines),
                    font_size: 14.0,
                    line_height: m.line_height,
                    bold: false,
                    label_h: 0.0,
                    image_h: 0.0,
                    image_ratio: 16.0 / 9.0,
                }
            }
        }
        let l = compute_layout(
            &d,
            &d.root_id.clone(),
            &settings("logicRight"),
            &Real(MonoMeasurer),
            None,
        );
        let a = rect(&l, "a");
        assert!((a.w - (3.0 * 14.0 + 28.0)).abs() < 0.01);
    }
}
