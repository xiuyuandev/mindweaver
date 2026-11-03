//! 缩略图（小地图 / minimap / overview）——**纯几何层**：世界 ⇄ 缩略卡的映射、
//! 视口矩形、拖动定位与大文档降采样。只消费 `mw_core::Scene`，不碰窗口、GPU 与 egui
//! （docs/04 §核心纪律 3「Scene 单一来源：画布、导出、缩略图共用」——不为缩略图另写布局）。
//!
//! 命名取 `overview` 而不是 `thumbnail`：docs/05 的 `.zwg` 附件里 `thumbnail.png` 是
//! **文档预览缩略图**，与这里的画布导航小地图是两回事，混名会串。
//!
//! 规格：docs/02 §4「小地图」（P1：右下角、显示全图与视口框、拖动视口框定位、可折叠、
//! 大文档降采样）；docs/06 §4「右下 190×130；视口框可拖；>5k 节点自动降采样；
//! 可折叠为 34×26 角标」。
//!
//! 取景口径与壳层**逐式一致**（壳层代码只是只读参考，本层复制公式、不依赖 `mw-shell`）：
//! · 可视矩形 == `mw-shell/src/canvas/canvas_methods_1.rs:407 view_bounds`
//!   （center ± viewport/2/zoom，zoom 下限 1e-4）——见 [`view_rect`]；
//! · 等比装入 == `canvas_methods_2.rs:1763 fit_bounds` 的骨架
//!   （`scale = min(内框宽/内容宽, 内框高/内容高)` 再居中）。`FIT_PAD=56`、
//!   `FIT_MIN_ZOOM=0.005`、`FIT_MAX_ZOOM=1.5` 是壳层**取景策略**，不归本层：缩略卡的
//!   pad 由调用方传入，卡内比例不需要钳进相机区间——退化内容直接 `None`，
//!   而不是像 `fit_bounds` 那样给 `w.max(1.0)` 兜底——见 [`Map::new`]；
//! · 拖动定位沿用 `canvas.rs:131 reveal_center` 的「视口必须保住内容可见」纪律，
//!   但余量口径不同（48 CSS px 是「目标盒到视口边」的边距；小地图拖的是整个视口框，
//!   等价纪律是「整框不出活动区」）——见 [`center_from_thumb`]。
//!
//! 抽样口径回收自旧实现（`MINIMAP_NODES=900`，`docs/archive/14-项目交接说明.md:212`：
//! 「小地图用 `draw_order` 等步长确定性抽样，别依赖 HashMap 顺序」）。

use mw_core::{Point, Rect, Scene};

/// 展开态缩略卡尺寸（CSS px）。出处：docs/06 §4「小地图：右下 190×130」。
pub const MAP_W: f32 = 190.0;
pub const MAP_H: f32 = 130.0;
/// 折叠态角标尺寸（CSS px）。出处：docs/06 §4「可折叠为 34×26 角标」。
/// 角标不是「什么都不画」：壳层仍要能往里塞一个视口框，所以折叠态也走 [`Map::new`]。
pub const COLLAPSED_W: f32 = 34.0;
pub const COLLAPSED_H: f32 = 26.0;
/// 降采样块数上限。旧实现口径 `MINIMAP_NODES=900`（archive/14 §6 面板纪律）；
/// 纯逻辑层不依赖绘制帧率，900 作为默认封顶沿用——改它只需要改这一处。
pub const MAX_BLOCKS: usize = 900;
/// 自动降采样的节点数阈值。出处：docs/06 §4「>5k 节点自动降采样」。
/// 壳层接线用 [`blocks_for`] 即可按这条口径走。
pub const DOWNSAMPLE_MIN_NODES: usize = 5_000;

/// 世界 ⇄ 缩略卡的等比映射。
///
/// 约定：`offset` 是**缩略卡原点 (0,0) 对应的世界坐标**，`size` 是卡的 CSS px 尺寸。
/// 内容包围盒（调用方传 `scene.bounds`）等比装进 `size` 内缩 `pad` 的内框，并且
/// 居中——与 `fit_bounds` 同一个骨架，只是不退化为「把相机塞进视口」而是「把内容塞进卡」。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Map {
    /// 世界 → 卡内像素的比例（world px × scale = thumb px）。
    pub scale: f32,
    /// 卡原点 (0,0) 对应的世界坐标。
    pub offset: Point,
    /// 卡的 CSS px 尺寸。
    pub size: [f32; 2],
}

impl Map {
    /// 把 `content`（即 `scene.bounds`）等比装进 `size` 的内框（四周内缩 `pad`），居中。
    ///
    /// `scale = min((size.0 - 2pad) / content.w, (size.1 - 2pad) / content.h)`。
    /// 退化返回 `None`：内容宽或高 ≤ 0（空场景/病态包围盒），或卡太小（内框 ≤ 0）——
    /// 与其画出一张除零爆炸的卡，不如让调用方不画。
    pub fn new(content: Rect, size: [f32; 2], pad: f32) -> Option<Map> {
        if content.w <= 0.0 || content.h <= 0.0 {
            return None;
        }
        let inner_w = size[0] - 2.0 * pad;
        let inner_h = size[1] - 2.0 * pad;
        if inner_w <= 0.0 || inner_h <= 0.0 {
            return None;
        }
        let scale = (inner_w / content.w).min(inner_h / content.h);
        // 居中 ⇔ content 的盒心落在卡的盒心：
        // world_to_thumb(content.center()) == size/2 ⇒ offset = center − size/(2·scale)。
        let c = content.center();
        let offset = Point::new(c.x - size[0] / (2.0 * scale), c.y - size[1] / (2.0 * scale));
        Some(Map {
            scale,
            offset,
            size,
        })
    }

    /// 世界坐标 → 卡内坐标。
    pub fn world_to_thumb(&self, p: Point) -> Point {
        Point::new(
            (p.x - self.offset.x) * self.scale,
            (p.y - self.offset.y) * self.scale,
        )
    }

    /// 卡内坐标 → 世界坐标。与 [`Map::world_to_thumb`] 互逆（单测钉 roundtrip）。
    pub fn thumb_to_world(&self, p: Point) -> Point {
        Point::new(
            p.x / self.scale + self.offset.x,
            p.y / self.scale + self.offset.y,
        )
    }

    /// 世界矩形 → 卡内矩形（轴对齐保序，直接线性变换四元组）。
    pub fn rect_in_thumb(&self, r: Rect) -> Rect {
        Rect::new(
            (r.x - self.offset.x) * self.scale,
            (r.y - self.offset.y) * self.scale,
            r.w * self.scale,
            r.h * self.scale,
        )
    }
}

/// 当前相机看到的**世界坐标**可视矩形。
///
/// 口径逐式 == `mw-shell/src/canvas/canvas_methods_1.rs:407 view_bounds`：
/// `zoom = zoom.max(1e-4)`，`hw = w*0.5/zoom`，`hh = h*0.5/zoom`，
/// `Rect::new(center.x − hw, center.y − hh, hw*2, hh*2)`。
/// 壳层接线时把 `camera.center`、`camera.zoom`、`last_viewport` 原样传进来即可，
/// 本层不出现 Camera 类型。
pub fn view_rect(center: Point, zoom: f32, viewport: [f32; 2]) -> Rect {
    let zoom = zoom.max(1e-4);
    let hw = viewport[0] * 0.5 / zoom;
    let hh = viewport[1] * 0.5 / zoom;
    Rect::new(center.x - hw, center.y - hh, hw * 2.0, hh * 2.0)
}

/// 点击/拖拽缩略图 → 新相机 center（纯函数；壳层拿到返回值直接写 `camera.center`）。
///
/// 语义：世界目标 `t = map.thumb_to_world(at)`，然后把 `t` 钳进**活动区**——
/// 活动区 = `zone = content ∪ view`（内容并上当前视口）每边内缩**半个视口**后的矩形：
///
/// ```text
/// c.x ∈ [zone.x + view.w/2, zone.right() − view.w/2]
/// c.y ∈ [zone.y + view.h/2, zone.bottom() − view.h/2]
/// ```
///
/// 钳住中心就等价于「新视口整框不出 zone」——这是 `reveal_center`（canvas.rs:131）
/// 「保住可见」纪律在小地图上的版本：
/// · 余量选**半视口**而不是 reveal_center 的 48 CSS px——48px 是「目标盒到视口边」的
///   边距，用在把视口本身整个搬走的拖框场景没有对应物；整框不出 zone 才是「拖到哪都
///   看得见内容」的字面保证。
/// · zone 取 `content ∪ view` 而不是 `content`——内容比视口小（缩小到全图外还有白边）
///   时纯按 content 钳会让区间上下限倒置；并上当前视口后 zone 每边至少半个视口，
///   区间必然非空，用户也不会被「弹回」到看不见自己原来所在之处。
/// · reveal_center 仍然有用武之地：双击缩略卡定位**某个节点**时，壳层应走
///   reveal_center 而不是本函数——那是「露出目标」，不是「拖视口框」。
pub fn center_from_thumb(map: &Map, content: Rect, at: Point, view: Rect) -> Point {
    let t = map.thumb_to_world(at);
    let zone = content.union(view);
    let half_w = view.w * 0.5;
    let half_h = view.h * 0.5;
    // zone ⊇ view ⇒ zone.w ≥ view.w ⇒ lo ≤ hi，恒成立；
    // 万一 view 病态（NaN 视口之类）让区间倒置，退回 zone 中心是唯一稳定读法。
    let (lo_x, hi_x) = (zone.x + half_w, zone.right() - half_w);
    let (lo_y, hi_y) = (zone.y + half_h, zone.bottom() - half_h);
    let cx = if lo_x <= hi_x {
        t.x.clamp(lo_x, hi_x)
    } else {
        zone.center().x
    };
    let cy = if lo_y <= hi_y {
        t.y.clamp(lo_y, hi_y)
    } else {
        zone.center().y
    };
    Point::new(cx, cy)
}

/// 双击缩略卡：世界点 `world` 落在哪颗节点上（可外扩 `pad_world` 容差）。
///
/// 缩略卡上一颗节点通常只有 1–2px 宽，严格包含几乎点不中；壳层按
/// `4 css px / map.scale` 折成世界容差再调本函数。多颗都命中时取**盒中心最近**的
/// 那颗（重叠的父子盒不会随机选）；一颗都没有返回 `None`，壳层按「双击空白」处理
/// （视口框照旧搬过去），不选中、不跳相机。
pub fn node_at<'a>(scene: &'a Scene, world: Point, pad_world: f32) -> Option<&'a str> {
    let pad = pad_world.max(0.0);
    let mut best: Option<(f32, &'a str)> = None;
    for (idx, node) in scene.nodes.iter().enumerate() {
        let Some(id) = scene.ids.get(idx).map(String::as_str) else {
            continue;
        };
        let r = node.geom.rect.expanded(pad);
        if !r.contains(world) {
            continue;
        }
        let c = node.geom.rect.center();
        let d = (c.x - world.x) * (c.x - world.x) + (c.y - world.y) * (c.y - world.y);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, id));
        }
    }
    best.map(|(_, id)| id)
}

/// 大文档降采样：按 `draw_order` 等步长抽 ≤ `max_blocks` 个**场景下标**。
///
/// 确定性：整数等距索引，无 HashMap 顺序、无随机数、无浮点累积（archive/14 §6 的
/// 「别依赖 HashMap 顺序」纪律）。节点数不超过 `max_blocks` 时原样全出。
/// 含首含尾：`k = 0` 取 `draw_order[0]`、`k = max_blocks − 1` 取最后一个——
/// 旧实现的「含首」思路照旧，再补「含尾」，保证最深/最浅的两端永远在卡上。
/// `max_blocks == 1` 时只含首（一块画不下两个点）；`== 0` 或空场景返回空。
pub fn sample(scene: &Scene, max_blocks: usize) -> Vec<u32> {
    let n = scene.draw_order.len();
    if max_blocks == 0 || n == 0 {
        return Vec::new();
    }
    if n <= max_blocks {
        return scene.draw_order.clone();
    }
    if max_blocks == 1 {
        return vec![scene.draw_order[0]];
    }
    // 整数等距：idx(k) = k·(n−1) ÷ (max−1)。n−1 ≥ max（n > max）⇒ 相邻差 ≥ 1，
    // 严格递增、无重复；k=0 → 0，k=max−1 → n−1，首尾必中。
    let last = (n - 1) as u64;
    let denom = (max_blocks - 1) as u64;
    (0..max_blocks)
        .map(|k| scene.draw_order[(k as u64 * last / denom) as usize])
        .collect()
}

/// docs/06 §4「>5k 节点自动降采样」的现成口径：不超过阈值全量，超过按 [`MAX_BLOCKS`]
/// 封顶。壳层不传数、直接调这个，两条规格（5k 触发、900 上限）就都在一处钉死。
pub fn blocks_for(scene: &Scene) -> Vec<u32> {
    let cap = if scene.nodes.len() > DOWNSAMPLE_MIN_NODES {
        MAX_BLOCKS
    } else {
        usize::MAX
    };
    sample(scene, cap)
}

/// 缩略卡上的一个可画代理。几何都已在**卡内坐标**，壳层只管填色描边。
#[derive(Clone, Debug, PartialEq)]
pub enum Proxy {
    /// 节点盒：`rect` 是 `scene.rect(idx)` 经 [`Map::rect_in_thumb`]；`depth` 供
    /// 按层级着色（与画布同一套 `geom.depth`，大文档一眼能看出分枝层次）。
    Block { idx: u32, rect: Rect, depth: u32 },
    /// 关系线代理：`start →（折到）label_at →（再到）to 节点中心` 的两段折线。
    /// 不展开 `path` 贝塞尔——卡上 200px 宽画曲线没有信息量，中点锚保证线与
    /// docs/30 的命题标签位置同源。`from` 用 `r.start`（盒边端点），不用 from 盒中心。
    Link { from: Point, mid: Point, to: Point },
}

/// 缩略代理数据：先块后线（壳层按序画，线自然压块）。
///
/// 块来自 [`sample`]（调用方决定封顶：全量口径用 [`blocks_for`]）；
/// 线来自 `scene.relationships` 全量——关系线在大文档里仍是小数目，且漏画一条
/// 就是「图上明明有却看不见」的假信息。节点下标越界（过渡 blend 帧之外的病态数据）
/// 跳过不 panic。
pub fn proxies(scene: &Scene, map: &Map, max_blocks: usize) -> Vec<Proxy> {
    let mut out = Vec::new();
    for &idx in sample(scene, max_blocks).iter() {
        let Some(node) = scene.nodes.get(idx as usize) else {
            continue;
        };
        out.push(Proxy::Block {
            idx,
            rect: map.rect_in_thumb(node.geom.rect),
            depth: node.geom.depth,
        });
    }
    for r in scene.relationships.iter() {
        // from 节点必须还在（r.start 是它的盒边端点，节点没了线就没有出处）；
        // to 节点中心是折线的终点。
        let (Some(_from), Some(to)) = (
            scene.nodes.get(r.from as usize),
            scene.nodes.get(r.to as usize),
        ) else {
            continue;
        };
        out.push(Proxy::Link {
            from: map.world_to_thumb(r.start),
            mid: map.world_to_thumb(r.label_at),
            to: map.world_to_thumb(to.geom.rect.center()),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::text::MonoMeasurer;
    use mw_core::{build_scene, get_theme, Command, Document, LayoutSettings, Scene};

    /// 夹具：星形树——根 + `children` 个子节点，共 `children + 1` 颗。
    fn star(children: usize) -> Document {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        for i in 0..children {
            doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: format!("节点 {i}"),
                style: None,
            })
            .unwrap();
        }
        doc
    }

    fn build(doc: &Document) -> Scene {
        build_scene(
            doc,
            &doc.root_id,
            &LayoutSettings::default(),
            &get_theme("snow"),
            &MonoMeasurer,
            None::<(&str, &str)>,
        )
    }

    fn fixture() -> Scene {
        build(&star(7))
    }

    fn near(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    /// M30b：双击缩略卡命中节点——盒内必中、容差内按外扩盒算、容差外不中。
    #[test]
    fn node_at_hits_the_box_and_honours_the_pad() {
        let scene = fixture();
        let root = scene.nodes[0].geom.rect;
        let rid = scene.ids[0].clone();
        assert_eq!(node_at(&scene, root.center(), 0.0), Some(rid.as_str()));
        // 根盒上方 8 世界单位：4 的容差够不着，12 的容差够得着。
        let p = Point::new(root.center().x, root.y - 8.0);
        assert_eq!(node_at(&scene, p, 4.0), None);
        assert_eq!(node_at(&scene, p, 12.0), Some(rid.as_str()));
        // 离全图很远：容差再大也不中。
        let far = Point::new(root.center().x + 10_000.0, root.center().y);
        assert_eq!(node_at(&scene, far, 12.0), None);
    }

    #[test]
    fn the_map_roundtrips_world_points_through_the_thumbnail() {
        let content = Rect::new(-80.0, -40.0, 320.0, 160.0);
        let map = Map::new(content, [MAP_W, MAP_H], 8.0).expect("非退化内容必须有地图");
        for p in [
            Point::ZERO,
            content.center(),
            Point::new(-80.0, -40.0),
            Point::new(240.0, 120.0),
            Point::new(123.5, -77.25),
        ] {
            let back = map.thumb_to_world(map.world_to_thumb(p));
            assert!(
                near(back.x, p.x, 1e-4) && near(back.y, p.y, 1e-4),
                "往返必须回到原点：({p:?}) → 卡 → ({back:?})，容差 1e-4"
            );
        }
    }

    #[test]
    fn the_map_fits_the_content_box_into_the_card() {
        // fit_bounds 同骨架：scale 取两个比例里小的那个（这里宽向封顶），四边 ≥ pad，
        // 宽向贴满、高向居中，等比不变形。
        let content = Rect::new(100.0, -50.0, 640.0, 400.0);
        let pad = 8.0;
        let map = Map::new(content, [MAP_W, MAP_H], pad).expect("必须有地图");
        let expect_scale = ((MAP_W - 2.0 * pad) / 640.0).min((MAP_H - 2.0 * pad) / 400.0);
        assert!(
            near(map.scale, expect_scale, 1e-6),
            "scale 必须 == min(内框宽/内容宽, 内框高/内容高)，实际 {} vs {}",
            map.scale,
            expect_scale
        );
        let tb = map.rect_in_thumb(content);
        assert!(
            near(tb.x, pad, 1e-4) && near(tb.right(), MAP_W - pad, 1e-4),
            "宽向必须贴满内框且左右各留 pad：{:?}",
            tb
        );
        assert!(
            tb.y >= pad - 1e-4 && tb.bottom() <= MAP_H - pad + 1e-4,
            "高向不许越出内框：{:?}",
            tb
        );
        assert!(
            near(tb.center().x, MAP_W / 2.0, 1e-3) && near(tb.center().y, MAP_H / 2.0, 1e-3),
            "内容盒心必须落在卡心：{:?}",
            tb.center()
        );
        assert!(
            (tb.w * content.h - tb.h * content.w).abs() < 1e-2,
            "等比：卡内宽高比必须等于世界宽高比，{:?} vs {:?}",
            tb,
            content
        );
    }

    #[test]
    fn a_degenerate_content_box_has_no_map() {
        // 零宽、零高、以及「卡太小装不下」三类退化：一律 None，不许炸、不许 inf。
        assert!(
            Map::new(Rect::new(5.0, 5.0, 0.0, 120.0), [MAP_W, MAP_H], 8.0).is_none(),
            "零宽内容没有地图"
        );
        assert!(
            Map::new(Rect::new(0.0, 0.0, 100.0, 0.0), [MAP_W, MAP_H], 8.0).is_none(),
            "零高内容没有地图"
        );
        assert!(
            Map::new(Rect::new(0.0, 0.0, 100.0, 100.0), [10.0, 10.0], 8.0).is_none(),
            "pad 吃光卡宽时没有地图"
        );
    }

    #[test]
    fn the_view_rect_agrees_with_the_canvas_formula() {
        // 对照公式**逐字节抄自** mw-shell/src/canvas/canvas_methods_1.rs:407 view_bounds
        // （壳层只读不许依赖，公式抄进来对拍；同一条 f32 式子应当精确相等）。
        fn shell_view_bounds(center: Point, zoom: f32, viewport: [f32; 2]) -> Rect {
            let zoom = zoom.max(1e-4);
            let hw = viewport[0] * 0.5 / zoom;
            let hh = viewport[1] * 0.5 / zoom;
            Rect::new(center.x - hw, center.y - hh, hw * 2.0, hh * 2.0)
        }
        let cases: [(Point, f32, [f32; 2]); 3] = [
            (Point::new(0.0, 0.0), 1.0, [1600.0, 900.0]),
            (Point::new(-250.5, 77.25), 0.317, [1234.0, 567.0]),
            (Point::new(4096.0, -8192.0), 5.0, [1920.0, 1080.0]),
        ];
        for (c, z, vp) in cases {
            assert_eq!(
                view_rect(c, z, vp),
                shell_view_bounds(c, z, vp),
                "view_rect 必须与壳层 view_bounds 逐式一致 (center={c:?} zoom={z})"
            );
        }
        // zoom 病态下限也同口径：0 与负数都被夹到 1e-4，不产生 inf。
        let r = view_rect(Point::new(10.0, 20.0), 0.0, [800.0, 600.0]);
        assert_eq!(
            r,
            shell_view_bounds(Point::new(10.0, 20.0), 0.0, [800.0, 600.0])
        );
        assert!(r.w.is_finite() && r.h.is_finite(), "零 zoom 不许爆炸");
        let c = view_rect(Point::new(12.5, -34.0), 0.8, [800.0, 600.0]).center();
        assert!(
            near(c.x, 12.5, 1e-3) && near(c.y, -34.0, 1e-3),
            "可视矩形中心必须就是相机中心"
        );
    }

    #[test]
    fn dragging_the_thumb_pins_the_viewport_inside_the_content() {
        let content = Rect::new(0.0, 0.0, 1200.0, 800.0);
        let map = Map::new(content, [MAP_W, MAP_H], 8.0).expect("必须有地图");
        let vp = [400.0, 300.0];
        let zoom = 1.0;
        // 当前视口歪在内容左上角外：zone = content ∪ view 才能把「自己所在」保住。
        let view = view_rect(Point::new(100.0, 100.0), zoom, vp);
        let zone = content.union(view);

        // 拖到卡右下角之外：目标远出内容，视口应被钳成「恰好贴住 zone 右下」，
        // 中心 = (zone.right − view.w/2, zone.bottom − view.h/2) = (1000, 650)。
        let c = center_from_thumb(&map, content, Point::new(MAP_W + 30.0, MAP_H + 30.0), view);
        assert!(
            near(c.x, 1000.0, 1e-2) && near(c.y, 650.0, 1e-2),
            "右下角钳位应为 (1000, 650)，实际 ({}, {})",
            c.x,
            c.y
        );
        let r = view_rect(c, zoom, vp);
        assert!(
            r.x >= zone.x - 1e-2
                && r.right() <= zone.right() + 1e-2
                && r.y >= zone.y - 1e-2
                && r.bottom() <= zone.bottom() + 1e-2,
            "钳后视口必须整框不出 zone：视口 {r:?} zone {zone:?}"
        );
        assert!(r.intersects(content), "钳后视口必须还看得见内容");

        // 拖到内容中心：不该被钳，中心精确落在 content.center()。
        let mid = center_from_thumb(&map, content, map.world_to_thumb(content.center()), view);
        assert!(
            near(mid.x, content.center().x, 1e-3) && near(mid.y, content.center().y, 1e-3),
            "中心不该被钳：({mid:?})"
        );

        // 拖到卡左上角 (0,0)：目标在 zone 左上之外，钳到左上极限 (zone.x+200, zone.y+150)。
        let c2 = center_from_thumb(&map, content, Point::new(-5.0, -5.0), view);
        assert!(
            near(c2.x, zone.x + 200.0, 1e-2) && near(c2.y, zone.y + 150.0, 1e-2),
            "左上角钳位应为 ({}, {})，实际 ({}, {})",
            zone.x + 200.0,
            zone.y + 150.0,
            c2.x,
            c2.y
        );
    }

    #[test]
    fn a_collapsed_badge_still_maps_the_whole_viewport_box() {
        // 折叠成 34×26 角标不是「停画」：Map::new 必须还能算，视口框还能摆正。
        let scene = fixture();
        let map = Map::new(scene.bounds, [COLLAPSED_W, COLLAPSED_H], 0.0)
            .expect("角标也要有地图（docs/06:127 折叠态仍显示视口）");
        assert!(map.scale > 0.0 && map.scale.is_finite(), "角标比例必须正常");
        let t = map.world_to_thumb(scene.bounds.center());
        assert!(
            near(t.x, COLLAPSED_W / 2.0, 1e-3) && near(t.y, COLLAPSED_H / 2.0, 1e-3),
            "内容盒心必须落在角标中心：({t:?})"
        );
        let view = view_rect(scene.bounds.center(), 0.25, [1600.0, 900.0]);
        let tb = map.rect_in_thumb(view);
        assert!(tb.w.is_finite() && tb.h.is_finite(), "视口框必须有限");
        let c = center_from_thumb(&map, scene.bounds, Point::new(1.0, 1.0), view);
        assert!(
            c.x.is_finite() && c.y.is_finite(),
            "角标上的拖动定位不许 NaN"
        );
    }

    #[test]
    fn sampling_keeps_every_node_when_small_and_caps_when_huge() {
        let small = fixture();
        assert_eq!(small.nodes.len(), 8, "夹具应为 8 颗");
        // 小文档：全量原样输出，顺序就是 draw_order。
        assert_eq!(
            sample(&small, MAX_BLOCKS),
            small.draw_order,
            "8 颗必须全出且顺序照旧"
        );
        // 封顶抽 4：首尾必中。
        let s4 = sample(&small, 4);
        assert_eq!(s4.len(), 4, "抽 4 就恰好 4 块");
        assert_eq!(s4[0], small.draw_order[0], "必须含首");
        assert_eq!(
            *s4.last().unwrap(),
            *small.draw_order.last().unwrap(),
            "必须含尾"
        );
        // 边界：0 块与单块不许 panic。
        assert!(sample(&small, 0).is_empty(), "max_blocks=0 返回空");
        assert_eq!(sample(&small, 1), vec![small.draw_order[0]], "单块只含首");

        // 大文档：直接 build_scene 造 1001 颗（与 marquee.rs 同一套夹具路径，
        // 不需要手写假 Scene——build_scene 对星形树足够快，实测远低于测试超时线）。
        let big = build(&star(1000));
        assert_eq!(big.nodes.len(), 1001, "夹具应为 1001 颗");
        let s = sample(&big, MAX_BLOCKS);
        assert_eq!(s.len(), MAX_BLOCKS, "1001 颗必须被钳到 900 块");
        assert_eq!(s[0], big.draw_order[0], "含首");
        assert_eq!(*s.last().unwrap(), *big.draw_order.last().unwrap(), "含尾");
        let mut uniq = s.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), MAX_BLOCKS, "等步长不许出重复下标");
        for &v in s.iter() {
            assert!(
                (v as usize) < big.nodes.len() && big.draw_order.contains(&v),
                "抽样下标必须是 draw_order 里的合法场景下标，实际 {v}"
            );
        }
        // 上限大于节点数：原样全出（含 usize::MAX 这种「不封顶」用法）。
        assert_eq!(sample(&big, usize::MAX), big.draw_order, "不封顶就全量");

        // blocks_for 的 docs/06 口径：5k 以下全量、以上封顶。
        assert_eq!(blocks_for(&big), big.draw_order, "1001 ≤ 5k：全量");
        let mut pseudo = big.clone();
        // SceneNode 只 Clone 不 Copy：cycle+take 凑 6006 颗（>5k）。
        // draw_order 仍指前 1001 个合法下标，sample/blocks_for 都只看这两个字段。
        pseudo.nodes =
            std::sync::Arc::new(pseudo.nodes.iter().cloned().cycle().take(6006).collect());
        assert_eq!(
            blocks_for(&pseudo).len(),
            MAX_BLOCKS,
            ">5k 必须自动降到 {MAX_BLOCKS} 块封顶"
        );
    }

    #[test]
    fn proxies_are_deterministic_and_bounded() {
        // 夹具带一条关系线（根 → 第一个孩子）。
        let mut doc = star(7);
        let root = doc.root_id.clone();
        let kid = doc.doc_order()[1].clone();
        doc.apply(&Command::AddRelationship {
            from: root.clone(),
            to: kid.clone(),
            label: Some("链接".into()),
        })
        .unwrap();
        let scene = build(&doc);
        assert!(!scene.relationships.is_empty(), "夹具必须带关系线");
        let map = Map::new(scene.bounds, [MAP_W, MAP_H], 8.0).expect("必须有地图");

        let a = proxies(&scene, &map, MAX_BLOCKS);
        let b = proxies(&scene, &map, MAX_BLOCKS);
        assert_eq!(a, b, "同一输入两次代理数据必须逐字节相等");

        let blocks: Vec<_> = a
            .iter()
            .filter(|p| matches!(p, Proxy::Block { .. }))
            .collect();
        let links: Vec<_> = a
            .iter()
            .filter(|p| matches!(p, Proxy::Link { .. }))
            .collect();
        assert_eq!(blocks.len(), scene.nodes.len(), "8 颗小文档全出块");
        assert_eq!(links.len(), scene.relationships.len(), "关系线全出");
        for p in a.iter() {
            match p {
                Proxy::Block { idx, rect, .. } => {
                    assert!((*idx as usize) < scene.nodes.len(), "块下标越界：{idx}");
                    let expect = map.rect_in_thumb(scene.rect(*idx));
                    assert_eq!(*rect, expect, "块矩形必须走 rect_in_thumb 同式");
                }
                Proxy::Link { from, mid, to } => {
                    for pt in [from, mid, to] {
                        assert!(pt.x.is_finite() && pt.y.is_finite(), "折点必须有限");
                    }
                }
            }
        }
        // 折线语义：from = r.start 经 world_to_thumb，to = 目标节点中心。
        let r = &scene.relationships[0];
        let want = Proxy::Link {
            from: map.world_to_thumb(r.start),
            mid: map.world_to_thumb(r.label_at),
            to: map.world_to_thumb(scene.rect(r.to).center()),
        };
        assert!(
            a.contains(&want),
            "Link 必须是 start→label_at→to 中心 的同式折线"
        );

        // 块数封顶：大文档的 proxies 也超不过 MAX_BLOCKS 块。
        let big = build(&star(1000));
        let big_map = Map::new(big.bounds, [MAP_W, MAP_H], 8.0).expect("必须有地图");
        let big_proxies = proxies(&big, &big_map, MAX_BLOCKS);
        let big_blocks = big_proxies
            .iter()
            .filter(|p| matches!(p, Proxy::Block { .. }))
            .count();
        assert_eq!(big_blocks, MAX_BLOCKS, "1001 颗的块必须钳到 900");

        // 越界引用：把关系线的 to 改成病态下标，跳过而不 panic。
        let mut broken = big.clone();
        broken.relationships = scene.relationships.clone();
        broken.relationships[0].to = 999_999;
        let bp = proxies(&broken, &map, MAX_BLOCKS);
        assert!(
            !bp.iter().any(|p| matches!(p, Proxy::Link { .. })),
            "越界的关系线必须被跳过，不许 panic"
        );
    }
}
