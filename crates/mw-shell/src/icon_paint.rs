//! 矢量图标：几何写在 16 单位网格（中心 0,0），渲染时按图标盒归一。
//!
//! 线宽是屏幕空间常量（`design::icon::STROKE`），只缩几何不缩描边——
//! 125%–200% 下图标变大了，线条仍然一样细，不会糊成一团。
//! 几何与绘制分开：`icon_prims` 是纯函数，单测直接查边界与方向。

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};
use mw_core::design;

use crate::icons::Icon;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum IconPrim {
    Line {
        a: (f32, f32),
        b: (f32, f32),
        fine: bool,
    },
    Rect {
        c: (f32, f32),
        size: (f32, f32),
        radius: f32,
        fine: bool,
    },
    Circle {
        c: (f32, f32),
        r: f32,
        filled: bool,
    },
    Polyline {
        pts: Vec<(f32, f32)>,
        fine: bool,
    },
    Polygon {
        pts: Vec<(f32, f32)>,
    },
}

impl IconPrim {
    /// 图元覆盖到的所有点（用于边界测试；圆按外接方框算）。
    #[cfg(test)]
    pub(crate) fn points(&self) -> Vec<(f32, f32)> {
        match self {
            IconPrim::Line { a, b, .. } => vec![*a, *b],
            IconPrim::Rect { c, size, .. } => {
                let (x, y) = *c;
                let (w, h) = *size;
                vec![
                    (x - w / 2.0, y - h / 2.0),
                    (x + w / 2.0, y - h / 2.0),
                    (x - w / 2.0, y + h / 2.0),
                    (x + w / 2.0, y + h / 2.0),
                ]
            }
            IconPrim::Circle { c, r, .. } => {
                let (x, y) = *c;
                vec![
                    (x - r, y - r),
                    (x + r, y - r),
                    (x - r, y + r),
                    (x + r, y + r),
                ]
            }
            IconPrim::Polyline { pts, .. } | IconPrim::Polygon { pts } => pts.clone(),
        }
    }
}

fn line(a: (f32, f32), b: (f32, f32)) -> IconPrim {
    IconPrim::Line { a, b, fine: false }
}

fn fine_line(a: (f32, f32), b: (f32, f32)) -> IconPrim {
    IconPrim::Line { a, b, fine: true }
}

fn rect(c: (f32, f32), size: (f32, f32), radius: f32) -> IconPrim {
    IconPrim::Rect {
        c,
        size,
        radius,
        fine: false,
    }
}

fn fine_rect(c: (f32, f32), size: (f32, f32), radius: f32) -> IconPrim {
    IconPrim::Rect {
        c,
        size,
        radius,
        fine: true,
    }
}

fn circle(c: (f32, f32), r: f32) -> IconPrim {
    IconPrim::Circle {
        c,
        r,
        filled: false,
    }
}

fn disc(c: (f32, f32), r: f32) -> IconPrim {
    IconPrim::Circle { c, r, filled: true }
}

fn poly(pts: Vec<(f32, f32)>) -> IconPrim {
    IconPrim::Polyline { pts, fine: false }
}

/// 带箭头的连线：主线 + 两根箭头线（箭头线走细笔，不喧宾夺主）。
fn arrow(a: (f32, f32), b: (f32, f32)) -> Vec<IconPrim> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    let (px, py) = (-uy, ux);
    let head = 2.4;
    let wing = 1.6;
    vec![
        IconPrim::Line { a, b, fine: false },
        IconPrim::Line {
            a: b,
            b: (b.0 - ux * head + px * wing, b.1 - uy * head + py * wing),
            fine: true,
        },
        IconPrim::Line {
            a: b,
            b: (b.0 - ux * head - px * wing, b.1 - uy * head - py * wing),
            fine: true,
        },
    ]
}

/// 撤销/重做共用一条曲线：`redo` 决定箭头指向（重做向右、撤销向左）。
fn history_curve(redo: bool) -> Vec<IconPrim> {
    let dir = if redo { 1.0 } else { -1.0 };
    let start = (-4.8 * dir, 1.6);
    let end = (4.8 * dir, 1.6);
    let ctrl = (0.0, -5.6);
    let mut pts = Vec::with_capacity(13);
    for i in 0..=12 {
        let t = i as f32 / 12.0;
        let u = 1.0 - t;
        pts.push((
            u * u * start.0 + 2.0 * u * t * ctrl.0 + t * t * end.0,
            u * u * start.1 + 2.0 * u * t * ctrl.1 + t * t * end.1,
        ));
    }
    let tine = |dx: f32, dy: f32| (end.0 - dx * dir, end.1 + dy);
    vec![
        poly(pts),
        line(end, tine(3.0, -1.8)),
        line(end, tine(2.3, 2.6)),
    ]
}

/// 图标的纯几何描述：16 单位网格、中心为零点。
pub(crate) fn icon_prims(icon: Icon) -> Vec<IconPrim> {
    match icon {
        Icon::Undo => history_curve(false),
        Icon::Redo => history_curve(true),
        Icon::AddChild | Icon::Plus => {
            vec![line((-5.5, 0.0), (5.5, 0.0)), line((0.0, -5.5), (0.0, 5.5))]
        }
        Icon::AddSibling => vec![
            line((-5.0, 6.0), (-5.0, -2.5)),
            line((-5.0, -2.5), (5.0, -2.5)),
            line((5.0, -2.5), (2.0, -5.2)),
            line((5.0, -2.5), (2.0, 0.2)),
        ],
        Icon::Delete => vec![
            line((-6.2, -4.0), (6.2, -4.0)),
            line((-2.2, -6.4), (2.2, -6.4)),
            rect((0.0, 1.8), (9.2, 9.6), 1.5),
            fine_line((-1.5, -0.6), (-1.5, 4.4)),
            fine_line((1.5, -0.6), (1.5, 4.4)),
        ],
        Icon::Fit => {
            let mut out = Vec::with_capacity(8);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = (5.5 * sx, 5.5 * sy);
                out.push(line(corner, (corner.0 - 4.2 * sx, corner.1)));
                out.push(line(corner, (corner.0, corner.1 - 4.2 * sy)));
            }
            out
        }
        // ── 概念图工具条（`docs/33` §5）──
        // 六枚都画在同一个 16 单位网格上，中心为零点；描边粗细走 `line`/`fine_line`，
        // 不在这儿定颜色（`paint_icon_sized` 统一给）。
        Icon::Concept => vec![
            rect((-2.8, -2.6), (8.8, 7.2), 1.3),
            line((3.2, 4.6), (7.4, 4.6)),
            line((5.3, 2.7), (5.3, 6.5)),
        ],
        Icon::Link => {
            let mut out = vec![disc((-5.4, -3.8), 1.8), disc((5.4, 3.8), 1.8)];
            out.extend(arrow((-3.4, -2.0), (3.4, 2.0)));
            out
        }
        Icon::Phrase => vec![
            rect((-1.8, -1.0), (11.6, 6.4), 3.2),
            line((-5.2, -1.0), (0.4, -1.0)),
            line((-5.2, 1.8), (-1.0, 1.8)),
            line((4.6, -5.6), (4.6, 4.4)),
        ],
        Icon::Arrange => vec![
            rect((-5.4, -1.6), (4.6, 5.6), 0.9),
            rect((0.0, -1.6), (4.6, 5.6), 0.9),
            rect((5.4, -1.6), (4.6, 5.6), 0.9),
        ],
        Icon::Pin => vec![
            circle((0.0, -2.8), 3.2),
            line((0.0, 0.4), (0.0, 6.2)),
            line((-3.4, 6.2), (3.4, 6.2)),
        ],
        Icon::FocusQ => vec![
            rect((-0.6, -1.2), (11.6, 8.4), 2.4),
            poly(vec![(-3.6, 3.4), (-3.6, 7.2), (0.4, 3.4)]),
            line((-1.8, -3.4), (-1.8, -1.4)),
            disc((-1.8, 1.2), 1.0),
        ],
        Icon::New => vec![
            rect((-0.8, 0.0), (9.6, 12.0), 1.5),
            line((3.2, 4.6), (7.2, 4.6)),
            line((5.2, 2.6), (5.2, 6.6)),
        ],
        Icon::Open => vec![
            rect((0.0, 1.5), (13.0, 8.0), 1.4),
            poly(vec![(-6.5, -2.5), (-6.5, -5.5), (-2.2, -5.5), (-0.6, -2.5)]),
        ],
        Icon::Save | Icon::SaveAs => {
            let mut out = vec![
                rect((0.0, 0.0), (11.5, 12.0), 1.8),
                fine_rect((0.0, -4.2), (6.8, 3.6), 0.8),
                fine_rect((0.0, 3.8), (7.6, 5.2), 0.8),
            ];
            if icon == Icon::SaveAs {
                out.push(line((3.7, 5.8), (7.7, 5.8)));
                out.push(line((5.7, 3.8), (5.7, 7.8)));
            }
            out
        }
        Icon::File => vec![
            rect((0.0, 0.0), (9.0, 11.5), 1.4),
            poly(vec![(0.8, -5.75), (4.5, -2.4)]),
        ],
        Icon::Export => vec![
            rect((0.0, 3.75), (13.0, 5.5), 1.4),
            line((0.0, 3.2), (0.0, -6.4)),
            line((-3.4, -2.8), (0.0, -6.4)),
            line((3.4, -2.8), (0.0, -6.4)),
        ],
        Icon::Panel => vec![
            rect((0.0, 0.0), (12.5, 10.5), 1.8),
            line((-2.6, -5.25), (-2.6, 5.25)),
        ],
        Icon::Theme => vec![circle((0.0, 0.0), 6.2), disc((0.0, 2.3), 3.0)],
        Icon::Home => vec![
            line((-7.0, -0.5), (0.0, -6.5)),
            line((0.0, -6.5), (7.0, -0.5)),
            line((-5.0, -1.0), (-5.0, 6.5)),
            line((5.0, -1.0), (5.0, 6.5)),
            line((-5.0, 6.5), (5.0, 6.5)),
            line((-1.2, 6.5), (-1.2, 2.2)),
            line((1.2, 2.2), (1.2, 6.5)),
            line((-1.2, 2.2), (1.2, 2.2)),
        ],
        Icon::Search => vec![circle((-1.2, -1.2), 4.2), line((1.9, 1.9), (5.4, 5.4))],
        Icon::Note => vec![
            rect((0.0, 0.0), (10.5, 12.5), 2.0),
            fine_line((-3.5, -2.6), (3.5, -2.6)),
            fine_line((-3.5, 1.0), (1.8, 1.0)),
        ],
        Icon::Library => vec![
            fine_rect((1.6, -3.8), (10.0, 7.6), 1.4),
            fine_rect((0.6, -2.2), (10.0, 7.6), 1.4),
            rect((-0.4, -0.6), (10.0, 7.6), 1.4),
        ],
        Icon::Tag => vec![
            IconPrim::Polygon {
                pts: vec![(-6.0, -2.0), (-2.0, -6.0), (6.0, -2.0), (2.0, 6.0)],
            },
            disc((-2.6, -2.6), 1.1),
        ],
        Icon::Exit => vec![
            line((-5.5, -6.0), (-5.5, 6.0)),
            line((-5.5, -6.0), (-2.0, -6.0)),
            line((-5.5, 6.0), (-2.0, 6.0)),
            line((-1.5, 0.0), (5.5, 0.0)),
            line((2.2, -3.2), (5.5, 0.0)),
            line((2.2, 3.2), (5.5, 0.0)),
        ],
        Icon::ChevronLeft => vec![poly(vec![(2.8, -5.2), (-2.6, 0.0), (2.8, 5.2)])],
        Icon::ChevronRight => vec![poly(vec![(-2.8, -5.2), (2.6, 0.0), (-2.8, 5.2)])],
        Icon::ChevronDown => vec![poly(vec![(-5.0, -2.6), (0.0, 2.8), (5.0, -2.6)])],
        Icon::ChevronUp => vec![poly(vec![(-5.0, 2.8), (0.0, -2.6), (5.0, 2.8)])],
        Icon::Close => vec![
            line((-4.5, -4.5), (4.5, 4.5)),
            line((4.5, -4.5), (-4.5, 4.5)),
        ],
        Icon::More => vec![
            disc((-5.0, 0.0), design::icon::DOT),
            disc((0.0, 0.0), design::icon::DOT),
            disc((5.0, 0.0), design::icon::DOT),
        ],
        Icon::Back => vec![
            line((4.6, -5.2), (-3.8, 0.0)),
            line((-3.8, 0.0), (4.6, 5.2)),
            line((-3.4, 0.0), (6.2, 0.0)),
        ],
        Icon::Levels => vec![
            line((-6.0, -5.0), (6.0, -5.0)),
            line((-6.0, 0.0), (2.4, 0.0)),
            line((-6.0, 5.0), (-1.2, 5.0)),
        ],
        Icon::Density => vec![
            line((-6.0, -5.4), (6.0, -5.4)),
            line((-6.0, -1.8), (6.0, -1.8)),
            line((-6.0, 1.8), (6.0, 1.8)),
            line((-6.0, 5.4), (6.0, 5.4)),
        ],
        Icon::Focus => {
            let mut out = vec![circle((0.0, 0.0), 5.3), disc((0.0, 0.0), 1.5)];
            for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                out.push(line((dx * 4.2, dy * 4.2), (dx * 7.2, dy * 7.2)));
            }
            out
        }
        Icon::Mindmap => {
            let mut out = vec![disc((0.0, 0.0), 1.9)];
            for (dx, dy) in [(-4.8, -4.4), (4.8, -4.4), (-4.8, 4.4), (4.8, 4.4)] {
                out.push(line((0.0, 0.0), (dx * 0.62, dy * 0.62)));
                out.push(disc((dx, dy), 1.15));
            }
            out
        }
        Icon::Logic => {
            let mut out = vec![rect((-4.4, 0.0), (4.6, 3.2), 0.7)];
            for y in [-4.4, 0.0, 4.4] {
                out.push(line((-2.1, 0.0), (2.1, y)));
                out.push(rect((4.4, y), (4.4, 2.8), 0.7));
            }
            out
        }
        Icon::Org => {
            let mut out = vec![rect((0.0, -4.9), (5.2, 3.2), 0.7)];
            out.push(line((0.0, -3.3), (0.0, 0.0)));
            out.push(line((-4.4, 0.0), (4.4, 0.0)));
            for x in [-4.4, 0.0, 4.4] {
                out.push(line((x, 0.0), (x, 1.4)));
                out.push(rect((x, 3.0), (3.8, 2.8), 0.7));
            }
            out
        }
        Icon::Fishbone => {
            let mut out = vec![line((-6.6, 0.0), (4.6, 0.0))];
            out.push(fine_line((2.6, -1.9), (4.6, 0.0)));
            out.push(fine_line((2.6, 1.9), (4.6, 0.0)));
            for (i, x) in [-4.0f32, -0.4, 3.2].into_iter().enumerate() {
                let tip = (x - 1.2, if i % 2 == 0 { 4.4 } else { -4.4 });
                out.push(line((x, 0.0), tip));
                out.push(rect(tip, (3.4, 2.4), 0.6));
            }
            out
        }
        Icon::Timeline => {
            let mut out = vec![line((-6.8, 0.0), (6.8, 0.0))];
            for (i, x) in [-4.8f32, -1.6, 1.6, 4.8].into_iter().enumerate() {
                let top = if i % 2 == 0 { -3.9 } else { 3.9 };
                out.push(disc((x, 0.0), 1.1));
                out.push(line((x, 0.0), (x, top * 0.62)));
                out.push(rect((x, top), (3.0, 2.4), 0.6));
            }
            out
        }
        Icon::ConceptMap => {
            let mut out = vec![
                rect((-4.6, -3.6), (5.0, 2.8), 0.9),
                rect((4.4, -0.8), (5.0, 2.8), 0.9),
                rect((0.2, 4.4), (5.0, 2.8), 0.9),
            ];
            out.extend(arrow((-2.1, -3.6), (1.9, -0.8)));
            out.extend(arrow((3.0, 0.6), (0.9, 3.0)));
            out
        }
    }
}

/// 画一个图标：`size` 只缩放几何，描边保持屏幕空间恒定。
pub(crate) fn paint_icon_sized(
    painter: &egui::Painter,
    c: Pos2,
    size: f32,
    icon: Icon,
    color: Color32,
) {
    let k = (size / design::icon::GRID).max(0.01);
    let p = |(x, y): (f32, f32)| Pos2::new(c.x + x * k, c.y + y * k);
    let v = |(w, h): (f32, f32)| Vec2::new(w * k, h * k);
    let stroke = |fine: bool| {
        Stroke::new(
            if fine {
                design::icon::STROKE_FINE
            } else {
                design::icon::STROKE
            },
            color,
        )
    };
    for prim in icon_prims(icon) {
        match prim {
            IconPrim::Line { a, b, fine } => {
                painter.line_segment([p(a), p(b)], stroke(fine));
            }
            IconPrim::Rect {
                c: center,
                size,
                radius,
                fine,
            } => {
                painter.rect_stroke(
                    Rect::from_center_size(p(center), v(size)),
                    radius * k,
                    stroke(fine),
                    StrokeKind::Inside,
                );
            }
            IconPrim::Circle {
                c: center,
                r,
                filled,
            } => {
                if filled {
                    painter.circle_filled(p(center), r * k, color);
                } else {
                    painter.circle_stroke(p(center), r * k, stroke(false));
                }
            }
            IconPrim::Polyline { pts, fine } => {
                painter.add(egui::Shape::line(
                    pts.into_iter().map(&p).collect(),
                    stroke(fine),
                ));
            }
            IconPrim::Polygon { pts } => {
                painter.add(egui::Shape::convex_polygon(
                    pts.into_iter().map(&p).collect(),
                    Color32::TRANSPARENT,
                    stroke(false),
                ));
            }
        }
    }
}

/// 默认 16px 图标盒（菜单、工具条）。
pub(crate) fn paint_icon(painter: &egui::Painter, c: Pos2, icon: Icon, color: Color32) {
    paint_icon_sized(painter, c, design::icon::SIZE, icon, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_stays_inside_the_sixteen_unit_grid() {
        for icon in Icon::ALL {
            let prims = icon_prims(icon);
            assert!(!prims.is_empty(), "{icon:?} 没有任何图元");
            for prim in prims {
                for (x, y) in prim.points() {
                    assert!(
                        x.abs() <= 8.01 && y.abs() <= 8.01,
                        "{icon:?} 越出 16 单位网格：({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn undo_points_left_and_redo_points_right() {
        for (icon, leftward) in [(Icon::Undo, true), (Icon::Redo, false)] {
            let curve = icon_prims(icon)
                .into_iter()
                .find_map(|p| match p {
                    IconPrim::Polyline { pts, .. } => Some(pts),
                    _ => None,
                })
                .expect("历史图标必须有曲线");
            let dx = curve.last().unwrap().0 - curve.first().unwrap().0;
            assert!(
                if leftward { dx < 0.0 } else { dx > 0.0 },
                "{icon:?} 的箭头方向反了（dx={dx}）"
            );
        }
    }

    #[test]
    fn chevrons_are_mirror_images() {
        let left = icon_prims(Icon::ChevronLeft);
        let right = icon_prims(Icon::ChevronRight);
        assert_eq!(left.len(), right.len());
        let (IconPrim::Polyline { pts: lp, .. }, IconPrim::Polyline { pts: rp, .. }) =
            (&left[0], &right[0])
        else {
            panic!("chevron 必须是折线");
        };
        for (l, r) in lp.iter().zip(rp.iter()) {
            assert!((l.0 + r.0).abs() < 0.001, "左右箭头没有镜像");
            assert_eq!(l.1, r.1);
        }
    }

    /// 图标控件不许再退回归字形：字符在不同字体/DPI 下基线与粗细都会漂。
    #[test]
    fn shell_sources_do_not_build_glyph_icon_buttons() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("读源码目录") {
                let path = entry.expect("目录项").path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&root, &mut files);
        let forbidden = ["Button::new(\"✕\")", "Button::new(\"↺\")", "\"▾\""];
        for path in files {
            let text = std::fs::read_to_string(&path).expect("读源码");
            for needle in forbidden {
                assert!(
                    !text.contains(needle),
                    "{} 仍在用字形图标 {needle}，改用 Icon + paint_icon_sized",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn nav_and_compact_icons_are_defined() {
        for icon in [
            Icon::ChevronLeft,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::ChevronUp,
            Icon::Close,
            Icon::More,
            Icon::Plus,
        ] {
            assert!(!icon_prims(icon).is_empty(), "{icon:?} 缺少几何");
        }
    }
}
