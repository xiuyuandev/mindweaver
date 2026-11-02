//! 折叠圆点的**双击识别**（M32：双击「展开到下一级」）。
//!
//! 壳层不能直接用 egui 的 `double_clicked()` 判这一下：圆点的第二击同时也会
//! 触发单击路径（把节点折起来），而双击要的是「从第一击之前的状态往下走一层」。
//! 所以壳层在第一次点击时记一份现场（节点 id / 时间 / 位置 / 落地后的撤销深度），
//! 第二次点击到达时先在这里问一句「它是不是同一处的双击」，再决定
//! 「撤销第一击 + 下发下钻命令」还是按普通单击处理。
//!
//! 三条判据都必须是**屏幕常量**、且时间来自 egui 的虚拟时钟（金样按帧推进）：
//! 同一节点、窗口内、位移不超过 4 css px；另外要求撤销深度没变——
//! 两次点击之间若别的操作落地，就不能把别人的命令当成第一击撤销掉。
//!
//! 本模块零 UI 依赖，纯逻辑可单测（`mw-ui` 的规矩）。

/// 屏幕坐标（css px），不依赖 egui / 窗口。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClickPoint {
    pub x: f32,
    pub y: f32,
}

impl ClickPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn distance_to(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// 一次折叠圆点点击的现场。
#[derive(Clone, Debug, PartialEq)]
pub struct ToggleClick {
    /// 被点的节点 id（不是场景下标——重建会重排下标）。
    pub id: String,
    /// 调用方给的单调时间（壳层传 `ui.input(|i| i.time)`）。
    pub at: f64,
    /// 指针位置（css px）。
    pub pos: ClickPoint,
    /// 这一击**之前**节点的折叠态。双击时按它决定下钻方向。
    pub collapsed_before: bool,
    /// 这一击的命令**落地之后**的撤销深度。两次点击之间别的命令会让它变化，此时不算双击。
    pub undo_depth: usize,
    /// 这一击是否真的往撤销栈里推了一条命令。
    ///
    /// 双击时要先撤掉第一击再按第一击之前的状态下钻；若第一击是空操作（没推命令），
    /// 就不能去撤销栈里拿别人那条。
    pub pushed: bool,
}

/// 双击判据（默认值就是 `docs/02` §3 的口径：500ms / 4 css px）。
#[derive(Clone, Copy, Debug)]
pub struct DrillClickConfig {
    pub window_s: f64,
    pub slop_css: f32,
}

impl Default for DrillClickConfig {
    fn default() -> Self {
        Self {
            window_s: 0.5,
            slop_css: 4.0,
        }
    }
}

/// 折叠圆点点击记忆：记住上一次点击，下一次点击到来时判定是否构成双击。
///
/// 用法是两段式：`is_double(&now)` 只**判断**不消费；命中后 `take()` 取走第一击；
/// 未命中则先执行单击，再把「落地后的」现场交给 `remember()`。
#[derive(Clone, Debug, Default)]
pub struct ToggleClickMemory {
    last: Option<ToggleClick>,
    config: DrillClickConfig,
}

impl ToggleClickMemory {
    pub fn with_config(config: DrillClickConfig) -> Self {
        Self { last: None, config }
    }

    pub fn clear(&mut self) {
        self.last = None;
    }

    pub fn pending(&self) -> Option<&ToggleClick> {
        self.last.as_ref()
    }

    /// 这次点击与上一次是否构成双击。
    ///
    /// 只有「同一节点 + 窗口内 + 位移不超限 + 撤销深度没变」才算；
    /// 任何一条不满足都不算，避免把两次无关点击串成下钻。
    pub fn is_double(&self, click: &ToggleClick) -> bool {
        let Some(prev) = self.last.as_ref() else {
            return false;
        };
        let dt = click.at - prev.at;
        click.id == prev.id
            && dt >= 0.0
            && dt <= self.config.window_s
            && click.pos.distance_to(prev.pos) <= self.config.slop_css
            && click.undo_depth == prev.undo_depth
    }

    /// 取走已登记的第一击（双击成立后调用）。
    pub fn take(&mut self) -> Option<ToggleClick> {
        self.last.take()
    }

    /// 登记一次点击（非双击时调用）。
    pub fn remember(&mut self, click: ToggleClick) {
        self.last = Some(click);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(id: &str, at: f64, x: f32, y: f32, depth: usize) -> ToggleClick {
        ToggleClick {
            id: id.to_string(),
            at,
            pos: ClickPoint::new(x, y),
            collapsed_before: false,
            undo_depth: depth,
            pushed: true,
        }
    }

    fn remembered(id: &str, at: f64, depth: usize) -> ToggleClickMemory {
        let mut m = ToggleClickMemory::default();
        m.remember(click(id, at, 522.0, 429.0, depth));
        m
    }

    #[test]
    fn two_clicks_on_the_same_dot_within_the_window_are_a_double_click() {
        let mut m = remembered("A", 1.0, 8);
        assert!(m.is_double(&click("A", 1.2, 523.0, 429.5, 8)), "应判双击");
        let first = m.take().expect("返回第一击的现场");
        assert_eq!(first.at, 1.0);
        assert!(m.pending().is_none(), "双击后记忆清空");
    }

    #[test]
    fn a_third_click_does_not_repeat_the_drill() {
        let mut m = remembered("A", 1.0, 8);
        assert!(m.is_double(&click("A", 1.1, 522.0, 429.0, 8)));
        m.take();
        assert!(
            !m.is_double(&click("A", 1.2, 522.0, 429.0, 8)),
            "第三击重新开始计数，不连着下钻两次"
        );
    }

    #[test]
    fn a_different_node_restarts_the_clock() {
        let m = remembered("A", 1.0, 8);
        assert!(!m.is_double(&click("B", 1.1, 522.0, 429.0, 8)));
    }

    #[test]
    fn clicks_outside_the_window_are_not_a_double_click() {
        let m = remembered("A", 1.0, 8);
        assert!(!m.is_double(&click("A", 1.6, 522.0, 429.0, 8)));
    }

    #[test]
    fn moving_more_than_four_css_pixels_is_not_a_double_click() {
        let m = remembered("A", 1.0, 8);
        assert!(!m.is_double(&click("A", 1.1, 527.0, 429.0, 8)), "5px 超限");
        assert!(m.is_double(&click("A", 1.1, 525.9, 429.0, 8)), "4px 内仍算");
    }

    #[test]
    fn a_command_between_the_two_clicks_cancels_the_drill() {
        let m = remembered("A", 1.0, 8);
        // 两次点击之间撤销深度变了（别的命令落地）——不能把别人的命令当成第一击撤销。
        assert!(!m.is_double(&click("A", 1.1, 522.0, 429.0, 9)));
    }
}
