//! QA 用笔记库：`MW_LIBRARY_DEMO=<场景>` 且没有现成笔记时，在仓库内 `target/tmp` 写入固定卡片。
//!
//! golden 要比像素，卡片的时间戳与顺序就必须固定；而真实用户笔记既不可控，
//! 也不该被测试读写。这里只使用仓库内 `target/tmp` 下的独立目录（用户 Temp 在受限令牌下可能拒写）。
//!
//! 一个旋钮带十场（`docs/28` §7.10）：值就是场景名，`1` 与 `default` 等价。
//! 两栏/单栏那两场不靠这里——它们由窗口宽度决定（`MW_WINDOW_SIZE`），
//! 库页自己按 §7.2 的断点分档。

use crate::note_library::NoteLibrary;
use crate::note_store::{CardMeta, NoteStore};
use crate::Shell;
use mw_core::note::CardScope;
use mw_core::NoteCard;

const BASE_TS: i64 = 1_793_000_000_000;

/// `MW_LIBRARY_DEMO` 认得的场景名。`1` 是历史值（`scripts/ci.ps1` 与
/// `x100-stage-b-gate.ps1` 里写的就是它），等同 `default`。
pub const SCENES: &[&str] = &[
    "default",
    "preview",
    "empty",
    "no-results",
    "long",
    "select",
    "tag-filter",
    "archive",
    "loading",
];

/// 当前场景名。认不出的值一律当 `default`：夹具不该因为拼错而静默变成空库。
pub fn scene() -> String {
    let raw = std::env::var("MW_LIBRARY_DEMO").unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() || raw == "1" {
        return "default".to_string();
    }
    let lower = raw.to_ascii_lowercase();
    if SCENES.contains(&lower.as_str()) {
        lower
    } else {
        "default".to_string()
    }
}

struct Seed<'a> {
    title: &'a str,
    body: &'a str,
    tags: &'a [&'a str],
    archived: bool,
    /// 挂到示例文档的根节点上（§7.6 的搜索范围含「来源节点标题」，
    /// §7.5 的预览要有「来源」一行）。
    sourced: bool,
}

/// 20 张固定卡片：四个范围都非空、标签树到三级（用来证明侧栏封顶两级）、
/// 一张长文（预览滚动）、一张空标题（列表与预览都退回「无标题」）。
///
/// 张数与分类是**证据的一部分**，改这里就等于改 `library-*` 十场的判词，
/// 所以每类都写了数：全部 16 / 归档 4 / 收件箱 2 / 未打标签 3。
const SEEDS: &[Seed] = &[
    Seed {
        title: "键盘色板验证",
        body: "# 会议要点\n\n今天讨论了三件事：视图跟随、卡片笔记、以及标签体系。\n\n## 待办\n\n- [ ] 补全大纲全屏编辑\n- [x] 移除右下角小地图\n\n- 检查 **斜体** 与 ~~删除线~~\n\n> 引用：所见即所得，不切模式。\n\n1. 第一项\n",
        tags: &["知识/方法", "整理术"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "视图跟随",
        body: "新增节点时保持上下文：只做最小平移，不抢用户的缩放。\n\n子节点落到屏幕外时，画布自己让路。\n",
        tags: &["知识"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "标签体系",
        body: "标签用斜杠分层：`知识/方法` 会同时出现在「知识」与「方法」下。\n\n卡片可挂多个标签，列表与搜索都按标签过滤。\n",
        tags: &["整理术", "项目/织思"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "卡片笔记",
        body: "一页只做一件事：读、写、看都在同一表面，没有阅读/编辑双态。\n\n`[[间隔重复]]` 让卡片在快忘记时回来。\n",
        tags: &["项目/织思"],
        archived: false,
        sourced: false,
    },
    // 三级标签：侧栏只画到两级（§7.1 / §7.9「标签深度不许无限」），
    // `知识/方法/整理术` 的计数聚合进 `知识/方法`，第三级不出现。
    Seed {
        title: "三层标签只到两级",
        body: "侧栏最多两级，再深的路径只在列表的标签行与搜索里出现。\n",
        tags: &["知识/方法/整理术"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "外壳令牌",
        body: "颜色只从 `design::content` 取，不从壳层 `Chrome` 取：\n亮色分支把面板写死成纯白，四套主题会塌成同一张灰白纸。\n",
        tags: &["项目/织思/外壳"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "读书笔记：设计中的设计",
        body: " affordance 不是装饰，是「这里能按」的暗示。\n\n## 摘录\n\n> 好的设计是不可见的。\n",
        tags: &["读书/笔记"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "禁则换行",
        body: "行首不许出现 `。，、）》，行尾不许出现 `（《「`。\n\n中英混排时按字符类判，不按字节。\n",
        tags: &["知识/方法"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "拼音检索",
        body: "全拼与首字母都要能命中：`zj` → 织思，`zhisi` → 织思。\n\n上限 240 字，超出只搜前缀。\n",
        tags: &["知识/方法"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "输入法不打架",
        body: "组合态里 `Enter` 是「上屏」，不是「提交」。\n\n候选窗跟着光标走，不遮住正在写的那一行。\n",
        tags: &["知识"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "撤销栈上限",
        body: "默认 500 步，可配 100–2000。\n\n调低时给一次提示，不静默截断。\n",
        tags: &["项目/织思"],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "概念图与思维导图",
        body: "思维导图是一棵树，概念图是一张网。\n\n命题（连接短语）是概念图区别于思维导图的关键。\n",
        tags: &["读书"],
        archived: false,
        sourced: false,
    },
    // 长文：预览要有自己的滚动条，且换卡片时回到顶部（§7.5）。
    Seed {
        title: "长文卡片：排版重校记录",
        body: "# 全局字阶\n\n重校之前各处自己定字号，同一个「次要文字」在大纲里是 12、在卡片里是 12.5、在检查器里是 11.5。\n重校之后统一成十档，每档只有一处定义。\n\n## 十档\n\n- 页标题 30/42/600\n- 二级标题 18/28/600\n- 三级标题 16/26/600\n- 列表标题 15/22/600\n- 列表摘要 13/19/400\n- 列表元信息 12/18/400\n- 导航 14/21/400\n- 元信息 12/18/400\n- 标签 12/18/600\n- 正文 16/28/400\n\n## 为什么不许小数\n\n小数字号在 1.25 与 1.5 缩放下会落到半个像素上，DirectWrite 的光栅化结果随缩放档跳变，\n金样因此会在两档之间出现 1 px 的文字位移。取整档之后同一份文本在 1.0/1.5/2.0 三档都稳定。\n\n## 消费者对照\n\n每一档都要写清谁在用：「导航 14/21」的消费者是库侧栏、大纲行、设置中心的分组标题。\n没有消费者的档位不留，否则下一次重校又会长出三个新的。\n\n## 未做\n\n字重只有 400 与 600 两档，500 那一档在规范里出现过但没有任何消费者，已删。\n",
        tags: &["读书/笔记"],
        archived: false,
        sourced: false,
    },
    // 收件箱 = 未归档 ∧ 无标签 ∧ 未挂节点。这两张是「还没整理」的那批。
    Seed {
        title: "待整理：随手记",
        body: "先记下来，标签以后再打。\n",
        tags: &[],
        archived: false,
        sourced: false,
    },
    Seed {
        title: "待整理：会议碎片",
        body: "- 下周三评审\n- 需要一份对比表\n",
        tags: &[],
        archived: false,
        sourced: false,
    },
    // 未打标签但**已挂节点**：进「未打标签」不进「收件箱」。
    // 这一张就是两个范围的差集，删掉它侧栏两个计数会相等、看不出区别。
    Seed {
        title: "已挂节点但没标签",
        body: "它从导图节点上生成，所以有来源，但还没打标签。\n",
        tags: &[],
        archived: false,
        sourced: true,
    },
    // 归档：只在侧栏「归档」下出现（§7.1）。
    Seed {
        title: "旧版视觉基准",
        body: "已被 `docs/28` 取代，留档备查。\n",
        tags: &["整理术"],
        archived: true,
        sourced: false,
    },
    Seed {
        title: "废弃：全屏大纲",
        body: "B2 起改为 232px 侧栏大纲。\n",
        tags: &["项目/织思"],
        archived: true,
        sourced: false,
    },
    Seed {
        title: "废弃：右下角小地图",
        body: "默认关闭，浮层不再占位。\n",
        tags: &["知识"],
        archived: true,
        sourced: false,
    },
    // 空标题：列表与预览都退回「无标题」，而不是画一行空白。
    Seed {
        title: "",
        body: "草稿，还没起标题。\n",
        tags: &[],
        archived: true,
        sourced: false,
    },
];

/// 打开临时目录里的示例库：**每次从零种**，卡片 id 也固定。
///
/// 2026-10-28 `library` 三档基线全红，根因有两层，都在这一个函数里：
///
/// 1. 种子写盘失败被 `let _ =` 吞掉。沙箱/受限令牌下临时目录可能不可写，
///    于是「库是空的」——页面画空态，而 golden 只看到「和基线不一样」。
///    现在写失败直接 panic，写完还要断言张数。
/// 2. 「目录不空就不再种」让上一次运行残留的库静默变成 golden 的内容。
///    现在每次先删再种。
///
/// 随机 id 也不行：列表按 `updated_at` 倒序、同时间再按 id 排，随机 id 会让顺序每次不同，
/// 所以固定成 `demo-01..`。落地目录见文件末尾的 `demo_root`。
pub fn seeded_store(scene: &str) -> NoteStore {
    let root = demo_root();
    let _ = std::fs::remove_dir_all(&root);
    let mut store = NoteStore::open_at(root.clone());
    if scene == "empty" {
        assert_eq!(store.len(), 0, "空库场景不该有残留卡片");
        return store;
    }
    for (i, seed) in SEEDS.iter().enumerate() {
        // 时间戳递减且**固定**：读墙钟会让「最近更新」排序每次不同。
        let ts = BASE_TS + (SEEDS.len() - i) as i64 * 60_000;
        let mut card = NoteCard::new("demo", None, seed.title, seed.body, ts);
        card.id = format!("demo-{:02}", i + 1);
        card.tags =
            NoteCard::normalize_tags(&seed.tags.iter().map(|t| t.to_string()).collect::<Vec<_>>());
        card.archived = seed.archived;
        // 先写占位来源，`seed_scene` 再换成真实文档与根节点的 id：
        // 这里拿不到画布，而 `source_doc = "demo"` 在 `card_sources()` 里没有条目，
        // 真解析不出来时 `source_phrase` 会安静地返回 None，不会画出假来源。
        if seed.sourced {
            card.source_node = Some("root".to_string());
        }
        // 写不进去必须炸出来：忽略返回值会让夹具静默变成「空库」，
        // golden 拍到的就是空态而不是这些卡（2026-10-28 就是这样连红三档）。
        store
            .upsert(&card)
            .unwrap_or_else(|e| panic!("示例库写入失败（{}）：{e}", root.display()));
    }
    assert_eq!(
        store.len(),
        SEEDS.len(),
        "示例库必须恰好 {} 张，实际 {}：临时目录不可写会让夹具悄悄变成空库",
        SEEDS.len(),
        store.len()
    );
    store
}

/// 启动即把库页摆成这一场要拍的样子。
///
/// 只写内存、不动盘（除种子本身）：`loading` 那场甚至把 `notes` 置空，
/// 好让 §7.7 的骨架真的画出来，而不是画一张空态。
pub fn seed_scene(shell: &mut Shell) {
    let scene = scene();
    let (sidebar_w, list_w) = shell.window_state.library_widths();

    if scene == "loading" {
        // §7.7「库不可用 / 索引还没读到」：三行 60px 骨架，不转菊花。
        shell.notes = None;
        shell.library = Some(NoteLibrary::new(Vec::new(), sidebar_w, list_w));
        return;
    }
    if scene == "long" {
        // §7.4「10k 首屏 ≤100ms、滚动 ≥55fps」：一万行只画看得见的那些。
        // 故意不落盘——这一场要量的是**行序与虚拟化**，写一万个文件只会把
        // 证据变成一次磁盘基准测试。
        shell.library = Some(NoteLibrary::new(bulk_metas(10_000), sidebar_w, list_w));
        return;
    }

    shell.notes = Some(seeded_store(&scene));
    // 「已挂节点但没标签」那张要真的有来源，否则收件箱与未打标签两个计数会相等。
    // 直接改字段而不是走 `attach_to_node`：那条会把 `updated_at` 写成墙钟，
    // 「最近更新」排序就每次不同了（M16 的教训）。
    if let (Some(notes), Some(canvas)) = (shell.notes.as_mut(), shell.canvas.as_ref()) {
        let doc_id = canvas.session().doc.id.clone();
        let root = canvas.session().doc.root_id.clone();
        if let Some(mut card) = notes.get(SOURCED_ID) {
            card.source_doc = doc_id;
            card.source_node = Some(root);
            let _ = notes.upsert(&card);
        }
    }
    shell.open_library();

    let Some(lib) = shell.library.as_mut() else {
        return;
    };
    match scene.as_str() {
        // 默认场就是刚打开的样子：没有焦点卡片，预览画 §7.5 的空态。
        "default" | "empty" => {}
        // 阅读面：焦点落在第一行（长文那张不在首位，靠 `preview` 场单独拍）。
        "preview" => lib.demo_focus(0),
        "no-results" => lib.demo_query("一个绝不会命中的查询串"),
        "tag-filter" => {
            lib.demo_tag_filter("知识");
            lib.demo_focus(0);
        }
        "archive" => {
            lib.demo_scope(CardScope::Archived);
            lib.demo_focus(0);
        }
        "select" => {
            lib.demo_focus(0);
            lib.demo_select(3);
        }
        _ => {}
    }
}

/// 挂来源的那张卡（`SEEDS` 里 `sourced: true` 的那一条）。
const SOURCED_ID: &str = "demo-16";

/// 一万行元数据：标题与摘要按下标生成，时间戳递减保证排序稳定。
fn bulk_metas(n: usize) -> Vec<CardMeta> {
    (0..n)
        .map(|i| CardMeta {
            id: format!("bulk-{i:05}"),
            title: format!("第 {:05} 张卡片：虚拟化只画看得见的行", i + 1),
            tags: vec![format!("知识/{}", if i % 2 == 0 { "甲" } else { "乙" })],
            source_doc: "bulk".to_string(),
            source_node: None,
            created_at: BASE_TS,
            updated_at: BASE_TS + (n - i) as i64,
            excerpt: format!("这是第 {} 行的摘要，只占一行，超出按省略号收尾。", i + 1),
            file: format!("bulk-{i:05}.md"),
            archived: false,
        })
        .collect()
}

/// 夹具的落地目录。
///
/// 不能直接用 `std::env::temp_dir()`：本机受限令牌下它返回的用户 Temp 是**拒写**的
/// （实测 `os error 5 拒绝访问`，`mw-format` 的 8 个往返用例也因此假失败），
/// 于是「库是空的」→ 页面画空态 → golden 三档全红。优先落在仓库内 `target/tmp`，
/// 那里一定可写；`MW_DEMO_LIB` 可以覆盖。
fn demo_root() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("MW_DEMO_LIB") {
        return std::path::PathBuf::from(p);
    }
    // 每场一个目录：`empty` 场要是与 `default` 场共用一个目录，
    // 上一场的残留会让空态悄悄变成 20 张卡。
    let local = std::env::current_dir()
        .unwrap_or_default()
        .join("target")
        .join("tmp")
        .join(format!("mw-library-demo-{}", scene()));
    if std::fs::create_dir_all(&local).is_ok() {
        return local;
    }
    std::env::temp_dir().join(format!("mw-library-demo-{}", scene()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 十场判词都建立在这几个数上（侧栏计数写进基线，改一张卡就全漂）。
    #[test]
    fn seed_counts_match_the_scopes() {
        use mw_core::note::{scope_counts, CardRef};
        let cards: Vec<CardMeta> = SEEDS
            .iter()
            .enumerate()
            .map(|(i, s)| CardMeta {
                id: format!("demo-{:02}", i + 1),
                title: s.title.to_string(),
                tags: s.tags.iter().map(|t| t.to_string()).collect(),
                source_doc: "demo".into(),
                source_node: s.sourced.then(|| "root".to_string()),
                archived: s.archived,
                ..CardMeta::default()
            })
            .collect();
        let c = scope_counts(&cards);
        assert_eq!(SEEDS.len(), 20);
        assert_eq!(c.all, 16, "全部卡片 = 未归档");
        assert_eq!(c.archived, 4);
        assert_eq!(c.inbox, 2, "收件箱 = 未归档 ∧ 无标签 ∧ 无来源");
        assert_eq!(c.untagged, 3, "未打标签 = 未归档 ∧ 无标签");
        // 恰好一张挂来源：它是「未打标签」与「收件箱」的差集。
        assert_eq!(
            cards
                .iter()
                .filter(|c| c.card_source_node().is_some())
                .count(),
            1
        );
    }

    /// 空标题那张必须存在：列表与预览的「无标题」退回路径要有东西可拍。
    #[test]
    fn one_seed_has_an_empty_title() {
        assert_eq!(SEEDS.iter().filter(|s| s.title.is_empty()).count(), 1);
    }

    /// 三级标签要真的有，否则「侧栏封顶两级」这条就没有反例可证。
    #[test]
    fn a_seed_uses_a_three_level_tag() {
        assert!(SEEDS
            .iter()
            .flat_map(|s| s.tags)
            .any(|t| t.split('/').count() >= 3));
    }

    #[test]
    fn scene_falls_back_to_default_on_typos() {
        // 拼错就静默变成空库，golden 会红得毫无头绪——所以认不出的值当 default。
        std::env::set_var("MW_LIBRARY_DEMO", "1");
        assert_eq!(scene(), "default");
        std::env::set_var("MW_LIBRARY_DEMO", "nope");
        assert_eq!(scene(), "default");
        std::env::set_var("MW_LIBRARY_DEMO", "Archive");
        assert_eq!(scene(), "archive");
        std::env::remove_var("MW_LIBRARY_DEMO");
        assert_eq!(scene(), "default");
    }

    #[test]
    fn bulk_metas_are_ordered_and_unique() {
        let v = bulk_metas(50);
        assert_eq!(v.len(), 50);
        assert_eq!(v[0].id, "bulk-00000");
        assert!(v[0].updated_at > v[49].updated_at, "首行该是最近更新");
        let mut ids: Vec<&str> = v.iter().map(|c| c.id.as_str()).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }
}
