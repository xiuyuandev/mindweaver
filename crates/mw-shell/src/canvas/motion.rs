use super::*;
use std::collections::HashSet;

pub(crate) use mw_core::design::motion::ease_out_cubic;

/// 过渡时长：小改动从令牌下限起步，跨布局的大位移不超过素材上限。再长就拖沓。
pub(crate) fn motion_duration(span: f32) -> f32 {
    (design::motion::SCENE_MIN + span / 4000.0)
        .clamp(design::motion::SCENE_MIN, design::motion::SCENE_MAX)
}

/// 过渡到底还跑不跑。
///
/// `reduce_motion` 是「系统动画设置 / 用户设置 / 金样」三方的合成开关，平时它一开就
/// 一个过渡都不起（[`motion_pin`] 的注释里说过为什么金样必须与时间无关）。
/// 但 `MW_MOTION_T` 把混合系数钉死之后，这一帧就不再看钟——时间无关这个理由不再成立，
/// 于是只放开**场景过渡**这一路；悬停/选中的 alpha 仍走 `reduce_motion` 的瞬时到位，
/// 画面照样是确定的。
pub(crate) fn motion_reduced(reduce_motion: bool) -> bool {
    reduce_motion && motion_pin().is_none()
}

impl Canvas {
    pub(crate) fn motion_reduced(&self) -> bool {
        motion_reduced(self.reduce_motion)
    }
}

/// 两帧场景的差异尺度；`None` = 几何没变，不值得起动画。
///
/// 只看节点盒：连线/边界由节点位置派生，主题换色也不该触发位移。
pub(crate) fn motion_span(from: &Scene, to: &Scene) -> Option<f32> {
    let mut structural = from.kind != to.kind || from.nodes.len() != to.nodes.len();
    let mut span: f32 = if from.kind != to.kind { 600.0 } else { 0.0 };
    // 关系线的新增/移除也要算「结构变了」：概念图里加一条命题线不挪任何盒子，
    // 只看几何的话新线会硬弹出来。比 id 集合就够——路径会随落点变，那不是结构。
    // 集合比较只在重建时跑一次，按 id 建哈希表，上千条线也不会退化成平方。
    if !structural {
        if from.relationships.len() != to.relationships.len() {
            structural = true;
        } else {
            let ids: HashSet<&str> = to.relationships.iter().map(|r| r.id.as_str()).collect();
            structural = from
                .relationships
                .iter()
                .any(|r| !ids.contains(r.id.as_str()));
        }
    }
    for (i, id) in to.ids.iter().enumerate() {
        match from.index.get(id.as_str()) {
            Some(&j) => {
                let a = from.nodes[j as usize].geom.rect;
                let b = to.nodes[i].geom.rect;
                let d = (a.x - b.x)
                    .abs()
                    .max((a.y - b.y).abs())
                    .max((a.w - b.w).abs())
                    .max((a.h - b.h).abs());
                if d > 0.01 {
                    structural = true;
                    span = span.max(d);
                }
            }
            None => structural = true,
        }
    }
    // 折叠/删除：消失节点滑向它在目标场景里的父节点。父节点往往原地不动，
    // 只量共享节点会把这段位移漏掉，折叠动画就永远是最短 0.20s。
    for j in 0..from.ids.len() {
        let id = &from.ids[j];
        if to.index.contains_key(id.as_str()) {
            continue;
        }
        structural = true;
        let old = from.nodes[j].geom.rect.center();
        let target = from.nodes[j]
            .parent
            .and_then(|p| from.ids.get(p as usize))
            .and_then(|pid| to.find(pid))
            .map(|i| to.rect(i).center())
            .unwrap_or(old);
        span = span.max((old.x - target.x).abs().max((old.y - target.y).abs()));
    }
    if structural {
        Some(span)
    } else {
        None
    }
}
