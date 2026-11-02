//! 学习合集：导图大纲 + 卡片正文，一次导出 Markdown 与可打印 HTML。
//!
//! 卡片顺序 = 文档前序（节点挂载的卡片），再追加收件箱/孤儿卡片（按标题）。
//! 打印分页以**整张卡片**为最小单位：卡片再长也不从中间切开；分页函数是纯函数，
//! 便于断言「不断行、顺序不乱」。

use mw_core::{Document, NoteCard};

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub source: String,
    pub body: String,
}

#[derive(Clone, Debug, Default)]
pub struct Bundle {
    /// (depth, text, note)
    pub outline: Vec<(u32, String, String)>,
    pub cards: Vec<Entry>,
    /// 整张导图的 SVG（导出时注入；纯文本/测试可为空）。
    pub map_svg: Option<String>,
}

pub fn collect(doc: &Document, extra: &[NoteCard]) -> Bundle {
    // 概念图里挂卡片的不是「主题」而是「概念」；导出的来源列不能写错词。
    let node_word = if doc.is_concept_map() {
        "概念"
    } else {
        "主题"
    };
    let order = doc.doc_order();
    let depth: std::collections::HashMap<&str, u32> = order
        .iter()
        .map(|id| {
            let mut d = 0u32;
            let mut cur = doc.node(id).and_then(|n| n.parent_id.clone());
            while let Some(p) = cur {
                d += 1;
                cur = doc.node(&p).and_then(|n| n.parent_id.clone());
                if d > 512 {
                    break;
                }
            }
            (id.as_str(), d)
        })
        .collect();
    let outline = order
        .iter()
        .filter_map(|id| {
            let n = doc.node(id)?;
            let text = if n.text.trim().is_empty() {
                "未命名主题".to_string()
            } else {
                n.text.clone()
            };
            Some((
                depth.get(id.as_str()).copied().unwrap_or(0),
                text,
                n.note.clone().unwrap_or_default(),
            ))
        })
        .collect();

    let mut cards: Vec<Entry> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for id in &order {
        let Some(n) = doc.node(id) else { continue };
        let Some(cid) = n.card_id.as_ref() else {
            continue;
        };
        let card = doc
            .card(cid)
            .cloned()
            .or_else(|| extra.iter().find(|c| &c.id == cid).cloned());
        if let Some(card) = card {
            if seen.insert(card.id.clone()) {
                cards.push(entry(&card, &format!("{node_word} · {}", text_of(n))));
            }
        }
    }
    let mut rest: Vec<&NoteCard> = extra.iter().filter(|c| !seen.contains(&c.id)).collect();
    rest.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.cmp(&b.id)));
    for card in rest {
        let source = if card.source_doc.is_empty() {
            "收件箱".to_string()
        } else {
            "卡片库".to_string()
        };
        cards.push(entry(card, &source));
    }
    Bundle {
        outline,
        cards,
        map_svg: None,
    }
}

fn text_of(n: &mw_core::Node) -> String {
    if n.text.trim().is_empty() {
        "未命名主题".into()
    } else {
        n.text.clone()
    }
}

fn entry(card: &NoteCard, source: &str) -> Entry {
    Entry {
        id: card.id.clone(),
        title: card.display_title().to_string(),
        tags: card.tags.clone(),
        source: source.to_string(),
        body: card.body.clone(),
    }
}

impl Bundle {
    /// 带上整张导图 SVG（打印 HTML 第一页）。
    pub fn with_map(mut self, svg: String) -> Self {
        self.map_svg = Some(svg);
        self
    }
    pub fn markdown(&self) -> String {
        let mut s = String::from("# 织思学习合集\n\n## 导图\n\n");
        for (d, text, note) in &self.outline {
            s.push_str(&"  ".repeat(*d as usize));
            s.push_str("- ");
            s.push_str(text);
            if !note.trim().is_empty() {
                s.push_str("  \n");
                s.push_str(&"  ".repeat(*d as usize + 1));
                s.push_str(note.trim());
            }
            s.push('\n');
        }
        s.push_str("\n## 卡片\n\n");
        for c in &self.cards {
            s.push_str("### ");
            s.push_str(&c.title);
            s.push_str("\n\n");
            if !c.tags.is_empty() {
                s.push_str(
                    &c.tags
                        .iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                s.push_str("\n\n");
            }
            s.push_str("> ");
            s.push_str(&c.source);
            s.push_str("\n\n");
            s.push_str(c.body.trim());
            s.push_str("\n\n---\n\n");
        }
        s
    }

    /// 可打印 HTML：整张卡片 `page-break-inside: avoid`，不会被分页切开。
    pub fn html(&self) -> String {
        let mut s = String::from(
            "<!doctype html><meta charset=\"utf-8\"><title>织思学习合集</title>\
             <style>body{font-family:system-ui,\"Microsoft YaHei\",sans-serif;max-width:760px;margin:32px auto;color:#222;line-height:1.6}\
             h1{font-size:24px}h2{margin-top:28px;border-bottom:1px solid #ddd;padding-bottom:4px}\
             .card{margin:18px 0;padding:14px 16px;border:1px solid #e5e5e5;border-radius:8px;page-break-inside:avoid;break-inside:avoid}\
             .map{margin:14px 0;page-break-after:always;break-after:page}.map svg{max-width:100%;height:auto}\
             .meta{color:#888;font-size:12px;margin-bottom:8px}pre{white-space:pre-wrap;font-family:inherit;margin:0}\
             @media print{.card{box-shadow:none}body{margin:0}}</style><h1>织思学习合集</h1>",
        );
        s.push_str("<h2>导图</h2>");
        if let Some(svg) = &self.map_svg {
            let body = svg
                .split_once("<svg")
                .map(|(_, rest)| format!("<svg{rest}"))
                .unwrap_or_else(|| svg.clone());
            s.push_str("<div class=\"map\">");
            s.push_str(&body);
            s.push_str("</div>");
        } else {
            s.push_str("<pre>");
            for (d, text, _) in &self.outline {
                s.push_str(&"  ".repeat(*d as usize));
                s.push_str("- ");
                s.push_str(&escape(text));
                s.push('\n');
            }
            s.push_str("</pre>");
        }
        s.push_str("<h2>卡片</h2>");
        for c in &self.cards {
            s.push_str("<div class=\"card\"><h3>");
            s.push_str(&escape(&c.title));
            s.push_str("</h3><div class=\"meta\">");
            s.push_str(&escape(&c.source));
            if !c.tags.is_empty() {
                s.push_str(" · ");
                s.push_str(&escape(
                    &c.tags
                        .iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                ));
            }
            s.push_str("</div><pre>");
            s.push_str(&escape(c.body.trim()));
            s.push_str("</pre></div>");
        }
        s
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 打印分页：每页最多 `lines_per_page` 行；一张卡片作为一个整体，超出就换页，
/// 绝不从卡片中间切开。空卡片按 3 行（标题+来源+空行）估算。
/// 当前 HTML 交给浏览器按 `page-break-inside: avoid` 分页；这个函数供测试与
/// 将来的固定分页打印使用。
#[cfg_attr(not(test), allow(dead_code))]
pub fn paginate(cards: &[Entry], lines_per_page: usize) -> Vec<Vec<&Entry>> {
    let max = lines_per_page.max(1);
    let mut pages: Vec<Vec<&Entry>> = Vec::new();
    let mut cur: Vec<&Entry> = Vec::new();
    let mut used = 0usize;
    for c in cards {
        let lines = 3 + c.body.lines().count() + c.tags.len() / 4;
        if !cur.is_empty() && used + lines > max {
            pages.push(std::mem::take(&mut cur));
            used = 0;
        }
        cur.push(c);
        used += lines;
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::Command;

    fn doc_with_cards() -> (Document, Vec<NoteCard>) {
        let mut d = Document::new("学习");
        let root = d.root_id.clone();
        let mut ids = Vec::new();
        for t in ["甲", "乙"] {
            let id = format!("n-{t}");
            d.apply(&Command::AddNode {
                parent: root.clone(),
                index: None,
                text: t.into(),
                style: None,
            })
            .unwrap();
            ids.push(id);
        }
        let order = d.doc_order();
        let a = NoteCard::new(d.id.clone(), Some(order[1].clone()), "卡片甲", "正文甲", 1);
        let b = NoteCard::new(d.id.clone(), Some(order[2].clone()), "卡片乙", "正文乙", 2);
        d.apply(&Command::AttachNewCard {
            node: order[1].clone(),
            card: a.clone(),
        })
        .unwrap();
        d.apply(&Command::AttachNewCard {
            node: order[2].clone(),
            card: b.clone(),
        })
        .unwrap();
        (d, vec![a, b])
    }

    #[test]
    fn cards_follow_document_order_then_inbox() {
        let (d, mut cards) = doc_with_cards();
        let mut inbox = NoteCard::new(d.id.clone(), None, "收件箱卡", "x", 3);
        inbox.source_doc.clear();
        cards.push(inbox);
        let bundle = collect(&d, &cards);
        let titles: Vec<&str> = bundle.cards.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["卡片甲", "卡片乙", "收件箱卡"]);
        assert!(bundle.markdown().contains("## 导图"));
    }

    #[test]
    fn pagination_never_splits_a_card() {
        let cards: Vec<Entry> = (0..5)
            .map(|i| Entry {
                id: i.to_string(),
                title: format!("卡{i}"),
                tags: vec![],
                source: "测试".into(),
                body: "行\n".repeat(10),
            })
            .collect();
        let pages = paginate(&cards, 25);
        assert_eq!(pages.iter().map(|p| p.len()).sum::<usize>(), 5);
        assert!(
            pages.iter().all(|p| p.len() <= 1),
            "每张卡 13 行，25 行只能放一张"
        );
        assert_eq!(pages.len(), 5);
    }

    #[test]
    fn map_svg_is_embedded_on_its_own_page() {
        let bundle = Bundle {
            outline: Vec::new(),
            cards: Vec::new(),
            map_svg: Some(
                "<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"></svg>".into(),
            ),
        };
        let html = bundle.html();
        assert!(html.contains("<div class=\"map\">"));
        assert!(html.contains("<svg xmlns="));
        assert!(!html.contains("<?xml"));
        assert!(html.contains("page-break-after:always"));
    }

    #[test]
    fn html_escapes_body_and_marks_page_breaks() {
        let bundle = Bundle {
            outline: vec![(0, "<根>".into(), String::new())],
            cards: vec![Entry {
                id: "1".into(),
                title: "<b>".into(),
                tags: vec![],
                source: "s".into(),
                body: "a<b".into(),
            }],
            map_svg: None,
        };
        let html = bundle.html();
        assert!(html.contains("&lt;b&gt;"));
        assert!(html.contains("page-break-inside"));
        assert!(!html.contains("<b>"));
    }
}
