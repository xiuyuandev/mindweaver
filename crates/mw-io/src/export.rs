//! 导出器：`Document` → 外部文本格式（有损项见文档 05 §7）。

use mw_core::Document;

/// XML 属性/文本转义。
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// 多行文本压成单行（Markdown 标题 / 大纲行无法承载换行）。
fn one_line(text: &str) -> String {
    text.replace('\n', " ")
}

/// 概念图导出的行标签：命题的另一半是**链接短语**，丢了层级就只剩名词。
///
/// 层级里没有「边」的位置（Markdown 标题、OPML 的 `text`、FreeMind 的 `TEXT` 都只有一个
/// 字符串），所以短语跟着终点概念走：`因为 → 用户痛点`。非概念图不会走到这里。
fn concept_label(doc: &Document, row: &mw_core::ConceptRow) -> String {
    let text = doc
        .node(&row.id)
        .map(|n| one_line(&n.text))
        .unwrap_or_default();
    match row.via.as_deref() {
        Some(via) => format!("{via} → {text}"),
        None => text,
    }
}

/// 概念图的 Markdown 文首标注（`docs/30` §5.8「在文首标注概念图」）。
const CONCEPT_NOTE: &str = "概念图 · 按链接层级展开";

/// Markdown 导出：根 = H1，深度 1–5 → H2–H6，更深转缩进列表；备注 → 引用块。
pub fn export_markdown(doc: &Document) -> String {
    fn walk(doc: &Document, id: &str, depth: usize, out: &mut String) {
        let node = match doc.node(id) {
            Some(n) => n,
            None => return,
        };
        let text = one_line(&node.text);
        if depth == 0 {
            out.push_str(&format!("# {text}\n\n"));
        } else if depth <= 5 {
            out.push_str(&format!("{} {text}\n\n", "#".repeat(depth + 1)));
        } else {
            out.push_str(&format!("{}- {text}\n", "  ".repeat(depth - 6)));
        }
        if let Some(note) = node.note.as_deref().filter(|s| !s.is_empty()) {
            for line in note.lines() {
                out.push_str(&format!("> {line}\n"));
            }
            out.push('\n');
        }
        for c in &node.children {
            walk(doc, c, depth + 1, out);
        }
    }
    // 概念图的层级不在 `children` 里，必须走生成树（CM-07）。
    if doc.is_concept_map() {
        let mut out = String::new();
        for (i, row) in mw_core::concept_rows(doc).iter().enumerate() {
            let text = concept_label(doc, row);
            let depth = row.depth as usize;
            if depth == 0 {
                out.push_str(&format!("# {text}\n\n"));
                if i == 0 {
                    out.push_str(&format!("> {CONCEPT_NOTE}\n\n"));
                }
            } else if depth <= 5 {
                out.push_str(&format!("{} {text}\n\n", "#".repeat(depth + 1)));
            } else {
                out.push_str(&format!("{}- {text}\n", "  ".repeat(depth - 6)));
            }
            if let Some(note) = doc
                .node(&row.id)
                .and_then(|n| n.note.as_deref())
                .filter(|s| !s.is_empty())
            {
                for line in note.lines() {
                    out.push_str(&format!("> {line}\n"));
                }
                out.push('\n');
            }
        }
        return out;
    }
    let mut out = String::new();
    walk(doc, &doc.root_id, 0, &mut out);
    out
}

/// 缩进纯文本大纲导出（4 空格一级）。
pub fn export_outline(doc: &Document) -> String {
    fn walk(doc: &Document, id: &str, depth: usize, out: &mut String) {
        let node = match doc.node(id) {
            Some(n) => n,
            None => return,
        };
        out.push_str(&"    ".repeat(depth));
        out.push_str(&one_line(&node.text));
        out.push('\n');
        for c in &node.children {
            walk(doc, c, depth + 1, out);
        }
    }
    if doc.is_concept_map() {
        let mut out = String::new();
        for row in mw_core::concept_rows(doc) {
            out.push_str(&"    ".repeat(row.depth as usize));
            out.push_str(&concept_label(doc, &row));
            out.push('\n');
        }
        return out;
    }
    let mut out = String::new();
    walk(doc, &doc.root_id, 0, &mut out);
    out
}

/// 生成树的邻接表：父概念 → 子行下标（顺序就是生成树的前序）。
///
/// OPML / FreeMind 是嵌套格式，只能靠父子关系展开；下标指向 `rows`，
/// 这样两处递归都不用再借一份 `HashMap<String, Vec<String>>` 的克隆。
fn concept_children(rows: &[mw_core::ConceptRow]) -> std::collections::HashMap<&str, Vec<usize>> {
    let mut map: std::collections::HashMap<&str, Vec<usize>> = std::collections::HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        if let Some(parent) = row.parent.as_deref() {
            map.entry(parent).or_default().push(i);
        }
    }
    map
}

/// OPML 2.0 导出。
pub fn export_opml(doc: &Document) -> String {
    fn write_outline(doc: &Document, id: &str, indent: usize, out: &mut String) {
        let node = match doc.node(id) {
            Some(n) => n,
            None => return,
        };
        let pad = "  ".repeat(indent);
        out.push_str(&pad);
        out.push_str(&format!("<outline text=\"{}\"", esc(&one_line(&node.text))));
        if let Some(note) = node.note.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!(" _note=\"{}\"", esc(note)));
        }
        if let Some(link) = node.link.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!(" _link=\"{}\"", esc(link)));
        }
        if node.collapsed {
            out.push_str(" _collapsed=\"true\"");
        }
        if !node.labels.is_empty() {
            out.push_str(&format!(" _labels=\"{}\"", esc(&node.labels.join("|"))));
        }
        if node.children.is_empty() {
            out.push_str("/>\n");
            return;
        }
        out.push_str(">\n");
        for c in &node.children {
            write_outline(doc, c, indent + 1, out);
        }
        out.push_str(&pad);
        out.push_str("</outline>\n");
    }

    // 概念图：按生成树嵌套；标注挂在 `<title>` 上（OPML 没有注释位，title 是唯一
    // 每个阅读器都会显示的一行）。`_via` 让链接短语跟着终点走，导入侧忽略未知属性。
    fn write_concept(
        doc: &Document,
        rows: &[mw_core::ConceptRow],
        kids: &std::collections::HashMap<&str, Vec<usize>>,
        idx: usize,
        indent: usize,
        out: &mut String,
    ) {
        let row = &rows[idx];
        let Some(node) = doc.node(&row.id) else {
            return;
        };
        let pad = "  ".repeat(indent);
        out.push_str(&pad);
        out.push_str(&format!(
            "<outline text=\"{}\"",
            esc(&concept_label(doc, row))
        ));
        if let Some(note) = node.note.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!(" _note=\"{}\"", esc(note)));
        }
        if let Some(link) = node.link.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!(" _link=\"{}\"", esc(link)));
        }
        if !node.labels.is_empty() {
            out.push_str(&format!(" _labels=\"{}\"", esc(&node.labels.join("|"))));
        }
        let children = kids.get(row.id.as_str()).cloned().unwrap_or_default();
        if children.is_empty() {
            out.push_str("/>\n");
            return;
        }
        out.push_str(">\n");
        for c in children {
            write_concept(doc, rows, kids, c, indent + 1, out);
        }
        out.push_str(&pad);
        out.push_str("</outline>\n");
    }

    let concept = doc.is_concept_map();
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head>\n    <title>",
    );
    out.push_str(&esc(&doc.meta.title));
    if concept {
        out.push_str(&format!("（{CONCEPT_NOTE}）"));
    }
    out.push_str("</title>\n  </head>\n  <body>\n");
    if concept {
        let rows = mw_core::concept_rows(doc);
        let kids = concept_children(&rows);
        for (i, row) in rows.iter().enumerate() {
            if row.depth == 0 {
                write_concept(doc, &rows, &kids, i, 2, &mut out);
            }
        }
    } else {
        write_outline(doc, &doc.root_id, 2, &mut out);
    }
    out.push_str("  </body>\n</opml>\n");
    out
}

/// FreeMind `.mm` 导出（文本/折叠/链接/文字色/备注均映射，见 05 §7）。
pub fn export_freemind(doc: &Document) -> String {
    fn write_node(doc: &Document, id: &str, indent: usize, out: &mut String) {
        let node = match doc.node(id) {
            Some(n) => n,
            None => return,
        };
        let pad = "  ".repeat(indent);
        out.push_str(&pad);
        out.push_str(&format!("<node TEXT=\"{}\"", esc(&node.text)));
        if node.collapsed {
            out.push_str(" FOLDED=\"true\"");
        }
        if let Some(link) = node.link.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!(" LINK=\"{}\"", esc(link)));
        }
        if let Some(color) = node.style.as_ref().and_then(|s| s.text_color.as_deref()) {
            out.push_str(&format!(" COLOR=\"{}\"", esc(color)));
        }
        if !node.labels.is_empty() {
            out.push_str(&format!(" LABELS=\"{}\"", esc(&node.labels.join("|"))));
        }
        let note = node.note.as_deref().filter(|s| !s.is_empty());
        if node.children.is_empty() && note.is_none() {
            out.push_str("/>\n");
            return;
        }
        out.push_str(">\n");
        if let Some(note) = note {
            let html = note
                .lines()
                .map(|l| format!("<p>{}</p>", esc(l)))
                .collect::<Vec<_>>()
                .join("");
            out.push_str(&format!(
                "{}  <richcontent TYPE=\"NOTE\"><html><body>{}</body></html></richcontent>\n",
                pad, html
            ));
        }
        for c in &node.children {
            write_node(doc, c, indent + 1, out);
        }
        out.push_str(&pad);
        out.push_str("</node>\n");
    }

    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<map version=\"1.0.1\">\n");
    if doc.is_concept_map() {
        // 概念图：生成树 + 文首注释（`.mm` 没有「文件标题」字段，注释是唯一不进结构的标注位）。
        out.push_str(&format!("  <!-- {CONCEPT_NOTE} -->\n"));
        fn write_concept_node(
            doc: &Document,
            rows: &[mw_core::ConceptRow],
            kids: &std::collections::HashMap<&str, Vec<usize>>,
            idx: usize,
            indent: usize,
            out: &mut String,
        ) {
            let row = &rows[idx];
            let Some(node) = doc.node(&row.id) else {
                return;
            };
            let pad = "  ".repeat(indent);
            out.push_str(&pad);
            out.push_str(&format!("<node TEXT=\"{}\"", esc(&concept_label(doc, row))));
            if let Some(link) = node.link.as_deref().filter(|s| !s.is_empty()) {
                out.push_str(&format!(" LINK=\"{}\"", esc(link)));
            }
            if let Some(color) = node.style.as_ref().and_then(|s| s.text_color.as_deref()) {
                out.push_str(&format!(" COLOR=\"{}\"", esc(color)));
            }
            if !node.labels.is_empty() {
                out.push_str(&format!(" LABELS=\"{}\"", esc(&node.labels.join("|"))));
            }
            let note = node.note.as_deref().filter(|s| !s.is_empty());
            let children = kids.get(row.id.as_str()).cloned().unwrap_or_default();
            if children.is_empty() && note.is_none() {
                out.push_str("/>\n");
                return;
            }
            out.push_str(">\n");
            if let Some(note) = note {
                let html = note
                    .lines()
                    .map(|l| format!("<p>{}</p>", esc(l)))
                    .collect::<Vec<_>>()
                    .join("");
                out.push_str(&format!(
                    "{}  <richcontent TYPE=\"NOTE\"><html><body>{}</body></html></richcontent>\n",
                    pad, html
                ));
            }
            for c in children {
                write_concept_node(doc, rows, kids, c, indent + 1, out);
            }
            out.push_str(&pad);
            out.push_str("</node>\n");
        }
        let rows = mw_core::concept_rows(doc);
        let kids = concept_children(&rows);
        for (i, row) in rows.iter().enumerate() {
            if row.depth == 0 {
                write_concept_node(doc, &rows, &kids, i, 1, &mut out);
            }
        }
    } else {
        write_node(doc, &doc.root_id, 1, &mut out);
    }
    out.push_str("</map>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{import_opml, import_outline};
    use mw_core::{new_id, Node};

    fn doc_with(children: &[&str]) -> (Document, String) {
        let mut d = Document::new("根");
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().text = "根".into();
        for (i, t) in children.iter().enumerate() {
            let id = new_id();
            let mut n = Node::new(id.clone(), Some(root.clone()), *t);
            n.note = Some(format!("备注{i}"));
            d.nodes.insert(id.clone(), n);
            d.nodes.get_mut(&root).unwrap().children.push(id);
        }
        (d, root)
    }

    #[test]
    fn opml_and_freemind_roundtrip_keep_notes_links_labels_and_collapse() {
        let mut d = Document::new("根");
        let root = d.root_id.clone();
        d.nodes.get_mut(&root).unwrap().text = "根".into();
        let id = new_id();
        let mut n = Node::new(id.clone(), Some(root.clone()), "子主题");
        n.note = Some("备注内容".into());
        n.link = Some("https://example.com/x".into());
        n.labels = vec!["重要".into(), "Q3".into()];
        n.collapsed = true;
        d.nodes.insert(id.clone(), n);
        d.nodes.get_mut(&root).unwrap().children.push(id);

        for (name, text) in [("opml", export_opml(&d)), ("freemind", export_freemind(&d))] {
            let doc = if name == "opml" {
                import_opml(&text).unwrap()
            } else {
                crate::import::import_freemind(&text).unwrap()
            };
            let child = doc
                .nodes
                .values()
                .find(|n| n.text == "子主题")
                .unwrap_or_else(|| panic!("{name} 丢了子主题"));
            assert_eq!(child.note.as_deref(), Some("备注内容"), "{name}");
            assert_eq!(
                child.link.as_deref(),
                Some("https://example.com/x"),
                "{name}"
            );
            assert_eq!(
                child.labels,
                vec!["重要".to_string(), "Q3".to_string()],
                "{name}"
            );
            assert!(child.collapsed, "{name}");
        }
    }

    #[test]
    fn markdown_export_has_headings_and_notes() {
        let (d, _) = doc_with(&["甲 & <乙>"]);
        let md = export_markdown(&d);
        assert!(md.starts_with("# 根"));
        assert!(md.contains("## 甲 & <乙>"));
        assert!(md.contains("> 备注0"));
    }

    #[test]
    fn xml_special_chars_survive_roundtrip() {
        let (d, _) = doc_with(&["A & B <C> \"D\""]);
        let xml = export_opml(&d);
        assert!(xml.contains("A &amp; B &lt;C&gt; &quot;D&quot;"));
        let back = import_opml(&xml).unwrap();
        let texts: Vec<String> = back.nodes.values().map(|n| n.text.clone()).collect();
        assert!(texts.contains(&"A & B <C> \"D\"".to_string()));
    }

    #[test]
    fn outline_export_roundtrip() {
        let (d, _) = doc_with(&["甲", "乙"]);
        let text = export_outline(&d);
        let back = import_outline(&text).unwrap();
        assert_eq!(back.nodes[&back.root_id].text, "根");
        assert_eq!(back.nodes[&back.root_id].children.len(), 2);
    }

    /// CM-07 夹具：主概念「用户痛点」→「竞品访谈」（因为）→「留存低」（导致），
    /// 外加一条回到主概念的回边（导出不能因此死循环）。
    fn concept_doc() -> Document {
        use mw_core::{Command, LayoutSettings};
        let mut d = Document::new("概念图");
        d.apply(&Command::SetLayout {
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = d.root_id.clone();
        d.apply(&Command::SetText {
            id: root.clone(),
            text: "用户痛点".into(),
        })
        .unwrap();
        d.apply(&Command::MoveConcept {
            id: root.clone(),
            x: 0.0,
            y: 0.0,
        })
        .unwrap();
        let mut ids = vec![root];
        for (i, text) in ["竞品访谈", "留存低"].iter().enumerate() {
            d.apply(&Command::AddFloatingTopic {
                text: (*text).into(),
                x: 200.0 * (i as f32 + 1.0),
                y: 0.0,
            })
            .unwrap();
            let id = d
                .nodes
                .values()
                .find(|n| &n.text == text)
                .map(|n| n.id.clone())
                .unwrap();
            ids.push(id);
        }
        for (from, to, label) in [(0, 1, "因为"), (1, 2, "导致"), (2, 0, "反过来")] {
            d.apply(&Command::AddRelationship {
                from: ids[from].clone(),
                to: ids[to].clone(),
                label: Some(label.into()),
            })
            .unwrap();
        }
        d
    }

    /// CM-07：概念图的四种文本导出都走同一张生成树，且文首带标注。
    /// 反例：把 `concept_label` 里的短语摘掉，`因为 → 竞品访谈` 立刻消失。
    #[test]
    fn concept_map_exports_follow_the_generated_tree() {
        let d = concept_doc();
        let md = export_markdown(&d);
        assert!(md.starts_with("# 用户痛点"), "主概念是 H1：{md}");
        assert!(
            md.contains("> 概念图 · 按链接层级展开"),
            "Markdown 文首要标注：{md}"
        );
        assert!(md.contains("## 因为 → 竞品访谈"), "{md}");
        assert!(md.contains("### 导致 → 留存低"), "{md}");
        // 回边（留存低 → 用户痛点）不该再展开一次主概念。
        assert_eq!(md.matches("用户痛点").count(), 1, "{md}");

        let txt = export_outline(&d);
        assert_eq!(
            txt,
            "用户痛点\n    因为 → 竞品访谈\n        导致 → 留存低\n"
        );

        let opml = export_opml(&d);
        assert!(opml.contains("<title>概念图（概念图 · 按链接层级展开）</title>"));
        // 嵌套：竞品访谈在外面，留存低在它里面。
        let outer = opml.find("text=\"因为 → 竞品访谈\"").unwrap();
        let inner = opml.find("text=\"导致 → 留存低\"").unwrap();
        let close = opml.find("</outline>").unwrap();
        assert!(outer < inner && inner < close, "{opml}");

        let mm = export_freemind(&d);
        assert!(mm.contains("<!-- 概念图 · 按链接层级展开 -->"), "{mm}");
        assert!(mm.contains("TEXT=\"因为 → 竞品访谈\""), "{mm}");
        assert!(mm.contains("TEXT=\"导致 → 留存低\""), "{mm}");
    }
}
