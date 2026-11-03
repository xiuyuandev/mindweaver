//! uniform grid 命中测试（docs/12 §3.1）。
//!
//! 只消费 [`Scene`]：所有目标的世界坐标包围盒登记进 128 世界单位见方的网格，
//! 查询时按「折叠点 > 关系线 > 摘要主题 > 标签胶囊 > 节点 > 花括号 > 边界」的
//! 优先级取唯一目标，同级按绘制顺序反序（后画的在上面）。
//!
//! 容差是**屏幕空间**的 CSS px（关系线 14、花括号 8、边界边框 5），查询时经
//! [`Camera::css_to_world`] 换算，所以放大后不会变得「隔老远也能点中」。
//! 曲线在构建时按 1 世界单位的平直度折线化一次，命中不再碰贝塞尔细分。

use std::collections::HashMap;

use mw_core::layout::toggle_pos;
use mw_core::{arrow_at_zoom, rounded_rect_contains, Path, Point, Rect, Scene};

use crate::camera::Camera;
use crate::flatten::flatten;
use crate::instances::{REL_LABEL_H, REL_LABEL_MIN_W, TOGGLE_HALO, TOGGLE_R};

/// 网格边长（世界单位）。128 与 docs/12 §3.1 一致。
pub const CELL: f32 = 128.0;
/// 一个目标横跨超过这么多格就放进「大件」列表，避免一个全图边界把网格撑爆。
const MAX_CELLS_PER_ENTRY: usize = 256;

/// 关系线命中容差（CSS px）。
pub const REL_HIT_CSS: f32 = 14.0;
/// 花括号 / 摘要连接线命中容差（CSS px）。
pub const BRACE_HIT_CSS: f32 = 8.0;
/// 边界描边命中带（CSS px）。
pub const BOUNDARY_HIT_CSS: f32 = 5.0;
/// 折叠圆点命中半径（CSS px）：画出来是 r8 + 1.5px 底色光晕，给 2px 手感余量。
pub const TOGGLE_HIT_CSS: f32 = TOGGLE_R + 2.0;
/// 视觉半径不能超过命中半径，否则「看得见却点不中」。
const _: () = assert!(TOGGLE_HIT_CSS >= TOGGLE_R + TOGGLE_HALO);
/// 命中也不许比视觉大出 2px 以上（X28 D7）：圆点优先级高于节点，太胖会误折子树。
const _: () = assert!(TOGGLE_HIT_CSS <= TOGGLE_R + TOGGLE_HALO + 2.0);

/// 命中的对象。所有下标都是 [`Scene`] 的密集索引。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// 折叠圆点 → 节点索引
    Toggle(u32),
    /// 关系线（曲线 / 箭头 / 标签）
    Relationship(u32),
    /// 摘要主题盒
    SummaryTopic(u32),
    /// 节点下方的标签胶囊（节点索引、标签序号）
    Label(u32, u32),
    /// 节点盒
    Node(u32),
    /// 概要花括号或摘要连接线
    SummaryBrace(u32),
    /// 边界描边
    Boundary(u32),
    /// 概念分组框的描边（CM-18）。只命中框线，框内空白仍归画布（否则框选会被吃掉）。
    Group(u32),
}

impl Target {
    fn priority(self) -> u8 {
        match self {
            Self::Toggle(_) => 7,
            Self::Relationship(_) => 6,
            Self::SummaryTopic(_) => 5,
            Self::Label(_, _) => 4,
            Self::Node(_) => 3,
            Self::SummaryBrace(_) => 2,
            Self::Boundary(_) => 1,
            // 分组框是最外层的容器，压在所有东西下面。
            Self::Group(_) => 0,
        }
    }
}

/// 命中结果映射回可高亮的节点：点标签、折叠点、摘要与边界时高亮其归属节点。
pub fn owner_node(scene: &Scene, t: Target) -> Option<u32> {
    match t {
        Target::Toggle(n) | Target::Label(n, _) | Target::Node(n) => Some(n),
        Target::SummaryTopic(s) | Target::SummaryBrace(s) => {
            scene.summaries.get(s as usize).map(|s| s.owner)
        }
        Target::Relationship(r) => scene.relationships.get(r as usize).map(|r| r.from),
        Target::Boundary(b) => scene.boundaries.get(b as usize).map(|b| b.owner),
        // 分组不属于某一个节点：它圈住的是一组概念。
        Target::Group(_) => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    priority: u8,
    z: u32,
}

enum Shape {
    Rect(Rect),
    /// 圆角矩形 / 胶囊：四角按圆弧判定（X23 G-8）。半径由 `corner_style` 给出，
    /// 与 wgpu/SVG/PDF/编辑环同源；圆角或胶囊的节点不再把四角外接矩形算成命中。
    RoundedRect {
        rect: Rect,
        radius: f32,
    },
    /// 椭圆（概念图的默认节点形状）。按外接矩形建，但判定要落进椭圆方程里：
    /// 四个角在矩形内、在椭圆外，那些像素**不该**算命中——概念图是自由布局，
    /// 两颗概念可以挨得很近，拿外接矩形当命中会抢走别人角落上的点击。
    Ellipse(Rect),
    /// 屏幕恒定半径的圆（折叠点）。
    Circle {
        center: Point,
        radius_css: f32,
        floor: f32,
    },
    /// 矩形描边带：外扩与内缩之间。
    Band {
        rect: Rect,
        pad_css: f32,
    },
    /// 折线（构建时已细分）。
    Curve {
        points: Vec<Point>,
        tol_css: f32,
    },
    Triangle([Point; 3]),
}

impl Shape {
    fn bounds(&self) -> Rect {
        match self {
            Self::Rect(r) | Self::Ellipse(r) => *r,
            Self::RoundedRect { rect, .. } => *rect,
            Self::Circle {
                center,
                radius_css,
                floor,
            } => {
                // 构建期不知道相机，用「大缩放」下的半径下界做包围盒，查询时还会再外扩。
                let r = radius_css.max(*floor);
                Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0)
            }
            Self::Band { rect, pad_css } => rect.expanded(*pad_css),
            Self::Curve { points, tol_css } => bounds_of(points).expanded(*tol_css),
            Self::Triangle(p) => bounds_of(p),
        }
    }

    fn hit(&self, p: Point, cam: Camera) -> bool {
        match self {
            Self::Rect(r) => r.contains(p),
            Self::RoundedRect { rect, radius } => rounded_rect_contains(*rect, *radius, p),
            Self::Ellipse(r) => {
                let (a, b) = (r.w * 0.5, r.h * 0.5);
                if a <= 0.0 || b <= 0.0 {
                    return false;
                }
                let dx = (p.x - r.center().x) / a;
                let dy = (p.y - r.center().y) / b;
                dx * dx + dy * dy <= 1.0
            }
            Self::Circle {
                center,
                radius_css,
                floor,
            } => p.dist(*center) <= cam.chrome(*radius_css, *floor),
            Self::Band { rect, pad_css } => {
                let pad = cam.css_to_world(*pad_css);
                let outer = rect.expanded(pad);
                let inner = rect.expanded(-pad);
                outer.contains(p) && !inner.contains(p)
            }
            Self::Curve { points, tol_css } => {
                dist_to_polyline(p, points) <= cam.css_to_world(*tol_css)
            }
            Self::Triangle(t) => {
                // 箭头要屏幕恒定，绘制与导出都走同一个换算（`arrow_at_zoom`）。
                // 登记时的包围盒仍用自然三角：放大后最多外探 `7/zoom - 6 < 16/zoom`
                // 世界单位，永远小于查询侧 `REL_HIT_CSS + 2` 的外扩，不会漏格。
                point_in_triangle(p, arrow_at_zoom(*t, cam.zoom))
            }
        }
    }
}

struct Entry {
    target: Target,
    rank: Rank,
    shape: Shape,
    bounds: Rect,
}

/// 场景的命中索引。随 `Scene` 一起重建；本身是纯 CPU 数据。
pub struct HitIndex {
    grid: HashMap<(i32, i32), Vec<u32>>,
    oversized: Vec<u32>,
    entries: Vec<Entry>,
}

impl HitIndex {
    pub fn new(scene: &Scene) -> Self {
        let mut me = Self {
            grid: HashMap::new(),
            oversized: Vec::new(),
            entries: Vec::new(),
        };

        for (i, g) in scene.groups.iter().enumerate() {
            me.push(
                Target::Group(i as u32),
                Rank {
                    priority: Target::Group(i as u32).priority(),
                    z: i as u32,
                },
                Shape::Band {
                    rect: g.rect,
                    pad_css: BOUNDARY_HIT_CSS,
                },
            );
        }

        for (i, b) in scene.boundaries.iter().enumerate() {
            me.push(
                Target::Boundary(i as u32),
                Rank {
                    priority: Target::Boundary(i as u32).priority(),
                    z: i as u32,
                },
                Shape::Band {
                    rect: b.rect,
                    pad_css: BOUNDARY_HIT_CSS,
                },
            );
        }

        for (i, s) in scene.summaries.iter().enumerate() {
            let z = i as u32;
            let rank = Rank {
                priority: Target::SummaryBrace(i as u32).priority(),
                z,
            };
            me.push_curve(
                Target::SummaryBrace(i as u32),
                rank,
                &s.brace,
                BRACE_HIT_CSS,
            );
            me.push_curve(
                Target::SummaryBrace(i as u32),
                rank,
                &s.connector,
                BRACE_HIT_CSS,
            );
            me.push(
                Target::SummaryTopic(i as u32),
                Rank {
                    priority: Target::SummaryTopic(i as u32).priority(),
                    z,
                },
                Shape::Rect(s.topic),
            );
        }

        for (i, r) in scene.relationships.iter().enumerate() {
            let z = i as u32;
            let rank = Rank {
                priority: Target::Relationship(i as u32).priority(),
                z,
            };
            me.push_curve(Target::Relationship(i as u32), rank, &r.path, REL_HIT_CSS);
            // ⑦ 之后箭头有两个可能的位置，各自单独入命中表。
            for tri in [r.arrow, r.arrow_back].into_iter().flatten() {
                me.push(Target::Relationship(i as u32), rank, Shape::Triangle(tri));
            }
            let w = r.label_w.max(REL_LABEL_MIN_W);
            me.push(
                Target::Relationship(i as u32),
                rank,
                Shape::Rect(Rect::new(
                    r.label_at.x - w * 0.5 - 2.0,
                    r.label_at.y - REL_LABEL_H * 0.5 - 2.0,
                    w + 4.0,
                    REL_LABEL_H + 4.0,
                )),
            );
        }

        // 节点与折叠点：draw_order 越靠后 z 越大，命中时压在上面。
        for (z, &idx) in scene.draw_order.iter().enumerate() {
            let n = scene.node(idx);
            let z = z as u32;
            me.push(
                Target::Node(idx),
                Rank {
                    priority: Target::Node(idx).priority(),
                    z,
                },
                if n.style.shape.is_ellipse() {
                    Shape::Ellipse(n.geom.rect)
                } else {
                    // 圆角/胶囊与绘制同源：半径读 `corner_style`，四角按圆弧命中。
                    let radius = crate::instances::corner_style(&n.style, n.geom.rect);
                    if radius > 0.0 {
                        Shape::RoundedRect {
                            rect: n.geom.rect,
                            radius,
                        }
                    } else {
                        Shape::Rect(n.geom.rect)
                    }
                },
            );
            for (li, p) in n.labels.iter().enumerate() {
                let world = Rect::new(
                    n.geom.rect.x + p.rect.x,
                    n.geom.rect.y + p.rect.y,
                    p.rect.w,
                    p.rect.h,
                );
                me.push(
                    Target::Label(idx, li as u32),
                    Rank {
                        priority: Target::Label(idx, li as u32).priority(),
                        z,
                    },
                    Shape::Rect(world),
                );
            }
            if n.has_children {
                if let Some(parent) = n.parent.and_then(|p| scene.nodes.get(p as usize)) {
                    let local = toggle_pos(n.geom.rect, scene.kind, Some(parent.geom.rect.x));
                    me.push(
                        Target::Toggle(idx),
                        Rank {
                            priority: Target::Toggle(idx).priority(),
                            z,
                        },
                        Shape::Circle {
                            center: Point::new(n.geom.rect.x + local.x, n.geom.rect.y + local.y),
                            radius_css: TOGGLE_HIT_CSS,
                            floor: 4.5,
                        },
                    );
                }
            }
        }

        me
    }

    fn push(&mut self, target: Target, rank: Rank, shape: Shape) {
        let bounds = shape.bounds();
        let id = self.entries.len() as u32;
        self.entries.push(Entry {
            target,
            rank,
            shape,
            bounds,
        });

        let (c0, c1) = cell_range(bounds);
        if cell_span_overflows(c0, c1, MAX_CELLS_PER_ENTRY) {
            self.oversized.push(id);
            return;
        }
        for cy in c0.1..=c1.1 {
            for cx in c0.0..=c1.0 {
                self.grid.entry((cx, cy)).or_default().push(id);
            }
        }
    }

    fn push_curve(&mut self, target: Target, rank: Rank, path: &Path, tol_css: f32) {
        // 1 世界单位的平直度：容差最小也有 8/8 = 1 世界单位，足够。
        let points = flatten(path, 1.0);
        self.push(target, rank, Shape::Curve { points, tol_css });
    }

    /// 世界里的一点 → 最上层命中的目标。
    pub fn hit(&self, cam: Camera, p: Point) -> Option<Target> {
        // 最大容差 14 CSS px，再给标签胶囊的 2px 外扩留余量。
        let expand = cam.css_to_world(REL_HIT_CSS + 2.0);
        let q = Rect::new(p.x - expand, p.y - expand, expand * 2.0, expand * 2.0);
        let (c0, c1) = cell_range(q);

        let mut best: Option<(Rank, Target)> = None;
        let mut consider = |id: u32| {
            let e = &self.entries[id as usize];
            if !e.bounds.expanded(expand).contains(p) || !e.shape.hit(p, cam) {
                return;
            }
            if best.map_or(true, |(r, _)| e.rank > r) {
                best = Some((e.rank, e.target));
            }
        };

        let mut seen_oversized = false;
        for cy in c0.1..=c1.1 {
            for cx in c0.0..=c1.0 {
                if let Some(ids) = self.grid.get(&(cx, cy)) {
                    for &id in ids {
                        consider(id);
                    }
                }
            }
            if !seen_oversized {
                for &id in &self.oversized {
                    consider(id);
                }
                seen_oversized = true;
            }
        }
        if !seen_oversized {
            for &id in &self.oversized {
                consider(id);
            }
        }
        best.map(|(_, t)| t)
    }

    /// 已登记的目标数，测试用来确认空场景/大件路径都走到了。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn cell_of(p: Point) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

fn cell_range(r: Rect) -> ((i32, i32), (i32, i32)) {
    (
        cell_of(Point::new(r.x, r.y)),
        cell_of(Point::new(r.right(), r.bottom())),
    )
}

/// 网格覆盖是否超过上限。
///
/// 用 `i128` 算：`f32 as i32` 会饱和到 `i32::MIN/MAX`，损坏文件里两个极端坐标
/// 一相减就在 `i32` 上溢出——debug 直接 panic，release 回绕成小正数后又会去遍历
/// 几十亿个格子（卡死）。这里只回答「超没超」，不关心确切数目。
fn cell_span_overflows(c0: (i32, i32), c1: (i32, i32), max: usize) -> bool {
    let w = c1.0 as i128 - c0.0 as i128 + 1;
    let h = c1.1 as i128 - c0.1 as i128 + 1;
    w * h > max as i128
}

fn bounds_of(points: &[Point]) -> Rect {
    if points.is_empty() {
        return Rect::default();
    }
    let mut min = points[0];
    let mut max = points[0];
    for p in &points[1..] {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
}

fn dist_to_polyline(p: Point, points: &[Point]) -> f32 {
    if points.is_empty() {
        return f32::INFINITY;
    }
    if points.len() == 1 {
        return p.dist(points[0]);
    }
    points
        .windows(2)
        .map(|w| dist_to_segment(p, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-9 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.dist(Point::new(a.x + dx * t, a.y + dy * t))
}

fn point_in_triangle(p: Point, t: [Point; 3]) -> bool {
    let sign = |a: Point, b: Point| (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
    let d1 = sign(t[0], t[1]);
    let d2 = sign(t[1], t[2]);
    let d3 = sign(t[2], t[0]);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::text::MonoMeasurer;
    use mw_core::{build_scene, theme::get_theme, Command, Document, LayoutSettings};

    fn id_of(doc: &Document, text: &str) -> String {
        doc.doc_order()
            .into_iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == text))
            .unwrap_or_else(|| panic!("没有「{text}」"))
    }

    /// 根 + 左右三个子节点 + 跨根关系线 + 边界 + 概要 + 标签。
    fn scene() -> Scene {
        let mut doc = Document::new("中心主题");
        let root = doc.root_id.clone();
        doc.apply(&Command::SetText {
            id: root.clone(),
            text: "中心主题".into(),
        })
        .unwrap();
        for t in ["左分支", "右分支", "目标"] {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: t.into(),
                style: None,
            })
            .unwrap();
        }
        doc.apply(&Command::AddNode {
            parent: id_of(&doc, "左分支"),
            index: None,
            text: "叶子".into(),
            style: None,
        })
        .unwrap();
        doc.apply(&Command::AddLabel {
            ids: vec![id_of(&doc, "左分支")],
            label: "重要".into(),
        })
        .unwrap();
        doc.apply(&Command::AddSummary {
            parent: root.clone(),
            targets: vec![id_of(&doc, "左分支"), id_of(&doc, "目标")],
            text: "小结".into(),
        })
        .unwrap();
        doc.apply(&Command::AddBoundary {
            parent: root,
            targets: vec![id_of(&doc, "右分支")],
            color: None,
        })
        .unwrap();
        // 左→右跨过根：根内的关系线点必须命中关系线而不是节点
        doc.apply(&Command::AddRelationship {
            from: id_of(&doc, "左分支"),
            to: id_of(&doc, "目标"),
            label: Some("关联".into()),
        })
        .unwrap();
        let theme = get_theme("snow");
        build_scene(
            &doc,
            &doc.root_id,
            &LayoutSettings::default(),
            &theme,
            &MonoMeasurer,
            None::<(&str, &str)>,
        )
    }

    fn cam() -> Camera {
        Camera::new(Point::ZERO, 1.0, 1.0)
    }

    #[test]
    fn node_center_hits_its_node() {
        let s = scene();
        let h = HitIndex::new(&s);
        for i in 0..s.nodes.len() as u32 {
            let c = s.node(i).geom.rect.center();
            assert_eq!(h.hit(cam(), c), Some(Target::Node(i)), "节点 {i}");
        }
        assert!(!h.is_empty());
    }

    #[test]
    fn toggle_center_hits_toggle_and_beats_parent() {
        let s = scene();
        let h = HitIndex::new(&s);
        let n = s
            .nodes
            .iter()
            .position(|n| n.has_children && n.parent.is_some())
            .map(|i| i as u32)
            .expect("至少要有一个带父节点的可折叠节点");
        let node = s.node(n);
        let parent_x = s.node(node.parent.unwrap()).geom.rect.x;
        let local = toggle_pos(node.geom.rect, s.kind, Some(parent_x));
        let p = Point::new(node.geom.rect.x + local.x, node.geom.rect.y + local.y);
        assert_eq!(h.hit(cam(), p), Some(Target::Toggle(n)));
    }

    /// 打磨轮 8：折叠圆点的热区是**屏幕常量**——画出来 r8 的环、命中 r10，
    /// 缩放到 50% 或 200% 时这个「看得见就点得中」的关系不变。
    ///
    /// 以前的单测只钉住「圆心命中」，看不出半径到底随不随缩放走：把
    /// `TOGGLE_HIT_CSS` 错写成世界单位，圆心那条照样过。
    #[test]
    fn toggle_hit_ring_keeps_its_screen_size_across_zoom() {
        let s = scene();
        let h = HitIndex::new(&s);
        let n = s
            .nodes
            .iter()
            .position(|n| n.has_children && n.parent.is_some())
            .map(|i| i as u32)
            .expect("至少要有一个带父节点的可折叠节点");
        let node = s.node(n);
        let parent_x = s.node(node.parent.unwrap()).geom.rect.x;
        let local = toggle_pos(node.geom.rect, s.kind, Some(parent_x));
        let c = Point::new(node.geom.rect.x + local.x, node.geom.rect.y + local.y);

        for zoom in [0.5, 1.0, 2.0] {
            let cam = Camera::new(Point::ZERO, zoom, 1.0);
            // 8 CSS px：比画出来的环（r8）略小，必须还能点中。
            let near = Point::new(c.x, c.y + 8.0 / zoom);
            assert_eq!(
                h.hit(cam, near),
                Some(Target::Toggle(n)),
                "zoom={zoom} 时半径内 8px 应命中折叠点"
            );
            // 12 CSS px：已经出圈，不能再把点击吞成「折叠」。
            let far = Point::new(c.x, c.y + 12.0 / zoom);
            assert_ne!(
                h.hit(cam, far),
                Some(Target::Toggle(n)),
                "zoom={zoom} 时半径外 12px 不该命中折叠点"
            );
        }
    }

    /// BUG-5：箭头三处不同源。同一 Scene 在 zoom 0.5 / 1.0 / 2.0 下，
    /// 画布画出来的三角与命中表实际判定的三角必须逐点相同。
    ///
    /// 改前画布在 `instances.rs` 里私有放大到 ≥7 css，命中与导出却读 Scene 的原始
    /// 6×4 世界三角——屏幕上的箭头看得见、低缩放时点不中，导出又比画布短一截。
    /// 这里同时校验绘制侧与命中侧，并把「放大后仍可点中」的行为一起钉住。
    #[test]
    fn drawn_arrow_triangles_match_the_hit_triangles_across_zoom() {
        let s = scene();
        let h = HitIndex::new(&s);
        let theme = get_theme("snow");
        for zoom in [0.5_f32, 1.0, 2.0] {
            let frame = crate::Frame {
                scene: &s,
                theme: &theme,
                camera: Camera::new(s.bounds.center(), zoom, 1.0),
                selected: Vec::new(),
                hovered: None,
                hover_alpha: 1.0,
                select_alpha: 1.0,
                editing: None,
                ghost: None,
                guides: Vec::new(),
                viewport: None,
            };
            let layers = crate::build_layers(&frame);
            let drawn: Vec<[Point; 3]> = layers[crate::L_OVERLAY]
                .tris
                .iter()
                .map(|t| t.p.map(|p| Point::new(p[0], p[1])))
                .collect();
            let registered: Vec<[Point; 3]> = h
                .entries
                .iter()
                .filter_map(|e| match e.shape {
                    Shape::Triangle(t) => Some(mw_core::arrow_at_zoom(t, zoom)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                drawn.len(),
                registered.len(),
                "zoom={zoom}：每条箭头都要有绘制与命中两份"
            );
            for (i, (d, r)) in drawn.iter().zip(registered.iter()).enumerate() {
                for k in 0..3 {
                    assert!(
                        (d[k].x - r[k].x).abs() < 1e-3 && (d[k].y - r[k].y).abs() < 1e-3,
                        "zoom={zoom} 第 {i} 条箭头第 {k} 个角：绘制 {d:?} vs 命中 {r:?}"
                    );
                }
            }
        }
    }

    /// 复核项 4 的证伪/证实：网格查询不该漏掉「暴力枚举能找到」的命中。
    ///
    /// 登记区间（`Shape::bounds`）对折叠圆点与描边带用的是世界单位常数，
    /// 而判定侧（`Shape::hit`）按相机换算；`zoom < 1` 时判定圈会比登记区间大。
    /// 查询侧有一层 16 CSS px 的统一外扩，理论上能把这层差距盖住——这条用
    /// 「网格结果 vs 全量枚举」在几个缩放档位上逐点对账，不靠推断。
    #[test]
    fn grid_query_finds_every_hit_the_brute_force_finds() {
        let s = scene();
        let mut h = HitIndex::new(&s);
        // 贴网格边界最坏的情形：把同一颗折叠圆点摆在 128 世界单位的各种余数上
        // （登记只覆盖它的世界半径，判定圈在 `zoom < 1` 时更大，正好卡在边界上）。
        for k in 0..3 {
            for off in [0.0f32, 1.0, 63.0, 64.0, 127.0] {
                h.push(
                    Target::Toggle(0),
                    Rank { priority: 7, z: 0 },
                    Shape::Circle {
                        center: Point::new(k as f32 * CELL + off, off),
                        radius_css: TOGGLE_HIT_CSS,
                        floor: 4.5,
                    },
                );
            }
        }
        // 显式对账每颗合成圆点：在判定圈内 90% 半径处八个方向都必须命中。
        let centers: Vec<Point> = (0..3)
            .flat_map(|k| {
                [0.0f32, 1.0, 63.0, 64.0, 127.0].map(|off| Point::new(k as f32 * CELL + off, off))
            })
            .collect();
        for zoom in [0.01f32, 0.1, 0.5, 1.0] {
            let cam = Camera::new(Point::ZERO, zoom, 1.0);
            let r = cam.chrome(TOGGLE_HIT_CSS, 4.5) * 0.9;
            for c in &centers {
                for (dx, dy) in [
                    (1.0, 0.0),
                    (-1.0, 0.0),
                    (0.0, 1.0),
                    (0.0, -1.0),
                    (0.7, 0.7),
                    (-0.7, 0.7),
                    (0.7, -0.7),
                    (-0.7, -0.7),
                ] {
                    let p = Point::new(c.x + dx * r, c.y + dy * r);
                    assert!(
                        h.hit(cam, p).is_some(),
                        "zoom={zoom} 时圆点 {c:?} 的判定圈内漏命中：{p:?}"
                    );
                }
            }
        }

        for zoom in [0.1f32, 0.25, 0.5, 1.0, 2.0] {
            let cam = Camera::new(Point::ZERO, zoom, 1.0);
            let b = s.bounds.expanded(300.0);
            let step = 24.0;
            let mut y = b.y;
            while y <= b.bottom() {
                let mut x = b.x;
                while x <= b.right() {
                    let p = Point::new(x, y);
                    let brute = h
                        .entries
                        .iter()
                        .filter(|e| e.shape.hit(p, cam))
                        .map(|e| e.rank)
                        .max();
                    if brute.is_some() {
                        assert!(
                            h.hit(cam, p).is_some(),
                            "zoom={zoom} 时网格漏了暴力枚举能命中的点：({x}, {y})"
                        );
                    }
                    x += step;
                }
                y += step;
            }
        }
    }

    #[test]
    fn relationship_beats_node_when_they_overlap() {
        let s = scene();
        let h = HitIndex::new(&s);
        // 曲线端点落在节点盒边框上 → 两者重叠，关系线优先
        let mut checked = false;
        for (i, r) in s.relationships.iter().enumerate() {
            for p in flatten(&r.path, 1.0) {
                if s.nodes.iter().any(|n| n.geom.rect.contains(p)) {
                    assert_eq!(h.hit(cam(), p), Some(Target::Relationship(i as u32)));
                    checked = true;
                }
            }
        }
        assert!(checked, "这组夹具的关系线端点应压在节点盒边框上");
    }

    #[test]
    fn label_pill_and_summary_topic_are_hit_before_their_background() {
        let s = scene();
        let h = HitIndex::new(&s);
        for (i, n) in s.nodes.iter().enumerate() {
            for (li, p) in n.labels.iter().enumerate() {
                let c = Point::new(
                    n.geom.rect.x + p.rect.x + p.rect.w * 0.5,
                    n.geom.rect.y + p.rect.y + p.rect.h * 0.5,
                );
                assert_eq!(h.hit(cam(), c), Some(Target::Label(i as u32, li as u32)));
            }
        }
        for (i, sum) in s.summaries.iter().enumerate() {
            assert_eq!(
                h.hit(cam(), sum.topic.center()),
                Some(Target::SummaryTopic(i as u32))
            );
        }
    }

    #[test]
    fn brace_and_boundary_edges_are_hit() {
        let s = scene();
        let h = HitIndex::new(&s);
        let sum = &s.summaries[0];
        let on_brace = sum.brace.start;
        assert_eq!(h.hit(cam(), on_brace), Some(Target::SummaryBrace(0)));
        let b = &s.boundaries[0];
        // 边界框左边缘上的一点（避开节点）
        let p = Point::new(b.rect.x + 1.0, b.rect.y + b.rect.h * 0.5);
        assert_eq!(h.hit(cam(), p), Some(Target::Boundary(0)));
        // 边界内部空白不抢点击：中心落在节点上，空白处不命中边界
        assert!(!matches!(
            h.hit(cam(), b.rect.center()),
            Some(Target::Boundary(0))
        ));
        let blank = Point::new(b.rect.x + 12.0, b.rect.y + 12.0);
        assert!(
            !s.nodes.iter().any(|n| n.geom.rect.contains(blank)),
            "夹具变了：取的点落在节点上"
        );
        assert_eq!(h.hit(cam(), blank), None);
    }

    #[test]
    fn tolerance_is_screen_space_not_world_space() {
        let s = scene();
        let h = HitIndex::new(&s);
        let r = &s.relationships[0];
        let a = r.path.start;
        let b = r.path.end();
        let len = a.dist(b).max(1.0);
        let n = Point::new(-(b.y - a.y) / len, (b.x - a.x) / len);
        // 世界单位偏移 10：zoom 1（容差 14 世界）命中，zoom 2（容差 7 世界）落空
        let p = Point::new(a.x + n.x * 10.0, a.y + n.y * 10.0);
        let at1 = h.hit(Camera::new(Point::ZERO, 1.0, 1.0), p);
        let at2 = h.hit(Camera::new(Point::ZERO, 2.0, 1.0), p);
        assert!(matches!(at1, Some(Target::Relationship(_))), "{at1:?}");
        assert!(!matches!(at2, Some(Target::Relationship(_))), "{at2:?}");
    }

    /// 平行命题各自命中：点第一条车道选第一条，点第二条选第二条。
    ///
    /// C14 让两条线在画面上分开；这里钉住「分开之后各自能被点中」。
    /// 反向命题（B→A）与正向命题同属一对概念，必须进同一组车道。
    #[test]
    fn parallel_propositions_are_individually_hittable() {
        use mw_core::scene::build_scene;
        let mut doc = Document::new("图");
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
            text: "图".into(),
        })
        .unwrap();
        for (x, y, text) in [(0.0, -220.0, "甲"), (0.0, 220.0, "乙")] {
            doc.apply(&Command::AddFloatingTopic {
                text: text.into(),
                x,
                y,
            })
            .unwrap();
        }
        let from = id_of(&doc, "甲");
        let to = id_of(&doc, "乙");
        // 一正一反：既验车道，也验反向命题没有被漏掉。
        doc.apply(&Command::AddRelationship {
            from: from.clone(),
            to: to.clone(),
            label: Some("导致".into()),
        })
        .unwrap();
        doc.apply(&Command::AddRelationship {
            from: to,
            to: from,
            label: Some("支持".into()),
        })
        .unwrap();
        let theme = get_theme("snow");
        let settings = LayoutSettings {
            kind: "conceptMap".into(),
            ..Default::default()
        };
        let s = build_scene(
            &doc,
            &doc.root_id,
            &settings,
            &theme,
            &MonoMeasurer,
            None::<(&str, &str)>,
        );
        assert_eq!(s.relationships.len(), 2, "两条命题都要进场景");
        let h = HitIndex::new(&s);
        for (i, r) in s.relationships.iter().enumerate() {
            let p = halfway_point(&r.path);
            assert_eq!(
                h.hit(cam(), p),
                Some(Target::Relationship(i as u32)),
                "第 {i} 条车道的路径中点必须命中自己"
            );
        }
    }

    fn halfway_point(path: &Path) -> Point {
        let pts = flatten(path, 0.5);
        let total: f32 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
        let mut acc = 0.0;
        for w in pts.windows(2) {
            let d = w[0].dist(w[1]);
            if acc + d >= total * 0.5 {
                let t = ((total * 0.5 - acc) / d.max(1e-6)).clamp(0.0, 1.0);
                return Point::new(
                    w[0].x + (w[1].x - w[0].x) * t,
                    w[0].y + (w[1].y - w[0].y) * t,
                );
            }
            acc += d;
        }
        *pts.last().unwrap()
    }

    #[test]
    fn empty_space_misses_and_owner_maps_back() {
        let s = scene();
        let h = HitIndex::new(&s);
        let p = Point::new(s.bounds.right() + 500.0, s.bounds.bottom() + 500.0);
        assert_eq!(h.hit(cam(), p), None);
        assert!(owner_node(&s, Target::Relationship(0)).is_some());
        assert!(owner_node(&s, Target::Boundary(0)).is_some());
        assert!(owner_node(&s, Target::SummaryBrace(0)).is_some());
    }

    /// 损坏文件里极端的 `f32` 坐标经 `as i32` 会饱和到 `i32::MIN/MAX`，
    /// 两个极端格子相减在 `i32` 上溢出：debug panic、release 回绕成小正数。
    /// 这里钉住判定本身不溢出，而且能正确回答「超没超」。
    #[test]
    fn cell_span_check_saturates_instead_of_overflowing() {
        // 整条 `i32` 轴：真实格子数（2^32）远超上限。
        assert!(cell_span_overflows(
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
            MAX_CELLS_PER_ENTRY
        ));
        // 单轴极端但另一轴只有 1 格，仍是 2^32 格，仍超。
        assert!(cell_span_overflows(
            (i32::MIN, 0),
            (i32::MAX, 0),
            MAX_CELLS_PER_ENTRY
        ));
        // 正常小范围：8×8 = 64 ≤ 256，不超。
        assert!(!cell_span_overflows((0, 0), (7, 7), MAX_CELLS_PER_ENTRY));
        // 边界：16×16 = 256 恰好等于上限，不超；16×17 = 272 超。
        assert!(!cell_span_overflows((0, 0), (15, 15), MAX_CELLS_PER_ENTRY));
        assert!(cell_span_overflows((0, 0), (16, 16), MAX_CELLS_PER_ENTRY));
    }
}
