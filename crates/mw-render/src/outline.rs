//! 字形 → 矢量轮廓（PDF 导出用）。
//!
//! DirectWrite 的 `GetGlyphRunOutline` 把字形轮廓喂给一个
//! `ID2D1SimplifiedGeometrySink`。这里实现一个最小 sink，只收
//! BeginFigure / AddLines / AddBeziers / EndFigure，落成 `mw_core::Path`。
//! 坐标与画布排版一致：DIP、y 向下、原点在笔位与基线交点。
//!
//! 这样导出的 PDF 不需要嵌入字体文件：文字就是路径，打印分辨率无关，
//! 换机器也不会因缺字体而回退。代价是文字不可选中（后续可加 Type3/ToUnicode）。

use std::cell::RefCell;
use std::rc::Rc;

use mw_core::{Path, PathSeg, Point};
use windows::core::{implement, Result};
use windows::Win32::Graphics::Direct2D::Common::{
    ID2D1SimplifiedGeometrySink, ID2D1SimplifiedGeometrySink_Impl, D2D1_BEZIER_SEGMENT,
    D2D1_FIGURE_BEGIN, D2D1_FIGURE_END, D2D1_FILL_MODE, D2D1_PATH_SEGMENT,
};
use windows::Win32::Graphics::DirectWrite::IDWriteFontFace;
use windows_numerics::Vector2;

type PathStore = Rc<RefCell<Vec<Path>>>;

#[implement(ID2D1SimplifiedGeometrySink, Agile = false)]
struct PathSink {
    paths: PathStore,
    current: RefCell<Option<Path>>,
}

fn v2(p: Vector2) -> Point {
    Point::new(p.X, p.Y)
}

impl ID2D1SimplifiedGeometrySink_Impl for PathSink_Impl {
    fn SetFillMode(&self, _: D2D1_FILL_MODE) {}

    fn SetSegmentFlags(&self, _: D2D1_PATH_SEGMENT) {}

    fn BeginFigure(&self, startpoint: &Vector2, _: D2D1_FIGURE_BEGIN) {
        *self.current.borrow_mut() = Some(Path::new(v2(*startpoint)));
    }

    fn AddLines(&self, points: *const Vector2, pointscount: u32) {
        if points.is_null() {
            return;
        }
        let mut cur = self.current.borrow_mut();
        let Some(path) = cur.as_mut() else {
            return;
        };
        for i in 0..pointscount as usize {
            let p = unsafe { *points.add(i) };
            path.line(v2(p));
        }
    }

    fn AddBeziers(&self, beziers: *const D2D1_BEZIER_SEGMENT, bezierscount: u32) {
        if beziers.is_null() {
            return;
        }
        let mut cur = self.current.borrow_mut();
        let Some(path) = cur.as_mut() else {
            return;
        };
        for i in 0..bezierscount as usize {
            let b = unsafe { *beziers.add(i) };
            path.cubic(v2(b.point1), v2(b.point2), v2(b.point3));
        }
    }

    fn EndFigure(&self, _: D2D1_FIGURE_END) {
        if let Some(path) = self.current.borrow_mut().take() {
            self.paths.borrow_mut().push(path);
        }
    }

    fn Close(&self) -> Result<()> {
        Ok(())
    }
}

/// 一个字形在 `em_size`（DIP）下的全部封闭轮廓。字形不存在时返回空。
pub fn glyph_paths(face: &IDWriteFontFace, glyph: u16, em_size: f32) -> Vec<Path> {
    let store: PathStore = Rc::new(RefCell::new(Vec::new()));
    let sink: ID2D1SimplifiedGeometrySink = PathSink {
        paths: store.clone(),
        current: RefCell::new(None),
    }
    .into();
    let indices = [glyph];
    let advances = [em_size];
    let result = unsafe {
        face.GetGlyphRunOutline(
            em_size,
            indices.as_ptr(),
            Some(advances.as_ptr()),
            None,
            1,
            false,
            false,
            &sink,
        )
    };
    if result.is_err() {
        return Vec::new();
    }
    // sink 还持有 store；先放掉 COM 引用，才能把 Rc 里的路径拿回来。
    drop(sink);
    Rc::try_unwrap(store)
        .map(|c| c.into_inner())
        .unwrap_or_default()
}

/// 平移轮廓（导出时把字形原点搬到笔位/基线）。
pub fn translate(paths: &mut [Path], dx: f32, dy: f32) {
    for p in paths {
        shift(&mut p.start, dx, dy);
        for seg in &mut p.segs {
            match seg {
                PathSeg::Line { to } => shift(to, dx, dy),
                PathSeg::Quad { ctrl, to } => {
                    shift(ctrl, dx, dy);
                    shift(to, dx, dy);
                }
                PathSeg::Cubic { c1, c2, to } => {
                    shift(c1, dx, dy);
                    shift(c2, dx, dy);
                    shift(to, dx, dy);
                }
            }
        }
    }
}

fn shift(p: &mut Point, dx: f32, dy: f32) {
    p.x += dx;
    p.y += dy;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_moves_every_segment_endpoint() {
        let mut p = Path::new(Point::new(1.0, 2.0));
        p.line(Point::new(3.0, 4.0));
        p.cubic(
            Point::new(5.0, 6.0),
            Point::new(7.0, 8.0),
            Point::new(9.0, 10.0),
        );
        translate(std::slice::from_mut(&mut p), 10.0, -2.0);
        assert_eq!(p.start, Point::new(11.0, 0.0));
        assert_eq!(
            p.segs[0],
            PathSeg::Line {
                to: Point::new(13.0, 2.0)
            }
        );
        assert_eq!(
            p.segs[1],
            PathSeg::Cubic {
                c1: Point::new(15.0, 4.0),
                c2: Point::new(17.0, 6.0),
                to: Point::new(19.0, 8.0)
            }
        );
    }
}
