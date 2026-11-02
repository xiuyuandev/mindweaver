use super::*;

/// 悬停/选中环的过渡时长（秒）。`reduce_motion` 时直接落位。
pub(crate) const HOVER_TRANSITION: f32 = design::motion::FAST;
pub(crate) const SELECT_TRANSITION: f32 = design::motion::BASE;

/// 拖拽到画布边缘时的自动滚动：边缘 56px 内按接近程度加速，最大 900 px/s。
pub(crate) const AUTO_SCROLL_MARGIN: f32 = 56.0;
pub(crate) const AUTO_SCROLL_SPEED: f32 = 900.0;

/// 指针在 `[min,max]` 内时，边缘滚动速度（px/s，正值向右/下）。
pub(crate) fn edge_scroll_speed(pos: f32, min: f32, max: f32, margin: f32, speed: f32) -> f32 {
    if margin <= f32::EPSILON || max <= min {
        return 0.0;
    }
    let left = ((min + margin - pos) / margin).clamp(0.0, 1.0);
    let right = ((pos - (max - margin)) / margin).clamp(0.0, 1.0);
    (right - left) * speed
}

/// 以屏幕点 `at` 为锚缩放：缩放前后该点对应的世界坐标不漂。
pub(crate) fn zoom_at_anchor(camera: &mut Camera, at: Point, viewport: [f32; 2], factor: f32) {
    let anchor = camera.screen_to_world(at, viewport);
    // 下界不越过「当前值」：`fit` 可以把相机放到 `FIT_MIN_ZOOM`（20k 文档必须那么低），
    // 从那里按一下缩小键，若按交互下限 0.1 夹取会**跳大**十倍——取景与交互是两个区间。
    let floor = MIN_ZOOM.min(camera.zoom);
    camera.zoom = (camera.zoom * factor).clamp(floor, MAX_ZOOM);
    let after = camera.screen_to_world(at, viewport);
    camera.center.x += anchor.x - after.x;
    camera.center.y += anchor.y - after.y;
}

/// 一次缩放的来源。**只用来写 `zoom_anchor` 轨迹**（X21）：四条入口的数学完全一样，
/// 但验收时要能从日志里分辨「这一档是滚轮滚的、键盘按的、点按钮点的，还是小地图滚的」。
/// 没有这个枚举，缩放证据只能靠「画面变了」倒推是谁改的相机。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZoomSrc {
    /// 画布内滚轮 / Ctrl+滚轮的连续缩放。
    Wheel,
    /// 双指捏合（触控板 / 触屏）。
    Pinch,
    /// 画布快捷键 `Ctrl+=` / `Ctrl+-`。
    Key,
    /// 缩放条的 −/+ 与命令面板/菜单的放大缩小。
    Button,
    /// 小地图（缩略卡）滚轮。
    Minimap,
    /// `Ctrl+0` / 点百分比：回到 100%。
    Reset,
}

impl ZoomSrc {
    /// 轨迹里 `src=` 的取值（X21 报告与探针读的是这一份口径）。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ZoomSrc::Wheel => "wheel",
            ZoomSrc::Pinch => "pinch",
            ZoomSrc::Key => "key",
            ZoomSrc::Button => "button",
            ZoomSrc::Minimap => "minimap",
            ZoomSrc::Reset => "reset",
        }
    }
}

/// 缩放一档的**纯核**：钳位 + 锚点补偿。滚轮/双指、键盘、缩放条、小地图四条入口
/// 都从这里过（[`Canvas::zoom_step_at`](crate::canvas::Canvas::zoom_step_at) 只负责轨迹）。
///
/// `at` 是画布内的屏幕点（逻辑 px）；返回相机是否真的动了——停在上下限时不该白重绘一帧。
/// 与旧的 `zoom_around_world`（世界点锚）相比只有坐标口径不同：小地图那边先把世界点
/// 投影回画布屏幕点（`Camera::world_to_screen`），再走这里，于是**锚点式子只剩一条**
/// ——过去两条式子各自演进过一次，第二次写成乘 k，锚点跟着相机中心反向漂。
pub(crate) fn zoom_step_camera(
    camera: &mut Camera,
    at: Point,
    viewport: [f32; 2],
    factor: f32,
) -> bool {
    // 病态因子一律不动相机：一个 NaN/0/负数因子不该把取景带跑。
    if !factor.is_finite() || factor <= 0.0 {
        return false;
    }
    let before = camera.zoom;
    if viewport[0] > 1.0 && viewport[1] > 1.0 {
        zoom_at_anchor(camera, at, viewport, factor);
    } else {
        // 还没画过一帧（没有视口）：退化成纯钳位，至少把区间守住。
        // 下界同样不越过当前值（`zoom_at_anchor` 里那条纪律），否则 20k 文档 fit 到
        // 0.5% 之后按一下缩小键，会被 0.1 一把抬回二十倍。
        let floor = MIN_ZOOM.min(before);
        camera.zoom = (before * factor).clamp(floor, MAX_ZOOM);
    }
    (camera.zoom - before).abs() > f32::EPSILON
}

/// 把倍率放回 100%（`Ctrl+0` / 点百分比 / 菜单「100%」）。已在 100% 时不动。
///
/// 返回是否真的改了相机：菜单与快捷键据此决定要不要重绘。
/// `1.0` 是交互区间 `[MIN_ZOOM, MAX_ZOOM]` 的成员，这条由下面的断言钉死——
/// 于是「复位也受钳位约束」不靠读者脑补。
pub(crate) fn zoom_reset(camera: &mut Camera) -> bool {
    if (camera.zoom - 1.0).abs() <= f32::EPSILON {
        return false;
    }
    camera.zoom = 1.0;
    true
}

const _: () = assert!(MIN_ZOOM <= 1.0 && 1.0 <= MAX_ZOOM);

/// `Shift+滚轮` = 横向平移（`docs/06:149`「横向 Shift 切换轴向」）。
///
/// 不按 `Shift` 时原样返回：纵向滚动仍然纵向平移，与改动前逐像素一致。
/// 按住 `Shift` 把纵向分量搬到横轴；纵向为 0 时保留原来的横向分量
/// （真·横向滚轮不该被 `Shift` 吃掉）。放在壳层而不是靠 egui 的轴重映射：
/// egui 只认**滚轮事件自己带的**修饰键，金样脚本注入的事件里那是 `NONE`，
/// 用户手上的 `Shift` 只能从 `i.modifiers` 读。
pub(crate) fn wheel_pan(scroll: Vec2, shift: bool) -> Vec2 {
    if !shift {
        return scroll;
    }
    if scroll.y != 0.0 {
        Vec2::new(scroll.y, 0.0)
    } else {
        Vec2::new(scroll.x, 0.0)
    }
}

/// 缩放读数：缩放条与 `MW_STATS` 浮层共用这一处。
///
/// 规则：`pct = zoom × 100`，**先四舍五入到 1 位小数，再按这一位决定档位**：
/// `r >= 10` 印整数百分比，否则保留一位小数。
/// 顺序不能反：先判档位再四舍五入，`9.96` 会印成 `10.0%`、`10.04` 印成 `10%`，
/// 同一档位两种宽度，百分比块每次进位都跳一下。一位小数只为一档存在——
/// 20k 文档 fit 下来是 0.5%，用整数位会显示成 `0%`，看起来像相机坏了。
/// 非有限值（某个 bug 把 `zoom` 写成 NaN/inf）印破折号：`NaN%` 只会让日志更难读。
///
/// 整数档**印 `pct` 而不是 `r`**：`r` 已经是 0.1 网格上的值，再对它取 0 位是
/// 二次舍入——fit 的 `0.85450012` 会先变成 `85.5` 再进成 `86%`，而真值是 85.45%，
/// 该读出 85%。`9.96` 那一侧不受影响：`r = 10.0` 已经 ≥10，走整数档印 `9.96` 的
/// 0 位仍然是 `10%`。两个档位的判据用 `r`、墨迹用 `pct`，两次舍入就不会叠起来。
pub(crate) fn format_zoom(zoom: f32) -> String {
    let pct = zoom * 100.0;
    if !pct.is_finite() {
        return "—".to_string();
    }
    let r = (pct * 10.0).round() / 10.0;
    if r >= 10.0 {
        format!("{pct:.0}%")
    } else {
        format!("{r:.1}%")
    }
}

/// 透明度向目标推进一帧；线性就够，环不是弹性动画。
pub(crate) fn advance_alpha(current: f32, target: f32, dt: f32, seconds: f32, reduce: bool) -> f32 {
    if reduce || seconds <= f32::EPSILON {
        return target;
    }
    let step = (dt / seconds).clamp(0.0, 1.0);
    if target > current {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

/// 交互缩放区间：10%–500%（合同口径）。
///
/// 与 `FIT_MIN_ZOOM` 分工：这里管「用户手能按到的范围」，那边管「适应视图要把全图装进来」。
/// 一个 20k 节点的文档适应下来只有 2%，那是取景结果，不是用户可按的档位。
pub(crate) const MIN_ZOOM: f32 = 0.1;
pub(crate) const MAX_ZOOM: f32 = 5.0;
/// 首次铺满时的世界留白（CSS px）。
pub(crate) const FIT_PAD: f32 = 56.0;
/// 5k 节点在 1600×900 下要看到 80% 约需 1%（§4.11），所以适配下限必须放到那么低。
pub(crate) const FIT_MIN_ZOOM: f32 = 0.005;
pub(crate) const FIT_MAX_ZOOM: f32 = 1.5;
/// 开始编辑时的最小屏幕字号：低于它先把画布放大，否则 IME 候选窗和 caret 都无从谈起。
pub(crate) const EDIT_MIN_PX: f32 = 13.0;
/// 节点圆角的世界单位，与 `mw_render::instances::NODE_RADIUS` 共用同一令牌。
pub(crate) const NODE_RADIUS: f32 = design::radius::MD;
/// 框选阈值（docs/06 §5）：位移超过它才算「拉框」，否则松手仍是「单击空白清空选择」。
///
/// 必须自己判，不能指望 egui 的 `max_click_dist`（6px）：两者不等时，位移落在
/// 4–6px 之间会「画了橡皮筋仍报单击」，把刚框出来的组立刻清掉。
pub(crate) const DRAG_THRESHOLD: f32 = 4.0;
/// 平移死区。空格/中键按下后先吃掉几像素抖动，避免「想按住不动」时画布漂移。
pub(crate) const PAN_DEADZONE: f32 = 3.0;
/// 吸附延迟（秒，docs/06:125）：候选对齐线要稳定存在这么久才真吸。
pub(crate) const SNAP_DELAY: f64 = 0.06;
/// 关系线改端点时的**吸附**距离（CSS px，屏幕空间恒定）。
///
/// 12 的来历：透镜实验里手抖的落点通常差 4–10px，而概念盒之间最小间距是 40+，
/// 取 12 既能把「差几个像素」的意图读成「改到这颗」，又不会把空白处的刻意落空读错
/// （打磨轮 12，`cm-relink-near` 金样钉住）。
pub(crate) const LINK_SNAP_CSS: f32 = 12.0;
/// 关系线锚点的视觉半径 / 命中半径（CSS px，屏幕空间恒定）。
pub(crate) const ANCHOR_R: f32 = 3.2;
pub(crate) const ANCHOR_HIT: f32 = 7.0;
/// 注解跨度手柄：屏幕空间的小圆点，命中半径比视觉半径宽 4.5px。
pub(crate) const SPAN_HANDLE_R: f32 = 4.5;
pub(crate) const SPAN_HANDLE_HIT: f32 = 9.0;
/// 悬停工具条的按钮边长（CSS px，屏幕空间恒定）。24 与缩放控件的图标节奏一致，
/// 22 在 150% 以上的画布上显得像误触留下的小点。
pub(crate) const HOVER_BTN: f32 = design::control::HIT;
/// 两个按钮之间的间距：落在 4px 网格上，与工具条本身的圆角节奏一致。
pub(crate) const HOVER_BTN_GAP: f32 = design::space::XS;
/// 工具条离节点盒的距离。必须**大于** `mw_ui::drag::DROP_PAD_X`（6）：
/// 命中区一旦压住节点盒，点按钮就会被画布当成「按在节点上」，顺带起一次拖拽。
pub(crate) const HOVER_GAP: f32 = design::space::SM;
/// 工具条与节点上/下缘的垂直间距。必须让开右侧中线的折叠圆环（r8 + 1.5 halo）。
pub(crate) const HOVER_GAP_V: f32 = design::space::XS;
/// 工具条与节点之间的缝隙必须大于拖拽容差，否则「点按钮」和「按住节点拖」的
/// 命中区会重叠。这条关系用编译期断言钉死，不靠注释提醒。
const _: () = assert!(HOVER_GAP > mw_ui::drag::DROP_PAD_X);

/// 概念图方向键微调的步长（世界 px）：素键 1px，`Shift` 8px（`docs/30` §5.7）。
/// 概念图没有「上/下一个节点」这种遍历语义——方向键在那儿是**摆位**。
pub(crate) const CONCEPT_NUDGE: f32 = 1.0;
pub(crate) const CONCEPT_NUDGE_SHIFT: f32 = 8.0;
/// 概念图新建概念的占位文字。概念是名词短语，「自由主题」是树的口径。
pub(crate) const CONCEPT_PLACEHOLDER: &str = "新概念";

/// 方向键的实际步长。抽成纯函数，好让「1px / 8px」这条被单测钉住。
pub(crate) fn nudge_step(shift: bool) -> f32 {
    if shift {
        CONCEPT_NUDGE_SHIFT
    } else {
        CONCEPT_NUDGE
    }
}
