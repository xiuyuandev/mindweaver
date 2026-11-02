use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    /// 默认落在开始页；打开文件或跑快照 harness 时进文档；钩子可以两头强制。
    #[test]
    fn startup_lands_on_home_unless_a_document_is_waiting() {
        assert!(start_at_home(None, false, false));
        assert!(
            !start_at_home(None, false, true),
            "MW_OPEN 打开的文档要直接显示"
        );
        assert!(!start_at_home(None, true, false), "harness 不能改基线");
        assert!(
            start_at_home(Some(true), true, true),
            "MW_HOME=1 强制开始页"
        );
        assert!(
            !start_at_home(Some(false), false, false),
            "MW_HOME=0 强制文档"
        );
    }

    /// 标题栏状态：从未落盘的文档不能显示“已保存”；只有脏/未落盘才可点保存。
    #[test]
    fn save_state_distinguishes_untitled_saved_and_dirty() {
        assert_eq!(SaveState::of(true, false, false, false), SaveState::Home);
        assert_eq!(SaveState::of(false, true, true, true), SaveState::Dirty);
        assert_eq!(SaveState::of(false, true, false, true), SaveState::Saved);
        assert_eq!(
            SaveState::of(false, true, false, false),
            SaveState::Untitled
        );
        assert_eq!(
            SaveState::of(false, false, false, false),
            SaveState::Untitled
        );
        assert!(SaveState::Dirty.can_save());
        assert!(SaveState::Untitled.can_save());
        assert!(!SaveState::Saved.can_save());
        assert!(!SaveState::Home.can_save());
    }

    /// 窄窗先丢标题，状态芯片再多撑一档；两者空间都不够才整组留白。
    #[test]
    fn title_cluster_drops_title_before_save_chip() {
        let both = title_cluster(400.0, 100.0);
        assert!(both.chip && both.title);
        let chip_only = title_cluster(200.0, 100.0);
        assert!(chip_only.chip && !chip_only.title);
        let none = title_cluster(100.0, 100.0);
        assert!(!none.chip && !none.title);
    }

    /// 单选显示主题名（截断），多选只给数量；未选中时状态栏不出现摘要。
    #[test]
    fn selection_summary_names_one_and_counts_many() {
        assert_eq!(selection_summary(0, None), None);
        assert_eq!(
            selection_summary(1, Some("  市场调研  ")).as_deref(),
            Some("已选：市场调研")
        );
        assert_eq!(
            selection_summary(1, Some("   ")).as_deref(),
            Some("已选：未命名主题")
        );
        let long = "一二三四五六七八九十一二三四五六七八九十";
        let short = selection_summary(1, Some(long)).expect("单选有摘要");
        assert_eq!(short, "已选：一二三四五六七八九十一二三四五六七…");
        assert_eq!(
            selection_summary(3, Some("任意")).as_deref(),
            Some("已选 3 个主题")
        );
    }

    /// `Ctrl+Tab` 一路向右绕回开头；`Ctrl+Shift+Tab` 从 0 向左落到末页。
    #[test]
    fn cycling_wraps_in_both_directions() {
        assert_eq!(next_tab(0, 1, 3), 1);
        assert_eq!(next_tab(2, 1, 3), 0, "最后一页向右回到第一页");
        assert_eq!(next_tab(0, -1, 3), 2, "第一页向左落到最后一页");
        assert_eq!(next_tab(1, -1, 3), 0);
    }

    /// 只有一页时循环没有去处；空窗口按 0 兜底而不是 panic。
    #[test]
    fn cycling_a_single_or_empty_tab_stays_put() {
        assert_eq!(next_tab(0, 1, 1), 0);
        assert_eq!(next_tab(0, -1, 1), 0);
        assert_eq!(next_tab(0, 1, 0), 0);
    }

    /// 关掉活动位左边的页，活动位左移一格；关右边的，活动位不动。
    #[test]
    fn closing_a_non_active_tab_keeps_the_right_page_active() {
        // [A B C]，活动在 C(2)，关掉 A(0) → 剩 [B C]，活动仍指 C，新下标 1。
        assert_eq!(active_after_close(0, 2, 2), 1);
        // 关掉 B(1)（在 C 左边）→ 剩 [A C]，活动指 C，新下标 1。
        assert_eq!(active_after_close(1, 2, 2), 1);
        // [A B C]，活动在 A(0)，关掉 C(2)（在右边）→ 活动不动，还是 A(0)。
        assert_eq!(active_after_close(2, 0, 2), 0);
    }

    /// 关掉的就是活动页：顺位到同下标；已在末页则退一格。
    #[test]
    fn closing_the_active_tab_lands_on_the_neighbor() {
        // [A B C]，活动在 B(1)，关掉 B → 剩 [A C]，顺位到 C(1)。
        assert_eq!(active_after_close(1, 1, 2), 1);
        // [A B C]，活动在末页 C(2)，关掉 C → 剩 [A B]，退回 B(1)。
        assert_eq!(active_after_close(2, 2, 2), 1);
        // 只剩一页时活动位被夹到 0。
        assert_eq!(active_after_close(0, 0, 1), 0);
    }

    /// 标签名截断必须按字符走：中文不能被切出半个字。
    #[test]
    fn tab_titles_truncate_on_char_boundaries() {
        assert_eq!(shorten("短", 22), "短");
        let twenty_three_cjk: String = "字".repeat(23);
        let short = shorten(&twenty_three_cjk, 22);
        assert_eq!(short.chars().count(), 22, "含省略号在内不超过 22 个字符");
        assert!(short.ends_with('…'));
        assert!(short.chars().all(|c| c == '字' || c == '…'));
    }

    /// 新标签槽的初值：相机在原点、缩放 1:1、没有选中。
    #[test]
    fn a_new_tab_starts_at_the_origin_with_no_selection() {
        let t = Tab::new(Session::blank());
        assert!(t.sel_ids.is_empty());
        assert_eq!(t.camera.center.x, 0.0);
        assert_eq!(t.camera.center.y, 0.0);
        assert_eq!(t.camera.zoom, 1.0);
    }

    /// 卡片演示的日期必须固定：`card-slash` 金样里印着这一天，
    /// 取墙钟会在 UTC 跨日（本地 08:00）后让基线每天漂一次。
    #[test]
    fn card_demo_timestamp_is_frozen_for_golden() {
        assert_eq!(
            crate::note_view::format_date(crate::app_methods_1::DEMO_CARD_TS_MS),
            "2026-10-24"
        );
    }

    /// 标题栏菜单/快捷键表在前台时，开始页不再收裸键：Esc 归菜单，
    /// Enter/1–5 不能在菜单背后新建文档。
    #[test]
    fn start_page_bare_keys_yield_to_foreground_layers() {
        use crate::app_methods_1::home_bare_keys_owned;
        assert!(home_bare_keys_owned(false, false));
        assert!(
            !home_bare_keys_owned(true, false),
            "菜单开着时 Esc 应先关菜单"
        );
        assert!(
            !home_bare_keys_owned(false, true),
            "快捷键表开着时 Enter 应归它"
        );
        assert!(!home_bare_keys_owned(true, true));
    }
}
