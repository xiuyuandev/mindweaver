use super::*;
use crate::canvas::canvas_methods_3::{
    densify, emphasis_alpha, emphasis_visible, emphasis_wanted, label_pill_rect_at, phrase_commit,
    sample_path, split_around, PhraseCommit, CONCEPT_UNNAMED_LINK,
};
use mw_core::layout::TOGGLE_OFFSET;
use mw_core::text::MonoMeasurer;
use mw_core::theme::get_theme;
use mw_core::{Document, LayoutSettings};
use mw_render::instances::{TOGGLE_HALO, TOGGLE_R};

/// 折叠点的屏幕包围盒（r8 + 1.5 halo），用来断言工具条没有盖住它。
fn toggle_disc(node: egui::Rect, on_left: bool) -> egui::Rect {
    let x = if on_left {
        node.left() - TOGGLE_OFFSET
    } else {
        node.right() + TOGGLE_OFFSET
    };
    egui::Rect::from_center_size(
        Pos2::new(x, node.center().y),
        Vec2::splat((TOGGLE_R + TOGGLE_HALO) * 2.0),
    )
}

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

fn id_of(doc: &Document, text: &str) -> String {
    doc.doc_order()
        .into_iter()
        .find(|id| doc.node(id).is_some_and(|n| n.text == text))
        .unwrap_or_else(|| panic!("找不到节点「{text}」"))
}

/// 无头排一次编辑器版式（字号固定 14，测试只关心行高与基线）。
///
/// 排版会把新字形写进图集增量，而 `TexturesDelta` 析构时断言「增量必须已被平台取走」——
/// 测试里没有平台，所以排版前后各跑一帧、把增量交给 `FullOutput` 后主动丢弃，
/// 否则上下文析构时这个 debug_assert 会直接把测试打断。字体池同样要跑完一帧才存在。
fn edit_galley_headless(text: &str, line_h: f32) -> Arc<Galley> {
    let ctx = egui::Context::default();
    let drain = |ctx: &egui::Context| {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    };
    drain(&ctx);
    let g = edit_galley(
        &ctx,
        text,
        &FontId::proportional(design::ui::LABEL),
        Color32::BLACK,
        f32::INFINITY,
        line_h,
    );
    drain(&ctx);
    g
}

#[test]
fn focus_band_sits_centred_at_the_top_of_the_canvas() {
    let origin = egui::Rect::from_min_size(egui::Pos2::new(40.0, 100.0), egui::vec2(600.0, 400.0));
    let r = focus_band_rect(origin, egui::vec2(200.0, 20.0));
    assert!(
        (r.center().x - origin.center().x).abs() < 0.01,
        "聚焦问题要横向居中"
    );
    assert!((r.top() - (origin.top() + 10.0)).abs() < 0.01, "贴顶 10px");
    assert!((r.height() - 34.0).abs() < 0.01, "行高 20 + 上下各 7");
    assert!((r.width() - 232.0).abs() < 0.01, "文字 200 + 左右各 16");
}

#[test]
fn focus_band_never_spills_out_of_a_narrow_canvas() {
    let origin = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 200.0));
    let r = focus_band_rect(origin, egui::vec2(900.0, 20.0));
    assert!((r.width() - 268.0).abs() < 0.01, "窄画布上按上限收窄");
    assert!(r.left() >= origin.left() - 0.01, "不越过画布左边");
    assert!(r.right() <= origin.right() + 0.01, "不越过画布右边");
}
#[test]
fn middle_button_pans_even_on_empty_canvas() {
    assert_eq!(classify(true, false, true, false, false), DragKind::Pan);
    assert_eq!(classify(false, false, true, false, false), DragKind::Pan);
}

#[test]
fn phrase_suggest_drops_below_the_editor_and_flips_up_near_the_bottom() {
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    // 编辑框在画布中间：下拉贴下方 4px，宽度取编辑框宽（180 起）。
    let anchor = egui::Rect::from_center_size(egui::pos2(400.0, 200.0), egui::vec2(180.0, 30.0));
    let below = phrase_suggest_rect(anchor, 3, canvas);
    assert!((below.top() - (anchor.bottom() + 4.0)).abs() < 0.01);
    assert!((below.height() - (3.0 * 24.0 + 8.0)).abs() < 0.01);
    assert!((below.width() - 180.0).abs() < 0.01);
    assert!((below.left() - anchor.left()).abs() < 0.01);

    // 贴底时翻到编辑框上方，绝不越过画布下沿。
    let low = egui::Rect::from_center_size(egui::pos2(400.0, 580.0), egui::vec2(180.0, 30.0));
    let up = phrase_suggest_rect(low, 3, canvas);
    assert!(up.bottom() <= low.top() - 4.0 + 0.01, "放不下就翻到上方");
    assert!(up.bottom() <= canvas.bottom() - 8.0 + 0.01);
}

#[test]
fn phrase_suggest_stays_inside_a_narrow_canvas() {
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(260.0, 400.0));
    let anchor = egui::Rect::from_center_size(egui::pos2(250.0, 100.0), egui::vec2(180.0, 30.0));
    let r = phrase_suggest_rect(anchor, 2, canvas);
    assert!(r.left() >= canvas.left() + 8.0 - 0.01, "不越过画布左边");
    assert!(r.right() <= canvas.right() - 8.0 + 0.01, "不越过画布右边");
}

#[test]
fn relationship_selection_toggles_and_keeps_the_last_one_as_primary() {
    let a = RelationshipSel {
        from: "n1".into(),
        id: "r1".into(),
        to: "n2".into(),
    };
    let b = RelationshipSel {
        from: "n1".into(),
        id: "r2".into(),
        to: "n3".into(),
    };
    let mut sel = RelationshipSelection::default();
    assert!(sel.is_empty());
    assert_eq!(sel.primary(), None);

    sel.select_only(a.clone());
    assert_eq!(sel.len(), 1);
    assert_eq!(sel.primary(), Some(&a), "单选就是主选");

    sel.toggle(b.clone());
    assert_eq!(sel.len(), 2);
    assert_eq!(sel.primary(), Some(&b), "最后选中的那条才是主选");

    // 已选中的再点一次 = 减选（框选错了撤一条）。
    sel.toggle(a.clone());
    assert_eq!(sel.len(), 1);
    assert_eq!(sel.primary(), Some(&b));
    assert!(sel.contains("n1", "r2"));
    assert!(!sel.contains("n1", "r1"));

    // 重建后失效的线要逐条清掉，不能一条不在就把整组带走。
    sel.retain_alive(|r| r.id == "r2");
    assert_eq!(sel.len(), 1);
    sel.retain_alive(|_| false);
    assert!(sel.is_empty());
}

#[test]
fn concept_link_intent_only_direct_links_exactly_two_concepts() {
    assert_eq!(concept_link_intent(0), ConceptLinkIntent::NeedSelection);
    assert_eq!(concept_link_intent(1), ConceptLinkIntent::Arm);
    assert_eq!(concept_link_intent(2), ConceptLinkIntent::Direct);
    assert_eq!(concept_link_intent(3), ConceptLinkIntent::TooMany);
    assert_eq!(concept_link_intent(9), ConceptLinkIntent::TooMany);
}

#[test]
fn space_drag_pans_even_on_a_node() {
    assert_eq!(classify(false, false, false, true, false), DragKind::Pan);
}

#[test]
fn left_drag_on_empty_canvas_starts_a_marquee() {
    assert_eq!(
        classify(true, false, false, false, false),
        DragKind::Marquee
    );
}

#[test]
fn left_drag_on_a_node_picks_it_up() {
    assert_eq!(classify(false, false, false, false, false), DragKind::Move);
}

#[test]
fn snapping_waits_out_the_60ms_delay() {
    // 候选刚出现（0ms / 30ms）不吸；过了阈值（60ms 及以上）才吸。
    assert!(!snap_ready(None, 100.0), "没有候选就永远不吸");
    assert!(!snap_ready(Some(10.0), 10.0));
    assert!(!snap_ready(Some(10.0), 10.03));
    assert!(snap_ready(Some(10.0), 10.06));
    assert!(snap_ready(Some(10.0), 11.0));
}

#[test]
fn anchors_win_over_the_node_they_sit_on() {
    // 锚点长在节点边上：不先判它，「想拉线」每次都会变成「搬节点」。
    assert_eq!(classify(false, true, false, false, false), DragKind::Link);
}

#[test]
fn editing_never_box_selects_never_picks_a_node_up_and_never_links() {
    // 编辑期拉框会把光标从节点里拽走，搬节点/拉线更会把编辑器连根拔起。
    assert_eq!(classify(true, false, false, false, true), DragKind::Pan);
    assert_eq!(classify(false, false, false, false, true), DragKind::Pan);
    assert_eq!(classify(false, true, false, false, true), DragKind::Pan);
}

#[test]
fn a_left_drag_inside_the_editor_belongs_to_the_editor() {
    // 实测过的缺陷（scripts/v5-sel.ps1）：编辑期在文字里划选，画布抢走同一次按下
    // 去平移，整张图跟着鼠标跑了 94×172px。盒内左键必须让给编辑器。
    let box_ = egui::Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(300.0, 180.0));
    let inside = egui::pos2(150.0, 140.0);
    assert!(editor_owns_press(Some(box_), true, false, false, inside));
    // 盒外（卡片空白、别的节点、别处画布）仍按 `classify` 的老语义走。
    assert!(!editor_owns_press(
        Some(box_),
        true,
        false,
        false,
        egui::pos2(400.0, 140.0)
    ));
    assert!(!editor_owns_press(None, true, false, false, inside));
    // 中键与空格 + 左键仍是平移：编辑长文字时要把视野挪开。
    assert!(!editor_owns_press(Some(box_), true, true, false, inside));
    assert!(!editor_owns_press(Some(box_), true, false, true, inside));
}

#[test]
fn the_motion_pin_clamps_and_ignores_junk() {
    // `MW_MOTION_T` 只在取证时设，读坏了不能把画面搞成空白：`NaN` 过 `clamp` 仍是 `NaN`。
    assert_eq!(parse_pin("0.5"), Some(0.5));
    assert_eq!(parse_pin(" 2 "), Some(1.0));
    assert_eq!(parse_pin("-1"), Some(0.0));
    assert_eq!(parse_pin("1"), Some(1.0));
    assert_eq!(parse_pin(""), None);
    assert_eq!(parse_pin("half"), None);
    assert_eq!(parse_pin("NaN"), None);
    assert_eq!(parse_pin("inf"), None);
}

#[test]
fn context_pos_reads_two_numbers_or_nothing() {
    // `MW_CONTEXT_POS` 只挪菜单的请求点；读坏了要当「没设」，退回默认点，
    // 不能把菜单开到画面外（那会拍出一张空图还看不出来）。
    let p = parse_pos("380, 690").expect("两个数都在");
    assert_eq!((p.x, p.y), (380.0, 690.0));
    let p = parse_pos(" 12.5 ,7 ").expect("首尾空白要吃掉");
    assert_eq!((p.x, p.y), (12.5, 7.0));
    assert_eq!(parse_pos("380"), None, "只有一个数");
    assert_eq!(parse_pos("380,690,1"), None, "三个数");
    assert_eq!(parse_pos("a,b"), None);
    assert_eq!(parse_pos("NaN,10"), None);
    assert_eq!(parse_pos(""), None);
}

#[test]
fn edit_time_enter_and_tab_build_the_next_node_on_a_mind_map_only() {
    // `docs/07` §3.2：编辑态 `Enter` 提交后「触发语义（新建同级）」，`Tab` 提交后「新建子节点」。
    // 建树时手不离键盘：敲完一个名字直接按下一个键，比「先 Esc 再 Tab」快一倍。
    assert_eq!(commit_follow_up(true, false, false), Some(AddKind::Sibling));
    assert_eq!(commit_follow_up(false, true, false), Some(AddKind::Child));
    // 两个键同时到（不该发生）时 `Enter` 优先，且不会同时建两个。
    assert_eq!(commit_follow_up(true, true, false), Some(AddKind::Sibling));
    assert_eq!(commit_follow_up(false, false, false), None);
    // 概念图没有「同级」这层结构：两个键都只提交，与 CmapTools 的就地改名一致。
    assert_eq!(commit_follow_up(true, false, true), None);
    assert_eq!(commit_follow_up(false, true, true), None);
}

#[test]
fn survivors_keep_their_order_and_primary_after_a_relayout() {
    let mut doc = doc_with(&["甲", "乙", "丙"]);
    let old = vec![id_of(&doc, "甲"), id_of(&doc, "丙")];
    let before = scene_of(&doc);
    // 前提：索引真的会变——否则这条断言证明不了「搬运的是 id」。
    let jia_before = before.find(&old[0]).expect("甲在场景里");
    let bing_before = before.find(&old[1]).expect("丙在场景里");

    // 删掉「乙」——前序布局整体前移，丙的索引会变，只有 id 还作数。
    let b = id_of(&doc, "乙");
    doc.apply(&Command::DeleteNodes { ids: vec![b] }).unwrap();
    let after = scene_of(&doc);
    let restored = restore_selection(&old, &after, &doc.root_id);

    assert_eq!(restored.len(), 2, "两个都还在");
    assert_ne!(
        after.find(&old[1]),
        Some(bing_before),
        "丙的索引必须确实前移了，这条测试才有意义"
    );
    assert_eq!(
        after.ids[restored[0] as usize], old[0],
        "甲仍在组首（原索引 {jia_before}）"
    );
    assert_eq!(
        after.ids[restored[1] as usize], old[1],
        "末尾仍是丙，主选没被换掉"
    );
}

#[test]
fn a_selection_that_vanishes_falls_back_to_the_root() {
    let mut doc = doc_with(&["甲", "乙"]);
    let old = vec![id_of(&doc, "甲")];
    let a = id_of(&doc, "甲");
    doc.apply(&Command::DeleteNodes { ids: vec![a] }).unwrap();
    let after = scene_of(&doc);

    let restored = restore_selection(&old, &after, &doc.root_id);
    assert_eq!(restored.len(), 1);
    assert_eq!(after.ids[restored[0] as usize], doc.root_id);
}

#[test]
fn an_empty_selection_stays_empty() {
    let doc = doc_with(&["甲"]);
    let after = scene_of(&doc);
    assert!(restore_selection(&[], &after, &doc.root_id).is_empty());
}

#[test]
fn a_partly_vanished_group_keeps_the_survivors() {
    let mut doc = doc_with(&["甲", "乙"]);
    let old = vec![id_of(&doc, "乙"), id_of(&doc, "甲")];
    let a = id_of(&doc, "甲");
    doc.apply(&Command::DeleteNodes { ids: vec![a] }).unwrap();
    let after = scene_of(&doc);

    let restored = restore_selection(&old, &after, &doc.root_id);
    assert_eq!(restored.len(), 1, "只剩乙，不该回落根");
    assert_eq!(after.ids[restored[0] as usize], id_of(&doc, "乙"));
}

#[test]
fn hover_bar_lifts_above_the_card_badge() {
    let node = egui::Rect::from_min_size(Pos2::new(100.0, 120.0), Vec2::new(160.0, 44.0));
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let (plus, more) = hover_bar_rects(node, canvas, true).expect("有角标也应放得下");
    let badge_center = card_badge_center(node.right_top(), 6.5);
    let badge = egui::Rect::from_center_size(badge_center, Vec2::splat(6.5 * 2.0 + 6.0));
    for r in [plus, more] {
        assert!(
            !r.intersects(badge),
            "工具条压住卡片角标：{r:?} vs {badge:?}"
        );
    }
    assert!(plus.bottom() < badge.top(), "工具条应整体抬到角标上方");
}

/// 视图跟随（W50）：`reveal_center` 的输出必须让目标盒落在视口内，
/// 留出 48 CSS px 安全边；已在视口内时相机一格都不动。
#[test]
fn reveal_keeps_the_target_inside_the_padded_viewport() {
    let vp = [800.0f32, 600.0];
    let zoom = 1.0f32;
    let pad = 48.0;
    let visible = |c: Point, r: Rect| {
        let hw = vp[0] / (2.0 * zoom);
        let hh = vp[1] / (2.0 * zoom);
        r.x >= c.x - hw + pad - 1e-3
            && r.right() <= c.x + hw - pad + 1e-3
            && r.y >= c.y - hh + pad - 1e-3
            && r.bottom() <= c.y + hh - pad + 1e-3
    };

    // 1) 目标已在屏内：相机保持不动。
    let inside = Rect::new(0.0, 0.0, 120.0, 40.0);
    assert_eq!(
        reveal_center(Point::new(0.0, 0.0), zoom, vp, inside),
        Point::ZERO
    );

    // 2) 目标在右侧屏外：最小右移后右缘刚好贴住安全边。
    let right = Rect::new(2000.0, -20.0, 100.0, 40.0);
    let c = reveal_center(Point::ZERO, zoom, vp, right);
    assert!((c.x - 1748.0).abs() < 1e-3, "相机右移量不对：{}", c.x);
    assert!(visible(c, right));

    // 3) 目标在下方屏外：上下安全边同样成立。
    let below = Rect::new(-40.0, 900.0, 80.0, 40.0);
    let c = reveal_center(Point::ZERO, zoom, vp, below);
    assert!((c.y - 688.0).abs() < 1e-3, "相机下移量不对：{}", c.y);
    assert!(visible(c, below));

    // 4) 目标比视口还大：居中是唯一稳定的读法。
    let huge = Rect::new(0.0, 0.0, 1000.0, 900.0);
    let c = reveal_center(Point::ZERO, zoom, vp, huge);
    assert_eq!(c, Point::new(huge.center().x, huge.center().y));
}

/// 悬停工具条：默认在节点右侧、贴右上角之上；命中区**不压节点盒**，也不压折叠点。
#[test]
fn hover_bar_sits_outside_the_node_and_clears_the_fold_toggle() {
    let node = egui::Rect::from_min_size(Pos2::new(100.0, 120.0), Vec2::new(160.0, 44.0));
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let (plus, more) = hover_bar_rects(node, canvas, false).expect("常规位置应放得下");
    for r in [plus, more] {
        assert!(
            (r.width() - HOVER_BTN).abs() < 1e-4 && (r.height() - HOVER_BTN).abs() < 1e-4,
            "按钮必须是 24×24：{r:?}"
        );
        assert!(!r.intersects(node), "命中区压住了节点盒：{r:?}");
        assert!(canvas.contains_rect(r), "按钮跑出画布：{r:?}");
    }
    // 离节点边至少 DROP_PAD_X：低于它，点击会先被画布判成「按在节点上」。
    assert!(
        plus.left() >= node.right() + mw_ui::drag::DROP_PAD_X,
        "工具条离节点太近：{} < {}",
        plus.left() - node.right(),
        mw_ui::drag::DROP_PAD_X
    );
    assert_eq!(HOVER_GAP, design::space::SM, "缝隙要落在 4px 网格上");
    // 纵向贴在节点上方，和上缘留 HOVER_GAP_V——不能垂直居中，
    // 否则 22px 的按钮会正好盖住右侧中线的折叠圆环。
    assert!(
        (node.top() - plus.bottom() - HOVER_GAP_V).abs() < 1e-3,
        "工具条没有贴在节点上缘之上：节点 top {} / 按钮 bottom {}",
        node.top(),
        plus.bottom()
    );
    let toggle = toggle_disc(node, false);
    for r in [plus, more] {
        assert!(
            !r.intersects(toggle),
            "工具条压住了折叠点：{r:?} vs {toggle:?}"
        );
    }
    // 两个按钮之间留 4px，不能贴成一整块。
    assert!((more.left() - plus.right() - HOVER_BTN_GAP).abs() < 1e-3);
}

/// 节点贴近画布右缘时镜像到左侧：宁可换边，也不把按钮叠在节点上。
#[test]
fn hover_bar_mirrors_to_the_left_when_the_right_edge_is_crowded() {
    let node = egui::Rect::from_min_size(Pos2::new(700.0, 200.0), Vec2::new(80.0, 40.0));
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let (plus, more) = hover_bar_rects(node, canvas, false).expect("左侧应放得下");
    assert!(!plus.intersects(node) && !more.intersects(node));
    assert!(
        more.right() <= node.left() - mw_ui::drag::DROP_PAD_X,
        "镜像后仍离节点太近：{}",
        node.left() - more.right()
    );
    assert!(canvas.contains_rect(plus) && canvas.contains_rect(more));
    // 左侧分支的折叠点在左缘中线，镜像后的工具条同样不能压住它。
    let toggle = toggle_disc(node, true);
    assert!(!plus.intersects(toggle) && !more.intersects(toggle));
}

/// 两侧都放不下（节点比画布还宽）时宁可不画，也不把命中区压到节点上。
#[test]
fn hover_bar_disappears_when_neither_side_has_room() {
    let node = egui::Rect::from_min_size(Pos2::new(-40.0, 80.0), Vec2::new(280.0, 40.0));
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 400.0));
    assert!(hover_bar_rects(node, canvas, false).is_none());
}

/// 画布比一个按钮还矮时不能 panic（`clamp` 的 min > max 会炸），直接不画。
#[test]
fn hover_bar_disappears_on_a_canvas_shorter_than_a_button() {
    let node = egui::Rect::from_min_size(Pos2::new(20.0, 0.0), Vec2::new(60.0, 10.0));
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 10.0));
    assert!(hover_bar_rects(node, canvas, false).is_none());
}

/// 从节点移到按钮要跨过缝隙；悬停保持区必须把整段缝隙包进去，
/// 否则指针走到一半工具条就消失（点击根本到不了）。
#[test]
fn hover_zone_covers_the_gap_between_the_node_and_its_buttons() {
    let node = egui::Rect::from_min_size(Pos2::new(100.0, 120.0), Vec2::new(160.0, 44.0));
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let (plus, more) = hover_bar_rects(node, canvas, false).unwrap();
    let bar = HoverBar {
        node: 0,
        node_rect: node,
        plus,
        more,
        compact: false,
    };
    let zone = bar.hover_zone();
    // 节点上缘与按钮下缘之间的正中点（工具条在节点上方，这一步是斜着走过去的）
    let mid = Pos2::new(plus.center().x, (node.top() + plus.bottom()) * 0.5);
    assert!(zone.contains(mid), "缝隙不在悬停区里：{mid:?}");
    assert!(zone.contains(more.center()));
    assert_eq!(bar.buttons(), plus.union(more));
}

/// 选中态常显只出「＋」：buttons 不包含「⋯」，点击区不会在不可见按钮上。
#[test]
fn compact_hover_bar_only_covers_the_plus_button() {
    let node = egui::Rect::from_min_size(Pos2::new(100.0, 120.0), Vec2::new(160.0, 44.0));
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let (plus, more) = hover_bar_rects(node, canvas, true).unwrap();
    let bar = HoverBar {
        node: 0,
        node_rect: node,
        plus,
        more: egui::Rect::NOTHING,
        compact: true,
    };
    assert_eq!(bar.buttons(), plus);
    assert!(bar.hover_zone().contains(plus.center()));
    assert!(!bar.buttons().contains(more.center()));
}

#[test]
fn annotation_click_toggles_with_shift_and_replaces_without() {
    let a = AnnotationSel {
        parent: "p".into(),
        kind: AnnotationKind::Summary,
        id: "s1".into(),
    };
    let b = AnnotationSel {
        parent: "p".into(),
        kind: AnnotationKind::Boundary,
        id: "b1".into(),
    };
    let mut list = Vec::new();
    annotation_click(&mut list, a.clone(), false);
    annotation_click(&mut list, b.clone(), true);
    assert_eq!(list, vec![a.clone(), b.clone()], "Shift 点击是追加");
    annotation_click(&mut list, a.clone(), true);
    assert_eq!(list, vec![b.clone()], "再 Shift 点同一条要移出");
    annotation_click(&mut list, a.clone(), false);
    assert_eq!(list, vec![a], "普通点击把多选收成单选");
}

/// 跨度拖拽的三条纯逻辑：区间归一、拖动不空、最近兄弟。
#[test]
fn span_bounds_use_child_order_not_target_order() {
    let children: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
    // 文档里乱序的 targets 也要归一成 [1, 2]
    let targets = vec!["c".to_string(), "b".to_string()];
    assert_eq!(span_bounds(&targets, &children), Some((1, 2)));
    assert_eq!(
        span_bounds(&["x".to_string()], &children),
        None,
        "找不到就不能拖"
    );
}

#[test]
fn span_range_never_empties_and_clamps_to_the_fixed_end() {
    // 上端往下拖过终点：夹在终点，区间仍是一行
    assert_eq!(span_range(1, 3, SpanEnd::Top, 9), (3, 3));
    // 下端往上拖过起点：夹在起点
    assert_eq!(span_range(1, 3, SpanEnd::Bottom, 0), (1, 1));
    // 正常扩/缩
    assert_eq!(span_range(1, 3, SpanEnd::Top, 0), (0, 3));
    assert_eq!(span_range(1, 3, SpanEnd::Bottom, 5), (1, 5));
}

#[test]
fn span_candidate_picks_the_nearest_vertical_sibling() {
    let centers = [(0usize, 10.0f32), (1, 40.0), (2, 90.0)];
    assert_eq!(span_candidate(&centers, 12.0), Some(0));
    assert_eq!(span_candidate(&centers, 44.0), Some(1));
    assert_eq!(
        span_candidate(&centers, 200.0),
        Some(2),
        "超出最远项时取最后一项"
    );
    assert_eq!(span_candidate(&[], 0.0), None);
}

fn add_child(doc: &mut Document, parent: &str, text: &str) -> String {
    let cs = doc
        .apply(&Command::AddNode {
            parent: parent.to_string(),
            index: None,
            text: text.into(),
            style: None,
        })
        .expect("添加节点");
    cs.after
        .iter()
        .find(|n| n.text == text)
        .map(|n| n.id.clone())
        .expect("新节点应在变更集里")
}

fn texts(doc: &Document, parent: &str) -> Vec<String> {
    doc.node(parent)
        .map(|n| {
            n.children
                .iter()
                .filter_map(|c| doc.node(c).map(|n| n.text.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// 概要/边界只圈「同一父级」的选中项，且按文档里的兄弟顺序——
/// 跨父级的选中不能把整条命令拖失败，也不能打乱括号的左右端。
#[test]
fn annotations_cover_same_parent_selection_in_document_order() {
    let mut doc = doc_with(&["甲", "乙", "丙"]);
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let bing = id_of(&doc, "丙");
    let child = add_child(&mut doc, &jia, "甲子");
    let picked = vec![yi.clone(), child, jia.clone()];
    let (parent, targets) = annotation_targets(&doc, &picked, &yi).expect("主选有父级，应当能圈");
    assert_eq!(parent, doc.root_id);
    assert_eq!(
        targets,
        vec![jia.clone(), yi.clone()],
        "只留同一父级、按兄弟顺序；子节点的子节点要丢掉"
    );
    assert!(!targets.contains(&bing), "没选中的兄弟不该被圈进来");
}

/// 松到空白处：树内节点变自由主题并落在拖到的位置，本来就是自由主题的只挪位置，
/// 主根不参与。一条 Batch 覆盖全部，撤销一次就该全回去。
#[test]
fn dropping_on_blank_detaches_trees_and_moves_floating_topics() {
    let mut doc = doc_with(&["甲"]);
    let root = doc.root_id.clone();
    let jia = id_of(&doc, "甲");
    doc.apply(&Command::AddFloatingTopic {
        text: "便签".into(),
        x: 10.0,
        y: 10.0,
    })
    .expect("新建自由主题");
    let fid = doc
        .doc_order()
        .into_iter()
        .find(|id| {
            doc.node(id)
                .is_some_and(|n| n.parent_id.is_none() && *id != root)
        })
        .expect("应当有且只有一个自由主题");

    let dropped = vec![
        (jia.clone(), Rect::new(100.0, 50.0, 80.0, 30.0)),
        (fid.clone(), Rect::new(200.0, 60.0, 80.0, 30.0)),
    ];
    let cmd = detach_commands(&doc, &dropped, 12.0, -7.0).expect("两个节点都能落位");
    doc.apply(&cmd).expect("批量落位应当成功");

    let jia_node = doc.node(&jia).expect("甲还在");
    assert_eq!(jia_node.parent_id, None);
    assert_eq!(jia_node.pos.as_ref().unwrap().x, 112.0);
    assert_eq!(jia_node.pos.as_ref().unwrap().y, 43.0);
    let f = doc.node(&fid).expect("便签还在");
    assert_eq!(f.pos.as_ref().unwrap().x, 212.0);
    assert_eq!(f.pos.as_ref().unwrap().y, 53.0);

    // 主根不参与：它没有父级可脱离
    let only_root = vec![(root.clone(), Rect::new(0.0, 0.0, 10.0, 10.0))];
    assert!(detach_commands(&doc, &only_root, 5.0, 5.0).is_none());
    // 拖到面板上松手（指针不在画布里）是一次放弃
    let outside = vec![(fid.clone(), Rect::new(0.0, 0.0, 10.0, 10.0))];
    assert!(
        detach_on_blank_command(&doc, &outside, false, 5.0, 5.0).is_none(),
        "指针离开画布后松手不该改动结构"
    );
    assert!(detach_on_blank_command(&doc, &outside, true, 5.0, 5.0).is_some());
    assert!(doc.validate().is_ok());
}

/// Ctrl+拖拽：原件留在原地，副本连着子树插到落点；一次撤销全部回退。
#[test]
fn ctrl_drag_copies_the_subtree_instead_of_moving_it() {
    let mut doc = doc_with(&["甲", "乙"]);
    let root = doc.root_id.clone();
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let _child = add_child(&mut doc, &jia, "甲子");
    let before = doc.nodes.len();

    let roots = vec![jia.clone()];
    let targets = vec![(yi.clone(), None)];
    let (cmd, tops) = copy_into_command(&doc, &roots, &targets).expect("应当生成复制命令");
    doc.apply(&cmd).expect("复制应当成功");

    assert_eq!(doc.nodes.len(), before + 2, "甲 + 甲子 各复制一份");
    assert_eq!(
        doc.node(&jia).unwrap().parent_id.as_deref(),
        Some(root.as_str()),
        "原件必须留在原来的父级"
    );
    let copy = doc.node(&tops[0]).expect("副本根");
    assert_eq!(copy.text, "甲");
    assert_eq!(copy.children.len(), 1, "副本要带着子树");
    assert_eq!(doc.node(&copy.children[0]).unwrap().text, "甲子");
    assert_eq!(doc.node(&yi).unwrap().children, vec![tops[0].clone()]);

    doc.undo().expect("一次撤销");
    assert_eq!(doc.nodes.len(), before);
    assert!(doc.validate().is_ok());
}

/// Ctrl+拖到空白：复制成自由主题，原件留在树上；一条 Batch 一次撤销。
#[test]
fn ctrl_drag_to_blank_copies_as_a_floating_topic() {
    let mut doc = doc_with(&["甲", "乙"]);
    let root = doc.root_id.clone();
    let jia = id_of(&doc, "甲");
    let _child = add_child(&mut doc, &jia, "甲子");
    let before = doc.nodes.len();

    let dropped = vec![(jia.clone(), Rect::new(100.0, 50.0, 80.0, 30.0))];
    let (cmd, tops) = copy_floating_command(&doc, &dropped, 12.0, -7.0).expect("应当生成命令");
    doc.apply(&cmd).expect("复制并落位应当成功");

    assert_eq!(doc.nodes.len(), before + 2, "甲 + 甲子 各复制一份");
    let copy = doc.node(&tops[0]).expect("副本根");
    assert_eq!(copy.parent_id, None, "副本必须是自由主题");
    assert_eq!(copy.pos.as_ref().unwrap().x, 112.0);
    assert_eq!(copy.pos.as_ref().unwrap().y, 43.0);
    assert_eq!(copy.children.len(), 1, "副本带着子树");
    assert_eq!(
        doc.node(&jia).unwrap().parent_id.as_deref(),
        Some(root.as_str()),
        "原件必须留在原来的父级"
    );

    doc.undo().expect("一次撤销");
    assert_eq!(doc.nodes.len(), before);
    assert!(doc.validate().is_ok());
}

/// CM-11：概念复制不带走父子结构，只保留副本之间的命题线；粘贴整体偏移、一次撤销。
#[test]
fn concept_copy_preserves_propositions_but_never_parent_links() {
    let mut doc = Document::new("主概念");
    doc.apply(&Command::SetLayout {
        layout: LayoutSettings {
            kind: "conceptMap".into(),
            ..LayoutSettings::default()
        },
    })
    .unwrap();
    let a = doc.root_id.clone();
    doc.apply(&Command::MoveConcept {
        id: a.clone(),
        x: 100.0,
        y: 120.0,
    })
    .unwrap();
    doc.apply(&Command::AddFloatingTopic {
        text: "乙".into(),
        x: 360.0,
        y: 120.0,
    })
    .unwrap();
    let b = id_of(&doc, "乙");
    doc.apply(&Command::AddRelationship {
        from: a.clone(),
        to: b.clone(),
        label: Some("导致".into()),
    })
    .unwrap();
    let before_count = doc.node_count();

    let nodes = concept_snapshot(&doc, &[a.clone(), b.clone()]);
    assert_eq!(nodes.len(), 2, "两个概念各复制一份");
    assert!(nodes.iter().all(|n| n.parent_id.is_none()));
    assert!(nodes.iter().all(|n| n.children.is_empty()));

    let (cmd, tops) =
        concept_clone_command(&nodes, (24.0, 18.0), (0.0, 0.0)).expect("应生成独立概念批");
    doc.apply(&cmd).expect("概念批应可插入");
    assert_eq!(doc.node_count(), before_count + 2);
    assert!(doc.validate().is_ok(), "{:?}", doc.validate());
    let copy_a = doc
        .nodes
        .values()
        .find(|n| n.id == tops[0])
        .expect("副本甲");
    let copy_b = doc.nodes.get(&tops[1]).expect("副本乙");
    assert_eq!(copy_a.parent_id, None);
    assert!(copy_a.children.is_empty(), "概念图粘贴不能长出子树");
    assert_eq!(
        copy_a.pos.as_ref().map(|p| (p.x, p.y)),
        Some((124.0, 138.0)),
        "副本位置按拖拽位移整体偏移"
    );
    assert_eq!(copy_a.relationships.len(), 1);
    assert_eq!(copy_a.relationships[0].to, copy_b.id);
    assert_eq!(
        copy_a.relationships[0].label.as_deref(),
        Some("导致"),
        "命题短语要跟着副本走"
    );
    assert_eq!(
        doc.node(&a).unwrap().pos.as_ref().map(|p| (p.x, p.y)),
        Some((100.0, 120.0)),
        "原件不能被 Ctrl 复制挪走"
    );

    doc.undo().expect("一次撤销");
    assert_eq!(doc.node_count(), before_count);
    assert!(doc.validate().is_ok());
}

/// 从树里复制来的节点粘到概念图：全部变成独立概念，不制造父子关系。
#[test]
fn tree_clipboard_pastes_as_independent_concepts_without_parents() {
    let mut source = doc_with(&["甲", "乙"]);
    let a = id_of(&source, "甲");
    let _child = add_child(&mut source, &a, "甲子");
    let nodes = copy_subtrees(&source, std::slice::from_ref(&a));
    assert!(nodes.len() >= 2, "来源是带子树的树节点");

    let mut cm = Document::new("主概念");
    cm.apply(&Command::SetLayout {
        layout: LayoutSettings {
            kind: "conceptMap".into(),
            ..LayoutSettings::default()
        },
    })
    .unwrap();
    cm.apply(&Command::MoveConcept {
        id: cm.root_id.clone(),
        x: 0.0,
        y: 0.0,
    })
    .unwrap();
    let (cmd, tops) = concept_clone_command(&nodes, (12.0, 12.0), (200.0, 200.0))
        .expect("树快照也要能转成概念批");
    cm.apply(&cmd).expect("概念图应接受扁平化后的批次");
    assert_eq!(tops.len(), nodes.len());
    for id in &tops {
        let n = cm.node(id).expect("副本存在");
        assert_eq!(n.parent_id, None, "粘贴不能把树带进概念图");
        assert!(n.children.is_empty());
        assert!(n.pos.is_some());
    }
    assert!(cm.validate().is_ok(), "{:?}", cm.validate());
}

/// 边界命中：点进边界但不在节点盒子里 → 选中边界；点在节点上 → 交给节点。
#[test]
fn clicking_inside_a_boundary_selects_it_unless_a_node_is_there() {
    use mw_core::model::AnnotationKind;
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "根".into(),
    })
    .unwrap();
    let a = add_child(&mut doc, &root, "甲");
    doc.apply(&Command::AddBoundary {
        parent: root.clone(),
        targets: vec![a.clone()],
        color: Some("#f6212d".into()),
    })
    .unwrap();
    let theme = get_theme("snow");
    let scene = build_scene(
        &doc,
        &doc.root_id,
        &doc.meta.layout,
        &theme,
        &MonoMeasurer,
        Default::default(),
    );
    let b = scene.boundaries.first().expect("场景里应有边界");
    let viewport = [1200.0, 800.0];
    let camera = Camera::new(b.rect.center(), 1.0, 1.0);
    let to_local = |x: f32, y: f32| camera.world_to_screen(Point::new(x, y), viewport);
    // 边界内、节点外：靠近边界左上角
    let p = to_local(b.rect.x + 3.0, b.rect.y + 3.0);
    let hit = annotation_hit(&scene, &camera, p, viewport);
    assert_eq!(
        hit.map(|h| (h.kind, h.parent)),
        Some((AnnotationKind::Boundary, root.clone())),
        "边界内点空白应选中边界"
    );
    // 节点盒子正中：应该让给节点
    let n = scene.find(&a).expect("节点在场景里");
    let r = scene.rect(n);
    let p = to_local(r.center().x, r.center().y);
    assert!(
        annotation_hit(&scene, &camera, p, viewport).is_none(),
        "点在卡片上不该被边界抢走"
    );
    // 远处空白：什么都不选
    let p = to_local(b.rect.right() + 200.0, b.rect.bottom() + 200.0);
    assert!(annotation_hit(&scene, &camera, p, viewport).is_none());
}

/// 贴边方向：右缘往右推、左缘往左推、中间不动。
#[test]
fn autoscroll_pushes_away_from_the_nearest_edge() {
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(800.0, 600.0));
    let (edge, step) = (26.0, 14.0);
    // 右缘内侧
    assert_eq!(
        autoscroll_step(egui::pos2(880.0, 300.0), rect, edge, step),
        Vec2::new(step, 0.0)
    );
    // 左缘
    assert_eq!(
        autoscroll_step(egui::pos2(110.0, 300.0), rect, edge, step),
        Vec2::new(-step, 0.0)
    );
    // 下缘（同时贴右下角：两轴都要推）
    assert_eq!(
        autoscroll_step(egui::pos2(890.0, 640.0), rect, edge, step),
        Vec2::new(step, step)
    );
    // 上缘
    assert_eq!(
        autoscroll_step(egui::pos2(500.0, 60.0), rect, edge, step),
        Vec2::new(0.0, -step)
    );
    // 正中：不动
    assert_eq!(
        autoscroll_step(egui::pos2(500.0, 300.0), rect, edge, step),
        Vec2::ZERO
    );
}

/// 路径：不含自己、从根往下、太长时间只留尾部。
#[test]
fn a_node_path_lists_ancestors_from_the_root() {
    // Document::new 只设 meta.title，根节点文字是空的：用 SetText 补上。
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "根".into(),
    })
    .expect("改根文字");
    let jia = add_child(&mut doc, &root, "甲");
    let child = add_child(&mut doc, &jia, "甲子");
    assert_eq!(path_of(&doc, &child), "根 › 甲");
    assert_eq!(path_of(&doc, &jia), "根");
    assert_eq!(path_of(&doc, &root), "");
    // 空标题的祖先按占位符保留
    let mut blank = Document::new("空根");
    let broot = blank.root_id.clone();
    let bjia = add_child(&mut blank, &broot, "甲");
    assert_eq!(path_of(&blank, &bjia), "（未命名）");
    // 长路径截断：只保留尾部，并以省略号开头
    let mut deep = Document::new("根");
    let mut cur = deep.root_id.clone();
    deep.apply(&Command::SetText {
        id: cur.clone(),
        text: "根".into(),
    })
    .expect("改根文字");
    for i in 0..8 {
        let cs = deep
            .apply(&Command::AddNode {
                parent: cur.clone(),
                index: None,
                text: format!("层级{i}"),
                style: None,
            })
            .expect("加节点");
        // ChangeSet 里可能还带着被触碰的父节点：按文字挑出新建的那个。
        cur = cs
            .after
            .iter()
            .find(|n| n.text == format!("层级{i}"))
            .map(|n| n.id.clone())
            .expect("新节点在 ChangeSet 里");
    }
    let long = path_of(&deep, &cur);
    assert!(long.starts_with("… "), "过长时要截断：{long}");
    assert!(long.ends_with("层级6"), "截断保留离自己最近的父级：{long}");
    assert!(long.chars().count() <= 42);
}

/// 选中子树摘下后必须是一份**合法独立文档**，并且文本序列化只出这棵子树。
#[test]
fn a_subtree_becomes_a_valid_standalone_document() {
    let mut doc = doc_with(&["甲", "乙"]);
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let _child = add_child(&mut doc, &jia, "甲子");
    doc.apply(&Command::AddRelationship {
        from: jia.clone(),
        to: yi.clone(),
        label: None,
    })
    .expect("关系线");

    let sub = subtree_document(&doc, &jia).expect("应当能摘出子树");
    assert_eq!(sub.root_id, jia);
    assert_eq!(sub.nodes.len(), 2, "甲 + 甲子");
    assert!(sub.nodes[&jia].parent_id.is_none(), "摘出来的根是主根");
    assert!(
        sub.nodes[&jia].relationships.is_empty(),
        "指向子树外部的关系线必须剪掉"
    );
    assert_eq!(sub.meta.title, sub.nodes[&jia].text);
    assert!(sub.validate().is_ok());

    let md = crate::export::Format::Markdown
        .text(&sub)
        .expect("Markdown");
    assert!(md.contains("甲"), "子树内容应当在：{md}");
    assert!(!md.contains("乙"), "兄弟分支不该出现：{md}");
}

/// 根节点没有父级，概要和边界都该拒绝；主选有父级时才放行。
#[test]
fn the_root_cannot_be_annotated() {
    let doc = doc_with(&["甲"]);
    let root = doc.root_id.clone();
    let jia = id_of(&doc, "甲");
    assert!(annotation_targets(&doc, std::slice::from_ref(&root), &root).is_none());
    assert!(annotation_targets(&doc, std::slice::from_ref(&jia), &jia).is_some());
}

/// 注解快捷键只认**精确**的 `Ctrl+B` / `Ctrl+]`：普通 `B` / `]` 是落字，
/// 带 Shift 的变体不是这个语义。
#[test]
fn annotation_shortcuts_map_only_exact_ctrl_b_and_bracket() {
    assert_eq!(
        annotation_add_for_key(Modifiers::COMMAND, Key::B),
        Some(AnnotationAdd::Boundary)
    );
    assert_eq!(
        annotation_add_for_key(Modifiers::COMMAND, Key::CloseBracket),
        Some(AnnotationAdd::Summary)
    );
    assert_eq!(annotation_add_for_key(Modifiers::NONE, Key::B), None);
    assert_eq!(
        annotation_add_for_key(Modifiers::NONE, Key::CloseBracket),
        None
    );
    assert_eq!(
        annotation_add_for_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::B),
        None,
        "Ctrl+Shift+B 不建边界"
    );
    assert_eq!(annotation_add_for_key(Modifiers::COMMAND, Key::A), None);
}

/// 键盘失败的两条原因必须分得开：没选 → no-selection；选了根 → no-parent。
#[test]
fn annotation_shortcut_errors_distinguish_no_selection_from_root() {
    let doc = doc_with(&["甲"]);
    let root = doc.root_id.clone();
    let jia = id_of(&doc, "甲");
    assert_eq!(
        annotation_shortcut_error(&doc, &[], None),
        Some(AnnotationShortcutError::NoSelection)
    );
    assert_eq!(
        annotation_shortcut_error(&doc, std::slice::from_ref(&root), Some(&root)),
        Some(AnnotationShortcutError::NoParent),
        "根主题没有同一父级可圈"
    );
    assert_eq!(
        annotation_shortcut_error(&doc, std::slice::from_ref(&jia), Some(&jia)),
        None
    );
}

#[test]
fn copying_a_parent_and_child_only_copies_the_parent_subtree_once() {
    let mut doc = doc_with(&["甲", "乙"]);
    let jia = id_of(&doc, "甲");
    let jia1 = add_child(&mut doc, &jia, "甲子");
    let yi = id_of(&doc, "乙");
    let roots = selected_roots(&doc, &[jia1, jia.clone(), yi.clone()]);
    assert_eq!(roots, vec![jia, yi], "子节点被父节点覆盖，且顺序按文档前序");
}

#[test]
fn remap_renews_ids_and_rewires_internal_references() {
    let mut doc = doc_with(&["甲", "乙"]);
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let c1 = add_child(&mut doc, &jia, "子一");
    let c2 = add_child(&mut doc, &jia, "子二");
    doc.apply(&Command::AddBoundary {
        parent: jia.clone(),
        targets: vec![c1.clone()],
        color: None,
    })
    .unwrap();
    doc.apply(&Command::AddSummary {
        parent: jia.clone(),
        targets: vec![c2.clone()],
        text: "概要".into(),
    })
    .unwrap();
    doc.apply(&Command::AddRelationship {
        from: c1.clone(),
        to: c2.clone(),
        label: None,
    })
    .unwrap();
    let mut snapshot = copy_subtrees(&doc, std::slice::from_ref(&jia));
    snapshot[0].pos = Some(mw_core::Position { x: 40.0, y: 50.0 });
    let (nodes, tops) = remap_subtrees(&snapshot, &yi);
    assert_eq!(nodes.len(), 3);
    assert_eq!(tops.len(), 1);
    assert!(nodes
        .iter()
        .all(|n| n.id != jia && n.id != c1 && n.id != c2));
    let top = nodes.iter().find(|n| n.id == tops[0]).expect("顶层节点");
    assert_eq!(top.parent_id.as_deref(), Some(yi.as_str()));
    assert!(top.pos.is_none(), "粘贴不携带自由坐标");
    assert_eq!(top.children.len(), 2);
    let new_c1 = nodes.iter().find(|n| n.text == "子一").expect("子一");
    let new_c2 = nodes.iter().find(|n| n.text == "子二").expect("子二");
    assert_eq!(top.boundaries.len(), 1);
    assert_eq!(top.boundaries[0].targets, vec![new_c1.id.clone()]);
    assert_ne!(
        top.boundaries[0].id,
        doc.node(&jia).unwrap().boundaries[0].id
    );
    assert_eq!(top.summaries.len(), 1);
    assert_eq!(top.summaries[0].targets, vec![new_c2.id.clone()]);
    assert_eq!(new_c1.relationships.len(), 1);
    assert_eq!(new_c1.relationships[0].to, new_c2.id);
}

#[test]
fn paste_expands_a_collapsed_target_first() {
    let mut doc = doc_with(&["甲", "乙"]);
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let child = add_child(&mut doc, &jia, "内容");
    doc.apply(&Command::SetCollapsed {
        id: jia.clone(),
        collapsed: true,
    })
    .unwrap();
    let snapshot = copy_subtrees(&doc, std::slice::from_ref(&child));
    let (cmd, tops) = paste_command(&doc, &snapshot, &jia).expect("粘贴命令");
    doc.apply(&cmd).unwrap();
    assert!(!doc.node(&jia).unwrap().collapsed, "目标先展开");
    assert_eq!(
        texts(&doc, &jia),
        vec!["内容".to_string(), "内容".to_string()]
    );
    assert_ne!(tops[0], child, "粘贴的是新 ID，不是原节点");
    assert_eq!(
        doc.node(&tops[0]).unwrap().parent_id.as_deref(),
        Some(jia.as_str())
    );
    assert!(doc.node(&yi).is_some());
}

#[test]
fn reveal_commands_open_collapsed_ancestors_root_to_near_in_one_batch() {
    let mut doc = doc_with(&["甲"]);
    let jia = id_of(&doc, "甲");
    let inner = add_child(&mut doc, &jia, "内层");
    let leaf = add_child(&mut doc, &inner, "叶子");
    for id in [&jia, &inner] {
        doc.apply(&Command::SetCollapsed {
            id: id.clone(),
            collapsed: true,
        })
        .unwrap();
    }
    assert!(
        scene_of(&doc).find(&leaf).is_none(),
        "折叠子树不进场景（前提）"
    );

    let commands = reveal_commands(&doc, &leaf);
    let ids: Vec<String> = commands
        .iter()
        .map(|c| match c {
            Command::SetCollapsed { id, .. } => id.clone(),
            other => panic!("只该有 SetCollapsed：{other:?}"),
        })
        .collect();
    assert_eq!(ids, vec![jia.clone(), inner.clone()], "按根 → 近展开");

    doc.apply(&Command::Batch { commands }).unwrap();
    assert!(!doc.node(&jia).unwrap().collapsed);
    assert!(!doc.node(&inner).unwrap().collapsed);
    assert!(
        scene_of(&doc).find(&leaf).is_some(),
        "祖先展开后叶子回到场景"
    );

    doc.undo().unwrap();
    assert!(doc.node(&jia).unwrap().collapsed);
    assert!(doc.node(&inner).unwrap().collapsed, "一次撤销收回全部祖先");
}

#[test]
fn reveal_commands_skip_visible_and_missing_nodes() {
    let mut doc = doc_with(&["甲"]);
    let jia = id_of(&doc, "甲");
    let leaf = add_child(&mut doc, &jia, "叶子");
    assert!(reveal_commands(&doc, &leaf).is_empty(), "链上无人折叠");
    assert!(
        reveal_commands(&doc, "不存在的ID").is_empty(),
        "节点不存在时不生成命令"
    );
}

#[test]
fn layout_subtree_membership_ignores_collapse_but_not_focus() {
    let mut doc = doc_with(&["甲"]);
    let root = doc.root_id.clone();
    let jia = id_of(&doc, "甲");
    let leaf = add_child(&mut doc, &jia, "叶子");
    assert!(in_layout_subtree(&doc, &root, &leaf));
    doc.apply(&Command::SetCollapsed {
        id: jia.clone(),
        collapsed: true,
    })
    .unwrap();
    assert!(
        in_layout_subtree(&doc, &root, &leaf),
        "折叠只影响场景，不影响归属"
    );
    assert!(
        !in_layout_subtree(&doc, &jia, &root),
        "聚焦到甲时根不在它的子树里"
    );
    assert!(!in_layout_subtree(&doc, "不存在的ID", &root));
}

#[test]
fn focus_exit_levels_only_fires_for_targets_outside_the_focused_subtree() {
    let mut doc = doc_with(&["甲", "乙"]);
    let root = doc.root_id.clone();
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let leaf = add_child(&mut doc, &jia, "叶子");

    assert_eq!(focus_exit_levels(&doc, &[], &yi), None, "没聚焦就不退");

    let one = vec![jia.clone()];
    assert_eq!(
        focus_exit_levels(&doc, &one, &yi),
        Some(1),
        "乙在聚焦子树外"
    );
    assert_eq!(
        focus_exit_levels(&doc, &one, &root),
        Some(1),
        "聚焦根之外的主根也算视图外"
    );
    assert_eq!(
        focus_exit_levels(&doc, &one, &leaf),
        None,
        "叶子在聚焦子树里：交给展开分支，不退聚焦"
    );
    assert_eq!(
        focus_exit_levels(&doc, &one, "不存在的ID"),
        None,
        "文档里没有这个节点时一条命令都不发"
    );

    let two = vec![root, jia];
    assert_eq!(
        focus_exit_levels(&doc, &two, &yi),
        Some(2),
        "多层聚焦一次退到主根，层数照实上报"
    );
}

#[test]
fn duplicate_lands_right_after_each_original() {
    let mut doc = doc_with(&["甲", "乙", "丙"]);
    let jia = id_of(&doc, "甲");
    let bing = id_of(&doc, "丙");
    let (cmd, tops) = duplicate_command(&doc, &[jia.clone(), bing.clone()]).expect("复制命令");
    doc.apply(&cmd).unwrap();
    assert_eq!(texts(&doc, &doc.root_id), ["甲", "甲", "乙", "丙", "丙"]);
    assert_eq!(tops.len(), 2);
    assert_ne!(tops[0], jia);
    assert_ne!(tops[1], bing);
}

#[test]
fn split_and_merge_are_single_batches_with_children_preserved() {
    // 回车拆行：一条 Batch 装 SetText + AddNode，撤销才能一步回去。
    let mut doc = doc_with(&["HelloWorld", "乙"]);
    let id = id_of(&doc, "HelloWorld");
    let cmd = split_sibling_command(&doc, &id, "Hello", "World").expect("可拆分");
    assert!(
        matches!(&cmd, Command::Batch { commands } if commands.len() == 2),
        "拆分必须是一条 Batch"
    );
    doc.apply(&cmd).unwrap();
    assert_eq!(texts(&doc, &doc.root_id), ["Hello", "World", "乙"]);
    assert_eq!(doc.node(&id).unwrap().text, "Hello");

    // Ctrl+Enter 拆子级：后半段挂到原节点下面。
    let mut doc = doc_with(&["AlphaBeta"]);
    let id = id_of(&doc, "AlphaBeta");
    let cmd = split_child_command(&doc, &id, "Alpha", "Beta").expect("可拆成子级");
    doc.apply(&cmd).unwrap();
    let kids = doc.node(&id).unwrap().children.clone();
    assert_eq!(kids.len(), 1);
    assert_eq!(doc.node(&kids[0]).unwrap().text, "Beta");
    assert_eq!(doc.node(&id).unwrap().text, "Alpha");

    // 行首退格并入上一同级：孩子的过继、改字、删除也在一条 Batch 里。
    let mut doc = doc_with(&["甲", "乙"]);
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let kid = add_child(&mut doc, &yi, "乙一");
    let (cmd, prev, caret) = merge_prev_command(&doc, &yi, "后").expect("可合并");
    assert_eq!(prev, jia);
    assert_eq!(caret, 1, "光标应落在「甲」之后、并入文字之前");
    assert!(
        matches!(&cmd, Command::Batch { commands } if commands.len() == 3),
        "合并必须是一条 Batch"
    );
    doc.apply(&cmd).unwrap();
    assert_eq!(doc.node(&jia).unwrap().text, "甲后");
    assert!(doc.node(&yi).is_none(), "被并的节点应删除");
    assert_eq!(
        doc.node(&kid).unwrap().parent_id.as_deref(),
        Some(jia.as_str()),
        "孩子要过继给上一兄弟，不能跟着消失"
    );

    // 根没有上一同级：拆分与合并都该拒绝，而不是造出孤立的自由主题。
    assert!(split_sibling_command(&doc, &doc.root_id, "根", "尾").is_none());
    assert!(merge_prev_command(&doc, &jia, "x").is_none());
}

#[test]
fn move_down_compensates_the_removal_shift() {
    let mut doc = doc_with(&["甲", "乙", "丙"]);
    let yi = id_of(&doc, "乙");
    let cmd = move_command(&doc, &yi, 1).expect("乙可以下移");
    doc.apply(&cmd).unwrap();
    assert_eq!(texts(&doc, &doc.root_id), ["甲", "丙", "乙"]);

    let jia = id_of(&doc, "甲");
    assert!(move_command(&doc, &jia, -1).is_none(), "第一个不能再上移");
    assert!(move_command(&doc, &yi, 1).is_none(), "最后一个不能再下移");
}

#[test]
fn promote_sits_after_the_old_parent_and_demote_joins_the_previous_sibling() {
    let mut doc = doc_with(&["甲", "乙"]);
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let a1 = add_child(&mut doc, &jia, "甲一");
    let a2 = add_child(&mut doc, &jia, "甲二");
    let cmd = promote_command(&doc, &a2).expect("甲二可以升级");
    doc.apply(&cmd).unwrap();
    assert_eq!(texts(&doc, &doc.root_id), ["甲", "甲二", "乙"]);
    assert_eq!(
        doc.node(&a2).unwrap().parent_id.as_deref(),
        Some(doc.root_id.as_str())
    );

    // 紧邻的前一个兄弟是「甲」：降级回到甲的最末。
    let cmd = demote_command(&doc, &a2).expect("甲二可以降回甲下");
    doc.apply(&cmd).unwrap();
    assert_eq!(texts(&doc, &jia), ["甲一", "甲二"]);

    // 乙的前一个兄弟是甲：降级成为甲的孩子。
    let cmd = demote_command(&doc, &yi).expect("乙可以降到甲下");
    doc.apply(&cmd).unwrap();
    assert_eq!(
        doc.node(&yi).unwrap().parent_id.as_deref(),
        Some(jia.as_str())
    );

    assert!(demote_command(&doc, &a1).is_none(), "甲一是第一个孩子");
    assert!(
        promote_command(&doc, &jia).is_none(),
        "主根的孩子升级没有落点"
    );
}

#[test]
fn same_parent_moves_scattered_roots_under_the_primarys_parent() {
    let mut doc = doc_with(&["甲", "乙"]);
    let jia = id_of(&doc, "甲");
    let yi = id_of(&doc, "乙");
    let a1 = add_child(&mut doc, &jia, "甲一");
    let b1 = add_child(&mut doc, &yi, "乙一");

    // 主选是「甲一」，落点因此是「甲」。乙带着乙一整棵挪进去；
    // 甲一本来就在落点下，不该产生无意义的移动命令。
    let cmd = same_parent_command(&doc, &[a1.clone(), yi.clone(), b1], &a1).expect("乙需要移动");
    doc.apply(&cmd).unwrap();
    assert_eq!(texts(&doc, &jia), ["甲一", "乙"]);
    assert_eq!(
        doc.node(&yi).unwrap().parent_id.as_deref(),
        Some(jia.as_str())
    );

    // 两个选中节点都已经在主选的父级下：没有可做的事。
    let a2 = add_child(&mut doc, &jia, "甲二");
    assert!(same_parent_command(&doc, &[a1.clone(), a2], &a1).is_none());
}

#[test]
fn motion_span_is_none_when_geometry_did_not_move() {
    let doc = doc_with(&["甲", "乙"]);
    let before = scene_of(&doc);
    let after = scene_of(&doc);
    assert!(motion_span(&before, &after).is_none());
}

/// 概念图里加一条命题线：盒子一个都不动，但也必须起过渡。
///
/// 这是「新线硬弹出来」的根因——`motion_span` 原来只看节点盒，
/// 关系线的新增/移除在它眼里等于「什么都没发生」，`blend_scenes` 里
/// 那条 `alpha = t` 的淡入分支就永远轮不到。
#[test]
fn motion_span_sees_a_new_relationship_when_no_box_moves() {
    let mut doc = doc_with(&["甲", "乙"]);
    let settings = LayoutSettings {
        kind: "conceptMap".into(),
        ..LayoutSettings::default()
    };
    let scene = |doc: &Document| {
        build_scene(
            doc,
            &doc.root_id,
            &settings,
            &get_theme("snow"),
            &MonoMeasurer,
            None::<(&str, &str)>,
        )
    };
    let before = scene(&doc);
    let (a, b) = (id_of(&doc, "甲"), id_of(&doc, "乙"));
    doc.apply(&Command::AddRelationship {
        from: a,
        to: b,
        label: None,
    })
    .unwrap();
    let after = scene(&doc);
    assert_eq!(
        after.relationships.len(),
        before.relationships.len() + 1,
        "概念图里这条关系要真的进场景"
    );
    assert_eq!(
        motion_span(&before, &after),
        Some(0.0),
        "几何没动也要起过渡（跨度 0 ⇒ 用最短时长）"
    );
}

#[test]
fn motion_span_catches_added_nodes_and_changed_boxes() {
    let mut doc = doc_with(&["甲", "乙"]);
    let before = scene_of(&doc);
    let root = doc.root_id.clone();
    doc.apply(&Command::AddNode {
        parent: root,
        index: None,
        text: "丙".into(),
        style: None,
    })
    .unwrap();
    let after = scene_of(&doc);
    let span = motion_span(&before, &after).expect("新增节点必须触发过渡");
    assert!(span > 0.0);
}

#[test]
fn motion_duration_grows_with_distance_and_clamps() {
    assert!((motion_duration(0.0) - 0.20).abs() < 1e-6);
    assert!(motion_duration(1200.0) > motion_duration(100.0));
    assert!((motion_duration(1.0e6) - 0.34).abs() < 1e-6);
}

#[test]
fn ease_out_cubic_has_fixed_endpoints_and_never_overshoots() {
    assert_eq!(ease_out_cubic(0.0), 0.0);
    assert_eq!(ease_out_cubic(1.0), 1.0);
    let mut prev = 0.0;
    for i in 0..=20 {
        let v = ease_out_cubic(i as f32 / 20.0);
        assert!(v >= prev, "缓动必须单调");
        assert!(v <= 1.0, "不允许冲过头");
        prev = v;
    }
}

/// 打开 260px 检查器：画布中心左移 130，相机跟着补偿后，世界点的**屏幕位置**不动。
#[test]
fn panel_shift_keeps_the_world_point_on_screen() {
    let zoom = 0.8f32;
    let world_x = 420.0f32;
    let old_center = 0.0f32;
    let old_view_center = 810.0f32;
    let screen_before = (world_x - old_center) * zoom + old_view_center;

    let (dx, dy) = camera_shift_for_viewport(egui::vec2(-130.0, 0.0), zoom);
    assert_eq!(dy, 0.0);
    let new_center = old_center + dx;
    let new_view_center = old_view_center - 130.0;
    let screen_after = (world_x - new_center) * zoom + new_view_center;

    assert!(
        (screen_before - screen_after).abs() < 0.01,
        "面板开合后画面不能平移：{screen_before} → {screen_after}"
    );
}

/// 补偿量按 zoom 换算成世界单位：放大到 2× 时同样的 130px 只需走一半的世界距离。
#[test]
fn panel_shift_is_converted_to_world_units() {
    let (dx, _) = camera_shift_for_viewport(egui::vec2(130.0, 0.0), 2.0);
    assert!((dx - 65.0).abs() < 1e-6);
    let (dx, _) = camera_shift_for_viewport(egui::vec2(130.0, 0.0), 0.5);
    assert!((dx - 260.0).abs() < 1e-6);
}

/// 面板动画末帧的亚像素抖动不能累计成漂移；非法缩放宁可不补偿。
#[test]
fn tiny_panel_shift_and_bad_zoom_are_ignored() {
    assert_eq!(
        camera_shift_for_viewport(egui::vec2(0.0005, -0.0005), 1.0),
        (0.0, 0.0)
    );
    // 0.01px 量级的位移必须照做：动画十几帧，丢掉它就会累计成 1px 漂移
    let (dx, _) = camera_shift_for_viewport(egui::vec2(0.01, 0.0), 1.0);
    assert!((dx - 0.01).abs() < 1e-6);
    assert_eq!(
        camera_shift_for_viewport(egui::vec2(130.0, 0.0), 0.0),
        (0.0, 0.0)
    );
    assert_eq!(
        camera_shift_for_viewport(egui::vec2(130.0, 0.0), f32::NAN),
        (0.0, 0.0)
    );
}

/// 方向键按屏幕几何走：根 → 右子（→）、左子（←），子 → 根（←），到头不动。
#[test]
fn arrow_keys_walk_the_map_geometrically() {
    let doc = doc_with(&["甲", "乙"]);
    let scene = scene_of(&doc);
    let root = scene.find(&doc.root_id).expect("根在场景里");
    let a = scene.find(&id_of(&doc, "甲")).expect("甲在场景里");
    let b = scene.find(&id_of(&doc, "乙")).expect("乙在场景里");
    // 思维导图布局会把兄弟分到左右两侧，先按几何判断谁在右
    let (right, left) = if scene.rect(a).center().x > scene.rect(b).center().x {
        (a, b)
    } else {
        (b, a)
    };
    assert_eq!(directional_neighbor(&scene, root, 1.0, 0.0), Some(right));
    assert_eq!(directional_neighbor(&scene, root, -1.0, 0.0), Some(left));
    assert_eq!(directional_neighbor(&scene, right, -1.0, 0.0), Some(root));
    assert_eq!(
        directional_neighbor(&scene, right, 1.0, 0.0),
        None,
        "右侧没有节点时不该乱跳"
    );
}

/// 斜上方很远、但侧向偏移过大的节点不算「右边」。
#[test]
fn arrow_navigation_rejects_far_side_jumps() {
    let doc = doc_with(&["甲", "乙", "丙"]);
    let scene = scene_of(&doc);
    let root = scene.find(&doc.root_id).expect("根在场景里");
    let picked = directional_neighbor(&scene, root, 0.0, 1.0);
    if let Some(idx) = picked {
        let from = scene.rect(root).center();
        let to = scene.rect(idx).center();
        assert!(to.y > from.y, "向下导航必须真的往下走");
        assert!(
            (to.x - from.x).abs() <= (to.y - from.y) * 1.6,
            "侧向偏移不能超过主轴 1.6 倍"
        );
    }
}

/// IME 组合态只看本帧最后一个 IME 事件：预编辑进入、提交/空预编辑退出、无事件沿用。
#[test]
fn ime_transition_follows_the_latest_ime_event() {
    let preedit = |s: &str| {
        Event::Ime(egui::ImeEvent::Preedit {
            text: s.to_string(),
            active_range_chars: None,
        })
    };
    assert_eq!(ime_transition(&[]), None, "没有 IME 事件时必须沿用上一帧");
    assert_eq!(
        ime_transition(&[Event::Key {
            key: Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]),
        None
    );
    assert_eq!(ime_transition(&[preedit("ni hao")]), Some(true));
    assert_eq!(
        ime_transition(&[preedit("")]),
        Some(false),
        "空预编辑表示候选窗已收起"
    );
    assert_eq!(
        ime_transition(&[
            preedit("nihao"),
            Event::Ime(egui::ImeEvent::Commit("你好".into()))
        ]),
        Some(false),
        "以最后一个事件为准：提交后不再算组合中"
    );
    assert_eq!(
        ime_transition(&[
            Event::Ime(egui::ImeEvent::Commit("你好".into())),
            preedit("shi jie"),
        ]),
        Some(true),
        "提交后紧接着下一次组合，必须重新进入组合态"
    );
}

/// 组合期间回车/取消归输入法：画布不能把 Enter 当成提交、把 Esc 当成取消。
#[test]
fn ime_composition_owns_enter_and_escape() {
    assert!(!edit_keys_owned(false, false), "非编辑态没有编辑键");
    assert!(edit_keys_owned(true, false), "普通编辑态回车提交、Esc 取消");
    assert!(!edit_keys_owned(true, true), "组合中两个键都归输入法");
    assert!(!edit_keys_owned(false, true));
}

/// 行内编辑器的行高必须锁到画布那一份（`字号 × 1.45`），否则多行节点一按 F2 就收紧。
#[test]
fn the_editor_locks_its_line_box_to_the_canvas_pitch() {
    let line_h = mw_core::text::line_height(14.0);
    let g = edit_galley_headless("第一行\n第二行\n第三行", line_h);
    assert_eq!(g.rows.len(), 3, "三行草稿就该排成三行");
    for w in g.rows.windows(2) {
        let pitch = (w[1].pos.y - w[0].pos.y).abs();
        assert!(
            (pitch - line_h).abs() <= 1.0,
            "行距 {pitch} 应等于画布行高 {line_h}"
        );
    }
}

/// 首行基线要落在自己的行盒里：越界说明量到的是别的行，编辑器会整块上下错位。
#[test]
fn the_measured_baseline_lands_inside_the_line_box() {
    let line_h = mw_core::text::line_height(14.0);
    let g = edit_galley_headless(BASELINE_PROBE, line_h);
    let base = first_baseline(&g, line_h);
    assert!(
        base > 0.0 && base < line_h,
        "基线 {base} 必须在 0..{line_h} 的行盒内"
    );
}

/// 空草稿没有字形可量，必须退回画布那套系数，不能让节点文字突然上跳。
#[test]
fn an_empty_draft_falls_back_to_the_canvas_baseline_ratio() {
    let line_h = mw_core::text::line_height(14.0);
    let g = edit_galley_headless("", line_h);
    let want = line_h * mw_core::layout::BASELINE_RATIO;
    assert!(
        (first_baseline(&g, line_h) - want).abs() < 0.01,
        "空文本应退回 {want}"
    );
}

/// 打字过程中行数与几何行数不一致时的居中补偿：少打的行补一半，多打的往下长。
#[test]
fn the_editor_compensates_only_for_lines_that_are_not_typed_yet() {
    let (box_h, label_h, line_h) = (48.0, 0.0, 20.0);
    // 几何两行、草稿一行：差额按居中补半行。
    assert!(
        (first_base_offset(box_h, label_h, line_h, 2, 1)
            - (line_baseline(box_h, label_h, 2, line_h, 0) + line_h * 0.5))
            .abs()
            < 0.01
    );
    // 草稿比几何多出行：首行基线不动，多出来的只能往下长。
    assert!(
        (first_base_offset(box_h, label_h, line_h, 1, 3)
            - line_baseline(box_h, label_h, 1, line_h, 0))
        .abs()
            < 0.01,
        "多打的行不该把首行顶上去"
    );
    // 行数吻合时就是画布读数本身。
    assert!(
        (first_base_offset(box_h, label_h, line_h, 2, 2)
            - line_baseline(box_h, label_h, 2, line_h, 0))
        .abs()
            < 0.01
    );
}

/// 先走两帧空帧，再走 `tabs` 帧 `Tab`，返回 `(结束时的焦点, 画布 id)`。
///
/// 两帧空帧不是凑数：`set_focus_lock_filter` 要求「上一帧就持有焦点」，所以画布抢到
/// 焦点后的下一帧锁才装得上——真人手速再怎么快也踩不到这一帧的窗口，测试却要按真实
/// 时序走。`grab` 只在第一帧生效，和生产一致：`refocus_canvas` 是一次性的。
fn keyboard_ring(
    wants_keys: bool,
    grab: bool,
    lock: bool,
    tabs: usize,
) -> (Option<egui::Id>, egui::Id) {
    let ctx = egui::Context::default();
    let chip = egui::Id::unique("accent-chip");
    let mut canvas = None;
    for i in 0..tabs + 2 {
        let events = if i < 2 {
            Vec::new()
        } else {
            vec![egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        };
        ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                let (_, resp) = canvas_area(ui, egui::vec2(600.0, 400.0), wants_keys);
                canvas = Some(resp.id);
                if wants_keys {
                    if grab && i == 0 {
                        resp.request_focus();
                    }
                    if lock {
                        ui.ctx()
                            .memory_mut(|m| m.set_focus_lock_filter(resp.id, canvas_key_lock()));
                    }
                }
                // 面板里的一个控件，和检查器的强调色色板一样：click() 自带 FOCUSABLE。
                ui.interact(
                    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(22.0, 22.0)),
                    chip,
                    egui::Sense::click(),
                );
            },
        )
        .drop_without_applying_deltas();
    }
    (ctx.memory(|m| m.focused()), canvas.expect("画布每帧都登记"))
}

/// `F6` 交接键盘的机制测试（不需要 GPU，也不需要真窗口）。
///
/// 三条断言把设计锁死：画布握着键盘时 `Tab` 是「加子主题」，被焦点锁留在画布上；
/// 画布只要还在焦点环里，`Tab` 第一下就转回画布——所以**只 `surrender_focus` 是不够的**，
/// 必须连「可聚焦」一起摘掉；摘掉之后 `Tab` 才进得去面板。
#[test]
fn handing_over_the_keyboard_moves_tab_into_the_panels() {
    let (focus, canvas) = keyboard_ring(true, true, true, 3);
    assert_eq!(
        focus,
        Some(canvas),
        "画布握着键盘时 Tab 不该跑掉——它归「加子主题」"
    );

    let (focus, _) = keyboard_ring(true, false, false, 1);
    assert_eq!(
        focus,
        Some(canvas),
        "画布还在焦点环里，Tab 第一下就转回画布"
    );

    let (focus, _) = keyboard_ring(false, false, false, 1);
    assert_ne!(focus, Some(canvas), "键盘交给面板后，Tab 必须能离开画布");
    let (focus, _) = keyboard_ring(false, false, false, 4);
    assert_ne!(focus, Some(canvas), "在面板里连按 Tab 也不该被画布抢回去");
}

/// 面板开合不该把画布的键盘焦点弄丢。
///
/// 画布的区域如果用布局自动分配的 id 登记：中央 `Ui` 的身份一变，id 就跟着变，而 egui
/// 的「死人开关」（dead-mans-switch）只认 id——上一帧被聚焦的 id 这一帧不再出现，
/// 焦点直接清零。用户看到的现象是：按了一下 `F8` 收起大纲，之后 `Tab`/`Enter`/`Delete`
/// 全体失灵，直到鼠标点一下画布。`canvas_area` 把 id 钉死，这条测试就是钉子的存在证明。
#[test]
fn a_panel_toggle_keeps_the_canvas_keyboard_focus() {
    let ctx = egui::Context::default();
    let mut ids: Vec<egui::Id> = Vec::new();
    for (i, outline) in [false, true, true].iter().enumerate() {
        ctx.run_ui(egui::RawInput::default(), |ui| {
            if *outline {
                egui::Panel::left("mw-outline")
                    .exact_size(220.0)
                    .resizable(false)
                    .show(ui, |ui| {
                        ui.label("大纲");
                    });
            }
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let size = ui.available_size();
                let (_, resp) = canvas_area(ui, size, true);
                ids.push(resp.id);
                if i == 0 {
                    resp.request_focus();
                }
            });
        })
        .drop_without_applying_deltas();
    }
    assert_eq!(ids[0], ids[1], "大纲从关到开，画布的 widget id 不该变");
    assert_eq!(ids[1], ids[2], "面板静止时 id 也不该漂");
    assert_eq!(
        ctx.memory(|m| m.focused()),
        Some(ids[2]),
        "id 一漂焦点就没了——这正是按 F8 之后快捷键全体失灵的成因"
    );
}

#[test]
fn a_style_chord_beats_a_plain_copy_in_the_same_frame() {
    // Ctrl+Shift+C 和 Ctrl+C 同帧到达（前后按了两次）时只能有一条动作：
    // 先判带 Shift 的，否则「复制样式」顺带把选中子树也压进剪贴板。
    assert_eq!(
        clipboard_chord(true, false, true, true, false),
        Some(ClipboardChord::StyleCopy)
    );
    assert_eq!(
        clipboard_chord(false, true, false, false, true),
        Some(ClipboardChord::StylePaste)
    );
}

#[test]
fn cut_wins_over_copy_because_cut_is_copy_plus_delete() {
    // 画布时代的 `if copy_ev || cut_ev { …复制…; if cut { 删除 } }` 就是这个语义：
    // 两条都在场时必须落到带删除的那条，不然剪切的删除会静默丢失。
    assert_eq!(
        clipboard_chord(false, false, true, true, false),
        Some(ClipboardChord::Cut)
    );
    assert_eq!(
        clipboard_chord(false, false, true, false, false),
        Some(ClipboardChord::Copy)
    );
}

#[test]
fn no_clipboard_chord_consumes_nothing() {
    // 返回 None 的那一帧一个事件都不该被摘走——面板的 `TextEdit` 在本帧后面才画，
    // `Copy/Cut/Paste` 是谁先 `retain` 谁赢。这条断言是那道门的唯一守门人。
    assert_eq!(clipboard_chord(false, false, false, false, false), None);
    assert_eq!(
        clipboard_chord(false, false, false, false, true),
        Some(ClipboardChord::Paste)
    );
}

#[test]
fn zoom_anchor_keeps_the_world_point_under_the_cursor() {
    let mut cam = Camera::new(Point::new(10.0, 20.0), 1.3, 1.0);
    let viewport = [800.0, 600.0];
    let at = Point::new(123.0, 77.0);
    let before = cam.screen_to_world(at, viewport);
    zoom_at_anchor(&mut cam, at, viewport, 2.0);
    let after = cam.screen_to_world(at, viewport);
    assert!((cam.zoom - 2.6).abs() < 1e-4);
    assert!(
        (before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3,
        "锚点漂了：{before:?} → {after:?}"
    );
    // 上界钳位后仍不漂。
    let before = cam.screen_to_world(at, viewport);
    zoom_at_anchor(&mut cam, at, viewport, 100.0);
    let after = cam.screen_to_world(at, viewport);
    assert!((cam.zoom - MAX_ZOOM).abs() < 1e-4);
    assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
}

#[test]
fn auto_scroll_only_near_edges_and_scales() {
    let (min, max, margin, speed) = (0.0, 800.0, 56.0, 900.0);
    assert_eq!(edge_scroll_speed(400.0, min, max, margin, speed), 0.0);
    assert_eq!(edge_scroll_speed(0.0, min, max, margin, speed), -900.0);
    assert_eq!(edge_scroll_speed(800.0, min, max, margin, speed), 900.0);
    assert!(edge_scroll_speed(28.0, min, max, margin, speed) < 0.0);
    assert!(edge_scroll_speed(772.0, min, max, margin, speed) > 0.0);
    assert_eq!(edge_scroll_speed(10.0, 0.0, 0.0, margin, speed), 0.0);
}

#[test]
fn lasso_polygon_contains_points() {
    let poly = [
        Pos2::new(10.0, 10.0),
        Pos2::new(90.0, 10.0),
        Pos2::new(90.0, 90.0),
        Pos2::new(10.0, 90.0),
    ];
    assert!(point_in_polygon(Pos2::new(50.0, 50.0), &poly));
    assert!(!point_in_polygon(Pos2::new(5.0, 50.0), &poly));
    assert!(!point_in_polygon(Pos2::new(50.0, 50.0), &poly[..2]));
}

/// 打磨轮 9：橡皮筋在屏幕上报的是两个点，结算时要按**当前缩放**换成世界矩形——
/// 缩到 50% 或放到 200% 都不能框歪。以前这条换算没有单测，只有「圆心命中」式的结果断言。
#[test]
fn marquee_pick_maps_the_screen_box_through_the_camera() {
    let doc = doc_with(&["甲", "乙", "丙"]);
    let scene = scene_of(&doc);
    let canvas = egui::Rect::from_min_size(Pos2::new(40.0, 30.0), Vec2::new(900.0, 700.0));
    let viewport = [canvas.width(), canvas.height()];
    let target = scene.nodes[1].geom.rect;

    for zoom in [0.5, 1.0, 2.0] {
        let camera = Camera::new(scene.bounds.center(), zoom, 1.0);
        // 世界盒的两个角 → 屏幕点（画布原点不是屏幕原点，要加上 canvas.min）。
        let tl = camera.world_to_screen(Point::new(target.x, target.y), viewport);
        let br = camera.world_to_screen(Point::new(target.right(), target.bottom()), viewport);
        let drag = Drag {
            kind: DragKind::Marquee,
            origin: Pos2::new(canvas.min.x + tl.x, canvas.min.y + tl.y),
            current: Pos2::new(canvas.min.x + br.x, canvas.min.y + br.y),
            applied: Pos2::ZERO,
            drawing: true,
            copy: false,
            additive: false,
            subtract: false,
        };
        let picked = marquee_pick(&camera, &scene, canvas, &drag, viewport);
        assert!(
            picked.contains(&1),
            "zoom={zoom} 时正好框住第二个节点的屏幕盒，必须选中它：{picked:?}"
        );
    }
}

#[test]
fn marquee_merge_honours_shift_and_alt() {
    assert_eq!(apply_marquee(&[1, 2], vec![3], false, false), vec![3]);
    assert_eq!(
        apply_marquee(&[1, 2], vec![2, 3], true, false),
        vec![1, 2, 3]
    );
    assert_eq!(apply_marquee(&[1, 2], vec![1], true, false), vec![1, 2]);
    assert_eq!(apply_marquee(&[1, 2, 3], vec![2], false, true), vec![1, 3]);
    assert_eq!(apply_marquee(&[1], vec![9], false, true), vec![1]);
}

/// 打磨轮 62（M7b）：套索的多边形报在**屏幕**上，命中判定要把每个节点盒中心按
/// 当前缩放换回屏幕再套进去。以前只有纯几何的 `point_in_polygon`，
/// 这半截屏幕→世界的换算没有任何单测（`marquee_pick` 那条只管矩形）。
#[test]
fn lasso_pick_maps_the_polygon_through_the_camera() {
    let doc = doc_with(&["甲", "乙", "丙"]);
    let scene = scene_of(&doc);
    let canvas = egui::Rect::from_min_size(Pos2::new(40.0, 30.0), Vec2::new(900.0, 700.0));
    let viewport = [canvas.width(), canvas.height()];
    let target = scene.nodes[1].geom.rect;

    for zoom in [0.5, 1.0, 2.0] {
        let camera = Camera::new(scene.bounds.center(), zoom, 1.0);
        let corner = |x: f32, y: f32| {
            let s = camera.world_to_screen(Point::new(x, y), viewport);
            Pos2::new(canvas.min.x + s.x, canvas.min.y + s.y)
        };
        // 多边形正好是这个节点的屏幕盒：中心在内，兄弟节点的中心都在外。
        let poly = [
            corner(target.x, target.y),
            corner(target.right(), target.y),
            corner(target.right(), target.bottom()),
            corner(target.x, target.bottom()),
        ];
        let picked = lasso_pick(&camera, &scene, canvas, &poly, viewport);
        assert_eq!(
            picked,
            vec![1],
            "zoom={zoom} 套索正好围住第二个节点的盒，必须只选中它：{picked:?}"
        );
        // 少于三个点没有面积：一次「Alt+单击空白」就是这种情况，什么都不选。
        assert!(lasso_pick(&camera, &scene, canvas, &poly[..2], viewport).is_empty());
    }
}

/// 打磨轮 62（M7b）：Alt 套索的结算口径。
///
/// `handle_pointer` 只在按住 Alt 时才把空白框选改判成 `Lasso`，所以套索在真实手势里
/// **恒为减选**：它不能建立新选择，只能从已有的组里摘掉圈中的节点。与矩形框选唯一
/// 的差别是「圈空时不清空选择」——按住 Alt 是想去掉一点，不是想全不选。
#[test]
fn lasso_subtract_only_removes_looped_nodes() {
    // 圈中几颗减几颗，组里其余不动。
    assert_eq!(
        apply_lasso(&[1, 2, 3], vec![2], false, true),
        Some(vec![1, 3])
    );
    // Alt 圈空：这一下不算，选择原样保留（不能被「替换成空集」）。
    assert_eq!(apply_lasso(&[1, 2, 3], vec![], false, true), None);
    // Ctrl/Shift 并入（套索手势走不到，纯函数仍按合并口径结算）。
    assert_eq!(
        apply_lasso(&[1, 2], vec![2, 3], true, false),
        Some(vec![1, 2, 3])
    );
    // 无修饰 → 替换。
    assert_eq!(apply_lasso(&[1, 2], vec![3], false, false), Some(vec![3]));
    // Ctrl 与 Alt 同时按：Alt 只减优先——圈中的摘掉，没圈中的不会被加进来。
    assert_eq!(apply_lasso(&[1, 2], vec![2, 3], true, true), Some(vec![1]));
}

#[test]
fn hover_bar_flips_at_four_corners_and_clears_fold_and_badge() {
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    // (角, 节点左上角, 期望在右侧, 期望在下方)
    let cases = [
        ("左上", Pos2::new(24.0, 24.0), true, true),
        ("右上", Pos2::new(600.0, 24.0), false, true),
        ("左下", Pos2::new(24.0, 500.0), true, false),
        ("右下", Pos2::new(600.0, 500.0), false, false),
    ];
    for (name, at, right, below) in cases {
        let node = egui::Rect::from_min_size(at, Vec2::new(160.0, 44.0));
        let (plus, more) =
            hover_bar_rects(node, canvas, false).unwrap_or_else(|| panic!("{name} 应放得下"));
        let fold = toggle_disc(node, false);
        for r in [plus, more] {
            assert!(canvas.contains_rect(r), "{name} 跑出画布：{r:?}");
            assert!(!r.intersects(node), "{name} 压住节点：{r:?}");
            assert!(!r.intersects(fold), "{name} 压住折叠点：{r:?} vs {fold:?}");
        }
        assert_eq!(plus.left() > node.right(), right, "{name} 横向翻转不对");
        assert_eq!(plus.top() > node.top(), below, "{name} 纵向翻转不对");

        // 有卡片角标时，任何角都必须与角标保持距离。
        let (plus, more) =
            hover_bar_rects(node, canvas, true).unwrap_or_else(|| panic!("{name} 应有角标避让"));
        let badge = egui::Rect::from_center_size(
            card_badge_center(node.right_top(), 6.5),
            Vec2::splat(6.5 * 2.0 + 6.0),
        );
        for r in [plus, more] {
            assert!(!r.intersects(badge), "{name} 压住角标：{r:?} vs {badge:?}");
            assert!(!r.intersects(fold), "{name} 有角标时压住折叠点：{r:?}");
        }
    }
}

#[test]
fn state_alpha_moves_at_120_and_180ms_and_snaps_when_reduced() {
    assert!((advance_alpha(0.0, 1.0, 0.06, 0.12, false) - 0.5).abs() < 1e-4);
    assert!((advance_alpha(0.0, 1.0, 0.12, 0.12, false) - 1.0).abs() < 1e-4);
    assert!((advance_alpha(1.0, 0.0, 0.09, 0.18, false) - 0.5).abs() < 1e-4);
    assert_eq!(advance_alpha(0.2, 1.0, 0.001, 0.12, true), 1.0);
    assert_eq!(advance_alpha(0.2, 0.0, 0.001, 0.12, true), 0.0);
}

#[test]
fn card_badge_is_outside_the_node_and_scales_with_radius() {
    let c = card_badge_center(egui::pos2(100.0, 40.0), 6.5);
    assert!(c.x > 100.0, "角标在右边界之外");
    assert!(c.y < 40.0, "角标在上边界之外");
    let small = card_badge_center(egui::pos2(100.0, 40.0), 3.5);
    assert!(small.x < c.x && small.y > c.y, "缩小半径应更贴近节点");
}

// ---- O04 大纲多选：批量结构操作 ----

#[test]
fn batch_indent_nests_selected_siblings_bottom_up_in_one_batch() {
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    let a = add_child(&mut doc, &root, "A");
    let b = add_child(&mut doc, &root, "B");
    let c = add_child(&mut doc, &root, "C");
    let cmd =
        batch_structure_command(&doc, &[b.clone(), c.clone()], BatchOp::Indent).expect("可以缩进");
    let Command::Batch { commands } = &cmd else {
        panic!("批量操作要是一条 Batch");
    };
    assert_eq!(commands.len(), 2);
    doc.apply(&cmd).unwrap();
    assert_eq!(doc.node(&a).unwrap().children, vec![b.clone()]);
    assert_eq!(doc.node(&b).unwrap().children, vec![c]);
}

#[test]
fn batch_outdent_keeps_sibling_order_after_the_parent() {
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    let p = add_child(&mut doc, &root, "P");
    let a = add_child(&mut doc, &p, "A");
    let b = add_child(&mut doc, &p, "B");
    let c = add_child(&mut doc, &p, "C");
    let cmd = batch_structure_command(&doc, &[a.clone(), b.clone(), c.clone()], BatchOp::Outdent)
        .expect("可以升级");
    doc.apply(&cmd).unwrap();
    let kids = &doc.node(&root).unwrap().children;
    let at = |id: &str| kids.iter().position(|x| x == id).unwrap();
    assert_eq!(kids.len(), 4);
    assert!(
        at(&p) < at(&a) && at(&a) < at(&b) && at(&b) < at(&c),
        "{kids:?}"
    );
}

/// M6 复核（打磨轮 60）：跨父级多选按 `Alt+↓` 不是「什么都不动」——
/// `batch_structure_command` 本来就逐个在**自己的父级内**找下一个位置；
/// 两颗各有后兄弟时，两边都应真的换位。这条钉住账本 M6 的旧结论（待纠正）。
#[test]
fn batch_move_cross_parent_moves_each_within_its_own_parent() {
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    let p = add_child(&mut doc, &root, "P");
    let q = add_child(&mut doc, &root, "Q");
    let a1 = add_child(&mut doc, &p, "A1");
    let a2 = add_child(&mut doc, &p, "A2");
    let b1 = add_child(&mut doc, &q, "B1");
    let b2 = add_child(&mut doc, &q, "B2");
    let cmd = batch_structure_command(&doc, &[a1.clone(), b1.clone()], BatchOp::MoveDown)
        .expect("跨父级也要各自下移");
    doc.apply(&cmd).unwrap();
    assert_eq!(doc.node(&p).unwrap().children, vec![a2, a1]);
    assert_eq!(doc.node(&q).unwrap().children, vec![b2, b1]);
}

/// M6b：边界判定四种情形（根 / 第一个 / 最后一个 / 还能动）。
#[test]
fn move_block_reason_covers_root_first_last_and_free() {
    use crate::canvas::MoveBlock;
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    let a = add_child(&mut doc, &root, "A");
    let b = add_child(&mut doc, &root, "B");
    assert_eq!(
        crate::canvas::move_block_reason(&doc, &root, -1),
        MoveBlock::Root
    );
    assert_eq!(
        crate::canvas::move_block_reason(&doc, &a, -1),
        MoveBlock::First
    );
    assert_eq!(
        crate::canvas::move_block_reason(&doc, &b, 1),
        MoveBlock::Last
    );
    assert_eq!(
        crate::canvas::move_block_reason(&doc, &a, 1),
        MoveBlock::None
    );
    assert_eq!(
        crate::canvas::move_block_reason(&doc, &b, -1),
        MoveBlock::None
    );
}

#[test]
fn batch_move_group_up_and_down_keeps_order_and_respects_edges() {
    fn four() -> (Document, String, Vec<String>) {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        let ids = ["A", "B", "C", "D"]
            .iter()
            .map(|t| add_child(&mut doc, &root, t))
            .collect();
        (doc, root, ids)
    }

    let (mut doc, root, ids) = four();
    let up = batch_structure_command(&doc, &[ids[1].clone(), ids[2].clone()], BatchOp::MoveUp)
        .expect("整组上移");
    doc.apply(&up).unwrap();
    assert_eq!(
        doc.node(&root).unwrap().children,
        vec![
            ids[1].clone(),
            ids[2].clone(),
            ids[0].clone(),
            ids[3].clone()
        ],
        "上移保持组内顺序"
    );

    let (mut doc, root, ids) = four();
    let down = batch_structure_command(&doc, &[ids[1].clone(), ids[2].clone()], BatchOp::MoveDown)
        .expect("整组下移");
    doc.apply(&down).unwrap();
    assert_eq!(
        doc.node(&root).unwrap().children,
        vec![
            ids[0].clone(),
            ids[3].clone(),
            ids[1].clone(),
            ids[2].clone()
        ],
        "下移保持组内顺序"
    );

    let (doc, _root, ids) = four();
    assert!(
        batch_structure_command(&doc, &[ids[0].clone(), ids[1].clone()], BatchOp::MoveUp).is_none(),
        "组头已在边缘，整组不动"
    );
    assert!(
        batch_structure_command(&doc, &[ids[2].clone(), ids[3].clone()], BatchOp::MoveDown)
            .is_none(),
        "组尾已在边缘，整组不动"
    );
}

/// 打磨轮 7：`Alt+↑↓` 多选走批量命令、单选仍走单条命令。两条路径必须给同一棵树——
/// 否则「选一颗按一下」和「选两颗再取消一颗按一下」会挪到不同位置。
#[test]
fn single_selection_moves_the_same_way_through_both_paths() {
    fn three() -> (Document, String, Vec<String>) {
        let mut doc = Document::new("根");
        let root = doc.root_id.clone();
        let ids = ["A", "B", "C"]
            .iter()
            .map(|t| add_child(&mut doc, &root, t))
            .collect();
        (doc, root, ids)
    }

    /// 兄弟顺序按文字比：两份文档的 id 是各自生成的，比 id 永远不等。
    fn order(doc: &Document, root: &str) -> Vec<String> {
        doc.node(root)
            .unwrap()
            .children
            .iter()
            .map(|id| doc.node(id).unwrap().text.clone())
            .collect()
    }

    for delta in [1, -1] {
        // 下移拿组头、上移拿中间那颗：两种方向都真的挪得动。
        let pick = if delta == 1 { 0 } else { 1 };

        let (mut single_doc, root, ids) = three();
        let via_single = move_command(&single_doc, &ids[pick], delta).expect("单条路径可以移动");
        single_doc.apply(&via_single).unwrap();

        let (mut batch_doc, batch_root, batch_ids) = three();
        let op = if delta < 0 {
            BatchOp::MoveUp
        } else {
            BatchOp::MoveDown
        };
        let via_batch = batch_structure_command(&batch_doc, &[batch_ids[pick].clone()], op)
            .expect("批量路径也可以移动");
        batch_doc.apply(&via_batch).unwrap();

        assert_eq!(
            order(&single_doc, &root),
            order(&batch_doc, &batch_root),
            "delta={delta} 时两条路径的兄弟顺序必须一致"
        );
    }

    let (doc, _root, ids) = three();
    assert!(
        move_command(&doc, &ids[0], -1).is_none()
            && batch_structure_command(&doc, &[ids[0].clone()], BatchOp::MoveUp).is_none(),
        "组头两条路径都不动"
    );
    assert!(
        move_command(&doc, &ids[2], 1).is_none()
            && batch_structure_command(&doc, &[ids[2].clone()], BatchOp::MoveDown).is_none(),
        "组尾两条路径都不动"
    );
}

#[test]
fn move_many_keeps_group_order_for_specific_and_appended_drops() {
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    let a = add_child(&mut doc, &root, "A");
    let b = add_child(&mut doc, &root, "B");
    let c = add_child(&mut doc, &root, "C");
    let d = add_child(&mut doc, &root, "D");

    let before_d =
        move_many_command(&doc, &[a.clone(), c.clone()], &root, Some(3)).expect("插到 D 之前");
    doc.apply(&before_d).unwrap();
    assert_eq!(
        doc.node(&root).unwrap().children,
        vec![b.clone(), a.clone(), c.clone(), d.clone()]
    );

    let to_end = move_many_command(&doc, &[a.clone(), c.clone()], &root, None).expect("追加到末尾");
    doc.apply(&to_end).unwrap();
    assert_eq!(
        doc.node(&root).unwrap().children,
        vec![b, d, a, c],
        "追加时组内顺序不翻"
    );
}

#[test]
fn move_many_refuses_to_drop_into_its_own_subtree() {
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    let a = add_child(&mut doc, &root, "A");
    let child = add_child(&mut doc, &a, "A1");
    assert!(move_many_command(&doc, std::slice::from_ref(&a), &a, None).is_none());
    assert!(move_many_command(&doc, &[a], &child, None).is_none());
}

#[test]
fn plain_text_paste_parses_indent_tabs_and_list_markers() {
    let nodes = parse_plain_outline("根\n  子一\n  子二\n    孙\n- 列表\n  续\n", "P").unwrap();
    let find = |t: &str| nodes.iter().find(|n| n.text == t).unwrap();
    assert_eq!(find("根").parent_id.as_deref(), Some("P"));
    assert_eq!(
        find("子一").parent_id.as_deref(),
        Some(find("根").id.as_str())
    );
    assert_eq!(
        find("孙").parent_id.as_deref(),
        Some(find("子二").id.as_str())
    );
    assert_eq!(find("列表").parent_id.as_deref(), Some("P"));
    assert_eq!(
        find("续").parent_id.as_deref(),
        Some(find("列表").id.as_str())
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|n| n.parent_id.as_deref() == Some("P"))
            .count(),
        2,
        "只有根与列表是顶层"
    );
}

#[test]
fn plain_text_paste_skips_blank_and_keeps_non_list_dashes() {
    let nodes = parse_plain_outline("\n-5℃\n\n  - 甲\n", "P").unwrap();
    assert_eq!(nodes[0].text, "-5℃", "负号不是列表前缀");
    assert_eq!(nodes[1].text, "甲");
    assert!(parse_plain_outline("  \n\n", "P").is_none());
}

/// CM-05：概念图方向键的步长只有两个值——素键 1px、`Shift` 8px。
/// 这条断言防的是「有人觉得 8px 太小改成 10」：`docs/30` §5.7 写的是 1/8。
#[test]
fn concept_nudge_steps_are_one_and_eight() {
    assert_eq!(nudge_step(false), 1.0);
    assert_eq!(nudge_step(true), 8.0);
    assert!(nudge_step(true) > nudge_step(false));
}

/// C7（打磨轮 13）：整理布局在撤销栈里只占**一条**——三颗概念各挪一截，一次 `undo()` 全部复原。
///
/// 账本 C7 行原先的「有单测」指的是 `concept_layout` 里的纯算法；这条命令本身没人测过，
/// 于是「整理丢了手工位置能不能一次撤回来」一直是句断言而不是证据。
#[test]
fn arranging_three_concepts_is_one_undo() {
    let mut doc = Document::new("主");
    doc.apply(&Command::SetLayout {
        layout: LayoutSettings {
            kind: "conceptMap".into(),
            ..LayoutSettings::default()
        },
    })
    .expect("切成概念图");
    let mut ids = vec![doc.root_id.clone()];
    for (t, x, y) in [("甲", 300.0, 0.0), ("乙", -300.0, 200.0)] {
        doc.apply(&Command::AddFloatingTopic {
            text: t.into(),
            x,
            y,
        })
        .expect("加浮动概念");
        ids.push(id_of(&doc, t));
    }
    let current = vec![(0.0, 0.0), (300.0, 0.0), (-300.0, 200.0)];
    for (id, (x, y)) in ids.iter().zip(&current) {
        doc.apply(&Command::MoveConcept {
            id: id.clone(),
            x: *x,
            y: *y,
        })
        .expect("先手工摆位");
    }
    let depth = doc.undo_depth();

    let pos = |doc: &Document, id: &str| {
        let p = doc
            .node(id)
            .and_then(|n| n.pos.as_ref())
            .expect("概念图里该有 pos");
        (p.x, p.y)
    };
    assert_eq!(pos(&doc, &ids[1]), (300.0, 0.0), "整理前是手工位置");

    let placed = vec![(10.0, 0.0), (0.0, 80.0), (0.0, 160.0)];
    let (cmd, moved) = Canvas::arrange_batch(&ids, &current, &placed).expect("三颗都动过");
    assert_eq!(moved, 3, "动过的颗数就是提示里的 N");
    assert!(
        matches!(cmd, Command::Batch { .. }),
        "多条必须合成一条 Batch"
    );
    doc.apply(&cmd).expect("落地");

    assert_eq!(doc.undo_depth(), depth + 1, "整理在撤销栈里只占一条");
    assert_eq!(pos(&doc, &ids[1]), (0.0, 80.0), "先确实摆到了新位置");

    doc.undo().expect("一次撤销");
    assert_eq!(doc.undo_depth(), depth, "一条撤销回到底");
    for (id, want) in ids.iter().zip(&current) {
        assert_eq!(&pos(&doc, id), want, "一次 Ctrl+Z 三颗都得回原位");
    }

    // 一颗都没动：不发命令（落地点也就不会弹提示）。
    assert!(
        Canvas::arrange_batch(&ids, &current, &current).is_none(),
        "位置没变就不该产生撤销记录"
    );
}

/// 问题带换行宽度（第 25 轮）：画布宽减 64，窄窗收在 120。
#[test]
fn focus_wrap_width_leaves_sixteen_of_padding_on_both_sides() {
    let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0));
    assert_eq!(focus_wrap_width(wide), 1216.0);
    let narrow = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(120.0, 720.0));
    assert_eq!(
        focus_wrap_width(narrow),
        120.0,
        "窄窗不再往下收，留一个最小行宽"
    );
}

/// 问题带是多行时，带子跟着长高、宽度仍被夹在画布内（长问题换行后靠它）。
#[test]
fn focus_band_grows_with_wrapped_lines_and_stays_inside_the_canvas() {
    let origin = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let one = focus_band_rect(origin, egui::vec2(300.0, 20.0));
    let two = focus_band_rect(origin, egui::vec2(focus_wrap_width(origin), 40.0));
    assert_eq!(one.height(), 34.0, "20 行高 + 上下各 7");
    assert_eq!(two.height(), 54.0, "两行 40 高 + 上下各 7");
    assert_eq!(two.width(), 768.0, "行宽 + 左右内距 = 画布宽 − 32");
    assert!(two.left() >= origin.left() && two.right() <= origin.right());
    assert!(
        (two.center().x - origin.center().x).abs() < 0.01,
        "仍然居中"
    );
}

#[test]
fn sample_path_keeps_a_straight_line_straight() {
    // 采样只为了「把曲线搬上屏幕」，直线段不该被采样成曲线：多了点就多几次描边，
    // 而且端点若不是原样保留，选中强调会比原线短一截。
    let mut path = mw_core::geom::Path::new(mw_core::geom::Point::new(-100.0, 0.0));
    path.line(mw_core::geom::Point::new(0.0, 0.0));
    let pts = sample_path(&path);
    assert_eq!(pts.len(), 2, "直线段不该被采样");
    assert_eq!(pts[0], path.start);
    assert_eq!(pts[1], path.end());
}

#[test]
fn sample_path_samples_a_cubic_twelve_times() {
    // 两个控制点都往上拉：一上一下会在 t=0.5 抵消，采样错不错都看不出来。
    let mut path = mw_core::geom::Path::new(mw_core::geom::Point::new(0.0, 0.0));
    path.cubic(
        mw_core::geom::Point::new(30.0, -40.0),
        mw_core::geom::Point::new(70.0, -40.0),
        mw_core::geom::Point::new(100.0, 0.0),
    );
    let pts = sample_path(&path);
    assert_eq!(pts.len(), 13, "一段三次曲线 = 起点 + 12 采样点");
    assert_eq!(pts[0], path.start);
    assert_eq!(pts[pts.len() - 1], path.end());
    // 曲线在 t=0.5 处应该鼓出直线之外：不鼓就说明控制点被丢了。
    let straight = mw_core::geom::Point::new(50.0, 0.0);
    let off = (pts[6].y - straight.y).abs();
    assert!(off > 10.0, "中点应离直线至少 10px，实际 {off:.2}");
}

#[test]
fn split_around_cuts_the_label_pill_out_of_the_line() {
    // 药丸在合成纹理里压在线上：强调线照直画过去就成了给标签画删除线。
    let pts: Vec<mw_core::geom::Point> = (0..=10)
        .map(|i| mw_core::geom::Point::new(i as f32 * 10.0, 0.0))
        .collect();
    let pill = mw_core::geom::Rect::new(40.0, -12.0, 20.0, 24.0);
    let runs = split_around(&pts, pill);
    assert_eq!(runs.len(), 2, "药丸在中间应断成两段");
    assert!(runs[0].last().unwrap().x <= 40.0, "第一段应停在药丸左边");
    assert!(runs[1].first().unwrap().x >= 60.0, "第二段应从药丸右边开始");
    assert!(runs.iter().all(|r| r.len() >= 2), "不足两点的段应被丢掉");
}

#[test]
fn split_around_leaves_a_line_that_misses_the_pill_alone() {
    let pts: Vec<mw_core::geom::Point> = (0..=4)
        .map(|i| mw_core::geom::Point::new(i as f32 * 10.0, 100.0))
        .collect();
    let pill = mw_core::geom::Rect::new(10.0, -12.0, 20.0, 24.0);
    let runs = split_around(&pts, pill);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].len(), pts.len());
}

#[test]
fn densify_adds_points_so_the_pill_can_be_cut_out_of_a_straight_line() {
    // 直线段只有首末两点，不加密就挖不掉药丸：这正是第一次实现漏掉的那种情况。
    let pts = vec![
        mw_core::geom::Point::new(0.0, 0.0),
        mw_core::geom::Point::new(60.0, 0.0),
    ];
    let dense = densify(&pts, 6.0);
    assert_eq!(dense.len(), 11, "60 长、步长 6 → 首点 + 10 点");
    assert_eq!(dense[0], pts[0]);
    assert_eq!(dense[dense.len() - 1], pts[1]);
    // 加密之后再挖洞，直线也能断成两段。
    let pill = mw_core::geom::Rect::new(24.0, -8.0, 12.0, 16.0);
    let runs = split_around(&dense, pill);
    assert_eq!(runs.len(), 2);
    assert!(runs[0].last().unwrap().x <= 24.0);
    assert!(runs[1].first().unwrap().x >= 36.0);
}

#[test]
fn densify_leaves_short_segments_untouched() {
    let pts = vec![
        mw_core::geom::Point::new(0.0, 0.0),
        mw_core::geom::Point::new(3.0, 4.0),
    ];
    assert_eq!(densify(&pts, 6.0).len(), 2, "短于步长不该插点");
    assert!(densify(&[], 6.0).is_empty());
}

/// 强调的不透明度要跟着关系的淡入淡出一起变：
/// 否则一条正在淡入的线会被强调画成满色，像幽灵线浮在图上。
#[test]
fn emphasis_alpha_fades_with_the_relationship() {
    assert!((emphasis_alpha(1.0, 1.0) - 1.0).abs() < 1e-6);
    assert!((emphasis_alpha(0.30, 0.5) - 0.15).abs() < 1e-6);
    assert_eq!(emphasis_alpha(0.45, 0.0), 0.0);
    // 两头都夹取：档位写错或 alpha 越界都不该画出「比实色更实」的线。
    assert_eq!(emphasis_alpha(3.0, 2.0), 1.0);
    assert_eq!(emphasis_alpha(-1.0, 1.0), 0.0);
}

/// 药丸挖洞的矩形必须**正对** `label_at`：偏一点点就会在线的一侧留下没挖掉的线头。
#[test]
fn label_pill_rect_is_centered_on_the_label() {
    let r = label_pill_rect_at(mw_core::geom::Point::new(100.0, 50.0), 40.0);
    assert!(
        (r.center().x - 100.0).abs() < 1e-4,
        "center_x={}",
        r.center().x
    );
    assert!(
        (r.center().y - 50.0).abs() < 1e-4,
        "center_y={}",
        r.center().y
    );
    // 宽 = label_w + 两侧各 1px 抗锯齿余量。
    assert!((r.w - 42.0).abs() < 1e-4, "w={}", r.w);
    assert!((r.h - (mw_render::instances::REL_LABEL_H + 2.0)).abs() < 1e-4);
}

/// 短标签（「引起」这种两个字）不会把药丸压成一条竖线：有最小宽度兜底。
#[test]
fn label_pill_rect_keeps_a_minimum_width() {
    let narrow = label_pill_rect_at(mw_core::geom::Point::new(0.0, 0.0), 4.0);
    assert!((narrow.w - (mw_render::instances::REL_LABEL_MIN_W + 2.0)).abs() < 1e-4);
    assert!((narrow.center().x - 0.0).abs() < 1e-4);
}

/// 药丸大到盖住整条线（长标签压在短线上）：一段都不该剩。
/// 剩下一段 1 点的「段」会让绘制器拿到退化折线，是崩溃的经典入口。
#[test]
fn split_around_drops_everything_when_the_pill_swallows_the_line() {
    let pts: Vec<mw_core::geom::Point> = (0..=4)
        .map(|i| mw_core::geom::Point::new(i as f32 * 10.0, 0.0))
        .collect();
    let pill = mw_core::geom::Rect::new(-50.0, -50.0, 300.0, 100.0);
    assert!(
        split_around(&pts, pill).is_empty(),
        "全被盖住时不该有残留段"
    );
}

/// 洞前只剩一个孤点：那个点必须被丢掉，不能变成 1 点段；洞后的尾巴照常保留。
#[test]
fn split_around_never_emits_a_one_point_run() {
    let pts = vec![
        mw_core::geom::Point::new(0.0, 0.0),
        mw_core::geom::Point::new(10.0, 0.0),
        mw_core::geom::Point::new(20.0, 0.0),
        mw_core::geom::Point::new(30.0, 0.0),
        mw_core::geom::Point::new(40.0, 0.0),
    ];
    // 只盖住第二点：第一点孤立，第三点起是尾巴。
    let pill = mw_core::geom::Rect::new(5.0, -5.0, 10.0, 10.0);
    let runs = split_around(&pts, pill);
    assert_eq!(runs.len(), 1, "孤点被丢、尾巴留下 ⇒ 只剩一段");
    assert!(runs.iter().all(|r| r.len() >= 2), "不该出现 1 点段");
    assert!((runs[0].first().unwrap().x - 20.0).abs() < 1e-4);
}

/// 全透明的强调不该被画：低于阈值直接跳过加密/挖洞/三角化。
#[test]
fn emphasis_below_the_threshold_is_not_painted() {
    assert!(!emphasis_visible(0.0));
    assert!(
        !emphasis_visible(0.001),
        "等于阈值也算看不见（用的是 > 不是 >=）"
    );
    assert!(emphasis_visible(0.002));
    // 淡出中的关系 × 最轻的档位 ⇒ 也应当被跳过。
    assert!(!emphasis_visible(emphasis_alpha(0.30, 0.0)));
    assert!(emphasis_visible(emphasis_alpha(0.30, 1.0)));
}

/// 三种强调触发条件都没有时，整段绘制可以直接跳过（等价，不是省事）。
#[test]
fn emphasis_is_skipped_only_when_nothing_can_trigger_it() {
    assert!(
        !emphasis_wanted(false, false, false),
        "什么都没指、什么都没选 ⇒ 画不出东西"
    );
    assert!(emphasis_wanted(true, false, false));
    assert!(emphasis_wanted(false, true, false));
    assert!(emphasis_wanted(false, false, true));
}

/// 覆盖层必须画在**合成纹理贴图之后**。
///
/// `canvas_methods_2.rs` 先 `self.compositor.render_texture(...)`，再用
/// `ui.painter_at(rect).image(id, ...)` 把结果当**一张不透明位图**贴上来；egui 按加入次序绘制，
/// 所以在那一行之前画的形状会被整张盖掉。第 50 轮之前，跨度手柄、跨度预览、关系端点手柄、
/// 拖终点预览与注解选中环都画在它前面——它们从来没有在界面上出现过。
///
/// 这条测试只读源码次序（和 `chrome::tests::shell_sources_do_not_hardcode_font_sizes` 同一路数）：
/// 像素级证据在 `shots/golden/cm-relink-drag`（预览线 7 140 px）与 `cm-relink-select-only`（强调 1 081 px）。
#[test]
fn overlays_are_painted_after_the_composited_texture() {
    let src = include_str!("canvas_methods_2.rs");
    let image = src.find(".image(id, rect, uv").expect("贴图那一行不见了");
    for call in [
        "self.paint_span_handles(ui, viewport)",
        "self.paint_span_preview(ui, viewport)",
        "self.paint_relationship_handles(ui, viewport)",
        "self.paint_relink_preview(ui, viewport)",
        "for ring in ann_rings {",
    ] {
        let at = src
            .find(call)
            .unwrap_or_else(|| panic!("找不到覆盖层调用：{call}"));
        assert!(at > image, "{call} 画在贴图之前，会被合成纹理整张盖掉",);
    }
}

// ───────────────────── 打磨轮 31：画布交互十条缺陷的单测 ─────────────────────

use crate::canvas::canvas_methods_3::{
    cancel_fresh_node, concept_relative_command, delete_confirm_plan, esc_plan, select_all_indices,
    soft_newline_insert, take_exact_key_from, take_type_start, CanvasEsc, TypeStart,
};

/// 「选中即打字」要能把这一条字符交给编辑框，并且**只拿一条**。
///
/// 剩下的留在队列里：`TextEdit` 这一帧还要自己收它们。若一次全吞掉，后面的字会丢；
/// 若一条都不吞，用户敲的字就落进了画布快捷键的虚空里（缺陷 #2）。
#[test]
fn type_to_edit_takes_one_char_and_leaves_the_rest_of_the_queue() {
    use egui::{Event, Key, Modifiers};
    let mut events = vec![
        Event::Key {
            key: Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
        Event::Text("你".into()),
        Event::Text("好".into()),
    ];
    let start = take_type_start(&mut events, Modifiers::NONE).expect("应当认出这一条落字");
    match start {
        TypeStart::Replace(text) => assert_eq!(text, "你"),
        TypeStart::Compose => panic!("这是普通文字，不是 IME 组合"),
    }
    // 只摘一条：剩一个 `Text` 与开头那条 `Key`。
    assert_eq!(events.len(), 2, "只该摘走那一对 `Text`，别的原样留下");
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Text(t) if t == "好")));
}

/// 空格、控制字符、换行都不算「落字」：空格在画布上另有归属，接走它等于废掉那个手势。
#[test]
fn type_to_edit_ignores_space_and_control_chars() {
    use egui::{Event, Modifiers};
    for bad in [" ", "\n", "\t", "\u{7}"] {
        let mut events = vec![Event::Text(bad.into())];
        assert!(
            take_type_start(&mut events, Modifiers::NONE).is_none(),
            "{bad:?} 不该触发打字进编辑"
        );
        assert_eq!(events.len(), 1, "不认得的事件不许动");
    }
    // 带 Ctrl 的落字是快捷键掉下来的碎片。
    let mut events = vec![Event::Text("v".into())];
    assert!(take_type_start(&mut events, Modifiers::CTRL).is_none());
}

/// `Shift+Enter` 在光标处插一个换行，光标停在换行**之后**。
#[test]
fn soft_newline_inserts_at_caret_and_keeps_caret_after_the_break() {
    // 光标在「第一」与「行」之间：0..3 是「第一行」。
    let (text, caret) = soft_newline_insert("第一行", Some((1, 1)));
    assert_eq!(text, "第\n一行");
    assert_eq!(caret, 2, "光标要落在换行后面，接着打才是新的一行");

    // 有选区：整段替换（与 TextEdit 里 Shift+Enter 的直觉一致）。
    let (text, caret) = soft_newline_insert("abc", Some((0, 3)));
    assert_eq!(text, "\n");
    assert_eq!(caret, 1);

    // 越界/空选区一律夹到末尾，绝不切进 UTF-8 中间。
    let (text, caret) = soft_newline_insert("中文", Some((99, 99)));
    assert_eq!(text, "中文\n");
    assert_eq!(caret, 3);
    let (text, caret) = soft_newline_insert("", None);
    assert_eq!(text, "\n");
    assert_eq!(caret, 1);
}

/// 带子节点的删除要先确认，叶子一键速删；计数含整片子树并去重。
#[test]
fn delete_confirm_counts_the_whole_subtree_and_skips_leaf_only_deletes() {
    // 手工建一棵 root → A → A1：`doc_with` 把每个都挂在根下，测不出子树。
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    doc.apply(&Command::AddNode {
        parent: root.clone(),
        index: None,
        text: "A".into(),
        style: None,
    })
    .unwrap();
    let a = id_of(&doc, "A");
    doc.apply(&Command::AddNode {
        parent: a.clone(),
        index: None,
        text: "A1".into(),
        style: None,
    })
    .unwrap();
    let a1 = id_of(&doc, "A1");
    assert_eq!(doc.subtree_ids(&a), vec![a.clone(), a1.clone()]);

    // 只删叶子：不确认（高频动线不能被两次点击毁掉）。
    assert_eq!(delete_confirm_plan(&doc, std::slice::from_ref(&a1)), None);
    // 删带子节点的 A：要确认，且数 2（A + A1）。
    assert_eq!(delete_confirm_plan(&doc, std::slice::from_ref(&a)), Some(2));
    // 父子同时选中：后代只算一次。
    assert_eq!(delete_confirm_plan(&doc, &[a.clone(), a1.clone()]), Some(2));
    // 空集合不该弹框。
    assert_eq!(delete_confirm_plan(&doc, &[]), None);
}

/// `Esc` 逐层退：插入关系线 → 关系线 → 聚焦栈 → 多选 → 什么都不做。
#[test]
fn escape_walks_rel_link_then_relationship_then_focus_then_selection() {
    assert!(matches!(esc_plan(true, true, 2, true), CanvasEsc::RelLink));
    assert!(matches!(
        esc_plan(true, false, 0, false),
        CanvasEsc::RelLink
    ));
    assert!(matches!(
        esc_plan(false, true, 2, true),
        CanvasEsc::Relationship
    ));
    assert!(matches!(esc_plan(false, false, 2, true), CanvasEsc::Focus));
    assert!(matches!(
        esc_plan(false, false, 0, true),
        CanvasEsc::Selection
    ));
    assert!(matches!(
        esc_plan(false, false, 0, false),
        CanvasEsc::Nothing
    ));
}

/// 插入关系线的落点：只有「另一个主题」建线；源自身/空白/注解/关系线都取消
/// （取消后这一击照常按普通点击处理，不吞选择）。
#[test]
fn rel_link_step_commits_only_on_another_node() {
    assert_eq!(rel_link_step(None, Some("b")), RelLinkStep::Idle);
    assert_eq!(
        rel_link_step(Some("a"), Some("b")),
        RelLinkStep::Commit("b".into())
    );
    assert_eq!(rel_link_step(Some("a"), Some("a")), RelLinkStep::Cancel);
    assert_eq!(rel_link_step(Some("a"), None), RelLinkStep::Cancel);
}

/// 概念图的 `Tab`/`Enter` 只能产生命题线，绝不能产生树子节点。
#[test]
fn concept_relative_never_produces_a_tree_child() {
    let mut doc = doc_with(&["甲"]);
    doc.apply(&mw_core::Command::SetLayout {
        layout: LayoutSettings {
            kind: "conceptMap".into(),
            ..LayoutSettings::default()
        },
    })
    .unwrap();
    let from = id_of(&doc, "甲");
    let anchor = (100.0, 200.0, 80.0, 32.0);

    let below = concept_relative_command(&from, AddKind::Child, anchor);
    let right = concept_relative_command(&from, AddKind::Sibling, anchor);
    // `Tab` 落下方、`Enter` 落右侧：命题网没有「同级」，两个键只差方向。
    assert!(matches!(
        &below,
        mw_core::Command::AddLinkedConcept { x, y, .. } if *x == 100.0 && *y == 256.0
    ));
    assert!(matches!(
        &right,
        mw_core::Command::AddLinkedConcept { x, y, .. } if *x == 236.0 && *y == 200.0
    ));

    // 真落到文档上：新概念没有 `parent_id`，而且有一条从 `from` 出发的命题线。
    doc.apply(&below).unwrap();
    let new_id = doc
        .doc_order()
        .into_iter()
        .find(|id| id != &from && doc.node(id).is_some_and(|n| n.text == CONCEPT_PLACEHOLDER))
        .expect("应当长出一颗新概念");
    assert!(
        doc.node(&new_id).unwrap().parent_id.is_none(),
        "概念图里绝不能出现树父子关系"
    );
    assert_eq!(
        doc.node(&from).unwrap().relationships.len(),
        1,
        "新概念必须同时连上一条命题线"
    );
}

/// 取消一枚刚建出来的节点：一次撤销就够，不该在栈里留下 `DeleteNodes`。
#[test]
fn cancelling_a_fresh_node_edit_leaves_no_extra_undo_step() {
    let mut doc = doc_with(&[]);
    let root = doc.root_id.clone();
    let depth_before = doc.undo_depth();

    doc.apply(&mw_core::Command::AddNode {
        parent: root.clone(),
        index: None,
        text: String::new(),
        style: None,
    })
    .unwrap();
    let made = depth_before + 1;
    let id = doc
        .doc_order()
        .into_iter()
        .find(|id| id != &root)
        .expect("新建的节点");
    let after_add = doc.undo_depth();
    assert_eq!(after_add, made);

    // 栈顶还是「建它」那条 ⇒ 一次 `undo` 收回，栈深回到建之前。
    assert!(cancel_fresh_node(&mut doc, &id, Some(after_add)));
    assert_eq!(doc.undo_depth(), depth_before, "撤销栈里不该多出一步");
    assert!(doc.node(&id).is_none(), "节点要跟着消失");

    // 深度对不上（用户中途干过别的）：退回「删掉这枚空盒子」，只压一条命令。
    doc.apply(&mw_core::Command::AddNode {
        parent: root.clone(),
        index: None,
        text: String::new(),
        style: None,
    })
    .unwrap();
    let id2 = doc
        .doc_order()
        .into_iter()
        .find(|id| id != &root)
        .expect("第二次新建的节点");
    let stale = Some(0);
    assert!(!cancel_fresh_node(&mut doc, &id2, stale));
    assert!(doc.node(&id2).is_none(), "节点仍要删掉");
    assert_eq!(
        doc.undo_depth(),
        depth_before + 2,
        "退回老口径：新建 + 删除两条命令，栈深不该变浅"
    );
}

/// 全选只覆盖场景里的可见节点；主选留在最后。
#[test]
fn select_all_visible_skips_what_folding_hid() {
    // 折叠 A 之后 A1 不在场景里 ⇒ 全选拿不到它。手工建 root → A → A1 与根下的 B。
    let mut doc = Document::new("根");
    let root = doc.root_id.clone();
    for (parent, text) in [(&root, "A"), (&root, "B")] {
        doc.apply(&Command::AddNode {
            parent: (*parent).clone(),
            index: None,
            text: (*text).into(),
            style: None,
        })
        .unwrap();
    }
    let a = id_of(&doc, "A");
    doc.apply(&Command::AddNode {
        parent: a.clone(),
        index: None,
        text: "A1".into(),
        style: None,
    })
    .unwrap();
    doc.apply(&mw_core::Command::SetCollapsed {
        id: a.clone(),
        collapsed: true,
    })
    .unwrap();
    let scene = scene_of(&doc);
    assert_eq!(scene.ids.len(), 3, "根 + A + B：A1 被折叠藏起来了");

    let all = select_all_indices(scene.ids.len(), Some(2));
    assert_eq!(all.len(), 3);
    assert_eq!(
        all.last(),
        Some(&2),
        "主选要排在最后（Selection 的最后一位是主选）"
    );
    assert_eq!(all.iter().filter(|&&i| i == 2).count(), 1, "主选不许重复");

    // 主选被折叠藏起来了：不强塞进集合。
    let all = select_all_indices(3, Some(9));
    assert_eq!(all, vec![0, 1, 2]);
}

/// 缩放区间是 10%–500%，两个方向都要夹住；低于下限的视图不该被「一跳」抬到 10%。
#[test]
fn zoom_range_is_ten_percent_to_five_hundred() {
    assert_eq!(MIN_ZOOM, 0.1);
    assert_eq!(MAX_ZOOM, 5.0);
    let viewport = [1000.0, 700.0];
    let anchor = Point::new(500.0, 350.0);

    let mut camera = Camera::new(Point::new(0.0, 0.0), 1.0, 1.0);
    for _ in 0..40 {
        zoom_at_anchor(&mut camera, anchor, viewport, 1.2);
    }
    assert!(
        (camera.zoom - MAX_ZOOM).abs() < 1e-4,
        "上限 {}",
        camera.zoom
    );
    for _ in 0..80 {
        zoom_at_anchor(&mut camera, anchor, viewport, 1.0 / 1.2);
    }
    assert!(
        (camera.zoom - MIN_ZOOM).abs() < 1e-4,
        "下限 {}",
        camera.zoom
    );

    // 取景可以停在区间以下（20k 文档要缩到 0.5%）；此时再缩不该「跳上来」。
    let mut camera = Camera::new(Point::new(0.0, 0.0), 0.005, 1.0);
    zoom_at_anchor(&mut camera, anchor, viewport, 1.0 / 1.2);
    assert!(
        camera.zoom < MIN_ZOOM,
        "从低于下限的取景继续缩小，必须待在下限以下：{}",
        camera.zoom
    );
}

/// 缩放读数（X21）：`pct >= 10` 用整数，否则一位小数。
///
/// 一位小数不是审美偏好：20k 文档 fit 下来 0.5%，用整数位会写成 `0%`——
/// 看上去像相机坏了。`0.996 → 100%` 与 `12.345` 是两个边界（四舍五入 + f32 半值）。
#[test]
fn format_zoom_switches_precision_at_ten_percent() {
    assert_eq!(format_zoom(0.005), "0.5%", "fit 出来的 0.5% 不许写成 0%");
    assert_eq!(format_zoom(0.05), "5.0%");
    assert_eq!(format_zoom(0.1), "10%", "10% 是整数位的下界");
    assert_eq!(format_zoom(0.996), "100%");
    assert_eq!(format_zoom(1.0), "100%");
    assert_eq!(format_zoom(5.0), "500%");
    // 12.345×100 在 f32 里正好落在 1234.5 这个半值上，`{:.0}` 按「半值取偶」给 1234。
    // 钉住它不是为了这条数字好看，而是承认「f32 半值」是这条读数的真实边界。
    assert_eq!(format_zoom(12.345), "1234%");

    // 档位必须先四舍五入再判：9.96 进位到 10.0 就该印整数档 `10%`，
    // 先判档位再四舍五入会印成 `10.0%`——同一档位两种宽度，每次进位都跳一下。
    assert_eq!(format_zoom(0.0996), "10%");
    assert_eq!(format_zoom(0.0994), "9.9%");
    // 二次舍入的活边界：默认画布 fit 出来的 `0.85450012`（真值 85.45%）必须读 85%，
    // 不许先收成 85.5 再进成 86%——那是一个字的像素漂，画布系列金样会跟着红。
    assert_eq!(format_zoom(0.854_500_1), "85%");
    assert_eq!(format_zoom(85.450_01 / 100.0), "85%");
    // 非有限值不许印成 `NaN%` / `inf%`：相机坏掉时读数该是一句「没有读数」。
    assert_eq!(format_zoom(f32::NAN), "—");
    assert_eq!(format_zoom(f32::INFINITY), "—");
    assert_eq!(format_zoom(f32::NEG_INFINITY), "—");
}

/// `Shift+滚轮` 换轴（`docs/06:149`）：不按 Shift 原样返回，按了把纵向搬到横轴。
#[test]
fn wheel_pan_switches_axis_only_with_shift() {
    // 不按 Shift：一个分量都不许改——纵向滚动本来就走纵向平移，手感与之前逐像素一致。
    for s in [
        Vec2::new(0.0, -240.0),
        Vec2::new(120.0, 0.0),
        Vec2::new(12.5, -7.5),
        Vec2::ZERO,
    ] {
        assert_eq!(wheel_pan(s, false), s, "不按 Shift 不许改轴向：{s:?}");
    }
    // 按 Shift：纵向分量搬到横轴，纵向清零。
    assert_eq!(
        wheel_pan(Vec2::new(0.0, -240.0), true),
        Vec2::new(-240.0, 0.0)
    );
    // 两个方向都有时纵向优先（鼠标滚轮报的就是纵向，横向是触控板的零头）。
    assert_eq!(
        wheel_pan(Vec2::new(40.0, -240.0), true),
        Vec2::new(-240.0, 0.0)
    );
    // 纵向恰好为 0：保留原来的横向分量，别把真·横向滚轮吃掉。
    assert_eq!(
        wheel_pan(Vec2::new(120.0, 0.0), true),
        Vec2::new(120.0, 0.0)
    );
}

/// 复位到 100%（`Ctrl+0` / 点百分比 / 菜单）与缩放同一条纪律：只改倍率、不动中心，
/// 已在 100% 时返回 `false`（菜单据此禁用，画布不必重绘）。
#[test]
fn zoom_reset_returns_to_hundred_percent_once() {
    let mut cam = Camera::new(Point::new(10.0, 20.0), 0.35, 1.0);
    assert!(zoom_reset(&mut cam), "35% 复位必须动相机");
    assert_eq!(cam.zoom, 1.0);
    // 中心不动：复位是绕视口中心缩放，轨迹里 world 就记 `camera.center`。
    assert_eq!(cam.center, Point::new(10.0, 20.0));
    assert!(!zoom_reset(&mut cam), "已在 100% 不许再报变了");
    // fit 出来的 0.5% 照常回 100%，不被 MIN_ZOOM 下限挡住。
    let mut tiny = Camera::new(Point::ZERO, 0.005, 1.0);
    assert!(zoom_reset(&mut tiny));
    assert_eq!(tiny.zoom, 1.0);
    // 「复位也受钳位约束」这句话要成立，1.0 必须是交互区间的成员。
    const { assert!(MIN_ZOOM <= 1.0 && 1.0 <= MAX_ZOOM) };
}

/// 缩放轨迹的 `src=` 取值是验收口径（X21 报告与探针照抄这一份）。
#[test]
fn zoom_src_names_are_the_trace_contract() {
    let all = [
        ZoomSrc::Wheel,
        ZoomSrc::Pinch,
        ZoomSrc::Key,
        ZoomSrc::Button,
        ZoomSrc::Minimap,
        ZoomSrc::Reset,
    ];
    let names: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        names,
        ["wheel", "pinch", "key", "button", "minimap", "reset"]
    );
}

/// `zoom_anchor` 之外的两种「没发生的缩放」各有自己的句法（X21，Lead 拍板）：
/// 编辑期锁写 `zoom_blocked src=… editing=1`，限位写 `zoom_anchor … clamped=1`。
/// 两条都只在 `zoom_step_at` 一处产生，别的地方再写一份就等于两套合同。
#[test]
fn zoom_trace_vocabulary_is_single_sourced() {
    let methods = include_str!("canvas_methods_1.rs");
    assert!(
        methods.contains("\"zoom_blocked src={} editing=1\""),
        "编辑期锁必须写独立的 zoom_blocked 行"
    );
    assert!(
        !methods.contains("locked=1"),
        "被锁住不该再借 zoom_anchor 的位置（`locked=` 后缀已废）"
    );
    assert!(
        methods.contains("\" clamped=1\""),
        "限位必须带 clamped=1 后缀，倍率没变也要留一行"
    );
    assert!(
        !methods.contains("reason=limit"),
        "没实现的 `reason=limit` 口径不许留在代码里"
    );
    // 小地图只留 `zoom_anchor src=minimap`：旧的 `minimap zoom factor=…` 是同一件事的
    // 第二套写法，探针会分叉。
    let shell = include_str!("../main.rs");
    assert!(
        !shell.contains("minimap zoom factor="),
        "小地图不许再打第二条缩放轨迹"
    );
    assert!(
        shell.contains("ZoomSrc::Minimap"),
        "小地图滚轮仍要走同一条入口"
    );
}

/// `Ctrl+0` 与缩放条百分比必须是**同一条路**（X21）。
///
/// 过去 `Ctrl+0` 在画布快捷键里直接写 `camera.zoom = 1.0`，绕过了 `zoom_reset_action`
/// ——既不写轨迹，也不受钳位口径约束。Canvas 要 wgpu `RenderState` 才造得出来，
/// 单测里没有实例，所以这条用源码钉住「复位只有一个写入点」。
#[test]
fn ctrl_zero_shares_the_single_reset_path() {
    let shortcuts = include_str!("canvas_methods_3.rs");
    assert!(
        shortcuts.contains("self.zoom_reset_action()"),
        "Ctrl+0 必须调 zoom_reset_action"
    );
    assert!(
        !shortcuts.contains("self.camera.zoom = 1.0"),
        "Ctrl+0 不许再直接写 camera.zoom"
    );
    let shell = include_str!("../main.rs");
    assert!(
        shell.contains("canvas.zoom_reset_action()"),
        "缩放条百分比走同一个方法"
    );
    assert!(!shell.contains("self.camera.zoom = 1.0"));
}

/// 精确修饰键：只摘「完全一致」的那一条，`Shift+Enter` 不会被纯 `Enter` 吃掉。
#[test]
fn exact_key_taking_leaves_shift_combos_for_their_own_handler() {
    use egui::{Event, Key, Modifiers};
    let mut events = vec![Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    }];
    // 纯 Enter 的匹配不该命中 Shift+Enter —— 这正是缺陷 #1 的机制。
    assert!(!take_exact_key_from(
        &mut events,
        Modifiers::NONE,
        Key::Enter
    ));
    assert_eq!(events.len(), 1, "没命中的事件必须原样留着");
    // 精确匹配才摘走。
    assert!(take_exact_key_from(
        &mut events,
        Modifiers::SHIFT,
        Key::Enter
    ));
    assert!(events.is_empty());
}

/// 空白短语的提交判定（打磨轮 40 · C16 → `docs/33` §2 B1a）。
///
/// 概念图里没有短语的边不构成命题（`docs/30` §5）。模型层已经拒绝
/// `SetRelationshipLabel{label: None}`；壳层必须先判一次，否则编辑器会静默关掉。
///
/// **新建的线留空不再撤线**（`docs/33` §2 B1a）：落成「未命名」占位，
/// 用户事后再补——改前是「留空就把线撤掉」，体感是「连不上」。
/// 撤线只留给显式 `Esc`（直接发 `DeleteRelationship`，不经过这个函数）。
#[test]
fn blank_phrase_commit_follows_the_concept_map_contract() {
    // 原文没动：不发命令（不压撤销栈、不把文档标脏）。
    assert_eq!(
        phrase_commit(true, "带来", "带来", false),
        PhraseCommit::Unchanged
    );
    assert_eq!(
        phrase_commit(true, " 带来 ", "带来", false),
        PhraseCommit::Unchanged,
        "首尾空白不算改动"
    );
    // 既有命题被清空：拒绝提交，保留原短语。
    assert_eq!(
        phrase_commit(true, "", "带来", false),
        PhraseCommit::RejectEmpty
    );
    assert_eq!(
        phrase_commit(true, "   ", "带来", false),
        PhraseCommit::RejectEmpty,
        "全是空白同样算清空"
    );
    // 新建线留空：**线留下**，短语落成占位（改前是 DropEdge 整条撤掉）。
    assert_eq!(
        phrase_commit(true, "", "", true),
        PhraseCommit::Set(Some(CONCEPT_UNNAMED_LINK.to_string())),
        "留空也该让线落地，短语留占位"
    );
    assert_eq!(
        phrase_commit(true, "  ", "", true),
        PhraseCommit::Set(Some(CONCEPT_UNNAMED_LINK.to_string()))
    );
    // 正常提交。
    assert_eq!(
        phrase_commit(true, " 推动 ", "", true),
        PhraseCommit::Set(Some("推动".into()))
    );
    assert_eq!(
        phrase_commit(true, "推动", "带来", false),
        PhraseCommit::Set(Some("推动".into()))
    );
    // 思维导图里清空短语仍然合法（模型只禁止概念图）。
    assert_eq!(
        phrase_commit(false, "", "带来", false),
        PhraseCommit::Set(None)
    );
}

// ───────────────────── 打磨轮 49：选中互斥（M26） ─────────────────────

/// 三条选中道（节点 / 注解 / 关系线）一次只允许一条非空。
///
/// 这条不变式以前没人守：点关系线时清了节点，但方向键遍历、框选松手、
/// 检查器选注解都能在关系线还选着的时候再立起另一道。`foreign_lanes` 是
/// 唯一一把尺：给一条要保留的道，其余非空的道都要报 `true`。
#[test]
fn selection_lanes_keep_exactly_one_lane() {
    use SelectionLane::{Annotation, Node, Relationship};

    // 三道都非空：只留下指定的那条。
    assert_eq!(foreign_lanes(Node, true, 2, 3), [true, true, false]);
    assert_eq!(foreign_lanes(Annotation, true, 2, 3), [true, false, true]);
    assert_eq!(foreign_lanes(Relationship, true, 2, 3), [false, true, true]);

    // 已经只有一条道：什么都不用清（顺序是 关系线 / 注解 / 节点）。
    assert_eq!(foreign_lanes(Node, false, 0, 3), [false, false, false]);
    assert_eq!(
        foreign_lanes(Annotation, false, 2, 0),
        [false, false, false]
    );
    assert_eq!(
        foreign_lanes(Relationship, true, 0, 0),
        [false, false, false]
    );

    // 全空同样是稳定状态。
    assert_eq!(foreign_lanes(Node, false, 0, 0), [false, false, false]);
}

/// M26 互斥不变式：任何「选一个节点」的入口都必须先清掉关系线 / 注解两道。
///
/// `Canvas` 要 wgpu `RenderState` 才造得出来，行为测试在这里跑不了，改用源码守卫：
/// 三个画布方法文件里不许再出现裸的 `self.sel = Selection::one(`。修复前这里有
/// 十七处（悬停工具条 ×2、连线拖拽 ×2、越阈值拖拽、折叠圆点、大纲七项、新建主题、
/// 右键节点分支），它们都把关系线/注解选中留了下来，下一次 `Delete` 按优先级删错
/// 对象。唯一合法入口是 `select_node_only`（内部走 `clear_foreign_selection` + `replace`）。
#[test]
fn node_selection_entries_go_through_the_mutex_helper() {
    for (name, src) in [
        ("canvas_methods_1.rs", include_str!("canvas_methods_1.rs")),
        ("canvas_methods_2.rs", include_str!("canvas_methods_2.rs")),
        ("canvas_methods_3.rs", include_str!("canvas_methods_3.rs")),
    ] {
        let hits: Vec<usize> = src
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains("self.sel = Selection::one("))
            .map(|(i, _)| i + 1)
            .collect();
        assert!(
            hits.is_empty(),
            "{name} 仍有绕过互斥的裸赋值，行号：{hits:?}"
        );
    }
}

/// 一次 `Delete` 只作用一道：关系线 → 注解 → 节点。
///
/// 以前这里连着调三个删除器：关系线选着、节点也选着时，一次 Delete
/// 既删线又删掉整棵子树（复现见 `shots/x100/M26/report.md` §1）。
#[test]
fn delete_press_reaches_exactly_one_lane() {
    use DeleteLane::{Annotation, Nodes, Nothing, Relationship};

    assert_eq!(delete_lane(true, 2, 3), Relationship, "关系线优先");
    assert_eq!(delete_lane(false, 2, 3), Annotation, "其次是注解");
    assert_eq!(delete_lane(false, 0, 3), Nodes, "最后才是节点");
    assert_eq!(delete_lane(false, 0, 0), Nothing);
    // 单道也存在时也要认出来，不许滑到下一道。
    assert_eq!(delete_lane(true, 0, 0), Relationship);
    assert_eq!(delete_lane(false, 1, 0), Annotation);
}

/// M30 + X21：小地图滚轮的锚点数学——指针底下那块世界内容必须原地不动。
///
/// 与画布内滚轮（收屏幕点）同一纪律，只是这里收的是**世界点**：小地图的指针不在
/// 画布视口里，先在卡上投影出世界坐标，再用 `Camera::world_to_screen` 换回画布屏幕点，
/// 走进**同一个** `zoom_step_camera`（`Canvas::zoom_step_at` 的纯核）。
/// X21 之前这里是第二条世界锚点式子（`zoom_around_world`），两条式子数学等价但各自
/// 演进过一次（第二次写成乘 k，锚点跟着相机中心反向漂）；现在只剩一条，
/// 这条测试钉的就是「投影 + 统一入口」不漂。
#[test]
fn zooming_around_a_world_point_pins_it_on_screen() {
    let vp = [800.0, 600.0];
    let at = Point::new(120.0, -80.0);
    // 世界 → 屏幕：`(p − center) × zoom + 视口中心`。
    let screen = |cam: &Camera, p: Point| {
        Point::new(
            (p.x - cam.center.x) * cam.zoom + vp[0] * 0.5,
            (p.y - cam.center.y) * cam.zoom + vp[1] * 0.5,
        )
    };

    let mut cam = Camera::new(Point::ZERO, 1.0, 1.0);
    let before = screen(&cam, at);
    let anchor = cam.world_to_screen(at, vp);
    assert!(
        zoom_step_camera(&mut cam, anchor, vp, 1.2),
        "1.0 → 1.2 在区间内，必须动相机"
    );
    let after = screen(&cam, at);
    assert!(
        (before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3,
        "锚点必须原地不动：({before:?}) → ({after:?})，相机 {cam:?}"
    );
    assert!(
        (cam.zoom - 1.2).abs() < 1e-6,
        "倍率照因子走，实际 {}",
        cam.zoom
    );

    // 反向同一因子回得到原相机——滚上去再滚下来不该留残差。
    let anchor = cam.world_to_screen(at, vp);
    assert!(zoom_step_camera(&mut cam, anchor, vp, 1.0 / 1.2));
    assert!(
        cam.center.x.abs() < 1e-2 && cam.center.y.abs() < 1e-2 && (cam.zoom - 1.0).abs() < 1e-6,
        "一滚一回必须回到原位，实际 {:?}",
        cam
    );
}

/// X21：新入口 `Canvas::zoom_step_at` 的**纯核**（`zoom_step_camera`）——
/// 任何屏幕点在缩放前后都钉在原地，四个档位 × 四个落点都要成立。
///
/// 这是「锚点不漂」那条纪律走新入口的版本：与上面小地图那条是同一件事的两个坐标
/// 口径（那边世界点先投影回屏幕点）。顺带钉住「停在下限/上限、因子非法」时返回
/// `false` 且一个字段都不动——调用方据此不必白重绘一帧。
#[test]
fn zoom_step_pins_the_screen_anchor_on_every_entry() {
    let vp = [1024.0, 768.0];
    let anchors = [
        Point::new(0.0, 0.0),
        Point::new(1024.0, 768.0),
        Point::new(320.0, 210.0),
        Point::new(800.0, 75.0),
    ];
    for start in [0.1_f32, 0.5, 1.0, 3.0] {
        for anchor in anchors {
            let mut cam = Camera::new(Point::new(-40.0, 25.0), start, 1.0);
            let before = cam.screen_to_world(anchor, vp);
            assert!(
                zoom_step_camera(&mut cam, anchor, vp, 1.2),
                "{start} 放大一档必须动相机"
            );
            let after = cam.screen_to_world(anchor, vp);
            assert!(
                (before.x - after.x).abs() < 1e-2 && (before.y - after.y).abs() < 1e-2,
                "锚点漂了（{start} → {}）：{before:?} → {after:?}",
                cam.zoom
            );
            // 轨迹里的 `drift=` 量的就是这条：同一世界点缩放后在屏幕上还得落在锚点上。
            let pinned = cam.world_to_screen(before, vp);
            assert!(
                (pinned.x - anchor.x).abs() < 1e-2 && (pinned.y - anchor.y).abs() < 1e-2,
                "屏幕上漂了（{start} → {}）：{pinned:?} vs 锚点 {anchor:?}",
                cam.zoom
            );
            // 一放一收回到原档（锚点同样不漂）。
            assert!(zoom_step_camera(&mut cam, anchor, vp, 1.0 / 1.2));
            assert!(
                (cam.zoom - start).abs() < 1e-4,
                "一放一收必须回原档：{start} vs {}",
                cam.zoom
            );
        }
    }

    // 停在 MAX_ZOOM：整段不动，返回 false（钳位后只改中心是更坏的错）。
    let mut maxed = Camera::new(Point::new(7.0, -3.0), MAX_ZOOM, 1.0);
    assert!(
        !zoom_step_camera(&mut maxed, Point::new(400.0, 300.0), vp, 1.2),
        "停在 MAX_ZOOM 不许偷偷搬中心"
    );
    assert_eq!(maxed.center, Point::new(7.0, -3.0));

    // `fit` 能把 20k 文档放到 0.5%：从那里继续缩小不许被 MIN_ZOOM=0.1 一把抬回二十倍
    // （`zoom_at_anchor` 里「下界不越过当前值」那条规矩）。再缩一档只能原地不动。
    let mut tiny = Camera::new(Point::ZERO, 0.005, 1.0);
    assert!(!zoom_step_camera(
        &mut tiny,
        Point::new(400.0, 300.0),
        vp,
        1.0 / 1.2
    ));
    assert!(
        (tiny.zoom - 0.005).abs() < 1e-6,
        "0.005 再缩一档必须原地不动，实际 {}",
        tiny.zoom
    );

    // 病态因子一律不动相机：NaN / inf / 0 / 负数都不该把取景带跑。
    let mut sick = Camera::new(Point::new(3.0, 4.0), 1.0, 1.0);
    for f in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            !zoom_step_camera(&mut sick, Point::new(400.0, 300.0), vp, f),
            "因子 {f} 不该动相机"
        );
        assert_eq!(sick.center, Point::new(3.0, 4.0), "因子 {f} 之后中心要原样");
        assert_eq!(sick.zoom, 1.0, "因子 {f} 之后倍率要原样");
    }

    // 还没画过一帧（视口是 0×0）：退化成纯钳位，区间照样守住。
    let mut early = Camera::new(Point::ZERO, 1.0, 1.0);
    assert!(zoom_step_camera(
        &mut early,
        Point::new(0.5, 0.5),
        [0.0, 0.0],
        1.2
    ));
    assert!((early.zoom - 1.2).abs() < 1e-6, "退化视口也要按因子走");
    assert_eq!(early.center, Point::ZERO, "没有视口就没法锚定，中心不动");
    for _ in 0..40 {
        zoom_step_camera(&mut early, Point::new(0.5, 0.5), [0.0, 0.0], 1.2);
    }
    assert!(
        (early.zoom - MAX_ZOOM).abs() < 1e-4,
        "退化视口也要受上限约束"
    );
}

/// BUG-10：重叠告警边的老化只读注入时钟，到点自动消失。
///
/// 以前 `paint_overlap_flash` 直接读 `Instant::now()`，金样按固定帧数截帧、
/// 墙钟却自由走动，同一份脚本两遍跑出不同画面（M16 口径）。现在
/// `paint_overlap_flash` 走 `self.now`（真机是墙钟、金样是帧计数虚拟时钟），
/// 这条钉住纯函数 `overlap_flash_live` 的边界：恰好到 0.25s 就必须灭。
#[test]
fn overlap_flash_expires_on_the_injected_clock() {
    let dur = OVERLAP_FLASH as f64;
    assert!(overlap_flash_live(10.0, 10.0, dur), "刚亮起时必须可见");
    assert!(
        overlap_flash_live(10.0, 10.0 + dur - 0.01, dur),
        "到期前一帧还该亮着"
    );
    assert!(
        !overlap_flash_live(10.0, 10.0 + dur, dur),
        "恰好到期就必须灭（半开区间 [started, started+dur)）"
    );
    assert!(
        !overlap_flash_live(10.0, 10.0 + dur + 0.5, dur),
        "过期后不能再亮"
    );
    // 时钟回拨（金样 `warmup` 与真机混跑时可能出现的负差）不许把告警卡在半亮。
    assert!(overlap_flash_live(10.0, 9.5, dur), "回拨时按仍有效处理");
    // 验收旋钮能把窗口拉长：金样在脚本结束后 ~0.6s 才截帧，0.25s 的窗口活不到。
    assert!(overlap_flash_live(10.0, 12.0, 2.0), "窗口拉长后仍可见");
    assert!(!overlap_flash_live(10.0, 12.5, 2.0), "拉长也要按时刻灭");
}
