//! Scene → 四层实例数据（docs/12 §3.1、docs/13 §3）。
//!
//! 两类尺寸分得很清：
//! - **形状**（盒尺寸、圆角、点阵间距、文字）是世界单位，随缩放一起放大——这是「地图缩放」；
//! - **铬件**（选中环 2px、折叠圆点 r8、连线 1.6/2.4px、关系线 1.6px、边界描边 1.5px、
//!   花括号 2px、节点描边、备注角标）是 CSS px 常量，一律 `÷zoom` 换算成世界单位，
//!   所以任何缩放档位下视觉粗细恒定（解 docs/12 D1/D2）。

use bytemuck::{Pod, Zeroable};
use mw_core::geom::Enclosing;
use mw_core::layout::{edge_path_with, toggle_pos, Edge};
use mw_core::theme::{Rgba, Shape};
use mw_core::{LayoutKind, Path, PathSeg, Point, Rect, Scene, SceneNode, SceneRelationship, Theme};

use crate::camera::Camera;
use crate::flatten::{flatten, tolerance};

pub const L_BACKDROP: usize = 0;
pub const L_LINES: usize = 1;
pub const L_NODES: usize = 2;
pub const L_OVERLAY: usize = 3;
/// 一共四层（背景点阵那层只有全屏三角形，没有实例）。
pub const LAYERS: usize = 4;

/// 圆角半径（世界单位，随缩放）：docs/12 D7 要求圆角成体系，这里只允许这一档节点圆角。
/// 壳层选中环、SVG/PDF 导出都读同一个令牌，不能再各写一个数。
pub const NODE_RADIUS: f32 = mw_core::design::radius::MD;
pub const BOUNDARY_RADIUS: f32 = mw_core::design::radius::PANEL;
/// 铬件（CSS px）。
pub const SELECT_RING_W: f32 = 1.5;
pub const SELECT_RING_GAP: f32 = 2.5;
/// 选中/悬停的底色（docs/30 §5.3：选中 8% 强调底、悬停 4%）。
/// 环负责「边界」，底色负责「整块被拿起来」——只有环时，彩色节点上的选中态很弱。
pub const SELECT_FILL: f32 = 0.08;
pub const HOVER_FILL: f32 = 0.04;
pub const HOVER_RING_W: f32 = 1.0;
pub const TOGGLE_R: f32 = 8.0;
pub const TOGGLE_W: f32 = 1.5;
/// 折叠点底色的外扩（CSS px）：环外侧再垫一圈画布色，穿过来的连线才不会
/// 贴着描边、把圆环「粘」在线上。命中半径 `TOGGLE_HIT_CSS` 必须盖住它。
pub const TOGGLE_HALO: f32 = 1.5;
pub const TOGGLE_BAR: f32 = 3.5;
pub const BADGE_R: f32 = 3.4;
pub const BADGE_OFFSET: f32 = 7.0;
/// 概念节点的卡片圆点（docs/30 §5.3.1）：直径 5 CSS px，落在盒内右下角。
/// 与大细纲行尾那颗同尺寸同色（`accent`），这样「同一个事实」在两个界面里长得一样。
pub const CONCEPT_CARD_DOT_R: f32 = 2.5;
pub const CONCEPT_CARD_DOT_INSET: f32 = 9.0;
/// 关系线起点圆点（对照旧 SVG 的 `r=3.2` + 下限 2.2）
pub const REL_START_R: f32 = 3.2;
pub const REL_START_FLOOR: f32 = 2.2;
/// 线宽（CSS px）。
pub const BOUNDARY_W: f32 = 1.5;
pub const BRACE_W: f32 = 2.0;
pub const CONNECTOR_W: f32 = 1.5;
pub const REL_W: f32 = 1.5;
/// 分组框：圆角 12（世界单位）、1.5px 描边、6/4 的虚线节奏（CM-18）。
pub const GROUP_RADIUS: f32 = 12.0;
pub const GROUP_W: f32 = 1.5;
pub const GROUP_DASH: [f32; 2] = [6.0, 4.0];
pub const REL_LABEL_W: f32 = 1.0;
/// 关系线标签胶囊：高 20 / 最小宽 24 是世界单位（旧 SVG 一致），描边 1px 是铬件。
pub const REL_LABEL_H: f32 = 20.0;
pub const REL_LABEL_MIN_W: f32 = 24.0;
// 背景点阵的几何（间距 / 半径）不在这里：它是**主题令牌**，唯一来源是
// `mw_core::theme::{DOT_SPACING, DOT_RADIUS}`，出口是 `Theme::canvas_dots()`。
// 渲染层与 SVG/PDF 导出都只读那个出口——各留一份常量就是「同源」破口的第一道缝。
/// 视口裁剪外扩（CSS px）：给描边、抗锯齿与 60fps 下的平移留一点余量。
pub const CULL_PAD_CSS: f32 = 64.0;
/// 文字 LOD：屏幕物理高度低于这个值就不整形（7 px 以下中文已不可读）。
pub const TEXT_MIN_PX: f32 = 7.0;
/// 节点数达到这个量级后隐藏标签胶囊与备注/链接角标（docs/12 §P1、08 §6）。
pub const LOD_NODE_COUNT: usize = 5_000;
/// 摘要主题的虚线节奏（世界单位，等同旧 SVG `stroke-dasharray: 4 3`）。
pub const SUMMARY_DASH: [f32; 2] = [4.0, 3.0];
/// 摘要主题描边宽度（CSS px，旧 SVG 为 1.2）。
pub const SUMMARY_TOPIC_W: f32 = 1.2;

/// sRGB 令牌值 → 着色器值。**不做线性转换**：与 egui / Direct2D / 旧 SVG 同一套
/// gamma 空间混合，换后端不会出现「同样的令牌换个灰度」的漂移。
#[inline]
pub fn rgba(c: Rgba) -> [f32; 4] {
    [
        c.0[0] as f32 / 255.0,
        c.0[1] as f32 / 255.0,
        c.0[2] as f32 / 255.0,
        c.0[3] as f32 / 255.0,
    ]
}

#[inline]
fn faded(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * a]
}

/// 圆角矩形（也可当胶囊、圆点、环用）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct RectInst {
    pub center: [f32; 2],
    pub half: [f32; 2],
    /// (圆角, 描边宽, 备用, 备用)，均为世界单位
    pub params: [f32; 4],
    pub fill: [f32; 4],
    pub stroke: [f32; 4],
}

/// 胶囊段（直线或细分后的小段）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SegInst {
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// (线宽, 虚线 0/1, 实长, 空长)
    pub meta: [f32; 4],
    pub color: [f32; 4],
}

/// 实心三角形（关系线箭头）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TriInst {
    pub p: [[f32; 2]; 3],
    pub _pad: [f32; 2],
    pub color: [f32; 4],
}

/// 一个字形四边形。`pos/size` 是世界单位（字形位图的像素矩形换算回 DIP），
/// `uv` 指向 R8 图集，`color` 是 sRGB 编码的原色，着色器按覆盖率输出预乘 alpha。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TextInst {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default)]
pub struct LayerGeom {
    pub rects: Vec<RectInst>,
    pub segs: Vec<SegInst>,
    pub tris: Vec<TriInst>,
    /// 文本实例由 `mw_render::text::TextEngine::paint` 填充；纯几何路径保持为空。
    pub texts: Vec<TextInst>,
}

impl LayerGeom {
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
            && self.segs.is_empty()
            && self.tris.is_empty()
            && self.texts.is_empty()
    }
}

/// 一帧要画什么：场景 + 主题 + 视口 + 交互态。
///
/// 文本（节点文字、标签胶囊文字、图标字形）由 `mw_render::text::TextEngine` 填入
/// 各层的 `texts`；`build_layers` 只出几何。
#[derive(Clone)]
pub struct Frame<'a> {
    pub scene: &'a Scene,
    pub theme: &'a Theme,
    pub camera: Camera,
    pub selected: Vec<u32>,
    pub hovered: Option<u32>,
    /// 悬停环透明度 0–1（120ms 过渡；`reduce_motion` 时恒为 1/0）。
    pub hover_alpha: f32,
    /// 选中环透明度 0–1（180ms 过渡）。
    pub select_alpha: f32,
    /// 正在行内编辑的节点：文字与图标交给 egui 编辑器绘制，合成器跳过它们避免重影。
    pub editing: Option<u32>,
    /// 拖拽幽灵；`None` = 没在拖。
    pub ghost: Option<Ghost>,
    /// 对齐参考线（拖拽吸附时出现）。空 = 没吸上。
    pub guides: Vec<GuideLine>,
    /// 视口逻辑尺寸（DIP）。`None` = 不裁剪（离屏全图、单元测试与导出）。
    pub viewport: Option<[f32; 2]>,
}

/// 一条对齐参考线。
///
/// 这里**故意**不复用 `mw_ui::snap::Guide`：渲染层不该反向依赖手势层。两者字段一致，
/// 壳层做一次三行的转换就行——比起让 `mw-render` 认识「吸附」这个概念，这点重复更便宜。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideLine {
    /// 竖线为 x、横线为 y（世界单位）。
    pub at: f32,
    /// 线段起止（垂直于线的方向）。
    pub from: f32,
    pub to: f32,
    pub vertical: bool,
}

/// 拖拽幽灵：被拖动的整棵子树平移到指针处画一份半透明副本，原位留淡化残影
/// （docs/12:79「半透明 50% + 原位淡化残影」）。
///
/// `nodes` 由壳层从 `mw_ui::drag::subtree` 得来（含各根及其后代），渲染层不再展树。
/// **必须与判「落点是否落在自己肚子里」用的是同一个集合**——两处各算一遍迟早会走岔。
#[derive(Clone, Debug, PartialEq)]
pub struct Ghost {
    pub nodes: Vec<u32>,
    /// 世界坐标偏移（当前指针 − 按下点）。
    pub offset: Point,
    /// 落点非法：幽灵轮廓转警示色。幽灵本身照常跟手——用户需要看见「我正拖着自己」。
    pub illegal: bool,
}

/// 幽灵副本的不透明度 / 原位残影的不透明度。文字层也用同一对系数。
pub const GHOST_ALPHA: f32 = 0.55;
pub const GHOST_ORIGIN_ALPHA: f32 = 0.28;
/// 幽灵轮廓的虚线节奏（世界单位）。
const GHOST_DASH: [f32; 2] = [6.0, 4.0];
/// 对齐参考线线宽（CSS px）。
const GUIDE_W: f32 = 1.0;
/// 非法落点警示色（docs/06 §5「指示变红」）。主题里没有语义红，单独定一个中性红，
/// 深浅色底上都够亮。
pub const DROP_BAD: Rgba = Rgba::rgb(0xE0, 0x4B, 0x4B);

impl Frame<'_> {
    /// 当前可见的世界矩形，含 [`CULL_PAD_CSS`] 缓冲；`None` 表示不做视口裁剪。
    pub fn view_world(&self) -> Option<Rect> {
        let [w, h] = self.viewport?;
        // 只防 0 除：缩得越小，世界视口越大，裁剪自然失效，不会误裁。
        let zoom = self.camera.zoom.max(1e-4);
        let pad = CULL_PAD_CSS / zoom;
        let hw = w * 0.5 / zoom + pad;
        let hh = h * 0.5 / zoom + pad;
        Some(Rect::new(
            self.camera.center.x - hw,
            self.camera.center.y - hh,
            hw * 2.0,
            hh * 2.0,
        ))
    }

    /// 矩形是否落在可见世界里。没有视口时一律算可见。
    pub fn sees(&self, r: Rect) -> bool {
        self.view_world().map_or(true, |v| v.intersects(r))
    }

    /// 字体在屏幕上是否还值得整形（LOD 的文本门槛）。
    pub fn text_visible(&self, font_size: f32) -> bool {
        font_size * self.camera.zoom * self.camera.ppp >= TEXT_MIN_PX
    }

    /// 大文档省渲染：5k 节点起不再画标签胶囊与备注/链接角标。
    pub fn labels_visible(&self) -> bool {
        self.scene.nodes.len() < LOD_NODE_COUNT
    }

    pub fn badges_visible(&self) -> bool {
        self.scene.nodes.len() < LOD_NODE_COUNT
    }

    /// 被拖动子树的遮罩（索引 → 是否属于幽灵）。几何与文字两条管线共用同一份判定，
    /// 否则会出现「盒子淡了、字还黑的」这种半截效果。
    pub fn ghost_mask(&self) -> Option<Vec<bool>> {
        let g = self.ghost.as_ref()?;
        let mut mask = vec![false; self.scene.nodes.len()];
        for &i in &g.nodes {
            if let Some(slot) = mask.get_mut(i as usize) {
                *slot = true;
            }
        }
        Some(mask)
    }
}

/// 构建四层几何。纯函数：同样的输入必然得到逐字节相同的实例，快照比对靠这一点。
pub fn build_layers(f: &Frame) -> [LayerGeom; LAYERS] {
    let cam = f.camera;
    let tol = tolerance(cam.zoom * cam.ppp);
    let mut out: [LayerGeom; LAYERS] = Default::default();

    // ---- L1 边界 + 连线 + 花括号 + 关系线 ----
    // 分组框（CM-18）：画在边界之前、所有连线之下——它是最底层的容器。
    for g in f.scene.groups.iter() {
        if !f.sees(g.rect) {
            continue;
        }
        out[L_LINES].rects.push(rect_dashed(
            g.rect,
            GROUP_RADIUS,
            cam.css_to_world(GROUP_W),
            faded(rgba(g.color), 0.06),
            faded(rgba(g.color), 0.55),
            GROUP_DASH,
        ));
    }
    for b in f.scene.boundaries.iter() {
        if !f.sees(b.rect) {
            continue;
        }
        out[L_LINES].rects.push(rect(
            b.rect,
            BOUNDARY_RADIUS,
            cam.css_to_world(BOUNDARY_W),
            faded(rgba(b.color), 0.07 * b.alpha),
            faded(rgba(b.color), 0.5 * b.alpha),
        ));
    }
    for s in f.scene.spines.iter() {
        if !f.sees(path_bounds(&s.path).expanded(cam.css_to_world(s.width))) {
            continue;
        }
        // 主轴（鱼骨/时间轴）的尾巴收成笔锋：等宽断头在 100% 以上像没画完。
        // 出生端保持主题宽度，末端收到 35%，方向与 bones 的锥形线一致。
        push_curve(
            &mut out[L_LINES],
            &s.path,
            cam,
            tol,
            s.width,
            s.width * 0.35,
            faded(rgba(s.color), s.alpha),
        );
    }
    for e in f.scene.edges.iter() {
        let path = edge_path_with(
            &Edge {
                from: e.from,
                to: e.to,
                start: e.start,
                end: e.end,
                orientation: e.orientation,
            },
            e.shape,
        );
        if !f.sees(path_bounds(&path).expanded(cam.css_to_world(e.width))) {
            continue;
        }
        push_curve(
            &mut out[L_LINES],
            &path,
            cam,
            tol,
            e.width,
            e.width_end,
            faded(rgba(e.color), e.alpha),
        );
        if e.arrow {
            let dx = e.end.x - e.start.x;
            let dy = e.end.y - e.start.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.01 {
                let size = cam.css_to_world(7.0);
                let ux = dx / len;
                let uy = dy / len;
                let base = Point::new(e.end.x - ux * size, e.end.y - uy * size);
                let half = size * 0.42;
                let px = -uy * half;
                let py = ux * half;
                out[L_LINES].tris.push(TriInst {
                    p: [
                        [e.end.x, e.end.y],
                        [base.x + px, base.y + py],
                        [base.x - px, base.y - py],
                    ],
                    _pad: [0.0; 2],
                    color: faded(rgba(e.color), e.alpha),
                });
            }
        }
    }
    for s in f.scene.summaries.iter() {
        if f.sees(path_bounds(&s.brace).expanded(cam.css_to_world(BRACE_W))) {
            push_curve(
                &mut out[L_LINES],
                &s.brace,
                cam,
                tol,
                BRACE_W,
                BRACE_W,
                faded(rgba(s.color), s.alpha),
            );
        }
        if f.sees(path_bounds(&s.connector).expanded(cam.css_to_world(CONNECTOR_W))) {
            push_curve(
                &mut out[L_LINES],
                &s.connector,
                cam,
                tol,
                CONNECTOR_W,
                CONNECTOR_W,
                faded(rgba(s.color), s.alpha),
            );
        }
    }
    for r in f.scene.relationships.iter() {
        if f.sees(path_bounds(&r.path).expanded(cam.css_to_world(r.width))) {
            let color = faded(rgba(r.color), r.alpha);
            if r.dashed {
                push_dashed_curve(&mut out[L_LINES], &r.path, cam, tol, r.width, color);
            } else {
                push_curve(
                    &mut out[L_LINES],
                    &r.path,
                    cam,
                    tol,
                    r.width,
                    r.width,
                    color,
                );
            }
        }
    }

    // ---- L2 节点 + 标签胶囊 + 摘要主题盒 ----
    // 被拖动的那片子树在原位只留淡化残影：整块消失会让人以为节点已经被删掉了。
    let mask = f.ghost_mask();
    let faded_at = |idx: u32| -> f32 {
        match &mask {
            Some(m) if m.get(idx as usize).copied().unwrap_or(false) => GHOST_ORIGIN_ALPHA,
            _ => 1.0,
        }
    };
    for idx in f.scene.draw_order.iter() {
        let n = f.scene.node(*idx);
        if !f.sees(n.geom.rect) {
            continue;
        }
        let st = n.style;
        let stroke = cam.css_to_world(st.stroke_width);
        let fade = faded_at(*idx) * n.alpha;
        if st.shape == Shape::Underline {
            // 下划线主题：没有卡片，只有文字下面一条线。线在盒子底边上，
            // 粗细仍按 CSS px 恒定（chrome），所以随缩放一起换成世界单位。
            let y = n.geom.rect.bottom() - cam.css_to_world(1.0);
            out[L_NODES].segs.push(seg(
                Point::new(n.geom.rect.x, y),
                Point::new(n.geom.rect.right(), y),
                stroke.max(cam.css_to_world(1.0)),
                faded(rgba(st.stroke), fade),
            ));
        } else {
            out[L_NODES].rects.push(rect(
                n.geom.rect,
                corner_style(&st, n.geom.rect),
                stroke,
                faded(rgba(st.fill), fade),
                faded(rgba(st.stroke), fade),
            ));
        }
        if n.geom.image_h > 0.0 {
            // 图片占位：真正解码/上传在 P53，这里先给一块可测量的空槽。
            let pad = cam.css_to_world(8.0);
            let inner_w = (n.geom.rect.w - pad * 2.0).max(1.0);
            let slot_h = (n.geom.image_h - pad * 2.0).max(1.0);
            let ratio = n.geom.image_ratio.max(0.05);
            let img_h = (inner_w / ratio).min(slot_h);
            let img = Rect::new(
                n.geom.rect.x + pad,
                n.geom.rect.y + pad + (slot_h - img_h) * 0.5,
                inner_w,
                img_h,
            );
            out[L_NODES].rects.push(rect(
                img,
                cam.css_to_world(6.0),
                cam.css_to_world(1.0),
                faded(rgba(f.theme.ui.panel), fade),
                faded(rgba(f.theme.ui.border), fade),
            ));
        }
        if !f.labels_visible() {
            continue;
        }
        let origin = Point::new(n.geom.rect.x, n.geom.rect.y);
        for p in n.labels.iter() {
            let world = offset(p.rect, origin);
            if !f.text_visible(p.font_size) || !f.sees(world) {
                continue;
            }
            out[L_NODES].rects.push(rect(
                world,
                world.h / 2.0,
                0.0,
                faded(rgba(st.branch_color), 0.92 * fade),
                rgba(Rgba::TRANSPARENT),
            ));
        }
    }
    // 幽灵副本最后压进 L2：它必须盖在被拖过去的那些节点上面。
    if let Some(g) = f.ghost.as_ref() {
        for &i in &g.nodes {
            let Some(n) = f.scene.nodes.get(i as usize) else {
                continue;
            };
            let r = shifted(n.geom.rect, g.offset);
            if !f.sees(r) {
                continue;
            }
            let st = n.style;
            let ghost_alpha = GHOST_ALPHA * n.alpha;
            out[L_NODES].rects.push(rect(
                r,
                corner_style(&st, n.geom.rect),
                cam.css_to_world(st.stroke_width),
                faded(rgba(st.fill), ghost_alpha),
                faded(rgba(st.stroke), ghost_alpha),
            ));
            if !f.labels_visible() {
                continue;
            }
            for p in n.labels.iter() {
                if !f.text_visible(p.font_size) {
                    continue;
                }
                out[L_NODES].rects.push(rect(
                    shifted(
                        offset(p.rect, Point::new(n.geom.rect.x, n.geom.rect.y)),
                        g.offset,
                    ),
                    p.rect.h / 2.0,
                    0.0,
                    faded(rgba(st.branch_color), 0.92 * ghost_alpha),
                    rgba(Rgba::TRANSPARENT),
                ));
            }
        }
    }
    for s in f.scene.summaries.iter() {
        if !f.sees(s.topic) {
            continue;
        }
        out[L_NODES].rects.push(rect_dashed(
            s.topic,
            s.topic.h / 2.0,
            cam.css_to_world(SUMMARY_TOPIC_W),
            faded(rgba(f.theme.ui.canvas_bg), 0.85 * s.alpha),
            faded(rgba(s.color), s.alpha),
            SUMMARY_DASH,
        ));
    }

    // ---- L3 覆盖层：关系线端点与箭头、选中环、折叠圆点、备注角标 ----
    let concept = f.scene.kind.is_concept_map();
    for r in f.scene.relationships.iter() {
        if !f.sees(relationship_bounds(r, cam)) {
            continue;
        }
        // 概念图的短语是「线在标签处断开」，胶囊不能有描边（`docs/30` §5.4）：
        // 有线宽的胶囊看起来像个按钮，还会把断口解释成「连接到一个控件」。
        let w = if concept {
            0.0
        } else {
            cam.css_to_world(REL_LABEL_W)
        };
        if r.label.is_some() {
            // 标签胶囊的尺寸是世界单位：里面的字是世界单位 12，随缩放一起长大
            let box_h = REL_LABEL_H;
            let pill = Rect::new(
                r.label_at.x - r.label_w / 2.0,
                r.label_at.y - box_h / 2.0,
                r.label_w.max(REL_LABEL_MIN_W),
                box_h,
            );
            out[L_OVERLAY].rects.push(rect(
                pill,
                box_h / 2.0,
                w,
                faded(rgba(f.theme.ui.canvas_bg), r.alpha),
                faded(rgba(r.color), r.alpha),
            ));
        }
        // 起点圆点是思维导图「关系线挂在卡片边上」的锚点提示；概念图节点本来
        // 就在线的两端，再点一颗只会脏了盒边。
        if !concept {
            out[L_OVERLAY].rects.push(dot(
                r.start,
                cam.chrome(REL_START_R, REL_START_FLOOR),
                faded(rgba(r.color), r.alpha),
            ));
        }
        // ⑦：终点端 / 起点端各一个三角形，没有就不画。
        // 屏幕恒定长度的换算只有 `mw_core::geom::arrow_at_zoom` 一份实现，命中与导出读同一份。
        for tri in [r.arrow, r.arrow_back].into_iter().flatten() {
            let tri = mw_core::geom::arrow_at_zoom(tri, cam.zoom);
            out[L_OVERLAY].tris.push(TriInst {
                p: [
                    [tri[0].x, tri[0].y],
                    [tri[1].x, tri[1].y],
                    [tri[2].x, tri[2].y],
                ],
                _pad: [0.0; 2],
                color: faded(rgba(r.color), r.alpha),
            });
        }
    }
    let canvas_bg = rgba(f.theme.ui.canvas_bg);
    for idx in f.scene.draw_order.iter() {
        let n = f.scene.node(*idx);
        if !f.sees(n.geom.rect.expanded(cam.chrome(TOGGLE_R + 2.0, 4.5))) {
            continue;
        }
        if let Some(h) = n.parent.and_then(|p| f.scene.nodes.get(p as usize)) {
            if n.has_children {
                push_toggle(
                    &mut out[L_OVERLAY],
                    n,
                    f.scene.kind,
                    h.geom.rect.x,
                    canvas_bg,
                    cam,
                    n.alpha,
                );
            }
        }
    }
    let has_sel = |idx: u32| f.selected.iter().any(|s| s == &idx);
    for idx in f.hovered.iter().chain(f.selected.iter()) {
        // 行内编辑时强调环由 egui 编辑器画，合成器再画一圈会重影。
        if f.editing == Some(*idx) {
            continue;
        }
        let n = f.scene.node(*idx);
        let (w, alpha) = if Some(*idx) == f.hovered && !has_sel(*idx) {
            (cam.css_to_world(HOVER_RING_W), f.hover_alpha * 0.55)
        } else {
            (cam.css_to_world(SELECT_RING_W), f.select_alpha)
        };
        if alpha <= 0.001 {
            continue;
        }
        let color = faded(rgba(f.theme.ui.accent), alpha);
        let gap = cam.css_to_world(SELECT_RING_GAP);
        if !f.sees(n.geom.rect.expanded(gap + w)) {
            continue;
        }
        // 底色先铺、描边后画：只有环时，深浅不同的节点看起来像「框了一圈」，
        // 加一层很淡的强调底，整颗节点才像被拿起来。色值按 §5.3 的 4% / 8%。
        let wash = if Some(*idx) == f.hovered && !has_sel(*idx) {
            HOVER_FILL * f.hover_alpha
        } else {
            SELECT_FILL * f.select_alpha
        };
        if wash > 0.001 {
            out[L_OVERLAY].rects.push(rect(
                n.geom.rect,
                corner_style(&n.style, n.geom.rect),
                0.0,
                faded(rgba(f.theme.ui.accent), wash),
                rgba(Rgba::TRANSPARENT),
            ));
        }
        out[L_OVERLAY].rects.push(rect(
            n.geom.rect.expanded(gap),
            corner_style(&n.style, n.geom.rect) + gap,
            w,
            rgba(Rgba::TRANSPARENT),
            color,
        ));
    }
    // 幽灵的并集轮廓：虚线框把「正在拖的是什么」圈成一块。
    //
    // 只在多根/多节点时才画：单个节点的幽灵本身就有边，再套一圈虚线会跟落点指示
    // 抢注意力（两者都是强调色，叠在一起反而看不出哪个是「将要落到哪」）。
    if let Some(g) = f.ghost.as_ref().filter(|g| g.nodes.len() > 1) {
        let mut box_ = Enclosing::new();
        for &i in &g.nodes {
            if let Some(n) = f.scene.nodes.get(i as usize) {
                box_.add(shifted(n.geom.rect, g.offset));
            }
        }
        if let Some(u) = box_.rect() {
            let pad = cam.css_to_world(2.0);
            let color = if g.illegal {
                rgba(DROP_BAD)
            } else {
                faded(rgba(f.theme.ui.accent), 0.8)
            };
            out[L_OVERLAY].rects.push(rect_dashed(
                u.expanded(pad),
                cam.chrome(6.0, 3.0),
                cam.css_to_world(1.5),
                rgba(Rgba::TRANSPARENT),
                color,
                GHOST_DASH,
            ));
        }
    }
    // 备注 / 链接角标：大文档进 LOD 后不再画（M1c）
    if f.badges_visible() {
        for idx in f.scene.draw_order.iter() {
            let n = f.scene.node(*idx);
            let r = cam.chrome(BADGE_R, 2.4);
            let dx = n.geom.rect.right() - cam.chrome(BADGE_OFFSET, 3.0);
            let dy = n.geom.rect.y + cam.chrome(BADGE_OFFSET, 3.0);
            if !f.sees(Rect::new(dx - r * 3.0, dy - r, r * 6.0, r * 2.0)) {
                continue;
            }
            if n.has_note {
                out[L_OVERLAY].rects.push(dot(
                    Point::new(dx, dy),
                    r,
                    faded(rgba(n.style.branch_color), 0.85 * n.alpha),
                ));
            }
            if n.has_link {
                out[L_OVERLAY].rects.push(dot(
                    Point::new(dx - r * 3.0, dy),
                    r,
                    faded(rgba(n.style.branch_color), 0.6 * n.alpha),
                ));
            }
        }
    }

    // 概念节点的卡片圆点（docs/30 §5.3.1）：右下角内侧 5px。
    //
    // 与备注/链接角标分开画：那两个在**右上角**且只表示「这个节点有备注/有链接」，
    // 概念图里的卡片是「这个概念本身就是一张卡片」——位置和语义都不能混。
    if f.scene.kind.is_concept_map() && f.badges_visible() {
        let r = cam.chrome(CONCEPT_CARD_DOT_R, 1.6);
        let inset = cam.chrome(CONCEPT_CARD_DOT_INSET, 4.0);
        for idx in f.scene.draw_order.iter() {
            let n = f.scene.node(*idx);
            if !n.has_card {
                continue;
            }
            let c = Point::new(n.geom.rect.right() - inset, n.geom.rect.bottom() - inset);
            if !f.sees(Rect::new(
                c.x - inset,
                c.y - inset,
                inset * 2.0,
                inset * 2.0,
            )) {
                continue;
            }
            out[L_OVERLAY]
                .rects
                .push(dot(c, r, faded(rgba(f.theme.ui.accent), n.alpha)));
        }
    }

    // 对齐参考线最后画：它要压在所有东西上面才像一把尺。
    // 线宽用 1 CSS px（`chrome`），任何缩放档位下都一样细。
    if !f.guides.is_empty() {
        let w = cam.chrome(GUIDE_W, 0.3);
        let color = faded(rgba(f.theme.ui.accent), 0.75);
        for g in &f.guides {
            let (a, b) = if g.vertical {
                (Point::new(g.at, g.from), Point::new(g.at, g.to))
            } else {
                (Point::new(g.from, g.at), Point::new(g.to, g.at))
            };
            out[L_OVERLAY].segs.push(seg(a, b, w, color));
        }
    }

    out
}

/// 路径的保守包围盒：贝塞尔落在控制点凸包内，所以取起点 + 全部控制点即可。
fn path_bounds(path: &Path) -> Rect {
    let mut e = Enclosing::new();
    let mut add = |p: Point| e.add(Rect::new(p.x, p.y, 0.0, 0.0));
    add(path.start);
    for seg in &path.segs {
        match *seg {
            PathSeg::Line { to } => add(to),
            PathSeg::Quad { ctrl, to } => {
                add(ctrl);
                add(to);
            }
            PathSeg::Cubic { c1, c2, to } => {
                add(c1);
                add(c2);
                add(to);
            }
        }
    }
    e.rect()
        .unwrap_or(Rect::new(path.start.x, path.start.y, 0.0, 0.0))
}

/// 关系线整组（曲线 + 箭头 + 起点圆点 + 标签胶囊）的包围盒，任一部件可见就整组保留。
fn relationship_bounds(r: &SceneRelationship, cam: Camera) -> Rect {
    let mut e = Enclosing::new();
    e.add(path_bounds(&r.path).expanded(cam.css_to_world(REL_W)));
    for tri in [r.arrow, r.arrow_back].into_iter().flatten() {
        let tri = mw_core::geom::arrow_at_zoom(tri, cam.zoom);
        let xs = tri.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let xe = tri.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let ys = tri.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let ye = tri.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        e.add(Rect::new(xs, ys, xe - xs, ye - ys));
    }
    let dot_r = cam.chrome(REL_START_R, REL_START_FLOOR);
    e.add(Rect::new(
        r.start.x - dot_r,
        r.start.y - dot_r,
        dot_r * 2.0,
        dot_r * 2.0,
    ));
    if r.label.is_some() {
        let w = r.label_w.max(REL_LABEL_MIN_W);
        e.add(Rect::new(
            r.label_at.x - w * 0.5,
            r.label_at.y - REL_LABEL_H * 0.5,
            w,
            REL_LABEL_H,
        ));
    }
    e.rect()
        .unwrap_or(Rect::new(r.start.x, r.start.y, 0.0, 0.0))
}

/// 把一条路径展开成一串胶囊段。`width_css` → `width_end_css` 沿弧长线性收细，
/// 得到 XMind 2026 主题里的「锥形分支线」（`line-tapered: tapered`）。
///
/// 用弧长而不是段序号：曲线细分后段长不等，按序号会让锥度在弯道处突然变陡。
/// 每段的宽度取段中点的比例；胶囊是圆头的，相邻段的宽度差在 0.2px 以内时接缝不可见。
fn push_curve(
    layer: &mut LayerGeom,
    path: &Path,
    cam: Camera,
    tol: f32,
    width_css: f32,
    width_end_css: f32,
    color: [f32; 4],
) {
    let pts = flatten(path, tol);
    if pts.len() < 2 {
        return;
    }
    let w0 = cam.css_to_world(width_css);
    let w1 = cam.css_to_world(width_end_css);
    if (w1 - w0).abs() < 1e-4 {
        for w in pts.windows(2) {
            layer.segs.push(seg(w[0], w[1], w0, color));
        }
        return;
    }
    let mut cum = Vec::with_capacity(pts.len());
    let mut total = 0.0f32;
    cum.push(0.0);
    for w in pts.windows(2) {
        total += ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        cum.push(total);
    }
    if total <= f32::EPSILON {
        return;
    }
    for (i, w) in pts.windows(2).enumerate() {
        let t = (0.5 * (cum[i] + cum[i + 1]) / total).clamp(0.0, 1.0);
        layer.segs.push(seg(w[0], w[1], w0 + (w1 - w0) * t, color));
    }
}

/// 虚线曲线：把折线按 [`mw_core::REL_DASH`] 的节奏切成实线段（CM-16）。
///
/// 为什么在渲染层切、而不是让 `Scene` 存一堆短线段：`Scene` 是几何真相源，
/// 虚线节奏属于「怎么画」。切在渲染层，`Scene` 仍是一条完整曲线——
/// 命中判定、包围盒、SVG/PDF 导出都不必认识「一段虚线由 12 个小段组成」。
///
/// 相位（`phase`）跨折线拐点连续累加，所以弯线上的虚线不会被拐点切断。
fn push_dashed_curve(
    layer: &mut LayerGeom,
    path: &Path,
    cam: Camera,
    tol: f32,
    width_css: f32,
    color: [f32; 4],
) {
    let pts = flatten(path, tol);
    if pts.len() < 2 {
        return;
    }
    let w = cam.css_to_world(width_css);
    let mut phase = 0.0f32;
    for pair in pts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 1e-6 {
            continue;
        }
        let (ux, uy) = (dx / len, dy / len);
        let (runs, next) = dash_walk(len, phase, mw_core::REL_DASH);
        for (s0, s1) in runs {
            let p0 = Point::new(a.x + ux * s0, a.y + uy * s0);
            let p1 = Point::new(a.x + ux * s1, a.y + uy * s1);
            layer.segs.push(seg(p0, p1, w, color));
        }
        phase = next;
    }
}

/// 在一段长度为 `len` 的直线上按 `dash = [实长, 空长]` 走一遍，
/// 返回**要画的区间**（相对这段起点）与走完后的相位。
///
/// 抽成纯函数是为了能测：相位跨折线拐点必须连续（否则弯线在拐点处会多断一截），
/// 而这条性质在屏幕上很难用眼睛验证。
fn dash_walk(len: f32, phase: f32, dash: [f32; 2]) -> (Vec<(f32, f32)>, f32) {
    let [on, off] = dash;
    let period = (on + off).max(0.5);
    let mut runs = Vec::new();
    let mut s = 0.0f32;
    let mut phase = phase;
    while s < len - 1e-6 {
        let cycle = phase % period;
        let inked = cycle < on;
        let until = if inked { on - cycle } else { period - cycle };
        let step = until.min(len - s).max(0.0);
        if inked && step > 1e-4 {
            runs.push((s, s + step));
        }
        s += step;
        phase += step;
        if step <= 1e-6 {
            break;
        }
    }
    (runs, phase)
}

/// 折叠圆点：一圈**描边环**（不是实心点），外加一圈比环大的画布色底。
///
/// 拆成两个实例而不是一个「填充+描边」矩形：填充只到环的内沿，连线会在环的
/// 外沿与描边反锯齿之间露出来，缩小后看着像「环粘在线上」。底色圆外扩
/// [`TOGGLE_HALO`]，先把穿过来的连线挖掉，再画环与 +/−。
fn push_toggle(
    layer: &mut LayerGeom,
    node: &SceneNode,
    kind: LayoutKind,
    parent_x: f32,
    canvas_bg: [f32; 4],
    cam: Camera,
    alpha: f32,
) {
    let local = toggle_pos(node.geom.rect, kind, Some(parent_x));
    let c = Point::new(node.geom.rect.x + local.x, node.geom.rect.y + local.y);
    let r = cam.chrome(TOGGLE_R, 4.5);
    let halo = cam.css_to_world(TOGGLE_HALO);
    let w = cam.css_to_world(TOGGLE_W);
    let color = faded(rgba(node.style.branch_color), alpha);
    // 1) 底色圆：把连线挖掉，并给环留 1.5px 净空。
    layer.rects.push(RectInst {
        center: [c.x, c.y],
        half: [r + halo, r + halo],
        params: [r + halo, 0.0, 0.0, 0.0],
        fill: faded(canvas_bg, alpha),
        stroke: rgba(Rgba::TRANSPARENT),
    });
    // 2) 描边环：无填充，只有一圈分支色。
    layer.rects.push(RectInst {
        center: [c.x, c.y],
        half: [r, r],
        params: [r, w, 0.0, 0.0],
        fill: rgba(Rgba::TRANSPARENT),
        stroke: color,
    });
    let bar = cam.css_to_world(TOGGLE_BAR);
    layer.segs.push(seg(
        Point::new(c.x - bar, c.y),
        Point::new(c.x + bar, c.y),
        w,
        color,
    ));
    if node.collapsed {
        layer.segs.push(seg(
            Point::new(c.x, c.y - bar),
            Point::new(c.x, c.y + bar),
            w,
            color,
        ));
    }
}

/// 圆角半径属于「形状」：胶囊的高一半、方角 0、圆角走统一档。
/// 形状 → 圆角半径。**全仓唯一出口**：wgpu 实例、SVG 导出、PDF 导出、egui 编辑环
/// 都问这一句，免得四处各写一份 `match` 然后慢慢分叉。
pub fn corner(shape: Shape, r: Rect) -> f32 {
    match shape {
        // 负半径 = 椭圆，见 `shaders/rect.wgsl` 的 `fs` 开头注释。
        Shape::Ellipse => ELLIPSE_CODE,
        Shape::Capsule => r.h / 2.0,
        Shape::Rect | Shape::Underline => 0.0,
        Shape::Rounded => NODE_RADIUS,
    }
}

/// 带样式覆盖的圆角半径：`radius` 只对圆角矩形/矩形生效（0–24），
/// 椭圆 / 胶囊 / 下划线仍走形状自己的规则（SE-13）。
pub fn corner_style(style: &mw_core::ResolvedStyle, r: Rect) -> f32 {
    match style.shape {
        Shape::Rounded | Shape::Rect => style
            .radius
            .map(|v| v.clamp(0.0, 24.0))
            .unwrap_or_else(|| corner(style.shape, r)),
        _ => corner(style.shape, r),
    }
}

/// 「画成椭圆」的哨兵半径。取负且绝对值足够大，任何真实圆角都不可能等于它
/// （圆角最大是 `r.h / 2`，概念盒高 36 时也只有 18）。
pub const ELLIPSE_CODE: f32 = -1_000.0;

fn rect(r: Rect, radius: f32, stroke_w: f32, fill: [f32; 4], stroke: [f32; 4]) -> RectInst {
    RectInst {
        center: [r.center().x, r.center().y],
        half: [r.w / 2.0, r.h / 2.0],
        params: [radius, stroke_w, 0.0, 0.0],
        fill,
        stroke,
    }
}

/// 虚线描边的圆角矩形；`dash = [实长, 空长]`，世界单位。
fn rect_dashed(
    r: Rect,
    radius: f32,
    stroke_w: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
    dash: [f32; 2],
) -> RectInst {
    RectInst {
        center: [r.center().x, r.center().y],
        half: [r.w / 2.0, r.h / 2.0],
        params: [radius, stroke_w, dash[0], dash[1]],
        fill,
        stroke,
    }
}

fn dot(c: Point, r: f32, color: [f32; 4]) -> RectInst {
    RectInst {
        center: [c.x, c.y],
        half: [r, r],
        params: [r, 0.0, 0.0, 0.0],
        fill: color,
        stroke: rgba(Rgba::TRANSPARENT),
    }
}

fn seg(a: Point, b: Point, width: f32, color: [f32; 4]) -> SegInst {
    SegInst {
        a: [a.x, a.y],
        b: [b.x, b.y],
        meta: [width, 0.0, 0.0, 0.0],
        color,
    }
}

fn offset(local: Rect, origin: Point) -> Rect {
    Rect::new(local.x + origin.x, local.y + origin.y, local.w, local.h)
}

/// 矩形整体平移。幽灵用它把整棵子树挪到指针处，尺寸不变。
fn shifted(r: Rect, d: Point) -> Rect {
    Rect::new(r.x + d.x, r.y + d.y, r.w, r.h)
}

#[cfg(test)]
mod dash_tests {
    use super::{corner_style, dash_walk, ELLIPSE_CODE, NODE_RADIUS};
    use mw_core::theme::{Rgba, Shape};
    use mw_core::Rect;

    /// 默认节奏：6 实 4 空。
    const D: [f32; 2] = [6.0, 4.0];

    #[test]
    fn a_short_line_gets_one_run_and_leaves_the_phase_mid_dash() {
        let (runs, phase) = dash_walk(4.0, 0.0, D);
        assert_eq!(runs, vec![(0.0, 4.0)], "4 < 6，整段都是实线");
        assert!((phase - 4.0).abs() < 1e-4);
    }

    #[test]
    fn phase_carries_across_segments_so_a_bend_never_restarts_the_rhythm() {
        // 第一段 4（还在实线里），第二段 4：先补 2 个实线单位，再空 2 个。
        let (first, phase) = dash_walk(4.0, 0.0, D);
        assert_eq!(first, vec![(0.0, 4.0)]);
        let (second, _) = dash_walk(4.0, phase, D);
        assert_eq!(second, vec![(0.0, 2.0)], "补齐 6 个实线单位就转空");
    }

    #[test]
    fn a_long_line_alternates_and_ends_exactly_at_the_length() {
        let (runs, _) = dash_walk(25.0, 0.0, D);
        assert_eq!(runs, vec![(0.0, 6.0), (10.0, 16.0), (20.0, 25.0)]);
        // 相邻两段的间距就是空长 4，节拍不漂。
        for w in runs.windows(2) {
            assert!((w[1].0 - w[0].1 - 4.0).abs() < 1e-4);
        }
    }

    /// SE-13：半径覆盖只对「有角」的两种形状生效。
    /// 胶囊取高一半、椭圆走负哨兵、下划线没有角——这三种下 `radius` 必须被忽略，
    /// 否则文件里一个残留的 `radius` 会把胶囊画成尖角或把椭圆画没。
    #[test]
    fn an_override_radius_only_applies_to_shapes_that_have_corners() {
        let r = Rect::new(0.0, 0.0, 120.0, 36.0);
        // `ResolvedStyle` 没有 `Default`（主题解析出来的每个字段都有意义），这里手工铺一份。
        let styled = |shape: Shape, radius: Option<f32>| mw_core::ResolvedStyle {
            fill: Rgba::TRANSPARENT,
            text: Rgba::TRANSPARENT,
            bold: false,
            italic: false,
            font_size: 14.0,
            shape,
            stroke: Rgba::TRANSPARENT,
            stroke_width: 1.0,
            branch_color: Rgba::TRANSPARENT,
            radius,
            pad: None,
            align: None,
            weight: mw_core::theme::WEIGHT_REGULAR,
        };
        // 圆角矩形：覆盖生效，并夹在 0–24。
        assert_eq!(corner_style(&styled(Shape::Rounded, Some(6.0)), r), 6.0);
        assert_eq!(corner_style(&styled(Shape::Rounded, Some(99.0)), r), 24.0);
        assert_eq!(corner_style(&styled(Shape::Rect, Some(0.0)), r), 0.0);
        // 没覆盖就走形状自己的规则。
        assert_eq!(corner_style(&styled(Shape::Rounded, None), r), NODE_RADIUS);
        assert_eq!(corner_style(&styled(Shape::Rect, None), r), 0.0);
        // 三个「没有角」的形状无视覆盖。
        assert_eq!(
            corner_style(&styled(Shape::Capsule, Some(3.0)), r),
            r.h * 0.5,
            "胶囊的半径是高一半"
        );
        assert_eq!(
            corner_style(&styled(Shape::Ellipse, Some(3.0)), r),
            ELLIPSE_CODE,
            "椭圆走负哨兵，不看 radius"
        );
        assert_eq!(corner_style(&styled(Shape::Underline, Some(3.0)), r), 0.0);
    }

    #[test]
    fn a_degenerate_dash_never_loops_forever() {
        // 实长 0：全部是空，一条线段都不画，而且必须能退出循环。
        let (runs, _) = dash_walk(10.0, 0.0, [0.0, 4.0]);
        assert!(runs.is_empty());
    }
}
