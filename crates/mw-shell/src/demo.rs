//! 演示模式（docs/02 §9）：先序步进、350ms 相机缓动、当前路径高亮。
//!
//! 演示不另起一条绘制管线：进入时复制一份场景，只改各元素的 `alpha`（路径 = 1，
//! 其余压暗），再把主题的画布底色换成深色。渲染、文字排版、LOD 与平时完全同源。
//!
//! 相机动画由本模块自己算，不复用布局过渡的 `Motion`：后者只在**场景结构**变化时
//! 启动，而演示每一步都要动相机。缩放按对数插值——人眼感知的是倍率变化，
//! 线性插值在 0.5→1.25 的过程中会先慢后快。

use std::collections::HashSet;
use std::time::{Duration, Instant};

use mw_core::theme::{Rgba, Theme};
use mw_core::{Point, Scene};
use mw_render::Camera;

/// 一步的相机时长（docs/06 §8）。
const STEP: Duration = Duration::from_millis(350);
/// 非路径节点的透明度：留一点轮廓当上下文，又不至于抢焦点。
const DIM_NODE: f32 = 0.16;
/// 连线/边界/概要比节点更暗——它们是背景结构。
const DIM_DECOR: f32 = 0.10;
/// 演示底色：与主题无关的深灰蓝，投影仪上也比纯黑少一点脏边。
const DEMO_BG: Rgba = Rgba::rgb(0x12, 0x14, 0x18);
/// 节点占视口的大致比例；越小留的上下文越多。
const FILL_RATIO: f32 = 0.45;
const ZOOM_MIN: f32 = 0.55;
const ZOOM_MAX: f32 = 1.25;

/// 一次演示会话。退出时把 `saved` 相机还给画布（`Esc` 返回原视口）。
pub struct Demo {
    order: Vec<u32>,
    index: usize,
    from: Camera,
    to: Camera,
    started: Instant,
    duration: Duration,
    saved: Camera,
    /// 演示用场景副本：几何/文字与 `Canvas::scene` 一致，只有 alpha 不同。
    scene: Scene,
    theme: Theme,
    bar_hidden: bool,
}

impl Demo {
    /// 从 `start` 指定的节点开始；它不在可见前序里（例如刚被折叠藏起来）时退回根。
    pub fn start(
        scene: &Scene,
        theme: &Theme,
        camera: Camera,
        start: Option<u32>,
        viewport: [f32; 2],
        animate: bool,
    ) -> Option<Self> {
        let order = preorder(scene);
        if order.is_empty() {
            return None;
        }
        let index = start
            .and_then(|i| order.iter().position(|&n| n == i))
            .unwrap_or(0);
        let current = order[index];
        let mut shown = scene.clone();
        paint_path(&mut shown, &path_of(scene, current));
        let mut theme = theme.clone();
        theme.ui.canvas_bg = DEMO_BG;
        theme.ui.bg = DEMO_BG;
        theme.ui.dot = Rgba::TRANSPARENT;
        Some(Self {
            order,
            index,
            from: camera,
            to: target(scene, current, viewport, camera.ppp),
            started: Instant::now(),
            duration: if animate { STEP } else { Duration::ZERO },
            saved: camera,
            scene: shown,
            theme,
            bar_hidden: false,
        })
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn current(&self) -> u32 {
        self.order[self.index]
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn saved_camera(&self) -> Camera {
        self.saved
    }

    pub fn bar_hidden(&self) -> bool {
        self.bar_hidden
    }

    pub fn toggle_bar(&mut self) {
        self.bar_hidden = !self.bar_hidden;
    }

    /// 到达下一个节点才返回 `true`；已经在末尾就原地不动。
    pub fn next(&mut self, viewport: [f32; 2], animate: bool) -> bool {
        if self.index + 1 >= self.order.len() {
            return false;
        }
        self.go(self.index + 1, viewport, animate);
        true
    }

    /// 回到上一个节点；已经在开头就返回 `false`。
    pub fn prev(&mut self, viewport: [f32; 2], animate: bool) -> bool {
        if self.index == 0 {
            return false;
        }
        self.go(self.index - 1, viewport, animate);
        true
    }

    fn go(&mut self, index: usize, viewport: [f32; 2], animate: bool) {
        let now = Instant::now();
        // 连按方向键时从**此刻的插值位置**接着走，否则相机会先跳回上一段的起点。
        self.from = self.camera(now);
        self.index = index;
        self.to = target(&self.scene, self.order[index], viewport, self.from.ppp);
        self.started = now;
        self.duration = if animate { STEP } else { Duration::ZERO };
        let path = path_of(&self.scene, self.order[index]);
        paint_path(&mut self.scene, &path);
    }

    pub fn camera(&self, now: Instant) -> Camera {
        let t = if self.duration.is_zero() {
            1.0
        } else {
            (now.saturating_duration_since(self.started).as_secs_f32()
                / self.duration.as_secs_f32())
            .clamp(0.0, 1.0)
        };
        let e = ease_out_cubic(t);
        let z = self.from.zoom * (self.to.zoom / self.from.zoom).powf(e);
        Camera::new(
            Point::new(
                self.from.center.x + (self.to.center.x - self.from.center.x) * e,
                self.from.center.y + (self.to.center.y - self.from.center.y) * e,
            ),
            z,
            self.to.ppp,
        )
    }

    pub fn settled(&self, now: Instant) -> bool {
        self.duration.is_zero() || now.saturating_duration_since(self.started) >= self.duration
    }
}

/// 可见树的**先序**：尊重折叠（看不到的分支不演示），自由主题排在主树之后。
///
/// 场景节点本身就按文档前序编号：`compute_layout` 用逆序入栈做前序遍历，折叠的子树
/// 直接不下探；自由主题随后各自按前序追加。所以索引升序就是演示顺序——不要改按
/// `draw_order`（它是绘制用的「深度 + y/x」，会把左右分叉的兄弟打乱，比如根的一级
/// 分支甲在左、乙在右时会先走到乙）。
pub fn preorder(scene: &Scene) -> Vec<u32> {
    (0..scene.nodes.len() as u32).collect()
}

/// 根 → 当前节点的路径（含两端）。环检测只是防御坏场景，正常数据不会触发。
fn path_of(scene: &Scene, idx: u32) -> HashSet<u32> {
    let mut path = HashSet::new();
    let mut cur = Some(idx);
    while let Some(i) = cur {
        if !path.insert(i) {
            break;
        }
        cur = scene.nodes[i as usize].parent;
    }
    path
}

fn paint_path(scene: &mut Scene, path: &HashSet<u32>) {
    for n in std::sync::Arc::make_mut(&mut scene.nodes) {
        n.alpha = DIM_NODE;
    }
    for e in &mut scene.edges {
        e.alpha = DIM_DECOR;
    }
    for s in &mut scene.spines {
        s.alpha = DIM_DECOR;
    }
    for b in &mut scene.boundaries {
        b.alpha = DIM_DECOR;
    }
    for s in &mut scene.summaries {
        s.alpha = DIM_DECOR;
    }
    for r in &mut scene.relationships {
        r.alpha = DIM_DECOR;
    }
    for &i in path {
        if let Some(n) = std::sync::Arc::make_mut(&mut scene.nodes).get_mut(i as usize) {
            n.alpha = 1.0;
        }
    }
    for e in &mut scene.edges {
        if path.contains(&e.from) && path.contains(&e.to) {
            e.alpha = 1.0;
        }
    }
}

/// 单步的目标相机：节点居中，并放到能看清的倍率。
fn target(scene: &Scene, idx: u32, viewport: [f32; 2], ppp: f32) -> Camera {
    let r = scene.rect(idx);
    let vw = viewport[0].max(320.0);
    let vh = viewport[1].max(240.0);
    let z = (vw * FILL_RATIO / r.w.max(1.0))
        .min(vh * FILL_RATIO / r.h.max(1.0))
        .clamp(ZOOM_MIN, ZOOM_MAX);
    Camera::new(r.center(), z, ppp)
}

fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::text::MonoMeasurer;
    use mw_core::theme::get_theme;
    use mw_core::{build_scene, Command, Document, LayoutSettings};

    fn doc_with(children: &[&str]) -> Document {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        for t in children {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: (*t).into(),
                style: None,
            })
            .expect("添加子节点");
        }
        doc
    }

    fn add_child(doc: &mut Document, parent: &str, text: &str) -> String {
        doc.apply(&Command::AddNode {
            parent: parent.to_string(),
            index: None,
            text: text.into(),
            style: None,
        })
        .expect("添加子节点");
        doc.doc_order()
            .into_iter()
            .find(|id| doc.node(id).is_some_and(|n| n.text == text))
            .expect("新节点应有 id")
    }

    fn scene_of(doc: &Document) -> Scene {
        build_scene(
            doc,
            &doc.root_id,
            &LayoutSettings::default(),
            &get_theme("snow"),
            &MonoMeasurer,
            None::<(&str, &str)>,
        )
    }

    fn camera() -> Camera {
        Camera::new(Point::new(-800.0, 600.0), 0.2, 1.0)
    }

    fn demo_of(_doc: &Document, scene: &Scene, start: Option<u32>) -> Demo {
        Demo::start(
            scene,
            &get_theme("snow"),
            camera(),
            start,
            [1600.0, 900.0],
            true,
        )
        .expect("演示应能开始")
    }

    #[test]
    fn preorder_walks_the_visible_tree_like_a_reader() {
        let mut doc = doc_with(&["甲", "乙"]);
        let a = doc.doc_order()[1].clone();
        add_child(&mut doc, &a, "甲一");
        let scene = scene_of(&doc);
        // `Document::new` 的标题不进根主题文字，根节点本身是空串。
        let texts: Vec<&str> = preorder(&scene)
            .iter()
            .map(|&i| {
                let id = &scene.ids[i as usize];
                doc.node(id).map(|n| n.text.as_str()).unwrap_or("?")
            })
            .collect();
        assert_eq!(texts, ["", "甲", "甲一", "乙"]);
    }

    #[test]
    fn preorder_follows_document_order_not_visual_y() {
        let mut doc = doc_with(&["甲", "乙", "丙"]);
        let a = doc.doc_order()[1].clone();
        add_child(&mut doc, &a, "甲一");
        let scene = scene_of(&doc);
        let texts: Vec<&str> = preorder(&scene)
            .iter()
            .map(|&i| {
                let id = &scene.ids[i as usize];
                doc.node(id).map(|n| n.text.as_str()).unwrap_or("?")
            })
            .collect();
        // 默认布局把一级分支分到左右两侧，视觉上「乙」比「甲」高；演示仍按文档顺序走。
        assert_eq!(texts, ["", "甲", "甲一", "乙", "丙"]);
    }

    #[test]
    fn preorder_skips_folded_branches() {
        let mut doc = doc_with(&["甲", "乙"]);
        let a = doc.doc_order()[1].clone();
        add_child(&mut doc, &a, "甲一");
        doc.apply(&Command::SetCollapsed {
            id: a.clone(),
            collapsed: true,
        })
        .expect("折叠");
        let scene = scene_of(&doc);
        let texts: Vec<&str> = preorder(&scene)
            .iter()
            .map(|&i| {
                let id = &scene.ids[i as usize];
                doc.node(id).map(|n| n.text.as_str()).unwrap_or("?")
            })
            .collect();
        assert_eq!(texts, ["", "甲", "乙"], "折叠的子主题不在演示顺序里");
    }

    #[test]
    fn starting_at_a_selected_node_keeps_the_progress_position() {
        let doc = doc_with(&["甲", "乙", "丙"]);
        let scene = scene_of(&doc);
        let b = scene
            .find(
                doc.doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == "乙"))
                    .as_deref()
                    .expect("乙"),
            )
            .expect("乙在场景里");
        let demo = demo_of(&doc, &scene, Some(b));
        assert_eq!(demo.index(), 2, "根 / 甲 / 乙");
        assert_eq!(demo.len(), 4);
        assert_eq!(demo.current(), b);
    }

    #[test]
    fn path_nodes_stay_bright_and_everything_else_dims() {
        let mut doc = doc_with(&["甲", "乙"]);
        let a = doc.doc_order()[1].clone();
        let a1 = add_child(&mut doc, &a, "甲一");
        let scene = scene_of(&doc);
        let idx = scene.find(&a1).expect("甲一");
        let demo = demo_of(&doc, &scene, Some(idx));
        let alpha = |id: &str| demo.scene().node(demo.scene().find(id).expect(id)).alpha;
        assert_eq!(alpha(&doc.root_id), 1.0);
        assert_eq!(alpha(&a), 1.0);
        assert_eq!(alpha(&a1), 1.0);
        assert_eq!(alpha(doc.doc_order().last().expect("乙")), DIM_NODE);
        // 通往「甲一」的两段连线保持亮，其余压暗。
        for e in &demo.scene().edges {
            let from = &demo.scene().ids[e.from as usize];
            let to = &demo.scene().ids[e.to as usize];
            let on_path = (from == &doc.root_id && to == &a) || (from == &a && to == &a1);
            assert_eq!(e.alpha, if on_path { 1.0 } else { DIM_DECOR });
        }
    }

    #[test]
    fn next_and_prev_walk_the_preorder_and_stop_at_the_ends() {
        let doc = doc_with(&["甲", "乙"]);
        let scene = scene_of(&doc);
        let mut demo = demo_of(&doc, &scene, None);
        assert_eq!(demo.index(), 0);
        assert!(!demo.prev([1600.0, 900.0], true));
        assert!(demo.next([1600.0, 900.0], true));
        assert!(demo.next([1600.0, 900.0], true));
        assert_eq!(demo.index(), 2);
        assert!(!demo.next([1600.0, 900.0], true));
        assert!(demo.prev([1600.0, 900.0], true));
        assert_eq!(demo.index(), 1);
    }

    #[test]
    fn camera_glides_from_the_old_view_to_the_node_center() {
        let doc = doc_with(&["甲"]);
        let scene = scene_of(&doc);
        let demo = demo_of(&doc, &scene, None);
        let t0 = Instant::now();
        let begin = demo.saved_camera();
        assert!((begin.center.x - camera().center.x).abs() < 1e-3);
        assert!((begin.zoom - camera().zoom).abs() < 1e-4);

        let end = demo.camera(t0 + STEP + Duration::from_millis(10));
        let r = scene.rect(demo.current());
        assert!((end.center.x - r.center().x).abs() < 1e-3);
        assert!((end.center.y - r.center().y).abs() < 1e-3);
        assert!(end.zoom > begin.zoom, "演示应从总览放大到节点");

        // 缓动是 ease-out：中点已经走完大半路程，且缩放按对数插值落在两端之间。
        let mid = demo.camera(t0 + STEP / 2);
        assert!(mid.center.x > begin.center.x + (end.center.x - begin.center.x) * 0.6);
        assert!(mid.zoom > begin.zoom && mid.zoom < end.zoom);
    }

    #[test]
    fn reduce_motion_lands_on_the_target_immediately() {
        let doc = doc_with(&["甲"]);
        let scene = scene_of(&doc);
        let demo = Demo::start(
            &scene,
            &get_theme("snow"),
            camera(),
            None,
            [1600.0, 900.0],
            false,
        )
        .expect("演示应能开始");
        let now = Instant::now();
        assert!(demo.settled(now));
        let cam = demo.camera(now);
        let r = scene.rect(demo.current());
        assert!((cam.center.x - r.center().x).abs() < 1e-3);
        assert!((demo.saved_camera().center.x - camera().center.x).abs() < 1e-3);
    }
}
