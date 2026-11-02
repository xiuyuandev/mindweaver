//! 拖出连线的判定（M2b-4）：锚点命中、落点节点。
//!
//! 和 `drag`/`marquee` 同一个定位：这里只回答「按到了哪个锚点」「松手连到谁」，
//! 画线、发命令都在壳层。这样这条手势也能在无显示器的地方断言。

use mw_core::{Point, Rect, Scene};

/// 节点四边中点，也就是「拖出关系线」的起点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];
}

/// 锚点的世界坐标（四边中点）。
pub fn anchor_point(r: Rect, side: Side) -> Point {
    match side {
        Side::Left => Point::new(r.x, r.y + r.h * 0.5),
        Side::Right => Point::new(r.right(), r.y + r.h * 0.5),
        Side::Top => Point::new(r.x + r.w * 0.5, r.y),
        Side::Bottom => Point::new(r.x + r.w * 0.5, r.bottom()),
    }
}

/// 指针落在某个锚点的命中半径内时返回它（多个都命中时取最近的那个）。
///
/// `radius` 由调用方按屏幕像素换算成世界单位——锚点是不随缩放变大的铬件，
/// 命中范围也必须跟着缩放走，否则缩小后点不中、放大后到处都能点中。
pub fn anchor_at(r: Rect, at: Point, radius: f32) -> Option<Side> {
    let mut best: Option<(f32, Side)> = None;
    for side in Side::ALL {
        let p = anchor_point(r, side);
        let d = p.dist(at);
        if d <= radius && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, side));
        }
    }
    best.map(|(_, s)| s)
}

/// 整张场景里**最近的**锚点（`docs/33` §2 B1b）。
///
/// 锚点长在盒边上，它的命中圆有一半落在盒外。壳层以前要求「先按在盒里」
/// 才去找锚点，那一半就白白丢了——用户明明按在锚点上，却被判成「拖画布」，
/// 于是「概念之间连不上线」。这条把顺序倒过来：先找最近的锚点，找不到
/// 再交给盒命中（[`target_node`] 那一支）。
///
/// 与 [`anchor_at`] 的区别：那条只在**给定的盒**里找，这条扫全图。
/// 距离相同取 `draw_order` 靠前的那颗，保证确定性（金样不随遍历顺序抖）。
pub fn nearest_anchor(scene: &Scene, at: Point, radius: f32) -> Option<(u32, Side)> {
    let mut best: Option<(f32, u32, Side)> = None;
    for &idx in scene.draw_order.iter() {
        let r = scene.rect(idx);
        if let Some(side) = anchor_at(r, at, radius) {
            let d = anchor_point(r, side).dist(at);
            if best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, idx, side));
            }
        }
    }
    best.map(|(_, idx, side)| (idx, side))
}

/// 松手处的落点节点：**包含该点**的面积最小者，且不能是连线起点自己。
///
/// 取最小面积是因为父盒套着子盒——指针落在子节点上时，用户想连的是子节点。
pub fn target_node(scene: &Scene, from: u32, at: Point) -> Option<u32> {
    target_node_snapped(scene, from, at, 0.0)
}

/// 和 [`target_node`] 一样，但**没压中**时允许吸附到 `snap` 距离以内的节点（世界单位）。
///
/// 为什么需要：关系线改端点时，指针差几个像素没落进目标概念的框里，旧判定就当「落空」，
/// 于是松手会在空白处**新建一个概念**——用户的意思是「改指到那颗」。（打磨轮 12 实测）
///
/// 优先级：压中 > 近邻。压中时仍取面积最小的（父子盒嵌套）；
/// 近邻按「到矩形的欧氏距离」取最近，距离相同再取面积小的，再相同取 `draw_order` 靠前的——
/// 全程确定性，金样才不会随遍历顺序抖。
pub fn target_node_snapped(scene: &Scene, from: u32, at: Point, snap: f32) -> Option<u32> {
    let mut inside: Option<(u32, f32)> = None;
    let mut near: Option<(u32, f32, f32)> = None; // (下标, 距离, 面积)
    for &idx in scene.draw_order.iter() {
        if idx == from {
            continue;
        }
        let r = scene.rect(idx);
        let area = r.w.max(1.0) * r.h.max(1.0);
        if r.contains(at) {
            if inside.is_none_or(|(_, a)| area < a) {
                inside = Some((idx, area));
            }
            continue;
        }
        if snap > 0.0 {
            // 点到矩形的距离：两个方向的「出界量」取欧氏长度，框内任一方为 0。
            let dx = (r.x - at.x).max(at.x - r.right()).max(0.0);
            let dy = (r.y - at.y).max(at.y - r.bottom()).max(0.0);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= snap {
                let better = match near {
                    None => true,
                    Some((_, bd, ba)) => {
                        d < bd - 1e-4 || ((d - bd).abs() <= 1e-4 && area < ba - 1e-4)
                    }
                };
                if better {
                    near = Some((idx, d, area));
                }
            }
        }
    }
    inside.map(|(i, _)| i).or_else(|| near.map(|(i, _, _)| i))
}

#[cfg(test)]
mod tests {
    use super::{anchor_at, anchor_point, nearest_anchor, target_node, target_node_snapped, Side};
    use mw_core::text::MonoMeasurer;
    use mw_core::{build_scene, get_theme, Command, Document, LayoutSettings, Point, Rect, Scene};

    fn fixture() -> Scene {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        for t in ["甲", "乙"] {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: t.into(),
                style: None,
            })
            .unwrap();
        }
        build_scene(
            &doc,
            &doc.root_id,
            &LayoutSettings::default(),
            &get_theme("snow"),
            &MonoMeasurer,
            None::<(&str, &str)>,
        )
    }

    #[test]
    fn anchors_sit_on_the_four_edge_midpoints() {
        let r = Rect::new(10.0, 20.0, 100.0, 40.0);
        assert_eq!(anchor_point(r, Side::Left), Point::new(10.0, 40.0));
        assert_eq!(anchor_point(r, Side::Right), Point::new(110.0, 40.0));
        assert_eq!(anchor_point(r, Side::Top), Point::new(60.0, 20.0));
        assert_eq!(anchor_point(r, Side::Bottom), Point::new(60.0, 60.0));
    }

    #[test]
    fn only_the_nearest_anchor_within_the_radius_wins() {
        let r = Rect::new(0.0, 0.0, 100.0, 40.0);
        // 右缘中点 (100,20) 旁边 3 单位 → 命中 Right
        assert_eq!(
            anchor_at(r, Point::new(102.0, 21.0), 6.0),
            Some(Side::Right)
        );
        // 同一个点，命中半径收到 1 就不该再中
        assert_eq!(anchor_at(r, Point::new(102.0, 21.0), 1.0), None);
        // 盒子正中间离哪个锚点都远
        assert_eq!(anchor_at(r, Point::new(50.0, 20.0), 6.0), None);
    }

    #[test]
    fn the_smallest_node_under_the_pointer_wins() {
        let scene = fixture();
        let root = scene.find(&scene.ids[0]).unwrap();
        let child = 1;
        let at = scene.rect(child).center();
        assert_eq!(target_node(&scene, root, at), Some(child));
    }

    #[test]
    fn a_relationship_never_starts_and_ends_on_the_same_node() {
        let scene = fixture();
        let child = 1;
        let at = scene.rect(child).center();
        assert_eq!(target_node(&scene, child, at), None);
    }

    /// 差几像素没落进框里：旧判定当落空（松手会新建概念），吸附后应读成「改到这颗」。
    #[test]
    fn a_near_miss_snaps_inside_the_margin_only() {
        let scene = fixture();
        let child = 1;
        let r = scene.rect(child);
        // 右边缘外 6 个单位
        let at = Point::new(r.right() + 6.0, r.center().y);
        assert_eq!(target_node(&scene, 0, at), None, "不吸附时必须算落空");
        assert_eq!(
            target_node_snapped(&scene, 0, at, 12.0),
            Some(child),
            "12 以内的近失要吸附到这颗"
        );
        assert!(
            target_node_snapped(&scene, 0, at, 5.0).is_none(),
            "比距离还小的余量不许吸"
        );
    }

    /// 压中就按压中算：即便旁边有个更近的「框外」节点，也不该被它抢走。
    #[test]
    fn an_inside_hit_beats_a_nearer_outside_one() {
        let scene = fixture();
        let child = 1;
        let at = scene.rect(child).center();
        assert_eq!(
            target_node_snapped(&scene, 0, at, 60.0),
            target_node(&scene, 0, at),
            "吸附不能改变「压中」的结论"
        );
    }

    /// 没压中时按距离取最近的；起点自己永远不会被吸成目标。
    #[test]
    fn the_nearest_outside_node_wins() {
        let scene = fixture();
        let a = 1;
        let ra = scene.rect(a);
        // 正下方 5 个单位：只有 a 在这条法线上
        let at = Point::new(ra.center().x, ra.bottom() + 5.0);
        assert_eq!(target_node_snapped(&scene, 0, at, 12.0), Some(a));
        // 起点自己不被吸成目标
        assert_ne!(target_node_snapped(&scene, a, at, 12.0), Some(a));
    }

    #[test]
    fn dropping_on_empty_space_targets_nothing() {
        let scene = fixture();
        let far = Point::new(scene.bounds.right() + 100.0, scene.bounds.bottom() + 100.0);
        assert_eq!(target_node(&scene, 0, far), None);
    }

    /// `docs/33` §2 B1b：锚点命中圆有一半落在盒外——按在那一半上也必须算。
    ///
    /// 壳层以前要求「先按在盒里」才去找锚点，于是用户明明按在锚点上却被判成
    /// 「拖画布」，体感就是「概念之间连不上线」。
    #[test]
    fn nearest_anchor_finds_the_one_hanging_outside_the_box() {
        let scene = fixture();
        let idx = 1;
        let r = scene.rect(idx);
        let p = anchor_point(r, Side::Right);
        // 探针落在盒外（右锚点再往右 5），但仍在命中半径内。
        let probe = Point::new(p.x + 5.0, p.y);
        assert!(!r.contains(probe), "探针必须在盒外，否则这条测试没意义");
        assert_eq!(
            nearest_anchor(&scene, probe, 12.0),
            Some((idx, Side::Right)),
            "按在盒外的锚点上也该拉出线"
        );
    }

    #[test]
    fn nearest_anchor_picks_the_closest_and_gives_up_past_the_radius() {
        let scene = fixture();
        let idx = 1;
        let r = scene.rect(idx);
        let p = anchor_point(r, Side::Right);
        assert_eq!(
            nearest_anchor(&scene, Point::new(p.x + 2.0, p.y), 12.0),
            Some((idx, Side::Right))
        );
        // 半径外：什么都不给（交给盒命中那支）。
        assert_eq!(
            nearest_anchor(&scene, Point::new(p.x + 40.0, p.y), 12.0),
            None
        );
    }
}
