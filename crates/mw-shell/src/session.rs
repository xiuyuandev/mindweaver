//! 文档会话：一个窗口里「当前这份文档」的全部状态。
//!
//! 之前这些东西散在 `Canvas` 里（`doc` / `settings` / `theme`），意味着**除了画布谁都拿不到
//! 文档**——于是"打开/保存"无处落脚。这一层就是 M3 的第一刀：把文档抬到壳层，画布只负责
//! 把它画出来、把编辑命令发进去。
//!
//! 纪律：
//! · **落盘只有一个入口**（[`Session::save_to`]），它同时负责更新"已保存修订号"；
//! · **脏标记只看修订号差**，不看"文本变过没有"这种模糊判据——`Document::revision`
//!   在每条命令成功后自增，是唯一可靠的口径；
//! · 会话不碰 UI、不碰 GPU、不开线程，所以它能在无显示器的测试里跑。

use std::path::{Path, PathBuf};

use mw_core::theme::{theme_for, Theme};
use mw_core::{Command, Document, LayoutSettings};

pub struct Session {
    /// `None` = 还没落过盘（标题显示"未命名"）。
    pub path: Option<PathBuf>,
    pub doc: Document,
    pub settings: LayoutSettings,
    pub theme: Theme,
    /// 上次落盘时的修订号；`None` = 从没保存过。
    saved_revision: Option<u64>,
    /// 导入来源的文件名。导入的会话没有 `path`（Ctrl+S 必须先另存为），
    /// 但标签页和标题栏该显示"从哪个文件来的"，而不是一句"未命名"。
    import_name: Option<String>,
    /// 模板/新建文档的显示名（如「鱼骨图」）。只在落盘前顶替"未命名.zmind"，
    /// **不是**文件名——Ctrl+S 仍走另存为。
    untitled_name: Option<String>,
}

impl Session {
    /// 内置夹具：M1b-3 的验收文档（中英混排、标签、概要、边界、关系线、可折叠子树各一份）。
    ///
    /// 它现在的正式身份是**未命名文档的初始内容**——产品要变成"新建即空文档"时只改这里。
    pub fn sample() -> Self {
        let (doc, _) = sample_document();
        let mut s = Self::from_doc(doc, None);
        // 夹具文档按"已落盘"对待：启动时标题栏不该顶着未保存点，
        // golden 基线也不该因为多了一个 ● 而漂。
        s.saved_revision = Some(s.doc.revision);
        s
    }

    /// CER-11 性能夹具：4000 分支 × 4 叶 + 根 = 20001 行。
    ///
    /// 只给 `MW_OUTLINE_20K` 实拍与性能脚本用；不写盘、不进产品路径。
    /// 故意保持结构与 `outline::tests::twenty_thousand_*` 一致。
    pub fn sample_20k() -> Self {
        let mut doc = Document::new("20k");
        let root = doc.root_id.clone();
        let _ = doc.apply(&Command::SetText {
            id: root.clone(),
            text: "20k 性能夹具".into(),
        });
        for b in 0..4_000 {
            let _ = doc.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: format!("分支 {b:04}"),
                style: None,
            });
            let Some(branch) = doc.node(&root).and_then(|n| n.children.last()).cloned() else {
                continue;
            };
            for leaf in 0..4 {
                let _ = doc.apply(&Command::AddNode {
                    parent: branch.clone(),
                    index: None,
                    text: format!("叶 {b:04}-{leaf}"),
                    style: None,
                });
            }
        }
        let mut s = Self::from_doc(doc, None);
        s.saved_revision = Some(s.doc.revision);
        s
    }

    /// 大纲实拍夹具：在样例上补一条备注与一个进行中的任务，
    /// 让任务框、备注点、卡片点与标签计数都能出现在同一张截图里。
    pub fn sample_with_marks() -> Self {
        let mut s = Self::sample();
        let ids = s.doc.doc_order();
        if let Some(id) = ids.iter().find(|id| {
            s.doc
                .node(id.as_str())
                .is_some_and(|n| n.text == "中文排版")
        }) {
            let _ = s.doc.apply(&Command::SetNote {
                id: id.clone(),
                note: Some("标点悬挂与避头尾规则要逐条对照。".into()),
            });
        }
        if let Some(id) = ids.iter().find(|id| {
            s.doc
                .node(id.as_str())
                .is_some_and(|n| n.text == "交互与性能")
        }) {
            let _ = s.doc.apply(&Command::SetTask {
                id: id.clone(),
                task: Some(mw_core::TaskState::Doing),
            });
        }
        // 挂一张卡片：行尾卡片点要能进实拍，也要能点开阅读页。
        if let Some(id) = ids.iter().find(|id| {
            s.doc
                .node(id.as_str())
                .is_some_and(|n| n.text == "中文排版")
        }) {
            let card = mw_core::NoteCard::new(
                s.doc.id.clone(),
                None,
                "排版清单",
                "## 排版\n\n- 标点悬挂\n- 禁则换行\n",
                1,
            );
            let _ = s.doc.apply(&Command::AttachNewCard {
                node: id.clone(),
                card,
            });
        }
        // 实拍里不该出现「未保存」圆点：夹具仍按已落盘对待。
        s.saved_revision = Some(s.doc.revision);
        s
    }

    /// B2-A 补拍夹具：docs/28 §5.2 非编辑态长文换行（`MW_OUTLINE_WRAP=1`）。
    ///
    /// 每行钉一个事实：① 超宽正文折行且悬挂缩进；② 短行不折；③ 深层缩进更早折行；
    /// ④ Shift+Enter 的硬换行照常断行；⑤ 说明行与折行叠加（32 + 24×(n−1) + 20）；
    /// ⑥ 三行以上的超长行，行高按 +24 累计。只给实拍用，不写盘、不进产品路径。
    pub fn sample_wrap() -> Self {
        let mut doc = Document::new("长文换行验收");
        let root = doc.root_id.clone();
        let add = |doc: &mut Document, parent: &str, text: &str| -> String {
            let _ = doc.apply(&Command::AddNode {
                parent: parent.into(),
                index: None,
                text: text.into(),
                style: None,
            });
            doc.node(parent)
                .and_then(|n| n.children.last().cloned())
                .unwrap_or_default()
        };
        // ① 约 52 个汉字：740px 的正文栏（16px 一行约 46 字）装不下，折两行。
        add(
            &mut doc,
            &root,
            "非编辑态的长行必须按正文栏宽自动折行，不截断也不加省略号，折下来的第二行与首行文字左对齐形成悬挂缩进。",
        );
        // ② 对照：短行只有一行高，不能被换行逻辑撑高。
        add(&mut doc, &root, "短行不折");
        // ③ 深层：缩进吃掉 48px，同样的字数在这一层更早折行。
        let deep = add(&mut doc, &root, "第二层");
        add(
            &mut doc,
            &deep,
            "深层长行：缩进吃掉 48px 之后，同样的字数在这里比第一层更早折行，而每行文字仍从同一条左缘起笔。",
        );
        // ④ 硬换行（Shift+Enter 写进草稿的 \n）：非编辑态照常断行。
        add(&mut doc, &root, "软换行第一行\n软换行第二行");
        // ⑤ 说明行 + 折行：行高 32 + 24 + 20，说明行贴行框底部。
        let noted = add(
            &mut doc,
            &root,
            "有说明行的长文本也要折行：正文折成两行之后，说明行仍贴在行框底部，两段之间不互相挤压，行尾角标也不遮住文字。",
        );
        let _ = doc.apply(&Command::SetNote {
            id: noted,
            note: Some("说明行 13/20，text.secondary。".into()),
        });
        // ⑥ 超长一行（约 150 字）：三行以上，行高按 +24 累计。
        add(
            &mut doc,
            &root,
            "这一行刻意写得比正文栏宽出三倍以上，用来验收多行折行时的行高累计：每多一行行框长高 24px，\
             圆点、任务框与行尾角标始终对齐首行文字带，折下来的每一行都从同一条左缘起笔，不会钻到缩进里，\
             也不会被行尾的备注点或卡片点压住；最后一行结束时行框正好收住，不留半行高的空隙。",
        );
        let mut s = Self::from_doc(doc, None);
        s.saved_revision = Some(s.doc.revision);
        s
    }

    /// 从一份已在内存里的文档开会话（打开文件、导入、新建都走这里）。
    pub fn from_doc(doc: Document, path: Option<PathBuf>) -> Self {
        let settings = doc.meta.layout.clone();
        let theme = theme_for(&doc.meta);
        Self {
            path,
            doc,
            settings,
            theme,
            saved_revision: None,
            import_name: None,
            untitled_name: None,
        }
    }

    /// 空白文档：一个「中心主题」根，没有子节点。
    ///
    /// 它按**已保存**对待——新建就该是干净的，Ctrl+S 时才弹"另存为"。
    pub fn blank() -> Self {
        let mut doc = Document::new("中心主题");
        let root = doc.root_id.clone();
        let _ = doc.apply(&Command::SetText {
            id: root,
            text: "中心主题".into(),
        });
        Self::fresh(doc)
    }

    /// 空白概念图：一个「中心概念」根，布局种类是 `conceptMap`。
    ///
    /// 概念图没有子树：别的概念都是自由摆放的浮点节点（`parent_id = None`），
    /// 概念之间的连接走 `relationships`。空白态就是一个中心概念。
    pub fn blank_concept() -> Self {
        let mut doc = Document::new("概念图");
        let root = doc.root_id.clone();
        let _ = doc.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        });
        let _ = doc.apply(&Command::SetText {
            id: root,
            text: "中心概念".into(),
        });
        Self::fresh(doc)
    }

    /// 新建/模板共用：内容还在内存里、没有路径，但按"未修改"对待。
    pub fn fresh(doc: Document) -> Self {
        let mut s = Self::from_doc(doc, None);
        s.saved_revision = Some(s.doc.revision);
        s
    }

    /// 带显示名的新文档（模板用）：标签写作「鱼骨图」，但还没有文件路径。
    pub fn fresh_named(doc: Document, name: impl Into<String>) -> Self {
        let mut s = Self::fresh(doc);
        s.untitled_name = Some(name.into());
        s
    }

    /// 是不是一份**什么都没做过的空白页**（无路径、无改动、根下无子节点）。
    /// 开始页用它决定"就地复用空白页"还是"另开一页"，避免每次点模板都堆一个空标签。
    pub fn is_pristine(&self) -> bool {
        self.path.is_none()
            && self.import_name.is_none()
            && self.untitled_name.is_none()
            && !self.dirty()
            && self.doc.nodes.len() == 1
            && self
                .doc
                .nodes
                .get(&self.doc.root_id)
                .is_some_and(|n| n.children.is_empty())
    }

    /// 这个扩展名我们认识吗（拖放分流用）。
    pub fn accepts(path: &Path) -> bool {
        matches!(
            ext_of(path).as_str(),
            "zmind" | "md" | "markdown" | "opml" | "mm" | "txt" | "xmind"
        )
    }

    /// 导入一份外部格式成新会话（未命名、脏——导进来的东西还没落过盘）。
    /// 兼容旧调用：只要会话。新代码应走 [`Session::import_with_report`]。
    #[allow(dead_code)]
    pub fn import(path: &Path) -> Result<Self, String> {
        Self::import_with_report(path).map(|(s, _)| s)
    }

    /// 同 [`Session::import`]，但带回丢失项报告（图片/附件等）。
    pub fn import_with_report(path: &Path) -> Result<(Self, mw_io::ImportReport), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("读取失败：{e}"))?;
        let ext = ext_of(path);
        let text = || String::from_utf8_lossy(&bytes).to_string();
        let (doc, mut report) = match ext.as_str() {
            "md" | "markdown" => {
                mw_io::import_markdown(&text()).map(|d| (d, mw_io::ImportReport::default()))
            }
            "opml" => mw_io::import_opml(&text()).map(|d| (d, mw_io::ImportReport::default())),
            "mm" => mw_io::import_freemind(&text()).map(|d| (d, mw_io::ImportReport::default())),
            "txt" => mw_io::import_outline(&text()).map(|d| (d, mw_io::ImportReport::default())),
            "xmind" => mw_io::import_xmind_with_report(&bytes),
            other => return Err(format!("不支持的格式：.{other}")),
        }
        .map_err(|e| format!("导入失败：{e}"))?;
        if report.counts.nodes == 0 {
            report.counts = Self::import_counts(&doc);
        }
        let mut s = Self::from_doc(doc, None);
        s.import_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string);
        Ok((s, report))
    }

    /// 数一数这次导入实际带走了哪些字段。
    fn import_counts(doc: &mw_core::Document) -> mw_io::ImportCounts {
        let mut c = mw_io::ImportCounts::default();
        for n in doc.nodes.values() {
            c.nodes += 1;
            c.notes += usize::from(n.note.is_some());
            c.links += usize::from(n.link.is_some());
            c.labels += n.labels.len();
            c.relationships += n.relationships.len();
            c.collapsed += usize::from(n.collapsed);
            c.styled += usize::from(n.style.is_some());
        }
        c
    }

    /// 从恢复点重建会话：**内容来自 `recovery`，路径指回 `doc_path`**。
    ///
    /// 这样用户按 Ctrl+S 会写回原文件，而不是把 `.autosave` 存成新文档。
    pub fn open_recovered(recovery: &Path, doc_path: &Path) -> Result<Self, String> {
        let mut s = Self::open(recovery)?;
        s.path = Some(doc_path.to_path_buf());
        // 还没写过盘：一保存就落回原文件。先标成脏，免得用户以为已经存好了。
        s.saved_revision = None;
        Ok(s)
    }

    /// 从磁盘打开。失败原因原样往上传（文件不存在、版本过新、内容损坏各有各的话）。
    /// 按扩展名分发（`.xmind`/`.cxl`/`.md`/`.zmind`，见 `docs/34` §6）。
    pub fn open(path: &Path) -> Result<Self, String> {
        let doc = mw_io::dispatch::load_document(path).map_err(|e| format!("打开失败：{e}"))?;
        let mut s = Self::from_doc(doc, Some(path.to_path_buf()));
        // 打开就算"已保存"：文件里的内容与内存一致。
        s.saved_revision = Some(s.doc.revision);
        Ok(s)
    }

    /// 导入来源文件名（没有导入过则为 None）。
    pub fn import_name(&self) -> Option<&str> {
        self.import_name.as_deref()
    }

    pub fn title(&self) -> String {
        match &self.path {
            Some(p) => p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("未命名.zmind")
                .to_string(),
            None => self
                .import_name
                .clone()
                .or_else(|| self.untitled_name.clone())
                .unwrap_or_else(|| "未命名.zmind".to_string()),
        }
    }

    /// 有未保存改动。判据是修订号，**不是**"有没有操作过"。
    pub fn dirty(&self) -> bool {
        self.saved_revision != Some(self.doc.revision)
    }

    /// 存回原路径。没有路径时返回 `Err`——调用方应当先弹"另存为"。
    pub fn save(&mut self) -> Result<(), String> {
        match self.path.clone() {
            Some(p) => self.save_to(&p),
            None => Err("还没有文件路径".into()),
        }
    }

    /// 存到指定路径（另存为也走这里）。
    ///
    /// 落盘前把 `doc.meta.title` 同步成文件名——`.zmind` 的 manifest 标题取自它，
    /// 不同步会出现"文件叫 A、manifest 里写着未命名"。
    /// 按扩展名分发格式（`.xmind`/`.cxl`/`.md`/`.zmind`，见 `docs/34` §6）。
    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            self.doc.meta.title = stem.to_string();
        }
        mw_io::dispatch::save_document(&self.doc, path).map_err(|e| format!("保存失败：{e}"))?;
        self.saved_revision = Some(self.doc.revision);
        self.path = Some(path.to_path_buf());
        self.import_name = None;
        self.untitled_name = None;
        Ok(())
    }
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// M1b-3 的验收夹具：中英混排、标签、概要、边界、关系线、可折叠子树各一份。
fn sample_document() -> (Document, LayoutSettings) {
    let mut doc = Document::new("织思");
    let root = doc.root_id.clone();
    doc.apply(&Command::SetText {
        id: root.clone(),
        text: "织思".into(),
    })
    .expect("设置根节点文字");

    let native = add_child(&mut doc, &root, "纯 Rust 原生");
    let winit = add_child(&mut doc, &native, "winit + windows-rs");
    let wgpu = add_child(&mut doc, &native, "wgpu 四层合成器");

    let cjk = add_child(&mut doc, &root, "中文排版");
    add_child(&mut doc, &cjk, "DirectWrite 度量");
    add_child(&mut doc, &cjk, "禁则换行");
    add_child(&mut doc, &cjk, "字形图集");

    let interact = add_child(&mut doc, &root, "交互与性能");
    add_child(&mut doc, &interact, "uniform grid 命中");
    add_child(&mut doc, &interact, "平移缩放");
    add_child(&mut doc, &interact, "选择与折叠");

    let export = add_child(&mut doc, &root, "导出");

    doc.apply(&Command::AddLabel {
        ids: vec![cjk.clone()],
        label: "验收".into(),
    })
    .expect("添加标签");
    doc.apply(&Command::AddSummary {
        parent: native.clone(),
        targets: vec![winit, wgpu],
        text: "渲染底座".into(),
    })
    .expect("添加概要");
    doc.apply(&Command::AddBoundary {
        parent: root,
        targets: vec![export],
        color: None,
    })
    .expect("添加边界");
    doc.apply(&Command::AddRelationship {
        from: native,
        to: doc.root_id.clone(),
        label: Some("字体同源".into()),
    })
    .expect("添加关系线");

    (doc, LayoutSettings::default())
}

fn add_child(doc: &mut Document, parent: &str, text: &str) -> String {
    doc.apply(&Command::AddNode {
        parent: parent.to_string(),
        index: None,
        text: text.to_string(),
        style: None,
    })
    .unwrap_or_else(|e| panic!("添加节点「{text}」失败：{e}"));
    doc.doc_order()
        .into_iter()
        .find(|id| doc.node(id).is_some_and(|n| n.text == text))
        .unwrap_or_else(|| panic!("找不到新节点「{text}」"))
}

#[cfg(test)]
mod tests {
    use super::Session;
    use mw_core::Command;

    /// 每个用例一个独占目录（并行测试下毫秒级名字会撞车）。
    fn temp_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mw-session-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    #[test]
    fn a_fresh_session_is_untitled_and_clean() {
        let s = Session::sample();
        assert_eq!(s.title(), "未命名.zmind");
        assert!(!s.dirty(), "刚建的文档没有未保存改动");
        assert!(s.path.is_none());
    }

    /// 大纲实拍夹具：三种行尾角标各自的来源都在，且不顶「未保存」圆点。
    #[test]
    fn outline_fixture_carries_marks_without_a_dirty_dot() {
        let s = Session::sample_with_marks();
        assert!(!s.dirty());
        assert!(s.doc.nodes.values().any(|n| n.note.is_some()));
        assert!(s.doc.nodes.values().any(|n| n.task.is_some()));
        assert!(s.doc.nodes.values().any(|n| !n.labels.is_empty()));
    }

    #[test]
    fn any_command_marks_the_session_dirty() {
        let mut s = Session::sample();
        let here = std::env::temp_dir();
        let _ = s.save_to(&here.join("mw-dirty-probe.zmind"));
        assert!(!s.dirty(), "存完就是干净的");
        let id = s.doc.root_id.clone();
        s.doc
            .apply(&Command::SetText {
                id,
                text: "改过了".into(),
            })
            .unwrap();
        assert!(s.dirty(), "改一条命令就该脏");
    }

    #[test]
    fn save_then_open_round_trips_the_document() {
        let dir = temp_dir();
        let path = dir.join("往返.zmind");
        let mut s = Session::sample();
        let before = s.doc.node_count();
        s.save_to(&path).expect("保存");

        let reopened = Session::open(&path).expect("重新打开");
        assert_eq!(reopened.doc.node_count(), before, "节点数要一致");
        assert_eq!(reopened.doc.root_id, s.doc.root_id, "根 id 要一致");
        assert_eq!(reopened.title(), "往返.zmind");
        assert!(!reopened.dirty(), "刚打开的文档是干净的");
        assert_eq!(
            reopened.doc.meta.title, "往返",
            "manifest 标题同步成文件名（不含扩展名）"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_without_a_path_is_an_error_not_a_silent_noop() {
        let mut s = Session::sample();
        assert!(s.save().is_err(), "没有路径时必须报错，让调用方去弹另存为");
    }

    #[test]
    fn a_blank_document_has_one_root_and_no_path() {
        let s = Session::blank();
        assert_eq!(s.doc.node_count(), 1);
        assert_eq!(s.doc.node(&s.doc.root_id).unwrap().text, "中心主题");
        assert!(!s.dirty(), "新建文档是干净的");
        assert!(s.path.is_none());
    }

    /// 扁平化为 (depth, text)：结构断言比数节点数更能抓住「层级挂错」。
    fn flatten(doc: &mw_core::Document) -> Vec<(usize, String)> {
        fn walk(doc: &mw_core::Document, id: &str, depth: usize, out: &mut Vec<(usize, String)>) {
            if let Some(n) = doc.node(id) {
                out.push((depth, n.text.clone()));
                for c in &n.children {
                    walk(doc, c, depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(doc, &doc.root_id, 0, &mut out);
        out
    }

    #[test]
    fn import_dispatch_recognizes_every_claimed_extension() {
        for ext in ["zmind", "md", "markdown", "txt", "opml", "mm", "xmind"] {
            assert!(
                Session::accepts(std::path::Path::new(&format!("a.{ext}"))),
                ".{ext} 是文档里承诺的导入格式"
            );
        }
        // 大小写不该影响分流（Windows 文件名常见全大写后缀）
        assert!(Session::accepts(std::path::Path::new("a.MD")));
        assert!(Session::accepts(std::path::Path::new("a.XMIND")));
        for ext in ["png", "pdf", "docx", ""] {
            assert!(
                !Session::accepts(std::path::Path::new(&format!("a.{ext}"))),
                ".{ext} 不该被当成可导入格式"
            );
        }
    }

    #[test]
    fn importing_markdown_produces_a_dirty_untitled_session() {
        let dir = temp_dir();
        let path = dir.join("大纲.md");
        std::fs::write(&path, "# 根\n\n- 甲\n- 乙\n").expect("写文件");
        let mut s = Session::import(&path).expect("导入");
        assert!(s.path.is_none(), "导入不是打开：还没有路径");
        assert!(s.dirty(), "导进来的内容还没落过盘");
        assert_eq!(
            flatten(&s.doc),
            vec![(0, "根".into()), (1, "甲".into()), (1, "乙".into())]
        );
        assert_eq!(s.title(), "大纲.md", "标签页显示来源文件名，而不是未命名");
        let out = dir.join("另存.zmind");
        s.save_to(&out).expect("另存为");
        assert_eq!(s.title(), "另存.zmind", "落盘后标题跟着新文件走");
        assert!(!s.dirty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn importing_indented_outline_rebuilds_depth() {
        let dir = temp_dir();
        let path = dir.join("规划.txt");
        std::fs::write(&path, "产品规划\n  市场调研\n    竞品分析\n  里程碑\n").expect("写文件");
        let s = Session::import(&path).expect("导入");
        assert_eq!(
            flatten(&s.doc),
            vec![
                (0, "产品规划".into()),
                (1, "市场调研".into()),
                (2, "竞品分析".into()),
                (1, "里程碑".into()),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn importing_opml_keeps_nesting_and_note() {
        let dir = temp_dir();
        let path = dir.join("交换.opml");
        std::fs::write(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0"><body>
  <outline text="根">
    <outline text="甲"><outline text="甲一" _note="一条备注"/></outline>
    <outline text="乙"/>
  </outline>
</body></opml>"#,
        )
        .expect("写文件");
        let s = Session::import(&path).expect("导入");
        assert_eq!(
            flatten(&s.doc),
            vec![
                (0, "根".into()),
                (1, "甲".into()),
                (2, "甲一".into()),
                (1, "乙".into()),
            ]
        );
        let note = s
            .doc
            .nodes
            .values()
            .find(|n| n.text == "甲一")
            .and_then(|n| n.note.as_deref());
        assert_eq!(note, Some("一条备注"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn importing_freemind_keeps_flags_link_and_rich_note() {
        let dir = temp_dir();
        let path = dir.join("旧友.mm");
        std::fs::write(
            &path,
            r##"<map version="1.0.1">
  <node TEXT="根">
    <node TEXT="甲" FOLDED="true" LINK="https://example.com">
      <richcontent TYPE="NOTE"><html><body><p>第一行</p><p>第二行</p></body></html></richcontent>
    </node>
    <node TEXT="乙" COLOR="#ff0000"/>
  </node>
</map>"##,
        )
        .expect("写文件");
        let s = Session::import(&path).expect("导入");
        assert_eq!(
            flatten(&s.doc),
            vec![(0, "根".into()), (1, "甲".into()), (1, "乙".into())]
        );
        let a = s.doc.nodes.values().find(|n| n.text == "甲").unwrap();
        assert!(a.collapsed);
        assert_eq!(a.link.as_deref(), Some("https://example.com"));
        assert_eq!(a.note.as_deref(), Some("第一行\n第二行"));
        let b = s.doc.nodes.values().find(|n| n.text == "乙").unwrap();
        assert_eq!(
            b.style.as_ref().and_then(|x| x.text_color.as_deref()),
            Some("#ff0000")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn importing_xmind_walks_zen_json_into_the_session() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = temp_dir();
        let path = dir.join("同事.xmind");
        let json = r#"[
          {
            "id": "sheet1",
            "rootTopic": {
              "id": "t0",
              "title": "中心主题",
              "children": { "attached": [
                { "id": "t1", "title": "分支甲", "notes": { "plain": { "content": "导入备注" } } },
                { "id": "t2", "title": "分支乙", "branch": "folded",
                  "children": { "attached": [ { "id": "t3", "title": "孙节点" } ] } }
              ] }
            }
          }
        ]"#;
        let file = std::fs::File::create(&path).expect("建文件");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("content.json", SimpleFileOptions::default())
            .expect("写条目");
        zip.write_all(json.as_bytes()).expect("写内容");
        zip.finish().expect("收尾");

        let s = Session::import(&path).expect("导入 XMind");
        assert_eq!(
            flatten(&s.doc),
            vec![
                (0, "中心主题".into()),
                (1, "分支甲".into()),
                (1, "分支乙".into()),
                (2, "孙节点".into()),
            ]
        );
        let a = s.doc.nodes.values().find(|n| n.text == "分支甲").unwrap();
        assert_eq!(a.note.as_deref(), Some("导入备注"));
        let b = s.doc.nodes.values().find(|n| n.text == "分支乙").unwrap();
        assert!(b.collapsed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn zmind_round_trip_keeps_advanced_elements_byte_for_byte() {
        let dir = temp_dir();
        let path = dir.join("全量.zmind");
        let mut s = Session::sample();
        s.save_to(&path).expect("保存");

        let reopened = Session::open(&path).expect("重新打开");
        assert_eq!(reopened.doc.id, s.doc.id, "文档 id 变了");
        assert_eq!(reopened.doc.root_id, s.doc.root_id, "根 id 变了");
        assert_eq!(reopened.doc.meta, s.doc.meta, "元数据（含布局）变了");
        assert_eq!(reopened.doc.nodes.len(), s.doc.nodes.len());
        for (id, n) in &s.doc.nodes {
            assert_eq!(&reopened.doc.nodes[id], n, "节点 {id} 往返后不一致");
        }
        // 夹具里的标签 / 概要 / 边界 / 关系线各至少一份，确保上面比的是「有内容」的文档。
        assert!(s.doc.nodes.values().any(|n| !n.labels.is_empty()));
        assert!(s.doc.nodes.values().any(|n| !n.summaries.is_empty()));
        assert!(s.doc.nodes.values().any(|n| !n.boundaries.is_empty()));
        assert!(s.doc.nodes.values().any(|n| !n.relationships.is_empty()));
        assert!(reopened.doc.validate().is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_recovered_session_saves_back_to_the_original_path() {
        let dir = temp_dir();
        let doc = dir.join("正本.zmind");
        let rec = dir.join("正本.autosave.zmind");
        let mut s = Session::sample();
        s.save_to(&doc).expect("写正本");
        s.save_to(&rec).expect("写恢复点");

        let recovered = Session::open_recovered(&rec, &doc).expect("恢复");
        assert_eq!(recovered.path.as_deref(), Some(doc.as_path()));
        assert!(recovered.dirty(), "恢复出来的内容还没写回正本");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn opening_a_missing_file_reports_rather_than_panics() {
        let missing = std::env::temp_dir().join("mw-does-not-exist-42.zmind");
        let err = match Session::open(&missing) {
            Ok(_) => panic!("不存在的文件不该打开成功"),
            Err(e) => e,
        };
        assert!(
            err.contains("打开失败"),
            "错误前缀要能直接显示给用户：{err}"
        );
    }
}
