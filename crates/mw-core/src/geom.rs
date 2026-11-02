//! 基础几何：点 / 矩形 / 包围盒 / 路径分段。
//!
//! 路径以**几何分段**表达，而不是 SVG 字符串：画布把它细分成三角形，导出器把它序列化成
//! SVG/PDF 曲线，两者读同一份数据（docs/13 §9「导出与画布同源」）。
//! 坐标一律为世界坐标、逻辑像素；缩放由渲染层施加。核心只在这里收一个**显式缩放参数**
//! （[`arrow_at_zoom`]）：关系线箭头要屏幕恒定，绘制 / 命中 / 导出必须读同一份换算，
//! 而这三处分布在 mw-render 与 mw-shell，所以唯一实现留在 core。

/// 二维点（也是向量）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dist(self, o: Self) -> f32 {
        (self.x - o.x).hypot(self.y - o.y)
    }
}

/// 轴对齐矩形（左上角 + 宽高）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn center(self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    /// 四边等量外扩（负值即内缩）。
    pub fn expanded(self, pad: f32) -> Self {
        Self::new(
            self.x - pad,
            self.y - pad,
            self.w + pad * 2.0,
            self.h + pad * 2.0,
        )
    }

    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// 是否与另一矩形相交（贴边也算；零面积矩形可以表示点/线段）。
    pub fn intersects(self, o: Self) -> bool {
        self.x <= o.right() && o.x <= self.right() && self.y <= o.bottom() && o.y <= self.bottom()
    }

    /// 两矩形并集。
    pub fn union(self, o: Self) -> Self {
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = self.right().max(o.right());
        let y1 = self.bottom().max(o.bottom());
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// 增量包围盒。空集合用 `None` 表达，避免拿 `Rect::default()` 当「零宽高」的假数据。
#[derive(Clone, Copy, Debug, Default)]
pub struct Enclosing {
    min: Point,
    max: Point,
    empty: bool,
}

impl Enclosing {
    pub fn new() -> Self {
        Self {
            min: Point::ZERO,
            max: Point::ZERO,
            empty: true,
        }
    }

    pub fn add(&mut self, r: Rect) {
        if self.empty {
            self.min = Point::new(r.x, r.y);
            self.max = Point::new(r.right(), r.bottom());
            self.empty = false;
            return;
        }
        self.min.x = self.min.x.min(r.x);
        self.min.y = self.min.y.min(r.y);
        self.max.x = self.max.x.max(r.right());
        self.max.y = self.max.y.max(r.bottom());
    }

    pub fn rect(&self) -> Option<Rect> {
        if self.empty {
            return None;
        }
        Some(Rect::new(
            self.min.x,
            self.min.y,
            self.max.x - self.min.x,
            self.max.y - self.min.y,
        ))
    }
}

/// 从矩形中心朝目标点方向，裁剪到矩形边界（关系线端点落在盒子上，而非中心）。
pub fn clip_toward(r: Rect, to: Point) -> Point {
    let c = r.center();
    let dx = to.x - c.x;
    let dy = to.y - c.y;
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return c;
    }
    let kx = if dx != 0.0 {
        r.w / 2.0 / dx.abs()
    } else {
        f32::INFINITY
    };
    let ky = if dy != 0.0 {
        r.h / 2.0 / dy.abs()
    } else {
        f32::INFINITY
    };
    let k = kx.min(ky);
    Point::new(c.x + dx * k, c.y + dy * k)
}

/// 关系线箭头的最小屏幕长度（CSS px）。
///
/// 树连线的箭头一直按屏幕恒定画；关系线在 `Scene` 里写死成 6×4 世界单位，
/// 缩到 30% 只剩 1.8 css px，四档端点语义（无/单向/反向/双向）在缩略视图里全看不出差别。
pub const ARROW_MIN_CSS_LEN: f32 = 7.0;

/// 把 `Scene` 的世界单位箭头三角形按「尖端为锚点」放大到最小屏幕长度。
///
/// 输入是 `SceneRelationship::arrow` / `arrow_back` 里的三角（`[尖端, 底边左, 底边右]`），
/// `zoom` 是消费方当前缩放。绘制、命中、导出都必须调这一个函数，否则三者会各画各的：
/// 以前画布放大到 ≥7 css、命中与导出读原始世界三角，导出的箭头比屏幕短一截。
///
/// 语义：尖端不动（箭头仍然钉在线的端点上）；底边沿「尖端 → 底边中点」方向外推，
/// 使整条长度取 `max(世界基准长, 最小 css / zoom)`。只放大不缩小——`zoom > 1` 时
/// 保持原来的世界尺寸，箭头在高缩放下不会反而变钝。导出固定传 `1.0`（100% 口径）。
pub fn arrow_at_zoom(tri: [Point; 3], zoom: f32) -> [Point; 3] {
    let tip = tri[0];
    let mx = (tri[1].x + tri[2].x) * 0.5;
    let my = (tri[1].y + tri[2].y) * 0.5;
    let len = ((tip.x - mx) * (tip.x - mx) + (tip.y - my) * (tip.y - my)).sqrt();
    if len <= 0.01 {
        return tri;
    }
    let z = if zoom.is_finite() {
        zoom.max(1e-4)
    } else {
        1.0
    };
    let k = (ARROW_MIN_CSS_LEN / z / len).max(1.0);
    if k <= 1.0001 {
        return tri;
    }
    let grow = |p: Point| Point::new(tip.x + (p.x - tip.x) * k, tip.y + (p.y - tip.y) * k);
    [tip, grow(tri[1]), grow(tri[2])]
}

/// 路径分段。`Line`/`Quad`/`Cubic` 足够表达连线、花括号与关系线。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSeg {
    Line { to: Point },
    Quad { ctrl: Point, to: Point },
    Cubic { c1: Point, c2: Point, to: Point },
}

/// 折/曲线：起点 + 连续分段。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub start: Point,
    pub segs: Vec<PathSeg>,
}

impl Path {
    pub fn new(start: Point) -> Self {
        Self {
            start,
            segs: Vec::new(),
        }
    }

    pub fn line(&mut self, to: Point) {
        self.segs.push(PathSeg::Line { to });
    }

    pub fn quad(&mut self, ctrl: Point, to: Point) {
        self.segs.push(PathSeg::Quad { ctrl, to });
    }

    pub fn cubic(&mut self, c1: Point, c2: Point, to: Point) {
        self.segs.push(PathSeg::Cubic { c1, c2, to });
    }

    /// 终点（无分段时即起点）。
    pub fn end(&self) -> Point {
        match self.segs.last() {
            None => self.start,
            Some(PathSeg::Line { to })
            | Some(PathSeg::Quad { to, .. })
            | Some(PathSeg::Cubic { to, .. }) => *to,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_union_spans_both() {
        let a = Rect::new(10.0, 10.0, 20.0, 20.0);
        let b = Rect::new(50.0, 0.0, 10.0, 5.0);
        assert_eq!(a.union(b), Rect::new(10.0, 0.0, 50.0, 30.0));
    }

    #[test]
    fn rect_intersects_counts_touching_edges() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(Rect::new(5.0, 5.0, 10.0, 10.0)));
        assert!(a.intersects(Rect::new(10.0, 10.0, 1.0, 1.0)));
        assert!(!a.intersects(Rect::new(10.1, 0.0, 1.0, 1.0)));
    }

    #[test]
    fn enclosing_reports_empty_until_added() {
        let mut e = Enclosing::new();
        assert_eq!(e.rect(), None);
        e.add(Rect::new(4.0, 4.0, 2.0, 2.0));
        e.add(Rect::new(0.0, 9.0, 1.0, 1.0));
        assert_eq!(e.rect(), Some(Rect::new(0.0, 4.0, 6.0, 6.0)));
    }

    #[test]
    fn clip_toward_hits_the_side_nearest_target() {
        let r = Rect::new(0.0, 0.0, 100.0, 40.0);
        // 目标在右侧远处 → 交点为右边界中点
        let p = clip_toward(r, Point::new(400.0, 20.0));
        assert!((p.x - 100.0).abs() < 1e-4);
        assert!((p.y - 20.0).abs() < 1e-4);
        // 目标在正下方 → 交点在下边界
        let q = clip_toward(r, Point::new(50.0, 300.0));
        assert!((q.x - 50.0).abs() < 1e-4);
        assert!((q.y - 40.0).abs() < 1e-4);
    }

    #[test]
    fn clip_toward_at_center_returns_center() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(clip_toward(r, Point::new(5.0, 5.0)), Point::new(5.0, 5.0));
    }

    #[test]
    fn path_end_follows_last_segment() {
        let mut p = Path::new(Point::new(0.0, 0.0));
        assert_eq!(p.end(), Point::new(0.0, 0.0));
        p.cubic(
            Point::new(1.0, 2.0),
            Point::new(3.0, 4.0),
            Point::new(9.0, 8.0),
        );
        assert_eq!(p.end(), Point::new(9.0, 8.0));
    }

    /// 6×4 的基准三角（与 `scene::straight_between` 生成的一致，尖端在原点、朝 +x 指）。
    fn base_tri() -> [Point; 3] {
        [
            Point::new(0.0, 0.0),
            Point::new(-6.0, 2.0),
            Point::new(-6.0, -2.0),
        ]
    }

    fn tri_len(t: [Point; 3]) -> f32 {
        let mx = (t[1].x + t[2].x) * 0.5;
        let my = (t[1].y + t[2].y) * 0.5;
        t[0].dist(Point::new(mx, my))
    }

    #[test]
    fn arrow_keeps_the_tip_and_grows_the_base() {
        let t = base_tri();
        let out = arrow_at_zoom(t, 1.0);
        assert_eq!(out[0], t[0], "尖端必须不动");
        // 7 css / 1.0 = 7 世界单位 > 基准 6，底边整体后移。
        assert!((tri_len(out) - ARROW_MIN_CSS_LEN).abs() < 1e-4);
        assert!((out[1].y - 2.0 * ARROW_MIN_CSS_LEN / 6.0).abs() < 1e-4);
        assert!((out[2].y + 2.0 * ARROW_MIN_CSS_LEN / 6.0).abs() < 1e-4);
    }

    #[test]
    fn arrow_length_is_min_css_over_zoom_and_never_shrinks() {
        // 低缩放：屏幕长度恒定 7 css。
        for zoom in [0.25_f32, 0.5, 1.0] {
            let out = arrow_at_zoom(base_tri(), zoom);
            assert!(
                (tri_len(out) * zoom - ARROW_MIN_CSS_LEN).abs() < 1e-4,
                "zoom={zoom} 应恰好撑到 {ARROW_MIN_CSS_LEN} css"
            );
        }
        // 高缩放：世界基准 6 已经比 7/zoom 长，保持原样。
        for zoom in [1.2_f32, 2.0, 4.0] {
            assert_eq!(arrow_at_zoom(base_tri(), zoom), base_tri());
        }
    }

    #[test]
    fn arrow_degrades_gracefully() {
        // 退化三角（长度为零）原样返回，不除零。
        let flat = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        ];
        assert_eq!(arrow_at_zoom(flat, 0.5), flat);
        // 非有限 zoom 按 1.0 兜底，不把 NaN 传进几何。
        let out = arrow_at_zoom(base_tri(), f32::NAN);
        assert!(out.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert!((tri_len(out) - ARROW_MIN_CSS_LEN).abs() < 1e-4);
    }
}
