use super::*;

/// `MW_CARD_DEMO` 卡片的固定时间戳：金样会渲染「创建于/更新于」，取墙钟
/// 会在 UTC 跨日（本地 08:00）时让 `card-slash` 基线漂一天。
pub(crate) const DEMO_CARD_TS_MS: i64 = 1_792_800_000_000;

/// `MW_CONTENT_DEMO` 与 `MW_SEARCH_DEMO` 共用的备注/链接。
///
/// 搜索验收需要一条「标题不含、备注/链接含」的可预测文本：`html` 只在备注里，
/// `https` 只在链接里（样例文档没有同名标题）。
const DEMO_NOTE: &str = "纯 Rust 原生：没有 WebView，没有 HTML。\n\
                         wgpu 四层合成器，DirectWrite 排版。\n\
                         离屏导出与画布共用同一份 Scene。";
const DEMO_LINK: &str = "https://wgpu.rs";

/// 开始页的裸键（Esc / Enter / 1–9 / 0）什么时候归开始页。
/// 标题栏菜单是 egui Memory 弹层，快捷键表是 Area：都是前台浮层，
/// 它们的 Esc/方向键/Enter 不能被开始页抢走。
/// CER-03 起委托 [`crate::content_keys::page_owns_bare_keys`]（同源门）。
pub(crate) fn home_bare_keys_owned(menu_open: bool, help_open: bool) -> bool {
    crate::content_keys::page_owns_bare_keys(crate::content_keys::Foreground::menu(
        menu_open || help_open,
    ))
}

impl Shell {
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        harness: Option<Harness>,
        exit: Arc<AtomicI32>,
        mut perf: Perf,
    ) -> Self {
        perf.mark("shell_new");
        let reduce_motion = system_reduce_motion();
        let settings = settings::Store::load();
        let mut canvas = cc.wgpu_render_state.as_ref().map(Canvas::new);
        if let Some(c) = canvas.as_mut() {
            // harness 必须与时间无关：离屏截图期间任何过渡都会让基线抖动。
            c.set_reduce_motion(reduce_motion || settings.reduce_motion || harness.is_some());
            if harness.is_some() {
                // 输入脚本自带确定指针，可以安全地打开悬停态给工具条留实拍；
                // 无脚本时仍关掉，免得桌面光标混进基线。
                c.set_hover_enabled(std::env::var_os("MW_GOLDEN_INPUT").is_some());
            }
        }
        if harness.is_some() {
            // 滚动条有淡入/淡出动画：同一份内容两次跑可能一条在、一条不在。
            // 离屏基线里直接收成 0 宽，布局与截图都稳定。
            cc.egui_ctx.all_styles_mut(|s| {
                s.spacing.scroll.bar_width = 0.0;
                s.spacing.scroll.floating_width = 0.0;
                s.spacing.scroll.floating_allocated_width = 0.0;
            });
        }
        // 测试钩子 + 将来的文件关联入口：`MW_OPEN=<path>` 启动即打开一份磁盘文档。
        // 没有它就只能靠人点对话框，自动化验收无从谈起。
        let mut recovery = None;
        let mut opened = false;
        let demo_recents = std::env::var_os("MW_START_DEMO").is_some_and(|v| v != "0");
        // 金样必须确定：同步读；要拍骨架就用 `MW_RECENTS_PENDING_FRAMES=<n>` 假装「还没读完」n 帧。
        // 真实运行走异步：起始页首帧就画 3 行骨架，不必等磁盘（`docs/30` §3.4）。
        let recents_pending_frames = if harness.is_some() {
            std::env::var("MW_RECENTS_PENDING_FRAMES")
                .ok()
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(0)
        } else {
            0
        };
        let mut recent_load = None;
        // 金样里**不读用户机器上的 recent.json**：那是一份会变的外部状态，会让
        // 「空态/窄窗」两张基线随谁在哪台机器上跑而漂（SE-03 就撞上了：同一份代码，
        // 只因真实列表里多了几条，空态基线整页对不上）。要列表就 `MW_START_DEMO=1` 播种。
        let mut recent = if harness.is_some() {
            Recent::pending()
        } else if demo_recents {
            Recent::load()
        } else {
            recent_load = Some(Recent::load_async());
            Recent::pending()
        };
        // 指向已删文件的项：启动时标一次（别每帧去 stat）。**只标不删**——`docs/30` §3.4。
        recent.mark_missing();
        // 验收钩子：内存里放三条固定最近文件，不读也不写 recent.json。
        if demo_recents {
            recent.seed_demo(mw_core::now_ms());
            // `MW_RECENTS_MISSING=<序号>` 把第 n 条（0 起）摆成「文件已移动」，拍 `start-missing` 用。
            if let Some(n) = std::env::var("MW_RECENTS_MISSING")
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
            {
                recent.mark_demo_missing(n);
            }
        }
        // 文件关联双击：Windows 把 `.zmind` 路径作为 argv[1]；`MW_OPEN` 仍可覆盖。
        let cli_open = zmind_arg(std::env::args_os().skip(1)).filter(|p| p.is_file());
        let env_open = std::env::var_os("MW_OPEN").map(std::path::PathBuf::from);
        if let (Some(c), Some(path)) = (canvas.as_mut(), cli_open.or(env_open)) {
            perf.mark("doc_open_start");
            match Session::open(&path) {
                Ok(session) => {
                    let title = session.title();
                    c.replace_session(session);
                    opened = true;
                    // 走的是 Shell::new，不是 open_path：最近文件得在这儿自己记一笔。
                    recent.touch(&path, &title, mw_core::now_ms());
                    // 金样不读「文档旁边」的恢复点：`.autosave/` 是运行产物，夹在夹具旁边
                    // 会让 `mm-cut-*` 这类「点一下 → 弹确认框」的场景先弹恢复提示，整门红
                    // （2026-11-01 实测：`tests/fixtures/x100/.autosave/` 残留一份比正本新的草稿）。
                    recovery = if harness.is_some() {
                        None
                    } else {
                        autosave::pending_for(&path).map(|slot| (slot, path.clone()))
                    };
                }
                Err(e) => eprintln!("MW_OPEN 打开失败：{e}"),
            }
            perf.mark("doc_open");
            // 替换探针：MW_REPLACE="旧=>新" 启动即跑一遍批量替换并打印命中数。
            // 键盘路径（Ctrl+F → Ctrl+H → 输入 → Ctrl+Enter）在探针里容易受焦点环抖动影响，
            // 这条钩子保证「命令链路」本身可被确定性验证。
            if let Some(spec) = std::env::var_os("MW_REPLACE") {
                let spec = spec.to_string_lossy().to_string();
                if let Some((query, to)) = spec.split_once("=>") {
                    let (query, to) = (query.trim(), to.trim());
                    let ids: Vec<String> = c
                        .search_nodes(query, mw_core::MatchMode::Pinyin)
                        .into_iter()
                        .map(|h| h.id)
                        .collect();
                    let mut commands = Vec::new();
                    let mut count = 0usize;
                    for id in ids {
                        let Some(text) = c.node_text(&id).map(str::to_string) else {
                            continue;
                        };
                        let (next, n) = mw_core::search::replace_all(&text, query, to);
                        if n > 0 {
                            count += n;
                            commands.push(mw_core::Command::SetText { id, text: next });
                        }
                    }
                    let total = commands.len();
                    let ok = commands.is_empty()
                        || c.apply_command(mw_core::Command::Batch { commands });
                    println!(
                        "replace_probe	query={query}	replacements={count}	nodes={total}	applied={ok}"
                    );
                }
            }
        }
        // 快照 harness 必须与用户上次的窗口布局无关：走 disabled（不读盘、不写盘）。
        let persist_ui = harness.is_none();
        let window_state = if persist_ui {
            WindowState::load()
        } else {
            WindowState::disabled()
        };
        let (outline_view, show_inspector) = window_state.panels();
        let inspector_w = window_state.inspector_width();
        // 缩略卡（M30）：出厂关，按用户上次的开合恢复；`MW_MINIMAP=1` 是金样旋钮
        // （离屏 harness 用 `WindowState::disabled()`，不设旋钮时一定不画，既有基线零漂移）。
        let show_minimap = match std::env::var_os("MW_MINIMAP") {
            Some(v) => v != "0",
            None => window_state.show_minimap(),
        };
        let minimap_collapsed = std::env::var_os("MW_MINIMAP_COLLAPSED").is_some_and(|v| v != "0");
        let onboarding = !window_state.onboarded() && harness.is_none();
        // 没有文件要打开、也不是快照 harness 时，默认落在开始页——不该先把夹具
        // 文档推给用户。`MW_HOME=0` 强制走文档（探针用），`=1` 强制开始页。
        let home_hook = std::env::var_os("MW_HOME").map(|v| v != "0");
        let home = start_at_home(home_hook, harness.is_some(), opened);
        let golden_input = golden_input::GoldenInput::from_env(harness.is_some());
        let mut shell = Self {
            backdrop: Backdrop::Pending,
            canvas,
            // 首个标签槽是"陈旧槽"：权威会话在画布里，切换时才回填，故先用占位内容。
            tabs: vec![Tab::new(Session::blank())],
            active: 0,
            confirm_close_tab: None,
            harness,
            golden_input,
            golden_clock: 0.0,
            exit,
            toast: None,
            style_gesture: None,
            confirm_close: false,
            close_confirmed: false,
            hwnd: None,
            pending: None,
            recent,
            recent_load,
            recents_pending_frames,
            recovery,
            next_autosave: 0.0,
            last_autosave_rev: std::collections::HashMap::new(),
            autosave: autosave::Background::new(),
            notes: note_store::NoteStore::open_default(),
            note_reader: None,
            reader_from_library: false,
            import_job: None,
            import_warnings: None,
            tag_manager: None,
            library: None,
            library_dialog: None,
            outline: panels::Outline::new(),
            outline_pin: None,
            outline_pin_left: 0,
            outline_pin_done: false,
            outline_question_edit: None,
            via_after_rename: None,
            outline_view,
            inspector: panels::Inspector::new(),
            show_inspector,
            inspector_w,
            show_minimap,
            minimap_collapsed,
            focus_mem: panels::FocusMemory::new(),
            onboarding,
            conflict: None,
            autosave_submitted: Default::default(),
            autosave_stalls: Default::default(),
            allow_replace: false,
            last_finish: None,
            recovery_center: None,
            settings,
            settings_open: false,
            settings_open_after: std::env::var("MW_SETTINGS_AFTER")
                .ok()
                .and_then(|v| v.trim().parse::<u32>().ok()),
            settings_warn_log: None,
            theme_studio: None,
            export_presets: export_presets::load(),
            history: None,
            chrome: chrome_of(&mw_core::get_theme("snow")),
            chrome_theme: "snow".into(),
            tone: Tone::of(&mw_core::get_theme("snow")),
            palette: panels::Palette::new(),
            search: panels::SearchBar::new(),
            context_menu: None,
            outline_row_menu: None,
            tab_menu: None,
            tab_drag: None,
            tab_seen: None,
            tab_scroll_x: 0.0,
            save_phase: SavePhase::Idle,
            pending_card_save: None,
            card_disk_print: None,
            card_overwrite: false,
            open_plan: None,
            open_failed: None,
            ctx: cc.egui_ctx.clone(),
            help: panels::Help::new(),
            reduce_motion,
            shortcuts: shortcuts::Bindings::load(),
            shortcuts_dialog: shortcuts::Dialog::new(),
            presets: theme_presets::Presets::load(),
            export: ExportOpts::default(),
            export_job: None,
            bench_export: std::env::var_os("MW_EXPORT_BENCH").map(std::path::PathBuf::from),
            bench_started: false,
            ui_accent: None,
            sized_to_monitor: false,
            focus_mode: None,
            reading: None,
            review_store: review::Store::load_default(),
            review: None,
            knowledge: None,
            stats_open: false,
            focus_prev_focused: false,
            file_focus_prev: false,
            window_state,
            geometry_restored: false,
            // 启动后 700ms 再写第一次：给默认尺寸/恢复命令留出落地时间。
            next_geometry_save: std::time::Instant::now() + std::time::Duration::from_millis(700),
            refit_until: None,
            // 60 帧（≈1s @60fps）足够让窗口尺寸与面板宽度落定；只在**视口变化**时重取景，
            // 所以多给一点预算不会带来抖动，只会让慢机器也等到落定那一刻。
            start_refit_frames: 60,
            home,
            home_return: false,
            start: StartPage::default(),
            window_title: None,
            perf,
            perf_saw_canvas: false,
            perf_saw_ui: false,
            stats: Stats::from_env(),
            dialog_demo: false,
            content_kit_demo: false,
            concept_hook: None,
            concept_hook_after: 2,
        };
        // 输入脚本按帧推进：warmup 必须覆盖整个脚本，再留几帧让命令落地、动画静止。
        if let Some(script) = shell.golden_input.as_ref() {
            let need = script.frames() as u32 + 4;
            if let Some(harness) = shell.harness.as_mut() {
                harness.extend_warmup(need);
            }
        }
        // 行密度跟着窗口状态走：上次选紧凑，下次打开大纲还是紧凑。
        shell
            .outline
            .set_density(crate::panels::outline::OutlineDensity::from_u8(
                shell.window_state.outline_density(),
            ));
        if home && !opened {
            // 开始页背后不该压着一份会被误存的示例文档：换成干净空白页，
            // 「新建空白图」就能就地复用它。
            if let Some(c) = shell.canvas.as_mut() {
                c.replace_session(Session::blank());
            }
        }
        if !shell.settings.notes_dir.trim().is_empty() {
            if let Some(root) = shell.settings.notes_root() {
                shell.notes = Some(note_store::NoteStore::open_at(root));
            }
        }
        // 视觉矩阵 QA：按页面抓图，不改变正常启动路径。
        if std::env::var_os("MW_HOME_SCREEN").is_some() {
            shell.home = true;
            if let Some(c) = shell.canvas.as_mut() {
                c.replace_session(Session::blank());
            }
        }
        if shell.home || std::env::var_os("MW_START_FOCUS").is_some_and(|v| v != "0") {
            // 首帧还没有任何控件握焦点：先把主操作卡排进下一帧的焦点队列。
            shell.start.request_new_focus();
        }
        // 视觉验收：`MW_THEME=ink/mist/sand` 启动即切主题，方便按主题抓图。
        if let Ok(id) = std::env::var("MW_THEME") {
            if let Some(c) = shell.canvas.as_mut() {
                let _ = c.apply_command(mw_core::Command::SetTheme { theme: id });
            }
        }
        if std::env::var_os("MW_INSPECTOR").is_some() {
            shell.show_inspector = true;
            if let Ok(w) = std::env::var("MW_INSPECTOR_W") {
                if let Ok(w) = w.parse::<f32>() {
                    shell.inspector_w = panels::inspector::clamp_width(w);
                }
            }
        }
        // 证据（`docs/31` SE-06）：`MW_INSPECTOR_GROUPS=style/color=0,style/shape=1`
        // 启动即预置分组开合——「形状与文字」默认折叠，不预置的话字号/边宽/形状
        // 四行永远进不了镜头。只写内存不落盘，理由见 `WindowState::seed_inspector_group`。
        if let Ok(v) = std::env::var("MW_INSPECTOR_GROUPS") {
            for item in v.split(',') {
                if let Some((key, on)) = item.split_once('=') {
                    shell
                        .window_state
                        .seed_inspector_group(key.trim(), on.trim() != "0");
                }
            }
        }
        // 视觉验收：`MW_CONCEPT_DEMO=1` 启动即打开概念图示例（CM 轨 `cm-*` 场景）。
        // 按 `layout` 找模板而不是按下标：模板表以后还会加，下标耦合会静默拍错图。
        if std::env::var_os("MW_CONCEPT_DEMO").is_some() {
            if let Some(tpl) = start::TEMPLATES.iter().find(|t| t.layout == "conceptMap") {
                let session = tpl.session();
                shell.home = false;
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(session);
                    c.request_fit();
                }
            }
        }
        // 视觉验收（CM-10 ⑥）：`MW_CONCEPT_FOCUS=1` 打开示例概念图并写上聚焦问题，
        // `cm-focus-question` 锁住画布顶部那一行字，`cm-outline-focus` 锁住大纲页头下面那一行。
        // 两处钩子都要用同一句话，抽成常量——各写一份迟早会漏改一处。
        const DEMO_FOCUS_QUESTION: &str = "优质睡眠由哪些习惯共同决定？";
        // `MW_CONCEPT_FOCUS_TEXT=<文字>`：临时换一句（验证长问题换行用），不进任何基线。
        let demo_question: String = std::env::var("MW_CONCEPT_FOCUS_TEXT")
            .ok()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| DEMO_FOCUS_QUESTION.to_string());
        if std::env::var_os("MW_CONCEPT_FOCUS").is_some() {
            if let Some(tpl) = start::TEMPLATES.iter().find(|t| t.layout == "conceptMap") {
                let session = tpl.session();
                shell.home = false;
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(session);
                    c.set_focus_question_for_demo(&demo_question);
                    c.request_fit();
                }
            }
        }
        // 视觉验收（CM-10 ⑦）：`MW_CONCEPT_ARROW=off|backward|both` 把示例图的箭头改成那一档。
        // 不设就是默认的前向箭头——17 张既有 `cm-*` 基线画的就是它，所以每档只需一张新基线。
        if let Ok(v) = std::env::var("MW_CONCEPT_ARROW") {
            let head = match v.trim() {
                "off" => Some(mw_core::model::ArrowHead::Off),
                "backward" => Some(mw_core::model::ArrowHead::Backward),
                "both" => Some(mw_core::model::ArrowHead::Both),
                _ => None,
            };
            if let (Some(head), Some(tpl)) = (
                head,
                start::TEMPLATES.iter().find(|t| t.layout == "conceptMap"),
            ) {
                let session = tpl.session();
                shell.home = false;
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(session);
                    c.set_arrow_head_for_demo(head);
                    c.request_fit();
                }
            }
        }
        // 视觉验收（CM-10 ⑦）：`MW_CONCEPT_ARROW_MENU=1` 选中示例图里第一条关系线，
        // 并展开「箭头方向」子菜单——四档互斥，菜单必须能看出现在是哪一档。
        if std::env::var_os("MW_CONCEPT_ARROW_MENU").is_some() {
            let target = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                let mut hit = None;
                for id in doc.doc_order() {
                    if let Some(n) = doc.node(&id) {
                        if let Some(r) = n.relationships.first() {
                            hit = Some((id.to_string(), r.id.clone()));
                            break;
                        }
                    }
                }
                hit
            });
            if let Some((from, id)) = target {
                if let Some(c) = shell.canvas.as_mut() {
                    c.select_relationship(&from, &id);
                    c.request_fit();
                }
            }
            shell.context_menu = shell.build_context_menu(canvas::ContextRequest {
                pos: egui::pos2(360.0, 120.0),
                target: canvas::ContextTarget::Relationship,
            });
            if let Some(menu) = shell.context_menu.as_mut() {
                if let Some(i) = menu.index_of("箭头方向") {
                    menu.demo_open_submenu(i);
                }
            }
        }
        // 视觉验收（CM-07）：`MW_CONCEPT_OUTLINE=1` 打开示例概念图并直接进全屏大纲。
        // `cm-outline` 要锁的是「命题网按连线展开成多级 + 页头标注 + 页脚数概念」，
        // 所以夹具自带模板，不依赖别的钩子先跑过。
        if std::env::var_os("MW_CONCEPT_OUTLINE").is_some() {
            if let Some(tpl) = start::TEMPLATES.iter().find(|t| t.layout == "conceptMap") {
                let session = tpl.session();
                shell.home = false;
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(session);
                    // 这个钩子会**再换一次** session，把 ⑥ 的钩子刚写进去的问题冲掉；
                    // 两个钩子同时给的时候，问题要在这里补回来（`cm-outline-focus` 靠它）。
                    if std::env::var_os("MW_CONCEPT_FOCUS").is_some() {
                        c.set_focus_question_for_demo(&demo_question);
                    }
                    c.request_fit();
                }
            }
            shell.home_return = false;
            // 上一次运行可能把「大纲开着」记进窗口状态；钩子要确定态，不取反。
            shell.outline_view = false;
            shell.toggle_outline_view();
        } // 视觉验收（CM-10 ⑥）：`MW_CONCEPT_FOCUS_EDIT=1` 让大纲页那条问题带立着编辑框，
          // `cm-outline-focus-edit` 锁住「就地改问题」的样子。
          // 放在大纲钩子**之后**：它会把 session 再换一次，先设就被冲掉了。
        if std::env::var_os("MW_CONCEPT_FOCUS_EDIT").is_some() {
            let text = shell
                .canvas
                .as_ref()
                .and_then(|c| c.focus_question())
                .unwrap_or_default();
            shell.outline_question_edit = Some(crate::canvas::FocusEdit {
                text: text.clone(),
                opening: text,
                focus: true,
                rect: egui::Rect::NOTHING,
                ime: false,
            });
        }
        // 视觉验收（C18，打磨轮 53）：`MW_CONCEPT_PIN=<概念文字>` 钉住某个概念（可逗号分隔多颗）。
        // 配合 `MW_CONCEPT_ARRANGE=1` 验证整理布局时钉住概念保持原位。
        if let Ok(targets) = std::env::var("MW_CONCEPT_PIN") {
            if let Some(c) = shell.canvas.as_mut() {
                for target in targets.split(',') {
                    let t = target.trim();
                    let id = c
                        .session()
                        .doc
                        .doc_order()
                        .into_iter()
                        .find(|id| c.session().doc.node(id).is_some_and(|n| n.text == t));
                    if let Some(id) = id {
                        c.toggle_concept_pinned(&id);
                    }
                }
            }
        }
        // 视觉验收：`MW_CONCEPT_ARRANGE=1` 在示例概念图上直接跑一次「整理布局」（CM-04）。
        // `cm-default` 是整理前，`cm-arrange` 是整理后——两张图就是这次的前后对比。
        if std::env::var_os("MW_CONCEPT_ARRANGE").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                c.arrange_concepts();
                c.request_fit();
            }
        }
        // 视觉验收（CM-05）：`MW_CONCEPT_CROSS=1` 给示例概念图补两条交叉连接，
        // 让 `cm-crosslink` 基线锁住「交叉可以存在且渲染稳定」。
        if std::env::var_os("MW_CONCEPT_CROSS").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                c.add_concept_cross_links();
            }
        }
        // 视觉验收（C15，打磨轮 39）：`MW_CONCEPT_PARALLEL=1` 在「用户痛点 → 增长策略」
        // 这组近竖直的边上补第二条命题，两条车道 + 两条错开的短语才进得了基线。
        // C21（打磨轮 63）：值是**补几条**，`=2` 才凑得出 3 条车道（等距避让只在 n ≥ 3 生效）。
        if let Some(v) = std::env::var_os("MW_CONCEPT_PARALLEL") {
            let extra = v
                .to_string_lossy()
                .parse::<usize>()
                .unwrap_or(1)
                .clamp(1, 3);
            if let Some(c) = shell.canvas.as_mut() {
                c.add_concept_parallel_links(extra);
            }
        }
        // 视觉验收（CM-05）：`MW_CONCEPT_SELECT=<概念文字>` 预选一个概念，不动相机。
        // `cm-crosslink` 的输入脚本要拿它做 `Shift+方向` 微调——脚本里点坐标会跟着相机漂。
        if let Ok(text) = std::env::var("MW_CONCEPT_SELECT") {
            if let Some(c) = shell.canvas.as_mut() {
                c.select_concept_by_text(&text);
            }
        }
        // 视觉验收（CM-08）：三个一次性钩子，覆盖 `docs/30` §5.10 里原本没有基线的三张——
        // `cm-node-edit`（就地改名）、`cm-link-create`（新建连线即进短语编辑）、
        // `cm-label-ime`（短语框的 IME 组合）。
        //
        // 这里只**登记**，不立刻执行：短语编辑框要落在线段中点，中点在画布第一帧绘制
        // 之后才有（`last_viewport` / `last_rect`）。真正的执行点在 `main.rs` 画布
        // `show()` 之后，见 `Shell::concept_hook`。
        //
        // 输入格式：`MW_CONCEPT_LINK="用户痛点|风险与依赖"`（`|` 分隔源与目标）；
        // `MW_CONCEPT_EDIT` / `MW_CONCEPT_REL_EDIT` 各取一个概念文字 / 链接短语。
        if let Ok(text) = std::env::var("MW_CONCEPT_EDIT") {
            if !text.trim().is_empty() {
                shell.concept_hook = Some(canvas::ConceptHook::Edit(text));
            }
        }
        if let Ok(spec) = std::env::var("MW_CONCEPT_RELINK_BLANK") {
            let parts: Vec<&str> = spec.split('|').collect();
            if parts.len() == 3 {
                if let (Ok(dx), Ok(dy)) = (
                    parts[1].trim().parse::<i32>(),
                    parts[2].trim().parse::<i32>(),
                ) {
                    shell.concept_hook = Some(canvas::ConceptHook::RelinkBlank(
                        parts[0].trim().to_string(),
                        dx,
                        dy,
                    ));
                }
            }
        }
        if let Ok(label) = std::env::var("MW_CONCEPT_SELECT_REL") {
            // 只选中某条关系、不拖：给 `cm-relink-drag` 做「强调是否被抑制」的对照。
            if !label.trim().is_empty() {
                shell.concept_hook = Some(canvas::ConceptHook::SelectRel(label.trim().to_string()));
            }
        }
        if let Ok(text) = std::env::var("MW_SUMMARY_SELECT") {
            // 给当前文档加一条概要并选中它：注解选中环与跨度手柄的像素证据（第 51 轮）。
            // 这两处曾画在合成纹理贴图之前、在界面上从来没显示过（见第 50 轮）。
            if !text.trim().is_empty() {
                shell.concept_hook =
                    Some(canvas::ConceptHook::SummarySelect(text.trim().to_string()));
            }
        }
        if let Ok(text) = std::env::var("MW_ANNOT_MULTI") {
            if !text.trim().is_empty() {
                // 概要 + 边界同时选中：additive 选择的画面证据（第 54 轮）。
                shell.concept_hook =
                    Some(canvas::ConceptHook::MultiSelect(text.trim().to_string()));
            }
        }
        if let Ok(text) = std::env::var("MW_ANNOT_MULTI_OFF") {
            if !text.trim().is_empty() {
                // 多选再取消一条：留不留残影，用「单选那张基线」当断言（第 56 轮）。
                shell.concept_hook =
                    Some(canvas::ConceptHook::MultiSelectOff(text.trim().to_string()));
            }
        }
        if let Ok(text) = std::env::var("MW_SPAN_COMMIT") {
            if !text.trim().is_empty() {
                // 跨度拖拽松手提交：发命令、只撤一次、画面停在变宽之后（第 57 轮）。
                shell.concept_hook = Some(canvas::ConceptHook::SpanCommit(text.trim().to_string()));
            }
        }
        if let Ok(text) = std::env::var("MW_SPAN_DRAG") {
            if !text.trim().is_empty() {
                // 跨度拖拽中的预览框：`paint_span_preview` 那条路径的画面证据（第 53 轮）。
                shell.concept_hook = Some(canvas::ConceptHook::SpanDrag(text.trim().to_string()));
            }
        }
        if std::env::var_os("MW_BOUNDARY_SELECT").is_some() {
            // 边界的选中环：与概要共用绘制代码、几何不同，单独拍一张才算证据（第 52 轮）。
            shell.concept_hook = Some(canvas::ConceptHook::BoundarySelect);
        }
        if let Ok(spec) = std::env::var("MW_CONCEPT_RELINK_DRAG") {
            // 格式：`短语|dx|dy`——选中那条关系、指针停在目标端偏移处，**不松手**（CM-10 ⑦）。
            let parts: Vec<&str> = spec.split('|').collect();
            if parts.len() == 3 {
                if let (Ok(dx), Ok(dy)) = (
                    parts[1].trim().parse::<i32>(),
                    parts[2].trim().parse::<i32>(),
                ) {
                    shell.concept_hook = Some(canvas::ConceptHook::RelinkDrag(
                        parts[0].trim().to_string(),
                        dx,
                        dy,
                    ));
                }
            }
        }
        if let Ok(spec) = std::env::var("MW_CONCEPT_LINK_DRAG") {
            let parts: Vec<&str> = spec.split('|').collect();
            if parts.len() == 3 {
                if let (Ok(dx), Ok(dy)) = (
                    parts[1].trim().parse::<i32>(),
                    parts[2].trim().parse::<i32>(),
                ) {
                    shell.concept_hook = Some(canvas::ConceptHook::LinkDrag(
                        parts[0].trim().to_string(),
                        dx,
                        dy,
                    ));
                }
            }
        }
        if let Ok(spec) = std::env::var("MW_CONCEPT_LINK_BLANK") {
            // 格式：`源概念|dx|dy`——落点相对源概念，跨机器可复现（CM-10 ①）。
            let parts: Vec<&str> = spec.split('|').collect();
            if parts.len() == 3 {
                if let (Ok(dx), Ok(dy)) = (
                    parts[1].trim().parse::<i32>(),
                    parts[2].trim().parse::<i32>(),
                ) {
                    shell.concept_hook = Some(canvas::ConceptHook::LinkBlank(
                        parts[0].trim().to_string(),
                        dx,
                        dy,
                    ));
                }
            }
        }
        if let Ok(pair) = std::env::var("MW_CONCEPT_LINK") {
            if let Some((from, to)) = pair.split_once('|') {
                shell.concept_hook = Some(canvas::ConceptHook::Link(
                    from.trim().to_string(),
                    to.trim().to_string(),
                ));
            }
        }
        if let Ok(label) = std::env::var("MW_CONCEPT_REL_EDIT") {
            if !label.trim().is_empty() {
                shell.concept_hook = Some(canvas::ConceptHook::Phrase(label));
            }
        }
        // 视觉验收（CM-14，连线体系）：四个夹具各拍一张。
        // 它们和真实手势走同一条函数（`begin_relationship_action` / `reverse_selected_relationship`
        // / `start_rel_edit`），所以基线锁的是真状态，不是另画一份假画面。
        if let Ok(text) = std::env::var("MW_CONCEPT_LINK_MODE") {
            if !text.trim().is_empty() {
                shell.concept_hook = Some(canvas::ConceptHook::LinkMode(text.trim().to_string()));
            }
        }
        if let Ok(pair) = std::env::var("MW_CONCEPT_LINK_TWO") {
            if let Some((a, b)) = pair.split_once('|') {
                shell.concept_hook = Some(canvas::ConceptHook::LinkTwo(
                    a.trim().to_string(),
                    b.trim().to_string(),
                ));
            }
        }
        if let Ok(label) = std::env::var("MW_CONCEPT_REVERSE") {
            if !label.trim().is_empty() {
                shell.concept_hook = Some(canvas::ConceptHook::Reverse(label.trim().to_string()));
            }
        }
        if let Ok(spec) = std::env::var("MW_CONCEPT_GROUP") {
            let members: Vec<String> = spec
                .split('|')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if members.len() >= 2 {
                shell.concept_hook = Some(canvas::ConceptHook::Group(members));
            }
        }
        if let Ok(spec) = std::env::var("MW_CONCEPT_MULTI_REL") {
            let labels: Vec<String> = spec
                .split('|')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !labels.is_empty() {
                shell.concept_hook = Some(canvas::ConceptHook::MultiRel(labels));
            }
        }
        if let Ok(label) = std::env::var("MW_CONCEPT_REL_STYLE") {
            if !label.trim().is_empty() {
                shell.concept_hook = Some(canvas::ConceptHook::RelStyle(label.trim().to_string()));
            }
        }
        if let Ok(spec) = std::env::var("MW_CONCEPT_SUGGEST") {
            if let Some((label, typed)) = spec.split_once('|') {
                shell.concept_hook = Some(canvas::ConceptHook::Suggest(
                    label.trim().to_string(),
                    typed.trim().to_string(),
                ));
            }
        }
        // 视觉验收（CM-06）：`MW_CONCEPT_CARD=1` 给「用户痛点」挂一张卡片并直接打开
        // 卡片页——`cm-card-link` 基线要锁的是「来源 = 概念图 · 用户痛点」这一行。
        // `MW_CONCEPT_CARD_BACK=<概念文字>` 再往前一步：走真实的回跳落地（与卡片页
        // 「回到概念」按钮同一条 `jump_to_source`），把选中态拍成 `cm-card-link-back`。
        //
        // 这里用 `replace_session` 造一份**带卡片的概念图模板**，而不是先开模板再发
        // `AttachNewCard`：那条命令会把 `updated_at` 刷成墙钟，卡片页的「更新于」
        // 每天漂一天，基线天天红。夹具口径与 `Template::session()` 一致——从零构造
        // 初始文档（新文档没有撤销历史要维护），`validate()` 兜底。
        if std::env::var_os("MW_CONCEPT_CARD").is_some() {
            if let Some(tpl) = start::TEMPLATES.iter().find(|t| t.layout == "conceptMap") {
                let mut session = tpl.session();
                let node = session
                    .doc
                    .nodes
                    .values()
                    .find(|n| n.text == "用户痛点")
                    .map(|n| n.id.clone());
                if let Some(node) = node {
                    let card = mw_core::NoteCard::new(
                        session.doc.id.clone(),
                        Some(node.clone()),
                        "痛点访谈 · 三条原话",
                        "## 访谈原话\n\n- 「切来切去太麻烦」\n- 「找不到上次改到哪」\n- 「导出还得再排一遍」\n\n> 这三条是概念「用户痛点」底下的材料。",
                        DEMO_CARD_TS_MS,
                    );
                    let cid = card.id.clone();
                    session.doc.cards.push(card);
                    if let Some(n) = session.doc.nodes.get_mut(&node) {
                        n.card_id = Some(cid);
                    }
                    assert!(session.doc.validate().is_ok(), "演示夹具必须产出合法文档");
                }
                shell.home = false;
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(session);
                    c.request_fit();
                }
            }
            let linked = shell
                .canvas
                .as_ref()
                .and_then(|c| c.session().doc.cards.first().cloned());
            if let Some(card) = linked {
                let reader = shell.card_reader_for(&card);
                shell.note_reader = Some(reader);
            }
            if let Ok(text) = std::env::var("MW_CONCEPT_CARD_BACK") {
                let back = shell.canvas.as_ref().and_then(|c| {
                    let doc = &c.session().doc;
                    doc.nodes
                        .values()
                        .find(|n| n.text == text)
                        .map(|n| n.id.clone())
                });
                if let Some(id) = back {
                    shell.jump_to_source(&id);
                    if let Some(c) = shell.canvas.as_mut() {
                        c.request_fit();
                    }
                }
            }
        }
        // 视觉验收（CM-06）：`MW_CARD_PICK=<概念文字>` 拍检查器「关联已有」的展开态。
        // 先造两张卡：一张挂在「风险与依赖」上（被占，不该进候选），一张挂上再解绑
        // 变成孤儿（该进候选）。两条命令都走 `Command`，卡片留在文档里。
        if let Ok(text) = std::env::var("MW_CARD_PICK") {
            let holder = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.nodes
                    .values()
                    .find(|n| n.text == "风险与依赖")
                    .map(|n| n.id.clone())
            });
            let doc_id = shell
                .canvas
                .as_ref()
                .map(|c| c.session().doc.id.clone())
                .unwrap_or_default();
            if let Some(holder) = holder {
                let taken = mw_core::NoteCard::new(
                    doc_id.clone(),
                    None,
                    "依赖清单 · 三条",
                    "",
                    DEMO_CARD_TS_MS,
                );
                let free =
                    mw_core::NoteCard::new(doc_id, None, "竞品访谈 · 两家", "", DEMO_CARD_TS_MS);
                if let Some(c) = shell.canvas.as_mut() {
                    let _ = c.apply_command(mw_core::Command::AttachNewCard {
                        node: holder.clone(),
                        card: free.clone(),
                    });
                    let _ = c.apply_command(mw_core::Command::SetNodeCard {
                        node: holder.clone(),
                        card: None,
                    });
                    let _ = c.apply_command(mw_core::Command::AttachNewCard {
                        node: holder,
                        card: taken,
                    });
                }
            }
            let target = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.nodes
                    .values()
                    .find(|n| n.text == text)
                    .map(|n| n.id.clone())
            });
            shell.show_inspector = true;
            if let Some(c) = shell.canvas.as_mut() {
                c.select_concept_by_text(&text);
            }
            if let Some(id) = target {
                shell.inspector.open_card_picker(&id);
            }
        }
        // 视觉验收：`MW_SELECT_DEMO=1` 启动即选中样例里的「纯 Rust 原生」，
        // 让顶部上下文工具条与状态区选中摘要进入基线。
        // 验收：`MW_INSPECTOR_FAIL=1` 走一次**真的失败路径**——发一条指向不存在节点的
        // `SetStyle`，让壳层像用户遇到失败那样贴错误条（`docs/30` §4.3 的 28px 条）。
        if std::env::var_os("MW_INSPECTOR_FAIL").is_some() {
            shell.show_inspector = true;
            let ok = shell.canvas.as_mut().is_some_and(|c| {
                c.apply_command(mw_core::Command::SetStyle {
                    ids: vec!["no-such-node".into()],
                    style: None,
                })
            });
            if !ok {
                shell
                    .inspector
                    .report_error("修改未生效：这条命令被文档拒绝了");
            }
        }
        if let Some(mode) = std::env::var_os("MW_SELECT_DEMO") {
            // `=multi` 额外选上它的父级与第一个子级，给 `inspector-multiselect` 当靶子（§4.6）。
            let multi = mode.to_string_lossy().eq_ignore_ascii_case("multi");
            let found = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                let id = doc
                    .doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == "纯 Rust 原生"))?;
                let parent = doc.node(&id).and_then(|n| n.parent_id.clone());
                let child = doc.node(&id).and_then(|n| n.children.first()).cloned();
                Some((id, parent, child))
            });
            if let Some((id, parent, child)) = found {
                if let Some(c) = shell.canvas.as_mut() {
                    c.select_node(&id);
                    if multi {
                        for extra in [parent, child].into_iter().flatten() {
                            c.toggle_select_node(&extra);
                        }
                    }
                    // 选中会把相机拉回 100%；基线要的是「整张图 + 选中环」，
                    // 仍然按默认适应取景，否则右侧子树会被裁掉。
                    c.request_fit();
                }
            }
        }
        // 视觉验收（SE-07）：`MW_INSPECTOR_TAB=note` 把检查器开在指定页；
        // `MW_CONTENT_DEMO=1` 先给「纯 Rust 原生」喂齐五项内容（备注 / 标签 / 任务 / 链接 / 卡片）。
        // 全部走 `Command`——基线里看到的和用户手点出来的是同一条路径、同一条撤销栈。
        if let Some(w) = std::env::var_os("MW_INSPECTOR_TAB") {
            shell.show_inspector = true;
            shell
                .inspector
                .open_tab(&w.to_string_lossy().to_lowercase());
        }
        // 视觉验收（SE-06b）：`MW_STEPPER_EDIT=行键:草稿` 让某一格数值一进来就是编辑态
        // （如 `inspector/style/size:48`）。真实路径要先点一下数值格，离屏默认每帧清指针，拍不到。
        if let Some(v) = std::env::var_os("MW_STEPPER_EDIT") {
            shell.show_inspector = true;
            let s = v.to_string_lossy().to_string();
            let (row, draft) = s.split_once(':').unwrap_or((s.as_str(), ""));
            shell.inspector.seed_stepper_edit(row.trim(), draft);
        }
        if std::env::var_os("MW_CONTENT_DEMO").is_some() {
            let target = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == "纯 Rust 原生"))
            });
            let doc_id = shell
                .canvas
                .as_ref()
                .map(|c| c.session().doc.id.clone())
                .unwrap_or_default();
            if let Some(id) = target {
                let card = mw_core::NoteCard::new(
                    doc_id,
                    Some(id.clone()),
                    "wgpu 渲染层 · 四层合成",
                    "离屏与交互共用同一份 Scene；字形图集常驻。",
                    DEMO_CARD_TS_MS,
                );
                if let Some(c) = shell.canvas.as_mut() {
                    let _ = c.apply_command(mw_core::Command::SetNote {
                        id: id.clone(),
                        note: Some(DEMO_NOTE.into()),
                    });
                    for label in ["渲染", "性能"] {
                        let _ = c.apply_command(mw_core::Command::AddLabel {
                            ids: vec![id.clone()],
                            label: label.into(),
                        });
                    }
                    let _ = c.apply_command(mw_core::Command::SetTask {
                        id: id.clone(),
                        task: Some(mw_core::TaskState::Doing),
                    });
                    let _ = c.apply_command(mw_core::Command::SetLink {
                        id: id.clone(),
                        link: Some(DEMO_LINK.into()),
                    });
                    let _ = c.apply_command(mw_core::Command::AttachNewCard { node: id, card });
                }
            }
            shell.show_inspector = true;
            shell.inspector.open_tab("note");
        }
        // 视觉验收（SE-13）：`MW_STYLE_DEMO=weight=600,align=center,radius=14,shape=ellipse`
        // 给「纯 Rust 原生」喂一份样式覆盖——四项新能力（字重三段 / 对齐 / 圆角 / 椭圆）
        // 每一张基线都要有「值真的生效」的画面，而离屏默认每帧清指针，点不到控件。
        // 与前几轮同一条纪律：一律走 `Command::SetStyle`，用户手点也是这条路。
        if let Some(spec) = std::env::var_os("MW_STYLE_DEMO") {
            shell.show_inspector = true;
            let target = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == "纯 Rust 原生"))
            });
            let mut over = mw_core::StyleOverride::default();
            for part in spec.to_string_lossy().split(',') {
                let Some((k, v)) = part.split_once('=') else {
                    continue;
                };
                match k.trim() {
                    "weight" => over.weight = v.trim().parse().ok(),
                    "align" => over.align = Some(v.trim().to_string()),
                    "radius" => over.radius = v.trim().parse().ok(),
                    "shape" => over.shape = Some(v.trim().to_string()),
                    "fontSize" => over.font_size = v.trim().parse().ok(),
                    _ => {}
                }
            }
            if let (Some(c), Some(id)) = (shell.canvas.as_mut(), target) {
                let _ = c.apply_command(mw_core::Command::SetStyle {
                    ids: vec![id],
                    style: Some(over),
                });
                c.request_fit();
            }
        }
        // 视觉验收（M20）：`MW_COLLAPSE_DEMO=<节点文字>` 启动即折叠该节点，
        // 给「搜索命中折叠子树」一个确定的夹具（默认样例里用「中文排版」）。
        if let Some(target_text) = std::env::var_os("MW_COLLAPSE_DEMO") {
            let target_text = target_text.to_string_lossy().to_string();
            let target = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == target_text))
            });
            if let (Some(c), Some(id)) = (shell.canvas.as_mut(), target) {
                let _ = c.apply_command(mw_core::Command::SetCollapsed {
                    id,
                    collapsed: true,
                });
            }
        }
        // 视觉验收（M32）：`MW_DRILL_DEMO=<节点文字>` 在该节点的**第一颗直接子级**下
        // 再挂一颗「GPU 合成」，让「双击折叠圆点再深入一层」有第三层可露。
        // 只在设了环境变量时生效——默认样例不变，既有 `mm-*` 基线不受影响。
        if let Some(target_text) = std::env::var_os("MW_DRILL_DEMO") {
            let target_text = target_text.to_string_lossy().to_string();
            let first_child = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == target_text))
                    .and_then(|id| doc.node(&id))
                    .and_then(|n| n.children.first().cloned())
            });
            if let (Some(c), Some(child)) = (shell.canvas.as_mut(), first_child) {
                let _ = c.apply_command(mw_core::Command::AddNode {
                    parent: child,
                    index: None,
                    text: "GPU 合成".into(),
                    style: None,
                });
            }
        }
        // 视觉验收（M22）：`MW_FOCUS_DEMO=<节点文字>` 启动即选中并聚焦该节点，
        // 给「搜索命中聚焦视图之外」一个确定的夹具（默认样例里用「中文排版」）。
        if let Some(target_text) = std::env::var_os("MW_FOCUS_DEMO") {
            let target_text = target_text.to_string_lossy().to_string();
            let target = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == target_text))
            });
            if let (Some(c), Some(id)) = (shell.canvas.as_mut(), target) {
                c.select_node(&id);
                if c.focus_selected() {
                    crate::trace::event(&format!("focus_demo id={id}"));
                }
            }
        }
        // 视觉验收（M19）：只给「纯 Rust 原生」喂备注与链接，**不开检查器**——
        // 搜索条在默认画布布局里居中，基线要的是「搜索条 + 结果行」本身，
        // 不是检查器。命令路径与用户手填同一条。
        if std::env::var_os("MW_SEARCH_DEMO").is_some() {
            let target = shell.canvas.as_ref().and_then(|c| {
                let doc = &c.session().doc;
                doc.doc_order()
                    .into_iter()
                    .find(|id| doc.node(id).is_some_and(|n| n.text == "纯 Rust 原生"))
            });
            if let (Some(c), Some(id)) = (shell.canvas.as_mut(), target) {
                let _ = c.apply_command(mw_core::Command::SetNote {
                    id: id.clone(),
                    note: Some(DEMO_NOTE.into()),
                });
                let _ = c.apply_command(mw_core::Command::SetLink {
                    id,
                    link: Some(DEMO_LINK.into()),
                });
            }
        }
        // 证据隔离（M19）：`MW_NOTES=none` 关掉本机真实笔记库。搜索条会把卡片
        // 正文并进结果，库里有「html」就会多一行——基线只能锁住文档本身的命中。
        if std::env::var_os("MW_NOTES")
            .is_some_and(|v| v.to_string_lossy().eq_ignore_ascii_case("none"))
        {
            shell.notes = None;
        }
        // 视觉验收：`MW_CONTEXT_DEMO=1` 固定选中「纯 Rust 原生」并在画布中部
        // 打开节点右键菜单、展开「样式」子菜单，锁住菜单引擎的完整形态。
        // `MW_CONTEXT_DEMO=blank` 改开**空白处**那张菜单：概念图的空白菜单里不该出现
        // 「全部展开 / 全部折叠」（那是树语义），也不该出现「新建自由主题」——CM-05 的规矩
        // 以前只有代码依据、没有基线，`cm-blank-menu` 补上。
        if let Some(which) = std::env::var_os("MW_CONTEXT_DEMO") {
            let mode = which.to_string_lossy().to_string();
            let blank = mode == "blank";
            // `multi`：选两个概念再开菜单，锁的是**多选那条 arm**（带「排列」子菜单），
            // 单选与多选是两个分支，以前只有单选有基线。
            let multi = mode == "multi";
            if multi {
                let ids: Vec<String> = shell
                    .canvas
                    .as_ref()
                    .map(|c| {
                        let doc = &c.session().doc;
                        doc.doc_order()
                            .into_iter()
                            // 主概念被「排列」排除（`selected_free_topics` 跳过 root_id），
                            // 所以这里也要跳过，否则选了两个却只算一个，子菜单不出现。
                            .filter(|id| {
                                *id != doc.root_id
                                    && doc.node(id).is_some_and(|n| n.parent_id.is_none())
                            })
                            .take(2)
                            .collect()
                    })
                    .unwrap_or_default();
                if ids.len() == 2 {
                    if let Some(c) = shell.canvas.as_mut() {
                        c.select_many(&ids, &ids[0]);
                        c.request_fit();
                    }
                }
            } else {
                // `concept:<概念文字>` 给概念图的节点菜单用（CM-14）：默认目标是思维导图
                // 示例里的「纯 Rust 原生」，概念图里没有这颗，菜单就开在一张空选中上。
                let want = mode
                    .strip_prefix("concept:")
                    .map(str::trim)
                    .unwrap_or("纯 Rust 原生");
                let target = shell.canvas.as_ref().and_then(|c| {
                    let doc = &c.session().doc;
                    doc.doc_order()
                        .into_iter()
                        .find(|id| doc.node(id).is_some_and(|n| n.text == want))
                });
                if let Some(id) = target {
                    if let Some(c) = shell.canvas.as_mut() {
                        c.select_node(&id);
                        c.request_fit();
                    }
                }
            }
            shell.context_menu = shell.build_context_menu(canvas::ContextRequest {
                // 默认 (360,120)：贴着左上开，菜单整张在屏幕里。
                // `MW_CONTEXT_POS` 只给验收用，用来把请求点挪到底边附近拍夹取（C10）。
                pos: canvas::context_pos().unwrap_or(egui::pos2(360.0, 120.0)),
                target: if blank {
                    canvas::ContextTarget::Blank
                } else {
                    canvas::ContextTarget::Node
                },
            });
            if !blank {
                let sub = if multi { "排列" } else { "样式" };
                if let Some(menu) = shell.context_menu.as_mut() {
                    if let Some(i) = menu.index_of(sub) {
                        menu.demo_open_submenu(i);
                    }
                }
            }
        }
        // CER-13 空卡片：标题与正文都空，只剩光标占位（golden 场景 card-empty）。
        if std::env::var_os("MW_CARD_EMPTY").is_some() {
            if let Some(c) = shell.canvas.as_ref() {
                let card = mw_core::NoteCard::new(
                    c.session().doc.id.clone(),
                    None,
                    "",
                    "",
                    DEMO_CARD_TS_MS,
                );
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        // CER-13 版心标尺：一段**没有空格**的长中文——CJK 断行必须落在版心右边界上。
        // 短段落量不出 720，只有长到必须换行的段落才量得出「换行线在哪」。
        if std::env::var_os("MW_CARD_LONG").is_some() {
            if let Some(c) = shell.canvas.as_ref() {
                let mut body = String::from("## 版心标尺\n\n");
                for _ in 0..6 {
                    body.push_str("知识不是收藏品，而是被反复改写的草稿：先压缩成一句话，再接回已有的卡片，最后留下一个下次能回答的问题。");
                }
                let card = mw_core::NoteCard::new(
                    c.session().doc.id.clone(),
                    None,
                    "版心标尺",
                    &body,
                    DEMO_CARD_TS_MS,
                );
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        // CER-19 打字机实拍：正文必须**真的溢出**正文视口（约 240 CSS），否则
        // 「把光标锚到 42%」这件事没有可滚动的距离，拍出来等于没演示。
        if std::env::var_os("MW_CARD_TALL").is_some() {
            if let Some(c) = shell.canvas.as_ref() {
                let mut body = String::from("## 长文分段\n\n");
                for i in 0..20 {
                    body.push_str("知识不是收藏品，而是被反复改写的草稿：先压缩成一句话，再接回已有的卡片，最后留下一个下次能回答的问题。");
                    body.push_str(&format!("（第 {} 段）\n\n", i + 1));
                }
                let card = mw_core::NoteCard::new(
                    c.session().doc.id.clone(),
                    None,
                    "长文分段",
                    &body,
                    DEMO_CARD_TS_MS,
                );
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        if std::env::var_os("MW_CARD_DEMO").is_some() {
            if let Some(c) = shell.canvas.as_ref() {
                let card = mw_core::NoteCard::new(
                    c.session().doc.id.clone(),
                    None,
                    "把知识变成自己的",
                    "# 把知识变成自己的\n\n> 阅读不是收藏，是重写。\n\n## 三个动作\n\n1. **压缩**：一句话说清\n2. **连接**：接到已有卡片\n3. **提问**：留下一个下次能回答的问题\n\n`[[间隔重复]]` 让卡片在快忘记时回来。",
                    DEMO_CARD_TS_MS,
                );
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        if std::env::var_os("MW_CARD_SLASH").is_some() {
            if let Some(r) = shell.note_reader.as_mut() {
                r.arm_slash_demo();
            }
        }
        // CER-19：专注模式 / 打字机模式两张实拍夹具。
        // `MW_CARD_FOCUS=0` 是**同一个人**（光标、标记揭示都一样）只是不淡——
        // 这样两张图的唯一差别就是「淡没淡」，才能拿纯白像素证明没有遮罩。
        if let Some(v) = std::env::var_os("MW_CARD_FOCUS") {
            if let Some(r) = shell.note_reader.as_mut() {
                r.arm_focus_demo();
                if v == "0" {
                    r.clear_focus_demo();
                }
            }
        }
        if std::env::var_os("MW_CARD_TYPEWRITER").is_some() {
            if let Some(r) = shell.note_reader.as_mut() {
                r.arm_typewriter_demo();
            }
        }
        // CER-14：一张「标签 / 来源 / 反链 / 断链」都齐的卡片，给 card-tags 场景用。
        // 反链要靠真实引用关系：第二张卡正文写 `[[把知识变成自己的]]`，指向第一张。
        // CER-17：任务框点击切换。两条任务项（一条未勾、一条已勾），脚本点第一条的勾选框。
        if std::env::var_os("MW_CARD_TASK").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "勾选框点一下就好",
                    "第一段\n\n- [ ] 待办一\n- [x] 待办二\n\n尾段\n",
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-task".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        if std::env::var_os("MW_CARD_TAGS").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id.clone(),
                    Some(root.clone()),
                    "把知识变成自己的",
                    "阅读不是收藏，是重写。\n\n另见 [[间隔重复]] 与 [[还没写的卡片]]。\n",
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-tags".into();
                card.tags = mw_core::NoteCard::normalize_tags(&[
                    "知识/方法".to_string(),
                    "整理术".to_string(),
                ]);
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                // 第二个节点挂一张引用它的卡：正文明写 `[[…]]`，页尾才有反链。
                let _ = c.apply_command(mw_core::Command::AddNode {
                    parent: root.clone(),
                    index: None,
                    text: "间隔重复".into(),
                    style: None,
                });
                let second = c
                    .session()
                    .doc
                    .node(&root)
                    .and_then(|n| n.children.last().cloned());
                if let Some(node) = second {
                    let mut other = mw_core::NoteCard::new(
                        doc_id,
                        Some(node.clone()),
                        "间隔重复",
                        "每次复习都重写一遍，而不是重读一遍。详见 [[把知识变成自己的]]。\n",
                        DEMO_CARD_TS_MS + 60_000,
                    );
                    other.id = "demo-tags-ref".into();
                    let _ = c.apply_command(mw_core::Command::AttachNewCard { node, card: other });
                }
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }

        // CER-16：片段级标记揭示。光标停在第 1 行的 `**粗体**` 里，
        // 只看这一段的两个 `**` 露出来——同一行的反引号、下一行的 `##`、`- ` 都仍是成稿态。
        if std::env::var_os("MW_CARD_REVEAL").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "把标记留在该留的地方",
                    r#"前 **粗体** 后 `码` 尾

## 小标题

- 列表一项
"#,
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-reveal".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        // CER-15：块语法全家桶——H2/H3/H4、段落、有序/无序/任务列表、引用、
        // 行内码、带语言的代码块、分隔线，全在一张卡里，方便逐块量盒子。
        // CER-21 `card-code`：代码块 + 行内码 + 链接 + 图片语法同页（`docs/28` §6.11）。
        if std::env::var_os("MW_CARD_CODE").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "代码、链接与图片",
                    "# 代码、链接与图片\n\n行内 `code` 与 [链接](https://example.com) 挨在同一段里。\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\n![示意](shot.png)\n",
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-code".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        if std::env::var_os("MW_CARD_BLOCKS").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "把知识变成自己的",
                    r#"## 为什么重写

阅读笔记的价值在重写，不在收藏。收藏只是把别人的句子搬进自己的抽屉。

### 具体做法

1. 读完一小节就合上原文
2. 用自己的话写三句
3. 一周后回来改写一次

#### 补充

- 每天只处理一小节
- [ ] 周二：间隔重复
- [x] 周一：把知识变成自己的

> 记笔记不是抄写，是把别人的句子拆开、再装回去。

行内 `code` 与 **粗体** 混排在同一段里。

```rust
fn main() {
    println!("hello");
}
```

---
"#,
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-blocks".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }

        // CER-20 长文性能：20k 中文字符的一张卡（打开 / 连续输入 / IME 三档都要量）。
        if std::env::var_os("MW_CARD_20K").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut body = String::new();
                while body.chars().count() < 20_000 {
                    body.push_str(
                        "间隔重复是把记忆交给时间的方法，每一次回想都会让下一次更容易。\n\n",
                    );
                }
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "二十万字的一页",
                    &body,
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-20k".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        // CER-20：保存态 / 外部修改 / 只读三张实拍夹具。同一张普通卡片，只摆读者页的状态，
        // 不碰正文——三张图的差别必须只在顶栏与那条 32px 提示条上。
        if let Ok(state) = std::env::var("MW_CARD_STATE") {
            if let Some(c) = shell.canvas.as_ref() {
                let card = mw_core::NoteCard::new(
                    c.session().doc.id.clone(),
                    None,
                    "保存这件事",
                    "第一段。\n\n第二段，写点字。\n",
                    DEMO_CARD_TS_MS,
                );
                let mut reader = shell.card_reader_for(&card);
                match state.as_str() {
                    "conflict" => reader.set_conflict(true),
                    "readonly" => reader.set_read_only(true),
                    "saving" => reader.mark_saving(),
                    other => eprintln!("[fixture] 未知 MW_CARD_STATE={other}"),
                }
                shell.note_reader = Some(reader);
                shell.reader_from_library = false;
            }
        }
        // CER-16 / CER-17：IME。正文两段，光标停在第二段行尾，组合串应该原地长出来，
        // 确认后原地替换 —— 两段之间的行距不许动（组合不参与排版）。
        if std::env::var_os("MW_CARD_IME").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "输入法不许打架",
                    r#"第一段在这里。

第二段在这里。"#,
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-ime".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        // CER-16 后半：行尾回车。清单很短，光标好定位：点第二项行尾按回车，
        // 看新行有没有带上 `- `（续写），以及空项再按回车有没有把前缀退掉（退出）。
        if std::env::var_os("MW_CARD_ENTER").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "回车要按规矩来",
                    r#"第一项
- 第二项
"#,
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-enter".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        // CER-16 后半：即时转换。三段普通文字，脚本在第二段行尾回车另起一行，
        // 再逐格敲 `- ` / `1. ` / `## `——每敲完那一格空格，这一行就当场变成对应块。
        if std::env::var_os("MW_CARD_INSTANT").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "敲下标记就成块",
                    "第一段\n\n第二段\n\n第三段\n",
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-instant".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        // CER-16：链接片段。光标停在 `[织思](url)` 里时括号与 URL 要露出来，
        // 同一行的 `[[双链]]` 仍是成稿态（只露光标那一段）。
        if std::env::var_os("MW_CARD_REVEAL_LINK").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                let root = c.session().doc.root_id.clone();
                let doc_id = c.session().doc.id.clone();
                let mut card = mw_core::NoteCard::new(
                    doc_id,
                    Some(root.clone()),
                    "链接也要能看见它自己",
                    r#"见 [织思](https://example.com/doc) 与 [[双链]]。
"#,
                    DEMO_CARD_TS_MS,
                );
                card.id = "demo-reveal-link".into();
                let _ = c.apply_command(mw_core::Command::AttachNewCard {
                    node: root.clone(),
                    card: card.clone(),
                });
                shell.note_reader = Some(shell.card_reader_for(&card));
                shell.reader_from_library = false;
            }
        }
        if std::env::var_os("MW_SETTINGS_DEMO").is_some() {
            shell.settings_open = true;
            // 金样必须确定：`当前：…` 那行读的是 notes_root()，不钉住的话
            // 基线会拍进本机/门里各自的 APPDATA 路径。
            shell.settings.notes_dir = "D:/织思/笔记".into();
        }
        // M28 验收：`MW_UNDO_LIMIT=100` 只改内存里的设置值（不落盘），
        // 每帧对账会在首帧把它作用到画布与所有标签，并吐一条 `history limit=…` 轨迹。
        if let Ok(v) = std::env::var("MW_UNDO_LIMIT") {
            if let Ok(n) = v.trim().parse::<usize>() {
                shell.settings.undo_limit = n;
            }
        }
        // 快捷键面板 golden / 行为脚本：`MW_SHORTCUTS_DEMO=1` 只打开面板，
        // `MW_SHORTCUTS_DEMO=save,undo` 顺带禁用列出的动作（只写内存，不落盘）。
        if let Some(raw) = std::env::var_os("MW_SHORTCUTS_DEMO") {
            shell.shortcuts_dialog.open();
            let raw = raw.to_string_lossy();
            if raw != "1" {
                for id in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    if let Some(action) = shortcuts::Action::from_id(id) {
                        shell.shortcuts.disable(action);
                    }
                }
            }
        }
        if std::env::var_os("MW_DIALOG_DEMO").is_some() {
            shell.dialog_demo = true;
        }
        // CER-02 组件陈列页：只服务 content-kit golden 与人工复核。
        // 不需要文档会话，主题由 MW_THEME 指定（默认 snow）。
        if std::env::var_os("MW_CONTENT_KIT").is_some() {
            shell.content_kit_demo = true;
            shell.home = false;
            shell.home_return = false;
        }
        // 标签条 golden / 行为脚本：三个固定标签——干净示例、有改动、未落盘空白。
        // `MW_TABS_DEMO=N`（N>3）追加「文档 4…N」，给横向溢出场景用。
        // 全程只在内存里，不写窗口状态、不碰磁盘。
        if let Ok(raw) = std::env::var("MW_TABS_DEMO") {
            let count = raw.trim().parse::<usize>().unwrap_or(3).max(3);
            let mut sample = Session::sample();
            let sample_doc = std::mem::replace(&mut sample.doc, mw_core::Document::new("中心主题"));
            let active = Session::fresh_named(sample_doc, "产品路线图");
            let mut dirty = Session::fresh_named(Session::blank().doc, "访谈纪要");
            let root = dirty.doc.root_id.clone();
            let _ = dirty.doc.apply(&mw_core::Command::SetText {
                id: root,
                text: "访谈纪要".into(),
            });
            let scratch = Session::fresh_named(Session::blank().doc, "灵感速记");
            let mut tabs = vec![Tab::new(active), Tab::new(dirty), Tab::new(scratch)];
            for i in 3..count {
                let title = format!("文档 {}", i + 1);
                tabs.push(Tab::new(Session::fresh_named(Session::blank().doc, &title)));
            }
            let session = std::mem::replace(&mut tabs[0].session, Session::blank());
            shell.tabs = tabs;
            shell.active = 0;
            if let Some(c) = shell.canvas.as_mut() {
                c.replace_session(session);
            }
            shell.home = false;
            shell.home_return = false;
            // 换会话会把上面 MW_THEME 的效果重置成文档自带主题，这里补一次。
            if let Ok(id) = std::env::var("MW_THEME") {
                if let Some(c) = shell.canvas.as_mut() {
                    let _ = c.apply_command(mw_core::Command::SetTheme { theme: id });
                }
            }
        }
        // 打开中的 golden 靶子：只画「正在打开」，永不读盘。
        if std::env::var_os("MW_OPENING_DEMO").is_some() {
            shell.open_plan = Some(OpenPlan {
                path: std::path::PathBuf::from("示例/产品路线图.zmind"),
                painted: false,
                demo: true,
            });
        }
        // 打开失败的 golden 靶子：通用示例路径，不涉及本机目录。
        if std::env::var_os("MW_OPEN_FAILED").is_some() {
            shell.open_failed = Some(OpenFailure {
                path: std::path::PathBuf::from("示例/已删除的产品路线图.zmind"),
                reason: "文件不存在或已被移动".into(),
                from_recent: true,
            });
        }
        if std::env::var_os("MW_THEME_STUDIO").is_some() {
            shell.home = false;
            shell.home_return = false;
            shell.onboarding = false;
            if !opened {
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(Session::sample());
                }
                opened = true;
            }
            shell.open_theme_studio();
        }
        if std::env::var_os("MW_LIBRARY_DEMO").is_some() {
            // golden 与实拍要可复现：用仓库内 `target/tmp` 的固定示例卡片，
            // 绝不读写用户的真实笔记库。场景名就在 `MW_LIBRARY_DEMO` 的取值里
            // （`docs/28` §7.10 十场），认不出的值当 `default`。
            crate::demo_notes::seed_scene(&mut shell);
        }
        if std::env::var_os("MW_EMPTY_DEMO").is_some() {
            if let Some(c) = shell.canvas.as_mut() {
                c.replace_session(Session::blank());
            }
        }
        // 视觉验收：`MW_OUTLINE=1` 启动即进全屏大纲，并保证有一份能看的样例内容。
        if std::env::var_os("MW_OUTLINE").is_some() {
            shell.home = false;
            shell.home_return = false;
            // `MW_OUTLINE_20K=1`：换成 20k 行性能夹具（CER-11）。
            if std::env::var_os("MW_OUTLINE_20K").is_some() {
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(Session::sample_20k());
                }
                opened = true;
            }
            // `MW_OUTLINE_WRAP=1`：长文折行夹具（docs/28 §5.2 非编辑态换行）。
            if std::env::var_os("MW_OUTLINE_WRAP").is_some() {
                if let Some(c) = shell.canvas.as_mut() {
                    c.replace_session(Session::sample_wrap());
                }
                opened = true;
            }
            if !opened {
                if let Some(c) = shell.canvas.as_mut() {
                    // 夹具补了备注与任务：三种行尾角标（任务/备注/标签）都要进实拍。
                    c.replace_session(Session::sample_with_marks());
                }
                opened = true;
            }
            // `MW_OUTLINE_LABELS=n`：给根节点补 n 个标签，实拍「弹层封顶 + 滚动」（docs/30 §5.11）。
            if let Ok(n) = std::env::var("MW_OUTLINE_LABELS") {
                if let Ok(n) = n.trim().parse::<usize>() {
                    if let Some(c) = shell.canvas.as_mut() {
                        // 每个节点有标签上限（`MAX_LABELS`），所以分摊到多行：
                        // 一行挂满就换下一行，直到攒够 n 个不同的标签。
                        let ids = c.session().doc.doc_order();
                        let mut ok = 0usize;
                        if !ids.is_empty() {
                            for i in 1..=n {
                                let id = ids[(i - 1) % ids.len()].clone();
                                if c.apply_command(mw_core::Command::AddLabel {
                                    ids: vec![id],
                                    label: format!("标签{i:02}"),
                                }) {
                                    ok += 1;
                                }
                            }
                        }
                        crate::trace::event(&format!(
                            "outline_labels_fixture n={} ok={} total={}",
                            n,
                            ok,
                            c.session()
                                .doc
                                .nodes
                                .values()
                                .map(|x| x.labels.len())
                                .sum::<usize>()
                        ));
                    }
                }
            }
            // 上一次运行可能把「大纲开着」记进了窗口状态；钩子要的是确定态，不是取反。
            shell.outline_view = false;
            shell.toggle_outline_view();
            // 视觉验收：`MW_OUTLINE_DENSITY=compact/comfortable`，截图矩阵用。
            if let Ok(d) = std::env::var("MW_OUTLINE_DENSITY") {
                use crate::panels::outline::OutlineDensity;
                shell.outline.set_density(match d.as_str() {
                    "compact" => OutlineDensity::Compact,
                    "comfortable" => OutlineDensity::Comfortable,
                    _ => OutlineDensity::Standard,
                });
            }
            // 换会话会把上面 MW_THEME 的效果重置成文档自带主题，这里补一次。
            if let Ok(id) = std::env::var("MW_THEME") {
                if let Some(c) = shell.canvas.as_mut() {
                    let _ = c.apply_command(mw_core::Command::SetTheme { theme: id });
                }
            }
            // 视觉验收：帮助表与文档信息弹层。
            if std::env::var_os("MW_OUTLINE_HELP").is_some() {
                shell.help.toggle_outline();
            }
            if std::env::var_os("MW_OUTLINE_INFO").is_some() {
                shell.outline.toggle_info();
            }
            // `MW_OUTLINE_SEARCH=词`：打开搜索行并预填，截条件搜索行与命中计数。
            if let Ok(q) = std::env::var("MW_OUTLINE_SEARCH") {
                shell.outline.open_search();
                shell.outline.set_filter(&q);
            }
            // `MW_OUTLINE_FILTER_TASK=todo|doing|done` / `_CARD=1` / `_LABEL=词`：
            // 打开筛选条并预置条件胶囊，截「条件 AND + 祖先上下文」。
            {
                let task =
                    std::env::var("MW_OUTLINE_FILTER_TASK")
                        .ok()
                        .and_then(|v| match v.as_str() {
                            "todo" => Some(mw_core::TaskState::Todo),
                            "doing" => Some(mw_core::TaskState::Doing),
                            "done" => Some(mw_core::TaskState::Done),
                            _ => None,
                        });
                let card = std::env::var("MW_OUTLINE_FILTER_CARD").is_ok_and(|v| v != "0");
                let label = std::env::var("MW_OUTLINE_FILTER_LABEL")
                    .ok()
                    .filter(|s| !s.is_empty());
                if task.is_some() || card || label.is_some() {
                    shell.outline.open_search();
                    shell
                        .outline
                        .set_filter_conditions(task, card, label.as_deref());
                }
                // `MW_OUTLINE_LABEL_POPUP=1`：把标签弹层直接打开（实拍用）。
                if std::env::var_os("MW_OUTLINE_LABEL_POPUP").is_some() {
                    shell.outline.open_search();
                    shell.outline.open_label_popup();
                }
            }
            // `MW_OUTLINE_SCROLL_TO=文字`：把该行滚到视野中央（20k 场景用；
            // 走 `scroll_to` 的离屏直滚，不依赖滚轮平滑动画，基线可复现）。
            if let Ok(target) = std::env::var("MW_OUTLINE_SCROLL_TO") {
                let id = shell.canvas.as_ref().and_then(|c| {
                    let doc = &c.session().doc;
                    doc.doc_order()
                        .into_iter()
                        .find(|id| doc.node(id).is_some_and(|n| n.text == target))
                });
                if let Some(id) = id {
                    crate::trace::event(&format!("outline_scroll_to id={id}"));
                    shell.outline.reveal(&id);
                } else {
                    crate::trace::event("outline_scroll_to target_missing");
                }
            }
            // `MW_OUTLINE_FOCUS=1`：聚焦样例里的「纯 Rust 原生」，截面包屑与子树视图。
            if std::env::var_os("MW_OUTLINE_FOCUS").is_some() {
                let target = shell.canvas.as_ref().and_then(|c| {
                    let doc = &c.session().doc;
                    doc.doc_order()
                        .into_iter()
                        .find(|id| doc.node(id).is_some_and(|n| n.text == "纯 Rust 原生"))
                });
                if let Some(id) = target {
                    if let Some(c) = shell.canvas.as_mut() {
                        c.select_node(&id);
                    }
                    shell.outline.focus_on(&id);
                    shell.outline.request_row_focus(&id);
                }
            }
            // `MW_OUTLINE_EDIT=文字`：选中并进入该行的行内编辑，截「编辑态就是行本身」。
            if let Ok(needle) = std::env::var("MW_OUTLINE_EDIT") {
                let target = shell.canvas.as_ref().and_then(|c| {
                    let doc = &c.session().doc;
                    doc.doc_order().into_iter().find(|id| {
                        doc.node(id)
                            .is_some_and(|n| needle.is_empty() || n.text.contains(&needle))
                    })
                });
                if let Some(id) = target {
                    if let Some(c) = shell.canvas.as_mut() {
                        c.select_node(&id);
                    }
                    // 行缓存要到下一帧才按新文档重建：先同步一次，编辑请求才找得到行。
                    if let Some(c) = shell.canvas.as_ref() {
                        shell.outline.sync(&c.session().doc, &c.session().theme);
                    }
                    shell.outline.begin_edit_at(&id, None, false);
                    // 进大纲时排在队里的行焦点请求要撤回：它会在本帧绘制之后执行，
                    // 把刚立起来的 TextEdit 光标抢回行上。
                    shell.outline.take_focus_request();
                }
            }
            // `MW_OUTLINE_SELECT=文字`：选中该行并把按键焦点交给大纲（CM-09 三张
            // 「大纲改结构」场景用）。与 `MW_OUTLINE_EDIT` 的区别是**不进编辑**：
            // 后续 `key` 要走大纲的键位分支（Tab / Shift+Delete / Shift+F2），
            // 进了 TextEdit 就全被输入框吃掉了。
            if let Ok(needle) = std::env::var("MW_OUTLINE_SELECT") {
                // 注意：概念节点浮在 `children` 之外（层级在 `relationships` 里），
                // `doc_order()` 走不到它们——按文字找概念必须扫全表。
                let target = shell.canvas.as_ref().and_then(|c| {
                    let doc = &c.session().doc;
                    doc.nodes
                        .values()
                        .find(|n| n.text == needle)
                        .map(|n| n.id.clone())
                });
                if let Some(id) = target {
                    crate::trace::event(&format!("outline_select id={id}"));
                    if let Some(c) = shell.canvas.as_mut() {
                        c.select_node(&id);
                    }
                    // 首帧会把「选中为空」回落到根行，所以这里只排一次意图：
                    // 真正每帧按回去的是 `raw_input_hook` 里的 `outline_pin`。
                    shell.outline_pin = Some(id);
                } else {
                    crate::trace::event("outline_select target_missing");
                }
            }
        }
        if let Ok(text) = std::env::var("MW_TOAST_DEMO") {
            shell.set_toast(text, false);
        }
        if !opened && shell.harness.is_none() {
            shell.restore_tabs();
        }
        shell
    }

    /// 把主题主色带进 egui 的焦点框、文本选中底色和链接色。
    ///
    /// egui 浅色主题默认给文本框一圈深蓝色焦点框；薄荷/深林文档里这一圈蓝比
    /// 任何排版瑕疵都显眼。`ui.visuals_mut()` 管当前帧（根 Ui 的子面板都继承它），
    /// `all_styles_mut` 管之后新建的弹窗。
    /// 把主题主色带进 egui 的焦点框、文本选中底色和链接色。
    ///
    /// egui 浅色主题默认给文本框一圈深蓝色焦点框；薄荷/深林文档里这一圈蓝比
    /// 任何排版瑕疵都显眼。`ui.visuals_mut()` 管当前帧（根 Ui 的子面板都继承它），
    /// `all_styles_mut` 管之后新建的弹窗。
    pub(crate) fn sync_accent(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let accent = self.canvas.as_ref().map(|c| {
            let a = c.session().theme.ui.accent;
            Color32::from_rgb(a.0[0], a.0[1], a.0[2])
        });
        if accent == self.ui_accent {
            return;
        }
        if let Some(accent) = accent {
            let tint = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 88);
            let v = ui.visuals_mut();
            v.selection.stroke = egui::Stroke::new(1.0, accent);
            v.selection.bg_fill = tint;
            v.hyperlink_color = accent;
            ctx.all_styles_mut(|s| {
                s.visuals.selection.stroke = egui::Stroke::new(1.0, accent);
                s.visuals.selection.bg_fill = tint;
                s.visuals.hyperlink_color = accent;
            });
        }
        self.ui_accent = accent;
    }

    /// 标签页显示名与脏标记。活动标签读画布（权威），其余读槽位。
    pub(crate) fn tab_label(&self, i: usize) -> (String, bool) {
        if i == self.active {
            match self.canvas.as_ref() {
                Some(c) => (c.session().title(), c.session().dirty()),
                None => ("未命名.zmind".into(), false),
            }
        } else {
            match self.tabs.get(i) {
                Some(t) => (t.session.title(), t.session.dirty()),
                None => ("未命名.zmind".into(), false),
            }
        }
    }

    /// 把当前画布的会话/相机/选择存回 `tabs[active]`（切页、新建、关闭前必做）。
    ///
    /// 用**对调**（`swap_session`）把画布里的权威会话搬进槽位；槽位里上一轮的陈旧值
    /// 顺带被换进画布，随后 `load_active` 会立刻用目标标签的会话覆盖它。
    /// 把当前画布的会话/相机/选择存回 `tabs[active]`（切页、新建、关闭前必做）。
    ///
    /// 用**对调**（`swap_session`）把画布里的权威会话搬进槽位；槽位里上一轮的陈旧值
    /// 顺带被换进画布，随后 `load_active` 会立刻用目标标签的会话覆盖它。
    pub(crate) fn stash_active(&mut self) {
        // 在途的保存必须先落地，否则槽位换走的是「还没写盘」的那份修订。
        self.pump_save();
        let Some(slot) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(c) = self.canvas.as_mut() else {
            return;
        };
        c.commit_editing();
        slot.camera = c.camera();
        slot.sel_ids = c.selected_ids();
        c.swap_session(&mut slot.session);
    }

    /// 把 `tabs[active]` 装回画布（切页/新建/关闭后调用）。
    pub(crate) fn load_active(&mut self) {
        {
            let Some(slot) = self.tabs.get_mut(self.active) else {
                return;
            };
            let cam = slot.camera;
            let sel = slot.sel_ids.clone();
            let Some(c) = self.canvas.as_mut() else {
                return;
            };
            c.swap_session(&mut slot.session);
            c.set_view(cam, &sel);
        }
        // 守卫跟着文档走：切到这一页，指纹/协作锁/只读态都要换成**这一份文档**的。
        // 以前这三项挂在窗口上、这里只重置 `save_phase`，于是切回 A 时 `Ctrl+S`
        // 拿的是 B 的指纹，弹假冲突（SR-02 问题 1）。
        self.arm_active_guard();
        // 切页当帧就把只读态灌进文档，别等到下一帧 `Shell::ui` 的同步——
        // 否则新页的第一下输入会落在「还没只读」的一帧里（X29 G-1）。
        let read_only = self.active_read_only();
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.set_read_only(read_only);
        }
        // 修订号是每份文档各自的；基线归零，脏的文档下一轮就能写恢复点。
        self.note_session_rev(0);
        // 保存阶段的失败只属于上一份文档；换页后不能把红点带到新文档上。
        self.save_phase = SavePhase::Idle;
    }

    /// 让活动标签的守卫与实际文档对齐（切页 / 换文档 / 换路径之后都要来一次）。
    pub(crate) fn arm_active_guard(&mut self) {
        let path = self.canvas.as_ref().and_then(|c| c.session().path.clone());
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.guard.arm(path.as_deref());
        }
    }

    /// 活动标签的守卫：所有「这份文档只读吗 / 磁盘指纹是什么」都从这里问。
    ///
    /// 窗口级字段退役后不存在第二种真相源，多标签也就不会再张冠李戴。
    pub(crate) fn guard(&self) -> Option<&TabGuard> {
        self.tabs.get(self.active).map(|t| &t.guard)
    }

    pub(crate) fn guard_mut(&mut self) -> Option<&mut TabGuard> {
        self.tabs.get_mut(self.active).map(|t| &mut t.guard)
    }

    /// 活动文档是否只读（另一个实例占着协作锁）。
    pub(crate) fn active_read_only(&self) -> bool {
        self.guard().is_some_and(|g| g.read_only)
    }

    /// 活动文档的已知磁盘指纹。
    pub(crate) fn active_print(&self) -> Option<file_guard::Fingerprint> {
        self.guard().and_then(|g| g.print)
    }

    /// 文档换代（打开/新建/恢复）或正本刚保存：把**这份文档**的自动保存基线设成
    /// `rev`。多标签下不能动其它槽位的基线，也不能 bump 全局 epoch——那会把别的
    /// 标签正在途的写结果判成过期。
    /// 文档换代（打开/新建/恢复）或正本刚保存：把**这份文档**的自动保存基线设成
    /// `rev`。多标签下不能动其它槽位的基线，也不能 bump 全局 epoch——那会把别的
    /// 标签正在途的写结果判成过期。
    pub(crate) fn note_session_rev(&mut self, rev: u64) {
        if let Some(s) = self.canvas.as_ref().map(|c| c.session()) {
            if let Some(slot) = autosave::slot_for(s) {
                self.last_autosave_rev.insert(slot, rev);
            }
        }
        self.sync_notes();
    }

    /// 把当前文档内嵌的卡片合并进本地库。文档是便携副本，库是工作副本：
    /// 只有文档那份更新时才覆盖库（`NoteStore::sync_document` 负责比较 updatedAt）。
    /// 把当前文档内嵌的卡片合并进本地库。文档是便携副本，库是工作副本：
    /// 只有文档那份更新时才覆盖库（`NoteStore::sync_document` 负责比较 updatedAt）。
    pub(crate) fn sync_notes(&mut self) {
        let (Some(canvas), Some(store)) = (self.canvas.as_ref(), self.notes.as_mut()) else {
            return;
        };
        let _ = store.sync_document(canvas.session().doc.cards());
    }

    /// 切到另一个标签页。搜索结果/命令面板的 id 属于旧文档，一律关掉。
    /// 把当前标签顺序/激活项落盘（未命名标签不记）。
    /// 切到另一个标签页。搜索结果/命令面板的 id 属于旧文档，一律关掉。
    /// 把当前标签顺序/激活项落盘（未命名标签不记）。
    pub(crate) fn persist_tabs(&mut self) {
        let active_path = self.canvas.as_ref().and_then(|c| c.session().path.clone());
        let paths: Vec<String> = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, tab)| {
                let p = if i == self.active {
                    active_path.as_ref()
                } else {
                    tab.session.path.as_ref()
                }?;
                Some(p.to_string_lossy().to_string())
            })
            .collect();
        self.window_state.remember_tabs(paths, self.active);
    }

    /// 启动时恢复上次的文件标签（只恢复仍存在的路径）。
    ///
    /// 每个路径先查有没有**比正本更新**的恢复点：以前这里直接 `Session::open`，
    /// 崩溃恢复在启动主路径上根本不生效——恢复点只有在「手动打开同一个文件」时
    /// 才会被问到（SR-02 问题 4）。
    pub(crate) fn restore_tabs(&mut self) -> bool {
        let paths = self.window_state.tab_paths().to_vec();
        if paths.is_empty() {
            return false;
        }
        let mut restored: Vec<Tab> = Vec::new();
        // (恢复点槽位, 正本路径)：活动标签交给既有的恢复提示，后台标签已经装进内存。
        let mut recovered: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        for p in &paths {
            let path = std::path::Path::new(p);
            if !path.is_file() {
                continue;
            }
            // 同一路径只恢复一次：窗口状态里有重复时两个标签会抢同一把协作锁，
            // 后一个必定被判成只读并把正本锁死。
            if restored
                .iter()
                .any(|t| t.session.path.as_deref() == Some(path))
            {
                continue;
            }
            if let Some((session, slot)) = restore_tab_session(path) {
                if let Some(slot) = slot {
                    recovered.push((slot, path.to_path_buf()));
                }
                restored.push(Tab::new(session));
            }
        }
        if restored.is_empty() {
            return false;
        }
        let saved_active = self.window_state.active_tab();
        self.tabs = restored;
        self.active = saved_active.min(self.tabs.len() - 1);
        let session = std::mem::replace(&mut self.tabs[self.active].session, Session::blank());
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.replace_session(session);
        }
        // 每个 `Tab::new` 已经各自武装过守卫；这里再对齐一次活动标签（画布是权威会话）。
        self.arm_active_guard();
        self.home = false;
        self.home_return = false;
        self.note_session_rev(0);
        // 活动标签的恢复点交给既有的恢复提示——跟「手动打开同一个文件」完全同一条动线。
        let active_path = self.canvas.as_ref().and_then(|c| c.session().path.clone());
        let active_slot = active_path.as_ref().and_then(|p| {
            recovered
                .iter()
                .find(|(_, doc)| doc == p)
                .map(|(slot, _)| (slot.clone(), p.clone()))
        });
        let others = recovered.len() - usize::from(active_slot.is_some());
        if let Some((slot, doc)) = active_slot {
            self.recovery = Some((slot, doc));
        }
        if others > 0 {
            // 后台标签没有可寻址的弹窗：内容已经装进内存（脏的），Ctrl+S 就写回正本。
            self.set_toast(
                format!("{others} 个标签已按更新的恢复点打开，Ctrl+S 写回原文件"),
                false,
            );
        } else {
            self.set_toast(format!("已恢复 {} 个标签页", self.tabs.len()), false);
        }
        true
    }

    pub(crate) fn switch_tab(&mut self, idx: usize) {
        if idx == self.active || idx >= self.tabs.len() {
            return;
        }
        // 上一帧刚按下的 Ctrl+S 还停在 «保存中»：先把它写完再换页，
        // 否则 AUTHORITATIVE 会话换成 B，A 的改动会写进 B 的路径。
        self.pump_save();
        self.stash_active();
        self.active = idx;
        self.load_active();
        self.search.close();
        self.palette.close();
        self.home = false;
        self.home_return = false;
        self.persist_tabs();
    }

    /// 新建一个空白标签页并切过去。
    pub(crate) fn new_tab(&mut self) {
        self.stash_active();
        self.tabs.push(Tab::new(Session::blank()));
        self.active = self.tabs.len() - 1;
        self.load_active();
        self.search.close();
        self.palette.close();
        self.home = false;
        self.home_return = false;
        self.set_toast("已新建标签页", false);
        self.persist_tabs();
    }

    /// 新建一张概念图标签页：快捷键 `Ctrl+Shift+N`、标题菜单「新建概念图」、模板第 6 张共用
    /// （`docs/30` §3.6）。开始页上用 `home_new_concept`——那里能复用空白页，不堆标签。
    pub(crate) fn new_concept_tab(&mut self) {
        self.stash_active();
        self.tabs.push(Tab::new(Session::blank_concept()));
        self.active = self.tabs.len() - 1;
        self.load_active();
        self.search.close();
        self.palette.close();
        self.home = false;
        self.home_return = false;
        crate::trace::event("home new concept source=shortcut");
        self.set_toast("已新建概念图", false);
        self.persist_tabs();
    }

    /// 关闭标签页（调用方负责脏数据的确认）。最后一个关掉后补一个空白页，
    /// 窗口不留空——同时把开始页推到前面，让用户有个"下一步"可点。
    /// 关闭标签页（调用方负责脏数据的确认）。最后一个关掉后补一个空白页，
    /// 窗口不留空——同时把开始页推到前面，让用户有个"下一步"可点。
    pub(crate) fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.stash_active();
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.tabs.push(Tab::new(Session::blank()));
            self.active = 0;
            self.load_active();
            self.home = true;
            self.home_return = false;
        } else {
            self.active = active_after_close(idx, self.active, self.tabs.len());
            self.load_active();
        }
        self.search.close();
        self.palette.close();
        crate::trace::event(&format!(
            "tab_close index={idx} remaining={} active={}",
            self.tabs.len(),
            self.active
        ));
        self.persist_tabs();
    }

    /// 标签页对应的磁盘路径。活动页以画布为准，其余读槽位。
    pub(crate) fn tab_path(&self, i: usize) -> Option<&std::path::Path> {
        if i == self.active {
            self.canvas
                .as_ref()
                .and_then(|c| c.session().path.as_deref())
        } else {
            self.tabs.get(i).and_then(|t| t.session.path.as_deref())
        }
    }

    /// 标签页右键菜单：只发 [`TabAction`]，不直接改文档。
    pub(crate) fn build_tab_menu(&self, i: usize, anchor: Pos2) -> Option<menu::Menu<TabAction>> {
        if i >= self.tabs.len() {
            return None;
        }
        let path = self.tab_path(i);
        let has_path = path.is_some();
        // 路径记着但文件已经被移走时，「定位」只会把资源管理器打开到空处。
        let can_reveal = path.is_some_and(|p| p.exists());
        let entries = vec![
            menu::Entry::action_with_icon("关闭", "Ctrl+W", Icon::Close, TabAction::Close(i), true),
            menu::Entry::action(
                "关闭其他",
                "",
                TabAction::CloseOthers(i),
                self.tabs.len() > 1,
            ),
            menu::Entry::separator(),
            menu::Entry::action_with_icon(
                "复制文件路径",
                "",
                Icon::File,
                TabAction::CopyPath(i),
                has_path,
            ),
            menu::Entry::action_with_icon(
                "在资源管理器中显示",
                "",
                Icon::Open,
                TabAction::Reveal(i),
                can_reveal,
            ),
        ];
        Some(menu::Menu::new(anchor, entries))
    }

    /// 标签条与右键菜单的共同落地点。菜单只发意图，切换/关闭/重排都在这里。
    pub(crate) fn run_tab_action(&mut self, ctx: &egui::Context, action: TabAction) {
        self.tab_menu = None;
        match action {
            TabAction::Switch(i) => self.switch_tab(i),
            TabAction::Close(i) => self.request_close_tab(i),
            TabAction::CloseOthers(i) => self.close_other_tabs(i),
            TabAction::NewTab => self.new_tab(),
            TabAction::CopyPath(i) => {
                let Some(path) = self.tab_path(i).map(std::path::Path::to_path_buf) else {
                    return;
                };
                ctx.copy_text(path.to_string_lossy().to_string());
                self.set_toast("已复制文件路径", false);
            }
            TabAction::Reveal(i) => {
                let Some(path) = self.tab_path(i).map(std::path::Path::to_path_buf) else {
                    return;
                };
                if let Err(e) = files::reveal_in_explorer(&path) {
                    self.set_toast(e, true);
                }
            }
            TabAction::Reorder(from, slot) => self.reorder_tab(from, slot),
        }
    }

    /// 拖拽重排：激活页跟着同一份文档走；插入槽按「移除前」的槽位解释。
    pub(crate) fn reorder_tab(&mut self, from: usize, slot: usize) {
        if from >= self.tabs.len() || self.tabs.len() < 2 {
            return;
        }
        self.stash_active();
        let new_active = crate::tab_strip::reorder_active(from, self.active, slot);
        crate::tab_strip::reorder(&mut self.tabs, from, slot);
        self.active = new_active;
        crate::trace::event(&format!(
            "tab_reorder from={from} slot={slot} active={new_active}"
        ));
        self.load_active();
        self.search.close();
        self.palette.close();
        self.persist_tabs();
    }

    /// 关闭其他标签页：有未保存改动就拒绝，避免静默丢内容。
    pub(crate) fn close_other_tabs(&mut self, keep: usize) {
        if keep >= self.tabs.len() || self.tabs.len() <= 1 {
            return;
        }
        let active_dirty = self.canvas.as_ref().is_some_and(|c| c.session().dirty());
        let dirty_other = self
            .tabs
            .iter()
            .enumerate()
            .any(|(i, t)| i != keep && i != self.active && t.session.dirty());
        if dirty_other || (keep != self.active && active_dirty) {
            self.set_toast("有标签页还没保存，先逐个关闭", true);
            return;
        }
        self.stash_active();
        let tab = self.tabs.remove(keep);
        self.tabs = vec![tab];
        self.active = 0;
        self.load_active();
        self.search.close();
        self.palette.close();
        self.persist_tabs();
    }

    /// 当前画布里的文档是否还是"什么都没做过"的空白页。
    pub(crate) fn active_pristine(&self) -> bool {
        self.canvas
            .as_ref()
            .is_some_and(|c| c.session().is_pristine())
    }

    /// 汉堡菜单「开始页」：盖住当前文档，但不关它。切标签/新建/打开都会自动离开。
    pub(crate) fn open_home(&mut self) {
        if self.home {
            return;
        }
        if let Some(c) = self.canvas.as_mut() {
            c.commit_editing();
        }
        self.search.close();
        self.palette.close();
        self.help.close();
        self.export.open = false;
        self.context_menu = None;
        self.tab_menu = None;
        self.tab_drag = None;
        self.home = true;
        self.home_return = true;
        self.start.request_new_focus();
    }

    /// 从开始页退回文档（只对"从菜单进入"有效）。
    pub(crate) fn leave_home(&mut self) {
        self.home = false;
        self.home_return = false;
        if let Some(c) = self.canvas.as_mut() {
            c.request_canvas_focus();
        }
    }

    /// 开始页上的「新建空白图」：手上就有一张没动过的空白页时**就地复用**，
    /// 否则另开一页。两种情况下都不碰已打开的其他文档。
    pub(crate) fn home_new_blank(&mut self) {
        self.home_new_with(Session::blank, "已新建思维导图");
    }

    /// 开始页上的「新建概念图」：复用规则与空白图一致，只是文档种类不同。
    pub(crate) fn home_new_concept(&mut self) {
        crate::trace::event(&format!(
            "home new concept kind={}",
            mw_core::layout::LayoutKind::ConceptMap.as_str()
        ));
        self.home_new_with(Session::blank_concept, "已新建概念图");
    }

    fn home_new_with(&mut self, make: fn() -> Session, toast: &str) {
        let session = make();
        let rev = session.doc.revision;
        let reuse = self.active_pristine();
        self.home = false;
        self.home_return = false;
        if reuse {
            if let Some(c) = self.canvas.as_mut() {
                c.replace_session(session);
            }
            if let Some(slot) = self.tabs.get_mut(self.active) {
                slot.sel_ids.clear();
            }
        } else {
            self.stash_active();
            self.tabs.push(Tab::new(session));
            self.active = self.tabs.len() - 1;
            self.load_active();
            self.persist_tabs();
        }
        self.note_session_rev(rev);
        if let Some(c) = self.canvas.as_mut() {
            c.request_canvas_focus();
        }
        self.set_toast(toast, false);
    }

    /// 开始页上的模板卡片。同样是"空白页就地复用、否则另开一页"。
    pub(crate) fn home_template(&mut self, idx: usize) {
        let Some(tpl) = start::Template::at(idx) else {
            return;
        };
        let session = tpl.session();
        let rev = session.doc.revision;
        let reuse = self.active_pristine();
        self.home = false;
        self.home_return = false;
        if reuse {
            if let Some(c) = self.canvas.as_mut() {
                c.replace_session(session);
            }
            if let Some(slot) = self.tabs.get_mut(self.active) {
                slot.sel_ids.clear();
            }
        } else {
            self.stash_active();
            self.tabs.push(Tab::new(session));
            self.active = self.tabs.len() - 1;
            self.load_active();
        }
        self.note_session_rev(rev);
        self.search.close();
        self.palette.close();
        if let Some(c) = self.canvas.as_mut() {
            c.request_canvas_focus();
        }
        self.set_toast(format!("已新建「{}」", tpl.name), false);
    }

    pub(crate) fn run_home(&mut self, action: start::Action) {
        match action {
            start::Action::NewBlank => self.home_new_blank(),
            start::Action::NewConcept => self.home_new_concept(),
            start::Action::OpenFile => self.action_open(),
            start::Action::Return => self.leave_home(),
            start::Action::OpenRecent(path) => self.request(PendingAction::Open(path)),
            start::Action::Template(i) => self.home_template(i),
            start::Action::CardLibrary => self.open_library(),
            start::Action::ClearRecents => {
                self.recent.clear();
                self.set_toast("已清除最近文件列表", false);
            }
        }
    }

    /// 关闭标签页前先看脏不脏：脏就弹确认，干净就直接关。
    pub(crate) fn request_close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        let dirty = if idx == self.active {
            self.canvas.as_ref().is_some_and(|c| c.session().dirty())
        } else {
            self.tabs[idx].session.dirty()
        };
        if dirty {
            self.confirm_close_tab = Some(idx);
        } else {
            self.close_tab(idx);
        }
    }

    /// 相对当前位置循环切页（`delta` = ±1）。
    pub(crate) fn cycle_tab(&mut self, delta: isize) {
        let n = self.tabs.len();
        if n <= 1 {
            return;
        }
        self.switch_tab(next_tab(self.active, delta, n));
    }

    /// 已打开同一份文件时返回它的标签页下标（路径按规范形式比对，避免 `.\a.zmind` 之类误判）。
    pub(crate) fn tab_index_of_path(&self, path: &std::path::Path) -> Option<usize> {
        let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        (0..self.tabs.len()).find(|&i| {
            let have = if i == self.active {
                self.canvas.as_ref().and_then(|c| c.session().path.clone())
            } else {
                self.tabs[i].session.path.clone()
            };
            match have {
                Some(p) => std::fs::canonicalize(&p).unwrap_or(p) == want,
                None => false,
            }
        })
    }

    /// 标题栏要显示的名字与脏标记。会话在画布里，这里只读。
    pub(crate) fn doc_state(&self) -> (String, bool) {
        if self.home {
            return ("开始页".into(), false);
        }
        match self.canvas.as_ref() {
            Some(c) => (c.session().title(), c.session().dirty()),
            None => ("未命名.zmind".into(), false),
        }
    }

    /// 标题栏保存状态：区分“从未落盘”和“已写入文件”，再叠「保存中/失败」阶段。
    pub(crate) fn save_state(&self) -> crate::app_helpers::SaveState {
        if self.home {
            return crate::app_helpers::SaveState::Home;
        }
        let session = self.canvas.as_ref().map(|c| c.session());
        let rev = session.map_or(0, |s| s.doc.revision);
        let base = crate::app_helpers::SaveState::of(
            false,
            session.is_some(),
            session.is_some_and(|s| s.dirty()),
            session.is_some_and(|s| s.path.is_some()),
        );
        crate::app_helpers::SaveState::with_phase(base, &self.save_phase, rev)
    }

    /// 任务栏 / Alt+Tab 的窗口标题跟随当前文档；开始页只显示产品名。
    pub(crate) fn sync_window_title(&mut self, ctx: &egui::Context) {
        let want = if self.home {
            "织思".to_string()
        } else {
            format!("{} — 织思", self.doc_state().0)
        };
        if self.window_title.as_deref() != Some(want.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(want.clone()));
            self.window_title = Some(want);
        }
    }

    pub(crate) fn set_toast(&mut self, text: impl Into<String>, bad: bool) {
        let text = text.into();
        // 提示条 2.4 秒后自己消失，不拍进 golden（慢机器上会时有时无），证据看这行轨迹。
        crate::trace::event(&format!("toast bad={bad} text={text}"));
        self.toast = Some(Toast {
            text,
            until: None, // 第一帧再盖时间：这里没有 ctx
            bad,
        });
    }

    /// 知识视图：把卡片元数据铺成 卡片—标签—主题 关系图。
    pub(crate) fn open_knowledge(&mut self) {
        let cards = self
            .notes
            .as_ref()
            .map(|n| n.graph_input())
            .unwrap_or_default();
        if cards.is_empty() {
            self.set_toast("笔记库还是空的，先写几张卡片", true);
            return;
        }
        let start = std::time::Instant::now();
        let graph = knowledge::build(&cards);
        let ms = start.elapsed().as_millis();
        let nodes = graph.nodes.len();
        self.knowledge = Some(KnowledgeView {
            graph,
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
        });
        self.set_toast(format!("{nodes} 个节点 · 布局 {ms}ms"), false);
    }

    /// 导出学习合集：导图大纲 + 当前文档卡片 + 库内其它卡片，Markdown 与可打印 HTML。
    pub(crate) fn export_bundle(&mut self) {
        let Some(canvas) = self.canvas.as_ref() else {
            self.set_toast("没有可导出的文档", true);
            return;
        };
        let doc = &canvas.session().doc;
        let extra: Vec<mw_core::NoteCard> = self
            .notes
            .as_ref()
            .map(|n| n.ids().iter().filter_map(|id| n.get(id)).collect())
            .unwrap_or_default();
        let bundle = collection::collect(doc, &extra);
        let svg = export_svg::render(
            &canvas.export_scene(false),
            &canvas.session().theme,
            &canvas.session().doc.assets,
            export_svg::Options {
                transparent: false,
                bounds: None,
            },
        );
        let bundle = bundle.with_map(svg);
        if bundle.cards.is_empty() && bundle.outline.is_empty() {
            self.set_toast("没有可导出的内容", true);
            return;
        }
        let dir = std::env::var("USERPROFILE")
            .map(|p| std::path::PathBuf::from(p).join("Desktop"))
            .unwrap_or_else(|_| std::env::temp_dir());
        let md = dir.join("织思学习合集.md");
        let html = dir.join("织思学习合集.html");
        let result = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(&md, bundle.markdown()))
            .and_then(|_| std::fs::write(&html, bundle.html()));
        match result {
            Ok(()) => self.set_toast(
                format!(
                    "已导出 {} 张卡片到桌面的「织思学习合集」",
                    bundle.cards.len()
                ),
                false,
            ),
            Err(e) => self.set_toast(format!("导出失败：{e}"), true),
        }
    }

    /// 扫描当前文档、各标签与 AppData 下的恢复点。
    pub(crate) fn open_recovery_center(&mut self) {
        let mut roots: Vec<std::path::PathBuf> = Vec::new();
        if let Some(d) = paths::appdata_root() {
            roots.push(d.join("autosave"));
        }
        let mut add_parent = |p: &std::path::Path| {
            if let Some(parent) = p.parent() {
                if !roots.iter().any(|r| r == parent) {
                    roots.push(parent.to_path_buf());
                }
            }
        };
        if let Some(c) = self.canvas.as_ref() {
            if let Some(p) = c.session().path.as_ref() {
                add_parent(p);
            }
        }
        for t in &self.tabs {
            if let Some(p) = t.session.path.as_ref() {
                add_parent(p);
            }
        }
        let mut points = autosave::scan(&roots);
        points = autosave::prune(&points, 5);
        if points.is_empty() {
            self.set_toast("没有可恢复的自动保存", false);
            return;
        }
        self.recovery_center = Some(points);
    }

    pub(crate) fn recovery_ui(&mut self, ctx: &egui::Context) {
        let Some(points) = self.recovery_center.clone() else {
            return;
        };
        let mut restore: Option<autosave::RecoveryPoint> = None;
        let mut discard: Option<autosave::RecoveryPoint> = None;
        let actions = [dialog::DialogAction::new(
            "关闭",
            ui_kit::ButtonTone::Secondary,
        )];
        let spec = dialog::DialogSpec::new("恢复中心")
            .width(520.0)
            .default_action(0);
        let out = dialog::show(ctx, self.chrome, "recovery", spec, &actions, |ui| {
            ui.label(format!(
                "共 {} 个恢复点（每个文档保留最近 5 份）",
                points.len()
            ));
            ui.separator();
            for p in &points {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&p.title).size(Type::BODY).family(
                            eframe::egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()),
                        ));
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {:.1} KB",
                                p.slot
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("恢复点"),
                                p.bytes as f64 / 1024.0
                            ))
                            .size(Type::CAPTION)
                            .color(self.chrome.dim),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("丢弃").clicked() {
                            discard = Some(p.clone());
                        }
                        if ui.button("恢复").clicked() {
                            restore = Some(p.clone());
                        }
                    });
                });
                ui.separator();
            }
        });
        if out.closed {
            self.recovery_center = None;
        } else if let Some(p) = discard {
            let _ = std::fs::remove_file(&p.slot);
            if let Some(list) = self.recovery_center.as_mut() {
                list.retain(|x| x.slot != p.slot);
            }
            self.set_toast("已丢弃恢复点", false);
        } else if let Some(p) = restore {
            let loaded = mw_format::load(&p.slot).map(|l| l.doc);
            match loaded {
                Ok(doc) => {
                    let session = Session::from_doc(doc, p.doc.clone());
                    let idx = self.tabs.iter().position(|t| t.session.path == p.doc);
                    if let Some(idx) = idx {
                        self.switch_tab(idx);
                        if let Some(canvas) = self.canvas.as_mut() {
                            canvas.replace_session(session);
                        }
                    } else {
                        self.tabs.push(Tab::new(session));
                        self.active = self.tabs.len() - 1;
                        self.load_active();
                    }
                    self.home = false;
                    self.recovery_center = None;
                    self.set_toast("已恢复到对应标签页", false);
                }
                Err(e) => self.set_toast(format!("恢复失败：{e}"), true),
            }
        }
    }

    /// 外部修改冲突：重载 / 覆盖 / 取消。只对当前文件生效。
    pub(crate) fn conflict_ui(&mut self, ctx: &egui::Context) {
        let Some(path) = self.conflict.clone() else {
            return;
        };
        let actions = [
            dialog::DialogAction::new("重新加载", ui_kit::ButtonTone::Danger),
            dialog::DialogAction::new("覆盖保存", ui_kit::ButtonTone::Primary),
            dialog::DialogAction::new("取消", ui_kit::ButtonTone::Secondary),
        ];
        let spec = dialog::DialogSpec::new("文件已被外部修改")
            .width(440.0)
            .default_action(2);
        let out = dialog::show(ctx, self.chrome, "conflict", spec, &actions, |ui| {
            ui.label(
                "磁盘上的文件在打开后发生了变化。重载会丢掉本地未保存的修改；\
                 覆盖会保留本地内容并写回磁盘。",
            );
        });
        let reload = out.action == Some(0);
        let overwrite = out.action == Some(1);
        let cancel = out.closed && out.action.is_none();
        if cancel {
            self.conflict = None;
        } else if reload {
            // 「重新加载」以前直接 `start_open` → `load_open`，绕开了唯一那道脏检查
            // （它只守在 `request()` 里），当前标签的未保存改动就这样静默消失。
            // SR-02 问题 1 的后果链断在这里：先问，再换。
            self.conflict = None;
            self.request_reload(&path);
        } else if overwrite {
            self.conflict = None;
            let mut ok = false;
            if let Some(canvas) = self.canvas.as_mut() {
                canvas.commit_editing();
                ok = canvas.save().is_ok();
            }
            if ok {
                if let Some(g) = self.guard_mut() {
                    g.refresh_print();
                }
                // 覆盖也是一次写盘成功：被覆盖掉的那一版该进 `.history`，
                // 恢复点该作废，最近文件与基线该刷新（SR-02 问题 5）。
                // 以前这里只刷了指纹和一句提示，被顶掉的内容再也找不回来。
                self.after_saved();
                self.set_toast("已覆盖磁盘上的版本", false);
            }
        }
    }

    /// 学习统计：只读本地索引与复习日志，不采集网络数据。
    pub(crate) fn stats_ui(&mut self, ctx: &egui::Context) {
        if !self.stats_open {
            return;
        }
        let today = review::local_today();
        let stats = self.review_store.stats(today);
        let cards = self.notes.as_ref().map_or(0, |n| n.len());
        let actions = [dialog::DialogAction::new(
            "关闭",
            ui_kit::ButtonTone::Secondary,
        )];
        let spec = dialog::DialogSpec::new("学习统计")
            .width(380.0)
            .default_action(0);
        let out = dialog::show(ctx, self.chrome, "stats", spec, &actions, |ui| {
            for (label, value) in [
                ("卡片", cards),
                ("总复习", stats.total),
                ("今日复习", stats.today),
                ("连续天数", stats.streak),
            ] {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .size(Type::CAPTION)
                            .color(self.chrome.dim),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(value.to_string()).size(22.0).family(
                            eframe::egui::FontFamily::Name(crate::fonts::SEMIBOLD.into()),
                        ));
                    });
                });
                ui.add_space(6.0);
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("全部在本机复算（review.json + 卡片索引），不联网。")
                    .size(Type::CAPTION)
                    .color(self.chrome.dim),
            );
        });
        if out.closed {
            self.stats_open = false;
        }
    }

    pub(crate) fn knowledge_ui(&mut self, ctx: &egui::Context) {
        let Some(mut view) = self.knowledge.take() else {
            return;
        };
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            return;
        }
        let mut close = false;
        let mut open_card = None;
        egui::Window::new("知识视图")
            .collapsible(false)
            .resizable(true)
            .default_size([940.0, 660.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} 个节点 · {} 条关系",
                            view.graph.nodes.len(),
                            view.graph.edges.len()
                        ))
                        .size(Type::CAPTION)
                        .color(self.chrome.dim),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("关闭（Esc）").clicked() {
                            close = true;
                        }
                    });
                });
                let (rect, resp) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                if resp.dragged() {
                    view.pan += resp.drag_delta();
                }
                if resp.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll.abs() > 0.1 {
                        view.zoom = (view.zoom * (scroll * 0.0018).exp()).clamp(0.2, 4.0);
                    }
                }
                let (bx, by, bw, bh) = view.graph.bounds;
                let scale = ((rect.width() - 40.0) / (bw.max(1.0)))
                    .min((rect.height() - 40.0) / bh.max(1.0))
                    * view.zoom;
                let cx = (bx + bw * 0.5) * scale;
                let cy = (by + bh * 0.5) * scale;
                let map = |x: f32, y: f32| {
                    egui::Pos2::new(
                        rect.center().x + x * scale - cx + view.pan.x,
                        rect.center().y + y * scale - cy + view.pan.y,
                    )
                };
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 8.0, self.chrome.panel2);
                let edge_color = self.chrome.border;
                for e in &view.graph.edges {
                    let (a, b) = (&view.graph.nodes[e.from], &view.graph.nodes[e.to]);
                    painter.line_segment(
                        [map(a.x, a.y), map(b.x, b.y)],
                        egui::Stroke::new(0.8, edge_color),
                    );
                }
                let mut hit = None;
                for n in &view.graph.nodes {
                    let p = map(n.x, n.y);
                    if !rect.contains(p) {
                        continue;
                    }
                    let (color, r) = match n.kind {
                        knowledge::Kind::Card => (self.chrome.accent, 5.0 * view.zoom),
                        knowledge::Kind::Tag => (self.chrome.warn, 7.0 * view.zoom),
                        knowledge::Kind::Topic => (self.chrome.dim, 6.0 * view.zoom),
                    };
                    painter.circle_filled(p, r.max(2.0), color);
                    if view.zoom >= 0.75 {
                        painter.text(
                            p + egui::Vec2::new(0.0, r + 8.0),
                            egui::Align2::CENTER_TOP,
                            &n.label,
                            egui::FontId::proportional(Type::CAPTION),
                            self.chrome.text,
                        );
                    }
                    if resp.hover_pos().is_some_and(|h| h.distance(p) < 14.0) {
                        hit = Some(n);
                    }
                }
                if let Some(n) = hit {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    if resp.clicked() && n.kind == knowledge::Kind::Card {
                        open_card = Some(n.id.trim_start_matches("card:").to_string());
                    }
                }
            });
        if close {
            return;
        }
        if let Some(id) = open_card {
            self.open_library_card(&id);
            return;
        }
        self.knowledge = Some(view);
    }

    /// 收集今天到期的节点/卡片（最多 20 条），开一次复习。
    pub(crate) fn start_review(&mut self) {
        let now = mw_core::now_ms();
        let mut items: Vec<(i64, ReviewItem)> = Vec::new();
        if let Some(canvas) = self.canvas.as_ref() {
            let doc = &canvas.session().doc;
            for id in doc.doc_order() {
                let Some(n) = doc.node(&id) else { continue };
                let answer = n.note.clone().unwrap_or_default();
                if answer.trim().is_empty() {
                    continue;
                }
                let key = format!("n:{id}");
                if !self.review_store.is_due(&key, now) {
                    continue;
                }
                let due = self.review_store.get(&key).map_or(0, |s| s.due);
                items.push((
                    due,
                    ReviewItem {
                        key,
                        title: n.text.clone(),
                        answer,
                    },
                ));
            }
        }
        if let Some(notes) = self.notes.as_ref() {
            for id in notes.ids() {
                let key = format!("c:{id}");
                if !self.review_store.is_due(&key, now) {
                    continue;
                }
                let Some(card) = notes.get(&id) else { continue };
                let due = self.review_store.get(&key).map_or(0, |s| s.due);
                items.push((
                    due,
                    ReviewItem {
                        key,
                        title: card.display_title().to_string(),
                        answer: card.body.clone(),
                    },
                ));
            }
        }
        if items.is_empty() {
            self.set_toast("今天没有到期的复习", false);
            return;
        }
        items.sort_by_key(|(due, _)| *due);
        let items = items.into_iter().take(20).map(|(_, i)| i).collect();
        self.review = Some(ReviewSession {
            items,
            at: 0,
            revealed: false,
        });
    }

    pub(crate) fn review_ui(&mut self, ctx: &egui::Context) {
        let Some(mut session) = self.review.take() else {
            return;
        };
        if session.at >= session.items.len() {
            self.review = None;
            self.set_toast("复习完成", false);
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.review = None;
            return;
        }
        let item = &session.items[session.at];
        let key = item.key.clone();
        let total = session.items.len();
        let at = session.at;
        let mut grade = None;
        egui::Area::new(egui::Id::unique("mw-review"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(self.chrome.panel)
                    .stroke(egui::Stroke::new(1.0, self.chrome.border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(28, 22))
                    .show(ui, |ui| {
                        ui.set_max_width(460.0);
                        ui.label(
                            egui::RichText::new(format!("复习 {}/{}", at + 1, total))
                                .size(Type::CAPTION)
                                .color(self.chrome.dim),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(if item.title.trim().is_empty() {
                                "未命名"
                            } else {
                                item.title.trim()
                            })
                            .size(20.0)
                            .family(eframe::egui::FontFamily::Name(
                                crate::fonts::SEMIBOLD.into(),
                            )),
                        );
                        ui.add_space(18.0);
                        if session.revealed {
                            ui.label(
                                egui::RichText::new(item.answer.trim())
                                    .size(Type::BODY)
                                    .color(self.chrome.text),
                            );
                            ui.add_space(20.0);
                            ui.horizontal(|ui| {
                                for (g, label) in [
                                    (review::Grade::Again, "再来"),
                                    (review::Grade::Hard, "困难"),
                                    (review::Grade::Good, "良好"),
                                    (review::Grade::Easy, "简单"),
                                ] {
                                    if ui.button(label).clicked() {
                                        grade = Some(g);
                                    }
                                }
                            });
                        } else {
                            ui.label(
                                egui::RichText::new("先在心里回答，再翻面。")
                                    .size(Type::SMALL)
                                    .color(self.chrome.dim),
                            );
                            ui.add_space(16.0);
                            if ui
                                .add(egui::Button::new("显示答案").fill(self.chrome.accent))
                                .clicked()
                            {
                                session.revealed = true;
                            }
                        }
                    });
            });
        if let Some(g) = grade {
            self.review_store.grade(&key, g);
            session.at += 1;
            session.revealed = false;
            if session.at >= session.items.len() {
                self.review = None;
                self.set_toast("复习完成", false);
            } else {
                self.review = Some(session);
            }
            return;
        }
        self.review = Some(session);
    }

    /// F10：阅读模式。隐藏大纲/检查器/悬浮工具条，退出恢复相机、选择与面板。
    pub(crate) fn toggle_reading(&mut self) {
        if self.reading.is_some() {
            self.exit_reading();
            return;
        }
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        self.reading = Some(ReadingRestore {
            camera: canvas.camera(),
            selection: canvas.selected_ids(),
            panels: self.panel_states(),
        });
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.set_reading(true);
        }
        self.outline_view = false;
        self.show_inspector = false;
        self.set_toast("阅读模式 · F10 / Esc 退出", false);
    }

    pub(crate) fn exit_reading(&mut self) {
        let Some(saved) = self.reading.take() else {
            return;
        };
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.set_reading(false);
            canvas.set_camera(saved.camera);
            canvas.select_ids(&saved.selection);
        }
        self.outline_view = saved.panels.0;
        self.show_inspector = saved.panels.1;
    }

    /// F11：进入/退出「专注模式」（铺满当前显示器 + 置顶）。
    ///
    /// **不用** `ViewportCommand::Fullscreen`，也**不用** egui 的
    /// `InnerSize`/`OuterPosition`：它们最终走 winit 的
    /// `SetWindowPos(..., SWP_ASYNCWINDOWPOS)`，在 Windows + DX12 交换链上 DWM
    /// 有时不刷新这层 composition——egui 画面、wgpu 表面都已按新尺寸重画，屏幕却
    /// 停在旧的 1620×1080 画面，直到一次同步的 resize 或最小化/还原才恢复
    /// （docs/14 教训 53）。这里自己发一次**同步**的 `SetWindowPos`（尺寸、位置、
    /// 置顶一次给全），实测显示即时正确。置顶用来盖住任务栏；失焦时撤掉。
    /// F11：进入/退出「专注模式」（铺满当前显示器 + 置顶）。
    ///
    /// **不用** `ViewportCommand::Fullscreen`，也**不用** egui 的
    /// `InnerSize`/`OuterPosition`：它们最终走 winit 的
    /// `SetWindowPos(..., SWP_ASYNCWINDOWPOS)`，在 Windows + DX12 交换链上 DWM
    /// 有时不刷新这层 composition——egui 画面、wgpu 表面都已按新尺寸重画，屏幕却
    /// 停在旧的 1620×1080 画面，直到一次同步的 resize 或最小化/还原才恢复
    /// （docs/14 教训 53）。这里自己发一次**同步**的 `SetWindowPos`（尺寸、位置、
    /// 置顶一次给全），实测显示即时正确。置顶用来盖住任务栏；失焦时撤掉。
    pub(crate) fn toggle_focus_mode(&mut self, ctx: &egui::Context) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOPMOST, SWP_SHOWWINDOW, SW_MAXIMIZE,
            SW_RESTORE,
        };

        let Some(h) = self.hwnd else { return };
        let hwnd = HWND(h as *mut core::ffi::c_void);

        if let Some(saved) = self.focus_mode.take() {
            let (x, y, w, ht) = saved.rect;
            unsafe {
                // 先回到普通态矩形再去最大化：系统会把这一刻的位置记成「还原位置」。
                let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), x, y, w, ht, SWP_SHOWWINDOW);
                if saved.maximized {
                    let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                }
            }
            Shell::set_window_corner(self.hwnd, true);
            return;
        }

        let Some(mon) = Shell::monitor_rect(h) else {
            return;
        };
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        unsafe {
            // 最大化窗口的 SetWindowPos 只改「还原尺寸」，先还原再铺满。
            // 还原的这一刻也顺手量出**普通态**尺寸：退出时要靠它把窗口放回原位，
            // 直接记最大化时的外框会把「还原位置」一起改掉。
            if maximized {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let Some((x, y, w, ht)) = Shell::window_rect(h) else {
                return;
            };
            self.focus_mode = Some(FocusRestore {
                rect: (x, y, w, ht),
                maximized,
            });
            self.focus_prev_focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                mon.0,
                mon.1,
                mon.2,
                mon.3,
                SWP_SHOWWINDOW,
            );
        }
        Shell::set_window_corner(self.hwnd, false);
    }

    /// 窗口外框的物理像素 `(x, y, w, h)`。
    pub(crate) fn window_rect(h: isize) -> Option<(i32, i32, i32, i32)> {
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let mut r = RECT::default();
        unsafe { GetWindowRect(HWND(h as *mut core::ffi::c_void), &mut r).ok()? };
        Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
    }

    /// 窗口所在显示器的完整矩形（物理像素，含任务栏区域）。
    pub(crate) fn monitor_rect(h: isize) -> Option<(i32, i32, i32, i32)> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };
        unsafe {
            let mon =
                MonitorFromWindow(HWND(h as *mut core::ffi::c_void), MONITOR_DEFAULTTONEAREST);
            if mon.is_invalid() {
                return None;
            }
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if !GetMonitorInfoW(mon, &mut info).as_bool() {
                return None;
            }
            let r = info.rcMonitor;
            Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
        }
    }

    /// 置顶开关。走原生 `SetWindowPos`：winit 缓存里的 level 仍是 Normal
    /// （进专注模式时我们绕过了它），用 `WindowLevel` 命令会因「状态没变」被跳过。
    /// 置顶开关。走原生 `SetWindowPos`：winit 缓存里的 level 仍是 Normal
    /// （进专注模式时我们绕过了它），用 `WindowLevel` 命令会因「状态没变」被跳过。
    pub(crate) fn set_topmost(hwnd: Option<isize>, on: bool) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        };
        let Some(h) = hwnd else { return };
        unsafe {
            let _ = SetWindowPos(
                HWND(h as *mut core::ffi::c_void),
                Some(if on { HWND_TOPMOST } else { HWND_NOTOPMOST }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    /// DWM 圆角开关。专注模式铺满整屏时圆角会在屏幕四角露出桌面，须切直角。
    pub(crate) fn set_window_corner(hwnd: Option<isize>, round: bool) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
        };
        let Some(h) = hwnd else { return };
        let pref: i32 = if round {
            DWMWCP_ROUND.0
        } else {
            DWMWCP_DONOTROUND.0
        };
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(h as *mut core::ffi::c_void),
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const i32 as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    /// 恢复上次的窗口几何 + 最大化状态。只在首次拿到 HWND 后跑一次。
    pub(crate) fn restore_window_state(&mut self, ctx: &egui::Context) {
        if self.geometry_restored {
            return;
        }
        let Some(h) = self.hwnd else {
            return;
        };
        self.geometry_restored = true;
        if let Some(rect) = self.window_state.restorable_rect() {
            window_state::apply_rect(h, rect);
            // 恢复成功就别再发默认尺寸/位置命令，否则会把刚搬好的窗口又推回去。
            self.sized_to_monitor = true;
            if self.window_state.maximized() {
                // 最大化交给 winit：它同时维护自己的状态，标题栏双击/按钮才不会打架。
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
            self.refit_until =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
        }
    }

    pub(crate) fn panel_states(&self) -> (bool, bool) {
        (self.outline_view, self.show_inspector)
    }

    /// 打开重浮层前先清掉轻浮层，保证同一时刻只有一个抢指针。
    pub(crate) fn close_light_overlays(&mut self) {
        self.context_menu = None;
        self.palette.close();
        self.search.close();
        self.tag_manager = None;
        self.library_dialog = None;
        // 文档信息挂在大纲面板上；换成阅读页/笔记库后它不该继续占着模态栈。
        self.outline.close_info();
    }

    /// 当前浮层状态（P28）。画布锁输入、Esc 退层都以它为准。
    pub(crate) fn overlay_state(&self) -> overlay::State {
        let state = overlay::State {
            context: self.context_menu.is_some()
                || self.tab_menu.is_some()
                || self.outline_row_menu.is_some(),
            palette: self.palette.is_open(),
            search: self.search.is_open(),
            export: self.export.open,
            help: self.help.is_open(),
            shortcuts: self.shortcuts_dialog.is_open(),
            tag_manager: self.tag_manager.is_some(),
            library_dialog: self.library_dialog.is_some(),
            library: self.library.is_some(),
            reader: self.note_reader.is_some(),
            review: self.review.is_some(),
            knowledge: self.knowledge.is_some(),
            recovery: self.recovery.is_some(),
            stats: self.stats_open,
            import_warnings: self.import_warnings.is_some(),
            onboarding: self.onboarding,
            settings: self.settings_open,
            theme_studio: self.theme_studio.is_some(),
            history: self.history.is_some(),
            recovery_center: self.recovery_center.is_some(),
            conflict: self.conflict.is_some(),
            close_confirm: self.confirm_close,
            discard_confirm: self.pending.is_some(),
            tab_close_confirm: self.confirm_close_tab.is_some(),
            import_busy: self.import_job.is_some(),
            opening: self.open_plan.is_some(),
            open_failed: self.open_failed.is_some(),
            outline_info: self.outline_view && self.outline.info_open(),
            library_remove: self
                .library
                .as_ref()
                .is_some_and(|library| library.remove_prompt_open()),
        };
        if crate::trace::is_enabled() && state.open_count() > 1 {
            crate::trace::event(&format!(
                "overlay-double {} {:?}",
                state.open_count(),
                state.top()
            ));
        }
        if crate::trace::is_enabled() && state.modal_count() > 1 {
            crate::trace::event(&format!(
                "modal-stack {} {:?}",
                state.modal_count(),
                state.top_modal()
            ));
        }
        state
    }

    /// 防抖写窗口状态（约 700ms 一次）。专注模式下的窗口几何是临时的，只存面板开关。
    pub(crate) fn geometry_tick(&mut self) {
        if self.harness.is_some() || !self.geometry_restored {
            return;
        }
        let now = std::time::Instant::now();
        if now < self.next_geometry_save {
            return;
        }
        self.next_geometry_save = now + std::time::Duration::from_millis(700);
        let placement = match (self.focus_mode.is_some(), self.hwnd) {
            (false, Some(h)) => window_state::read_placement(h),
            _ => None,
        };
        let (rect, maximized) = match placement {
            Some((r, m)) => (Some(r), m),
            None => (None, false),
        };
        self.window_state
            .remember(rect, maximized, self.panel_states());
    }

    /// 退出时兜底：把最后一次移动/关面板写下去。被 taskkill 时走不到这里，
    /// 但正常关窗（含 Alt+F4）都会。
    /// 退出时兜底：把最后一次移动/关面板写下去。被 taskkill 时走不到这里，
    /// 但正常关窗（含 Alt+F4）都会。
    pub(crate) fn save_window_state_now(&mut self) {
        if self.harness.is_some() {
            return;
        }
        let placement = match (self.focus_mode.is_some(), self.hwnd) {
            (false, Some(h)) => window_state::read_placement(h),
            _ => None,
        };
        let (rect, maximized) = match placement {
            Some((r, m)) => (Some(r), m),
            None => (None, false),
        };
        self.window_state
            .remember(rect, maximized, self.panel_states());
    }

    /// 窗口级快捷键。**不要求画布有焦点**，编辑中按 Ctrl+S 也该存。
    ///
    /// 用 `consume_key` 而不是 `key_pressed`：同一帧画布的 `shortcuts` 也在读键盘，
    /// 不消费掉就可能被更晚的判断再处理一次。
    /// 窗口级快捷键。**不要求画布有焦点**，编辑中按 Ctrl+S 也该存。
    ///
    /// 用 `consume_key` 而不是 `key_pressed`：同一帧画布的 `shortcuts` 也在读键盘，
    /// 不消费掉就可能被更晚的判断再处理一次。
    pub(crate) fn window_shortcuts(&mut self, ctx: &egui::Context) {
        // 卡片页是前景模态：键盘先归它自己的文本框。否则开始页的 Enter
        // 会隔着卡片再开一份空白文档，还顺手吞掉输入框里的回车。
        if self.note_reader.is_some() {
            return;
        }
        // 大纲「文档信息」是弹窗：Esc 由壳层统一路由，这里不再单独消费。
        if self.reading.is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.exit_reading();
            return;
        }
        // Alt+F 打开汉堡菜单；菜单内的按钮可用 Tab/Enter 操作，不依赖鼠标。
        if self.reading.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::F))
        {
            egui::Popup::open_id(ctx, egui::Id::unique("mw-main-menu"));
        }
        // 大纲里 `?` 打开专门的三栏快捷键表。Shift+/ 在不同布局下可能报 Slash
        // 也可能报 Questionmark，两支都收；输入框握着键盘时不抢。
        if self.outline_view
            && !self.outline.info_open()
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::SHIFT, egui::Key::Questionmark)
                    || i.consume_key(egui::Modifiers::SHIFT, egui::Key::Slash)
            })
        {
            self.help.toggle_outline();
            return;
        }
        // 画布焦点下 Ctrl+V：剪贴板有图片就挂图，没有就留给节点/文字的普通粘贴。
        if self.canvas.is_some()
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::V))
        {
            if let Some(png) = files::clipboard_image() {
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::V);
                });
                self.attach_image_bytes(png);
                return;
            }
        }
        // 开始页盖住文档时，文档类快捷键一律不接：Tab/Delete 不能让隐藏的图变脏。
        // 只留新建/打开、F1 与 Esc（Esc 要留给速查浮层先关）。
        // 笔记库页盖在开始页上：库里的搜索框也要用回车，别在背后开新文档。
        // 标题栏菜单开着时裸键归菜单（同 ⑨/⑩/⑪ 的前台门）：Esc 先关菜单，
        // Enter/1–9/0 不能在菜单背后新建文档。读的是上一帧的弹层状态。
        if self.home && self.library.is_none() {
            let help_open = self.help.is_open();
            if self.shortcuts.consume(ctx, shortcuts::Action::Help) {
                self.help.toggle();
            } else if home_bare_keys_owned(egui::Popup::is_any_open(ctx), help_open) {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                    if self.home_return {
                        self.leave_home();
                    }
                } else if self.shortcuts.consume(ctx, shortcuts::Action::NewConcept) {
                    // `docs/30` §3.5：开始页上 Ctrl+Shift+N 直接新建概念图。
                    // 排在 `New` 之前（理由见文档分支同一处注释）。
                    self.home_new_concept();
                } else if self.shortcuts.consume(ctx, shortcuts::Action::New) {
                    self.home_new_blank();
                } else if self.shortcuts.consume(ctx, shortcuts::Action::Open) {
                    self.action_open();
                } else if ctx.memory(|m| m.focused()).is_none()
                    && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                {
                    // 焦点在开始页卡片上时 Enter 归卡片（`ui_kit::activated`）；
                    // 只有整页都没有焦点时才走窗口级「新建」。
                    self.home_new_blank();
                } else {
                    // 开始页上 1–9 / 0 直接选模板（`docs/30` §3.5）：键盘用户不必先学鼠标位置。
                    // 键位表在 `start::template_hotkey`，与模板表的长度同源，加卡不用回来改这里。
                    for idx in 0..start::TEMPLATES.len() {
                        let Some(key) = start::template_hotkey(idx) else {
                            break;
                        };
                        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                            self.home_template(idx);
                            break;
                        }
                    }
                }
            }
            return;
        }
        // 演示模式把键盘收窄到五个键：其它快捷键（保存/导出/切标签）都不该在
        // 幕布上留下痕迹。Esc 恢复视口，F5 也当退出用（同一只手按进来再按出去）。
        if self.demo_active() {
            self.demo_shortcuts(ctx);
            return;
        }
        // 撤销/重做在这儿，不在画布的 `shortcuts` 里：那条路要求画布握着焦点，于是
        // 「鼠标点一下工具条的撤销，再按 Ctrl+Z」会静默失灵——焦点已经交给那个按钮了。
        // 但**键盘归文本框时它归文本框自己**，而且这个判断必须在 `consume_key` **之前**做：
        // 事件一旦被摘走，本帧后面才画的 `TextEdit` 就再也收不到，那下 Ctrl+Z 两头落空。
        // 搜索条、命令面板、检查器的标题/备注都算，所以用 egui 的 `text_edit_focused`；
        // 行内编辑器另加一道显式护栏——编辑态画布自己最清楚。
        let history_is_ours =
            !(ctx.text_edit_focused() || self.canvas.as_ref().is_some_and(Canvas::is_editing));
        self.window_clipboard(ctx);
        if history_is_ours
            && (self.shortcuts.consume(ctx, shortcuts::Action::Redo)
                || self.shortcuts.consume(ctx, shortcuts::Action::RedoAlt))
        {
            self.step_history(true);
        } else if history_is_ours && self.shortcuts.consume(ctx, shortcuts::Action::Undo) {
            self.step_history(false);
        } else if self.shortcuts.consume(ctx, shortcuts::Action::SaveAs) {
            self.action_save_as();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Save) {
            self.action_save();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Open) {
            self.action_open();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::NewConcept) {
            // 文档里按 Ctrl+Shift+N：开一张概念图标签页（不动当前这篇）。
            // **必须排在 `New` 之前**：egui 的 `matches_logically` 只查「模式要 Shift 而没按
            // Shift」，反向不查——按下 Ctrl+Shift+N 同样能满足 Ctrl+N 的模式，先判 `New`
            // 就会被它吃掉（2026-10-29 的 SE-03 轨迹里真这么发生了：模态记的是「已新建思维导图」）。
            self.new_concept_tab();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::New) {
            self.action_new();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Export) {
            self.open_export();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::ExportPdf) {
            self.open_export_pdf();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Preferences) {
            self.shortcuts_dialog.open();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Demo) {
            self.start_demo();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::FocusMode) {
            self.toggle_focus_mode(ctx);
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Reading) {
            self.toggle_reading();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Help) {
            if !self.help.is_open() {
                if let Some(canvas) = self.canvas.as_mut() {
                    canvas.commit_editing();
                }
                self.palette.close();
                self.search.close();
                self.export.open = false;
            }
            // 大纲里 F1 给的是大纲键位表（导航/编辑/结构），不是画布那张总表。
            if self.outline_view {
                self.help.toggle_outline();
            } else {
                self.help.toggle();
            }
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Outline) {
            self.toggle_outline_view();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::PaneCycle) {
            self.cycle_pane_focus(ctx);
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Inspector)
            || self.shortcuts.consume(ctx, shortcuts::Action::InspectorAlt)
        {
            // 合同键 `Ctrl+Shift+I`（§4.4）与备用键 `F9` 同义。
            self.toggle_inspector();
        } else if self.outline_view
            && self
                .shortcuts
                .consume(ctx, shortcuts::Action::OutlineFilter)
        {
            // docs/28 §5.7：大纲内筛选条是 Ctrl+Shift+F；Ctrl+F 仍归全局搜索。
            crate::trace::event("outline_toggle_search");
            self.outline.toggle_search();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Find) {
            self.open_search();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::Palette) {
            if self.search.is_open() {
                self.search.close();
            }
            self.palette.toggle();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::NewConcept) {
            // 文档里按 Ctrl+Shift+N：开一张概念图标签页（不动当前这篇）。
            self.new_concept_tab();
        } else if !self.outline_view && self.shortcuts.consume(ctx, shortcuts::Action::NewTab) {
            // 大纲里 Ctrl+T 是行任务状态，不是新建标签页（顶栏 + 仍可新建）。
            self.new_tab();
        } else if self.shortcuts.consume(ctx, shortcuts::Action::CloseTab) {
            self.request_close_tab(self.active);
        } else if self.shortcuts.consume(ctx, shortcuts::Action::PrevTab)
            || self.shortcuts.consume(ctx, shortcuts::Action::PrevTabAlt)
        {
            self.cycle_tab(-1);
        } else if self.shortcuts.consume(ctx, shortcuts::Action::NextTab)
            || self.shortcuts.consume(ctx, shortcuts::Action::NextTabAlt)
        {
            self.cycle_tab(1);
        } else {
            // Alt+1..9 跳到第 N 个标签页（旧版语义）。
            for (n, key) in [
                (1, egui::Key::Num1),
                (2, egui::Key::Num2),
                (3, egui::Key::Num3),
                (4, egui::Key::Num4),
                (5, egui::Key::Num5),
                (6, egui::Key::Num6),
                (7, egui::Key::Num7),
                (8, egui::Key::Num8),
                (9, egui::Key::Num9),
            ] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, key)) {
                    if n <= self.tabs.len() {
                        self.switch_tab(n - 1);
                    }
                    break;
                }
            }
        }
    }

    pub(crate) fn demo_active(&self) -> bool {
        self.canvas.as_ref().is_some_and(Canvas::demo_active)
    }

    /// 最近文件还在读（起始页画 3 行骨架，而不是「还没有最近文件」，`docs/30` §3.4）。
    pub(crate) fn recents_pending(&self) -> bool {
        self.recent_load.is_some() || self.recents_pending_frames > 0
    }
}
