//! 面板焦点身份与开合恢复（H3）：让「面板一关一开，键盘焦点回到原处」成立。
//!
//! 两个成因叠在一起才会丢焦点：
//! 1. egui 的自动 id 由控件在 UI 树里的**序号**派生（`next_auto_id_salt`）。面板里
//!    新增 / 移除一个控件，后面所有控件的序号一起挪，id 跟着变；
//! 2. 面板关掉之后控件整个不再绘制，egui 的「死人开关」发现上一帧被聚焦的 id 这一帧
//!    没人认领，直接把焦点清零。
//!
//! 前者靠 [`FocusKey`] 解决——给需要保持焦点的控件一套**与位置、序号无关**的稳定 id；
//! 后者靠 [`FocusMemory`] 解决——记住「关之前谁握着焦点」，重开那一帧把焦点请求回去。
//!
//! 这不是新功能，是把「光标回到原处」这件本该成立的事补齐。

use std::collections::HashMap;

use eframe::egui;

use crate::trace;

/// 面板标识：语义键的第一段，也是焦点记忆的分桶键。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PanelId {
    Outline,
    Inspector,
}

/// 面板内可聚焦控件的**稳定身份**。
///
/// `base` 是语义键（`<panel>/<region>/<role>`，如 `inspector/style/branch/swatch`）；
/// `tail` 用于同一角色下的多格控件（第 N 个色点、某个大纲行）。
///
/// 与设计里 `FocusKey(&'static str)` 的差异：色点序号与大纲行 id 都是运行期才知道的，
/// 单个 `&'static str` 装不下，所以加了 `tail` 一段。id 的派生方式不变，仍是
/// `Id::unique(("mw-focus", base, tail))`，**不掺布局位置**。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FocusKey {
    base: &'static str,
    tail: Option<String>,
}

impl FocusKey {
    /// 整件控件一个身份（无序号）。
    pub fn new(base: &'static str) -> Self {
        Self { base, tail: None }
    }

    /// 同一角色下的第 `index` 格（一排色板的第几个色点）。
    pub fn indexed(base: &'static str, index: usize) -> Self {
        Self {
            base,
            tail: Some(index.to_string()),
        }
    }

    /// 同一角色下的动态区分（大纲行 / 折叠箭头的节点 id）。
    pub fn dynamic(base: &'static str, tail: impl Into<String>) -> Self {
        Self {
            base,
            tail: Some(tail.into()),
        }
    }

    pub fn base(&self) -> &'static str {
        self.base
    }

    /// 与布局位置、控件序号都无关的稳定 id。
    ///
    /// 用 `Id::unique` 而不是挂在某个父 id 下加盐：这套键本身就是全局命名空间，
    /// `("mw-focus", …)` 前缀保证不与 egui 自动 id 撞车。
    pub fn id(&self) -> egui::Id {
        egui::Id::unique(("mw-focus", self.base, self.tail.as_deref()))
    }

    /// 轨迹与语义回落用的名字。
    pub fn trace_name(&self) -> String {
        match &self.tail {
            Some(t) => format!("{} {t}", self.base),
            None => self.base.to_string(),
        }
    }
}

/// 分配控件 id 并把语义名挂给 `MW_TRACE`（未设环境变量时只是一次原子读）。
///
/// 面板里凡是需要保持焦点的控件都该从这里拿 id：`ui.allocate_space` 占位、
/// `ui.interact(rect, widget_id(..), Sense::click())` 登记——与 `allocate_exact_size`
/// 完全等价，只是 id 由我们钉死。
pub fn widget_id(ui: &egui::Ui, key: &FocusKey) -> egui::Id {
    let id = key.id();
    if trace::is_enabled() {
        trace::name(ui.ctx(), id, &key.trace_name());
    }
    id
}

#[derive(Default)]
struct Entry {
    /// 上一次离开该面板时，谁握着焦点。
    last: Option<FocusKey>,
    /// 本帧该面板里出现过、且可聚焦的键。用来判断「目标还在不在」，以及挑语义最近的回落项。
    known: Vec<FocusKey>,
    /// `known` 的去重索引：20k 行时线性 `contains` 会退化成 O(n²)。
    seen: std::collections::HashSet<FocusKey>,
    /// 面板当前是否处于打开态（壳层每帧告知），用于识别「由关到开」那一帧。
    open: bool,
}

/// 「面板 → 上次焦点键」的记忆：随 `Shell` 生命周期长期存活，**不随面板关闭清空**
/// （清空就没得恢复了）。
#[derive(Default)]
pub struct FocusMemory {
    entries: HashMap<PanelId, Entry>,
}

impl FocusMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// 每帧面板开始绘制前调用一次：清空「本帧已知键」。
    pub fn begin_frame(&mut self, panel: PanelId) {
        let e = self.entries.entry(panel).or_default();
        e.known.clear();
        e.seen.clear();
    }

    /// 面板里一个可聚焦控件登记自己：进「本帧已知」，若正握着焦点则记进 `last`。
    pub fn note(&mut self, panel: PanelId, ctx: &egui::Context, key: &FocusKey) {
        let focused = ctx.memory(|m| m.focused()) == Some(key.id());
        let e = self.entries.entry(panel).or_default();
        if e.seen.insert(key.clone()) {
            e.known.push(key.clone());
        }
        if focused {
            e.last = Some(key.clone());
        }
    }

    /// 批量登记一帧的键（`begin_frame` + 逐个 `note`）。
    ///
    /// 生产路径里键是在绘制时逐个产生的，面板用 `note` 就地登记；
    /// 这把「整批键一次记完」的糖留给无窗口测试（任务 1.3：`observe` 后 `restore`
    /// 回到同一语义键），所以只在测试构建里被调用。
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn observe(&mut self, panel: PanelId, ctx: &egui::Context, keys: &[FocusKey]) {
        self.begin_frame(panel);
        for k in keys {
            self.note(panel, ctx, k);
        }
    }

    /// 告知面板开合状态；返回是否发生了「由关到开」。
    pub fn set_open(&mut self, panel: PanelId, open: bool) -> bool {
        let e = self.entries.entry(panel).or_default();
        let just_opened = open && !e.open;
        e.open = open;
        just_opened
    }

    /// 面板「由关到开」那一帧调用：把焦点请求回原控件；目标不在则语义回落。
    ///
    /// **绝不**退到面板第一项（5.3.9）：目标既不在、又没有语义近邻时，只在 `fallback`
    /// 确实存在时才聚焦它；`fallback` 也不在就不强行聚焦——那正是「重开聚焦第一项」。
    pub fn restore(&mut self, panel: PanelId, ctx: &egui::Context, fallback: FocusKey) {
        let Some(e) = self.entries.get(&panel) else {
            return;
        };
        if let Some(last) = e.last.clone() {
            if e.known.contains(&last) {
                ctx.memory_mut(|m| m.request_focus(last.id()));
                return;
            }
            if let Some(near) = semantic_fallback(&last, &e.known) {
                ctx.memory_mut(|m| m.request_focus(near.id()));
                return;
            }
        }
        if e.known.contains(&fallback) {
            ctx.memory_mut(|m| m.request_focus(fallback.id()));
        }
    }

    /// 上一次记住的焦点键（测试与轨迹用）。
    pub fn last(&self, panel: PanelId) -> Option<FocusKey> {
        self.entries.get(&panel).and_then(|e| e.last.clone())
    }

    /// 在本帧已登记的可聚焦控件里按创建顺序移动焦点（环形）。
    ///
    /// `delta > 0` 向后、`< 0` 向前；当前没有焦点时，向后落第一个、向前落最后一个。
    /// 返回是否真的移动了（面板没有控件时返回 false）。
    pub fn step_focus(&mut self, panel: PanelId, ctx: &egui::Context, delta: isize) -> bool {
        let Some(e) = self.entries.get(&panel) else {
            return false;
        };
        if e.known.is_empty() || delta == 0 {
            return false;
        }
        let current = ctx.memory(|m| m.focused());
        let idx = current.and_then(|id| e.known.iter().position(|k| k.id() == id));
        let next = match idx {
            Some(i) => (i as isize + delta).rem_euclid(e.known.len() as isize) as usize,
            None if delta > 0 => 0,
            None => e.known.len() - 1,
        };
        let id = e.known[next].id();
        if crate::trace::is_enabled() {
            crate::trace::event(&format!("focus-step {}", e.known[next].trace_name()));
        }
        ctx.memory_mut(|m| m.request_focus(id));
        true
    }
}

/// 语义最近的可用键：与目标共享最多**前缀段**（按 `/` 切）的那个（目标自身除外）。
///
/// 要求至少共享两段（`panel/region`）——同一面板的标题、「关闭」按钮这类控件只共享
/// `inspector` 一段，不该被当成近邻；`inspector/style/branch/swatch` 的兄弟格共享
/// `inspector/style/branch` 三段，才算「最近」。
fn semantic_fallback(target: &FocusKey, known: &[FocusKey]) -> Option<FocusKey> {
    let tid = target.id();
    let tname = target.trace_name();
    let mut best: Option<(usize, FocusKey)> = None;
    for k in known {
        if k.id() == tid {
            continue;
        }
        let n = shared_prefix_segments(&tname, &k.trace_name());
        if n < 2 {
            continue;
        }
        if best.as_ref().is_none_or(|(bn, _)| n > *bn) {
            best = Some((n, k.clone()));
        }
    }
    best.map(|(_, k)| k)
}

/// 两个语义名按 `/` 切段后，从头数起相同的段数。
fn shared_prefix_segments(a: &str, b: &str) -> usize {
    a.split('/')
        .zip(b.split('/'))
        .take_while(|(x, y)| x == y)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILL: &str = "inspector/style/fill/swatch";
    const BRANCH: &str = "inspector/style/branch/swatch";
    const NOTE_FIELD: &str = "inspector/note/field";
    const CLOSE: &str = "inspector/close";
    /// 面板锚点（检查器里是「分支线首格」）。大纲的锚点是虚拟键，故意不在 `known` 里。
    const ANCHOR: &str = "inspector/anchor";

    fn keys(spec: &[(&'static str, Option<usize>)]) -> Vec<FocusKey> {
        spec.iter()
            .map(|(b, i)| match i {
                Some(i) => FocusKey::indexed(b, *i),
                None => FocusKey::new(b),
            })
            .collect()
    }

    /// 跑一帧：把 `keys` 全部登记成可聚焦控件，其中 `focus` 那一格请求焦点。
    /// 帧末用 [`FocusMemory::observe`] 整批登记（`observe=false` 时只画不记，
    /// 模拟"面板已经关掉不再观察"）——焦点状态留在 `ctx` 里，帧末读与绘制中读等价。
    fn frame(
        ctx: &egui::Context,
        mem: &mut FocusMemory,
        panel: PanelId,
        keys: &[FocusKey],
        focus: Option<&FocusKey>,
        observe: bool,
    ) {
        ctx.run_ui(egui::RawInput::default(), |ui| {
            for (i, k) in keys.iter().enumerate() {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(0.0, i as f32 * 24.0),
                    egui::vec2(20.0, 20.0),
                );
                let resp = ui.interact(rect, widget_id(ui, k), egui::Sense::click());
                if focus == Some(k) {
                    resp.request_focus();
                }
            }
        })
        .drop_without_applying_deltas();
        if observe {
            mem.observe(panel, ctx, keys);
        }
    }

    #[test]
    fn arrow_navigation_moves_through_known_keys_and_wraps() {
        let ctx = egui::Context::default();
        let mut mem = FocusMemory::new();
        let all = keys(&[
            (FILL, Some(0)),
            (BRANCH, Some(0)),
            (BRANCH, Some(1)),
            (NOTE_FIELD, None),
        ]);
        frame(
            &ctx,
            &mut mem,
            PanelId::Inspector,
            &all,
            Some(&all[1]),
            true,
        );
        assert!(mem.step_focus(PanelId::Inspector, &ctx, 1));
        assert_eq!(ctx.memory(|m| m.focused()), Some(all[2].id()));
        assert!(mem.step_focus(PanelId::Inspector, &ctx, 1));
        assert_eq!(ctx.memory(|m| m.focused()), Some(all[3].id()));
        // 末项再向后 → 回第一项
        assert!(mem.step_focus(PanelId::Inspector, &ctx, 1));
        assert_eq!(ctx.memory(|m| m.focused()), Some(all[0].id()));
        // 向前 → 回末项
        assert!(mem.step_focus(PanelId::Inspector, &ctx, -1));
        assert_eq!(ctx.memory(|m| m.focused()), Some(all[3].id()));
    }

    #[test]
    fn step_focus_without_a_current_item_lands_on_an_end() {
        let ctx = egui::Context::default();
        let mut mem = FocusMemory::new();
        let all = keys(&[(FILL, Some(0)), (BRANCH, Some(0))]);
        frame(&ctx, &mut mem, PanelId::Inspector, &all, None, true);
        assert!(mem.step_focus(PanelId::Inspector, &ctx, 1));
        assert_eq!(ctx.memory(|m| m.focused()), Some(all[0].id()));
    }

    #[test]
    fn reopening_a_panel_returns_focus_to_the_same_control() {
        let ctx = egui::Context::default();
        let mut mem = FocusMemory::new();
        let all = keys(&[(FILL, Some(0)), (BRANCH, Some(0)), (BRANCH, Some(1))]);

        // 帧 1：用户把焦点移到「分支线」第二个色点。
        frame(
            &ctx,
            &mut mem,
            PanelId::Inspector,
            &all,
            Some(&all[2]),
            true,
        );
        assert_eq!(mem.last(PanelId::Inspector), Some(all[2].clone()));

        // 帧 2：面板被关掉（不再观察），焦点随之清零。
        assert!(
            !mem.set_open(PanelId::Inspector, false),
            "关闭不是「由关到开」"
        );
        frame(&ctx, &mut mem, PanelId::Inspector, &[], None, false);

        // 帧 3：重开——先画本帧控件并登记，再把焦点恢复到原处。
        // 顺序不能反：这一帧到底画了哪些键，只有画完才知道；拿上一帧的键去
        // `request_focus`，目标已删时会被 egui 的死人开关清掉。
        assert!(mem.set_open(PanelId::Inspector, true), "由关到开");
        frame(&ctx, &mut mem, PanelId::Inspector, &all, None, true);
        mem.restore(PanelId::Inspector, &ctx, FocusKey::new(CLOSE));

        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(all[2].id()),
            "开合后焦点必须回到同一格"
        );
    }

    #[test]
    fn a_removed_control_falls_back_to_its_nearest_sibling() {
        let ctx = egui::Context::default();
        let mut mem = FocusMemory::new();
        let before = keys(&[(FILL, Some(0)), (BRANCH, Some(0)), (BRANCH, Some(1))]);
        frame(
            &ctx,
            &mut mem,
            PanelId::Inspector,
            &before,
            Some(&before[2]),
            true,
        );

        // 文档变化把「分支线」第二格删了：只剩填充与分支线首格。
        let after = keys(&[(FILL, Some(0)), (BRANCH, Some(0))]);
        frame(&ctx, &mut mem, PanelId::Inspector, &after, None, true);
        mem.restore(PanelId::Inspector, &ctx, FocusKey::new(CLOSE));

        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(after[1].id()),
            "回落到同排的兄弟色点，而不是落空、也不是面板首项"
        );
    }

    /// 5.3.9：焦点原先不在首项，目标又整个消失时，**不得**重置到面板第一项。
    #[test]
    fn a_lost_target_never_resets_focus_to_the_first_item() {
        let ctx = egui::Context::default();
        let mut mem = FocusMemory::new();
        let before = keys(&[(BRANCH, Some(3)), (CLOSE, None)]);
        frame(
            &ctx,
            &mut mem,
            PanelId::Inspector,
            &before,
            Some(&before[0]),
            true,
        );

        // 重开时只剩「关闭」这一个控件（面板首项）。锚点键是面板没画出来的虚拟键，
        // 于是 fallback 也不存在——此时**不请求焦点**，而不是退回首项。
        let after = keys(&[(CLOSE, None)]);
        frame(&ctx, &mut mem, PanelId::Inspector, &after, None, true);
        mem.restore(PanelId::Inspector, &ctx, FocusKey::new(ANCHOR));

        assert_ne!(
            ctx.memory(|m| m.focused()),
            Some(after[0].id()),
            "目标消失不等于「聚焦第一项」"
        );
    }

    /// 回退链第三级：目标与其语义近邻都不在（跨页签，只共享 `inspector` 一段），
    /// 但面板锚点（分支线首格）在——焦点落在锚点上，仍不是面板首项。
    #[test]
    fn a_missing_target_settles_on_the_panel_anchor() {
        let ctx = egui::Context::default();
        let mut mem = FocusMemory::new();
        let before = keys(&[(NOTE_FIELD, None), (CLOSE, None)]);
        frame(
            &ctx,
            &mut mem,
            PanelId::Inspector,
            &before,
            Some(&before[0]),
            true,
        );

        let after = keys(&[(CLOSE, None), (BRANCH, Some(0))]);
        frame(&ctx, &mut mem, PanelId::Inspector, &after, None, true);
        mem.restore(PanelId::Inspector, &ctx, FocusKey::indexed(BRANCH, 0));

        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(after[1].id()),
            "近邻不在时落到面板锚点（分支线首格）"
        );
        assert_ne!(
            ctx.memory(|m| m.focused()),
            Some(after[0].id()),
            "仍然不是首项"
        );
    }

    /// 5.3.2：同一面板里其它控件增删，不该改变已有键的身份与焦点归属。
    #[test]
    fn adding_other_controls_leaves_the_remembered_key_alone() {
        let ctx = egui::Context::default();
        let mut mem = FocusMemory::new();
        let branch = FocusKey::indexed(BRANCH, 1);

        let narrow = keys(&[(FILL, Some(0)), (BRANCH, Some(1))]);
        frame(
            &ctx,
            &mut mem,
            PanelId::Inspector,
            &narrow,
            Some(&branch),
            true,
        );
        assert_eq!(mem.last(PanelId::Inspector), Some(branch.clone()));

        // 前面插了一整排新控件：旧实现里 id 序号被顶走，焦点就丢了。
        let wide = keys(&[(FILL, Some(0)), (FILL, Some(1)), (BRANCH, Some(1))]);
        frame(
            &ctx,
            &mut mem,
            PanelId::Inspector,
            &wide,
            Some(&branch),
            true,
        );
        assert_eq!(mem.last(PanelId::Inspector), Some(branch.clone()));
        assert_eq!(mem.last(PanelId::Inspector).unwrap().id(), branch.id());
    }
}
