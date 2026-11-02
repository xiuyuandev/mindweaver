# 打磨账本 · 思维导图 / 概念图的编辑体验

> 建立：2026-10-29。用途：**计划内工作全部收口后**，按这本账一轮一轮往下打磨。
> 每轮只挑一行「最弱」的做，做完把证据与数字回填到本行 + `docs/STATUS.md` + 执行日志。
> 规则：不许「换肤」冒充打磨；不许把「已实现」写成「已验证」；每轮都要有测试或金样。

## 0. 怎么用这本账

1. 从上往下找第一行 `状态` 不是 `稳` 的；`粗糙` 优先于 `未验证`，`未验证` 优先于 `稳`。
2. 用真实应用（脚本 + trace，必要时金样）复现那一行的「已知粗糙」。
3. 改到位后：补/改单测 → 需要时补金样（`--update` + 双跑 `--tol 0`）→ 回填本行与 STATUS。
4. 一轮只动一行；跨行的改动要先记账，不许顺手夹带。

## 1. 思维导图（画布）

| # | 操作 | 单测证据 | 视觉 / 端到端证据 | 状态 | 已知粗糙 |
| --- | --- | --- | --- | --- | --- |
| M1 | `Tab` 建子节点并直接进编辑 | `commit_follow_up` 新单测 + `add_child` 等 | `mm-new-child`、`mm-new-chain`（`--tol 0` 双跑，见 `shots/x100/M1/`）；轨迹 `editor_open` | 稳（`Tab` 建树链） | 实机连按节奏（抢焦点那一帧、系统键重复）没量；只拍 ppp 1.0 |
| M2 | `Enter` 建同级（根上建一级） | `commit_follow_up` + `add_child` | `mm-enter-sibling`；`cm-node-edit` 未漂移（概念图 Enter 仍只提交） | 稳 | 同 M1 |
| M3 | 编辑态：`Shift+Enter` 换行、超宽折行、CJK 禁则 | `the_editor_locks_its_line_box_to_the_canvas_pitch` | `canvas-text-ppp1.00.png` | 稳 | — |
| M4 | IME 组合态：Enter/Esc 归输入法 | `ime_transition_follows_the_latest_ime_event`、`ime_composition_owns_enter_and_escape` | 无实拍 | 未验证 | 实机微软拼音（全员共用缺口） |
| M5 | 拖拽重排：三区落点、禁止拖进自己子树、Ctrl 复制 | `left_drag_on_a_node_picks_it_up`、`dropping_on_blank_*`、`ctrl_drag_*`；轨迹 `drop_zone` / `scene_node` / `canvas_click hit=` | `mm-drag-{child,before,after,illegal}` 四张基线（`--tol 0` 双跑，见 `shots/x100/M5/`） | 稳（四形态） | `Ctrl` 拖拽复制、150/200% 下的条宽没拍；实机拖拽手感（4px 阈值、60ms 吸附）无人量过。**150/200% 不是加一档注册就行**：金样窗口物理 1620×1080 固定，`--ppp 1.5` 把逻辑视口压成 1080×646、`fit` 的 zoom 从 1.3312 变 0.8545，是另一张布局，旧坐标全落在空白——见 §3 第 15 项（2026-11-01 第 67 轮实测） |
| M6 | 键盘移动：`Alt+↑↓`、`Ctrl+Shift+←/→` | `move_down_compensates_the_removal_shift`、`promote_sits_after…`、`single_selection_moves_the_same_way_through_both_paths` | `mm-move-keys`、`mm-move-multi`、`mm-move-keys-reveal`（`--tol 0` 双跑，见 `shots/x100/M6/`）+ `structure` / `reveal` / `marquee` 轨迹 | 稳 | 实机连按节拍（系统键重复率）未量；跨父级多选按 `Alt+↓` 什么都不动且不给反馈（`batch_structure_command` 的既有语义）——**2026-11-01 打磨轮 60 证伪并补反馈**：跨父级批量移动本来就按各自的父级逐颗换位（`batch_move_cross_parent_moves_each_within_its_own_parent`），真正缺的是边界上的提示，现由 `move_block_reason` + `notice` 补齐（`mm-move-blocked`，轨迹 `structure_move_blocked reason=First|Last|Root|batch`，见 `shots/x100/M6b/`） |
| M7 | 框选 / Ctrl+点击 / 批量操作 | `left_drag_on_empty_canvas_starts_a_marquee`、`marquee_pick_maps_the_screen_box_through_the_camera`、`mods_sets_and_clears_the_modifier_state`、`marquee_merge_honours_shift_and_alt` | `mm-marquee`（橡皮筋 + 实时选中环，未松手定格）、`mm-marquee-add`（Ctrl 加选 1 → 3 → 5）+ `marquee preview` 轨迹 + 2px/5px 阈值对照（见 `shots/x100/M7/`） | 稳 | 4px 阈值是规格值，「5px 抖动就清掉多选」的手感没有实机数据；拖动中每帧一次 `intersecting`，20k 文档下的帧率没实测；Alt 减选与套索没拍（**2026-11-01 打磨轮 62 / M7b 已拍**：`mm-lasso` 多边形定格 + `mm-marquee-alt` 3→2 减选，轨迹 `path=lasso`；核对出的真实语义是**套索恒为减选、不能建立新选择**，见 `shots/x100/M7b/`） |
| M8 | 撤销/重做粒度（每次提交一条） | `split_and_merge_are_single_batches…` + `coalesced_create_and_text_is_one_undo` / `coalescing_is_opt_in_per_call` | `mm-undo-built/one/three`、`mm-redo-one` + `undo`/`redo` 轨迹 | 稳 | 实机 IME 组合期未核；只测 `Ctrl+Y` |
| M9 | 折叠/展开与折叠数徽标 | `toggle_disc`、`toggle_hit_ring_keeps_its_screen_size_across_zoom`、`blend_collapse_slides_children_into_parent_and_edges_follow` | `mm-collapse`、`mm-collapse-mid`（定格 t=0.5）、`mm-toggle-edge`、`mm-toggle-miss`（`--tol 0` 双跑，见 `shots/x100/M9/`）+ `toggle` / `canvas_click` 轨迹 | 稳 | 实机手感（触控笔、150%/200% DPI）没量；热区 10 CSS px 是原值，没有手感数据说明该大该小；整图折叠与聚焦模式下的圆点没拍。**2026-10-31 打磨轮 37 复标**：这四张的点击坐标自第 19 轮起失效（点在空白处），基线锁的是「没折」；已按落定相机改为 (522,429)/(522,438)/(522,441) 并重拍，见 `shots/x100/M17/` |
| M9b | 整图折叠与聚焦视图**下**的圆点集合（纯证据） | 无新增（本轮不改语义）；判据由两张基线的 `scene_toggle` 轨迹对照钉住 | `mm-collapse-all-dots`（`Ctrl+/` 终态，sha256-12 `ab78308c91d1`，脚本 43 帧）与 `mm-focus-dots`（`MW_DRILL_DEMO=纯 Rust 原生` + `MW_FOCUS_DEMO=纯 Rust 原生`，sha256-12 `053b98ae6977`，脚本 19 帧），各 `--tol 0` 两遍；折叠后场景 5 节点 / 圆点恰 **3** 条（纯 Rust 原生、中文排版、交互与性能），聚焦视图 4 节点 / 圆点恰 **1** 条（`winit + windows-rs`，屏幕 (536,551)），见 `shots/x100/M9b/` | 稳（机器；实机手感仍 `未验证`） | 2026-11-01 第 67 轮：M9 那四张拍的都是默认展开态的**单颗**圆点，整体折叠与聚焦裁剪两种状态下「该画的圆点在不在、不该在的地方有没有残点」从没核对过。查出**一处证据的 bug**：`canvas_methods_2.rs` 的 `scene_toggle` 轨迹只按 `has_children` 判，把**根主题**也列进圆点清单，而绘制与命中（`mw-render/src/hit.rs`）要求「有父级且有下级」——轨迹会诱导下一轮去点一颗屏幕上不存在、点也点不到的圆点（与第 37 轮那三张点在空白处的坐标同类）。已把 dump 判据对齐 `HitMap`（`if crate::trace::is_enabled()` 块内，不设 `MW_TRACE` 零路径，**不改像素**）。聚焦场景第一版无效：只 `MW_FOCUS_DEMO=中文排版` 时三颗子级全是叶子，画面一颗圆点都没有，证不了「边界圆点还在」——改借 `MW_DRILL_DEMO` 挂一颗孙子才有可拍的边界点。**未做**：整图折叠后再单击圆点的展开路径没拍；聚焦视图内 `Ctrl+/` 与聚焦的相互作用画布侧无画面证据（大纲侧有 `collapse_level_respects_the_focus_root`）；实机手感（触控笔、150%/200% DPI、真键盘）缺口原样保留 |
| M17 | 折叠圆点 / 概要 / 边界的悬停反馈 | `every_hit_target_maps_to_its_own_hover_feedback`、`toggle_grows_from_exact_size_to_one_point_three`、`toggle_radius_keeps_the_world_floor` | `mm-toggle-hover`、`mm-summary-hover`、`mm-boundary-hover`（`--tol 0` 双跑，见 `shots/x100/M17/`）+ `hover_detail` / `scene_summary` / `scene_boundary` / `scene_toggle` 轨迹 | 稳（机器；实机手感未核） | 2026-10-31 第 37 轮：悬停是交互态，全部画在 egui 覆盖层，`Frame` 未动、导出无污染；圆点按 1.3× 重绘（`docs/06` §5）、概要与边界加亮 55% 强调色。实机淡入手感、150%/200% DPI 未拍；概要悬停不区分花括号与主题盒 |
| M18 | `Ctrl+X` 带子树删除是否先确认 | 既有 `delete_confirm_counts_the_whole_subtree_and_skips_leaf_only_deletes`（叶子 → 不确认、子树 → 计 3） | `mm-cut-confirm`、`mm-cut-cancel`（`--tol 0` 双跑，见 `shots/x100/M18/`）+ `delete-confirm pending=3 selected=1` / `cancel count=3 reason=esc` / `dialog` 轨迹 | 稳（机器；实机键盘未核） | 2026-10-31 第 38 轮：HANDOFF §7.5③ 是旧结论——`cut_action` / `apply_clipboard(Cut)` 本来就都走 `delete_selected()` 的确认分支；产品行为零改动，只补三条轨迹与两场金样。取消后与只选中态 **0 px**。多选路径同函数覆盖但未单独拍图；「删除」按钮接受路径未拍 |
| M19 | 全局搜索：标题 / 备注 / 链接 + 摘录定位 | `note_field_hit_maps_range_to_note`、`link_field_hit`、`note_field_pinyin_and_initials`、`title_wins_over_note_on_same_node`、`title_hit_ranks_above_note_hit`、`long_note_text_still_matches_beyond_pinyin_cap`、`note_cache_invalidates_with_field_text`、`excerpt_at_start_has_no_leading_ellipsis`、`excerpt_middle_folds_range_and_adds_ellipses`、`excerpt_flattens_newlines_and_keeps_range_on_hit`、`blank_note_never_hits`（`mw-core`，11 条 / 覆盖 12 条验收） | `search-note` / `search-link`（`--tol 0` 双跑，见 `shots/x100/M19/`）+ `search_hits q=html rows=1 note=1 link=0` / `q=https rows=1 note=0 link=1` + 改前「没有找到匹配的主题」对照（差异只在搜索卡片 `x515..1106 y80..221`） | 稳（机器；实机 IME/键盘未核） | 2026-10-31 第 41 轮：`docs/02` §4 的 P0 口径「标题+备注+链接」此前只做了标题——`SearchIndex::hits` 只读 `node.text`，备注里的 `html` 搜不到。现在三字段都支持原文/全拼/首字母（标题 1000/700/500、备注 650/520/400、链接 600/480/380，同节点取最高一条）；备注/链接命中显示 ±24 字摘录 + 「备注」/「链接」角标，第二行「节点 · 节点标题 · 祖先路径」；超过 240 字的字段只做原文（`FIELD_PINYIN_CAP`）。**未做**：`Ctrl+H` 替换仍只改标题、关系线短语/标签/卡片正文不归节点检索、摘录固定单行、20k 长备注的首查延迟未实测 |
| M20 | 搜索命中折叠子树后的跳转（展开祖先 + 选中） | `collapsed_ancestors_are_listed_near_to_far`、`collapsed_ancestors_skip_expanded_and_self`、`collapsed_ancestors_is_empty_for_missing_node`（`mw-core`，3 条）+ `reveal_commands_open_collapsed_ancestors_root_to_near_in_one_batch`、`reveal_commands_skip_visible_and_missing_nodes`、`layout_subtree_membership_ignores_collapse_but_not_focus`（`mw-shell`，3 条） | `search-collapsed`（`--tol 0` 双跑，见 `shots/x100/M20/`）+ 改前 `search_hits … rows=1` 之后没有跳转事件、改后 `search_reveal … expanded=1` / `search_jump … ok=1` 轨迹；改前→改后差 **252 082 px**（搜索卡片区域 **0 px**、标题栏 132 px 是撤销按钮亮起） | 稳（机器；实机 IME/键盘未核） | 2026-10-31 第 43 轮：检索遍历文档前序，折叠子树里的节点照样命中，但画布只排可见节点，`select_node_keep_focus` 在 `scene.find` 失败时直接 return（结果行在、画布不动、无 toast/轨迹）。现在先按根 → 近展开挡路祖先（一条 `Command::Batch`，一次撤销全收回）再选中 + 相机居中 + glow；聚焦模式仍不可达时只 toast「这个主题在当前视图之外」，**不自动退出聚焦**；跳转展开会入撤销栈 |
| M21 | `Ctrl+H` 替换：逐个/全部 + 替换预览 | `replace_char_range_replaces_a_cjk_slice`、`replace_char_range_rejects_invalid_ranges`（`mw-core`，2 条）+ `replace_target_only_accepts_title_literal_hits`、`replace_current_emits_one_command_and_parks_the_next_row`、`settle_after_refresh_moves_past_the_replaced_row_and_wraps`、`replace_current_starts_at_the_first_row_and_clears_on_query_change`、`replace_current_refuses_pinyin_and_note_rows`（`mw-shell`，5 条） | `search-replace` / `search-replace-one`（`--tol 0` 双跑，见 `shots/x100/M21/`）+ `search_replace_one … chars=4 before_chars=4` / `toast 已替换 1 处` 轨迹（无 `focus canvas`）；改前→预览差 **3964 px**（只在搜索卡片），两新基线互差 **78 247 px**（结果清空 + 标题 + 相机 + toast） | 稳（机器；实机 IME/键盘未核） | 2026-10-31 第 44 轮：`docs/02` §4 只要求「标题、逐个/全部、预览」，改前只有全部替换、无预览、`Ctrl+H` 焦点还在搜索框、备注/链接/拼音命中静默跳过。现在 `Ctrl+Enter` 替换当前、`Ctrl+Shift+Enter` 全部，展开替换行即聚焦替换框；当前行显示 `→ 新标题`，被替换行仍命中的话前进一行、到底绕回，不再命中则按原下标夹取；只接受节点标题的字面命中，其它行不给替换入口；替换走一条 `SetText` 与一次撤销，选中但**不抢键盘**。顺带修 `card-autosave` 金样按 UTC 跨日腐烂（`MW_CARD_NOW_MS` 定钟） |
| M22 | 聚焦视图外的搜索跳转（先退出全部聚焦） | `focus_exit_levels_only_fires_for_targets_outside_the_focused_subtree`（`mw-shell`，1 条） | `search-focus-jump`（`--tol 0` 双跑，见 `shots/x100/M22/`）+ 改前 `search_jump … ok=0` / 红 toast「这个主题在当前视图之外」；改后 `search_focus_exit … levels=1` → `search_jump … ok=1` → `toast bad=false text=已退出聚焦并跳转`；改前→改后差 **189 421 px**（8.2213%；搜索卡片 **0 px**、面包屑 5 156 px、画布区约 172 000 px、提示条 11 390 px） | 稳（机器；实机 IME/键盘未核） | 2026-10-31 第 45 轮：M20 留下的「聚焦模式仍不可达只 toast」。聚焦是用户显式进入的视图态，但点搜索结果同样是显式动作——目标在聚焦子树外时先 `focus_to(0)` 退出全部聚焦，再走既有的展开/选中 + 居中；`focus_exit_levels` 是纯函数（无聚焦 / 目标可见 / 目标不存在 → `None`）；视图态变更不入撤销栈 |
| M23 | 插入关系线（`Ctrl+L` / 右键菜单，思维导图） | `rel_link_step_commits_only_on_another_node`（命中另一节点=提交、自身/空白=取消、无源=空闲）、`escape_walks_rel_link_then_relationship_then_focus_then_selection`（Esc 四层顺序） | `mm-rel-link` / `mm-rel-link-created`（`--tol 0` 双跑，见 `shots/x100/M23/`）+ `rel_link_mode` / `rel_link_created` / `select relationship` / `toast` 轨迹；预览帧 vs 落地帧差 **38 459 px**（2.1982%；画布 21 616、提示条 3 270、标题栏 1 843、状态栏 1 274） | 稳（机器；实机键鼠/IME 未核） | 2026-10-31 第 46 轮：`docs/02` §10 的 `Ctrl+L` 与 §3 的关系线此前没有**可发现**入口——只有悬停节点四边的锚点能拖出一条线（M2b-4），菜单/快捷键都没有。现在单选节点按 `Ctrl+L` 或右键「插入关系线」进入插入态（源环 + 跟随指针的虚线 + 十字光标），点另一主题落一条 `AddRelationship` 并选中、`Ctrl+Z` 一步收回；点自身/空白/注解/折叠点取消且这一击照常生效，`Esc` 取消；重复对提示不落地。**未做**：跨标签页的插入态不主动清理；实机 IME 未核。菜单多一行后，640×460 矮窗的节点菜单进入滚动（`menu-narrow` 可见滚动条，「聚焦」需滚一下） |
| M24 | 边界 / 概要的键盘路径（`Ctrl+B` / `Ctrl+]`，思维导图） | `annotation_shortcuts_map_only_exact_ctrl_b_and_bracket`（B/]、无修饰、Ctrl+Shift+B、Ctrl+A）、`annotation_shortcut_errors_distinguish_no_selection_from_root`（没选 vs 选根） | `mm-boundary-key` / `mm-summary-key`（`--tol 0` 双跑，见 `shots/x100/M24/`）+ `annotation_add` / `annotation_add_failed` 轨迹；边界帧 vs 只选中差 **10 579 px**（0.6047%）、概要帧 vs 两颗多选差 **4 954 px**（0.2832%） | 稳（机器；实机 IME 未核） | 2026-10-31 第 47 轮：`docs/02` §10 脚注⑤ 自 10-18 起写着「`Ctrl+B/]` 仍无快捷键」，右键菜单第二列也是空的。补 `Ctrl+B` 添加边界、`Ctrl+]` 添加概要（仅思维导图，复用菜单同名动作，一次撤销）；`take_exact_key` 精确匹配，`Ctrl+Shift+B` / 普通 `B`、`]` 不受影响；没选/选根给两种提示并记轨迹，概念图不响应；菜单两行补快捷键列。**未做**：实机 IME 下 `Ctrl+]` 归属未核；单颗子主题也能建概要，未额外提示 |
| M25 | 大纲行「更多」`⋯` 与行右键菜单（思维导图） | `row_more_rect_sits_in_the_action_lane`（24×28、距右 8、垂直居中）、`row_menu_scope_keeps_a_multi_selection_only_for_its_members` | `outline-more` / `outline-row-menu` / `outline-row-menu-add`（`--tol 0` 双跑，见 `shots/x100/M25/`）+ `outline_row_context` / `outline_row_menu entries=12|5` / `outline_row_action` / `menu_close reason=escape` / `outline_row_menu_skipped reason=concept_map` 轨迹；`outline*` 九场有意重拍（角标左移 128–272 px，`outline-wrap` 换行差 37 166 / 47 089 px） | 稳（机器；实机 IME 未核） | 2026-10-31 第 48 轮：`docs/06` §6 记着「大纲行的「…」仍是占位」。补固定 24px 动作道 + 悬停/选中出现的 16px `⋯`，与行右键共用一张菜单（单行 12 项 / 多行 5 项，画布专属项不收），菜单键盘先于画布与行键；概念图大纲不出 `⋯`（动作集另开轮次）。证据阶段修掉两处与大纲键位不符的菜单标签（`Tab` 留空、`/` → `Alt+.`）。**未做**：设为根主题（P2）、概念图大纲行菜单 |
| M26 | 节点 / 关系线 / 注解三条选中道互斥；`Delete` 只删当前道 | `selection_lanes_keep_exactly_one_lane`（三条道 × 非空组合）、`delete_press_reaches_exactly_one_lane`（关系线 → 注解 → 节点 → 无） | `mm-rel-select-arrow` / `mm-rel-select-delete`（`--tol 0` 双跑，见 `shots/x100/M26/`）+ `selection_clear lane=relationship reason=navigate` / `delete_dispatch lane=nodes` 轨迹；改前→改后差 **1 628 px**（关系线包围盒内 710,515–1094,619）与 **163 px**（确认框外露出的线段 1045,517–1095,526） | 稳（机器；实机键鼠未核） | 2026-10-31 第 49 轮：`HANDOFF` §7.5 第 5 条「节点与关系线选中互斥」未做——方向键遍历、框选/套索松手、`select_ids`、检查器选注解都能在关系线/注解仍选中时再立起节点道；一次 `Delete` 连调三个删除器，会同时删线并立节点确认。现在 `SelectionLane` + `foreign_lanes` + `clear_foreign_selection` 守住三选一，`Delete` 按 `delete_lane` 只走一道。**未做**：概念图的三道像素证据只锁轨迹未各加金样；关系线多选仍不存在 |
| M27 | 快捷键配置：查看 / 改绑 / 冲突 / **禁用** / 持久化 | `a_disabled_shortcut_frees_its_chord_and_survives_a_round_trip`、`enabling_restores_the_factory_key_unless_it_is_taken`、`none_and_blank_values_disable_while_broken_values_fall_back`（`mw-shell`，3 条） | `shortcuts-default` / `shortcuts-disabled`（`--tol 0` 两遍，见 `shots/x100/M27/`）+ `shortcut disable action=open` → `"open":"none"` → 重启后 `shortcut enable action=open` → `"open":"Ctrl+O"` 行为脚本；默认态↔禁用态差 **3 557 px**（只在三行的和弦/按钮列） | 稳（机器；实机 IME 未核） | 2026-10-31 第 50 轮：`docs/02` §10 从 10-18 起写着「禁用单项冲突项」，面板却只有「改」，`shortcuts.json` 也表达不了未绑定。现在 `entries` 改 `Option<Chord>`，`disable`/`enable`（恢复出厂键、被占用报占用者）、`"none"` 持久化、禁用项不占和弦；28 行面板每行补「禁用/启用」。**未做**：画布编辑键仍不可重绑；`"none"` 无格式版本；实机输入法占用未核 |
| M28 | 撤销/重做：栈上限 **100–2000 可配**（设置中心「编辑」组） | `history_limit_defaults_and_clamps`、`lowering_history_limit_drops_oldest_and_undo_stops_there`、`redo_cannot_push_history_past_limit_after_lowering`、`coalesced_apply_honors_history_limit`（`mw-core`，4 条）、`undo_limit_missing_uses_default_and_clamps`（`mw-shell`，1 条） | `settings`（`--tol 0` 两遍，见 `shots/x100/M28/`）+ `history limit=100 depth=17/0` 轨迹；`settings-after` ↔ `settings-limit-100` 差 **812 px**，bbox `(445,476)–(632,511)`（只在滑轨行） | 稳（机器；实机键盘未核） | 2026-10-31 第 51 轮：`docs/02` §2 的「默认 500 步（可配 100–2000）」此前是编译期常量，用户只能接受 500。`Document` 新增内存字段 `history_limit`（不进 `.zmind`、不升 `FORMAT_VERSION`），`apply` / `apply_coalesced` / `redo` 三条入栈路径统一裁剪；`settings.json` 加 `undoLimit`（缺键 500）；设置中心「编辑」组加滑轨（100–2000，步进 100）；保存立即作用到画布与所有标签，每帧对账兜住新文档；`MW_UNDO_LIMIT=100` 旋钮。**未做**：滑轨步进 100（150 需键入）；降上限后旧记录不可恢复且无二次确认（M28b 已补提示）；不做按文档覆盖 / 撤销历史面板 |
| M28b | 撤销栈上限调低的**提示**（保存前说清丢几步） | `undo_limit_dropped_counts_only_real_losses`（六档：持平/抬高/深度未顶到/真丢 10 步/正好等于上限/空历史） | `settings-undo-warn`（`--tol 0` 两遍，见 `shots/x100/M28b/`）+ `settings warning dropped=27 depth=127 limit=100` 轨迹；警告色像素 377、bbox `(577,546)–(774,556)`；两个不同 appdata 目录各跑一遍零差异 | 稳（机器；实机 IME/键盘未核） | 2026-11-01 第 59 轮：M28 的「降上限无二次确认」落地为滑轨下方的 28px 警告行（不弹模态），文案由纯函数按「撤销+重做深度 − 新上限」算。金样踩到两个坑并修掉：① `MW_SETTINGS_DEMO` 开局就开对话框，后续 Enter 全被吃掉 → 新增帧控 `MW_SETTINGS_AFTER`；② 对话框显示 APPDATA 下的笔记路径，换目录就漂 47 px → 显示改草稿值 + `MW_SETTINGS_NOTES_DIR` 固定。**未做**：不改保存按钮文案、不弹模态；滑轨仍步进 100；实机未核 |
| M29 | 全局搜索的匹配方式（默认拼音 / **精确文本**可切） | `text_mode_ignores_pinyin_and_initials`、`text_mode_still_matches_notes_links_and_relationships`、`text_mode_does_not_build_pinyin_keys`、`match_mode_defaults_to_pinyin_and_text_is_case_insensitive`（`mw-core`，4 条）、`search_bar_toggles_mode_and_clears_the_current_row`（`mw-shell`，1 条） | `search-text`（`--tol 0` 两遍，见 `shots/x100/M29/`）+ `search_hits q=zw rows=1` → `rows=0`、`search_mode mode=text rows=0`；八张既有搜索基线有意重拍，每张差 **2 028 px**、bbox 只在脚注开关（约 101×20） | 稳（机器；实机 IME/键盘未核） | 2026-10-31 第 52 轮：`docs/02` §3 的 P0 验收「匹配方式可切换「精确文本 / 拼音」」从 10-18 起就存在，代码里 `pinyin_eligible` 只看查询词是否 ASCII——搜 `xiangmu` 必带拼音命中，想精确找字面串也关不掉。`SearchBar` 新增 `MatchMode`（默认拼音，单次搜索的临时选择、关掉即复位），脚注右侧两段开关（8% 强调底 + 强调色文字）；`Document::search_with_mode` / `search_relationships_with_mode` 透传，Text 模式**不建也不读**拼音键。键位实测两败：`Ctrl+P` 与 `Ctrl+Shift+P` 都被全局「导出 PDF」先吃（egui 逻辑匹配允许额外 Shift），最终 `Ctrl+Shift+M`。**未做**：窄窗短标签已由 M29b 补实拍；匹配方式不持久化；实机 IME 未核 |
| M29b | 搜索脚注开关的窄窗布局（实拍 + 纯函数） | `mode_toggle_fits_the_footer_at_every_width`（240/326/420/560 四档：左留 64 给提示、右留 74 给 Esc、两段首尾相接） | `search-narrow`（`--tol 0` 两遍，见 `shots/x100/M29b/`）+ `search_footer width=359 narrow=1 toggle=[218,298]` 轨迹；宽窗同轨迹仍 `width=560 narrow=0 toggle=[434,536]`，既有八张搜索基线零漂移 | 稳（机器；实机 IME/键盘未核） | 2026-11-01 第 57 轮：M29 的窄窗分支（「精确文本」→「文本」）从未实拍，且 40/40/62/右让 74 是行内常量、无单测。抽出 `mode_toggle(footer, narrow)`（+6 个常量）并补一张 640×700 基线；harness 的 `--ppp 1.0` 会让逻辑宽=物理宽（卡片仍 560），必须 `--ppp 1.5` 才拍得到 `narrow=1`。**未做**：卡片 240 下限时左侧提示被裁多少未逐字核对 |
| M30 | 画布缩略卡（小地图）：右下 190×130、视口框、点击/拖动定位、滚轮缩放、可折叠为 34×26 角标 | `minimap_rect_layout`（尺寸/锚点/24px 折叠命中区）、`minimap_hides_when_the_canvas_is_too_small`、`minimap_wheel_steps_one_zoom_gear_per_notch`；`mw-ui::overview` 20 余条纯几何单测（世界⇄卡、拖动钳制、900 块降采样） | `mm-minimap` / `mm-minimap-collapsed`（`--tol 0` 两遍，见 `shots/x100/M30/`）| `minimap toggle` / `minimap pan` / `minimap zoom` 轨迹；展开↔折叠互差 **26 482 px**，对「不开缩略卡」的对照帧差 **27 080 / 1 184 px**（bbox 只在右下角 198×138 / 42×34）| 稳（机器；实机键鼠手感未核） | 2026-11-01 第 55 轮：`docs/02` §4 的 P1「小地图」在视觉 v2 重写时整块丢了——纯几何层 `mw-ui::overview`、壳层布局与 `Shell` 两个字段都在，却没有任何绘制/交互入口，构造 `Shell` 时字段甚至没初始化（`E0063`）。现在接回中央画布：右下角、缩放条上方 8px；节点代理块按分支色、关系线 0.8px、视口框淡强调底 + 1px 强调边；卡内点击/拖动走 `overview::center_from_thumb` 钳制，悬停滚轮一档缩放；`–` 折叠、角标点击展开；命令面板 / 顶栏「视图」菜单可开关，状态落 `window.json`（出厂关，既有基线零漂移）。折叠命中区 18 → 24（`docs/13` §4.19 旧账），视觉图标仍 12px。**未做**：双击缩略卡定位节点、缩略图生成 `thumbnail.png`、选中环/根描边细节；实机 150%/200% DPI 与拖动手感未核 |
| M30b | 缩略卡双击定位节点 | `node_at_hits_the_box_and_honours_the_pad`（盒内必中、4 容差外不中、12 容差内中、远处不中） | `mm-minimap-locate`（`--tol 0` 两遍，见 `shots/x100/M30b/`）+ `minimap locate id=… hit=1` / 空白 `hit=0` 轨迹；定位帧对 `mm-minimap` 差 **274 314 px**（选中环 + 相机居中） | 稳（机器；实机鼠标双击节奏未核） | 2026-11-01 第 58 轮：M30 只做了「拖视口框」，而 `overview.rs` 注释里早写明双击定位节点应走 `reveal_center`。新增 `mw_ui::overview::node_at(scene, world, pad)`（多颗命中取盒中心最近），壳层在 `minimap_panel` 里处理 `double_clicked`：命中 → `select_node` + `reveal_id`；空白 → 只记 `hit=0`，相机按普通点击搬视口框。容差按卡上 4 css px / `map.scale` 折成世界单位。**未做**：双击空白新建概念（概念图惯例但不引入）、悬停显示节点名 |
| M31 | 「设为根主题」（把选中的主题提为中心主题） | `set_root_rewires_the_tree_and_undoes_in_one_step`、`set_root_from_grandchild_keeps_the_ancestor_chain_under_the_old_root`、`set_root_clears_the_free_position_of_the_promoted_topic`、`set_root_rejects_concept_maps_the_root_itself_and_missing_ids`（`mw-core`，4 条） | `mm-set-root`（`--tol 0` 两遍，见 `shots/x100/M31/`）+ 轨迹 `menu_open … entries=21` → `menu_nav key=down label=设为根主题` → `set-root id=… ok=1` → `scene_build nodes=13`；sha256-12 `B95E11237D8F` | 稳（机器；`stage-b` 整门结果见 STATUS §1） | 2026-11-01 第 64 轮：`docs/02` §2 的「节点编辑」与 M25 的「未做：设为根主题（P2）」都只差这一条。`ChangeSet` 早有 `before_root`/`after_root` 快照（概念图删主概念用过），所以换根做成一条命令 `Command::SetRoot { id }`：新根摘出原父级（概要跨度收口）、旧根整棵树追加到新根孩子末尾、`root_id` 换新，一次撤销全还原；自由主题提根交出 `pos`；概念图 / 已是根 / 未知 id 三种拒绝路径。入口：画布节点右键菜单与大纲行菜单（单选、非根行），落地后 `fit` 一次防「点了没反应」。**未做**：快捷键、确认框（不必要，一步可撤销）、旧根插入位置固定末尾、实机键鼠节奏 |
| M32 | 双击折叠圆点「展开到下一级」（逐级深入） | `drilling_into_a_collapsed_node_reveals_exactly_one_level`、`drilling_into_an_expanded_node_opens_only_its_direct_children`、`drilling_is_a_noop_when_nothing_would_change`、`the_plan_skips_children_already_in_the_target_state`、`leaves_never_get_a_collapsed_flag_written`、`drilling_is_one_undo`、`leaves_unknown_ids_and_concept_maps_do_not_drill`（`mw-core::drill`，7 条）；`two_clicks_on_the_same_dot_within_the_window_are_a_double_click`、`a_third_click_does_not_repeat_the_drill`、`a_different_node_restarts_the_clock`、`clicks_outside_the_window_are_not_a_double_click`、`moving_more_than_four_css_pixels_is_not_a_double_click`、`a_command_between_the_two_clicks_cancels_the_drill`（`mw-ui::drill_click`，6 条） | `mm-toggle-drill` / `mm-toggle-drill-deep`（夹具 `MW_DRILL_DEMO=纯 Rust 原生`，`--tol 0` 两遍，见 `shots/x100/M32/`）；sha256-12 `3C799A366D85` / `69083B4AF024`；轨迹 `toggle idx=1 collapsed=…` → `undo label=折叠/展开` → `toggle_drill id=… depth=N open=0|1`；对「只单击折叠」的对照帧差 **61 774 px / 3.5307%**（bbox `(271,381)–(934,720)`），`-deep` 对 `mm-toggle-drill` 差 **11 975 px / 0.6844%**（bbox 只在展开的子树） | 已实现（机器；实机双击节奏未核） | 2026-11-01 第 66 轮：`docs/02` 的 P2「展开层级」10-18 就写着，代码里却没有——双击与单击同义，`docs/06` §5 还把这条标成「未实现」。规划器 `mw_core::drill::drill_toggle_command` 在**点之前的状态**上算「恰好露一层」（折叠态：展开自己 + 有下一层的直接子级折回；展开态：直接子级展开），叶子不写折叠态、0 条变化返回 `None` 所以不留空撤销记录；双击识别在 `mw_ui::drill_click::ToggleClickMemory`（同节点 + ≤500ms + ≤4 css px + 撤销深度没变，时间用 egui 虚拟时钟不是墙钟），第二击先撤掉第一击再下钻，一次双击 = 一条撤销。**代价**：双击前一帧会闪出第一击的中间态（金样按最终帧截，不受影响）。**未做**：不读系统双击间隔（写死 500ms）、实机鼠标节奏未核、概念图不提供（无折叠语义） |

| M10 | 平移/缩放锚点（空格拖、中键、Ctrl+滚轮、双击空白） | `middle_button_pans…`、`space_drag_pans…`、`panel_shift_*` | `tabs-wheel*` | 稳 | — |
| M11 | 悬停工具条（及其贴边翻转） | `hover_bar_*`（6 条） | `o03`/`o04` | 稳 | — |
| M12 | 5k/20k 大文档：增量重排、平移不重排 | `the_box_cache_remeasures_only_when_a_size_input_changes`、`press_can_name_its_button_and_defaults_to_primary`、`a_cache_hit_shares_the_line_storage_instead_of_cloning_it`、`the_scene_index_agrees_with_ids_whether_floaters_are_merged_or_not` | `scene_build` 轨迹 + `perf-scene.txt`（`shots/x100/M12/`）；分阶段账 `MW_LAYOUT_LOG`（`M12b/`）；A/B 账（`M12d/p7-scene-total.txt`） | 5k 稳 · **20k P7 口径达标** | **P7「布局函数计时」p50 14.49 → 13.00 / p95 16.07 → 13.90 / max 16.67 → 15.55ms（打磨轮 14 首次达标）**；端到端（含命令 + 账本克隆）中位 16.7ms、p95 18.4ms，**仍超 16ms**——差额在 `[scene]` 之外：账本克隆约 1.2ms 与 20k 次 `String` 分配（要 `Arc<str>` 化 `Scene::ids/index`，动 ~150 处）；`bounds` 1.15ms 与 `order` 1.0ms 仍是每轮 O(n)；`m5-perf.ps1` 本机没拉起窗口，真实鼠标节拍的 P5 未复测；"
 "**P9 残账已收口（打磨轮 15/16）**：文字撤销/重做 20k **15.5 / 15.4ms**、结构撤销 **15.8 / p95 17.1ms**（都达标）—— "
 "两代账 + `prefer_previous()` + 「指纹对不上的那代也留作上一代」；账本内存翻倍未封顶 |

| M13 | 打开就有文档时的取景（启动重取景） | 无（挂在 `App::update` 帧循环上，单测没有无头 `App`） | `event fit vp=…` 轨迹（首帧 `1280×728 → 1.2369`、落定后 `1920×1128 → 1.5`）+ `cm-default` 与手动 `Ctrl+1` **整图 0 像素差**（`shots/x100/M13/`） | 稳（本机；更慢的机器未测） | 打磨轮 19：`App` 记 `start_refit_frames=60`，启动阶段**视口一变才重取景**（`fit_viewport` 对比 `viewport`），修掉「首帧视口没落定 → 文档被画小一档」；第一版「每帧重取 12 帧」会让基线跟着落定时刻飘（`cm-arrow-menu-click` / `cm-focus-question-hover` 两遍之间自己不同），已换成按变化重取 |
| M14 | 选中 / 悬停的**底色**反馈 | 渲染常量 `SELECT_FILL=0.08` / `HOVER_FILL=0.04`；金样为证 | `cm-crosslink` 等选中场景重拍前后差 | 稳（机器；实机色彩未核） | 2026-10-30：以前只有环、没有 §5.3 要求的底色；本轮在覆盖层先铺淡强调底再画环，深色/彩色节点上「被拿起来」的感觉才成立。**未做**：键盘焦点环与选中底叠加时的色量没有单独测 |

## 2. 概念图

| # | 操作 | 单测证据 | 视觉 / 端到端证据 | 状态 | 已知粗糙 |
| --- | --- | --- | --- | --- | --- |
| C1 | 双击空白新建概念 | canvas 测试 + `cm-*` | `cm-default` 等 | 稳 | — |
| C2 | 拖动锚点建关系（含拖到空白建新概念） | `anchors_win_over_the_node…` | `cm-link-create`、`cm-link-drag-pointer` | 稳 | — |
| C3 | 关系端点拖拽（RelinkDrag） | `a_near_miss_snaps_inside_the_margin_only`、`an_inside_hit_beats_a_nearer_outside_one`、`the_nearest_outside_node_wins` | `cm-relink-drag`、`-control`、`-select-only`、`cm-relink-near`（框外 6px，进门）+ `relink-preview ... target=` 轨迹 | 稳（实机鼠标手感未核） | 打磨轮 12：落点判定从「严格包含」放宽到「压中 ∪ 12 css px 内最近」（`LINK_SNAP_CSS`，按镜头换算），修掉「差几像素就新建一颗概念」；夹具不再写死 `target: None`；圆角盒按外接矩形算余量、四角各多 ~1px；吸附没有中间态 |
| C4 | 四档箭头 + 检查器入口 | `mw-core` 箭头单测 | `cm-arrow-menu*`、`cm-arrow-inspector` | 稳 | — |
| C5 | 关系短语：双击/点前缀就地改、空串清零（**仅思维导图**） | `mw-core::via_id` 等 | `cm-outline-via`、`cm-rel-focus-*` | 稳 | 打磨轮 40（C16）复核：**概念图里「空串清零」不成立**——模型守卫 `SetRelationshipLabel` 拒绝，壳层以前静默失败（见 C16 行）；思维导图仍可清空短语 |
| C6 | 聚焦问题（画布带 / 检查器 / 大纲 / 导出） | 有 | `cm-focus-question*` | 稳 | 带子不封顶行数（取舍已记） |
| C7 | 整理布局（丢弃手工偏移） | `arranging_three_concepts_is_one_undo`（`arrange_batch` 一条 Batch + 一次 `undo` 全复原） | `cm-arrange`、`cm-arrange-undo`（整理 → 一次 Ctrl+Z，进门）+ `concept-arrange` / `undo` / `toast` / `undo fit=` 轨迹 + 画布逐像素对照（`pixel-diff.txt`） | 稳（实机键鼠手感未核） | 打磨轮 13：落地补 toast「已整理 N 颗概念 · Ctrl+Z 撤销」；打磨轮 18：**撤销踩到整理那一条时连取景一起 `fit` 回来**（重做再 `fit` 一次），并让提示条在 harness 里不按墙钟过期（基线锁得住）。残留：`fit` 是重算不是回放，用户如果先手动平移过、再整理、再撤销，镜头回的是「这套位置的适应取景」而不是整理前那一帧 |
| C8 | 交叉微弧 + 跨分支取色 | `segments_cross`、`bow_crossings` | `cm-crosslink`、`cm-rel-curve` | 稳 | — |
| C9 | 强调淡入（动画中间帧） | `emphasis_alpha` 5 断言 + `parse_pin` + `motion_span_sees_a_new_relationship…` | `cm-link-fade-mid`（定格 t=0.5，进门）+ `rel_emphasis` 轨迹 | 稳 | 定格是常量 t 不是实拍；只拍 0.5 |
| C10 | 右键菜单与「排列」子菜单贴底夹取 | `place_submenu` 四条（含竖向两条） | `cm-submenu-bottom`（贴底夹取，进门）+ `menu_submenu_rect` 轨迹 | 稳 | 夹取后盖住父菜单下半部分（既有取舍） |
| C11 | 大纲 ↔ 概念图互转（Tab/Shift+Delete/Shift+F2） | `row_key` 断言 | `cm-outline-link/unlink/via`（已进门） | 稳 | — |
| C12 | 概念 ↔ 卡片联动（圆点 / 反链） | `has_card` | `cm-card-link*` | 稳 | 圆点只有一张耦合证据 |
| C13 | 复制 / 粘贴 / `Ctrl+D` / `Ctrl+拖拽` | `inserting_concepts_keeps_them_floating_and_undoes_in_one_step`、`insert_concepts_is_concept_map_only_and_rejects_tree_payloads`、`inserting_a_concept_does_not_steal_an_already_bound_card`、`concept_copy_preserves_propositions_but_never_parent_links`、`tree_clipboard_pastes_as_independent_concepts_without_parents` | 既有 `cm-*` 选中态 + `trace concept-copy n=…`（新语义无独立稳态外观，不新增基线） | 稳（机器；实机手感未核） | 2026-10-30：四条路径都改走 `InsertConcepts`，副本是独立概念、保留内部命题、一次撤销；卡片绑定解绑（1:1 约束）。**未做**：跨文档卡片复制与真实鼠标连续拖拽 |
| C15 | 平行命题的短语避让与独立点选 | `opposite_propositions_between_the_same_pair_also_take_lanes`、`parallel_proposition_labels_do_not_overlap_on_a_vertical_edge`、`crossing_bow_leaves_a_lane_curve_alone`（`mw-core`）；`parallel_propositions_are_individually_hittable`（`mw-render`） | `cm-parallel-select-a` / `-b`（两张各点一条车道，`--tol 0` 两遍，见 `shots/x100/C15/`）+ `scene_rel` / `select relationship` 轨迹 + A/B 差 2 528 px（bbox 只在车道竖带） | 稳（机器；实机鼠标手感未核） | 2026-10-31 第 39 轮：车道必须**跨源节点**收齐（反向命题曾整条重合，`label_at` 距离 0.00）；短语沿规范切线错开 24（竖边曾重叠 12 世界单位）；`bow_crossings` 不再重弯车道曲线。三条以上车道仍用常量步长（**2026-11-01 打磨轮 63 / C21 已收口**：≥3 条按短语盒投影宽自适应、净空 ≥ 24，见下行）；靠近端点时两条线汇合，点选仍会落到后一条（曲线几何，未改） |
| C16 | 命题短语的**空白提交口径**（拒绝 + 提示，不留线团） | `blank_phrase_commit_follows_the_concept_map_contract`（九条：未改 / 去空白未改 / 空拒绝 / 空格拒绝 / 新建线空撤线 / 新建线空格撤线 / 新词 / 改动 / 思维导图 `Set(None)`） | `cm-phrase-clear`、`cm-phrase-blank`、`cm-outline-via-clear`（`--tol 0` 两遍，见 `shots/x100/C16/`）+ `rel-label-reject reason=empty/blank_new`、`outline_concept_via_reject`、`toast` 轨迹 + 像素对照（画布 0 差、只多提示条） | 稳（机器；实机微软拼音未核） | 2026-10-31 第 40 轮：以前两个入口都静默失败——既有短语被清空时命令被模型拒绝、编辑器却已关掉（什么都没说）；新建边按 Enter 留空会**真的落一条 `w=0` 的线团**。现在既有命题空提交 = 保留原短语 + 提示，新建边空提交 = 撤线 + 提示；顺带修掉短语框开着时 `Ctrl+A` 漏进大纲、把 6 行全选中的键盘归属问题。**未做**：撤销可把线团恢复回来（模型无全局不变量）、实机 IME |
| C17 | 命题短语进全局搜索（原文 / 全拼 / 首字母 + 选中跳转） | `relationship_text_hit_reports_both_ends_and_range`、`relationship_pinyin_hit`、`relationship_initials_hit`、`blank_relationship_label_never_hits`、`relationship_cache_invalidates_with_phrase`、`long_phrase_still_matches_beyond_pinyin_cap`、`relationship_ties_follow_doc_order_then_edge_index`、`relationship_excerpt_folds_middle_range`、`relationship_excerpt_flattens_newlines`（`mw-core`，9 条）+ `relationship_results_jump_to_the_edge`、`relationship_group_sits_between_nodes_and_cards`（`mw-shell`，2 条） | `search-rel` / `search-rel-jump`（`--tol 0` 两遍，见 `shots/x100/C17/`）+ `search_hits q=支撑 rows=1 note=0 link=0 rel=1`、`search_rel_jump … ok=1` 轨迹 + 改前 `rows=0` 对照；改前→改后差 27 216 px（只在搜索卡片），两新基线互差 71 547 px（画布选中环 + 相机） | 稳（机器；实机 IME/键盘未核） | 2026-10-31 第 42 轮：关系线是边不是节点，此前 `Ctrl+F` 只扫 `node.text/note/link`，搜「支撑」一条都不出。现在边短语按文档前序 + 边下标稳定排序（原文 560 / 全拼 450 / 首字母 350），结果新增「命题」组（节点 < 命题 < 卡片 < 标签 < 路径），第二行「概念 · 源 → 目标」，点击/Enter 选中该边并居中两端中点、搜索框不丢焦点；拼音缓存 `rel_cache` 按短语快照失效、240 字封顶。**未做**：命题不参与 `Ctrl+H`、不索引颜色/箭头/概要、每帧仍扫全部边、20k × 大量命题的首查延迟未实测 |
| C18 | 「钉住」概念与「只整理选中」局部布局 | `arrange_scoped_keeps_fixed_concepts_at_current_positions`、`arrange_scoped_all_fixed_is_noop`、`arrange_scoped_empty_fixed_behaves_like_arrange`、`arrange_scoped_clamps_overflow_and_underflow`（`concept_layout` 4 条）；`pinned_concept_toggles_and_undoes_in_one_step`、`pinned_concept_refuses_tree_and_non_concept_map`（`document` 2 条） | `cm-pin`（117px 钉住徽标）、`cm-arrange-pinned`（`--tol 0` 两遍，见 `shots/x100/C18/`）+ `concept-arrange scoped=1` / `concept-pin id=...` 轨迹 + `cm-multi-menu` / `cm-submenu-bottom` 有意重拍 | 稳（机器；实机键鼠未核） | 2026-10-31 第 53 轮：`docs/30` §5.5 规定「整理布局支持全局或局部整理，允许用户钉住某些概念位置」。此前只有整张全局整理，所有概念强制覆盖为 DAG 自动位置，用户精心摆放的格局全被冲掉。新增 `Command::SetConceptPinned`（一次撤销、树节点拒绝）与纯函数 `concept_layout::arrange_scoped(input, fixed)`，固定概念保持 `current[i]` 严格不动，但仍参与其他可动概念的拓扑分层与重心吸附。右键菜单补单选/多选「钉住位置」、「整理选中概念」、「整理全部布局」；覆盖层绘制右上角 5.5px 强调色圆点徽章。**未做**：钉住状态不进 `.zmind` 持久化（内存交互态）；检查器未设钉住开关；实机手感未核 |
| C19 | 概念图检查器属性：概念元信息、钉住开关与局部整理入口 | `object_meta_for_concept_map_*` (2 条) + `concept_map_style_body_*` + `concept_layout_pinned_command_and_undo` (4 条) + `concept_proposition_count_and_pinned_helpers_work` (1 条) | `cm-inspector`、`cm-inspector-pinned`（`--tol 0` 两遍，见 `shots/x100/C19/`）+ `trace.txt` + 两图差 4 659 px（检查器 3 027 px + 画布徽标 1 632 px） | 稳（机器；实机键鼠未核） | 2026-11-01 第 54 轮：`docs/30` §4.2/§5.5。改造对象行消除树语义（识别为「概念」、统计命题数与钉住态）；样式页分组标题隐藏「含子树」切换并屏蔽「文字样式应用到子树」；新增「概念排布」分组（键 `concept/layout`，默认开，活摘要），内嵌 28px `switch_row`「钉住位置」轨道开关与「整理选中概念」/「整理全部布局」快捷按钮。**未做**：钉住状态未持久化；实机高 DPI/手感未核 |
| C20 | 概念图大纲行菜单（`⋯` 与行右键，命题网动作集） | `concept_row_menu_is_proposition_only`（7 条动作顺序 + 无树语义 + 两个依赖边的禁用态） | `cm-outline-menu` / `cm-outline-menu-cancel`（`--tol 0` 两遍，见 `shots/x100/C20/`）+ `outline_row_menu … concept=1 entries=9` → `menu_nav` ×3 → `outline_row_action kind=EditVia` 轨迹 + 菜单帧对无菜单对照 **25 848 px**（bbox 只在菜单区）；`cm-outline-via` / `-via-clear` / `-unlink` 各 **36 px**（选中行 `⋯`，bbox `x[1336..1351] y[166..169]`） | 稳（机器；实机键鼠未核） | 2026-11-01 第 56 轮：`docs/30` §5.6 要求「像编辑思维导图一样用大纲改结构」，键盘路径（Tab / Enter / Shift+Delete / Shift+F2）在 CM-09 就有，但 M25 把概念图行菜单**有意跳过**（`outline_row_menu_skipped reason=concept_map`），用户没有任何可发现入口。现在概念图大纲也有同一枚 `⋯` 与行右键菜单，动作集只有命题网语义：新建概念并连线 / 新建概念 / 编辑连接短语 / 断开进入的边 / 重命名概念 / 复制 / 删除概念，全部复用既有 `OA` 动作（无新命令）；无进来边的行禁用改短语与断边。**未做**：概念图多选只给「复制 / 删除概念」（无批量改短语/断边）；页头搜索行开着时的互斥未单独拍图；实机 IME / 手感未核 |
| C21 | 三车道以上平行命题的等距避让（短语盒净空 ≥ 24） | `three_lane_scene_keeps_24_world_unit_label_clearance`、`two_lane_offsets_match_the_legacy_formula`、`five_lane_long_labels_do_not_overlap`、`diagonal_lane_band_stays_symmetric`、`degenerate_inputs_are_safe`、`short_edge_three_lanes_do_not_overlap`（`mw-core`，10 条含场景级） | `cm-parallel-3`（`--tol 0` 两遍，见 `shots/x100/C21/`）+ sha256-12 `6BA2BF3A32FE`；改前量得 3 车道相邻短语净空 **4.00**（竖边）/ **0.00（真重叠）**（短边）；2 车道与既有金样逐位不变 | 稳（机器；实机键鼠未核） | 2026-11-01 第 63 轮：C15 的车道偏移是常量 `min(len/6,24)` + 切线常量 24，短语加宽不会换来间距，短边上会真重叠。新增纯函数 `concept_layout::proposition_label_offsets(label_widths, tangent)`：≥3 条时按每条短语盒在切线上的投影半宽累加（+24 净空），镜像侧取大、中车道为 0；1/2 条走旧公式逐位一致。`scene.rs` 只做「收车道 → 量宽 → 查表」。**未做**：法向间距仍可能 < 20（靠切线避让，标签可能伸到端点外）；「法向也参与分配」会改车道曲线，需单独批准；实机高 DPI 未核 |

## 3. 下一轮候选（按「最弱」排序，随时可换）

1. ~~**M5 拖拽落点指示器**~~ ✅ 2026-10-29 第 1 轮完成：四张基线 `mm-drag-*` 进门。
2. ~~**M1/M2 新建后的编辑态**~~ ✅ 2026-10-29 第 2 轮完成：`Tab`/`Enter` 按 `docs/07` §3.2 补齐，
   `Esc` 收回新建节点；四张基线 `mm-new-*` / `mm-enter-sibling` 进门。
3. ~~**C9 强调淡入中间帧**~~ ✅ 2026-10-29 第 3 轮完成：`MW_MOTION_T` 定格 + 关系线增删也算结构变化
   + `blend_scenes` 插 alpha + 强调读插值帧；基线 `cm-link-fade-mid` 进门。
4. ~~**C10 子菜单贴底**~~ ✅ 2026-10-29 第 4 轮完成：`MW_CONTEXT_POS` 把请求点挪到矮窗底边，
   基线 `cm-submenu-bottom` 第一次锁住夹取（`pos.y = 780−300−8 = 472`）。
5. ~~**M8 撤销节奏**~~ ✅ 2026-10-29 第 5 轮完成：查出「建节点 + 写名字」是两条撤销，
   一次 `Ctrl+Z` 只留下空盒子；`Document::apply_coalesced` 把这一次编辑里刚建的节点并进新建那条，
   四张基线 `mm-undo-{built,one,three}` / `mm-redo-one` 与 `undo`/`redo` 轨迹进门。
6. ~~**M12 大文档帧率**~~ ✅ 2026-10-29 第 6 轮完成（一半）：`scene_build` 轨迹证明「平移/缩放零重建」，
   全量重建的常数压下来 30–45%（5k 打字 13.7 → 7.7ms，20k 66.9 → 46.1ms）；
   但 20k 仍是整棵重排，P7 的 16ms 没达标——**真增量布局仍是未做项**，见 `shots/x100/M12/report.md` §7。
7. **20k 增量布局**：只重排脏子树（新行，2026-10-29 由 M12 拆出；20k 打字 47ms、撤销 47ms 都卡在这里）。
   进度：第 10 轮归账（`shots/x100/M12b/report.md` §4）、第 11 轮真增量（`shots/x100/M12c/`）——
   `SceneCache` 里留一份「上一次的账」（内容指纹 + 结构指纹 + 每节点盒子指纹 + 场景节点表），
   对得上就整段复用，20k 打字只剩一颗重测（`reused=19999`）：缓存路径 **29.0 → 18.6ms**。
   **P7 仍未完全达标**（p50 15.5 / p95 16.9）。下一刀：`Scene::ids` / `Scene::index` 改 `Arc`
   （`collect` 3.0 + `index` 1.2 都是每轮两万次 id 克隆）→ 预计再省 3–4ms。
   **第 14 轮**：`index` 不重建了（没有自由主题就沿用 `Layout::index`）、collect 栈不再克隆 id →
   P7 口径 **p50 13.00 / p95 13.90 / max 15.55ms，首次达标**；端到端 16.7 / 18.4ms 仍超线（`shots/x100/M12d/`）。
8. ~~**C3 关系端点落点余量**~~ ✅ 2026-10-29 第 12 轮完成：判定从「严格包含」放宽到「压中 ∪ 12 css px」
   （`target_node_snapped`），修掉「差几像素就新建一颗概念」；新基线 `cm-relink-near`，证据 `shots/x100/C3/`。
9. ~~**C7 整理布局的提示与撤销动线**~~ ✅ 2026-10-29 第 13 轮完成：落地补 toast、`arrange_batch` 纯函数 +
   `arranging_three_concepts_is_one_undo` 单测、新基线 `cm-arrange-undo`；证据 `shots/x100/C7/`。
10. ~~**撤销要不要连相机一起回退**~~ ✅ 2026-10-29 第 18 轮完成：撤销踩到整理那一条时重新 `fit`
    （不是回放旧相机——整理可能跑在首帧 `fit` 之前，那份相机不是用户看到的取景），重做同样 `fit`；
    证据 `shots/x100/C7/`（画布逐像素 0 差、`undo fit=1` 轨迹、新基线双跑）。
    顺带把提示条改成 harness 里不按墙钟过期，`cm-arrange-undo` 基线不再和时钟赛跑。
11. ~~**首帧 `fit` 用的是没落定的视口**~~ ✅ 2026-10-29 第 19 轮完成：启动阶段只在视口变化时重取景
    （`Canvas::fit_viewport` 对比 `Canvas::viewport`），同一包围盒首帧 `1.2369` → 落定后 `1.5`；证据 `shots/x100/M13/`。
    第一版「每帧重取 12 帧」不可取：结果取决于窗口在第几帧落定，两张悬停/菜单基线因此自己飘。
12. **M4 实机微软拼音**：阻塞项，要真机 IME 组合期核对（Esc 取消、候选窗、组合期 `Ctrl+Z`）。
13. ~~**M32 双击折叠圆点「展开到下一级」**~~ ✅ 2026-11-01 第 66 轮完成：`mw_core::drill::drill_toggle_command`
    在**点之前的状态**上算「恰好露一层」（折叠态展开自己并把有下一层的直接子级折回；展开态展开直接子级），
    0 条变化返回 `None` 不留空撤销；双击识别在 `mw_ui::drill_click`（同节点 + ≤500ms + ≤4 css px + 撤销深度没变），
    第二击先撤第一击再下钻 = 一次双击一条撤销。新轨迹 `toggle_drill` + 两张基线 `mm-toggle-drill` / `-deep`；
    顺带把 `docs/02`「展开层级」行的验收要点补上、`docs/06` 折叠圆点行的「未实现」改成真行为。证据 `shots/x100/M32/`。
14. **C12 概念 ↔ 卡片联动证据**（M 轨残留：圆点只有 `cm-card-link` 一张耦合证据；反链列表没有基线）。
15. **M5 的 `Ctrl` 拖拽复制与 150%/200% 条宽证据**（M5 行「已知粗糙」；纯证据轮，四张基线各加一档）。
    **2026-11-01 第 67 轮量过一件事，先记下来省下一轮的时间**：`--ppp 1.5` **不是**「同一张图更密的像素」。
    金样窗口物理尺寸固定 1620×1080，ppp 一变，**逻辑视口跟着变**：ppp 1.0 落定 `fit vp=1620x1006 zoom=1.3312`，
    ppp 1.5 落定 `fit vp=1080x646 zoom=0.8545`（实测 `shots/x100/M9b/probe-ppp150-trace.txt`，用 `mm-drag-child.txt` 跑的探针）。
    所以 150%/200% 档是**另一个取景下的另一张布局**——`mm-drag-child.txt` 里的 (463,605) / (1027,631)
    在 1.5 档落在空白，直接复制注册只会拍到「按下没拖起来」。正确做法是每档各跑一次 `MW_TRACE`，
    从 `scene_node` 的落定矩形取源/目标中心再写脚本：**四张基线 = 四次量坐标**，不是一次注册。
16. ~~**M9 整图折叠与聚焦模式下的圆点证据**~~ ✅ 2026-11-01 第 67 轮完成：两张新基线
    `mm-collapse-all-dots` / `mm-focus-dots` 进门（stage-b 153 → **155 场 / 310 次比较**，两遍零差异）。
    结论：圆点只画在**有父级且有下级**的节点上——整图折叠后 5 节点 / 3 颗圆点（根与叶子都不画），
    聚焦视图内 4 节点 / 1 颗圆点（聚焦根不画、视图外无残点）。顺带修掉一处**证据的 bug**：
    `scene_toggle` 轨迹清单把根也列进去，与绘制/命中的判据不一致（改动在 `MW_TRACE` 守卫内，不碰像素）。
    实机手感那条缺口不在本轮口径，M9 行原样保留。证据 `shots/x100/M9b/`。
17. **20k 端到端 `Arc<str>` 化**（M12 残账：P7 口径已达标，端到端 16.7 / 18.4ms 仍超线；
    `Scene::ids` / `Scene::index` 改 `Arc` 预计省 3–4ms，动 ~150 处，属大改，需单独排一轮）。


- 2026-11-01 00:25 · **第 54 轮 · C19 概念图检查器属性（概念元信息、钉住开关与局部整理入口）**。`docs/30` §4.2 / §5.5
  规定「概念图与思维导图共用同一检查器，但内容随对象类型切换；提供局部排布与固定控制」。此前概念图在检查器仍残留树语义
  （对象行显示中心主题/一级主题、样式页带「含子树」作用域），且缺少钉住开关。
  改造对象行：单选概念识别为「概念」，已钉住前置「已钉住」，追加「N 条命题」、「已挂卡片」、「N 个标签」，屏蔽子主题计数；
  多选识别为「N 个概念」与钉住统计。样式页分组标题隐藏「含子树」切换并隐藏「文字样式应用到子树」；
  新增「概念排布」分组（键 `concept/layout`，默认展开，活摘要「已钉住 / 自动重排」），内嵌 28px `switch_row`「钉住位置」
  轨道开关与辅助说明（单选 `SetConceptPinned`，多选 `Batch` 一次撤销），以及「整理选中概念」/「整理全部布局」快捷操作按钮。
  新增 5 条单测（`mw-core` 1 条 + `mw-shell` 4 条），测试 1109 → **1114**；
  新基线 `cm-inspector`（`DDEDFEFA1445`）与 `cm-inspector-pinned`（`B19304716E8F`），
  两图差 **4 659 px**（画布钉住徽标 1,632 px、检查器元信息/摘要/开关 3,027 px）；
  修复 `document.rs` line 1362 clippy 告警，基线 36 → **35** 条；
  stage-cm **62/124**（2026-11-01）、stage-b **140/280**（2026-11-01）两遍全过；
  基线目录 203 → **205**、输入脚本仍 135。证据 `shots/x100/C19/`。
  **未做**：钉住状态不进 `.zmind` 持久化（内存交互态）；实机高 DPI/手感未核。

- 2026-10-31 23:25 · **第 53 轮 · C18 概念图「钉住」概念与「只整理选中」局部布局**。`docs/30` §5.5 规定
  「整理布局支持全局或局部整理，允许用户钉住某些概念位置」。此前只有全局整张整理，
  所有概念强制覆盖为 DAG 自动位置，用户精心安排的局部空间结构全被冲掉。
  新增 `Command::SetConceptPinned { id, pinned }`（仅概念图、一次撤销、树节点拒绝）与纯函数
  `concept_layout::arrange_scoped(input, fixed)`：固定概念保持 `current[i]` 严格不动，
  但仍参与其他可动概念的拓扑分层与重心吸附计算。壳层右键菜单补单选概念「钉住位置 / 取消钉住位置」、
  多选概念「整理选中概念」、「整理全部布局」、「钉住/取消钉住位置」；全局整理时保护钉住节点，
  局部整理时未选中及已钉住节点统一保护；落地弹 toast 提示整理颗数并支持一次 Ctrl+Z 复原。
  覆盖层（不改 `mw_render::Frame` ABI）绘制钉住徽标（节点右上角内嵌 5.5px 强调色圆点徽章）。
  新增 6 条单测（`concept_layout` 4 条 + `document` 2 条），测试 1103 → **1109**；
  新基线 `cm-pin`（`F5BB4AA8DBC8`，117px 徽标）与 `cm-arrange-pinned`（`8C2D3960524A`，
  技术方案钉在 (280,40) 保持不动，其余概念重新拓扑重排，与 `cm-arrange` 差 42 569 px）；
  既有右键菜单场景 `cm-multi-menu` 与 `cm-submenu-bottom` 有意重拍；修复 `scripts/x100-stage-cm-gate.ps1`
  脚本 warmup 与 UTF-8 BOM；stage-cm **60/120**（`23:16–23:20`）、stage-b **140/280**（`23:20–23:30`）
  两遍全过；基线目录 201 → **203**、输入脚本仍 135。证据 `shots/x100/C18/`。
  **未做**：钉住状态不进 `.zmind` 持久化（内存交互态）；检查器未设钉住开关；实机键鼠手感未核。


- 2026-10-31 11:57 · **第 48 轮 · M25 大纲行「更多」与行右键菜单**。`docs/06` §6 从外壳 v2 起
  就记着「大纲行的「…」仍是占位」：大纲键盘路径齐全，指针用户却没有行级入口。行尾补
  **固定 24px 动作道**（16px `⋯` 距列右 8，悬停/选中出现；编辑与浮层时隐身但占位不收回），
  备注/卡片圆点左移让位，测量与绘制同减 `MORE_LANE`；`⋯` 与行右键共用一张菜单：单行 12 项、
  多行 5 项（目标在多选里才走批量），画布专属项不收；菜单键盘复用 `menu` 引擎并进
  `overlay_state().context`。概念图大纲没有树级语义，本轮不出 `⋯`、不弹菜单，只落
  `outline_row_menu_skipped reason=concept_map`。**证据阶段抓到两处骗人的快捷键标签**——
  `新建子主题` 抄了画布的 `Tab`（大纲的 `Tab` 是缩进）、`折叠` 写 `/`（大纲是 `Alt+.`）——
  改成留空与 `Alt+.` 并重拍 `outline-row-menu`。新基线三张、`outline*` 九场有意重拍
  （角标 128–272 px；`outline-wrap` 换行 37 166 / 47 089 px）；测试 1069 → **1071**；
  stage-b **134/268**、stage-cm **58/116** 两遍全过。证据 `shots/x100/M25/`。
- 2026-10-31 09:25 · **第 45 轮 · M22 聚焦视图外的搜索跳转**。M20 收尾时留下一条
  「聚焦模式仍不可达，只 toast 不替用户退出」；对着 `docs/02` §4 与 §5 复核，
  点搜索结果本身就是显式动作，停在「这个主题在当前视图之外」等于让检索在聚焦态下失效。
  改前轨迹（`MW_FOCUS_DEMO=中文排版` + 搜 `wgpu` + Enter）：`search_hits q=wgpu rows=1` →
  `search_jump … ok=0` → 红 toast，画布不动、focus 层级仍在。
  新增纯函数 `canvas::focus_exit_levels(doc, focus, id)`：无聚焦 / 目标可见 /
  目标不在文档 → `None`，否则返回要退的层数；`select_node_keep_focus` 在
  `scene.find` 失败时先按它 `focus_to(0)`（视图态，不进撤销栈），轨迹
  `search_focus_exit id=… levels=…`，再走既有 `reveal_commands` 展开 + 选中 + 居中。
  `SA::Jump` 对比跳转前后的 `focus_path()`，退出聚焦时 toast「已退出聚焦并跳转」；
  真正不可达（节点不存在等）仍是红 toast。新基线 `search-focus-jump`（Warmup 48，
  sha256-12 `D271AB2E4183`）；改前→改后 **189 421 px / 8.2213%**，
  搜索卡片区域 **0 px**、面包屑消失 5 156 px、画布区约 172 000 px、提示条 11 390 px。
  1 条单测；测试 1065 → **1066**；stage-b 126/252 → **127/254**（`09:14:32–09:21:18`）、
  stage-cm **58/116**（`09:09:47–09:14:25`）两遍全过、既有场景零漂移；
  fmt 干净、clippy 基线 36 条新增 0；基线目录 187 → **188**、输入脚本 124 → **125**。
  证据 `shots/x100/M22/`。**未做**：聚焦层级不随撤销恢复（视图态本就不入栈）；
  实机 IME/键盘未核。
- 2026-10-31 09:05 · **第 44 轮 · M21 `Ctrl+H` 替换（逐个 + 预览）**。`docs/02` §4 要求
  「标题、逐个/全部、预览」，改前只有全部替换、无预览、展开替换行焦点仍在搜索框，
  备注/链接/拼音命中混在结果里却没有可替换的位置（点「全部替换」静默少算）。
  新增 `mw_core::search::replace_char_range`（按字符区间换一处，挡非法区间）；
  结果行对可替换的标题字面命中显示 `→ 新标题` 预览，备注/链接/拼音行不给替换入口；
  `Ctrl+Enter` 替换当前、`Ctrl+Shift+Enter` 全部替换（原有按钮保留），打开替换行即聚焦替换框。
  逐个替换后按 **id + 原下标** 把当前行往下推：被替换行若仍命中（同一节点第二处）就前进一行，
  到底绕回，不再命中则按原下标夹取（`settle_after_refresh`）；每次替换一条 `SetText` = 一步撤销，
  全部替换仍是一条 `Command::Batch`。替换后仅当节点排得出来才选中 + 居中（`node_visible` +
  `select_node_keep_focus`），**不抢键盘**（改前这里会 `focus canvas`）。轨迹
  `search_replace_one id=… chars=4 before_chars=4` + `toast 已替换 1 处`。
  新基线 `search-replace`（sha256-12 `8A6F28044B44`）/ `search-replace-one`（`7BCAAB250C9F`；
  改前→预览差 **3964 px**（0.1720%，只在搜索卡片），两新基线互差 **78 247 px**（3.3961%——
  结果清空 + 标题 + 相机 + toast））。顺带修 `card-autosave` 金样按 UTC 跨日腐烂：
  自动保存日期改读 `MW_CARD_NOW_MS`（`1792800000000`）定钟，缺省仍是墙钟。
  7 条单测（core 2 + panel 5）；测试 1058 → **1065**；stage-b 124/248 → **126/252**
  （`08:48:29–08:55:25`）、stage-cm **58/116**（`08:55:46–08:58:52`）两遍全过、
  既有场景零漂移；fmt 干净、clippy 基线 36 条新增 0；基线目录 185 → **187**、
  输入脚本 122 → **124**。证据 `shots/x100/M21/`。
  **未做**：备注/链接/拼音行仍不可替换（只做标题字面）；无正则/整词/大小写选项；
  实机 IME/键盘未核。
- 2026-10-31 06:35 · **第 43 轮 · M20 搜索跳进折叠子树**。`docs/02` §4 要求结果能定位，
  但检索遍历文档前序、折叠子树里的节点照样进结果；画布场景只排可见节点
  （`layout.rs` 在 `n.collapsed` 时跳过子树），`select_node_keep_focus` 在 `scene.find`
  返回 `None` 时直接 return（改前轨迹只有 `search_hits q=禁则换行 rows=1`，随后没有任何跳转事件）。
  新增 `Document::collapsed_ancestors`（近 → 远、不含自身、`seen` 挡环）与壳层
  `reveal_commands`（按根 → 近摊成 `SetCollapsed { collapsed: false }`）；跳转先应用一条
  `Command::Batch`（一次撤销全收回）再选中 + 相机居中 + glow；轨迹
  `search_reveal … expanded=1` / `search_jump … ok=1`，展开后仍不可达时 toast
  「这个主题在当前视图之外」而不替用户退出聚焦。夹具 `MW_COLLAPSE_DEMO=中文排版`；
  新基线 `search-collapsed`（sha256-12 `37AF90C2C865`；改前→改后 **252 082 px**——
  画布整体平移 + 子树展开 + 选中环，搜索卡片 `x600..1320 y45..225` **0 px**、
  标题栏 132 px 是撤销按钮变亮）。
  6 条单测；测试 1052 → **1058**；stage-b 123/246 → **124/248**（`07:14:21–07:20:56`）、
  stage-cm **58/116**（`07:07:48–07:12:31`）两遍全过、既有场景零漂移；fmt 干净、
  clippy 基线 36 条新增 0（既有告警行号 1313 → 1337）；基线目录 184 → **185**、
  输入脚本 121 → **122**。证据 `shots/x100/M20/`。
  **未做**：聚焦模式仍不可达（只 toast）；跳转展开会入撤销栈；实机 IME/键盘未核。
- 2026-10-31 05:05 · **第 42 轮 · C17 概念图命题短语进全局检索**。`docs/30` §5.1/§5.4 把关系线
  定义成「有向且带链接短语的命题」，但 `Ctrl+F` 只走节点三字段，搜「支撑」一条都不出
  （改前轨迹 `search_hits q=支撑 rows=0 note=0 link=0`）。新增 `RelationshipHit`（from/id/to/
  score/kind/range/excerpt）与 `SearchIndex::relationship_hits`：逐节点按文档前序、边按节点内
  下标扫，只检索非空白短语；原文 560 / 全拼 450 / 首字母 350，位置罚与 prefix/exact 加成沿用；
  第四张拼音缓存 `rel_cache`（键 `(from,id)`，短语快照失效，240 字封顶）。壳层新增
  `Group::Relationship`「命题」（节点 < 命题 < 卡片 < 标签 < 路径），结果行显示 ±24 字摘录 +
  「概念 · 源 → 目标」，角标「命题」；点击/Enter 走 `select_relationship_keep_focus`：
  选中该边、相机居中到两端中点、**不抢键盘**（轨迹焦点仍是 search-input）。
  新基线 `search-rel` / `search-rel-jump`（改前 → `search-rel` 差 **27 216 px**，bbox
  `x665..1256 y80..221` 只在搜索卡片；两新基线互差 **71 547 px**，全在画布选中环与相机）。
  11 条单测；测试 1041 → **1052**；stage-cm 56/112 → **58/116**（`04:31:44–04:36:32`）、
  stage-b **123/246**（`04:36:37–04:43:01`）两遍全过、既有场景零漂移；fmt 干净、
  clippy 基线 36 条新增 0；基线目录 182 → **184**、输入脚本 119 → **121**。
  证据 `shots/x100/C17/`。
  **未做**：命题短语不参与 `Ctrl+H`；不索引颜色/箭头/概要；每帧仍扫全部边（只缓存拼音键）；
  20k 概念图 × 大量命题的首查延迟未实测。
- 2026-10-31 03:31 · **第 41 轮 · M19 搜索命中备注/链接**。`docs/02` §4 的 P0 口径是「标题 + 备注 +
  链接」，实现只索引标题；`SearchIndex::hits` 只读 `node.text`，备注里的 `html` 搜不到。扩三字段
  原文/全拼/首字母（标题 1000/700/500、备注 650/520/400、链接 600/480/380，位序罚 6，同节点取
  最高一条）；`SearchHit` 加 `field` 与 `excerpt`，非标题命中给 ±24 字单行摘录（换行 1:1 压空格、
  拼音命中折回原文区间、两端 `…` 只在截断时出现）；结果行第二行前置「备注 / 链接」角标。
  备注/链接的拼音索引只建到 240 字（`FIELD_PINYIN_CAP`），更长的仍可原文命中；缓存按字段拆分并
  随文本快照失效。金样旋钮 `MW_SEARCH_DEMO` 播种固定备注/链接、`MW_NOTES=none` 关本机笔记库
  （否则卡片正文的 html 会多一行）。11 条单测；新基线 `search-note` / `search-link`（`--tol 0`
  双跑，差 **29 422 / 27 349 px**，bbox `x515..1106 y80..221` 只在搜索卡片内）。
  测试 1030 → **1041**；stage-b 121/242 → **123/246**（`03:17:23–03:26:07`）、stage-cm **56/112**
  （`03:26:19–03:29:39`）两遍全过、既有场景零漂移；fmt 干净、clippy 基线 36 条新增 0；
  基线目录 180 → **182**、输入脚本 117 → **119**。证据 `shots/x100/M19/`。
  **未做**：`Ctrl+H` 仍只替换标题（P2）；关系线短语/标签/卡片正文不归节点检索——命题短语另立 C17
  （第 42 轮）；摘录固定单行；20k 长备注的原文扫描延迟未实测。
- 2026-10-31 02:56 · **第 40 轮 · C16 命题短语的空白提交口径**（先复现后改）。查 `docs/30` §5.4/§5.6
  与模型守卫时发现两处**静默失败**：① 既有命题被清空——画布编辑器把空串折成 `label: None` 发命令，
  `SetRelationshipLabel` 拒绝，但编辑器已经关掉，没有 toast、没有轨迹，用户只看到「回车没反应」；
  ② 新建连线按 `Enter` 留空——`blank_new` 只在「失焦且为空」时撤线，回车走提交分支，被拒绝后**线还留着**，
  真的落一条 `w=0` 的线团（改前截图为证）。大纲 `SetVia` 同样给空串，同样被拒、同样无声。
  改法：提交判定抽成纯函数 `phrase_commit(concept_map, text, opening, blank_new)` 四态
  （`Unchanged` 不发命令 / `RejectEmpty` 保留原短语 + 提示 / `DropEdge` 撤掉刚建的线 + 提示 /
  `Set` 正常提交），画布新增一次性 `notice` 通道交给壳层 toast；大纲 `OA::SetVia` 走同一口径并打
  `outline_concept_via_reject`。**金样顺带抓到第三个真 bug**：短语编辑框打开时 `editing_active`
  只认行文本编辑，`Ctrl+A` 同时被大纲列表吃掉、6 行全选中（新基线初版与 `cm-outline` 差 129480 px）；
  `editing_active` 现在也认 `via_edit`，同一场景差降到 34416 = 23768（键盘焦点行）+ 10648（提示条）。
  新增单测 `blank_phrase_commit_follows_the_concept_map_contract`（九条）；三条轨迹
  （`rel-label-reject reason=empty/blank_new`、`outline_concept_via_reject`、`toast`）；
  三张基线 `cm-phrase-clear` / `cm-phrase-blank` / `cm-outline-via-clear`（`--tol 0` 双跑）。
  像素账：`cm-phrase-blank` 与 `cm-default` **画布 0 px**、只多提示条 12695 px（线团消失）；
  `cm-phrase-clear` 与 Esc 对照只差提示条 10760 px，bbox `x16..375 y1148..1177`。
  测试 1029 → **1030**；stage-cm 53/106 → **56/112**（`02:44:54–02:48:15`）、stage-b **121/242**
  （`02:49:47–02:56:11`）两遍全过、既有场景零漂移；fmt 干净、clippy 子集无新增；
  基线目录 177 → **180**、输入脚本 114 → **117**。证据 `shots/x100/C16/`。
  **未做**：实机微软拼音（M4 同缺口）、撤销会把撤掉的线团恢复回来（模型无全局不变量）。
- 2026-10-31（第 39 轮 · C15 平行命题短语避让与独立点选）：`HANDOFF.md` §7.5⑤ 记着「平行命题在画布上的
  独立点选未做」。查下去发现 C14（第 34 轮）的车道实现有两处真缺陷，先写两个会红的单测再修：
  ① **反向命题整条重合**——车道按源节点各分各的组，`A→B` 与 `B→A` 各自成「组内一条」，都走零偏移直线，
     修复前两条 `label_at` 距离 **0.00**；改为按无向对跨 `doc_order()` 一次收齐。
  ② **竖边短语叠字**——法向扇开只解决线重合，竖边的法向是水平的，两个 36 宽的短语中心只差 24，
     横向重叠 **12 世界单位**；新增 `CONCEPT_LABEL_STAGGER = 24`，沿规范方向切线错开。
  顺带堵住 `bow_crossings` 会把车道 Quad 按弦线重弯、抹掉偏移的隐患（第三条单测）。
  `mw-render` 补 `parallel_propositions_are_individually_hittable`（两条车道中点各命中自己）。
  新增 `scene_rel` 轨迹（每条关系线的 `start/end/mid/label/w` 屏幕坐标）与 `MW_CONCEPT_PARALLEL=1`
  验收钩子；两场金样 `cm-parallel-select-a/-b` 分别点两条车道的短语，轨迹给出两个不同的关系 id
  （A → idx=3、B → idx=4），两基线差 **2 528 px**、bbox `x500..549 y409..765` 只在车道竖带。
  `doc_order()` 只取一次（常数优化，输出逐像素不变）。测试 1025 → **1029**；
  `stage-cm` 51/102 → **53/106**，两遍全绿、既有 51 张零漂移；`stage-b` 同轮复跑全绿。
  证据 `shots/x100/C15/`。**未做**：实机鼠标手感、三条以上车道的短语越界、端点汇合区的点选歧义。
- 2026-10-31（第 38 轮 · M18 `Ctrl+X` 删除确认）：`HANDOFF.md` §7.5③ 记着「`Ctrl+X` 多节点不弹删除确认」。
  本轮**先验证后改**：`cut_action()` 与 `apply_clipboard(ClipboardChord::Cut)` 最后一行都是
  `self.delete_selected()`，而 `delete_selected()` 在 `delete_confirm_plan` 返回 `Some(count)` 时
  只挂 `confirm_delete`、不删节点——所以旧结论不成立，产品行为本来就正确。
  新增三条 `MW_TRACE`（只写轨迹）：`delete-confirm pending=3 selected=1`、
  `delete-confirm cancel count=3 reason=esc`、`delete-confirm accept count=N`。
  两场金样：`mm-cut-confirm`（点「目标」→ `Ctrl+X`，确认框 + 全屏遮罩）与 `mm-cut-cancel`
  （同脚本 + `Esc`）。**取消后与「只选中、不剪切」对照逐像素 0 px**，证明剪切没有提前删节点；
  确认框在反而整屏不同（遮罩）。新增两场基线，`stage-b` 119 → **121** 场；
  证据 `shots/x100/M18/`（报告、轨迹、像素差、清单）。
- 2026-10-31（第 37 轮 · M17 悬停反馈）：`HANDOFF.md` §7.5② 记着「折叠圆点 / 边界 / 概要的 hover
  仍是『没有反馈』」。核对后确认这不只是缺反馈：`hit.rs` 早就能把指针解析成
  `Toggle/SummaryTopic/SummaryBrace/Boundary`，只是 `Canvas::show` 只把 `Toggle/Node/Label`
  映射进 `hovered`；`Frame` 只有单个 `hovered` 下标，装不下注解。HANDOFF 原判「要改渲染层 ABI」
  **不成立**——画布已有「合成纹理之上再画 egui 覆盖层」的成熟通道（跨度手柄、拖线预览、框选都走这条），
  悬停是交互态，本来就不该进导出路径。做法：新增 `canvas/hover.rs`（`HoverDetail` + 三张纯函数 +
  `paint_hover_detail`），圆点按 **1.3× 重绘整颗**（先铺画布色底盖住原圆点和穿过的线，再画环与 +/−，
  alpha=0 时与渲染层重合，所以淡入就是「长大」），概要把主题盒外扩 2px 描圈、花括号/连接线描 2px，
  边界圆角圈外扩 2px，统一 **55% 强调色、1.5px、120ms 淡入**；正在选中/编辑的注解不重复画。
  三条单测（七个 `Target` 全覆盖、1.3× 插值端点、屏幕半径 4.5 世界单位下限 8/8/9px）。
  新增三张基线 `mm-toggle-hover` / `mm-summary-hover` / `mm-boundary-hover`；与对照差
  **377 / 1 484 / 2 748 px**，bbox 只落在圆点/概要/边界本身。
  **顺带查出四张旧基线是错的**：`mm-collapse` / `mm-collapse-mid` / `mm-toggle-edge` / `mm-toggle-miss`
  的坐标 (623,473)/(623,482)/(623,485) 自第 19 轮「启动取景用落定视口」之后就点在空白处，
  两张「折叠」基线锁的一直是「没折」。按落定相机复标为 (522,429)/(522,438)/(522,441) 并重拍：
  旧 vs 新 108 025 / 108 179 / 108 025 / 0 px，中点帧与终态 62 619 px，边缘点击与圆心 0 px。
  测试 1022 → **1025**；`stage-b` 118/236 → **119/238**，`stage-cm` 49/98 → **51/102**；
  证据 `shots/x100/M17/`（报告、轨迹、像素差、清单）。
- 2026-10-31（第 36 轮 · 补齐八份 manifest）：`CM-12`/`SE-11`/`CM-13`/`M15`/`C14`/`SR-01`/`SR-02`
  的报告早在 2026-10-30 就写好了，清单一直缺（`HANDOFF.md` §7.5⑥）。本轮按各自 `report.md`
  回填七份，另补第 35 轮 `M16` 一份，结构照 `shots/x100/C9/manifest.json`。
  原则：`_note` 标明是补记、不新增结论；没有外观改动的包（M15/C14/SR-01/SR-02）`scenes` 为空
  并写明原因；报告里写作时未跑成的两条 `mw-shell` 导出单测在 `tests.total` 里如实标注。
  **不改代码、不重拍基线、不动门**。八份 JSON 均通过解析校验。
- 2026-10-31（第 35 轮 · M16 金样按帧确定性）：账本之外的一轮，但它是所有视觉证据的地基——
  `stage-b` 第 1 遍总在两场大纲场景上偶发 2.7–2.8% 差异（`outline-drag-illegal` /
  `outline-hover-select`），第 2 遍又全绿。根因两层：① 金样按固定帧数截帧，而大纲握把 120ms /
  拖拽展开 500ms 读 `Instant` 墙钟、egui 滚动落定读 `stable_dt`，帧耗时随负载变；② 离屏进程仍会收到
  宿主 `PointerGone` / 真实键入，足以抹掉悬停态或往标题里敲字。改法：`GoldenInput::advance_clock`
  帧计数虚拟时钟（`sleep` 冻结、到期补名义时长）、`RawInput::time` / `predicted_dt` 由壳层写、
  harness 下剥离全部宿主输入事件、大纲两处计时改 `ui.input(|i| i.time)`。三场风险场景逐字节不动；
  测试 1021 → **1022**；`stage-b` 连跑两次、四遍矩阵 **472 次比较零差异**，两道门当前全绿。
  证据 `shots/x100/M16/`。**未做**：没有「旧码红 → 新码绿」A/B（10 次复跑都没复现），
  失败图里标题「4|」只能归因、未在真机重放。
- 2026-10-30（第 34 轮 · 平行命题分车道）：C14 把「同一对概念之间允许多条命题」在模型层放开后，
  画布上两条命题**仍画成同一条直线**——用户看不出到底有几条，第二条也点不中（多对多关系是概念图的
  基本能力，`docs/30` §5.4 明写）。修在 `scene.rs`：先按**无向对** `(min,max)` 分组收集同一对之间的命题，
  再按组内序号做法向偏移 `gap = min(CONCEPT_LANE_GAP 24, len/6)`，用二次贝塞尔 `bowed_straight()` 弓出去；
  单条命题走零偏移分支，直接 early-return `straight_between()`，所以**既有单命题基线与 `--tol 0` 逐字节一致**。
  一个必须记住的坑：法向要在**规范方向**（小索引 → 大索引）里取，否则 `A→B` 与 `B→A` 各自把自己的法向当正的，
  偏移正好相消又叠回一条线。三条单测钉住：平行命题锚点分离 ≥8、必须走曲线、单命题仍是单段直线。
  证据见本文件 §2 概念图行与 `docs/STATUS.md` §1（golden 重拍 71 张、整体 0.494% 像素）。
- 2026-10-30（第 33 轮 · CM-13 关系线箭头改为屏幕恒定）：概念图四档箭头语义是「命题朝哪边」的唯一线索，
  但三角形是 `Scene` 里写死的 6×4 世界单位 —— 30% 缩放下只剩 **1.8 css px**，四档在缩略视图里全看不出差别；
  更刺眼的是同一画布上两套口径：树连线箭头一直 `css_to_world(7.0)` 屏幕恒定，只有关系线跟着缩小。
  修在渲染层（`Scene` 必须与缩放无关，画布/golden/PNG-SVG-PDF 导出共用）：`screen_arrow()` 以尖端为锚点
  等比放大到 ≥7 css px，**只放大不缩小**（`k >= 1`，高缩放下不变钝）；`relationship_bounds` 同步用放大后的
  三角形，否则会误裁出「箭头忽然不见」。证据 `shots/x100/CM-13/`。
- 2026-10-30（第 32 轮 · SE-11 对比度达标 + 令牌归一）：**「现代专业」必须能量出来**。量出三处不达标：
  ① `ui.dim`（面板里真当正文用的次要文字）在 `panel2`/`hover` 上只有 4.31–4.36:1，摸到 AA 线下方
  → 逐主题挪 3–5 个色阶（snow `#67706c→#646d6a` 4.34→4.53、mist `4.36→4.56`、sand `4.31→4.51`，ink 不变）；
  ② `faint` 在三种浅底上只有 2.64–2.82:1，连非正文 3:1 都不到 → alpha 0.48→0.56（暗色 0.50→0.58，
  守住「暗≥亮」不变式），四底最差 3.20–3.50，且仍**严格淡于 dim**；
  ③ 令牌归一两处：字号阶梯第四档从裸写 `14.0` 收成 `design::text::CANVAS_LEAF`、焦点环
  `component::FOCUS_RING/OFFSET`（1.4/1.5）直接引用 `content::focus`（两套常量差 0.5px），
  并删掉 `layout::PAD_X/PAD_Y`（18/9，全仓**零引用**却写得像在驱动几何，真出口是 `design::card_padding`）。
  新增 `tests/contrast.rs` 两条硬门：`secondary_text_clears_aa_on_every_surface`（逐主题×四底 ≥4.5）、
  `faint_text_stays_visible_but_never_becomes_body_text`（≥3.0 且必须比 dim 淡）。contrast 7 条全绿。
  证据 `shots/x100/SE-11/`。**已知差异**：四家分支色板本来就不等长（snow 8、ink/mist/sand 各 6），
  取色按 `palette_at(i % len)` 确定性回卷；把 snow 压到 6 会改变第 7/8 分支配色（可见回归），
  给三家补色自动挑出来的是灰/品红，本机看不了截图无法自证，故不改色板、只把合同写实。
- 2026-10-30（第 31 轮 · CM-12 概念节点改为椭圆）："概念图不能沿用思维导图结构"落到视觉层 ——
  思维导图是"盒子里挂子盒子"，概念图是"散落椭圆 + 有向连线"。**不动数据模型**（无新字段、无
  `FORMAT_VERSION` 变更、无迁移）：`conceptMap` 且无显式覆盖时形状默认解析成 `Shape::Ellipse`。
  五处一个真相源：`Shape::Ellipse` + `is_ellipse()` 收口判定、`mw_render::corner()`（改成 `pub`，
  **全仓唯一出口**）返回哨兵 `ELLIPSE_CODE = -1000`、WGSL 加 `sd_ellipse` 有向距离场、`hit` 用椭圆方程
  而不是外接矩形、SVG 输出原生 `<ellipse>`、PDF 用 `k=0.5522848` 四段贝塞尔、编辑态强调环改画
  `egui::Shape::Ellipse`（圆角矩形环的四角会露出来）。**为什么用负圆角哨兵**：`RectInst` 68 字节、Rust 对齐 4，
  WGSL 按 `roundUp(16,68)=80` 算 stride，加一个 `f32` 会让两边对不上。
  盒子外扩但**文字盒不动**（宽 `+min(0.22w,24)`、高 `+min(0.16h,8)`），所以换行点/省略号阈值/tooltip
  逐字节不变；界证明：96×36 内接 → 画 117.1×41.8，240×52 → 画 264×60，最宽两行文字仍在内接矩形里。
  `concept_boxes_follow_the_node_contract` 重写成椭圆契约，并在**同图内**把一颗概念显式改回 `rounded`
  验它不外扩 —— 同时钉住 `BoxKey` 必须比形状，否则第二颗会命中第一颗的缓存尺寸。证据 `shots/x100/CM-12/`。
- 2026-10-30（第 30 轮 · CM-11 概念图复制语义 + 选中底色）：四条旧路径（复制/粘贴、`Ctrl+D`、
  `Ctrl+拖拽`）都走树语义，粘贴会造出 `parent->child`。新增 `Command::InsertConcepts`：
  副本一律独立浮动概念、内部命题重映射、整批一次撤销；卡片 1:1 冲突拒绝。共享渲染层补上
  §5.3 一直缺的「选中 8% / 悬停 4% 强调底」，26 个 shell 场景 + canvas + 3 张 render 基线重拍
  （max 通道差 ≤16、bbox 只在节点盒内）。测试 963 → **968**；stage-b 118/236、stage-cm 49/98
  双跑全过。证据 `shots/x100/CM-11/`。
- 2026-10-30（第 29 轮 · 检查器数值格可键入 + 方向键步进）：不属 M/C 两表，记在这里只为接上轮次。
  `docs/30` §4.4 要「每个可改属性都有键盘路径」，而数值格原先只有 `−`/`+` 两个按钮、中间那格是不可点的字
  （字号 14 → 48 要点 17 下）。本轮：点一下 / `Enter` / 直接敲数字都进编辑，`↑`/`↓` 与 `+`/`−` 等价，
  `Enter` 提交、`Esc` 取消、点别处按提交算；`parse_stepper_input()` 挡脏输入与越界。
  **两个只有拍图才看得见的问题**：`ui.put` 会把下一行顶下去（改 `ui.new_child`）；开编辑那一帧就自己提交（改 `was_fresh` + 金样 `pinned`）。
  新基线 `inspector-stepper-edit`（与 `inspector-shape` 差 372 px，只在编辑格内）；测试 959 → 963；
  stage-b 118 场 / 236 次比较、stage-cm 48 场 / 96 次比较。证据 `shots/x100/SE-06b/`。
- 2026-10-30（第 28 轮 · P 轨 SE-10 检查器证据包）：不属 M/C 两表，记在这里只为接上轮次。**不改功能**：
  `docs/30` §4 逐条回填「合同 → 落地 → 证据」；§4.2 必选能力 14 项 = **10 已实现 / 1 部分（形状缺椭圆）/
  3 延后（字重、对齐、圆角，都要动 `StyleOverride`，需单独批准）**；十张基线逐张写清钉住什么；
  v1→v2 漂移收敛到 0.03–0.26%。门清单 `shots/x100/SE-10/gate.md`（G-B3-2 五判词留 ☐）。
  **本包摊出两条下一轮候选**：§4.1「组内行间距 6」（现为全局 8）、`dim`@panel2 = 4.14:1。
  测试与门数与 SE-09 同（959 / 117·234 / 48·96）。证据 `shots/x100/SE-10/`。
- 2026-10-30（第 27 轮 · P 轨 SE-09 检查器键盘与无障碍）：不属 M/C 两表，记在这里只为接上轮次。
  `Action::Inspector` 出厂键 F9 → **`Ctrl+Shift+I`**（§4.4），F9 降为 `InspectorAlt`；
  色板补 **Enter 选择**与**数字键 1–9 直选**（只在色板握焦点时吃键）；**Esc 两级**（`esc_step`：
  控件 → 松焦点，latch → 回画布，都没焦点 → 不吃）；**28px 错误条**（`Inspector.error`，失败时条 + toast）。
  **对比度**：浅主题 `faint` 2.37:1 → 检查器文字一律换 `dim`（对 panel ≥4.5），`faint` 只留装饰；
  `dim`@panel2 = 4.14，立 4.0 下限并记账。新基线 `inspector-error`；十张检查器基线重拍。
  测试 956 → 959；stage-b 117 场 / 234 次比较、stage-cm 48 场 / 96 次比较。证据 `shots/x100/SE-09/`。
- 2026-10-30（第 26 轮 · P 轨 SE-08 作用域与批量编辑）：不属 M/C 两表，记在这里只为接上轮次。
  作用域按 `docs/30` §4.2 从「样式页顶上单独一行」搬进**分组标题右侧**，两段「本节点 / 含子树」
  （11.5/16、选中 8% 强调底 + 强调色文字）；有开关的组不再画右摘要；点开关不折叠分组。
  补 §4.3 的 **500ms 合并撤销**：手势键 = 排序去重后的 id 集（只有 SetStyle / 全 SetStyle 的 Batch 有），
  判据 `same_gesture()`，`Canvas::apply_command_coalesced` 落到 `Document::apply_coalesced`；
  非样式命令把窗口关掉。新增 `MW_SELECT_DEMO=multi` 与 `inspector-multiselect` 基线。
  样式页六张按预期重拍（`inspector` 59055 px / 2.848%、`-narrow` 56253 / 2.713%、`-shape` 43983 / 2.121%、
  `-ink` 63195 / 3.048%、`-mist` 62522 / 3.015%、`-sand` 62521 / 3.015%，包围盒全在面板内），
  `inspector-empty` / `inspector-content` 零漂移；测试 954 → 956；stage-b 116/232（2026-10-30 06:29，双跑）、stage-cm 48/96（2026-10-30 06:32，双跑）。
  证据 `shots/x100/SE-08/`。**未做**：作用域开关的键盘路径（SE-09）。
- 2026-10-30（第 25 轮 · P 轨 SE-07 检查器内容组 v2）：不属 M/C 两表，记在这里只为接上轮次。
  按 `docs/30` §4.1/§4.2/§4.5 把内容页做完：五组**默认展开**（旧值五组全 `false`）并各带一句活摘要；
  备注 `note_rows()` 3 行起、超过 8 行封顶 8 行 + 「展开 · 共 N 行」；标签从「标记」页搬到内容页，
  删掉 `corner_radius(9)` 胶囊底，只剩 **6px 色点 + 12.5 文字**；卡片补「来源」一行（四态文案）。
  **两处只有量得出来的回归**（机器门当时是绿的）：① 「添加」按钮自然宽 + `TextEdit::desired_width`
  不含控件自身 8+8 内距 → 整行比可用宽多 16，面板最小宽被顶到 **316.65 且每帧 +2**（→ 326.65）；
  改 `label_field_width()` 先算宽再排 + `add_sized([44, 22])`，轨迹 `inspector w=288.00`。
  ② `ui.horizontal` 按全局 `interact_size.y = 30` 要高度 → 标签行 30 高、行距 38；新增 `content_row()`
  用显式 `desired_size` 钉死 28，色点间距 38 → 36。两条都补了会红的单测——`the_label_row_fits_the_panel`
  的第一版拿 `ui.set_max_width` 量面板宽，被证明恒真（`set_max_width` 会夹住 `min_rect`），已删。
  新增基线 `inspector-content` + 验收旋钮 `MW_INSPECTOR_TAB` / `MW_CONTENT_DEMO`；测试 948 → 954；
  stage-b 115/230（03:27）、stage-cm 48/96（03:30） 双跑全过；`inspector` 等七张老基线 零漂移（`inspector` 等七张全 0 px，双跑即证）。
  证据 `shots/x100/SE-07/`。**未核**：标签框/备注框里的实机微软拼音。
- 2026-10-30（第 24 轮 · P 轨 SE-06 检查器样式组 v2）：不属 M/C 两表，记在这里只为接上轮次。按 `docs/30` §4.2 重做样式组：色板 **8 列 × 24px、列距 8**（288 = 8 列、240 = 7 列）、选中 **2px 强调环 + 1px 内白**、**第 9 格自定义虚线圆**（原地取色，删掉三处整行「自定义…」）、**无颜色斜杠格**、字号 12–96 与边宽 0–8 的 **24×72 stepper**、边框色板同一网格；每项一条 `SetStyle`。**两个真缺口**：① 色板只有 ←→，「雪」主题第二行（自定义 + 无颜色）键盘走不到 → 补 **↑↓ 跨行**；② 「形状与文字」默认折叠，本包交付的边框/边宽/形状/字号**三张老基线一张都拍不到** → `MW_INSPECTOR_GROUPS` 预置分组开合 + `WindowState::seed_inspector_group`（**只写内存不落盘**，新单测钉住）。顺手把 `stepper()` 的 8 参数收进 `StepperRow`（clippy 新增 0）。基线：`inspector` 29434 px / 1.419%（面板内，画布 0 px）、`inspector-narrow` 28147 px / 1.357%、`inspector-empty` 0 px、`cm-arrow-inspector` 0 px；新增 `inspector-shape` / `-ink` / `-mist` / `-sand` 四场。**既有事实**：`Theme.palette` 雪 8 色、墨夜/雾/沙各 6 色，「第 9 格」只在雪主题字面成立——统一要动主题令牌，记待决定。测试 944 → **948**；stage-b **114/228**（02:23）、stage-cm **48/96**（02:26）双跑全过。证据 `shots/x100/SE-06/`。
- 2026-10-30（第 23 轮 · P 轨 SE-05 检查器骨架 v2）：不属 M/C 两表，记在这里只为接上轮次。按 `docs/30` §4.1–§4.5 把检查器整列重排：面板 288（可拖 240–360、落盘记忆）、头行 40、对象行 44（多选「N 个节点」）、分段 30 无描边（8% 强调底 + 1.5px 底条，删掉旧 chip 描边胶囊）、分组 28（chevron + 右摘要 + 组间 8px hairline）、折叠记忆进 `window_state.inspector_groups`（标记组默认折叠）、组内左右 16 → 12、无选中时一句话 + 「选中第一个主题」。**修掉一个真回归**：「形状」四段写死 66×4 → 内容最小宽 292，面板被顶到 292 且旧回写把 293 记成用户选择（拖到 240 下次打开变 293）；改 `shape_seg_w(available)` 均分 + 只在 `dragged/drag_stopped` 回写。三张新基线 `inspector` / `inspector-narrow` / `inspector-empty`（`--tol 0` 双跑），`cm-arrow-inspector` 重拍（95242 px / 4.134%）。测试 939 → **944**；stage-b **110/220**、stage-cm **48/96** 双跑全过（2026-10-30 01:33 / 01:45）。证据 `shots/x100/SE-05/`。
- 2026-10-30（第 22 轮 · P 轨 SE-04 起始页证据包）：不属 M/C 两表，记在这里只为接上轮次。按 `docs/30` §3.1–§3.7 逐条审代码，唯一与合同相反的是「最近文件缺失」：旧行为启动时删行，改成留行 + 标题 secondary + 副行「文件已移动」+ 点它走打开失败态（`Recent::Entry.missing` 只进内存、`mark_missing` 只在载入/触摸时算）；新基线 `start-missing`（与 `start` 只差第二条行 1082 px，bbox `x562..627 y441..470`），点击轨迹 `open_start` → `open_fail from_recent=1`。单测换成 `missing_files_stay_in_the_list_and_get_marked`，测试仍 939；stage-b 门 **107/214**、stage-cm 门 **48/96** 双跑全过（2026-10-30 00:40 / 00:43）。G-B3-1 门清单 `shots/x100/SE-04/gate.md`（五判词留 ☐）。
- 2026-10-30（第 21 轮 · P 轨 SE-03 起始页键盘）：不属 M/C 两表，记在这里只为接上轮次。Tab 序归位（清除按钮「标题行占位 + 行画完后注册」，`ui_kit::text_button_at`）、`Ctrl+Shift+N` 新建概念图（判别排在 `New` 之前——egui 的 `matches_logically` 忽略多余 Shift，修之前真被 `Ctrl+N` 吃掉）、主卡焦点环改画卡内 2px；顺带把金样从「读本机 recent.json」改成确定性空列表 + `MW_START_DEMO` 播种（`start-empty`/`start-narrow` 此前会随本机列表漂）。测试 937 → 939，8 张起始页基线 `--update` 重拍后双跑零差异。证据 `shots/x100/SE-03/`。
- 2026-10-29（第 19 轮 · M13 启动取景）：账本第 11 条。`App::start_refit_frames`（60 帧预算）+
  `Canvas::fit_viewport`：启动阶段**只在视口变化时**重取景，首帧 `zoom=1.2369`（默认窗口 `1280×728`）
  改成落定后的 `1.5`（`1920×1128`）；`set_camera` 也记 `fit_viewport`，阅读模式/夹具的相机不被覆盖。
  证据 `shots/x100/M13/`：两次 `fit` 轨迹、新 `cm-default` 与手动 `Ctrl+1` **整图 0 像素差**。
  漂移 **60 个文件 / 58 个场景**（31 `cm-*`、27 `mm-*`、`shell-ppp1.00/2.00`），全是自动取景场景；
  `ci.ps1 -Update` + 两道门 `-Update` 重拍，两道门 100/200 与 48/96 全过，测试 935。
  **走的弯路如实记**：第一版「头 12 帧每帧重取」把「窗口第几帧落定」写进了结果，
  `cm-arrow-menu-click` / `cm-focus-question-hover` 两遍之间自己不同；改按变化重取后消失。
  另：`scripts/x100-stage-b-gate.ps1` 补 `-Update`（此前只有 stage-cm 有）；重拍时两次撞上
  `os error 1224`（基线文件被映射占用），两张没写成，但第二遍比对全过说明它们本来没漂。
- 2026-10-29（第 18 轮 · C7 的取景）：账本第 10 条问「撤销要不要连相机一起回退」。答案是要，但**不是回放旧相机**——
  整理可能跑在首帧 `fit` 之前，那一刻存下来的相机并不是用户看到的取景。做法：`Canvas` 记
  `arrange_undo_depth` / `arrange_redo_fit`，撤销踩到的正是整理那一条时 `rebuild()` 后 `request_fit()`，
  重做同样 `fit` 并重新挂栈深；取景按撤回来的位置**重算**。证据：`undo fit=1` 轨迹；整理+一次 `Ctrl+Z`
  与手动 `Ctrl+1` 的**画布行 100–1099 最大差 0**（全图 1828 px 只在顶栏缩放读数与提示条）；
  `cm-arrange-undo` 重拍、双跑 `--tol 0`。顺带把提示条改成 harness 里不按墙钟过期（否则基线与真实时钟赛跑）。
  测试仍 935（本轮无新单测，`arrange`/`undo` 要真实 `Canvas`）；两道门 100/200 与 48/96 全过。
  **新发现**：首帧 `fit` 用的是没落定的视口——同一包围盒首帧 `zoom=1.2369`、稳定视口 `1.5`，
  已记为本节 §3 第 11 条候选。
- 2026-10-29（第 17 轮 · 证据债）：`SE-01`、`CM-01/02/03/10` 一直缺 `manifest.json`。按各包报告原文回填五份
  （`_note` 标明补记、不新增结论），场景清单逐个核对过 `shots/golden/` 是否真的存在，状态沿用报告
  （`CM-02`/`CM-10` 仍是 `进行中`）。无代码改动、无门。
- 2026-10-29（第 16 轮 · M12e 第二步）：第 15 轮之后结构撤销只到 28.9ms，查出来是账被扔了——
  加节点让内容指纹（含结构）变了，当前代进不了候选，`put_memo` 又只留「没用上的候选」，
  「改之前」那份当场被丢。改法：候选只挑指纹对得上的，**对不上的留在手里**（没有别的可留时留最近一代）。
  20k 结构撤销 **28.9 → 15.8 / p95 17.1ms（P9 达标）**；文字撤销/重做与打字持平。新单测
  `a_structural_undo_reuses_the_previous_generation`。测试 934 → 935；两道门 100/200 与 48/96 全过、
  152 场零漂移。证据 `shots/x100/M12e/` §8。
- 2026-10-29（第 15 轮 · M12 的 P9 残账）：先纠正口径——应用的 `Ctrl+Z` 走 `build_scene_cached`，
  `perf_scene` 原来的撤销例程量的却是无缓存路径。查下去真正付全量的是**结构撤销**（加/删节点让可见序列变，
  当前代的账整份作废）。做法：`SceneCache` 留两代账（用掉的那代也退成上一代），撤销/重做前
  `prefer_previous()`；两代都可用时按「复用得更全」选。20k 文字撤销 43.1 → **15.7 / p95 17.5ms**、
  重做 15.3 / 17.0（P9 30ms 达标）；结构撤销 **28.9 / 31.1ms 差 1ms**。新单测在第 一版实现上直接红。
  测试 933 → 934；两道门 100/200 与 48/96 全过、152 场零漂移。证据 `shots/x100/M12e/`。
  **未做**：结构撤销时 `resolve`（8ms）仍整段重算；账本内存翻倍未封顶。

- 2026-10-29（第 14 轮 · §3 第 7 条收尾）：M12c 的账上 `index` 1.1–1.3ms 是纯浪费——`ids → 下标` 的哈希表，
  「只改一个字」时 `ids` 逐字相同。两处改动：`build_scene_cached` 没并进自由主题就沿用 `Layout::index`；
  collect 栈改持 `&Node`（不再为每个孩子克隆 id）。**P7 口径 p50 14.49 → 13.00、p95 16.07 → 13.90、
  max 16.67 → 15.55ms，首次达标**；端到端 18.3 → 16.7ms（p95 19.9 → 18.4）仍超 16，差额在账本克隆与命令。
  新单测做变异检查（无条件沿用会立刻红）。两道门 100/200 与 48/96 全过、152 场零漂移；测试 932 → 933。
  证据 `shots/x100/M12d/`。

- 2026-10-29（第 13 轮 · C7）：账本这一行写「提示与撤销动线只有单测」，实查两头都空——`concept_layout` 的纯算法有单测，
  但「整理发出的是不是一条 `Batch`、一次 `Ctrl+Z` 能不能整张回来」一条测试都没有，右键菜单落地后也没有任何提示。
  做法：① 抽 `arrange_batch(ids, current, placed) -> Option<(Command, usize)>`（没动的不出命令、多条合成一条 Batch、
  返回颗数），`arrange_concepts` 改 `Option<usize>`；② 菜单落地点补 toast「已整理 N 颗概念 · Ctrl+Z 撤销」，
  并给 `set_toast` 加 `toast bad= text=` 轨迹（提示条 2.4s 墙钟，拍不进 golden）；③ 新单测
  `arranging_three_concepts_is_one_undo` + 新基线 `cm-arrange-undo`（整理 → 一次 Ctrl+Z，`undo label=摆放概念 left=12`）。
  stage-cm 门 47 → 48 场 / 96 次比较，测试 931 → 932。证据 `shots/x100/C7/`。
  **未做**：撤销不还原相机；实机键鼠手感未核。

- 2026-10-29（第 12 轮 · C3）：账本这一行写的是「手感（阈值/吸附）没量」，量下去发现不是手感而是**行为缺陷**：
  `mw_ui::link::target_node` 要求严格矩形包含，指针离目标框 1–10px 就判落空，而落空走
  `RetargetLinkToNewConcept` 在落点新建概念——用户的意思是「改指到那颗」，结果多长一颗。
  新增 `target_node_snapped(..., snap)`（压中 > 近邻，近邻按到矩形距离，同距取面积小、再同取 `draw_order`），
  `target_node` 变成余量 0 的包装；余量 `LINK_SNAP_CSS = 12` css px，经 `css_to_world` 换算成世界单位，
  缩放后仍是 12 屏幕像素。两条鼠标路径（连线拖、改端点拖）都改；离屏夹具 `MW_CONCEPT_RELINK_DRAG`
  原来写死 `target: None`，现在走同一判定并把 `target=` 打进轨迹。三条新单测、A/B 同帧差 6991 px、
  新基线 `cm-relink-near`（stage-cm 门 46 → 47 场 / 94 次比较），测试 928 → 931。证据 `shots/x100/C3/`。
  **未做**：实机鼠标手感（12px 是 UI 惯例不是试出来的）、吸附的「越近越明显」中间态。

- 2026-10-29（第 11 轮 · §3 第 7 条后半）：账本要的是「只重排脏子树」。做法不是改布局算法，
  而是在 `SceneCache` 里留一份**上一次的账**：内容指纹（样式 + 备注/链接/卡片 + 图标/标签/图片 +
  层级序号，**不含文字**）、结构指纹（可见节点序列与深度）、每节点的盒子输入指纹（文字/图标/标签/图片）、
  盒子与组装好的 `SceneNode` 表。任何一项对不上就整份作废走全量，所以「算错」退化成「慢」。
  实测 20k 打字 `reused=19999/20000`，缓存路径 **29.0 → 18.6ms**、5k **4.7 → 3.5ms**，
  P7 口径 `[scene] total` **p50 15.5 / p95 16.9**——**p95 差 0.9ms 未达标**。
  **两条单测当场抓到真 bug**：第一版指纹没覆盖备注（加备注后画布不显示）、第二版盒子指纹漏了图标
  （改图标仍用旧盒子、宽度画错）。测试 925 → **928**，两道门 146 场零漂移（视觉逐字节一致）。
  证据 `shots/x100/M12c/`。**未做**：`collect`/`index` 的 `Arc` 化（再省 3–4ms 的唯一路）、
  概念图不吃这份账、实机连续打字帧率没量。

- 2026-10-29（第 10 轮 · §3 第 7 条前半）：账本只说「20k 打字 47ms」，先补账再动手。
  `MW_LAYOUT_LOG=1` 让 `compute_layout` 与 `build_scene_cached` 各打一组里程碑，20k 缓存路径一轮
  = `resolve 5.4–6.0 / layout 12–21（measure 9–14）/ index 1.0–1.5 / nodes 6.3–8.5 / order 1.0–1.4`，
  且**测量缓存 799,960 命中 / 0 未命中**——全命中也要 28ms，成本在「每节点每轮查表 + 装配」的骨架里。
  两笔白花当场砍掉：行数据 `Vec<String>` → `Arc<[String]>`（命中只加引用计数）、测量循环改借用不克隆文本；
  `resolve_all` 不再逐节点 `clone` 样式覆盖（带样式的文档每轮少克隆几万个 String，
  20k 全带样式 **10.5 → 7.4ms**）。20k 改字重建 **46.1 → 41.4ms**、缓存路径 **34.1 → 29.0ms**，
  5k 缓存路径 5.6 → **4.7ms**。**否决**：FxHasher 换三处 SipHash，交替 A/B 四轮
  38.9/37.9/39.1/39.8 对现行的 31.7/32.8/32.2/33.4——慢 6–7ms，撤回；散列不是瓶颈。
  新增单测 `a_cache_hit_shares_the_line_storage_instead_of_cloning_it`；测试 924 → **925**，
  两道门 100/200 与 46/92 全过、146 个场次零漂移（无视觉改动、无新基线）。证据 `shots/x100/M12b/`。
  **P7 16ms 仍未达标**：下一刀必须是真增量，不是再砍常数。

- 2026-10-29（第 9 轮 · M7）：账本说「框选阈值 4px 的手感未量」，可证据列里只有边界选择的图，
  **橡皮筋自己一张图都没有**。跑一遍发现更要紧的是反馈缺失：`marquee_pick` 只在松手那一刻调用，
  拉半个屏幕也不知道框住了什么。改成拖动中每帧算预览、并入 `Frame.selected` 实时挂选中环
  （轨迹 `marquee preview picked=… selected=…`，只在集合变化时写）。阈值也量了：2px 拖动零 marquee 事件、
  5px 才 `marquee picked=0 selected=0`，比较在 egui points 空间做——屏幕常量。Ctrl+框选 1 → 3 → 5，
  靠新动词 `mods` 让修饰键在按下前就位。新增单测 `marquee_pick_maps_the_screen_box_through_the_camera`
  与 `mods_sets_and_clears_the_modifier_state`；两张基线 `mm-marquee` / `mm-marquee-add`；
  stage-b 门 98 → **100 场 / 200 次比较**，测试 922 → **924**。证据 `shots/x100/M7/`。
  **踩坑**：第一版只改 `Frame.selected`、没改 `select_alpha` 的目标值，透明度停在 0，
  A/B 对比逐字节零差异才暴露；补进 `select_target` 才有 3975 px（0.227%）的差。

- 2026-10-29（第 8 轮 · M9）：账本这行写「折叠动画与点击热区」，两样都缺**壳层**证据——
  `blend_scenes` 的中点有场景层单测，但壳层到屏幕这一路没人拍过；热区只有「圆心命中」一条，
  把半径写成世界常量照样过。本轮先把结论量出来：`MW_MOTION_T` 钉 0.0 / 0.5 / 1.0 与终态比像素
  = **34374 / 27913 / 0**（终点逐字节一致，动画不缺中间帧也不会停半路）；圆点画 r8、命中 r10，
  点圆心下方 9px 仍折叠、12px `hit=none` 不误折叠。补 `toggle` 轨迹 + 单测
  `toggle_hit_ring_keeps_its_screen_size_across_zoom`（0.5×/1×/2×），新增四张基线
  `mm-collapse` / `mm-collapse-mid` / `mm-toggle-edge` / `mm-toggle-miss`；stage-b 门
  94 → **98 场 / 196 次比较**，测试 921 → **922**。证据 `shots/x100/M9/`。
  **本轮无产品行为改动**：这两件事本来就对，账本缺的是证据；热区半径不趁机动。

- 2026-10-29（第 7 轮 · M6）：账本这行原话是「连续按多次的节奏没人看过」。按脚本连按 `Alt+↓` 三下逐帧看，
  毛病不在节奏，在两处不一致：**层级命令不跟镜头**（折叠 / 方向键遍历 / `Tab` 新建 / 右键菜单都跟），
  镜头被拖走后按 `Alt+↓` 节点仍停在 `(-716,-428)` 屏幕外；**多选被忽略**，框两颗按 `Alt+↓` 只动主选，
  而大纲与右键菜单早走 `BatchOp` 批量命令。补 `reveal_id` 接上这三条命令、多选改走
  `move_step_many` / `indent_many` / `outdent_many`，并加两条轨迹 (`structure … path=single|batch`、
  `marquee picked=… selected=…`) 让「走了哪条路」可读。新增三张基线 `mm-move-keys` / `mm-move-multi` /
  `mm-move-keys-reveal`，stage-b 门 91 → **94 场 / 188 次比较**，既有 91 张零漂移；测试 920 → **921**。
  证据 `shots/x100/M6/`。**整组移动是一条撤销**（`undo label=移动节点` 一次复原）已验。
  **第一版基线不成立**：`mm-move-keys-reveal` 原来拖的距离不够，节点留在视口顶端仍可见，
  相机几乎没变——那张图改与不改一样；改成把节点整颗推出屏幕才锁住。

- 2026-10-29（第 6 轮 · M12）：账本这行写了两件事，先分开验：**「平移不重排」成立**（20k 文档里
  中键拖 + 滚轮缩放，`scene_build` 轨迹一次都没有），**「增量重排」从来没有过**（改一个字整棵重建）。
  顺带量到最要紧的一条：打字每敲一下就重建一次，5k 9.3ms（还在 60fps 里）、20k 47ms（约 21fps）。
  本轮只去掉白花的钱、不动架构：测量缓存的键从 `format!`（含三个 `{:?}`）换成结构体逐项比较；
  没有边界/概要/关系线的文档不再跑 `doc_order()`（两万个 id 的克隆，10.2 → 0.3ms）；
  布局结果整块搬移不克隆三份；`resolve_all` 预分配。5k 打字 13.7 → **7.7ms**，
  20k 改字 66.9 → **46.1ms**，冲突/撤销同步下降。新增 `scene_build nodes=… edges=… ms=…` 轨迹、
  `press middle x y` 输入原语（平移脚本）、perf 例程的「缓存路径」一项。测试 918 → **920**，
  两道门 91/182 与 46/92 全过，既有基线逐字节零漂移（本轮无视觉改动、无新基线）。
  **未达标**：20k 仍 47ms（P7 16ms），拆出账本 §3 第 7 条「20k 增量布局」。证据 `shots/x100/M12/`。
- 2026-10-29（第 5 轮 · M8）：账本原判只是「`Ctrl+Y` 与中文输入混排未验证」，跑脚本才发现真问题：
  建节点是 `AddNode`、写名字是 `SetText` 两条命令，**一次 `Ctrl+Z` 只撤掉字、树上留个空盒子**，
  连建三颗要按六次才干净。新增 `ChangeSet::created` + `Document::apply_coalesced`（判据由调用方给，
  返回 `false` 时与 `apply` 逐字节同行为），`commit_edit` 对 `Editor.fresh` 的节点改走合并；
  `undo`/`redo` 各加一行栈深轨迹。第一版合并整段覆盖上一条 `after`，重做只把节点塞回 `nodes`
  不挂回父节点（树上看得见空、看不见盒子），改为按 id 合并并补单测。新增四张基线
  `mm-undo-built/one/three`、`mm-redo-one`；stage-b 门 87 场 / 174 次 → **91 场 / 182 次**，
  既有 87 张零漂移。证据 `shots/x100/M8/`。
  **产品行为变化**：本次编辑里刚建的节点，其文字提交并入「新建节点」那条撤销——一次 `Ctrl+Z` 整颗收回。
- 2026-10-29（第 4 轮 · C10）：账本原判「`place_submenu` 只有两条单测、贴底没图」——单测那半过期了
  （现有四条，含竖向夹取两条），图那半是真的：所有菜单场景都在写死的 `(360,120)` 开，
  子菜单永远装得下。补 `MW_CONTEXT_POS='x,y'` 只挪请求点（不设时与原来逐字节一致），
  在 `1280x520` 的矮窗里让 300 高的「排列」子菜单必然溢出，新增基线 `cm-submenu-bottom`
  与一行 `menu_submenu_rect` 形状轨迹；stage-cm 门 45 → 46 场 / 92 次比较，既有 45 张零漂移。
  证据 `shots/x100/C10/`。
- 2026-10-29（第 3 轮 · C9）：账本原判是「只能拍稳态」，查下去是三层断链：① 离屏一律 `reduce_motion`，
  中间帧不存在；② `motion_span` 只看节点盒，**加一条命题线从来不起过渡**，`blend_scenes` 的
  `alpha = t` 分支永远走不到；③ 强调读目标场景的 `rel.alpha`，真在淡入时会画成满色。
  三处都补：`MW_MOTION_T` 定格（只给验收用）+ `motion_reduced` 只放开场景过渡一路；
  `motion_span` 认关系线 id 集合变化（跨度 0 ⇒ 0.20s）；共有项插值同时插 `alpha`；
  强调改读插值帧。新增 `cm-link-fade-mid` 一张基线与 `rel_emphasis` 轨迹；stage-cm 门
  44 场 / 88 次比较 → 45 场 / 90 次比较，既有 44 张基线零漂移。证据 `shots/x100/C9/`。
  **产品行为变化**：概念图新增/删除关系线现在有 0.20s 淡入淡出（时长令牌未动）。
- 2026-10-29（第 2 轮 · M1/M2）：编辑态 `Tab` 补成「提交 + 建子 + 接着编辑」（以前只把焦点漏给壳层），
  `Enter` 补成「提交 + 建同级」；`Esc` 取消时把这次编辑建出来的节点一起收回（新 `Editor.fresh` →
  `Command::DeleteNodes`）。概念图两键仍只提交，41 场概念图基线零漂移。新增
  `mm-new-child` / `mm-new-chain` / `mm-new-cancel` / `mm-enter-sibling` 四张基线与 `editor_open` 轨迹；
  stage-b 门 83 场 / 166 次比较 → 87 场 / 174 次比较。证据 `shots/x100/M1/`。
- 2026-10-29（第 1 轮 · M5）：新增 `mm-drag-child/before/after/illegal` 四张基线、
  四个输入脚本、三行 `MW_TRACE` 轨迹（`scene_node` / `drop_zone` / `canvas_click` 补坐标与命中）；
  stage-b 门 79 场 / 158 次比较 → 83 场 / 166 次比较。证据 `shots/x100/M5/`。
- 2026-10-29：创建。种子来自 `docs/02` §3/§4（P0/P1 节点编辑与导航）、`docs/30` §5（概念图）、
  `canvas/tests.rs` 的测试名、`shots/golden/` 现有场景。状态列只有 `稳 / 未验证 / 粗糙` 三档，
  没有证据的一律 `未验证`。

## 4. 卡片笔记（编辑体验）

> 2026-11-01 建立。用户口径：「只升级笔记的编辑体验，其他不管」。
> 这一节的场景用 `scripts/ce01-card-sweep.sh` 批量复核（34 场，与门同款
> `APPDATA`/`LOCALAPPDATA`/`TEMP` 隔离）。

| # | 操作 | 单测证据 | 视觉 / 端到端证据 | 状态 | 已知粗糙 |
| --- | --- | --- | --- | --- | --- |
| CE-01 | 光标 / 滚动条 / 列表 / Tab 与退格 / 标签误删 / 保存重试 / `Ctrl+/` / 标题 Enter / 多行缩进 / 字数缓存 / `Ctrl+Enter` 解绑 / **标题重复** / 标签退格（共 18 处） | `note_view` 新增 16 条 + `scroll_bar` 5 条（全量 665 passed） | `card-blank-caret`、`card-scroll`（新），卡片 34 场 `--tol 0` 两遍全绿，见 `shots/x100/CE-01/` | 已实现（机器；实机键鼠/IME 未核） | 2026-11-01 用户报「换行后光标找不到、右侧滚动条很奇怪」。查出 7 条：① 空行行高被压成块间距（连续空行更是 0），回车分段后光标成零高度的线；② egui 默认浮动滚动条休眠态完全不可见、还浮在版心右缘；③ 有序列表编号被 `marker_indent` 缩成 4px 深色墨点；④ 同一条 `marker_indent` 让光标进出行时正文横跳 9→24px；⑤ Tab 往正文写裸 `\t`（排版器不认，留影子字符）；⑥ 行尾空白被排版层吃掉（行尾敲空格光标不动）；⑦ 标记揭示用上一帧的光标。**未做**：整道 `stage-b` 未复跑（跑过一次卡住 4 小时无输出）；若干场景重拍前有 1–2px 整体纵向位移，逐条排查未归因，留人工门必看；`Ctrl+Shift+O` 目录浮层未做。**第二批 4 条**：⑧ 标签以前整枚都是删除按钮、且删除不进撤销栈（手滑就丢），现在只有 × 生效，命中区落在标签间隙里不占版面；⑨ 保存失败只有一句「保存失败」，补顶栏「重试」按钮（hover 显示原因）；⑩ 合同 §6.5 的 `Ctrl+/` 触发没做；⑪ 合同 §6.8「标题行 Enter 转到正文」没做（单行 TextEdit 不理 Enter）。**第三批**：⑫ 选中多行按 Tab 只缩进光标那一行还打散选区，改成逐行缩进、选区保住；⑬ 补 D4 的后账——`Tab` 推进一层后 `list_prefix` 认不出行首空格，回车不再续写、粘贴退化成纯文本。**第四批**：⑭ 字数每帧解析两遍 Markdown（按正文快照缓存）；⑮ `Ctrl+Enter` 绑成「保存并关闭」而合同 §6.8 明写「不绑定提交」，解绑。**第五批**：⑯ **标题重复**（合同 §6.10）——正文首行是 `# 标题` 时标题框下面又画一遍同一句，改成打开时摘掉那一行、保存时按原样拼回去（字节级可逆：含 CRLF / 整篇只有一行标题 / `# 标题党` 这类例外），`card-default` 等七张基线有意重拍；⑰ 标签输入框空着按退格退回上一个标签（合同 §6.6）。金样门 **155 场连跑两遍全过**（`scripts/golden-sweep.sh` 独立复核 + 正式脚本各一遍）。 |

## 5. 全局排版（打磨轮 65）

> 2026-11-01 建立。用户口径：「打磨所有页面的字体，字号，字间距等和文字相关产生的视觉效果，
> 目前的整个产品看着累，而且不够舒服，不够美观。」
> 方案快照：[`docs/x100/typography-plan-2026-11-01.md`](typography-plan-2026-11-01.md)。

| # | 操作 | 单测证据 | 视觉 / 端到端证据 | 状态 | 已知粗糙 |
| --- | --- | --- | --- | --- | --- |
| TYPO-01 | 全局字体栈、字阶、真字重、行高/几何、文字对比度、CJK 禁则回填 | `design` 字阶与几何、`contrast` 七条分表面 AA、`content_kit::font_for`、`chrome` 禁止 `.strong()` 扫描、`menu`/`search`/`inspector`/`outline`/`note_view` 行高；全量 **1202 passed / 0 failed / 1 ignored**（23 套件） | `stage-b` **153 场 / 306 次比较**、`stage-cm` **72 场 / 144 次比较**，`--tol 0` 两遍零差异（冻结二进制复刻门环境，见 `shots/x100/TYPO-01/report.md` §6 与 `gate/`）；`mw-render` 两张离屏基线 18:10 重拍 | 已实现（机器；人工目检/实机 IME 未核） | 此前的字体栈是「雅黑优先 + 12px 小字 + `.strong()` 假字重」；本包把 DWrite 改成 Segoe 优先后雅黑回退、外壳六档 12–26、内容十四档、画布 ±1px、标签改色点；`text_faint` 从 0.56/0.58 提到 0.68 才让雾青/暖砂 page/panel ≥4.5。**未做**：`typo-*` 专项场景、100/125/150/200% DPI 与微软拼音实机目检（M4 仍阻塞）；人工门未开 |

## 5. 概念图 v2（对标 CmapTools，设计合同 `docs/33`）

> 2026-11-02 建立。用户口径：「修复概念图的功能…概念图之间不能建立联系，概念图之间的
> 联系的文字编辑漂移…概念图居然能叠加在同一个位置…概念图应该有专属的自己的主题…
> 有自己的悬浮工具栏…有自己的检查器或者属性，专属的 UI。对标 cmaptools。」

| # | 操作 | 单测证据 | 视觉 / 端到端证据 | 状态 | 已知粗糙 |
| --- | --- | --- | --- | --- | --- |
| CM-V2a | 建线：短语留空不再撤线 + 锚点命中扫全图 | `blank_phrase_commit_follows_the_concept_map_contract`（+2 断言）、`nearest_anchor_finds_the_one_hanging_outside_the_box`、`nearest_anchor_picks_the_closest_and_gives_up_past_the_radius`（mw-ui，+2） | 金样 152/155（概念图场景零漂移） | 已实现（机器；实机键鼠/IME 未核） | (a) 短语留空以前整条线被撤（`docs/30` §5.9），现在落「未命名」占位，只有显式 Esc 撤线；(b) 锚点命中圆有一半在盒外，而壳层要求「先按在盒里」再找——按在锚点上被判成拖画布。`ANCHOR_HIT=7` css 未动 |
| CM-V2b | 短语编辑不漂 | `lane_offsets_do_not_move_when_one_label_is_retyped`、`three_lanes_on_a_horizontal_edge_pay_for_their_width`（改判等距对称带） | 金样 `cm-parallel-*` 零漂移（等宽车道下新旧逐位一致） | 已实现（机器） | 车道偏移改等距对称带（步长只认组内最宽那条），改一条短语不再挪动同组；编辑框每帧跟随 `label_at`、字体与渲染同源（13px）。**未做**：法向仍按长度收窄（`gap = min(len/6, 24)`），节点一挪短语仍会沿法向动 |
| CM-V2c | 概念落点防重叠 | `dropping_on_an_occupied_spot_slides_to_the_nearest_free_one`、`resolve_overlap_finds_the_exact_minimum_when_the_grid_hits_it`、`resolve_overlap_is_a_no_op_when_the_spot_is_free`、`resolve_overlap_is_deterministic`、`resolve_overlap_gives_up_and_returns_the_wanted_spot`、`clear_of_rejects_touching_and_squeezed_pairs`（+6） | — | 已实现（机器） | 同心环网格（16 方向 × 16 圈），拿到的是「附近」而非严格最近；绕不出去原样交回。**未接线到壳层**：落点/拖放还没调 `resolve_overlap`，只闪红框 |
| CM-V2d | 概念图专属主题（4 套） | `concept_themes_do_not_share_tokens_with_mindmap_themes`、`concept_themes_carry_their_own_shapes`、`concept_theme_falls_back_to_paper`（+3） | — | 已实现（机器） | `cm-paper`/`cm-slate`/`cm-mint`/`cm-clay`，有自己的色板、概念盒形状（椭圆/圆角矩形）与关系线粗细，与 `THEME_IDS` 零共享色；`apply_concept_defaults` 对 `cm-*` 不再覆盖形状/填色/描边。**未接线**：设置里还没有主题入口，命题胶囊/箭头色还没读主题 |

| CM-V2e | `resolve_overlap` 接到落点上 | `dropping_on_an_occupied_spot_*` 等 6 条（CM-V2c）+ 金样全绿 | 金样 155/155 连跑两遍 | 已实现（机器） | 单颗概念拖放/新建落点避让（`free_concept_spot`），既有盒不动；多选整组一起动不避让。**未做**：方向键微调（`nudge_concepts`）仍不避让 |
| CM-V2f | 概念图专属工具条（6 动作） | `every_icon_stays_inside_the_sixteen_unit_grid`、`every_icon_has_a_unique_trace_name`（`Icon::ALL` 37→43） | 6 张 `cm-*` 有意重拍（中央工具条换图标），金样 155/155 连跑两遍 | 已实现（机器） | 画布是概念图时中央工具条换成 新建概念/连接/改命题短语/整理布局/钉住位置/聚焦问题 + 适应画布，与思维导图那条互斥。「聚焦问题」打开检查器并把焦点送进那一格（写入口在检查器）。**未做**：快捷键（Tab/Ctrl+L/F2/Ctrl+P）还没接成全局键位；禁用态只灰显不弹提示 |

| CM-V2g | 概念图专属检查器（概念/命题/图 三段） | `concept_panel_picks_by_whats_selected`（+1）；旧测试 `concept_map_style_body_*` 按 §6 改判为 `concept_map_inspector_shows_the_concept_panel_not_tree_groups` | 金样 155 场第二遍全过（`cm-card-link-back` 有意重拍） | 已实现（机器） | 概念图不再进思维导图那三页（树语义连门都进不来）。**概念**：文字/形状/钉住/坐标；**命题**：短语/箭头四档/源→目标可点；**图**：聚焦问题/整理布局/计数。分组键 `concept/*`，折叠记忆不串味。**未做**：命题的颜色/线宽/虚线/备注（`SetRelationshipColor/Width/Dashed/Note` 命令都在，只差控件）；多选命题批量页仍走 CM-17 那套 |

| CM-V2h | 主题接线：`themes_for(doc)` 按文档类型分流 | `themes_are_listed_by_document_kind`（+1，含 `get_theme` 认 `cm-*`） | 金样 155/155 一遍全过（无重拍） | 已实现（机器） | 四套主题建好却一直没有入口——设置/菜单里看不到，等于没做。现在检查器「主题」组与顶栏「视图 › 主题」都按文档类型列，两套**不混排**；`get_theme` 认 `cm-*` 且不过 `legible`（再拧一遍色相会把「零共享色」拧掉）。**未做**：命令面板仍用 `themes()` |

| CM-V2i | 命题段补四组属性 + F2/Ctrl+P 接线 | 无新增单测（UI 控件与键位走人工门） | 金样 155/155 一遍全过（无重拍） | 已实现（机器） | 命题段补上 线型（实/虚）/ 线宽三档（1.0/1.5/2.5）/ 颜色（跟随主题 + 调色板前四色）/ 备注（多行，失焦落盘）。`F2` = 改文字（选中命题改短语、选中概念改概念文字）、`Ctrl+P` = 钉住——此前工具条提示与菜单都印着 F2，但**键没绑**，是空承诺。方向键微调判定为**不该避让**（精确移动，加了会「按一下跳一下」），已写进 `docs/33` §2 B3 附注 |

| CM-V2j | 卡片基线重同步 + 检查器「所属分组」+ 工具条禁用态提示 | 无新增单测（只读行与提示走人工门） | 36 张卡片页重拍（并行会话的整页配色改动）；金样 173/173 连跑两遍 | 已实现（机器） | 卡片页基线是并行改动留下的，**先看图判定渲染正确、diff 整页纯红=全局配色**才重拍，没盲拍。检查器补「分组 / 未分组」只读行。`Canvas::hint` 让「改短语/钉住/整理」在没选中时不再静默 |

**未做（`docs/33` §7 七项已全部收口）**：命令面板的主题列表仍是 `const` 静态表——`Item.label` 是 `&'static str`，要按文档类型分流得把整张表改活，收益不抵风险；工具条「连接」的禁用提示由 `begin_relationship_action` 自己给，其余三项已补。
