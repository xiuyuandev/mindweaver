//! 导入器：把外部文本结构转成 `mw_core::Document`。

use mw_core::{new_id, Document, Node, StyleOverride};
use quick_xml::events::BytesStart;

use crate::IoError;

/// 在 `parent` 下追加一个子节点并返回新节点 id。
fn add_child(doc: &mut Document, parent: &str, text: &str) -> String {
    let id = new_id();
    let node = Node::new(id.clone(), Some(parent.to_string()), text.to_string());
    doc.nodes.insert(id.clone(), node);
    if let Some(p) = doc.nodes.get_mut(parent) {
        p.children.push(id.clone());
    }
    id
}

fn append_note(doc: &mut Document, id: &str, addition: &str) {
    if addition.is_empty() {
        return;
    }
    if let Some(n) = doc.nodes.get_mut(id) {
        let note = n.note.get_or_insert_with(String::new);
        if !note.is_empty() {
            note.push('\n');
        }
        note.push_str(addition);
    }
}

fn finish(mut doc: Document) -> Result<Document, IoError> {
    for n in doc.nodes.values_mut() {
        n.text = n.text.trim().to_string();
    }
    doc.validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    Ok(doc)
}

// ---------------------------------------------------------------- 缩进大纲

fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn detect_unit(raw: &[(usize, &str)]) -> usize {
    let mut unit = usize::MAX;
    for w in raw.windows(2) {
        if w[1].0 > w[0].0 {
            unit = unit.min(w[1].0 - w[0].0);
        }
    }
    if unit == usize::MAX {
        2
    } else {
        unit.max(1)
    }
}

/// 缩进纯文本 → 文档。首行为根，其余按缩进确定层级。
pub fn import_outline(text: &str) -> Result<Document, IoError> {
    let raw: Vec<(usize, &str)> = text
        .lines()
        .filter_map(|l| {
            let t = l.trim_end();
            if t.trim().is_empty() {
                return None;
            }
            Some((indent_width(t), t.trim_start()))
        })
        .collect();
    if raw.is_empty() {
        return Err(IoError::Empty);
    }
    let unit = detect_unit(&raw);
    let root_text = raw[0].1;
    let mut doc = Document::new(root_text);
    doc.meta.title = root_text.chars().take(60).collect();
    let root = doc.root_id.clone();
    if let Some(n) = doc.nodes.get_mut(&root) {
        n.text = root_text.to_string();
    }

    let base = raw[0].0;
    let mut stack: Vec<(usize, String)> = vec![(0, root.clone())];
    for (indent, t) in raw.iter().skip(1) {
        let depth = (*indent).saturating_sub(base) / unit + 1;
        while stack.len() > 1 && stack.last().unwrap().0 >= depth {
            stack.pop();
        }
        let parent = stack.last().unwrap().1.clone();
        let id = add_child(&mut doc, &parent, t);
        stack.push((depth, id));
    }
    finish(doc)
}

// ---------------------------------------------------------------- Markdown

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = line.get(hashes..)?;
    let rest = rest.strip_prefix(' ')?.trim();
    if rest.is_empty() {
        return None;
    }
    Some((hashes as u8, rest.to_string()))
}

fn parse_list_item(line: &str) -> Option<(usize, String)> {
    let indent = line.chars().take_while(|c| *c == ' ').count();
    let rest = line.get(indent..)?;
    let stripped = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))
        .or_else(|| rest.strip_prefix("+ "))
        .or_else(|| {
            let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 {
                return None;
            }
            let after = rest.get(digits..)?;
            after
                .strip_prefix(". ")
                .or_else(|| after.strip_prefix(") "))
        })?;
    let item = stripped.trim();
    if item.is_empty() {
        return None;
    }
    Some((indent, item.to_string()))
}

/// Markdown → 文档。H1–H6 建主干，列表按缩进挂载，引用行归入上一节点备注。
pub fn import_markdown(text: &str) -> Result<Document, IoError> {
    let mut doc = Document::new("导入内容");
    let root = doc.root_id.clone();
    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut list_stack: Vec<(usize, String)> = Vec::new();
    let mut last_node: Option<String> = None;
    let mut has_content = false;

    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            continue;
        }
        if let Some((level, title)) = parse_heading(trimmed) {
            has_content = true;
            heading_stack.retain(|(lvl, _)| *lvl < level);
            list_stack.clear();
            let id = if heading_stack.is_empty() && doc.nodes[&root].children.is_empty() {
                if let Some(n) = doc.nodes.get_mut(&root) {
                    n.text = title.clone();
                }
                doc.meta.title = title.chars().take(60).collect();
                root.clone()
            } else {
                let parent = heading_stack
                    .last()
                    .map(|(_, id)| id.clone())
                    .unwrap_or_else(|| root.clone());
                add_child(&mut doc, &parent, &title)
            };
            heading_stack.push((level, id.clone()));
            last_node = Some(id);
        } else if let Some((indent, item)) = parse_list_item(trimmed) {
            has_content = true;
            list_stack.retain(|(ind, _)| *ind < indent);
            let parent = list_stack
                .last()
                .map(|(_, id)| id.clone())
                .or_else(|| heading_stack.last().map(|(_, id)| id.clone()))
                .unwrap_or_else(|| root.clone());
            let id = add_child(&mut doc, &parent, &item);
            list_stack.push((indent, id.clone()));
            last_node = Some(id);
        } else if let Some(rest) = trimmed.trim_start().strip_prefix('>') {
            if let Some(id) = last_node.clone() {
                append_note(&mut doc, &id, rest.trim());
            }
        } else if let Some(id) = last_node.clone() {
            append_note(&mut doc, &id, trimmed.trim());
        }
    }
    if !has_content {
        return Err(IoError::Empty);
    }
    finish(doc)
}

// ---------------------------------------------------------------- OPML / FreeMind 公共

#[derive(Default)]
struct PendingNode {
    title: String,
    note: Option<String>,
    collapsed: bool,
    color: Option<String>,
    link: Option<String>,
    labels: Vec<String>,
}

fn parse_labels(v: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in v.split('|') {
        let t = part.trim();
        if t.is_empty() || t.chars().count() > 64 || out.iter().any(|x| x == t) {
            continue;
        }
        out.push(t.to_string());
        if out.len() >= 5 {
            break;
        }
    }
    out
}

fn insert_node(doc: &mut Option<Document>, stack: &mut Vec<String>, p: PendingNode) {
    match doc {
        None => {
            let title = if p.title.is_empty() {
                "导入内容".to_string()
            } else {
                p.title.clone()
            };
            let mut d = Document::new(&title);
            d.meta.title = title.chars().take(60).collect();
            let root = d.root_id.clone();
            if let Some(n) = d.nodes.get_mut(&root) {
                n.text = title;
                n.note = p.note;
                n.collapsed = p.collapsed;
                n.link = p.link;
                n.labels = p.labels;
                if let Some(c) = p.color {
                    n.style = Some(StyleOverride {
                        text_color: Some(c),
                        ..Default::default()
                    });
                }
            }
            *doc = Some(d);
            stack.push(root);
        }
        Some(d) => {
            let parent = stack.last().cloned().unwrap_or_else(|| d.root_id.clone());
            let id = add_child(d, &parent, &p.title);
            if let Some(n) = d.nodes.get_mut(&id) {
                n.note = p.note;
                n.collapsed = p.collapsed;
                n.link = p.link;
                n.labels = p.labels;
                if let Some(c) = p.color {
                    n.style = Some(StyleOverride {
                        text_color: Some(c),
                        ..Default::default()
                    });
                }
            }
            stack.push(id);
        }
    }
}

fn attr_value(attr: &quick_xml::events::attributes::Attribute) -> Result<String, IoError> {
    attr.unescape_value()
        .map(|v| v.into_owned())
        .map_err(|e| IoError::Xml(e.to_string()))
}

fn outline_pending(e: &BytesStart) -> Result<PendingNode, IoError> {
    let mut p = PendingNode::default();
    for attr in e.attributes() {
        let a = attr.map_err(|err| IoError::Xml(err.to_string()))?;
        let val = attr_value(&a)?;
        match a.key.as_ref() {
            b"text" | b"title" => p.title = val,
            b"_note" => p.note = Some(val),
            b"_link" => p.link = Some(val),
            b"_collapsed" => p.collapsed = val.eq_ignore_ascii_case("true"),
            b"_labels" => p.labels = parse_labels(&val),
            _ => {}
        }
    }
    Ok(p)
}

fn freemind_pending(e: &BytesStart) -> Result<PendingNode, IoError> {
    let mut p = PendingNode::default();
    for attr in e.attributes() {
        let a = attr.map_err(|err| IoError::Xml(err.to_string()))?;
        let val = attr_value(&a)?;
        match a.key.as_ref() {
            b"TEXT" => p.title = val,
            b"FOLDED" => p.collapsed = val.eq_ignore_ascii_case("true"),
            b"COLOR" => p.color = Some(val),
            b"LINK" => p.link = Some(val),
            b"LABELS" => p.labels = parse_labels(&val),
            _ => {}
        }
    }
    Ok(p)
}

/// OPML → 文档（嵌套 outline；text/title 为标题，`_note` 为备注）。
pub fn import_opml(text: &str) -> Result<Document, IoError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(text);
    let mut doc: Option<Document> = None;
    let mut stack: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Err(e) => return Err(IoError::Xml(e.to_string())),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"outline" {
                    let p = outline_pending(&e)?;
                    insert_node(&mut doc, &mut stack, p);
                }
            }
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"outline" {
                    let p = outline_pending(&e)?;
                    insert_node(&mut doc, &mut stack, p);
                    stack.pop(); // 自闭合元素没有 End 事件
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"outline" {
                    stack.pop();
                }
            }
            Ok(_) => {}
        }
    }
    let doc = doc.ok_or(IoError::Empty)?;
    finish(doc)
}

/// FreeMind `.mm` → 文档（node 元素；TEXT/FOLDED/COLOR/LINK；richcontent 文本归入备注）。
pub fn import_freemind(text: &str) -> Result<Document, IoError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(text);
    let mut doc: Option<Document> = None;
    let mut stack: Vec<String> = Vec::new();
    let mut rich_depth = 0usize;
    let mut note_buf = String::new();
    let mut last_node: Option<String> = None;

    loop {
        match reader.read_event() {
            Err(e) => return Err(IoError::Xml(e.to_string())),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                if rich_depth > 0 {
                    if name == b"richcontent" {
                        rich_depth += 1;
                    } else if name == b"p" && !note_buf.is_empty() {
                        // richcontent 内的段落边界还原为换行
                        note_buf.push('\n');
                    }
                    continue;
                }
                match name.as_slice() {
                    b"richcontent" => {
                        rich_depth = 1;
                        note_buf.clear();
                    }
                    b"node" => {
                        let p = freemind_pending(&e)?;
                        insert_node(&mut doc, &mut stack, p);
                        last_node = stack.last().cloned();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name().as_ref().to_vec();
                if rich_depth > 0 || name.as_slice() != b"node" {
                    continue;
                }
                let p = freemind_pending(&e)?;
                insert_node(&mut doc, &mut stack, p);
                stack.pop();
                last_node = stack.last().cloned();
            }
            Ok(Event::Text(t)) => {
                if rich_depth > 0 {
                    if let Ok(s) = t.unescape() {
                        note_buf.push_str(&s);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_vec();
                if rich_depth > 0 {
                    if name == b"richcontent" {
                        rich_depth -= 1;
                        if rich_depth == 0 {
                            if let (Some(d), Some(nid)) = (doc.as_mut(), last_node.clone()) {
                                let normalized = note_buf
                                    .lines()
                                    .map(|l| l.trim())
                                    .filter(|l| !l.is_empty())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                if !normalized.is_empty() {
                                    if let Some(n) = d.nodes.get_mut(&nid) {
                                        n.note = Some(normalized);
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }
                if name.as_slice() == b"node" {
                    stack.pop();
                }
            }
            Ok(_) => {}
        }
    }
    let doc = doc.ok_or(IoError::Empty)?;
    finish(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 扁平化为 (depth, text) 序列，便于比较结构。
    fn flatten(doc: &Document) -> Vec<(usize, String)> {
        fn walk(doc: &Document, id: &str, depth: usize, out: &mut Vec<(usize, String)>) {
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
    fn outline_import_builds_hierarchy() {
        let doc = import_outline("产品规划\n  市场调研\n    竞品分析\n  里程碑\n").unwrap();
        assert_eq!(
            flatten(&doc),
            vec![
                (0, "产品规划".into()),
                (1, "市场调研".into()),
                (2, "竞品分析".into()),
                (1, "里程碑".into()),
            ]
        );
    }

    #[test]
    fn markdown_import_headings_lists_and_notes() {
        let md = "# 标题\n## 子主题\n- 项目一\n  - 项目二\n> 这是备注\n";
        let doc = import_markdown(md).unwrap();
        assert_eq!(
            flatten(&doc),
            vec![
                (0, "标题".into()),
                (1, "子主题".into()),
                (2, "项目一".into()),
                (3, "项目二".into()),
            ]
        );
        let n2 = doc
            .nodes
            .values()
            .find(|n| n.text == "项目二")
            .expect("应有节点 项目二");
        assert_eq!(n2.note.as_deref(), Some("这是备注"));
    }

    #[test]
    fn opml_roundtrip_preserves_structure_and_note() {
        use crate::export::export_opml;
        let mut src = Document::new("根");
        let root = src.root_id.clone();
        {
            let n = src.nodes.get_mut(&root).unwrap();
            n.text = "根".into();
            n.note = Some("根备注".into());
        }
        let a = add_child(&mut src, &root, "甲");
        add_child(&mut src, &a, "甲一");
        add_child(&mut src, &root, "乙");

        let xml = export_opml(&src);
        let back = import_opml(&xml).unwrap();
        assert_eq!(flatten(&back), flatten(&src));
        assert_eq!(back.nodes[&back.root_id].note.as_deref(), Some("根备注"));
    }

    #[test]
    fn freemind_roundtrip_with_style_and_note() {
        use crate::export::export_freemind;
        let mut src = Document::new("根");
        let root = src.root_id.clone();
        {
            let n = src.nodes.get_mut(&root).unwrap();
            n.text = "根".into();
        }
        let a = add_child(&mut src, &root, "甲");
        {
            let n = src.nodes.get_mut(&a).unwrap();
            n.collapsed = true;
            n.link = Some("https://example.com".into());
            n.note = Some("第一行\n第二行".into());
            n.style = Some(StyleOverride {
                text_color: Some("#ff0000".into()),
                ..Default::default()
            });
        }

        let xml = export_freemind(&src);
        let back = import_freemind(&xml).unwrap();
        assert_eq!(flatten(&back), flatten(&src));
        let na = back
            .nodes
            .values()
            .find(|n| n.text == "甲")
            .expect("应有节点 甲");
        assert!(na.collapsed);
        assert_eq!(na.link.as_deref(), Some("https://example.com"));
        assert_eq!(na.note.as_deref(), Some("第一行\n第二行"));
        assert_eq!(
            na.style.as_ref().and_then(|s| s.text_color.as_deref()),
            Some("#ff0000")
        );
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(matches!(import_outline("  \n \n"), Err(IoError::Empty)));
        assert!(matches!(import_markdown("\n\n"), Err(IoError::Empty)));
    }
}
