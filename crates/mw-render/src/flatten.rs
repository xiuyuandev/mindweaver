//! 曲线细分：`Path` 的几何段 → 折线端点序列。
//!
//! 连线、花括号、关系线在 core 里都是贝塞尔段（`PathSeg`），这里按「足够直就停」
//! 递归细分成线段，交给实例化着色器画胶囊段。容差用**物理像素**换算成世界单位，
//! 所以放大时曲线更顺滑、缩小时段数自动变少（docs/08 P5 帧预算的第一道闸门）。

use mw_core::{Path, PathSeg, Point};

/// 目标平直度（物理像素）：0.3px 的弦误差在 200% 屏上肉眼不可见。
const FLATNESS_PHYS: f32 = 0.3;
/// 递归上限：极端控制点也不会退化成上千段。
const MAX_DEPTH: u32 = 12;

/// 世界单位容差。`zoom_ppp` = 缩放 × 系统缩放。
pub fn tolerance(zoom_ppp: f32) -> f32 {
    FLATNESS_PHYS / zoom_ppp.max(0.005)
}

/// 展开成连续点列（含起点）。空路径返回只含起点。
pub fn flatten(path: &Path, tol: f32) -> Vec<Point> {
    let mut out = vec![path.start];
    let mut cursor = path.start;
    for seg in &path.segs {
        match *seg {
            PathSeg::Line { to } => {
                out.push(to);
                cursor = to;
            }
            PathSeg::Quad { ctrl, to } => {
                // 升阶为三次曲线，共用一套细分逻辑
                let c1 = lerp3(cursor, ctrl, 2.0 / 3.0);
                let c2 = lerp3(to, ctrl, 2.0 / 3.0);
                cubic(cursor, c1, c2, to, tol, &mut out, 0);
                cursor = to;
            }
            PathSeg::Cubic { c1, c2, to } => {
                cubic(cursor, c1, c2, to, tol, &mut out, 0);
                cursor = to;
            }
        }
    }
    out
}

fn cubic(p0: Point, c1: Point, c2: Point, p3: Point, tol: f32, out: &mut Vec<Point>, depth: u32) {
    if depth >= MAX_DEPTH || is_flat(p0, c1, c2, p3, tol) {
        out.push(p3);
        return;
    }
    let a = lerp3(p0, c1, 0.5);
    let b = lerp3(c1, c2, 0.5);
    let c = lerp3(c2, p3, 0.5);
    let d = lerp3(a, b, 0.5);
    let e = lerp3(b, c, 0.5);
    let f = lerp3(d, e, 0.5);
    cubic(p0, a, d, f, tol, out, depth + 1);
    cubic(f, e, c, p3, tol, out, depth + 1);
}

/// 两个控制点到弦的距离都小于容差即视为直线。
fn is_flat(p0: Point, c1: Point, c2: Point, p3: Point, tol: f32) -> bool {
    let dx = p3.x - p0.x;
    let dy = p3.y - p0.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0e-4 {
        return p0.dist(c1) <= tol && p0.dist(c2) <= tol;
    }
    let dev = |c: Point| (dy * (c.x - p0.x) - dx * (c.y - p0.y)).abs() / len;
    dev(c1) <= tol && dev(c2) <= tol
}

#[inline]
fn lerp3(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts() -> Vec<Point> {
        vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)]
    }

    #[test]
    fn line_yields_two_points() {
        let mut p = Path::new(Point::new(0.0, 0.0));
        p.line(Point::new(100.0, 0.0));
        assert_eq!(flatten(&p, 0.3), pts());
    }

    #[test]
    fn straight_curve_collapses_to_endpoints() {
        let mut p = Path::new(Point::new(0.0, 0.0));
        p.cubic(
            Point::new(30.0, 0.0),
            Point::new(60.0, 0.0),
            Point::new(90.0, 0.0),
        );
        assert_eq!(flatten(&p, 0.3).len(), 2);
    }

    #[test]
    fn curved_path_subdivides_and_keeps_endpoints() {
        let mut p = Path::new(Point::new(0.0, 0.0));
        p.cubic(
            Point::new(0.0, 100.0),
            Point::new(100.0, 100.0),
            Point::new(100.0, 0.0),
        );
        let out = flatten(&p, 0.3);
        assert!(out.len() > 8, "四分之一圆弧应被细分：{}", out.len());
        assert_eq!(out[0], Point::new(0.0, 0.0));
        assert_eq!(*out.last().unwrap(), Point::new(100.0, 0.0));
        // 顶点到弦（这里是端点连线）的最大偏离应随细分收敛到容差内
        let mid = &out[out.len() / 2];
        assert!((mid.x - 50.0).abs() < 5.0 && (mid.y - 75.0).abs() < 5.0);
    }

    #[test]
    fn tighter_tolerance_gives_more_segments() {
        let mut p = Path::new(Point::new(0.0, 0.0));
        p.quad(Point::new(50.0, 80.0), Point::new(100.0, 0.0));
        assert!(flatten(&p, 0.02).len() > flatten(&p, 2.0).len());
    }

    #[test]
    fn tolerance_tracks_device_scale() {
        // 同样的世界尺寸，高 DPI 上容差更严（段数更多）
        assert!(tolerance(3.0) < tolerance(1.0));
        assert_eq!(tolerance(0.0), tolerance(0.005));
    }

    #[test]
    fn empty_path_returns_start() {
        let p = Path::new(Point::new(7.0, 9.0));
        assert_eq!(flatten(&p, 0.3), vec![Point::new(7.0, 9.0)]);
    }
}
