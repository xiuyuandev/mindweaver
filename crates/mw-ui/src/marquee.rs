//! 框选（橡皮筋）。
//!
//! docs/06 §4：「单击空白取消选择；拖拽 >4px 变框选」。命中判定用**相交**而不是
//! 包含——用户拉一个框是想「把这一片都拿住」，要求整节点进框会让人反复重拉。

use mw_core::{Rect, Scene};

/// 与 `rect` 相交的节点，按 `draw_order`（浅→深）返回；**末尾即主选**。
///
/// 主选取最深的那个是旧版留下的手感：框住一个分支时，眼睛盯着的通常是里面那层，
/// 而不是刚好被框到边的父节点。
pub fn intersecting(scene: &Scene, rect: Rect) -> Vec<u32> {
    scene
        .draw_order
        .iter()
        .copied()
        .filter(|&i| rect.intersects(scene.rect(i)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::intersecting;
    use mw_core::text::MonoMeasurer;
    use mw_core::{build_scene, get_theme, Command, Document, LayoutSettings, Rect, Scene};

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
        let a = doc.doc_order()[1].clone();
        doc.apply(&Command::AddNode {
            parent: a,
            index: None,
            text: "甲一".into(),
            style: None,
        })
        .unwrap();
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
    fn a_box_touching_part_of_a_node_selects_it() {
        let scene = fixture();
        let leaf = scene.nodes.len() as u32 - 1;
        let r = scene.rect(leaf);
        // 只盖住右下角 1 像素也要算命中。
        let marquee = Rect::new(r.right() - 1.0, r.bottom() - 1.0, 4.0, 4.0);
        assert!(intersecting(&scene, marquee).contains(&leaf));
    }

    #[test]
    fn primary_is_the_deepest_node_in_the_box() {
        let scene = fixture();
        let whole = scene.bounds;
        let picked = intersecting(&scene, whole);
        assert_eq!(picked.len(), scene.nodes.len());
        let root = scene.find(&scene.ids[0]).unwrap();
        assert_eq!(picked[0], root, "draw_order 浅的在前");
        assert_ne!(picked.last().copied(), Some(root), "主选不该是根");
    }

    #[test]
    fn an_empty_region_selects_nothing() {
        let scene = fixture();
        let far = Rect::new(
            scene.bounds.right() + 50.0,
            scene.bounds.bottom() + 50.0,
            10.0,
            10.0,
        );
        assert!(intersecting(&scene, far).is_empty());
    }
}
