//! XMind 导入：双解析器——
//! - Zen / 新版（zip 内 `content.json`）
//! - XMind 8 旧版（zip 内 `content.xml`）
//!
//! 映射：topic → 节点（title/notes/折叠/标记尽力映射，见文档 05 §7 导入矩阵）。

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use mw_core::{new_id, DocMeta, Document, Node, Relationship};
use serde_json::{json, Value};

use crate::{ImportReport, IoError};

const MAX_TOTAL_UNCOMPRESSED: u64 = 128 * 1024 * 1024;

#[derive(Clone)]
struct TopicDraft {
    id: String,
    /// 源文件中的主题 id（用于解析关系线端点）
    src_id: Option<String>,
    parent: Option<String>,
    text: String,
    note: Option<String>,
    /// 超链接（XMind 的 `topic.href` / 旧版 `<link xlink:href>`）。
    link: Option<String>,
    collapsed: bool,
    icon: Option<String>,
    labels: Vec<String>,
}

/// 关系线草稿（端点用源文件 id 表示，构建时再映射）。
#[derive(Clone, Debug)]
struct RelDraft {
    from_src: String,
    to_src: String,
    label: Option<String>,
}

fn build_document(drafts: Vec<TopicDraft>, rels: Vec<RelDraft>) -> Result<Document, IoError> {
    if drafts.is_empty() {
        return Err(IoError::Empty);
    }
    let root_id = drafts[0].id.clone();
    let mut src_map: HashMap<String, String> = HashMap::new();
    for d in &drafts {
        if let Some(s) = &d.src_id {
            src_map.entry(s.clone()).or_insert_with(|| d.id.clone());
        }
    }
    let mut nodes: HashMap<String, Node> = HashMap::new();
    for d in &drafts {
        let mut n = Node::new(d.id.clone(), d.parent.clone(), d.text.clone());
        n.note = d.note.clone();
        n.link = d.link.clone();
        n.collapsed = d.collapsed;
        n.icon = d.icon.clone();
        let mut seen_labels: Vec<String> = Vec::new();
        for l in &d.labels {
            let t = l.trim();
            if t.is_empty() || t.chars().count() > 64 || seen_labels.iter().any(|x| x == t) {
                continue;
            }
            seen_labels.push(t.to_string());
            if seen_labels.len() >= 5 {
                break;
            }
        }
        n.labels = seen_labels;
        nodes.insert(d.id.clone(), n);
    }
    for d in &drafts {
        if let Some(p) = &d.parent {
            if let Some(pn) = nodes.get_mut(p) {
                pn.children.push(d.id.clone());
            }
        }
    }
    // 关系线：仅当两端都能映射到导入的节点时保留
    for r in &rels {
        let (Some(f), Some(t)) = (src_map.get(&r.from_src), src_map.get(&r.to_src)) else {
            continue;
        };
        if f == t {
            continue;
        }
        let Some(node) = nodes.get_mut(f) else {
            continue;
        };
        if node.relationships.iter().any(|x| &x.to == t) {
            continue;
        }
        let label = r
            .label
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.chars().count() <= 200);
        node.relationships.push(Relationship {
            id: new_id(),
            to: t.clone(),
            label,
            color: None,
            // XMind 的关系线不带方向信息，读进来就是默认的前向箭头。
            head: Default::default(),
            ..Default::default()
        });
    }

    let mut meta = DocMeta::default();
    let title: String = drafts[0].text.chars().take(60).collect();
    meta.title = if title.trim().is_empty() {
        "导入内容".to_string()
    } else {
        title
    };
    // 注意：非首个的无父主题（浮动主题）保持为自由主题，不再兜底挂到主根；
    // 首个子主题若无父是根自身（截至首个主题）。
    Document::from_parts(None, meta, root_id, nodes).map_err(|e| IoError::Invalid(e.to_string()))
}

/// XMind 标记 id → 本产品图标 key（尽力映射）。
fn map_marker(id: &str) -> Option<String> {
    if let Some(rest) = id.strip_prefix("priority-") {
        if let Ok(n) = rest.parse::<u32>() {
            let n = n.clamp(1, 6);
            return Some(format!("priority-{n}"));
        }
        return None;
    }
    let mapped = match id {
        "task-done" => "task-done",
        "task-start" | "task-half" => "task-doing",
        "task-oct" | "task-quarter" | "task-todo" => "task-todo",
        s if s.starts_with("star-") => "star",
        "symbol-question" => "question",
        "symbol-exclamation" => "exclaim",
        _ => return None,
    };
    Some(mapped.to_string())
}

// ---------------------------------------------------------------- Zen（content.json）

fn walk_topic_json(t: &Value, parent: Option<String>, out: &mut Vec<TopicDraft>) {
    let id = new_id();
    let text = t
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let note = t
        .pointer("/notes/plain/content")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let collapsed = t.get("branch").and_then(|x| x.as_str()) == Some("folded");
    let icon = t.get("markers").and_then(|m| m.as_array()).and_then(|arr| {
        arr.iter()
            .filter_map(|m| m.get("markerId").and_then(|x| x.as_str()))
            .find_map(map_marker)
    });
    let labels = t
        .get("labels")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let src_id = t.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
    let link = t
        .get("href")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    out.push(TopicDraft {
        id: id.clone(),
        src_id,
        parent: parent.clone(),
        text,
        note,
        link,
        collapsed,
        icon,
        labels,
    });
    if let Some(children) = t.pointer("/children/attached").and_then(|c| c.as_array()) {
        for c in children {
            walk_topic_json(c, Some(id.clone()), out);
        }
    }
}

fn parse_zen(json: &[u8]) -> Result<(Vec<TopicDraft>, Vec<RelDraft>), IoError> {
    let v: Value = serde_json::from_slice(json)
        .map_err(|e| IoError::Invalid(format!("content.json 解析失败：{e}")))?;
    let sheets = v
        .as_array()
        .ok_or_else(|| IoError::Invalid("XMind 内容结构异常（缺少画布列表）".into()))?;
    let mut out = Vec::new();
    let mut rels = Vec::new();
    let mut first_sheet = true;
    for sheet in sheets {
        if let Some(rt) = sheet.get("rootTopic") {
            if first_sheet {
                walk_topic_json(rt, None, &mut out);
                first_sheet = false;
            } else {
                // 多画布：后续画布整体作为自由主题导入（保留内容而不混入主树）
                walk_topic_json(rt, None, &mut out);
            }
            // 浮动主题：现代 XMind 存在 `rootTopic.children.detached`。
            if let Some(detached) = rt.pointer("/children/detached").and_then(|c| c.as_array()) {
                for f in detached {
                    walk_topic_json(f, None, &mut out);
                }
            }
        }
        // 浮动主题（XMind 自由主题，旧形状 `sheet.floatingTopics`）
        if let Some(floats) = sheet.get("floatingTopics").and_then(|x| x.as_array()) {
            for f in floats {
                walk_topic_json(f, None, &mut out);
            }
        }
        if let Some(arr) = sheet.get("relationships").and_then(|x| x.as_array()) {
            for r in arr {
                let from_src = r.get("end1Id").and_then(|x| x.as_str());
                let to_src = r.get("end2Id").and_then(|x| x.as_str());
                if let (Some(f), Some(t)) = (from_src, to_src) {
                    rels.push(RelDraft {
                        from_src: f.to_string(),
                        to_src: t.to_string(),
                        label: r
                            .get("title")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string()),
                    });
                }
            }
        }
    }
    Ok((out, rels))
}

// ---------------------------------------------------------------- XMind 8（content.xml）

enum Sink {
    Title(String),
    Plain(String),
    Marker(String),
    Label(String),
}

fn parse_legacy(xml: &str) -> Result<(Vec<TopicDraft>, Vec<RelDraft>), IoError> {
    use quick_xml::events::Event as XmlEvent;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut drafts: Vec<TopicDraft> = Vec::new();
    let mut rels: Vec<RelDraft> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut sink: Option<Sink> = None;
    let mut buf = String::new();

    fn draft_mut<'a>(drafts: &'a mut [TopicDraft], id: &str) -> Option<&'a mut TopicDraft> {
        drafts.iter_mut().find(|d| d.id == id)
    }

    fn read_rel_attrs(e: &quick_xml::events::BytesStart, rels: &mut Vec<RelDraft>) {
        let mut end1 = None;
        let mut end2 = None;
        let mut title = None;
        for attr in e.attributes().flatten() {
            match attr.key.as_ref() {
                b"end1" => end1 = attr.unescape_value().ok().map(|v| v.to_string()),
                b"end2" => end2 = attr.unescape_value().ok().map(|v| v.to_string()),
                b"title" => title = attr.unescape_value().ok().map(|v| v.to_string()),
                _ => {}
            }
        }
        if let (Some(f), Some(t)) = (end1, end2) {
            rels.push(RelDraft {
                from_src: f,
                to_src: t,
                label: title,
            });
        }
    }

    loop {
        match reader.read_event() {
            Err(e) => return Err(IoError::Xml(e.to_string())),
            Ok(XmlEvent::Eof) => break,
            Ok(XmlEvent::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"topic" => {
                        let id = new_id();
                        let parent = stack.last().cloned();
                        let mut collapsed = false;
                        let mut src_id = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"branch" => {
                                    if let Ok(v) = attr.unescape_value() {
                                        collapsed = v == "folded";
                                    }
                                }
                                b"id" => {
                                    src_id = attr.unescape_value().ok().map(|v| v.to_string());
                                }
                                _ => {}
                            }
                        }
                        drafts.push(TopicDraft {
                            id: id.clone(),
                            src_id,
                            parent,
                            text: String::new(),
                            note: None,
                            link: None,
                            collapsed,
                            icon: None,
                            labels: Vec::new(),
                        });
                        stack.push(id);
                    }
                    b"relationship" => read_rel_attrs(&e, &mut rels),
                    b"marker-ref" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"marker-id" {
                                if let (Ok(v), Some(cur)) =
                                    (attr.unescape_value(), stack.last().cloned())
                                {
                                    if let Some(d) = draft_mut(&mut drafts, &cur) {
                                        if d.icon.is_none() {
                                            d.icon = map_marker(v.as_ref());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    b"title" => {
                        if let Some(cur) = stack.last().cloned() {
                            sink = Some(Sink::Title(cur));
                            buf.clear();
                        }
                    }
                    b"plain" => {
                        if let Some(cur) = stack.last().cloned() {
                            sink = Some(Sink::Plain(cur));
                            buf.clear();
                        }
                    }
                    b"marker-id" => {
                        if let Some(cur) = stack.last().cloned() {
                            sink = Some(Sink::Marker(cur));
                            buf.clear();
                        }
                    }
                    b"label" => {
                        if let Some(cur) = stack.last().cloned() {
                            sink = Some(Sink::Label(cur));
                            buf.clear();
                        }
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Empty(e)) => {
                let name = e.name().as_ref().to_vec();
                if name.as_slice() == b"relationship" {
                    read_rel_attrs(&e, &mut rels);
                } else if name.as_slice() == b"topic" {
                    let id = new_id();
                    let parent = stack.last().cloned();
                    let mut src_id = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"id" {
                            src_id = attr.unescape_value().ok().map(|v| v.to_string());
                        }
                    }
                    drafts.push(TopicDraft {
                        id,
                        src_id,
                        parent,
                        text: String::new(),
                        note: None,
                        link: None,
                        collapsed: false,
                        icon: None,
                        labels: Vec::new(),
                    });
                } else if name.as_slice() == b"marker-ref" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"marker-id" {
                            if let (Ok(v), Some(cur)) =
                                (attr.unescape_value(), stack.last().cloned())
                            {
                                if let Some(d) = draft_mut(&mut drafts, &cur) {
                                    if d.icon.is_none() {
                                        d.icon = map_marker(v.as_ref());
                                    }
                                }
                            }
                        }
                    }
                } else if name.as_slice() == b"link" || name.as_slice() == b"xlink:href" {
                    // XMind 8 的链接：`<link xlink:href="…"/>`（自闭合）。
                    let href = e
                        .attributes()
                        .flatten()
                        .find(|a| {
                            a.key.as_ref() == b"xlink:href" || a.key.as_ref().ends_with(b":href")
                        })
                        .and_then(|a| a.unescape_value().ok())
                        .map(|v| v.trim().to_string())
                        .filter(|s| !s.is_empty());
                    if let (Some(href), Some(cur)) = (href, stack.last().cloned()) {
                        if let Some(d) = draft_mut(&mut drafts, &cur) {
                            if d.link.is_none() {
                                d.link = Some(href);
                            }
                        }
                    }
                }
            }
            Ok(XmlEvent::Text(t)) => {
                if sink.is_some() {
                    if let Ok(s) = t.unescape() {
                        buf.push_str(&s);
                    }
                }
            }
            Ok(XmlEvent::End(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"topic" => {
                        stack.pop();
                    }
                    b"title" | b"plain" | b"marker-id" | b"label" => {
                        if let Some(s) = sink.take() {
                            let text = buf.trim().to_string();
                            buf.clear();
                            match s {
                                Sink::Title(id) => {
                                    if let Some(d) = draft_mut(&mut drafts, &id) {
                                        d.text = text;
                                    }
                                }
                                Sink::Plain(id) => {
                                    if !text.is_empty() {
                                        if let Some(d) = draft_mut(&mut drafts, &id) {
                                            d.note = Some(text);
                                        }
                                    }
                                }
                                Sink::Label(id) => {
                                    if !text.is_empty() {
                                        if let Some(d) = draft_mut(&mut drafts, &id) {
                                            d.labels.push(text);
                                        }
                                    }
                                }
                                Sink::Marker(id) => {
                                    if let Some(d) = draft_mut(&mut drafts, &id) {
                                        if d.icon.is_none() {
                                            d.icon = map_marker(&text);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(_) => {}
        }
    }
    Ok((drafts, rels))
}

// ---------------------------------------------------------------- 入口

/// 导入 `.xmind`（字节流）：自动识别 Zen / XMind 8 两种容器。
pub fn import_xmind(bytes: &[u8]) -> Result<Document, IoError> {
    import_xmind_with_report(bytes).map(|(doc, _)| doc)
}

/// 同 [`import_xmind`]，但带回丢失项报告（当前统计图片/附件资源数量）。
pub fn import_xmind_with_report(bytes: &[u8]) -> Result<(Document, ImportReport), IoError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| IoError::Invalid(format!("不是有效的 XMind 文件：{e}")))?;

    let mut total: u64 = 0;
    let mut resources = 0usize;
    for i in 0..archive.len() {
        let f = archive
            .by_index(i)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        total += f.size();
        if total > MAX_TOTAL_UNCOMPRESSED {
            return Err(IoError::Invalid("XMind 文件解压超过安全上限".into()));
        }
        let name = f.name().map(|n| n.into_owned()).unwrap_or_default();
        if !f.is_dir() && (name.starts_with("resources/") || name.starts_with("attachments/")) {
            resources += 1;
        }
    }

    let mut json_bytes: Option<Vec<u8>> = None;
    let mut xml_bytes: Option<Vec<u8>> = None;
    {
        if let Ok(mut f) = archive.by_name("content.json") {
            let mut b = Vec::new();
            f.read_to_end(&mut b)
                .map_err(|e| IoError::Invalid(e.to_string()))?;
            json_bytes = Some(b);
        }
    }
    {
        if let Ok(mut f) = archive.by_name("content.xml") {
            let mut b = Vec::new();
            f.read_to_end(&mut b)
                .map_err(|e| IoError::Invalid(e.to_string()))?;
            xml_bytes = Some(b);
        }
    }

    let (drafts, rels) = if let Some(j) = json_bytes {
        parse_zen(&j)?
    } else if let Some(x) = xml_bytes {
        parse_legacy(&String::from_utf8_lossy(&x))?
    } else {
        return Err(IoError::Invalid(
            "XMind 文件缺少 content.json / content.xml".into(),
        ));
    };
    let mut doc = build_document(drafts, rels)?;
    // 图片/附件字节进资源表；节点上的 `topic.image.src` 引用尚未映射（见下面的 warn）。
    attach_resources(&mut doc, bytes);

    let mut counts = crate::ImportCounts::default();
    for n in doc.nodes.values() {
        counts.nodes += 1;
        counts.notes += usize::from(n.note.is_some());
        counts.links += usize::from(n.link.is_some());
        counts.labels += n.labels.len();
        counts.relationships += n.relationships.len();
        counts.collapsed += usize::from(n.collapsed);
        counts.styled += usize::from(n.style.is_some());
    }
    let mut report = ImportReport {
        resources,
        counts,
        ..Default::default()
    };
    if resources > 0 {
        report.warn(format!(
            "XMind 内含 {resources} 个图片/附件资源；字节已收进资源包，但主题上的图片引用尚未映射（P53 图片管线接入后补）"
        ));
    }
    Ok((doc, report))
}

/// 导出为 XMind Zen（zip 内 `content.json`）。只写导入器认识的字段：
/// 标题、备注、标签、折叠、子主题；链接/样式/关系线暂不写（XMind 格式映射另开）。
pub fn export_xmind(doc: &Document) -> Result<Vec<u8>, IoError> {
    fn topic(doc: &Document, id: &str) -> Value {
        let n = doc
            .nodes
            .get(id)
            .cloned()
            .unwrap_or_else(|| Node::new(id.to_string(), None, ""));
        let mut t = json!({
            "id": id,
            "class": "topic",
            "title": if n.text.trim().is_empty() { "未命名" } else { n.text.as_str() },
        });
        if let Some(note) = n.note.as_deref().filter(|s| !s.trim().is_empty()) {
            t["notes"] = json!({"plain": {"content": note}});
        }
        if !n.labels.is_empty() {
            t["labels"] = json!(n.labels);
        }
        if n.collapsed && !n.children.is_empty() {
            t["branch"] = json!("folded");
        }
        if !n.children.is_empty() {
            let kids: Vec<Value> = n.children.iter().map(|c| topic(doc, c)).collect();
            t["children"] = json!({"attached": kids});
        }
        t
    }
    let sheet = json!([{
        "id": format!("sheet-{}", doc.id),
        "class": "sheet",
        "title": doc.meta.title,
        "rootTopic": topic(doc, &doc.root_id),
    }]);
    let body = serde_json::to_vec(&sheet).map_err(|e| IoError::Invalid(e.to_string()))?;
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zw.start_file("content.json", opt)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.write_all(&body)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.finish().map_err(|e| IoError::Invalid(e.to_string()))?;
    }
    Ok(buf.into_inner())
}

// ---------------------------------------------------------------------------
// 开放格式 `.xmind`：XMind 互通视图 + 织思保真槽（docs/34 §3）
// ---------------------------------------------------------------------------

/// 内置图标 key → XMind markerId（互通用，尽力映射；精确 icon 在保真槽）。
fn icon_marker(icon: &str) -> Option<String> {
    let s = match icon {
        "task-todo" => "task-todo",
        "task-doing" => "task-start",
        "task-done" => "task-done",
        "star" => "star-1",
        "question" => "symbol-question",
        "exclaim" => "symbol-exclamation",
        other if other.starts_with("priority-") => other,
        _ => return None,
    };
    Some(s.to_string())
}

/// XMind 互通视图：`Document` → `content.json`（数组，一张 sheet）。
/// 写结构/标题/备注/标签/图标/折叠/子树/浮动主题/关系线；样式/箭头等进保真槽。
fn interop_content_json(doc: &Document) -> Value {
    // 关系线：全节点收集 → sheet.relationships。
    let mut rels = Vec::new();
    for n in doc.nodes.values() {
        for r in &n.relationships {
            rels.push(json!({
                "id": r.id,
                "end1Id": n.id,
                "end2Id": r.to,
                "title": r.label.clone().unwrap_or_default(),
            }));
        }
    }

    fn topic(doc: &Document, id: &str) -> Value {
        let Some(n) = doc.nodes.get(id) else {
            return json!({ "id": id, "class": "topic", "title": "未命名" });
        };
        let mut t = json!({
            "id": id,
            "class": "topic",
            "title": if n.text.trim().is_empty() { "未命名" } else { n.text.as_str() },
        });
        if let Some(note) = n.note.as_deref().filter(|s| !s.trim().is_empty()) {
            t["notes"] = json!({"plain": {"content": note}});
        }
        if !n.labels.is_empty() {
            t["labels"] = json!(n.labels);
        }
        if let Some(m) = n.icon.as_deref().and_then(icon_marker) {
            t["markers"] = json!([{ "markerId": m }]);
        }
        if let Some(link) = n.link.as_deref().filter(|s| !s.trim().is_empty()) {
            t["href"] = json!(link);
        }
        if n.collapsed && !n.children.is_empty() {
            t["branch"] = json!("folded");
        }
        if !n.children.is_empty() {
            let kids: Vec<Value> = n.children.iter().map(|c| topic(doc, c)).collect();
            t["children"] = json!({"attached": kids});
        }
        t
    }

    let mut root_topic = topic(doc, &doc.root_id);

    // 浮动主题（自由主题）→ rootTopic.children.detached + position。
    let mut detached = Vec::new();
    for (id, n) in &doc.nodes {
        if id != &doc.root_id && n.parent_id.is_none() {
            let mut t = topic(doc, id);
            if let Some(p) = &n.pos {
                t["position"] = json!({ "x": p.x, "y": p.y });
            }
            detached.push(t);
        }
    }
    if !detached.is_empty() {
        let attached = root_topic
            .get("children")
            .and_then(|c| c.get("attached"))
            .cloned()
            .unwrap_or_else(|| json!([]));
        root_topic["children"] = json!({ "attached": attached, "detached": detached });
    }

    json!([{
        "id": format!("sheet-{}", doc.id),
        "class": "sheet",
        "title": doc.meta.title,
        "rootTopic": root_topic,
        "relationships": rels,
    }])
}

/// 保存为开放格式 `.xmind`（XMind Zen ZIP）：
/// `content.json`（互通，XMind 读）+ `mindweaver.json`（保真槽，织思读）+
/// `metadata.json` + `manifest.json`。XMind 忽略 `mindweaver.json` 照常打开。
pub fn save_xmind(doc: &Document) -> Result<Vec<u8>, IoError> {
    let content = interop_content_json(doc);
    let content_bytes =
        serde_json::to_vec(&content).map_err(|e| IoError::Invalid(e.to_string()))?;
    let fidelity =
        mw_format::doc_to_fidelity_json(doc, None).map_err(|e| IoError::Invalid(e.to_string()))?;
    let metadata = json!({
        "creator": { "name": "MindWeaver", "version": "0.1.0" },
        "timestamp": mw_core::now_ms(),
    });
    let mut entries = serde_json::Map::new();
    for name in ["content.json", "metadata.json", "mindweaver.json"] {
        entries.insert(name.to_string(), json!({}));
    }
    for (hash, _) in doc.assets.iter() {
        entries.insert(format!("resources/{hash}"), json!({}));
    }
    let manifest = json!({ "file-entries": Value::Object(entries) });

    let mut buf = Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zw.start_file("content.json", opt)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.write_all(&content_bytes)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        let meta_bytes =
            serde_json::to_vec(&metadata).map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.start_file("metadata.json", opt)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.write_all(&meta_bytes)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        let man_bytes =
            serde_json::to_vec(&manifest).map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.start_file("manifest.json", opt)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.write_all(&man_bytes)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.start_file("mindweaver.json", opt)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        zw.write_all(&fidelity)
            .map_err(|e| IoError::Invalid(e.to_string()))?;
        // 图片/附件字节：节点里存的是 `asset:<hash>` 引用，字节必须一起进 ZIP，
        // 否则另存成 `.xmind` 再打开只剩悬空引用（`docs/34` §3.1 的 `resources/` 条目）。
        for (hash, bytes) in doc.assets.iter() {
            zw.start_file(format!("resources/{hash}"), opt)
                .map_err(|e| IoError::Invalid(e.to_string()))?;
            zw.write_all(bytes)
                .map_err(|e| IoError::Invalid(e.to_string()))?;
        }
        zw.finish().map_err(|e| IoError::Invalid(e.to_string()))?;
    }
    Ok(buf.into_inner())
}

/// 从 `.xmind` 读回。**保真槽优先**（`mindweaver.json`，无损）；
/// 缺失（被外部工具改存）→ 降级走 `import_xmind` 解析互通视图。
pub fn load_xmind(bytes: &[u8]) -> Result<Document, IoError> {
    let mut doc = load_xmind_doc(bytes)?;
    attach_resources(&mut doc, bytes);
    Ok(doc)
}

/// 读 `resources/<hash>` 进资源表。缺条目、超限、读失败都跳过（渲染层本来就降级成占位）。
fn attach_resources(doc: &mut Document, bytes: &[u8]) {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return;
    };
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let Ok(mut f) = archive.by_index(i) else {
            continue;
        };
        let Ok(name) = f.name() else {
            continue;
        };
        let name = name.to_string();
        let Some(hash) = name.strip_prefix("resources/") else {
            continue;
        };
        if hash.is_empty() || f.size() > MAX_TOTAL_UNCOMPRESSED {
            continue;
        }
        total += f.size();
        if total > MAX_TOTAL_UNCOMPRESSED {
            return;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_ok() {
            doc.assets.put(hash.to_string(), buf);
        }
    }
}

fn load_xmind_doc(bytes: &[u8]) -> Result<Document, IoError> {
    if let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) {
        if let Ok(mut f) = archive.by_name("mindweaver.json") {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)
                .map_err(|e| IoError::Invalid(e.to_string()))?;
            let loaded = mw_format::doc_from_fidelity_json(&buf)
                .map_err(|e| IoError::Invalid(e.to_string()))?;
            return Ok(loaded.doc);
        }
    }
    import_xmind(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();
            for (name, data) in entries {
                zw.start_file(*name, opts).unwrap();
                zw.write_all(data.as_bytes()).unwrap();
            }
            zw.finish().unwrap();
        }
        buf.into_inner()
    }

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

    /// 链接回读：互通视图写 `topic.href`，降级解析必须读回来（否则用户导进 XMind
    /// 再另存回来，链接全没了）。旧版 `content.xml` 的 `<link xlink:href>` 同理。
    #[test]
    fn topic_links_are_read_back_from_both_xmind_generations() {
        let zen = r#"[{
          "id": "sheet1", "class": "sheet", "title": "画布",
          "rootTopic": {
            "id": "t0", "class": "topic", "title": "根",
            "children": { "attached": [
              { "id": "t1", "class": "topic", "title": "子",
                "href": "https://example.com/a" }
            ]}
          }
        }]"#;
        let doc = import_xmind(&zip_bytes(&[("content.json", zen)])).unwrap();
        let child = doc.nodes.values().find(|n| n.text == "子").unwrap();
        assert_eq!(child.link.as_deref(), Some("https://example.com/a"));

        let legacy = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmap-content xmlns="urn:xmind:xmap:xmlns:content:2.0"
              xmlns:xlink="http://www.w3.org/1999/xlink">
  <sheet id="s1">
    <topic id="t0"><title>旧根</title>
      <children><topics type="attached">
        <topic id="t1"><title>旧子</title>
          <link xlink:href="https://example.com/b"/></topic>
      </topics></children>
    </topic>
  </sheet>
</xmap-content>"#;
        let doc = import_xmind(&zip_bytes(&[("content.xml", legacy)])).unwrap();
        let child = doc.nodes.values().find(|n| n.text == "旧子").unwrap();
        assert_eq!(child.link.as_deref(), Some("https://example.com/b"));
    }

    #[test]
    fn import_zen_json() {
        let json = r#"[
          {
            "id": "sheet1",
            "class": "sheet",
            "title": "画布 1",
            "rootTopic": {
              "id": "t0",
              "class": "topic",
              "title": "中心主题",
              "children": {
                "attached": [
                  {
                    "id": "t1",
                    "title": "分支甲",
                    "notes": { "plain": { "content": "这是备注" } },
                    "markers": [{ "markerId": "priority-2" }]
                  },
                  {
                    "id": "t2",
                    "title": "分支乙",
                    "branch": "folded",
                    "children": { "attached": [{ "id": "t3", "title": "孙子节点" }] }
                  }
                ]
              }
            }
          }
        ]"#;
        let bytes = zip_bytes(&[("content.json", json)]);
        let doc = import_xmind(&bytes).unwrap();
        assert_eq!(doc.meta.title, "中心主题");
        assert_eq!(
            flatten(&doc),
            vec![
                (0, "中心主题".into()),
                (1, "分支甲".into()),
                (1, "分支乙".into()),
                (2, "孙子节点".into()),
            ]
        );
        let a = doc.nodes.values().find(|n| n.text == "分支甲").unwrap();
        assert_eq!(a.note.as_deref(), Some("这是备注"));
        assert_eq!(a.icon.as_deref(), Some("priority-2"));
        let b = doc.nodes.values().find(|n| n.text == "分支乙").unwrap();
        assert!(b.collapsed);
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn import_report_counts_embedded_resources() {
        let json = r#"[{"id":"s","class":"sheet","title":"T",
            "rootTopic":{"id":"t0","class":"topic","title":"根"}}]"#;
        let bytes = zip_bytes(&[
            ("content.json", json),
            ("resources/image-1.png", "not-really-a-png"),
            ("attachments/a.pdf", "pdf"),
        ]);
        let (doc, report) = import_xmind_with_report(&bytes).unwrap();
        assert_eq!(doc.meta.title, "根");
        assert_eq!(report.resources, 2);
        assert_eq!(report.counts.nodes, 1);
        assert_eq!(report.warnings.len(), 1);
        assert!(
            report.warnings[0].contains("2 个图片/附件资源"),
            "{report:?}"
        );
        assert!(!report.is_empty());
    }

    #[test]
    fn import_legacy_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmap-content xmlns="urn:xmind:xmap:xmlns:content:2.0">
  <sheet id="s1">
    <topic id="t0">
      <title>旧版中心</title>
      <children>
        <topics type="attached">
          <topic id="t1">
            <title>子一</title>
            <notes><plain>旧版备注</plain></notes>
            <marker-refs><marker-ref marker-id="task-done"/></marker-refs>
          </topic>
          <topic id="t2" branch="folded"><title>子二</title></topic>
        </topics>
      </children>
    </topic>
  </sheet>
</xmap-content>"#;
        let bytes = zip_bytes(&[("content.xml", xml)]);
        let doc = import_xmind(&bytes).unwrap();
        assert_eq!(
            flatten(&doc),
            vec![
                (0, "旧版中心".into()),
                (1, "子一".into()),
                (1, "子二".into()),
            ]
        );
        let c1 = doc.nodes.values().find(|n| n.text == "子一").unwrap();
        assert_eq!(c1.note.as_deref(), Some("旧版备注"));
        assert_eq!(c1.icon.as_deref(), Some("task-done"));
        let c2 = doc.nodes.values().find(|n| n.text == "子二").unwrap();
        assert!(c2.collapsed);
    }

    #[test]
    fn import_zen_relationships_labels_floating() {
        let json = r#"[
          {
            "id": "sheet1",
            "class": "sheet",
            "rootTopic": {
              "id": "t0",
              "title": "中心",
              "children": { "attached": [
                { "id": "t1", "title": "甲", "labels": ["重要", "Q3"] },
                { "id": "t2", "title": "乙" }
              ] }
            },
            "relationships": [
              { "id": "r1", "end1Id": "t1", "end2Id": "t2", "title": "依赖" },
              { "id": "r2", "end1Id": "t9", "end2Id": "t2" }
            ],
            "floatingTopics": [
              { "id": "f1", "title": "浮动便签" }
            ]
          }
        ]"#;
        let bytes = zip_bytes(&[("content.json", json)]);
        let doc = import_xmind(&bytes).unwrap();
        assert!(doc.validate().is_ok());
        let a = doc.nodes.values().find(|n| n.text == "甲").unwrap();
        assert_eq!(a.labels, vec!["重要".to_string(), "Q3".to_string()]);
        assert_eq!(a.relationships.len(), 1);
        let b = doc.nodes.values().find(|n| n.text == "乙").unwrap();
        assert_eq!(a.relationships[0].to, b.id);
        assert_eq!(a.relationships[0].label.as_deref(), Some("依赖"));
        let f = doc.nodes.values().find(|n| n.text == "浮动便签").unwrap();
        assert_eq!(f.parent_id, None);
        assert_ne!(f.id, doc.root_id);
        assert_eq!(doc.node_count(), 4);
    }

    #[test]
    fn import_legacy_labels_and_relationship() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmap-content xmlns="urn:xmind:xmap:xmlns:content:2.0">
  <sheet id="s1">
    <topic id="t0">
      <title>旧版中心</title>
      <children>
        <topics type="attached">
          <topic id="t1"><title>子一</title><labels><label>标签A</label></labels></topic>
          <topic id="t2"><title>子二</title></topic>
        </topics>
      </children>
    </topic>
    <relationships>
      <relationship id="rel1" end1="t1" end2="t2" title="相关"/>
    </relationships>
  </sheet>
</xmap-content>"#;
        let bytes = zip_bytes(&[("content.xml", xml)]);
        let doc = import_xmind(&bytes).unwrap();
        assert!(doc.validate().is_ok());
        let c1 = doc.nodes.values().find(|n| n.text == "子一").unwrap();
        assert_eq!(c1.labels, vec!["标签A".to_string()]);
        assert_eq!(c1.relationships.len(), 1);
        assert_eq!(c1.relationships[0].label.as_deref(), Some("相关"));
    }

    #[test]
    fn reject_zip_without_content() {
        let bytes = zip_bytes(&[("meta.xml", "<x/>")]);
        assert!(matches!(import_xmind(&bytes), Err(IoError::Invalid(_))));
    }

    /// `save_xmind` → `load_xmind` 走保真槽，必须无损（比互通视图多出的字段：
    /// 链接、样式、边界、概要、卡片、分组…）。互通视图那条路由
    /// `import_xmind` / `export_xmind` 的单测覆盖，这里不重复。
    #[test]
    fn save_then_load_keeps_every_field_through_the_fidelity_slot() {
        let mut doc = Document::new("保真往返");
        let root = doc.root_id.clone();
        let before: std::collections::HashSet<String> = doc.doc_order().into_iter().collect();
        doc.apply(&mw_core::Command::AddNode {
            parent: root.clone(),
            index: None,
            text: "子节点".into(),
            style: None,
        })
        .unwrap();
        let child = doc
            .doc_order()
            .into_iter()
            .find(|id| !before.contains(id))
            .unwrap();
        let n = doc.nodes.get_mut(&child).unwrap();
        n.note = Some("备注".into());
        n.link = Some("https://example.com".into());
        n.labels = vec!["标签A".into()];
        n.collapsed = true;
        n.icon = Some("idea".into());

        let bytes = save_xmind(&doc).unwrap();
        let back = load_xmind(&bytes).unwrap();
        assert!(back.validate().is_ok(), "读回的结构要合法");
        assert_eq!(back.meta.title, doc.meta.title);
        assert_eq!(back.meta.theme, doc.meta.theme);
        let b = &back.nodes[&child];
        assert_eq!(b.text, "子节点");
        assert_eq!(b.note.as_deref(), Some("备注"));
        assert_eq!(b.link.as_deref(), Some("https://example.com"));
        assert_eq!(b.labels, vec!["标签A".to_string()]);
        assert!(b.collapsed);
        assert_eq!(b.icon.as_deref(), Some("idea"));
    }

    #[test]
    fn reject_non_zip() {
        assert!(matches!(
            import_xmind(b"not a zip"),
            Err(IoError::Invalid(_))
        ));
    }

    /// 图片/附件字节必须随 `.xmind` 往返（`docs/34` §3.1 的 `resources/`）。
    /// 以前 `save_xmind` 只写 JSON，节点里的 `asset:<hash>` 另存再打开就是悬空引用。
    #[test]
    fn embedded_resources_round_trip_through_save_and_load() {
        let mut doc = Document::new("图片往返");
        let key = doc.assets.insert(vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
        let root = doc.root_id.clone();
        doc.nodes.get_mut(&root).unwrap().image = Some(format!("asset:{key}"));

        let bytes = save_xmind(&doc).unwrap();
        let back = load_xmind(&bytes).unwrap();
        assert_eq!(
            back.assets.get(&key),
            Some(&[0x89u8, b'P', b'N', b'G', 1, 2, 3][..]),
            "资源字节必须读回来"
        );
        assert_eq!(
            back.nodes[&root].image.as_deref(),
            Some(format!("asset:{key}").as_str()),
            "节点的资源引用保持不变"
        );
        // 清单里要列出资源条目（XMind 读 manifest 决定取哪些文件）。
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let mut man = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("manifest.json").unwrap(), &mut man)
            .unwrap();
        assert!(
            man.contains(&format!("resources/{key}")),
            "manifest 要登记资源：{man}"
        );
    }
}
