//! 画布浮层的几何与绘制：顶部上下文工具条、右下缩放条。
//!
//! 全部是屏幕空间坐标：浮层大小不随画布缩放变，命中区也不进 `Scene`。
//! 几何是纯函数，方便单测钉住「互不重叠、不出画布、避让面包屑」。

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};
use mw_core::design;

use crate::chrome::Chrome;
use crate::icons::Icon;
use crate::ui_kit::{tool_button, ToolButtonState};

pub(crate) const FLOAT_RADIUS: f32 = design::radius::CONTROL;
pub(crate) const FLOAT_PAD: f32 = design::space::LG;

/// 画布浮层共用投影：比菜单轻一档，只把玻璃条从纸上托起来。
pub(crate) fn float_shadow() -> egui::epaint::Shadow {
    let s = design::elevation::PANEL;
    egui::epaint::Shadow {
        offset: s.offset,
        blur: s.blur,
        spread: s.spread,
        color: Color32::from_black_alpha(s.alpha),
    }
}

const TOOL: f32 = design::component::TOOL;
const TOOL_GAP: f32 = design::component::TOOL_GAP;
const TOOL_SEP: f32 = design::component::TOOL_GROUP_SEP;
const TOOLBAR_PAD: f32 = design::space::SM;
const TOOLBAR_TOP: f32 = design::space::MD;
const TOOLBAR_INSET: f32 = design::space::MD;
/// 三组按钮：撤销/重做 | 加子/同级/删除 | 卡片/更多。
const TOOL_GROUPS: [usize; 3] = [2, 3, 2];
/// 276 = 7×28 + 4×4 + 2×24 + 2×8。
pub(crate) const TOOLBAR_W: f32 = 7.0 * TOOL + 4.0 * TOOL_GAP + 2.0 * TOOL_SEP + 2.0 * TOOLBAR_PAD;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectionToolbar {
    pub bar: Rect,
    pub undo: Rect,
    pub redo: Rect,
    pub add_child: Rect,
    pub add_sibling: Rect,
    pub delete: Rect,
    pub card: Rect,
    pub more: Rect,
    seps: [Rect; 2],
}

/// 顶部居中；与面包屑相交时下移。空间不够就不画——不压节点、不出画布。
pub(crate) fn toolbar_layout(central: Rect, crumbs: Option<Rect>) -> Option<SelectionToolbar> {
    if central.width() < TOOLBAR_W + TOOLBAR_INSET * 2.0
        || central.height() < design::component::TOOLBAR_H + TOOLBAR_INSET * 2.0
    {
        return None;
    }
    let mut bar = Rect::from_min_size(
        Pos2::new(
            central.center().x - TOOLBAR_W * 0.5,
            central.top() + TOOLBAR_TOP,
        ),
        Vec2::new(TOOLBAR_W, design::component::TOOLBAR_H),
    );
    if let Some(c) = crumbs {
        let overlap_x =
            bar.left() < c.right() + TOOLBAR_PAD && bar.right() > c.left() - TOOLBAR_PAD;
        let overlap_y =
            bar.top() < c.bottom() + TOOLBAR_PAD && bar.bottom() > c.top() - TOOLBAR_PAD;
        if overlap_x && overlap_y {
            bar = bar.translate(Vec2::new(0.0, c.bottom() + TOOLBAR_PAD - bar.top()));
        }
    }
    if bar.bottom() > central.bottom() - TOOLBAR_INSET {
        return None;
    }

    let by = bar.top() + (design::component::TOOLBAR_H - TOOL) * 0.5;
    let mut x = bar.left() + TOOLBAR_PAD;
    let mut buttons = [Rect::from_min_size(Pos2::ZERO, Vec2::ZERO); 7];
    let mut at = 0;
    let mut seps = [Rect::from_min_size(Pos2::ZERO, Vec2::ZERO); 2];
    let mut sep_at = 0;
    for (gi, n) in TOOL_GROUPS.iter().enumerate() {
        for i in 0..*n {
            buttons[at] = Rect::from_min_size(Pos2::new(x, by), Vec2::splat(TOOL));
            at += 1;
            x += TOOL;
            if i + 1 < *n {
                x += TOOL_GAP;
            }
        }
        if gi + 1 < TOOL_GROUPS.len() {
            let cx = x + TOOL_SEP * 0.5;
            seps[sep_at] = Rect::from_min_size(
                Pos2::new(cx - design::stroke::HAIRLINE * 0.5, bar.top() + 10.0),
                Vec2::new(
                    design::stroke::HAIRLINE,
                    design::component::TOOLBAR_H - 20.0,
                ),
            );
            sep_at += 1;
            x += TOOL_SEP;
        }
    }
    Some(SelectionToolbar {
        bar,
        undo: buttons[0],
        redo: buttons[1],
        add_child: buttons[2],
        add_sibling: buttons[3],
        delete: buttons[4],
        card: buttons[5],
        more: buttons[6],
        seps,
    })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ToolbarState {
    pub undo: bool,
    pub redo: bool,
    pub add: bool,
    pub delete: bool,
    pub card: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolbarAction {
    Undo,
    Redo,
    AddChild,
    AddSibling,
    Delete,
    Card,
    More,
}

impl ToolbarAction {
    pub(crate) fn trace_name(self) -> &'static str {
        match self {
            Self::Undo => "canvas-tool-undo",
            Self::Redo => "canvas-tool-redo",
            Self::AddChild => "canvas-tool-child",
            Self::AddSibling => "canvas-tool-sibling",
            Self::Delete => "canvas-tool-delete",
            Self::Card => "canvas-tool-card",
            Self::More => "canvas-tool-more",
        }
    }
}

/// 画工具条并返回本帧被点击的动作。状态每帧由调用方读取，按钮不缓存。
pub(crate) fn toolbar(
    ui: &mut egui::Ui,
    ch: Chrome,
    t: SelectionToolbar,
    st: ToolbarState,
) -> Option<ToolbarAction> {
    let painter = ui.painter();
    painter.add(float_shadow().as_shape(t.bar, FLOAT_RADIUS));
    painter.rect_filled(t.bar, FLOAT_RADIUS, ch.panel);
    painter.rect_stroke(
        t.bar,
        FLOAT_RADIUS,
        Stroke::new(design::stroke::HAIRLINE, ch.border),
        StrokeKind::Inside,
    );
    for sep in t.seps {
        painter.vline(
            sep.center().x,
            sep.y_range(),
            Stroke::new(design::stroke::HAIRLINE, ch.border),
        );
    }

    let items = [
        (
            t.undo,
            Icon::Undo,
            st.undo,
            "撤销 (Ctrl+Z)",
            ToolbarAction::Undo,
        ),
        (
            t.redo,
            Icon::Redo,
            st.redo,
            "重做 (Ctrl+Y)",
            ToolbarAction::Redo,
        ),
        (
            t.add_child,
            Icon::AddChild,
            st.add,
            "添加子主题 (Tab)",
            ToolbarAction::AddChild,
        ),
        (
            t.add_sibling,
            Icon::AddSibling,
            st.add,
            "添加同级主题 (Enter)",
            ToolbarAction::AddSibling,
        ),
        (
            t.delete,
            Icon::Delete,
            st.delete,
            "删除选中主题 (Delete)",
            ToolbarAction::Delete,
        ),
        (
            t.card,
            Icon::Note,
            st.card,
            "新建卡片笔记",
            ToolbarAction::Card,
        ),
        (t.more, Icon::More, true, "更多操作", ToolbarAction::More),
    ];
    let mut clicked = None;
    for (rect, icon, enabled, tip, action) in items {
        let id = egui::Id::unique(("mw-canvas-tool", action.trace_name()));
        let resp = tool_button(
            ui,
            ch,
            rect,
            id,
            icon,
            ToolButtonState {
                active: false,
                enabled,
                tip,
            },
        );
        crate::trace::name(ui.ctx(), resp.id, action.trace_name());
        if enabled && resp.clicked() {
            crate::trace::event(action.trace_name());
            clicked = Some(action);
        }
    }
    clicked
}

const ZOOM_W: f32 = 154.0;
const ZOOM_BTN: f32 = 24.0;
const ZOOM_PCT_W: f32 = 48.0;
const ZOOM_FIT_W: f32 = 44.0;
const ZOOM_INNER_PAD: f32 = 2.0;
const ZOOM_Y_PAD: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ZoomBar {
    pub bar: Rect,
    pub minus: Rect,
    pub pct: Rect,
    pub plus: Rect,
    pub sep_x: f32,
    pub fit: Rect,
}

/// 右下角 16px 锚定；内部四块命中区互不重叠。
pub(crate) fn zoom_bar_layout(central: Rect) -> ZoomBar {
    let bar = Rect::from_min_size(
        Pos2::new(
            central.right() - FLOAT_PAD - ZOOM_W,
            central.bottom() - FLOAT_PAD - design::component::FLOATING_H,
        ),
        Vec2::new(ZOOM_W, design::component::FLOATING_H),
    );
    let y = bar.top() + ZOOM_Y_PAD;
    let x = bar.left() + ZOOM_INNER_PAD;
    let minus = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(ZOOM_BTN));
    let pct = Rect::from_min_size(Pos2::new(minus.right(), y), Vec2::new(ZOOM_PCT_W, ZOOM_BTN));
    let plus = Rect::from_min_size(Pos2::new(pct.right(), y), Vec2::new(ZOOM_BTN, ZOOM_BTN));
    let sep_x = plus.right() + design::space::XS;
    let fit = Rect::from_min_size(
        Pos2::new(sep_x + design::space::XS, y),
        Vec2::new(ZOOM_FIT_W, ZOOM_BTN),
    );
    ZoomBar {
        bar,
        minus,
        pct,
        plus,
        sep_x,
        fit,
    }
}

// ─────────────────────────── 缩略卡（小地图）───────────────────────────

/// 缩略卡与缩放条之间的垂直间隙：两块浮层纵向堆叠，靠右对齐。
const MAP_GAP: f32 = design::space::SM;
/// 卡内四周留白。色块贴边会糊掉边框，`Map::new` 的 `pad` 也用这一个数。
const MAP_PAD: f32 = 6.0;
/// 右上角折叠按钮（`–`）命中区边长。
///
/// 24 而不是 18：`docs/13` §4.19 记着旧版 17×17 太小、探针第一下就打偏。
/// 视觉图标仍只画 12px（`minimap_panel`），按钮变大只扩大可点区域，不压地图内容。
const MAP_BTN: f32 = 24.0;
/// 折叠按钮与卡边的距离。
const MAP_BTN_INSET: f32 = 4.0;
/// 画布小于这个尺寸就收起缩略卡：浮层不该压过主体内容（与缩放条同一条线）。
const MAP_MIN_W: f32 = 320.0;
const MAP_MIN_H: f32 = 220.0;
/// 一格的滚轮行程（逻辑 px）。Windows 鼠标滚轮一格约 24–30 px；低于它当作还没滚过一格。
const MAP_WHEEL_NOTCH: f32 = 24.0;
/// 一档缩放倍率：缩放条「+/−」与小地图滚轮共用同一档，两条路径手感必须一致。
pub(crate) const ZOOM_STEP: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Minimap {
    /// 整卡（展开态）或角标（折叠态）的屏幕矩形。这也是遮罩矩形：
    /// 卡上的点击/滚轮不能再穿透成画布手势。
    pub card: Rect,
    /// 内容区：卡去掉 [`MAP_PAD`]，世界几何等比装进这里。
    pub inner: Rect,
    /// 右上角折叠按钮。折叠态下等于整卡——点角标任何位置都展开。
    pub collapse: Rect,
    pub collapsed: bool,
}

/// 右下角、缩放条上方 8px。
///
/// 下限 320×220 与缩放条同一条线：再小的画布先保住主体内容。这两档尺寸在这条下限之上
/// 必然放得下（展开态最高的那一头距顶栏还有 34px），所以不需要额外的「放不下就不画」分支。
pub(crate) fn minimap_layout(central: Rect, collapsed: bool) -> Option<Minimap> {
    if central.width() < MAP_MIN_W || central.height() < MAP_MIN_H {
        return None;
    }
    let size = if collapsed {
        Vec2::new(mw_ui::overview::COLLAPSED_W, mw_ui::overview::COLLAPSED_H)
    } else {
        Vec2::new(mw_ui::overview::MAP_W, mw_ui::overview::MAP_H)
    };
    // 底边锚在缩放条顶边再往上 8px：两块浮层永远各占各的命中区。
    let zoom_top = central.bottom() - FLOAT_PAD - design::component::FLOATING_H;
    let card = Rect::from_min_size(
        Pos2::new(
            central.right() - FLOAT_PAD - size.x,
            zoom_top - MAP_GAP - size.y,
        ),
        size,
    );
    let collapse = if collapsed {
        card
    } else {
        Rect::from_min_size(
            card.right_top() + Vec2::new(-(MAP_BTN + MAP_BTN_INSET), MAP_BTN_INSET),
            Vec2::splat(MAP_BTN),
        )
    };
    Some(Minimap {
        inner: card.shrink(MAP_PAD),
        card,
        collapse,
        collapsed,
    })
}

/// 缩略卡滚轮 → 缩放因子：一格滚轮 = 缩放条「+/−」那一档（×1.2）。
///
/// 只认「凑满一格」：`raw_scroll_delta` 是累加的像素增量，精度触控板一甩就是上千 px，
/// 按像素连续换算会让一次甩动跳到 5.0 上限，跟用户预期的「滚一格看一眼」差得远。
/// 小于 [`MAP_WHEEL_NOTCH`] 的零头当没发生——慢推触控板本来也该是一档一档的。
pub(crate) fn minimap_wheel_factor(dy: f32) -> f32 {
    if !dy.is_finite() {
        return 1.0;
    }
    if dy.abs() < MAP_WHEEL_NOTCH {
        return 1.0;
    }
    if dy > 0.0 {
        ZOOM_STEP
    } else {
        1.0 / ZOOM_STEP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn central() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 48.0), Vec2::new(1080.0, 648.0))
    }

    #[test]
    fn toolbar_is_centered_and_its_buttons_stay_inside() {
        let t = toolbar_layout(central(), None).expect("1080 宽必须有工具条");
        assert_eq!(t.bar.width(), TOOLBAR_W);
        assert_eq!(t.bar.center().x, central().center().x);
        assert_eq!(t.bar.top(), central().top() + TOOLBAR_TOP);
        for r in [
            t.undo,
            t.redo,
            t.add_child,
            t.add_sibling,
            t.delete,
            t.card,
            t.more,
        ] {
            assert!(t.bar.contains_rect(r), "{r:?} 超出工具条 {:?}", t.bar);
        }
        assert!(t.undo.right() <= t.redo.left());
        assert!(t.redo.right() < t.add_child.left(), "组间必须留分隔线");
        assert!(t.delete.right() < t.card.left(), "组间必须留分隔线");
        assert!(t.card.right() <= t.more.left());
    }

    #[test]
    fn toolbar_drops_below_a_wide_breadcrumb() {
        let narrow = Rect::from_min_size(Pos2::new(0.0, 48.0), Vec2::new(600.0, 648.0));
        let crumbs = Rect::from_min_size(Pos2::new(12.0, 58.0), Vec2::new(300.0, 32.0));
        let t = toolbar_layout(narrow, Some(crumbs)).expect("垂直空间够");
        assert!(t.bar.top() >= crumbs.bottom() + 8.0 - 0.01);
    }

    #[test]
    fn toolbar_does_not_draw_when_the_canvas_is_too_narrow() {
        let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(280.0, 400.0));
        assert!(toolbar_layout(narrow, None).is_none());
    }

    #[test]
    fn zoom_bar_is_anchored_bottom_right_and_ordered() {
        let z = zoom_bar_layout(central());
        assert_eq!(z.bar.right(), central().right() - FLOAT_PAD);
        assert_eq!(z.bar.bottom(), central().bottom() - FLOAT_PAD);
        assert_eq!(z.bar.height(), design::component::FLOATING_H);
        for r in [z.minus, z.pct, z.plus, z.fit] {
            assert!(z.bar.contains_rect(r), "{r:?} 超出缩放条 {:?}", z.bar);
        }
        assert!(z.minus.right() <= z.pct.left());
        assert!(z.pct.right() <= z.plus.left());
        assert!(z.plus.right() < z.sep_x);
        assert!(z.sep_x < z.fit.left());
        assert!(z.fit.right() <= z.bar.right());
    }

    /// M30：缩略卡的几何——尺寸照 `docs/06` §4，位置在缩放条上方 8px，靠右对齐。
    #[test]
    fn minimap_rect_layout() {
        let c = central();
        let zoom = zoom_bar_layout(c).bar;

        let m = minimap_layout(c, false).expect("1080×648 必须有缩略卡");
        assert_eq!(
            m.card.size(),
            Vec2::new(190.0, 130.0),
            "docs/06 §4 展开态 190×130"
        );
        assert_eq!(m.card.right(), c.right() - FLOAT_PAD, "与缩放条右对齐");
        assert_eq!(
            m.card.bottom(),
            zoom.top() - MAP_GAP,
            "纵向堆叠在缩放条上方 8px"
        );
        assert_eq!(m.inner, m.card.shrink(MAP_PAD), "内容区是卡内缩 6px");
        assert!(
            m.card.contains_rect(m.collapse),
            "折叠按钮必须在卡内：{:?} vs {:?}",
            m.collapse,
            m.card
        );
        assert_eq!(m.collapse.right(), m.card.right() - MAP_BTN_INSET);
        assert_eq!(m.collapse.top(), m.card.top() + MAP_BTN_INSET);
        // 命中区 24×24：`docs/13` §4.19 的「17×17 太小」旧账。视觉图标仍只画 12px。
        assert_eq!(m.collapse.size(), Vec2::splat(24.0), "折叠按钮命中区 ≥24px");
        assert!(
            m.collapse.height() < m.card.height(),
            "按钮再大也不许吞掉整张卡"
        );

        // 折叠成角标：同一个右下角锚点，只是不再压住那么大一块画布。
        let k = minimap_layout(c, true).expect("角标也要摆得下");
        assert_eq!(
            k.card.size(),
            Vec2::new(34.0, 26.0),
            "docs/06 §4 折叠态 34×26"
        );
        assert_eq!(k.card.right(), m.card.right());
        assert_eq!(k.card.bottom(), zoom.top() - MAP_GAP);
        assert_eq!(k.collapse, k.card, "折叠态点整块角标就展开");

        // 两块浮层的命中区互不重叠，且都不出画布。
        assert!(k.card.bottom() <= zoom.top());
        assert!(!m.card.intersects(zoom));
        for r in [m.card, k.card] {
            assert!(c.contains_rect(r), "{r:?} 探出画布 {c:?}");
        }
    }

    /// M30：折叠按钮命中区 24×24（`docs/13` §4.19 的「17×17 太小」旧账）。
    /// 视觉图标仍只画 12px，按钮变大只扩大可点区域，不压缩略图内容。
    #[test]
    fn minimap_collapse_hit_area_is_at_least_24() {
        let m = minimap_layout(central(), false).expect("1080×648 必须有缩略卡");
        assert!(m.collapse.width() >= 24.0 && m.collapse.height() >= 24.0);
        assert!(
            m.collapse.width() < m.inner.width() * 0.5,
            "按钮不许吃掉半张地图：{:?} vs {:?}",
            m.collapse,
            m.inner
        );
    }

    #[test]
    fn minimap_hides_when_the_canvas_is_too_small() {
        // 与缩放条同一条下限：宽 < 320 或高 < 220 时浮层全部让位给画布。
        assert!(minimap_layout(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(319.0, 600.0)),
            false
        )
        .is_none());
        assert!(minimap_layout(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 219.0)),
            false
        )
        .is_none());
        assert!(
            minimap_layout(
                Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 219.0)),
                true
            )
            .is_none(),
            "折叠也不能绕过这条线——浮层隐去是整块隐去"
        );
        // 下限之上的最小窗：展开态底边贴着缩放条，顶边还在画布里面 34px。
        let tight = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 220.0));
        let m = minimap_layout(tight, false).expect("刚好够放");
        assert!(m.card.top() > tight.top(), "{:?}", m.card);
        assert!(m.card.left() >= tight.left());
    }

    #[test]
    fn minimap_wheel_steps_one_zoom_gear_per_notch() {
        // 一格 = 缩放条那一档；反向正好互逆。
        assert_eq!(minimap_wheel_factor(30.0), ZOOM_STEP);
        assert_eq!(minimap_wheel_factor(-30.0), 1.0 / ZOOM_STEP);
        // 没滚 / 差一点没到一格：不动。慢推触控板不该被当成连按放大。
        assert_eq!(minimap_wheel_factor(0.0), 1.0);
        assert_eq!(minimap_wheel_factor(23.9), 1.0);
        // 一次甩出上千 px 也只走一档，剩下的零头交给下一帧。
        assert_eq!(minimap_wheel_factor(4_000.0), ZOOM_STEP);
        assert_eq!(minimap_wheel_factor(f32::NAN), 1.0, "NaN 不许把相机带跑");
    }
}
