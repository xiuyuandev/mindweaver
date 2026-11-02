//! 笔记库的纯逻辑单测（`docs/28` §7.11 的边界条件）。
//!
//! 只钉**画不出来的那部分**：分档、分栏、Tab 环、行序、选择语义、来源短语。
//! 外观一律交给 `library-*` 十场金样（§7.10），两边不重复。
//!
//! 这批测试的共同前提是「一份数据口径」：侧栏计数、列表行、页头计数都必须
//! 从同一个 `CardQuery` 出来。旧版「收件箱 4 = 全部 4」就是各面自己数一遍数出来的，
//! 所以这里专门有一条测试把两个数摆在一起比。

use super::*;
use mw_core::note::{CardScope, CardSort};

// ---------------------------------------------------------------------------
// 分档与分栏
// ---------------------------------------------------------------------------

#[test]
fn pane_breakpoints_follow_7_2() {
    assert_eq!(Pane::of(1199.0), Pane::Two);
    assert_eq!(Pane::of(1200.0), Pane::Three);
    assert_eq!(Pane::of(840.0), Pane::Two);
    assert_eq!(Pane::of(839.0), Pane::One);
    assert!(!Pane::One.sidebar_docked());
    assert!(Pane::Two.sidebar_docked());
    assert!(!Pane::Two.preview_docked());
    assert!(Pane::Three.preview_docked());
}

#[test]
fn three_pane_keeps_preview_at_least_480() {
    // §7.2：三栏时预览余量 ≥480。切档线（1200）减去两栏的最大宽（280+440）
    // 正好是 480，所以这个下限在 `Pane::of` 判过档之后**天然成立**——
    // 这条测试钉的就是这个巧合不是巧合：把两栏都拖到头也不许挤掉预览。
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(BREAK_THREE, 800.0));
    let cols = columns(area, Pane::Three, SIDEBAR_MAX, LIST_MAX);
    let (s, l, p) = (
        cols.sidebar.unwrap(),
        cols.list.unwrap(),
        cols.preview.unwrap(),
    );
    assert!(
        near(p.width(), PREVIEW_MIN),
        "预览 {} ≠ {}",
        p.width(),
        PREVIEW_MIN
    );
    assert!(
        near(s.width(), SIDEBAR_MAX) && near(l.width(), LIST_MAX),
        "两栏不该被收"
    );
    assert!(near(p.left(), l.right()));
    assert!(near(l.left(), s.right()));
}

#[test]
fn three_pane_shrinks_the_list_before_the_sidebar() {
    // 收的顺序是先列表后侧栏：预览是阅读面，版心 600 折成两半比列表窄一点更难读。
    // 1050 宽在 `Pane::of` 眼里已经是两栏，这里直接喂 `Pane::Three` 是为了钉
    // `columns` 自己的兜底——切档与收缩各判一次，才不会出现「1199 宽却是三栏」的缝。
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1050.0, 700.0));
    let cols = columns(area, Pane::Three, SIDEBAR_W, LIST_W);
    let (s, l, p) = (
        cols.sidebar.unwrap(),
        cols.list.unwrap(),
        cols.preview.unwrap(),
    );
    assert!(near(s.width(), SIDEBAR_W), "侧栏不该先被收：{}", s.width());
    assert!(near(l.width(), LIST_MIN), "列表该先收到下限：{}", l.width());
    assert!(
        p.width() >= PREVIEW_MIN,
        "预览 {} < {}",
        p.width(),
        PREVIEW_MIN
    );
}

#[test]
fn three_pane_shrinks_sidebar_only_as_last_resort() {
    // 1120 宽：侧栏 200 + 列表 320 之后只剩 600，仍然够 480；
    // 再窄一点就逼侧栏落到 200。这里取一个「列表已经到下限、侧栏必须让步」的宽度。
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
    let cols = columns(area, Pane::Three, SIDEBAR_W, LIST_W);
    let (s, l, p) = (
        cols.sidebar.unwrap(),
        cols.list.unwrap(),
        cols.preview.unwrap(),
    );
    assert!(near(s.width(), SIDEBAR_MIN));
    assert!(near(l.width(), LIST_MIN));
    assert!(near(p.width(), area.width() - SIDEBAR_MIN - LIST_MIN));
}

#[test]
fn two_and_one_pane_have_no_preview_column() {
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
    let two = columns(area, Pane::Two, SIDEBAR_W, LIST_W);
    assert!(two.sidebar.is_some());
    assert!(two.list.is_some());
    assert!(two.preview.is_none());
    assert!(near(two.list.unwrap().width(), area.width() - SIDEBAR_W));

    let one = columns(area, Pane::One, SIDEBAR_W, LIST_W);
    assert!(one.sidebar.is_none());
    assert!(one.preview.is_none());
    assert!(near(one.list.unwrap().width(), area.width()));
}

#[test]
fn sidebar_width_is_clamped_not_wrapped() {
    // 拖宽把手把值写进 `sidebar_w`，分栏再夹一次：夹取只能在分栏里发生一次，
    // 否则「拖到 300 又被记成 280」这种缝会在两处各判一遍时出现。
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 800.0));
    let wide = columns(area, Pane::Three, 900.0, 900.0);
    assert!(near(wide.sidebar.unwrap().width(), SIDEBAR_MAX));
    let narrow = columns(area, Pane::Three, 10.0, 10.0);
    assert!(near(narrow.sidebar.unwrap().width(), SIDEBAR_MIN));
    assert!(near(narrow.list.unwrap().width(), LIST_MIN));
}

#[test]
fn split_header_gives_48_and_the_rest() {
    let screen = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(1440.0, 900.0));
    let (header, area) = split_header(screen);
    assert!(near(header.height(), HEADER_H));
    assert!(near(area.top(), HEADER_H));
    assert!(near(area.height(), 900.0 - HEADER_H));
    assert!(near(area.width(), 1440.0));
}

#[test]
fn split_header_degenerates_on_a_tiny_window() {
    // 高不足 48 时页头吃满、内容区为空：`columns` 再拿到 0 高也不能崩。
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 20.0));
    let (header, area) = split_header(screen);
    assert!(near(header.height(), 20.0));
    assert!(area.height() <= 0.0);
}

// ---------------------------------------------------------------------------
// Tab 环（§7.8：侧栏 → 列表 → 预览）
// ---------------------------------------------------------------------------

#[test]
fn tab_ring_three_pane_cycles_all_three() {
    let (p, d, o) = (Pane::Three, false, false);
    assert_eq!(Zone::List.next(p, d, o), Zone::Preview);
    assert_eq!(Zone::Preview.next(p, d, o), Zone::Sidebar);
    assert_eq!(Zone::Sidebar.next(p, d, o), Zone::List);
    assert_eq!(Zone::List.prev(p, d, o), Zone::Sidebar);
    assert_eq!(Zone::Sidebar.prev(p, d, o), Zone::Preview);
}

#[test]
fn tab_ring_two_pane_skips_the_docked_preview() {
    // 两栏没有预览列；整页预览没打开时环里只有两站。
    let (p, d, o) = (Pane::Two, true, false);
    assert_eq!(Zone::Sidebar.next(p, d, o), Zone::List);
    assert_eq!(Zone::List.next(p, d, o), Zone::Sidebar);
    // 预览整页打开后它变成第三站。
    assert_eq!(Zone::List.next(p, d, true), Zone::Preview);
    assert_eq!(Zone::Preview.next(p, d, true), Zone::Sidebar);
}

#[test]
fn tab_ring_one_pane_skips_a_closed_drawer() {
    // 单栏且抽屉关着：侧栏不可停，`Tab` 只剩列表一站，按了也留在原地。
    let (p, o) = (Pane::One, false);
    assert_eq!(Zone::Sidebar.next(p, false, o), Zone::List);
    assert_eq!(Zone::List.next(p, false, o), Zone::List);
    // 抽屉拉开后侧栏回到环里，`Tab` 从列表绕回它。
    assert_eq!(Zone::List.next(p, true, o), Zone::Sidebar);
    assert_eq!(Zone::List.next(p, false, true), Zone::Preview);
    assert_eq!(Zone::Preview.prev(p, false, true), Zone::List);
}

// ---------------------------------------------------------------------------
// 命中高亮（§7.6）
// ---------------------------------------------------------------------------

#[test]
fn match_ranges_is_case_insensitive_and_char_indexed() {
    let text = "Rust 与 rust 都是 rust";
    let r = match_ranges(text, "rust");
    assert_eq!(r.len(), 3);
    // 返回的是**字符**下标而不是字节下标：`hit_segs` 按 `chars()` 切片，
    // 混用字节下标会在中文行上切到半个字（那就是 panic，不是错位）。
    let chars: Vec<char> = text.chars().collect();
    for (a, b) in &r {
        assert_eq!(
            chars[*a..*b].iter().collect::<String>().to_lowercase(),
            "rust"
        );
    }
}

#[test]
fn match_ranges_handles_cjk_without_splitting_chars() {
    let text = "思维导图与概念图";
    let r = match_ranges(text, "概念");
    assert_eq!(r.len(), 1);
    let chars: Vec<char> = text.chars().collect();
    assert_eq!(chars[r[0].0..r[0].1].iter().collect::<String>(), "概念");
    // 「概念」是第 5、6 个字符；按字节算是 15..21——这两个数不许相等，
    // 相等就说明口径退回字节了。
    assert_eq!(r[0], (5, 7));
}

#[test]
fn match_ranges_empty_cases() {
    assert!(match_ranges("anything", "").is_empty());
    assert!(match_ranges("anything", "   ").is_empty());
    assert!(match_ranges("", "x").is_empty());
    assert!(match_ranges("ab", "abc").is_empty());
    assert!(match_ranges("abc", "zzz").is_empty());
}

// ---------------------------------------------------------------------------
// 来源短语（`docs/30` §5.6）
// ---------------------------------------------------------------------------

#[test]
fn source_phrase_is_doc_dot_node() {
    let s = CardSource {
        doc_label: "读书笔记".into(),
        node_id: "n1".into(),
        node_text: "第三章".into(),
        concept_map: false,
    };
    assert_eq!(s.phrase().as_deref(), Some("读书笔记 · 第三章"));
}

#[test]
fn source_phrase_gives_up_on_an_unknown_doc() {
    // 跨文档的卡片查不到文档名：编一个假名字比说「另一份文档」更糟。
    let s = CardSource {
        doc_label: "   ".into(),
        node_id: String::new(),
        node_text: "第三章".into(),
        concept_map: true,
    };
    assert_eq!(s.phrase(), None);
}

// ---------------------------------------------------------------------------
// 一份数据口径（§7.11）
// ---------------------------------------------------------------------------

fn card(id: &str, tags: &[&str], archived: bool, node: Option<&str>) -> CardMeta {
    CardMeta {
        id: id.into(),
        title: format!("卡片 {id}"),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        source_doc: "demo".into(),
        source_node: node.map(|n| n.to_string()),
        created_at: 1_000,
        updated_at: 2_000,
        excerpt: String::new(),
        file: format!("{id}.md"),
        archived,
    }
}

fn lib() -> NoteLibrary {
    NoteLibrary::new(
        vec![
            card("a", &[], false, None),
            card("b", &["知识"], false, Some("n1")),
            card("c", &["知识", "读书"], false, None),
            card("d", &[], true, None),
        ],
        SIDEBAR_W,
        LIST_W,
    )
}

#[test]
fn sidebar_counts_are_one_source_of_truth() {
    let l = lib();
    // 四个数都从 `scope_counts` 出来，页头与侧栏读的是同一份。
    assert_eq!(l.count_for(CardScope::All), 3);
    assert_eq!(l.count_for(CardScope::Inbox), 1);
    assert_eq!(l.count_for(CardScope::Untagged), 1);
    assert_eq!(l.count_for(CardScope::Archived), 1);
    // 「全部卡片」不含归档：3 而不是 4。
    assert_eq!(l.scope_count(), 3);
    assert_eq!(l.scope_count(), l.count_for(CardScope::All));
}

#[test]
fn list_rows_match_the_sidebar_count_when_unfiltered() {
    // 旧版的缝就在这儿：侧栏自己数一遍、列表自己滤一遍，两边能不一致。
    let mut l = lib();
    for scope in CardScope::ROWS {
        l.set_scope(scope);
        assert_eq!(
            l.rows.len(),
            l.scope_count(),
            "{scope:?}：行数 {} ≠ 计数 {}",
            l.rows.len(),
            l.scope_count()
        );
    }
}

#[test]
fn archived_scope_shows_only_archived() {
    let mut l = lib();
    l.set_scope(CardScope::Archived);
    assert_eq!(l.rows.len(), 1);
    assert_eq!(l.row_id(0).as_deref(), Some("d"));
}

#[test]
fn scope_switch_keeps_filters_visible() {
    // §7.4：换范围时筛选**保留**——chip 一直可见可删，悄悄清掉更让人找不到北。
    let mut l = lib();
    l.set_tag_filter("知识".into(), false);
    assert_eq!(l.rows.len(), 2);
    l.set_scope(CardScope::Archived);
    assert!(l.has_filters());
    assert!(l.rows.is_empty());
    l.clear_filters();
    assert!(!l.has_filters());
    assert_eq!(l.rows.len(), 1);
}

#[test]
fn tag_click_is_single_select_and_ctrl_click_is_and() {
    // §7.6：单击 = 换掉整组，`Ctrl+click` = 往 AND 里加一项。
    let mut l = lib();
    l.set_tag_filter("知识".into(), false);
    l.set_tag_filter("读书".into(), false);
    assert_eq!(l.tag_filter, vec!["读书".to_string()]);
    assert_eq!(l.rows.len(), 1);

    l.set_tag_filter("知识".into(), true);
    assert_eq!(l.tag_filter.len(), 2);
    // AND：没有卡片同时有「知识」和「读书」之外的组合，只有 c 两个都有。
    assert_eq!(l.rows.len(), 1);
    assert_eq!(l.row_id(0).as_deref(), Some("c"));

    // 再 `Ctrl+click` 同一个标签是取消它。
    l.set_tag_filter("读书".into(), true);
    assert_eq!(l.tag_filter, vec!["知识".to_string()]);
}

#[test]
fn clicking_the_only_tag_again_clears_it() {
    let mut l = lib();
    l.set_tag_filter("知识".into(), false);
    assert_eq!(l.rows.len(), 2);
    l.set_tag_filter("知识".into(), false);
    assert!(l.tag_filter.is_empty());
    assert_eq!(l.rows.len(), 3);
}

#[test]
fn tag_tree_is_at_most_two_levels() {
    // §7.1：标签树最多两级，再深的路径不进侧栏。
    let mut l = lib();
    l.sync(vec![
        card("a", &["一级/二级/三级"], false, None),
        card("b", &["一级/二级"], false, None),
        card("c", &["散"], false, None),
    ]);
    let depth = |path: &str| path.split('/').count();
    assert!(
        l.tree.iter().all(|row| depth(&row.path) <= 2),
        "标签树出现了三级：{:?}",
        l.tree.iter().map(|r| r.path.clone()).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// 排序（§7.6）
// ---------------------------------------------------------------------------

#[test]
fn sort_default_is_updated_desc() {
    let mut l = NoteLibrary::new(
        vec![
            CardMeta {
                title: "Zebra".into(),
                updated_at: 300,
                created_at: 100,
                ..card("a", &[], false, None)
            },
            CardMeta {
                title: "Apple".into(),
                updated_at: 100,
                created_at: 300,
                ..card("b", &[], false, None)
            },
        ],
        SIDEBAR_W,
        LIST_W,
    );
    assert_eq!(l.sort, CardSort::UpdatedDesc);
    assert_eq!(l.row_id(0).as_deref(), Some("a"));
    l.sort = CardSort::CreatedDesc;
    l.recompute();
    assert_eq!(l.row_id(0).as_deref(), Some("b"));
    l.sort = CardSort::Title;
    l.recompute();
    assert_eq!(l.row_id(0).as_deref(), Some("b"));
}

#[test]
fn searching_puts_relevance_first() {
    // §7.6：搜索时相关度优先，同分再落回用户选的排序。
    let mut l = lib();
    l.sort = CardSort::Title;
    l.hits = Some(
        [("b".to_string(), 1), ("a".to_string(), 9)]
            .into_iter()
            .collect(),
    );
    l.recompute();
    let ids: Vec<String> = l.rows.iter().map(|i| l.cards[*i].id.clone()).collect();
    assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
    // 不在命中表里的一律不出现。
    assert!(!ids.iter().any(|i| i == "c" || i == "d"));
}

// ---------------------------------------------------------------------------
// 选择语义（§7.4：只有两种）
// ---------------------------------------------------------------------------

#[test]
fn plain_click_moves_focus_without_selecting() {
    let mut l = lib();
    l.click_row(1, Modifiers::NONE);
    assert!(!l.select_mode);
    assert!(l.selected.is_empty());
    assert_eq!(l.focused.as_deref(), Some("b"));
    assert_eq!(l.cursor, 1);
}

#[test]
fn ctrl_click_enters_select_mode_and_toggles() {
    let mut l = lib();
    l.click_row(0, Modifiers::CTRL);
    assert!(l.select_mode);
    assert_eq!(l.selected_ids(), vec!["a".to_string()]);
    l.click_row(1, Modifiers::CTRL);
    assert_eq!(l.selected_ids(), vec!["a".to_string(), "b".to_string()]);
    // 再点一次是取消勾选，不是退出选择模式。
    l.click_row(0, Modifiers::NONE);
    assert!(l.select_mode);
    assert_eq!(l.selected_ids(), vec!["b".to_string()]);
}

#[test]
fn shift_click_selects_the_range_from_the_anchor() {
    let mut l = lib();
    l.click_row(0, Modifiers::NONE);
    l.click_row(2, Modifiers::SHIFT);
    assert!(l.select_mode);
    assert_eq!(
        l.selected_ids(),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
    // 反向连选同样成立。
    let mut l = lib();
    l.click_row(2, Modifiers::NONE);
    l.click_row(0, Modifiers::SHIFT);
    assert_eq!(l.selected.len(), 3);
}

#[test]
fn select_all_only_takes_the_filtered_rows() {
    // §7.8：`Ctrl+A` 全选**当前筛选结果**，不是全库。
    let mut l = lib();
    l.set_tag_filter("知识".into(), false);
    l.select_all_rows();
    assert_eq!(l.selected_ids(), vec!["b".to_string(), "c".to_string()]);
}

#[test]
fn exit_select_clears_both_kinds_of_state() {
    let mut l = lib();
    l.select_all_rows();
    assert!(!l.selected.is_empty());
    l.exit_select();
    assert!(!l.select_mode);
    assert!(l.selected.is_empty());
    assert!(l.anchor.is_none());
}

#[test]
fn export_falls_back_to_the_whole_filter() {
    let mut l = lib();
    l.set_tag_filter("知识".into(), false);
    assert_eq!(l.export_ids(), vec!["b".to_string(), "c".to_string()]);
    l.click_row(0, Modifiers::CTRL);
    assert_eq!(l.export_ids(), vec!["b".to_string()]);
}

// ---------------------------------------------------------------------------
// 重排与游标（§7.11：选择按 id，不按行下标）
// ---------------------------------------------------------------------------

#[test]
fn sync_drops_selection_for_cards_that_are_gone() {
    let mut l = lib();
    l.select_all_rows();
    assert_eq!(l.selected.len(), 3);
    l.sync(vec![card("a", &[], false, None)]);
    assert_eq!(l.selected_ids(), vec!["a".to_string()]);
    // 选择集空了就自动退出选择模式，否则批量条会挂在 0 张上。
    let mut l = lib();
    l.click_row(0, Modifiers::CTRL);
    l.sync(vec![card("z", &[], false, None)]);
    assert!(!l.select_mode);
}

#[test]
fn a_plain_click_opens_the_reading_page_only_outside_three_pane() {
    // §7.2：两栏/单栏「选中卡片后预览以整页阅读面打开」；三栏有常驻预览列。
    let mut l = lib();
    l.row_clicked(1, Modifiers::NONE);
    l.resolve_preview_page(Pane::Three);
    assert!(!l.preview_page, "三栏不该盖整页：有常驻预览列");
    l.resolve_preview_page(Pane::Two);
    assert!(l.preview_page, "两栏点行必须开整页阅读面");
    l.resolve_preview_page(Pane::One);
    assert!(l.preview_page);
}

#[test]
fn ticking_a_checkbox_does_not_open_the_reading_page() {
    let mut l = lib();
    l.row_clicked(1, Modifiers::CTRL);
    assert!(l.select_mode);
    assert!(!l.preview_wanted, "勾选语义不该顺手开阅读面");
    // 选择模式里的普通单击是切换勾选，同样不开。
    l.row_clicked(2, Modifiers::NONE);
    assert!(!l.preview_wanted);
}

#[test]
fn sync_keeps_focus_by_id_across_a_reorder() {
    let mut l = lib();
    l.click_row(2, Modifiers::NONE);
    assert_eq!(l.focused.as_deref(), Some("c"));
    assert_eq!(l.cursor, 2);
    // 让 c 变成最近更新的那张：行序翻过来，焦点必须跟着 id 走，游标跟着焦点走。
    // `lib()` 里三张卡时间戳相同（排序退回按 id），所以这里要显式给时间。
    let mut c = card("c", &["知识", "读书"], false, None);
    c.updated_at = 5_000;
    let mut b = card("b", &["知识"], false, Some("n1"));
    b.updated_at = 4_000;
    let mut a = card("a", &[], false, None);
    a.updated_at = 3_000;
    l.sync(vec![a, b, c]);
    assert_eq!(l.focused.as_deref(), Some("c"));
    assert_eq!(l.rows.len(), 3);
    assert_eq!(l.cursor, 0, "c 挪到第 0 行，游标不该留在 2");
}

#[test]
fn sync_forgets_the_cached_body() {
    // 正文可能在库外被改过：`sync` 必须把预览缓存丢掉，下一帧按 id 重读。
    let mut l = lib();
    l.preview = Some(Preview {
        id: "a".into(),
        title: "旧标题".into(),
        body: "旧正文".into(),
        tags: Vec::new(),
        source_node: None,
        created_at: 0,
        updated_at: 0,
    });
    l.sync(vec![card("a", &[], false, None)]);
    assert!(l.preview.is_none());
}

#[test]
fn cursor_is_clamped_when_rows_shrink() {
    let mut l = lib();
    l.move_cursor(10);
    assert_eq!(l.cursor, l.rows.len() - 1);
    l.set_tag_filter("读书".into(), false);
    assert_eq!(l.rows.len(), 1);
    assert_eq!(l.cursor, 0);
}

#[test]
fn move_cursor_stops_at_both_ends() {
    let mut l = lib();
    l.move_cursor(-5);
    assert_eq!(l.cursor, 0);
    l.move_cursor(1);
    assert_eq!(l.cursor, 1);
    l.move_cursor(99);
    assert_eq!(l.cursor, l.rows.len() - 1);
}

#[test]
fn empty_library_has_no_cursor_and_no_focus() {
    let l = NoteLibrary::new(Vec::new(), SIDEBAR_W, LIST_W);
    assert!(l.rows.is_empty());
    assert_eq!(l.cursor, 0);
    assert!(l.focused.is_none());
    assert!(l.focused_card().is_none());
    assert_eq!(l.scope_count(), 0);
    assert!(l.export_ids().is_empty());
}

// ---------------------------------------------------------------------------
// 双击（§7.4：双击或 Enter 打开）
// ---------------------------------------------------------------------------

#[test]
fn double_click_needs_two_hits_on_the_same_id() {
    let mut l = lib();
    l.clock = 10.0;
    assert!(!l.is_double_click("a"));
    l.clock = 10.1;
    assert!(l.is_double_click("a"));
    // 换一张卡片不算双击。
    l.clock = 10.2;
    assert!(!l.is_double_click("b"));
    assert!(!l.is_double_click("a"));
}

#[test]
fn double_click_window_is_400ms() {
    let mut l = lib();
    l.clock = 10.0;
    assert!(!l.is_double_click("a"));
    l.clock = 10.0 + DOUBLE_CLICK_SECS + 0.01;
    assert!(!l.is_double_click("a"));
}

// ---------------------------------------------------------------------------
// 折叠（§7.3：默认全展开）
// ---------------------------------------------------------------------------

#[test]
fn tag_tree_starts_fully_expanded() {
    let mut l = lib();
    l.sync(vec![
        card("a", &["知识/读书"], false, None),
        card("b", &["知识/读书"], false, None),
    ]);
    let paths: Vec<String> = l.tree.iter().map(|r| r.path.clone()).collect();
    assert!(paths.contains(&"知识".to_string()));
    assert!(
        paths.iter().any(|p| p.contains("读书")),
        "默认该展开到二级：{paths:?}"
    );
    l.toggle_collapse("知识");
    let after: Vec<String> = l.tree.iter().map(|r| r.path.clone()).collect();
    assert!(!after.iter().any(|p| p.contains("读书")));
    l.toggle_collapse("知识");
    let back: Vec<String> = l.tree.iter().map(|r| r.path.clone()).collect();
    assert!(back.iter().any(|p| p.contains("读书")));
}
