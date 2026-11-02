//! 概念图的「整理布局」与「力导向」：**纯几何 + 纯图论**，不认识文档、命令或 egui。
//!
//! 概念图不是树（允许多根、交叉、成环），所以不能复用 `layout.rs` 的树布局。
//! 这里做的是两套路子：
//!
//! - [`arrange`] / [`arrange_scoped`]：分层摆放，把命题网压成「层 × 层内顺序」，再给每层一个列位
//!   （`arrange_scoped` 的 `fixed` 支持「只整理选中 / 钉住不动」）。
//! - [`flow`]：确定性力导向（弹簧 + 近距斥力），环不截断，收尾二维压开。
//!
//! 分层整理的五步（`docs/30` §5.5）：
//!
//! 1. **分层**：先丢掉「指向正在访问节点」的回边（环按首次访问截断），
//!    再对剩下的边做最长路径松弛，保证 `layer(to) ≥ layer(from) + 1`。
//! 2. **排序**：层内先按当前位置排，再按「入边父重心」稳定重排几趟。
//! 3. **初摆**：列宽取该层最宽概念，列间 `LAYER_GAP`；层内自上而下等距 `NODE_GAP`。
//! 4. **松弛**：每个概念朝邻居中心均值靠一半，然后层内压开、整块不越上边界。
//! 5. **收口**：列位与行位合成，整体左对齐到原包围盒左上角。
//!
//! 确定性来自两条：集合一律走 `Vec` + 下标兜底比较，不碰 `HashMap` 迭代顺序；
//! 每个排序都以 `(主键, 下标)` 双键收尾。

/// 层间横向间距下限（`docs/30` §5.5）。
pub const LAYER_GAP: f32 = 56.0;
/// 层内纵向间距下限。
pub const NODE_GAP: f32 = 24.0;
/// 重心松弛轮数。
pub const RELAX_ROUNDS: usize = 24;

// ---- 平行命题的短语避让（C21，`docs/30` §5.4）----

/// 相邻两条车道的短语胶囊之间必须留下的净空下限（世界单位）。
pub const PROPOSITION_LABEL_CLEARANCE: f32 = 24.0;
/// 短语胶囊高（与 `mw-render::instances::REL_LABEL_H` 同口径；`mw-core` 不依赖渲染层）。
pub const PROPOSITION_LABEL_HEIGHT: f32 = 20.0;
/// 短语胶囊最小宽（与 `mw-render::instances::REL_LABEL_MIN_W` 同口径）。
pub const PROPOSITION_LABEL_MIN_W: f32 = 24.0;
/// 两条平行命题沿切线错开的**旧步长**（C15 的常量）。
pub const PROPOSITION_LABEL_LEGACY_STEP: f32 = 24.0;

/// 同一对概念之间多条平行命题的**短语切线偏移**（世界单位，C21 → `docs/33` §2 B2 R1）。
///
/// `label_widths` 按车道组内顺序给出每条短语的胶囊宽（文本宽 + 12），
/// `tangent` 是规范方向（小索引 → 大索引）的向量（不必预先归一）。
/// 返回值与输入等长：`offset[k]` 沿 `tangent` 的单位向量加在该车道曲线的中点上。
///
/// - 1 条：`[0]` —— 短语落回曲线中点（单条命题的既有几何，基线逐字节不动）。
/// - 2 条：`[-12, 12]` —— 与 C15 的常量公式**逐位一致**（`cm-parallel-select-a/b`
///   锁的就是这组坐标，不许漂）。
/// - ≥3 条：**等距对称带**——步长 `2·max(投影半宽) + 净空`，让整条带以中车道为 0
///   对称展开（奇数车道的中车道严格为零偏移）。相邻两盒的投影净空
///   `step − span[i] − span[i+1] ≥ 2·max + 24 − 2·max = 24`，C21 的不重叠验收仍成立。
///
/// **为什么不再按每条自己的字宽分别排**（`docs/33` §2 B2 R1）：那会让
/// 「改一条短语」把同组其它短语一起挪走——用户在概念图里编辑连接短语时
/// 看到的是「文字在漂」。改成只随**组内最宽那条**变之后：
/// - 改的那条不是最宽 → 整条带一动不动（最常见的情况，零漂移）；
/// - 改成了最宽 → 整条带以中点为轴对称撑开一次，肉眼几乎无感。
///
/// 位置只由车道序号 + 切线方向 + 组内最宽决定，字宽不再逐条进公式。
///
/// 为什么按切线投影算最宽那条：短语盒是轴对齐矩形，它在单位向量 `u` 上的
/// 投影半宽恰是 `(w/2)|ux| + (h/2)|uy|`；车道的法向偏移垂直于 `u`，
/// 不改变投影，所以曲线本身（`scene.rs::bowed_straight`）一点都不用动。
pub fn proposition_label_offsets(label_widths: &[f32], tangent: (f32, f32)) -> Vec<f32> {
    let n = label_widths.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0.0];
    }
    if n == 2 {
        // C15 的旧公式原样保留，连运算顺序都不改：`step = (k - (n-1)/2) * 24`。
        let legacy = |k: f32| (k - (2.0f32 - 1.0) * 0.5) * PROPOSITION_LABEL_LEGACY_STEP;
        return vec![legacy(0.0), legacy(1.0)];
    }
    let (tx, ty) = tangent;
    let len = (tx * tx + ty * ty).sqrt();
    let (ux, uy) = if len > 1e-6 {
        (tx / len, ty / len)
    } else {
        (1.0, 0.0)
    };
    let half_h = PROPOSITION_LABEL_HEIGHT * 0.5;
    // 组内最宽那条的投影半宽：整条带的步长只认这一个数。
    let max_span = label_widths
        .iter()
        .map(|w| {
            let half_w = (*w).max(PROPOSITION_LABEL_MIN_W) * 0.5;
            half_w * ux.abs() + half_h * uy.abs()
        })
        .fold(0.0f32, f32::max);
    // 相邻盒净空 ≥ PROPOSITION_LABEL_CLEARANCE：
    // step − span[i] − span[i+1] ≥ 2·max + 净空 − 2·max = 净空。
    let step = 2.0 * max_span + PROPOSITION_LABEL_CLEARANCE;
    let mid = (n as f32 - 1.0) * 0.5;
    (0..n).map(|k| (k as f32 - mid) * step).collect()
}

/// 一次整理的输入。
///
/// 三个切片都以**概念下标**为准，`sizes` 与 `current` 长度一致；
/// `current` 只用于「同分时谁在前」和左对齐锚点，不参与几何计算。
#[derive(Clone, Copy, Debug)]
pub struct Arrange<'a> {
    pub sizes: &'a [(f32, f32)],
    pub edges: &'a [(usize, usize)],
    pub current: &'a [(f32, f32)],
}

/// 一次「力导向流动」的输入。与 [`Arrange`] 同构，多两个字段：
///
/// - `fixed`：按概念下标的「钉住」标记；`fixed[i] == true` 的槽位**原样输出 `current[i]`**，
///   但仍参与弹簧/斥力/隔断（别的概念会绕着它摆）。长度不足 `n` 时缺的位按 `false`（可动）处理，
///   多出的位忽略——与 `arrange_scoped` 同一口径。
/// - `iterations`：物理轮数，由调用方定，内部不夹死（`0` = 不跑物理，只做散开/平移/收口）。
#[derive(Clone, Copy, Debug)]
pub struct Flow<'a> {
    pub sizes: &'a [(f32, f32)],
    pub edges: &'a [(usize, usize)],
    pub current: &'a [(f32, f32)],
    pub fixed: &'a [bool],
    pub iterations: usize,
}

/// 整理后的左上角坐标，顺序与输入一致。
pub fn arrange(input: Arrange<'_>) -> Vec<(f32, f32)> {
    arrange_scoped(input, &[])
}

/// 带「钉住/选中范围」的整理：`fixed[i] == true` 的概念原样留在 `current[i]`，
/// 其余概念照 [`arrange`] 的五步重摆，并对齐到**可动子集**自己的包围盒左上角
/// （只整理选中时，不该被全图的包围盒拖走）。
///
/// `arrange(i) == arrange_scoped(i, &[])`：空 `fixed` = 全部可动，行为与今天逐字节一致。
///
/// `fixed` 的长度按**不足位 = 可动**处理（不 panic、不静默丢弃多余的位）：
/// 调用方从选择集拼标记数组时最容易差一两个位，布局层按语义兜底。
///
/// **分层是拓扑的，摆位才受 fixed 影响**：钉住/未选中的概念照常进 `kept_edges`
/// 与分层计算——否则「只整理选中」会让选中簇的层级凭空浅一层。受影响的只是：
/// 初摆/松弛/压开只在可动槽位上进行（`fixed` 槽不进 `order`），锚点只取可动子集，
/// 收尾 [`settle`] 再把压到 `fixed` 盒子上的可动概念竖直推开（`docs/30` §5.5 的
/// 不重叠验收对全图成立）。
pub fn arrange_scoped(input: Arrange<'_>, fixed: &[bool]) -> Vec<(f32, f32)> {
    let n = input.sizes.len();
    if n == 0 {
        return Vec::new();
    }
    let movable = movable_flags(n, fixed);
    if movable.iter().all(|m| !m) {
        // 全钉住 = 一步都不动，原样交回。
        return input.current.to_vec();
    }
    let anchor = anchor_of(input.current, &movable);
    if n == 1 {
        return vec![anchor];
    }

    // 分层/边集走全图拓扑（fixed 也参与，见函数头注释）。
    let kept = kept_edges(n, input.edges, input.current);
    let layers = layers_of(n, &kept);
    // 摆位只在可动子集上进行：`order` 滤掉 fixed 槽，`slot_of` 给松弛/重心查「这个概念的槽位」。
    let mut order: Vec<usize> = initial_order(n, &layers, input.current)
        .into_iter()
        .filter(|&i| movable[i])
        .collect();
    let slot_of = slot_of_of(&order, n);
    let spans = spans_of(&order, &layers);
    // 重心只决定层内先后，不决定间距：重排完再等距铺一遍。
    for _ in 0..4 {
        order = barycenter_order(&order, &spans, &kept, &slot_of);
    }
    let mut ys = initial_rows(&order, &spans, input.sizes, anchor.1);

    for _ in 0..RELAX_ROUNDS {
        relax_once(&order, &spans, &kept, input, &mut ys, anchor.1, &slot_of);
    }
    separate(&order, &spans, input.sizes, &mut ys, anchor.1);

    let xs = column_x(&layers, input.sizes, anchor.0);
    let mut out = vec![(0.0f32, 0.0f32); n];
    for (slot, &idx) in order.iter().enumerate() {
        out[idx] = (xs[idx], ys[slot]);
    }
    for (i, m) in movable.iter().enumerate() {
        if !m {
            out[i] = input.current[i];
        }
    }
    // 收尾隔断：可动概念不得压在 fixed/未选中概念之上。全可动时分层算法自己的
    // `separate` + 列宽保证已不重叠，这一步直接跳过（0 轮），`arrange` 的旧行为逐字节不变。
    let rounds = if movable.iter().all(|m| *m) { 0 } else { n + 4 };
    settle(&mut out, input.sizes, &movable, rounds);
    out
}

/// `fixed` → `movable`：下标语义反转 + 长度兜底（缺位按可动）。
fn movable_flags(n: usize, fixed: &[bool]) -> Vec<bool> {
    (0..n)
        .map(|i| !fixed.get(i).copied().unwrap_or(false))
        .collect()
}

/// 概念下标 → 它在 `order` 里的槽位；不在（= 被钉住）的槽位给 `usize::MAX` 哨兵。
fn slot_of_of(order: &[usize], n: usize) -> Vec<usize> {
    let mut slot_of = vec![usize::MAX; n];
    for (s, &idx) in order.iter().enumerate() {
        slot_of[idx] = s;
    }
    slot_of
}

fn anchor_of(current: &[(f32, f32)], movable: &[bool]) -> (f32, f32) {
    let mut x = f32::INFINITY;
    let mut y = f32::INFINITY;
    for (i, &(cx, cy)) in current.iter().enumerate() {
        if !movable[i] {
            continue;
        }
        if cx.is_finite() {
            x = x.min(cx);
        }
        if cy.is_finite() {
            y = y.min(cy);
        }
    }
    let x = if x.is_finite() { x } else { 0.0 };
    let y = if y.is_finite() { y } else { 0.0 };
    (x, y)
}

/// 弹簧理想边长：盒子半宽和 + `LAYER_GAP`（`docs/30` §5.5 的层间距下限思路）。
fn rest_len(sizes: &[(f32, f32)], a: usize, b: usize) -> f32 {
    LAYER_GAP + (axis(sizes[a].0) + axis(sizes[b].0)) * 0.5
}

/// 斥力作用半径：两盒对角线的均值。半径内互相推开，半径外互不打扰。
fn repel_range(sizes: &[(f32, f32)], a: usize, b: usize) -> f32 {
    (diag(sizes[a]) + diag(sizes[b])) * 0.5
}

fn axis(v: f32) -> f32 {
    if v.is_finite() {
        v.max(1.0)
    } else {
        1.0
    }
}

fn diag(s: (f32, f32)) -> f32 {
    (axis(s.0) * axis(s.0) + axis(s.1) * axis(s.1)).sqrt()
}

/// 落点避让的搜索单步（世界单位）。与 [`NODE_GAP`] 同档：一圈比一圈远，
/// 第一圈就是「刚好让开一个净空」。
pub const RESOLVE_STEP: f32 = 24.0;
/// 落点避让最多找几圈：`RESOLVE_STEP × RESOLVE_RINGS` 是放弃的半径。
pub const RESOLVE_RINGS: usize = 16;
/// 每圈试几个方向。16 个足够密——相邻两个方向的弦长 < `RESOLVE_STEP / 2`，
/// 漏掉的空位一定比一圈小，下一圈还会捡到。
pub const RESOLVE_DIRS: usize = 16;

/// 落点避让（`docs/33` §2 B3）：把新概念从 `want` 挪到**最近的不重叠位置**。
///
/// 既有盒**一个像素都不动**——那可能是用户精心摆好的；重叠已经是既成事实时
/// （粘贴、导入），只在「整理布局」里处理，不在这里替用户重排。
///
/// 搜索是确定性的：以 `want` 为中心、半径按 [`RESOLVE_STEP`] 递增的同心环，
/// 每环 [`RESOLVE_DIRS`] 个方向按固定顺序试，取第一个与所有既有盒都留够
/// [`NODE_GAP`] 净空的候选。找不到就**原样返回 `want`**，由调用方决定要不要提示
/// （返回值相同，意味着「避让失败」，不是「避让到原地」——见返回值的说明）。
///
/// 参数里位置与尺寸成对放在一个切片里，避免调用方把两个平行切片拼错位。
/// 坐标口径与 [`arrange`] 一致：**盒子左上角**。
pub fn resolve_overlap(
    want: (f32, f32),
    size: (f32, f32),
    others: &[((f32, f32), (f32, f32))],
) -> (f32, f32) {
    let fits = |p: (f32, f32)| {
        others
            .iter()
            .all(|&(q, t)| clear_of(p, size, q, t, NODE_GAP))
    };
    if fits(want) {
        return want;
    }
    for ring in 1..=RESOLVE_RINGS {
        let r = RESOLVE_STEP * ring as f32;
        for k in 0..RESOLVE_DIRS {
            // 固定角度网格：同一份文档两次落点必须一致（`arrange_is_deterministic` 同一要求）。
            let ang = std::f32::consts::TAU * k as f32 / RESOLVE_DIRS as f32;
            let p = (want.0 + r * ang.cos(), want.1 + r * ang.sin());
            if fits(p) {
                return p;
            }
        }
    }
    // 绕了一圈没找到：原样交回。调用方拿 `p == want` 当「避让失败」的信号。
    want
}

/// 两盒之间是否留够 `gap` 净空（不相交，且四边都不小于 `gap`）。
fn clear_of(p: (f32, f32), s: (f32, f32), q: (f32, f32), t: (f32, f32), gap: f32) -> bool {
    p.0 + axis(s.0) + gap <= q.0 + 0.01
        || q.0 + axis(t.0) + gap <= p.0 + 0.01
        || p.1 + axis(s.1) + gap <= q.1 + 0.01
        || q.1 + axis(t.1) + gap <= p.1 + 0.01
}

/// 两个盒子是否重叠（与测试夹具同一口径：边缘 0.01 内不算）。
fn boxes_overlap(p: (f32, f32), s: (f32, f32), q: (f32, f32), t: (f32, f32)) -> bool {
    p.0 + axis(s.0) > q.0 + 0.01
        && q.0 + axis(t.0) > p.0 + 0.01
        && p.1 + axis(s.1) > q.1 + 0.01
        && q.1 + axis(t.1) > p.1 + 0.01
}

/// 弹簧边集：只过滤越界与自环，**保留平行边**。
///
/// 与 [`kept_edges`] 的分工：那张是「分层」用的 DAG（去重 + 截回边），力导向
/// 不吃拓扑——环、平行边都只是力。同一对概念挂两条命题（「A 影响 B」+「A 包含 B」）
/// 在这里就是两根弹簧，拉得更紧：这与 `kept_edges:90-94` 那段说的是同一件事——
/// 平行命题是**不同的命题**（C14 口径），分层可以合流，力学不该合流。
/// 顺序按 `(a, b, 原下标)` 定死，保证确定性。
fn spring_edges(n: usize, edges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut all: Vec<(usize, usize, usize)> = edges
        .iter()
        .enumerate()
        .filter(|&(_, &(a, b))| a < n && b < n && a != b)
        .map(|(i, &(a, b))| (a, b, i))
        .collect();
    all.sort_unstable();
    all.into_iter().map(|(a, b, _)| (a, b)).collect()
}

/// 确定性力导向：弹簧拉 + 盒子近距互斥 + 收尾压开，整体平移到可动子集
/// 原包围盒左上角（与 [`arrange`] 「左对齐到原包围盒」同一用户预期）。
///
/// 无随机、无墙钟：全部散开/推挤按下标定序。`fixed` 槽位原样输出 `current[i]`，
/// 但弹簧与斥力都拿它当一根**桩子**——可动概念会被它拉、被它挤开，它自己不动。
///
/// 退化输入的对称性是显式破掉的：可动概念彼此同点（或全 NaN 折叠到同一点）时，
/// 斥力两两对称抵消、永远原地。所以初摆阶段对「同点组」按黄金角螺旋散开
/// （每组第一个不动、其余按组内下标散），之后才进物理。
pub fn flow(input: Flow<'_>) -> Vec<(f32, f32)> {
    let n = input.sizes.len();
    if n == 0 {
        return Vec::new();
    }
    let movable = movable_flags(n, input.fixed);
    if movable.iter().all(|m| !m) {
        return input.current.to_vec();
    }
    let anchor = anchor_of(input.current, &movable);
    if n == 1 {
        return vec![anchor];
    }

    let springs = spring_edges(n, input.edges);

    // —— 初摆：fixed 原位；可动取 current（非有限折到 f32::MAX），同点组螺旋散开 ——
    let mut p: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            if movable[i] {
                (rank(input.current[i].0), rank(input.current[i].1))
            } else {
                input.current[i]
            }
        })
        .collect();
    let mut seen: Vec<(u32, u32)> = Vec::new(); // 已出现的可动落点（按位）
    for i in 0..n {
        if !movable[i] {
            continue;
        }
        let key = (p[i].0.to_bits(), p[i].1.to_bits());
        match seen.iter().position(|&k| k == key) {
            // 组内下标 0 的留在原地；后面的沿黄金角螺旋散开（按散开半径与角度都只依赖下标）。
            Some(_) => {
                let j = i as f32; // 用全局下标定角与半径：同点组内谁先谁后由下标唯一决定
                let theta = j * 2.399_632_3;
                let r = repel_range(input.sizes, i, i) * (1.0 + j).sqrt();
                p[i].0 += r * theta.cos();
                p[i].1 += r * theta.sin();
            }
            None => seen.push(key),
        }
    }

    // —— 物理：每轮「成对斥力 + 逐弹簧引力」，位移先攒后放，fixed 端不动 ——
    const SPRING_K: f32 = 0.25; // 弹簧每轮的收敛比例（两端各吃 share，fixed 端独吃双倍）
    const REPEL_K: f32 = 0.25;
    const MAX_STEP: f32 = 168.0; // 单轮位移上限：防退化初距把节点弹飞
    for _ in 0..input.iterations {
        let mut disp = vec![(0.0f32, 0.0f32); n];
        for i in 0..n {
            for j in i + 1..n {
                if !movable[i] && !movable[j] {
                    continue;
                }
                let range = repel_range(input.sizes, i, j);
                let dx = p[j].0 - p[i].0;
                let dy = p[j].1 - p[i].1;
                let d = (dx * dx + dy * dy).sqrt();
                if d >= range {
                    continue;
                }
                // 同点（d≈0）时斥力方向没有定义：按 i/j 下标定一条确定轴（i 推 -x、j 推 +x）。
                let (ux, uy) = if d > 1e-3 {
                    (dx / d, dy / d)
                } else {
                    (-1.0, 0.0)
                };
                let push = (range - d) * REPEL_K;
                if movable[i] {
                    let mut m = disp[i];
                    m.0 -= ux * push;
                    m.1 -= uy * push;
                    disp[i] = m;
                }
                if movable[j] {
                    let mut m = disp[j];
                    m.0 += ux * push;
                    m.1 += uy * push;
                    disp[j] = m;
                }
            }
        }
        for &(a, b) in &springs {
            if !movable[a] && !movable[b] {
                continue;
            }
            let rest = rest_len(input.sizes, a, b);
            let dx = p[b].0 - p[a].0;
            let dy = p[b].1 - p[a].1;
            let d = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (ux, uy) = (dx / d, dy / d);
            let share = (d - rest) * SPRING_K;
            let other_fixed_a = !movable[a];
            let other_fixed_b = !movable[b];
            if movable[a] {
                let s = if other_fixed_b { share * 2.0 } else { share };
                let mut m = disp[a];
                m.0 += ux * s;
                m.1 += uy * s;
                disp[a] = m;
            }
            if movable[b] {
                let s = if other_fixed_a { share * 2.0 } else { share };
                let mut m = disp[b];
                m.0 -= ux * s;
                m.1 -= uy * s;
                disp[b] = m;
            }
        }
        for i in 0..n {
            if !movable[i] {
                continue;
            }
            let d = disp[i];
            let mag = (d.0 * d.0 + d.1 * d.1).sqrt();
            let scale = if mag > MAX_STEP { MAX_STEP / mag } else { 1.0 };
            p[i].0 += d.0 * scale;
            p[i].1 += d.1 * scale;
        }
    }

    // —— 平移：可动子集整体搬到 `anchor`（fixed 不动；与 arrange 的「左对齐到原包围盒」同款预期）——
    let (min_x, min_y) = {
        let mut mx = f32::INFINITY;
        let mut my = f32::INFINITY;
        for i in 0..n {
            if movable[i] {
                mx = mx.min(p[i].0);
                my = my.min(p[i].1);
            }
        }
        (mx, my)
    };
    if min_x.is_finite() && min_y.is_finite() {
        let dx = anchor.0 - min_x;
        let dy = anchor.1 - min_y;
        for i in 0..n {
            if movable[i] {
                p[i].0 += dx;
                p[i].1 += dy;
            }
        }
    }

    // —— 收口：二维压开，不许有重叠（`docs/30` §5.5 对全图成立，fixed 也守）——
    settle(&mut p, input.sizes, &movable, n + 4);
    p
}

/// 二维收口：把可动概念朝**下/右**推，直到任何两盒都不重叠；`fixed` 概念当墙。
///
/// 每轮按 `(y, x, 下标)` 定序扫一遍，把压到别人盒子上的可动概念下缘顶过对方
/// 下缘 + `NODE_GAP`；一轮扫不干净就再来一轮（下推可能撞上下一个），最多 `rounds` 轮。
/// 没有任何重叠时这是恒等变换——`arrange_scoped` 的「全 false == arrange 逐字节」
/// 就靠这一点兜住。
fn settle(pos: &mut [(f32, f32)], sizes: &[(f32, f32)], movable: &[bool], rounds: usize) {
    let mut idx: Vec<usize> = (0..pos.len()).collect();
    for _ in 0..rounds {
        idx.sort_by(|&a, &b| {
            (rank(pos[a].1), rank(pos[a].0), a)
                .partial_cmp(&(rank(pos[b].1), rank(pos[b].0), b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut dirty = false;
        for k in 0..idx.len() {
            let i = idx[k];
            if !movable[i] {
                continue;
            }
            for j in idx.iter() {
                if *j == i {
                    continue;
                }
                if boxes_overlap(pos[i], sizes[i], pos[*j], sizes[*j]) {
                    let want = pos[*j].1 + axis(sizes[*j].1) + NODE_GAP;
                    if pos[i].1 < want {
                        pos[i].1 = want;
                        dirty = true;
                    }
                }
            }
        }
        if !dirty {
            return;
        }
    }
}

/// 过滤非法边 + 去重 + 丢回边（DFS 里指向「正在访问」节点的边）。
///
/// 返回的边构成 DAG，顺序按「起点当前 y、终点当前 y、下标」定死。
///
/// **这里的去重是布局口径，不是数据口径。** 概念图允许同一对概念之间有多条边
/// （两条边 = 两条不同的命题，如「A 影响 B」+「A 包含 B」，见 `Document::apply_inner`
/// 的 `AddRelationship` 分支），但摆位只关心「谁在谁的下游」：同一对概念算两条边
/// 会让层级权重凭空翻倍，图就摆歪了。所以 `(a, b)` 元组在这里按几何去重，
/// `document.nodes[..].relationships` 里那几条边一条都不会少——删掉一条，另一条还在。
fn kept_edges(n: usize, edges: &[(usize, usize)], current: &[(f32, f32)]) -> Vec<(usize, usize)> {
    let mut all: Vec<(usize, usize)> = edges
        .iter()
        .copied()
        .filter(|&(a, b)| a < n && b < n && a != b)
        .collect();
    // 只按 `(a, b)` 去重：平行边（同起点同终点、不同 `Relationship.id`/短语）在这里合流。
    all.sort_unstable();
    all.dedup();

    // 邻接表：出边按 (终点当前 y, 下标) 排，DFS 顺序才能复现。
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b) in &all {
        out[a].push(b);
    }
    for list in out.iter_mut() {
        list.sort_by(|&p, &q| {
            (rank(current[p].1), rank(current[p].0), p)
                .partial_cmp(&(rank(current[q].1), rank(current[q].0), q))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // 0 未访问 / 1 正在访问 / 2 已完成。指向 1 的边是回边，丢掉。
    let mut state = vec![0u8; n];
    let mut roots: Vec<usize> = (0..n).collect();
    roots.sort_by(|&p, &q| {
        (rank(current[p].0), rank(current[p].1), p)
            .partial_cmp(&(rank(current[q].0), rank(current[q].1), q))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept = Vec::with_capacity(all.len());
    // 显式栈：`(节点, 已展开到第几条出边)`，不用递归，深度不受限。
    for root in roots {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(&(v, i)) = stack.last() {
            if i >= out[v].len() {
                state[v] = 2;
                stack.pop();
                continue;
            }
            let w = out[v][i];
            if let Some(top) = stack.last_mut() {
                top.1 += 1;
            }
            match state[w] {
                0 => {
                    state[w] = 1;
                    kept.push((v, w));
                    stack.push((w, 0));
                }
                // 1 = 回边（环）：截断，不参与分层。
                1 => {}
                // 2 = 已完成：可能是交叉边/前向边，保留但不能再成环（终点已定层）。
                _ => kept.push((v, w)),
            }
        }
    }
    kept.sort_unstable();
    kept
}

/// 最长路径分层。
fn layers_of(n: usize, kept: &[(usize, usize)]) -> Vec<u32> {
    let mut layer = vec![0u32; n];
    // DAG 上反复松弛，最多 n 轮必然收敛（每轮至少把一条边「拉直」）。
    for _ in 0..n {
        let mut moved = false;
        for &(a, b) in kept {
            let want = layer[a].saturating_add(1);
            if layer[b] < want {
                layer[b] = want;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    layer
}

fn initial_order(n: usize, layers: &[u32], current: &[(f32, f32)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        (layers[a], rank(current[a].1), rank(current[a].0), a)
            .partial_cmp(&(layers[b], rank(current[b].1), rank(current[b].0), b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}

/// `order` 里每一层的槽位区间（`order` 已按层分组，所以区间连续）。
fn spans_of(order: &[usize], layers: &[u32]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    while start < order.len() {
        let layer = layers[order[start]];
        let mut end = start + 1;
        while end < order.len() && layers[order[end]] == layer {
            end += 1;
        }
        spans.push((start, end));
        start = end;
    }
    spans
}

fn initial_rows(
    order: &[usize],
    spans: &[(usize, usize)],
    sizes: &[(f32, f32)],
    top: f32,
) -> Vec<f32> {
    let mut ys = vec![0.0; order.len()];
    for &(start, end) in spans {
        let mut y = top;
        for slot in start..end {
            ys[slot] = y;
            y += sizes[order[slot]].1 + NODE_GAP;
        }
    }
    ys
}

/// 按入边父重心稳定重排每一层（`order` 的分段不变）。
///
/// 入边父概念若被钉住（不在可动 `order` 里、`slot_of` 给哨兵），它的边照样计入重心——
/// 分层是拓扑的，摆位才受 fixed 影响。
fn barycenter_order(
    order: &[usize],
    spans: &[(usize, usize)],
    kept: &[(usize, usize)],
    slot_of: &[usize],
) -> Vec<usize> {
    let mut out = order.to_vec();
    for &(start, end) in spans {
        let mut keyed: Vec<(f32, usize)> = (start..end)
            .map(|i| {
                let idx = order[i];
                let mut sum = 0.0;
                let mut count = 0.0;
                for &(a, b) in kept {
                    if b == idx && slot_of[a] != usize::MAX {
                        sum += slot_of[a] as f32;
                        count += 1.0;
                    }
                }
                let key = if count > 0.0 {
                    sum / count
                } else {
                    f32::INFINITY
                };
                (key, i)
            })
            .collect();
        keyed.sort_by(|&(ka, ia), &(kb, ib)| {
            ka.partial_cmp(&kb)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(ia.cmp(&ib))
        });
        for (dst, &(_, src)) in keyed.iter().enumerate() {
            out[start + dst] = order[src];
        }
    }
    out
}

/// 列位：每层一列，列宽取该层最宽概念，列间 `LAYER_GAP`；整体左对齐到 `left`。
fn column_x(layers: &[u32], sizes: &[(f32, f32)], left: f32) -> Vec<f32> {
    let max_layer = layers.iter().copied().max().unwrap_or(0) as usize;
    let mut widths = vec![0.0f32; max_layer + 1];
    for (i, &l) in layers.iter().enumerate() {
        let l = l as usize;
        widths[l] = widths[l].max(sizes[i].0);
    }
    let mut xs = vec![0.0f32; max_layer + 1];
    let mut cursor = left;
    for (l, w) in widths.iter().enumerate() {
        xs[l] = cursor;
        cursor += w + LAYER_GAP;
    }
    layers.iter().map(|&l| xs[l as usize]).collect()
}

/// 一轮松弛：朝邻居中心均值靠一半，再压开。
///
/// 邻居若被钉住（`slot_of[o] == usize::MAX`），它不参与「槽位」而直接以 `current[o]`
/// 的纵坐标计入中心均值——钉住的概念是一根桩子，可动的概念会朝它靠拢。
fn relax_once(
    order: &[usize],
    spans: &[(usize, usize)],
    kept: &[(usize, usize)],
    input: Arrange<'_>,
    ys: &mut [f32],
    top: f32,
    slot_of: &[usize],
) {
    let mut target = vec![f32::INFINITY; order.len()];
    for (i, &idx) in order.iter().enumerate() {
        let mut sum = 0.0;
        let mut count = 0.0;
        for &(a, b) in kept {
            let other = if b == idx {
                Some(a)
            } else if a == idx {
                Some(b)
            } else {
                None
            };
            if let Some(o) = other {
                let os = slot_of[o];
                let oy = if os == usize::MAX {
                    rank(input.current[o].1)
                } else {
                    ys[os]
                };
                sum += oy + input.sizes[o].1 * 0.5;
                count += 1.0;
            }
        }
        if count > 0.0 {
            target[i] = sum / count - input.sizes[idx].1 * 0.5;
        }
    }
    for (i, &idx) in order.iter().enumerate() {
        if target[i].is_finite() {
            let _ = idx;
            ys[i] += (target[i] - ys[i]) * 0.5;
        }
    }
    separate(order, spans, input.sizes, ys, top);
}

/// 层内压开：每段从上界开始，保证相邻间隙 ≥ `NODE_GAP`。
fn separate(
    order: &[usize],
    spans: &[(usize, usize)],
    sizes: &[(f32, f32)],
    ys: &mut [f32],
    top: f32,
) {
    for &(start, end) in spans {
        if start >= end {
            continue;
        }
        ys[start] = ys[start].max(top);
        for slot in start + 1..end {
            let prev = slot - 1;
            let min = ys[prev] + sizes[order[prev]].1 + NODE_GAP;
            if ys[slot] < min {
                ys[slot] = min;
            }
        }
    }
}

/// 排序里用的键：`f32` 没有 `Ord`，非有限值统一推到末尾。
fn rank(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        f32::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects(sizes: &[(f32, f32)], pos: &[(f32, f32)]) -> Vec<(f32, f32, f32, f32)> {
        sizes
            .iter()
            .zip(pos)
            .map(|(&(w, h), &(x, y))| (x, y, w, h))
            .collect()
    }

    fn overlap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        a.0 + a.2 > b.0 + 0.01
            && b.0 + b.2 > a.0 + 0.01
            && a.1 + a.3 > b.1 + 0.01
            && b.1 + b.3 > a.1 + 0.01
    }

    fn assert_no_overlap(sizes: &[(f32, f32)], out: &[(f32, f32)]) {
        let rs = rects(sizes, out);
        for i in 0..rs.len() {
            for j in i + 1..rs.len() {
                assert!(!overlap(rs[i], rs[j]), "第 {i} 与第 {j} 个概念重叠：{rs:?}");
            }
        }
    }

    /// 演示概念图：一个中心 + 五个概念 + 一条交叉连接。
    fn demo() -> (Vec<(f32, f32)>, Vec<(usize, usize)>, Vec<(f32, f32)>) {
        let sizes = vec![
            (120.0, 36.0),
            (96.0, 36.0),
            (150.0, 36.0),
            (110.0, 52.0),
            (104.0, 36.0),
            (128.0, 36.0),
        ];
        let edges = vec![(0, 1), (0, 3), (0, 2), (1, 4), (3, 5), (2, 5)];
        let current = vec![
            (0.0, 0.0),
            (-330.0, -170.0),
            (250.0, -230.0),
            (280.0, 40.0),
            (-300.0, 110.0),
            (-40.0, 200.0),
        ];
        (sizes, edges, current)
    }

    #[test]
    fn arrange_separates_layers_left_to_right() {
        let (sizes, edges, current) = demo();
        let out = arrange(Arrange {
            sizes: &sizes,
            edges: &edges,
            current: &current,
        });
        assert_eq!(out.len(), 6);
        assert_no_overlap(&sizes, &out);
        // 中心在第一列，直接子概念严格在右侧 ≥ LAYER_GAP 之外（列宽不同，取列左缘）
        let x0 = out[0].0;
        for i in [1, 2, 3] {
            assert!(out[i].0 >= x0 + sizes[0].0 + LAYER_GAP - 0.01, "{out:?}");
        }
        // 左对齐：最左缘落在原包围盒左上角
        let min_x = out.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        assert!((min_x - (-330.0)).abs() < 0.01, "{min_x}");
    }

    #[test]
    fn arrange_keeps_min_gap_inside_a_layer() {
        let sizes = vec![(100.0, 36.0), (100.0, 36.0), (100.0, 36.0)];
        let edges = vec![(0, 1), (0, 2)];
        let current = vec![(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)];
        let out = arrange(Arrange {
            sizes: &sizes,
            edges: &edges,
            current: &current,
        });
        assert_no_overlap(&sizes, &out);
        let mut ys = [out[1].1, out[2].1];
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(ys[1] - (ys[0] + 36.0) >= NODE_GAP - 0.01, "{out:?}");
    }

    #[test]
    fn arrange_survives_cycles() {
        // a → b → c → a：回边被截断，结果仍是三列且不重叠。
        let sizes = vec![(100.0, 36.0); 3];
        let edges = vec![(0, 1), (1, 2), (2, 0)];
        let current = vec![(0.0, 0.0), (120.0, 0.0), (240.0, 0.0)];
        let out = arrange(Arrange {
            sizes: &sizes,
            edges: &edges,
            current: &current,
        });
        assert_no_overlap(&sizes, &out);
        let mut xs: Vec<f32> = out.iter().map(|p| p.0).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            (xs[1] - xs[0] - (100.0 + LAYER_GAP)).abs() < 0.01,
            "{out:?}"
        );
        assert!(
            (xs[2] - xs[1] - (100.0 + LAYER_GAP)).abs() < 0.01,
            "{out:?}"
        );
    }

    #[test]
    fn arrange_is_deterministic_and_keeps_single_node_put() {
        let (sizes, edges, current) = demo();
        let a = arrange(Arrange {
            sizes: &sizes,
            edges: &edges,
            current: &current,
        });
        let b = arrange(Arrange {
            sizes: &sizes,
            edges: &edges,
            current: &current,
        });
        assert_eq!(a, b);

        let solo = arrange(Arrange {
            sizes: &[(120.0, 36.0)],
            edges: &[],
            current: &[(7.0, 9.0)],
        });
        assert_eq!(solo, vec![(7.0, 9.0)]);
        assert!(arrange(Arrange {
            sizes: &[],
            edges: &[],
            current: &[]
        })
        .is_empty());
    }

    #[test]
    fn arrange_ignores_dangling_or_self_edges() {
        let sizes = vec![(100.0, 36.0), (100.0, 36.0)];
        let current = vec![(0.0, 0.0), (0.0, 200.0)];
        let out = arrange(Arrange {
            sizes: &sizes,
            edges: &[(0, 0), (0, 9), (5, 1)],
            current: &current,
        });
        assert_eq!(out.len(), 2);
        assert_no_overlap(&sizes, &out);
    }

    #[test]
    fn arrange_scoped_empty_fixed_matches_arrange() {
        let (sizes, edges, current) = demo();
        let regular = arrange(Arrange {
            sizes: &sizes,
            edges: &edges,
            current: &current,
        });
        let scoped_empty = arrange_scoped(
            Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &[],
        );
        let scoped_all_false = arrange_scoped(
            Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &[false; 6],
        );
        assert_eq!(regular, scoped_empty);
        assert_eq!(regular, scoped_all_false);
    }

    #[test]
    fn arrange_scoped_all_fixed_keeps_current_positions() {
        let (sizes, edges, current) = demo();
        let out = arrange_scoped(
            Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &[true; 6],
        );
        assert_eq!(out, current);
    }

    #[test]
    fn arrange_scoped_fixed_node_stays_put_and_movable_nodes_move() {
        let (sizes, edges, current) = demo();
        // 钉住节点 1 与 4：它们的位置必须逐字节等于 current[1] 与 current[4]
        let mut fixed = [false; 6];
        fixed[1] = true;
        fixed[4] = true;
        let out = arrange_scoped(
            Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &fixed,
        );
        assert_eq!(out.len(), 6);
        assert_eq!(out[1], current[1], "钉住的节点 1 必须原样不动");
        assert_eq!(out[4], current[4], "钉住的节点 4 必须原样不动");
        // 未钉住的节点至少有一颗发生了位移
        assert!(
            out[0] != current[0] || out[2] != current[2] || out[3] != current[3],
            "未钉住的节点应该重新摆放"
        );
        // 全图不重叠（movable 节点避让 fixed 节点）
        assert_no_overlap(&sizes, &out);
    }

    #[test]
    fn arrange_scoped_handles_short_and_long_fixed_slices() {
        let (sizes, edges, current) = demo();
        // 切片短于 n：不足位按可动处理，不 panic
        let out_short = arrange_scoped(
            Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &[true, false],
        );
        assert_eq!(out_short.len(), 6);
        assert_eq!(out_short[0], current[0]);
        assert_no_overlap(&sizes, &out_short);

        // 切片长于 n：超出位安全忽略
        let out_long = arrange_scoped(
            Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &[false, false, false, false, false, false, true, true],
        );
        let regular = arrange(Arrange {
            sizes: &sizes,
            edges: &edges,
            current: &current,
        });
        assert_eq!(out_long, regular);
    }

    // ---- C21：三车道以上平行命题的短语避让 ----

    /// 短语胶囊盒（宽夹最小宽、高 `PROPOSITION_LABEL_HEIGHT`，与渲染层同口径）。
    fn pill(cx: f32, cy: f32, width: f32) -> (f32, f32, f32, f32) {
        let w = width.max(PROPOSITION_LABEL_MIN_W);
        (
            cx - w * 0.5,
            cy - PROPOSITION_LABEL_HEIGHT * 0.5,
            w,
            PROPOSITION_LABEL_HEIGHT,
        )
    }

    /// 两个轴对齐盒的净空：0 表示相接或重叠。
    ///
    /// 用**区间距离**而不是中心距：短语宽度不等时（如 36 与 108），
    /// 中心距减去半宽和才是净空，左边距差不是。
    fn box_gap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> f32 {
        let (a_left, a_top, a_w, a_h) = a;
        let (b_left, b_top, b_w, b_h) = b;
        let dx = (a_left - (b_left + b_w))
            .max(b_left - (a_left + a_w))
            .max(0.0);
        let dy = (a_top - (b_top + b_h)).max(b_top - (a_top + a_h)).max(0.0);
        (dx * dx + dy * dy).sqrt()
    }

    /// 偏移序列必须关于 0 对称（`k` 与 `n-1-k` 互为相反数）。
    fn assert_symmetric(offs: &[f32]) {
        let n = offs.len();
        for k in 0..n {
            assert!(
                (offs[k] + offs[n - 1 - k]).abs() < 0.01,
                "第 {k} 条与第 {} 不对称：{offs:?}",
                n - 1 - k
            );
        }
        if n % 2 == 1 {
            assert!(
                offs[n / 2].abs() < 0.01,
                "奇数车道的中车道必须零偏移：{offs:?}"
            );
        }
    }

    /// 相邻（以及任意两条）短语盒的净空都必须 ≥ `PROPOSITION_LABEL_CLEARANCE`。
    fn assert_clearance(offs: &[f32], widths: &[f32], tangent: (f32, f32)) {
        let len = (tangent.0 * tangent.0 + tangent.1 * tangent.1)
            .sqrt()
            .max(1e-6);
        let (ux, uy) = (tangent.0 / len, tangent.1 / len);
        let pills: Vec<_> = offs
            .iter()
            .zip(widths)
            .map(|(&t, &w)| pill(ux * t, uy * t, w))
            .collect();
        for i in 0..pills.len() {
            for j in i + 1..pills.len() {
                let gap = box_gap(pills[i], pills[j]);
                assert!(
                    gap >= PROPOSITION_LABEL_CLEARANCE - 0.01,
                    "第 {i} 与第 {j} 条短语净空 {gap:.2} < 24：{pills:?}（偏移 {offs:?}）"
                );
            }
        }
    }

    /// 单条命题：偏移恒为 0，短语仍落在曲线中点。
    #[test]
    fn one_lane_label_offset_is_always_zero() {
        for widths in [&[36.0][..], &[200.0][..]] {
            for tangent in [(0.0f32, 1.0f32), (1.0, 0.0), (0.6, 0.8)] {
                assert_eq!(proposition_label_offsets(widths, tangent), vec![0.0]);
            }
        }
    }

    /// **回归锚**：两条车道的偏移必须与 C14/C15 的旧公式逐位一致
    /// （`step = (k - (n-1)/2) * 24`）——`cm-parallel-select-a/b` 与 `cm-crosslink`
    /// 两张金样锁的就是这组数，短边 / 长短语 / 斜边都不许改口。
    #[test]
    fn two_lane_offsets_match_the_legacy_constant_step_bit_for_bit() {
        let cases: [(&[f32], (f32, f32)); 5] = [
            (&[36.0, 36.0], (0.0, 1.0)),
            (&[36.0, 36.0], (1.0, 0.0)),
            (&[240.0, 12.0], (0.6, 0.8)),
            (&[24.0, 24.0], (-0.5, -0.866_025_4)),
            (&[0.0, 500.0], (0.0, 0.0)),
        ];
        for (widths, tangent) in cases {
            let legacy: Vec<f32> = (0..widths.len())
                .map(|k| {
                    (k as f32 - (widths.len() as f32 - 1.0) * 0.5) * PROPOSITION_LABEL_LEGACY_STEP
                })
                .collect();
            let got = proposition_label_offsets(widths, tangent);
            assert_eq!(
                got.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                legacy.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "2 车道偏移与旧公式逐位不一致：{got:?} vs {legacy:?}（{widths:?} @{tangent:?}）"
            );
        }
    }

    /// 三条等宽短语（竖边，C15 场景的加一条版）：净空从 4 抬到 24，带对称。
    #[test]
    fn three_equal_lanes_get_real_clearance_and_stay_symmetric() {
        let widths = [36.0, 36.0, 36.0];
        let offs = proposition_label_offsets(&widths, (0.0, 1.0));
        assert_eq!(offs.len(), 3);
        // 竖边：切线是竖直的，投影半宽只剩盒高的一半 → 相邻必须差 44（20 + 24）。
        assert!(
            (offs[1] - offs[0] - 44.0).abs() < 0.01,
            "相邻切线间距应为 44（盒高 20 + 净空 24）：{offs:?}"
        );
        assert_symmetric(&offs);
        assert_clearance(&offs, &widths, (0.0, 1.0));
    }

    /// 水平边上的长短语：净空要按**短语宽度**要，不再只看盒高。
    #[test]
    fn three_lanes_on_a_horizontal_edge_pay_for_their_width() {
        let widths = [36.0, 108.0, 36.0];
        let offs = proposition_label_offsets(&widths, (1.0, 0.0));
        assert_symmetric(&offs);
        assert_clearance(&offs, &widths, (1.0, 0.0));
        // 等距对称带：相邻间距处处相等（`docs/33` §2 B2 R1）。
        let step = offs[1] - offs[0];
        assert!((offs[2] - offs[1] - step).abs() < 0.01, "{offs:?}");
        // 步长 = 2·max(投影半宽) + 净空 = 2·54 + 24 = 132。
        // 改前这里锁的是「按每条自己的字宽分别排」的 96——那正是「改一条短语
        // 把同组其它短语一起挪走」的元凶，见
        // `lane_offsets_do_not_move_when_one_label_is_retyped`。
        assert!((step - 132.0).abs() < 0.01, "{offs:?}");
    }

    /// 斜边（切线两个轴都有分量）：投影半宽把宽与高一起算进去。
    #[test]
    fn diagonal_tangent_uses_the_projected_half_extent() {
        let widths = [36.0, 36.0, 200.0];
        let offs = proposition_label_offsets(&widths, (0.6, 0.8));
        assert_symmetric(&offs);
        assert_clearance(&offs, &widths, (0.6, 0.8));
    }

    /// 五条长短语混排：任意两条都不许挤在一起（C15 的常量步长在这里只会留 4）。
    #[test]
    fn five_lanes_with_mixed_widths_never_crowd() {
        let widths = [24.0, 120.0, 36.0, 200.0, 24.0];
        for tangent in [(0.0f32, 1.0f32), (1.0, 0.0), (0.6, 0.8)] {
            let offs = proposition_label_offsets(&widths, tangent);
            assert_eq!(offs.len(), 5);
            assert_symmetric(&offs);
            assert_clearance(&offs, &widths, tangent);
            // 单调：车道序号越大，短语越往终点方向挪。
            for pair in offs.windows(2) {
                assert!(pair[1] > pair[0], "偏移必须随车道序号递增：{offs:?}");
            }
        }
    }

    /// 退化输入不炸：零向量切线按水平处理，宽度 0 按最小宽算。
    #[test]
    fn degenerate_inputs_stay_finite_and_spaced() {
        let offs = proposition_label_offsets(&[0.0, 0.0, 0.0], (0.0, 0.0));
        assert_eq!(offs.len(), 3);
        assert_clearance(&offs, &[0.0, 0.0, 0.0], (1.0, 0.0));
        assert!(proposition_label_offsets(&[], (1.0, 0.0)).is_empty());
    }

    /// 平行命题的最小夹具：两个概念 + 给定方向的若干条命题。
    /// `rels` 每项是 `(from, id, to, label)`。
    fn lane_scene(
        rels: &[(&str, &str, &str, &str)],
        a: (f32, f32),
        b: (f32, f32),
    ) -> crate::scene::Scene {
        let mut doc = crate::tfix::Builder::root("产品定位")
            .float("a", "甲概念", Some(a))
            .float("b", "乙概念", Some(b));
        for &(from, id, to, label) in rels {
            doc = doc.relationship(from, id, to, Some(label));
        }
        let mut doc = doc.build();
        doc.meta.layout.kind = "conceptMap".into();
        let settings = crate::model::LayoutSettings {
            kind: "conceptMap".into(),
            ..Default::default()
        };
        let theme = crate::theme::get_theme("snow");
        let root = doc.root_id.clone();
        crate::scene::build_scene(
            &doc,
            &root,
            &settings,
            &theme,
            &crate::text::MonoMeasurer,
            None,
        )
    }

    /// 三条同向命题（每条短语宽 36）。
    fn three_lane_scene(a: (f32, f32), b: (f32, f32)) -> crate::scene::Scene {
        lane_scene(
            &[
                ("a", "r1", "b", "导致"),
                ("a", "r2", "b", "支持"),
                ("a", "r3", "b", "推动"),
            ],
            a,
            b,
        )
    }

    #[test]
    fn three_lane_scene_keeps_24_world_unit_label_clearance() {
        let scene = three_lane_scene((0.0, -220.0), (0.0, 220.0));
        assert_eq!(scene.relationships.len(), 3, "三条命题都要进场景");
        let pills: Vec<_> = scene
            .relationships
            .iter()
            .map(|r| pill(r.label_at.x, r.label_at.y, r.label_w))
            .collect();
        for pair in pills.windows(2) {
            let gap = box_gap(pair[0], pair[1]);
            assert!(
                gap >= PROPOSITION_LABEL_CLEARANCE - 0.01,
                "相邻短语净空 {gap:.2} < 24：{pills:?}"
            );
        }
        // 中车道必须落在两条外侧车道的正中（法向 + 切线偏移都对称）。
        let (a, b, c) = (
            &scene.relationships[0],
            &scene.relationships[1],
            &scene.relationships[2],
        );
        assert!((a.label_at.x + c.label_at.x - 2.0 * b.label_at.x).abs() < 0.01);
        assert!((a.label_at.y + c.label_at.y - 2.0 * b.label_at.y).abs() < 0.01);
    }

    /// 短边（裁后约 64 世界单位）：旧公式的法向间距 `len/6 ≈ 10.7` < 盒高 20，
    /// 切线又只错开 24，两个轴同时不够——三条车道会真的叠字。
    #[test]
    fn short_edge_three_lanes_do_not_overlap() {
        let scene = three_lane_scene((-80.0, 0.0), (80.0, 0.0));
        assert_eq!(scene.relationships.len(), 3);
        let pills: Vec<_> = scene
            .relationships
            .iter()
            .map(|r| pill(r.label_at.x, r.label_at.y, r.label_w))
            .collect();
        for pair in pills.windows(2) {
            let gap = box_gap(pair[0], pair[1]);
            assert!(
                gap >= PROPOSITION_LABEL_CLEARANCE - 0.01,
                "短边相邻短语净空 {gap:.2} < 24（盒重叠）：{pills:?}"
            );
        }
    }

    /// A→B 与 B→A 混在同一对里：仍按无向对收成一组三条车道
    /// （C15 的「跨源节点收齐」不能被方向拆开），净空照样 ≥ 24。
    #[test]
    fn mixed_direction_three_lanes_share_one_band() {
        let scene = lane_scene(
            &[
                ("a", "r1", "b", "导致"),
                ("b", "r2", "a", "推动"),
                ("a", "r3", "b", "支持"),
            ],
            (0.0, -220.0),
            (0.0, 220.0),
        );
        assert_eq!(scene.relationships.len(), 3, "三条命题都要进场景");
        assert!(
            scene.relationships.iter().any(|r| r.from > r.to),
            "必须真的包含一条反向命题（from > to）"
        );
        let pills: Vec<_> = scene
            .relationships
            .iter()
            .map(|r| pill(r.label_at.x, r.label_at.y, r.label_w))
            .collect();
        for pair in pills.windows(2) {
            let gap = box_gap(pair[0], pair[1]);
            assert!(
                gap >= PROPOSITION_LABEL_CLEARANCE - 0.01,
                "混合方向相邻短语净空 {gap:.2} < 24：{pills:?}"
            );
        }
    }

    // ── B3 落点避让（`docs/33` §2）──

    /// 落点被占时挪到附近的空位，而且**既有盒一动不动**。
    ///
    /// 搜索是**同心环网格**（[`RESOLVE_DIRS`] 个方向），所以拿到的是「附近」
    /// 而不是严格最近——真实最近点不一定落在网格上。这条测试断言的是
    /// ① 不许再重叠 ② 不许跑远（在搜索半径内）③ 确定性由下一条兜。
    #[test]
    fn dropping_on_an_occupied_spot_slides_to_the_nearest_free_one() {
        let others = [((0.0, 0.0), (100.0, 50.0))];
        let want = (10.0, 10.0);
        let got = resolve_overlap(want, (100.0, 50.0), &others);
        assert_ne!(got, want, "落点被占就必须挪开");
        assert!(
            clear_of(got, (100.0, 50.0), (0.0, 0.0), (100.0, 50.0), NODE_GAP),
            "挪完之后必须留够净空：{got:?}"
        );
        let d = ((got.0 - want.0).powi(2) + (got.1 - want.1).powi(2)).sqrt();
        assert!(
            d <= RESOLVE_STEP * RESOLVE_RINGS as f32 + 0.01,
            "不许跑出搜索半径：{d:.1}"
        );
    }

    /// 网格命中时要正好落在**严格最近**的那个空位上：
    /// 这里最近点在正左方 24，正好是第一圈的一个方向。
    #[test]
    fn resolve_overlap_finds_the_exact_minimum_when_the_grid_hits_it() {
        // 新盒 100×50 想落在原点，右边 100 处有另一颗：只能往左退 24。
        let others = [((100.0, 0.0), (100.0, 50.0))];
        let got = resolve_overlap((0.0, 0.0), (100.0, 50.0), &others);
        assert!(
            (got.0 + 24.0).abs() < 0.01 && got.1.abs() < 0.01,
            "最近空位是正左方 24，网格正好命中：{got:?}"
        );
    }

    #[test]
    fn resolve_overlap_is_a_no_op_when_the_spot_is_free() {
        assert_eq!(
            resolve_overlap((5.0, 5.0), (100.0, 50.0), &[]),
            (5.0, 5.0),
            "落点空着就不该动它"
        );
        // 紧挨着但不相交（净空刚好够）也算空着。
        let others = [((148.0, 0.0), (100.0, 50.0))];
        assert_eq!(
            resolve_overlap((0.0, 0.0), (100.0, 50.0), &others),
            (0.0, 0.0)
        );
    }

    #[test]
    fn resolve_overlap_is_deterministic() {
        let others = [
            ((0.0, 0.0), (80.0, 40.0)),
            ((200.0, 0.0), (80.0, 40.0)),
            ((60.0, 300.0), (80.0, 40.0)),
        ];
        let a = resolve_overlap((10.0, 10.0), (80.0, 40.0), &others);
        let b = resolve_overlap((10.0, 10.0), (80.0, 40.0), &others);
        assert_eq!(a, b, "同一份文档两次落点必须一致");
    }

    /// 绕不出来就**原样返回**——调用方拿 `p == want` 当「避让失败」的信号，
    /// 由它决定是提示还是接受重叠（`docs/33` §2 B3：不替用户重排）。
    #[test]
    fn resolve_overlap_gives_up_and_returns_the_wanted_spot() {
        // 用一张密不透风的网格把整个搜索半径盖住：一格都找不到。
        let mut others = Vec::new();
        let step = RESOLVE_STEP * 0.9;
        let half = RESOLVE_STEP * RESOLVE_RINGS as f32 + 40.0;
        let mut y = -half;
        while y < half {
            let mut x = -half;
            while x < half {
                others.push(((x, y), (step * 0.98, step * 0.98)));
                x += step;
            }
            y += step;
        }
        let want = (0.0, 0.0);
        assert_eq!(
            resolve_overlap(want, (step, step), &others),
            want,
            "绕不出去就原样交回，别自己挪到别处"
        );
    }

    /// B2 R1：改一条短语的字宽，**同组其它短语的偏移一动不动**。
    ///
    /// 改前每条短语的字宽都进间距公式，改一条就把整条带重排一遍——
    /// 用户在概念图里编辑连接短语时看到的就是「文字在漂」。
    #[test]
    fn lane_offsets_do_not_move_when_one_label_is_retyped() {
        let tangent = (1.0, 0.0);
        let mut widths = vec![100.0f32, 40.0, 40.0, 40.0, 40.0];
        let before = proposition_label_offsets(&widths, tangent);
        // 改第 2 条（比最宽那条短）：整条带必须一动不动。
        widths[1] = 45.0;
        let after = proposition_label_offsets(&widths, tangent);
        assert_eq!(before, after, "改一条短语不该挪动同组其它短语");
        // 只有把它改成**最宽**那条时，整条带才以中点为轴对称撑开一次。
        widths[1] = 200.0;
        let wider = proposition_label_offsets(&widths, tangent);
        assert!(
            wider[1].abs() > after[1].abs(),
            "自己变宽后要往外挪：{:?} -> {:?}",
            after[1],
            wider[1]
        );
        assert_eq!(wider[0], -wider[4], "撑开必须以中点为轴对称：{wider:?}");
    }

    /// 净空判定：相交、或贴太近，都算「不让」。
    #[test]
    fn clear_of_rejects_touching_and_squeezed_pairs() {
        // 完全分开。
        assert!(clear_of(
            (0.0, 0.0),
            (10.0, 10.0),
            (100.0, 0.0),
            (10.0, 10.0),
            24.0
        ));
        // 相交。
        assert!(!clear_of(
            (0.0, 0.0),
            (10.0, 10.0),
            (5.0, 5.0),
            (10.0, 10.0),
            24.0
        ));
        // 不相交但净空只有 23（< 24）。
        assert!(!clear_of(
            (0.0, 0.0),
            (10.0, 10.0),
            (33.0, 0.0),
            (10.0, 10.0),
            24.0
        ));
        // 净空刚好 24。
        assert!(clear_of(
            (0.0, 0.0),
            (10.0, 10.0),
            (34.0, 0.0),
            (10.0, 10.0),
            24.0
        ));
    }
}
