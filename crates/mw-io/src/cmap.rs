//! 概念图 ↔ CmapTools CXL（`.cxl`）。
//!
//! CmapTools 能直接打开/导入 CXL（`cmap.xsd`）。织思的概念图模型（命题的端点箭头/
//! 线色/虚线/注释、pinned、概念分组、逐概念样式）比 CXL 宽，故采用**互通视图 + 保真槽**
//! （`docs/34` §2）：
//!
//! - **互通视图**：概念 → `<concept>`、命题 → 概念—`<linking-phrase>`—概念三元组、
//!   坐标/外观 → `<*-appearance>`、聚焦问题 → `res-meta/dc:description`。CmapTools 读这些。
//! - **保真槽**：完整 `FileDoc` 塞进 `<extra-properties-list>` 的 `mindweaver` 属性，
//!   CmapTools 忽略、织思优先读。外部工具改完另存丢掉保真槽 → 降级解析互通视图。

use std::collections::HashMap;

use mw_core::model::ConceptGroup;
use mw_core::{Document, Node};

use crate::IoError;

/// CXL 的命名空间（`cmap.xsd`）。
const NS_CMAP: &str = "http://cmap.ihmc.us/xml/cmap";
const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
const NS_VCARD: &str = "http://www.w3.org/2001/vcard-rdf/3.0#";

// ---------------------------------------------------------------------------
// 写：Document → CXL
// ---------------------------------------------------------------------------

/// 把概念图文档写成 CmapTools CXL XML（含保真槽）。只对概念图有意义。
pub fn save_cxl(doc: &Document) -> Result<String, IoError> {
    // 概念分组（`groups`）**不进互通视图**：CXL 没有「分组」这种元素，硬写就是发明格式。
    // 它只随保真槽（整份 `FileDoc` JSON 的 `groups` 键）走，由 [`load_cxl`] 的保真分支还原。
    // 这里显式传空表，免得留下「传进 block 却没人读 `block.groups`」的死代码（CM-23 §7 第 4 条）。
    let block = mw_format::concept_map_block(
        &doc.nodes,
        doc.meta.layout.focus_question.clone(),
        Vec::new(),
    );
    // 保真槽要先算：`dcterms:created` 取它的 `createdAt`——`Document` 模型里没有单独的创建时间
    // 字段，保真槽是唯一记得住它的地方（CM-23 §7 第 2 条）。
    let fidelity = mw_format::doc_to_fidelity_json(doc, None)
        .map_err(|e| IoError::Invalid(format!("保真槽序列化失败：{e}")))?;
    let fidelity_str = String::from_utf8(fidelity)
        .map_err(|e| IoError::Invalid(format!("保真槽非 UTF-8：{e}")))?;
    let created_at = serde_json::from_str::<serde_json::Value>(&fidelity_str)
        .ok()
        .and_then(|v| v.get("createdAt").and_then(|x| x.as_i64()))
        .unwrap_or_else(mw_core::now_ms);
    let title = xml_escape(&doc.meta.title);
    let focus = doc
        .meta
        .layout
        .focus_question
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let created = iso8601_utc(created_at);
    let modified = iso8601_utc(mw_core::now_ms());

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<cmap xmlns=\"{NS_CMAP}\" xmlns:dc=\"{NS_DC}\" xmlns:dcterms=\"{NS_DCTERMS}\" xmlns:vcard=\"{NS_VCARD}\">\n"
    ));
    // res-meta：标题 + 聚焦问题（dc:description）。
    out.push_str("  <res-meta>\n");
    out.push_str(&format!("    <dc:title>{title}</dc:title>\n"));
    if !focus.is_empty() {
        out.push_str(&format!("    <dc:description>{focus}</dc:description>\n"));
    }
    out.push_str("    <dc:format>x-cmap/text-xml</dc:format>\n");
    out.push_str(&format!(
        "    <dcterms:created>{created}</dcterms:created>\n"
    ));
    out.push_str(&format!(
        "    <dcterms:modified>{modified}</dcterms:modified>\n"
    ));
    out.push_str("  </res-meta>\n");
    out.push_str("  <map>\n");

    // concept-list + concept-appearance-list。
    out.push_str("    <concept-list>\n");
    for c in &block.concepts {
        out.push_str(&format!(
            "      <concept id=\"{}\" label=\"{}\"/>\n",
            xml_escape(&c.id),
            xml_escape(&c.text)
        ));
    }
    out.push_str("    </concept-list>\n");

    // 命题 → linking-phrase + 两条 connection。
    struct Triplet {
        phrase_id: String,
        label: String,
        from: String,
        to: String,
        head: mw_core::model::ArrowHead,
    }
    let mut tris = Vec::new();
    for p in &block.propositions {
        let label = p
            .label
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "related to".to_string());
        tris.push(Triplet {
            phrase_id: format!("lp-{}", p.id),
            label,
            from: p.from.clone(),
            to: p.to.clone(),
            head: p.head,
        });
    }
    out.push_str("    <linking-phrase-list>\n");
    for t in &tris {
        out.push_str(&format!(
            "      <linking-phrase id=\"{}\" label=\"{}\"/>\n",
            xml_escape(&t.phrase_id),
            xml_escape(&t.label)
        ));
    }
    out.push_str("    </linking-phrase-list>\n");

    out.push_str("    <connection-list>\n");
    for t in &tris {
        // 概念 → 短语 → 概念。
        out.push_str(&format!(
            "      <connection id=\"{}\" from-id=\"{}\" to-id=\"{}\"/>\n",
            xml_escape(&format!("c-{}-a", t.phrase_id)),
            xml_escape(&t.from),
            xml_escape(&t.phrase_id)
        ));
        out.push_str(&format!(
            "      <connection id=\"{}\" from-id=\"{}\" to-id=\"{}\"/>\n",
            xml_escape(&format!("c-{}-b", t.phrase_id)),
            xml_escape(&t.phrase_id),
            xml_escape(&t.to)
        ));
    }
    out.push_str("    </connection-list>\n");

    // concept-appearance（坐标 + 字号 + 填充/边框）。
    out.push_str("    <concept-appearance-list>\n");
    for (i, c) in block.concepts.iter().enumerate() {
        let (x, y) = concept_xy(c, i);
        let mut attrs = format!(
            "id=\"{}\" x=\"{}\" y=\"{}\"",
            xml_escape(&c.id),
            x as i32,
            y as i32
        );
        if let Some(style) = &c.style {
            if let Some(size) = style.font_size {
                attrs.push_str(&format!(" font-size=\"{size}\""));
            }
            if let Some(fill) = style.fill.as_deref().and_then(hex_to_rgba) {
                attrs.push_str(&format!(" background-color=\"{fill}\""));
            }
            if let Some(stroke) = style.stroke.as_deref().and_then(hex_to_rgba) {
                attrs.push_str(&format!(" border-color=\"{stroke}\""));
            }
        }
        out.push_str(&format!("      <concept-appearance {attrs}/>\n"));
    }
    out.push_str("    </concept-appearance-list>\n");

    // linking-phrase-appearance（放在两概念中点附近；CmapTools 会自动重排）。
    out.push_str("    <linking-phrase-appearance-list>\n");
    for (i, t) in tris.iter().enumerate() {
        let (fx, fy) = pos_of(&block.concepts, &t.from, i);
        let (tx, ty) = pos_of(&block.concepts, &t.to, i + 1);
        out.push_str(&format!(
            "      <linking-phrase-appearance id=\"{}\" x=\"{}\" y=\"{}\"/>\n",
            xml_escape(&t.phrase_id),
            ((fx + tx) / 2.0) as i32,
            ((fy + ty) / 2.0) as i32
        ));
    }
    out.push_str("    </linking-phrase-appearance-list>\n");

    // connection-appearance（箭头方向，CM-23 §7 第 3 条）。
    //
    // 一条命题在 CXL 里是「概念 A →(connection a)→ 短语 →(connection b)→ 概念 B」两条边。
    // 方向不该靠一个含糊的枚举值硬塞，而应该**把箭头写在它实际所在的那条边上**：
    //
    // | 织思 `ArrowHead` | connection a（A→短语） | connection b（短语→B） | 读作 |
    // | --- | --- | --- | --- |
    // | `Off` | `no` | `no` | 无向关联 |
    // | `Forward` | `no` | `if-to-concept` | A → B |
    // | `Backward` | `if-to-concept` | `no` | A ← B（反向） |
    // | `Both` | `yes` | `yes` | A ↔ B |
    //
    // 之前 `Backward` 与 `Forward` 都写 `if-to-concept`，两个方向的 XML 一模一样——
    // 那是**撒谎的降级**，会让 CmapTools 把反向命题画成正向。
    out.push_str("    <connection-appearance-list>\n");
    for t in &tris {
        let (arrow_a, arrow_b) = match t.head {
            mw_core::model::ArrowHead::Off => ("no", "no"),
            mw_core::model::ArrowHead::Forward => ("no", "if-to-concept"),
            mw_core::model::ArrowHead::Backward => ("if-to-concept", "no"),
            mw_core::model::ArrowHead::Both => ("yes", "yes"),
        };
        for (suffix, arrow) in [("a", arrow_a), ("b", arrow_b)] {
            out.push_str(&format!(
                "      <connection-appearance id=\"{}\" arrowhead=\"{arrow}\"/>\n",
                xml_escape(&format!("c-{}-{suffix}", t.phrase_id))
            ));
        }
    }
    out.push_str("    </connection-appearance-list>\n");

    // 保真槽：完整 FileDoc JSON（XML 转义后塞进属性值）。内容在函数开头就算好了——
    // 这里只负责写进 XML（`dcterms:created` 要用它的 `createdAt`）。
    out.push_str("    <extra-properties-list>\n");
    out.push_str("      <properties-list id=\"mindweaver\">\n");
    out.push_str("        <property><key>doc</key><value>");
    out.push_str(&xml_escape(&fidelity_str));
    out.push_str("</value></property>\n");
    out.push_str("      </properties-list>\n");
    out.push_str("    </extra-properties-list>\n");

    out.push_str("  </map>\n");
    out.push_str("</cmap>\n");
    Ok(out)
}

/// 概念坐标：有 `x/y` 用之，否则按序号铺个确定性网格（保真槽里仍是 `None`，回到织思
/// 交给整理布局）。
fn concept_xy(c: &mw_format::ConceptEntry, i: usize) -> (f32, f32) {
    match (c.x, c.y) {
        (Some(x), Some(y)) => (x, y),
        _ => {
            let col = (i % 5) as f32;
            let row = (i / 5) as f32;
            (80.0 + col * 180.0, 80.0 + row * 120.0)
        }
    }
}

fn pos_of(concepts: &[mw_format::ConceptEntry], id: &str, seed: usize) -> (f32, f32) {
    concepts
        .iter()
        .enumerate()
        .find(|(_, c)| c.id == id)
        .map(|(i, c)| concept_xy(c, i))
        .unwrap_or((100.0 + seed as f32 * 30.0, 100.0))
}

/// 毫秒 epoch → ISO 8601 UTC（`YYYY-MM-DDTHH:MM:SS.mmmZ`）。
///
/// 手写公历换算，**不引入 `chrono` 之类的新依赖**（圆桌纪律：不加依赖）。
/// 算法是 Howard Hinnant 的 `civil_from_days`（把 1970-01-01 起的天数换算成年月日），
/// 只用整数运算，对 `1970` 前后的日子都成立。
///
/// 以前这里是 `format!("1970-01-01T00:00:{secs:02}Z")`：既不是合法 ISO 8601
/// （秒数会是 `1794258000` 这种十位数），日期也永远是 1970-01-01（CM-23 §6 第 5 条）。
fn iso8601_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);

    // civil_from_days：把「1970-01-01 起的天数」换成 (y, m, d)。
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]，3 月为 0
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// 测试用：把 [`iso8601_utc`] 写出来的串解析回毫秒 epoch。
///
/// 只给单测用（「写进去的时间戳必须能被解析」），所以 `#[cfg(test)]`。
#[cfg(test)]
fn parse_iso8601(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    // 固定形状：`YYYY-MM-DDTHH:MM:SS.mmmZ`（24 字节）。
    if b.len() != 24 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' {
        return None;
    }
    if b[16] != b':' || b[19] != b'.' || b[23] != b'Z' {
        return None;
    }
    let num = |from: usize, len: usize| -> Option<i64> {
        std::str::from_utf8(&b[from..from + len]).ok()?.parse().ok()
    };
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    let millis = num(20, 3)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // days_from_civil（`civil_from_days` 的逆运算）。
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400_000 + (hour * 3_600 + minute * 60 + second) * 1_000 + millis)
}

/// `#rrggbb` / `#aarrggbb` → CXL 的 `"r,g,b,a"`（0–255）。非法返回 `None`。
fn hex_to_rgba(hex: &str) -> Option<String> {
    let h = hex.trim().trim_start_matches('#');
    let (r, g, b, a) = match h.len() {
        6 => (
            u8::from_str_radix(&h[0..2], 16).ok()?,
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
            255u8,
        ),
        8 => (
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
            u8::from_str_radix(&h[6..8], 16).ok()?,
            u8::from_str_radix(&h[0..2], 16).ok()?,
        ),
        _ => return None,
    };
    Some(format!("{r},{g},{b},{a}"))
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
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

// ---------------------------------------------------------------------------
// 读：CXL → Document
// ---------------------------------------------------------------------------

/// 从 CXL 读回概念图文档。**保真槽优先**（无损）；缺失则解析互通视图（降级）。
pub fn load_cxl(xml: &str) -> Result<Document, IoError> {
    // 1) 保真槽：extra-properties-list → mindweaver → doc。
    if let Some(fidelity) = extract_fidelity(xml) {
        let loaded = mw_format::doc_from_fidelity_json(fidelity.as_bytes())
            .map_err(|e| IoError::Invalid(format!("保真槽解析失败：{e}")))?;
        return Ok(loaded.doc);
    }
    // 2) 降级：解析 CXL 标准元素。
    parse_cxl_interop(xml)
}

/// 从 CXL 文本里抠出保真槽的 JSON（`properties-list id="mindweaver"` 下 `key=doc` 的 `value`）。
fn extract_fidelity(xml: &str) -> Option<String> {
    // 轻量扫描：找 <key>doc</key><value>…</value>。value 内是 XML 转义的 JSON。
    let key_pos = xml.find("<key>doc</key>")?;
    let after = &xml[key_pos + "<key>doc</key>".len()..];
    let vstart = after.find("<value>")? + "<value>".len();
    let vend = after[vstart..].find("</value>")? + vstart;
    let raw = &after[vstart..vend];
    Some(xml_unescape(raw))
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// 解析 CXL 互通视图（概念 / 短语 / 连接 / 外观 / 聚焦问题）→ 概念图文档（降级）。
fn parse_cxl_interop(xml: &str) -> Result<Document, IoError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut concepts: Vec<(String, String)> = Vec::new(); // (id, label)
    let mut phrases: Vec<(String, String)> = Vec::new();
    // (id, from, to, isBidirectional)——带 id 才能和 `connection-appearance` 对上（CM-23 §7 第 3 条）。
    let mut conns: Vec<(String, String, String, bool)> = Vec::new();
    // connection id → arrowhead 值（`yes` / `no` / `if-to-concept` …）。
    let mut arrows: HashMap<String, String> = HashMap::new();
    let mut xy: HashMap<String, (f32, f32)> = HashMap::new();
    let mut focus: Option<String> = None;
    let mut title: Option<String> = None;
    let mut section = String::new();
    let mut in_desc = false;
    let mut in_title = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "concept-list"
                    | "linking-phrase-list"
                    | "connection-list"
                    | "concept-appearance-list"
                    | "linking-phrase-appearance-list" => {
                        section = name.clone();
                    }
                    "dc:description" => in_desc = true,
                    // 标题：CM-23 §7 第 1 条。以前只写不读，降级回读恒为「概念图」。
                    "dc:title" => in_title = true,
                    _ => {}
                }
            }
            // 自闭合叶元素（`<concept/>` 等）在 quick-xml 里是 Empty，不是 Start。
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "concept" => {
                        let id = attr(&e, "id").unwrap_or_default();
                        let label = attr(&e, "label").unwrap_or_default();
                        if !id.is_empty() {
                            concepts.push((id, label));
                        }
                    }
                    "linking-phrase" => {
                        let id = attr(&e, "id").unwrap_or_default();
                        let label = attr(&e, "label").unwrap_or_default();
                        if !id.is_empty() {
                            phrases.push((id, label));
                        }
                    }
                    "connection" => {
                        let id = attr(&e, "id").unwrap_or_default();
                        let f = attr(&e, "from-id").unwrap_or_default();
                        let t = attr(&e, "to-id").unwrap_or_default();
                        if !f.is_empty() && !t.is_empty() {
                            let bidir = attr(&e, "isBidirectional")
                                .map(|v| v.eq_ignore_ascii_case("true"))
                                .unwrap_or(false);
                            conns.push((id, f, t, bidir));
                        }
                    }
                    "connection-appearance" => {
                        let id = attr(&e, "id").unwrap_or_default();
                        if let Some(ar) = attr(&e, "arrowhead") {
                            arrows.insert(id, ar);
                        }
                    }
                    "concept-appearance" => {
                        let id = attr(&e, "id").unwrap_or_default();
                        let x = attr(&e, "x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                        let y = attr(&e, "y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                        if !id.is_empty() {
                            xy.insert(id, (x, y));
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if in_desc {
                    focus = Some(t.unescape().unwrap_or_default().to_string());
                } else if in_title {
                    title = Some(t.unescape().unwrap_or_default().to_string());
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "dc:description" {
                    in_desc = false;
                }
                if name == "dc:title" {
                    in_title = false;
                }
                if name == section {
                    section.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(IoError::Xml(e.to_string())),
            _ => {}
        }
    }

    if concepts.is_empty() {
        return Err(IoError::Invalid("CXL 里没有概念".into()));
    }

    // 重建命题：每个短语 P，找「A→P」与「P→B」两条连接 → 命题 A--label-->B。
    //
    // 箭头方向（CM-23 §7 第 3 条）：`connection-appearance` 里 `arrowhead` 不是 `no` 的那条边
    // 就是箭头所在的那条边。两条都有 → `Both`；只有 A→P 有 → `Backward`（箭头指向源概念）；
    // 只有 P→B 有 → `Forward`；都没有 → `Off`。`isBidirectional="true"` 视作该边两端都有箭头。
    // 一条 appearance 都没写（外部工具常见）时保持旧默认 `Forward`，不凭空改成 `Off`。
    let into: HashMap<&str, (&str, &str, bool)> = conns
        .iter()
        .filter(|(_, _, to, _)| phrases.iter().any(|(pid, _)| pid == to))
        .map(|(cid, f, t, b)| (t.as_str(), (f.as_str(), cid.as_str(), *b)))
        .collect();
    let out: HashMap<&str, (&str, &str, bool)> = conns
        .iter()
        .filter(|(_, f, _, _)| phrases.iter().any(|(pid, _)| pid == f))
        .map(|(cid, f, t, b)| (f.as_str(), (t.as_str(), cid.as_str(), *b)))
        .collect();
    let has_arrow = |conn_id: &str, bidir: bool| -> bool {
        if bidir {
            return true;
        }
        match arrows.get(conn_id) {
            Some(v) => !v.eq_ignore_ascii_case("no"),
            None => false,
        }
    };
    let mut block = mw_format::ConceptMapBlock::default();
    for (pid, label) in &phrases {
        let (Some((from, in_id, in_bidir)), Some((to, out_id, out_bidir))) =
            (into.get(pid.as_str()), out.get(pid.as_str()))
        else {
            continue;
        };
        let arrow_in = has_arrow(in_id, *in_bidir);
        let arrow_out = has_arrow(out_id, *out_bidir);
        let head = match (arrow_in, arrow_out) {
            (true, true) => mw_core::model::ArrowHead::Both,
            (true, false) => mw_core::model::ArrowHead::Backward,
            (false, true) => mw_core::model::ArrowHead::Forward,
            (false, false) => {
                if arrows.is_empty() {
                    // 一个 appearance 都没有：旧文件的默认行为，保持 `Forward`。
                    mw_core::model::ArrowHead::Forward
                } else {
                    mw_core::model::ArrowHead::Off
                }
            }
        };
        block.propositions.push(mw_format::PropositionEntry {
            id: pid.clone(),
            from: (*from).to_string(),
            to: (*to).to_string(),
            label: Some(label.clone()),
            color: None,
            head,
            dashed: false,
            width: None,
            note: None,
        });
    }
    for (id, label) in &concepts {
        let (x, y) = xy.get(id.as_str()).copied().unwrap_or((0.0, 0.0));
        block.concepts.push(mw_format::ConceptEntry {
            id: id.clone(),
            text: label.clone(),
            x: Some(x),
            y: Some(y),
            pinned: None,
            style: None,
        });
    }
    block.focus_question = focus;

    build_concept_doc(block, title)
}

/// `ConceptMapBlock` → 概念图 `Document`（降级路径与保真路径共用的收口）。
///
/// `title` 取互通视图里的 `<dc:title>`：有就用（CM-23 §7 第 1 条），空或全空白才退回「概念图」。
fn build_concept_doc(
    block: mw_format::ConceptMapBlock,
    title: Option<String>,
) -> Result<Document, IoError> {
    let nodes: HashMap<String, Node> = mw_format::concept_to_nodes(&block);
    // 根：任取一个概念（概念图节点全是 parent_id=None）。确定性取排序后第一个。
    let mut ids: Vec<&String> = nodes.keys().collect();
    ids.sort();
    let root_id = ids
        .first()
        .map(|s| (*s).clone())
        .ok_or_else(|| IoError::Invalid("概念图没有概念".into()))?;
    let meta = mw_core::DocMeta {
        title: title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "概念图".to_string()),
        theme: "snow".to_string(),
        theme_accent: None,
        theme_overrides: None,
        layout: mw_core::LayoutSettings {
            kind: "conceptMap".to_string(),
            focus_question: block.focus_question.clone(),
            ..Default::default()
        },
    };
    let groups: Vec<ConceptGroup> = block.groups.clone();
    Document::from_parts_full(None, meta, root_id, nodes, Vec::new(), groups)
        .map_err(|e| IoError::Invalid(format!("概念图结构无效：{e}")))
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().with_checks(false).flatten().find_map(|a| {
        if a.key.as_ref() == key.as_bytes() {
            Some(String::from_utf8_lossy(&a.value).to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concept_doc() -> Document {
        let mut nodes = HashMap::new();
        let a = Node::new("c-a".into(), None, "植物");
        let mut b = Node::new("c-b".into(), None, "水");
        b.relationships.push(mw_core::model::Relationship {
            id: "p1".into(),
            to: "c-a".into(),
            label: Some("需要".into()),
            ..Default::default()
        });
        nodes.insert("c-a".into(), a);
        nodes.insert("c-b".into(), b);
        let meta = mw_core::DocMeta {
            title: "植物图".into(),
            theme: "snow".into(),
            theme_accent: None,
            theme_overrides: None,
            layout: mw_core::LayoutSettings {
                kind: "conceptMap".into(),
                focus_question: Some("植物由什么构成？".into()),
                ..Default::default()
            },
        };
        Document::from_parts_full(None, meta, "c-a".into(), nodes, Vec::new(), Vec::new()).unwrap()
    }

    #[test]
    fn cxl_roundtrips_fidelity_lossless() {
        let doc = concept_doc();
        let xml = save_cxl(&doc).unwrap();
        let back = load_cxl(&xml).unwrap();
        assert_eq!(
            back.meta.layout.focus_question.as_deref(),
            Some("植物由什么构成？")
        );
        assert_eq!(back.nodes.len(), 2);
        let b = back.nodes.get("c-b").unwrap();
        assert_eq!(b.relationships.len(), 1);
        assert_eq!(b.relationships[0].label.as_deref(), Some("需要"));
    }

    #[test]
    fn cxl_interop_has_concepts_phrases_connections_and_focus() {
        let xml = save_cxl(&concept_doc()).unwrap();
        assert!(
            xml.contains("<concept id=\"c-a\" label=\"植物\"/>"),
            "{xml}"
        );
        assert!(xml.contains("linking-phrase"), "{xml}");
        assert!(xml.contains("from-id=\"c-b\" to-id=\"lp-p1\""), "{xml}");
        assert!(
            xml.contains("植物由什么构成？"),
            "聚焦问题进 dc:description"
        );
    }

    #[test]
    fn cxl_degraded_parse_without_fidelity() {
        // 纯互通视图（无保真槽）也要能打开，只是丢织思扩展字段。
        let doc = concept_doc();
        let xml = save_cxl(&doc).unwrap();
        // 去掉保真槽模拟外部工具另存。
        let back = load_cxl(&strip_fidelity(&xml)).unwrap();
        assert!(back.nodes.len() >= 2, "降级仍要读到概念");
    }

    /// 去掉保真槽，模拟「外部工具打开后另存、把织思属性抹掉」。
    fn strip_fidelity(xml: &str) -> String {
        let start = xml.find("<extra-properties-list>").expect("有保真槽");
        let end = xml.find("</extra-properties-list>").expect("有保真槽")
            + "</extra-properties-list>".len();
        format!("{}{}", &xml[..start], &xml[end..])
    }

    /// 换掉那唯一一条命题的箭头（`concept_doc` 里是 `c-b → c-a`）。
    fn doc_with_head(head: mw_core::model::ArrowHead) -> Document {
        let mut doc = concept_doc();
        doc.nodes.get_mut("c-b").unwrap().relationships[0].head = head;
        doc
    }

    /// 一条命题在 XML 里那两条 `connection-appearance` 长什么样（`-a` 是源→短语，`-b` 是短语→目标）。
    fn appearance_pair(xml: &str, conn: &str) -> (String, String) {
        let grab = |suffix: &str| -> String {
            let needle = format!("<connection-appearance id=\"c-{conn}-{suffix}\" arrowhead=\"");
            let at = xml.find(&needle).expect("有 connection-appearance");
            let rest = &xml[at + needle.len()..];
            rest[..rest.find('"').unwrap()].to_string()
        };
        (grab("a"), grab("b"))
    }

    // ---- CM-23 §7 第 1 条：降级路径不再丢标题 ---------------------------------

    #[test]
    fn cxl_degraded_parse_keeps_title_from_dc_title() {
        let xml = save_cxl(&concept_doc()).unwrap();
        assert!(
            xml.contains("<dc:title>植物图</dc:title>"),
            "标题要写进 dc:title"
        );
        let back = load_cxl(&strip_fidelity(&xml)).unwrap();
        assert_eq!(back.meta.title, "植物图", "有 dc:title 就该用它");
    }

    #[test]
    fn cxl_degraded_parse_falls_back_to_default_title() {
        let xml = save_cxl(&concept_doc()).unwrap();
        // ① 完全没有 dc:title。
        let no_title = strip_fidelity(&xml).replace("<dc:title>植物图</dc:title>", "");
        assert_eq!(load_cxl(&no_title).unwrap().meta.title, "概念图");
        // ② 有 dc:title 但只有空白。
        let blank_title =
            strip_fidelity(&xml).replace("<dc:title>植物图</dc:title>", "<dc:title>   </dc:title>");
        assert_eq!(load_cxl(&blank_title).unwrap().meta.title, "概念图");
    }

    // ---- CM-23 §7 第 2 条：时间戳必须是合法 ISO 8601 且有序 --------------------

    #[test]
    fn cxl_timestamps_are_iso8601_utc_and_ordered() {
        let xml = save_cxl(&concept_doc()).unwrap();
        let field = |tag: &str| -> String {
            let open = format!("<{tag}>");
            let close = format!("</{tag}>");
            let at = xml.find(&open).expect("有时间戳") + open.len();
            xml[at..xml.find(&close).unwrap()].to_string()
        };
        let created = field("dcterms:created");
        let modified = field("dcterms:modified");
        // 形状：`YYYY-MM-DDTHH:MM:SS.mmmZ`（24 字节），且必须能被解析回去。
        assert_eq!(created.len(), 24, "created 形状不对：{created}");
        assert!(created.ends_with('Z'), "必须是 UTC：{created}");
        let created_ms =
            parse_iso8601(&created).unwrap_or_else(|| panic!("created 不可解析：{created}"));
        let modified_ms =
            parse_iso8601(&modified).unwrap_or_else(|| panic!("modified 不可解析：{modified}"));
        assert!(created_ms <= modified_ms, "created 不能晚于 modified");
        // 不许再出现 1970 那种占位：离当前时间不超过一天。
        let now = mw_core::now_ms();
        assert!(
            (now - modified_ms).abs() < 86_400_000,
            "modified 应为当前时间，实际 {modified}（now={now}）"
        );
    }

    #[test]
    fn iso8601_utc_roundtrips_known_instants() {
        // 0 = 1970-01-01T00:00:00.000Z；2026-11-02T12:34:56.789Z 是手算的独立样本。
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(parse_iso8601("1970-01-01T00:00:00.000Z"), Some(0));
        let sample = "2026-11-02T12:34:56.789Z";
        let ms = parse_iso8601(sample).expect("可解析");
        assert_eq!(iso8601_utc(ms), sample, "格式化与解析必须互逆");
    }

    // ---- CM-23 §7 第 3 条：两个方向不许写成同一个值 ---------------------------

    #[test]
    fn cxl_arrowhead_slots_encode_direction() {
        use mw_core::model::ArrowHead;
        // (head, a 边, b 边)：箭头写在它实际所在的那条边上。
        let table = [
            (ArrowHead::Off, "no", "no"),
            (ArrowHead::Forward, "no", "if-to-concept"),
            (ArrowHead::Backward, "if-to-concept", "no"),
            (ArrowHead::Both, "yes", "yes"),
        ];
        for (head, want_a, want_b) in table {
            let xml = save_cxl(&doc_with_head(head)).unwrap();
            let (a, b) = appearance_pair(&xml, "lp-p1");
            assert_eq!(
                (a.as_str(), b.as_str()),
                (want_a, want_b),
                "{head:?} 的两条边"
            );
        }
        // 反向与正向的 XML 必须**不同**（以前两者都写成 if-to-concept，等于撒谎）。
        let fwd = appearance_section(
            &save_cxl(&doc_with_head(mw_core::model::ArrowHead::Forward)).unwrap(),
        );
        let bwd = appearance_section(
            &save_cxl(&doc_with_head(mw_core::model::ArrowHead::Backward)).unwrap(),
        );
        assert_ne!(fwd, bwd, "Forward 与 Backward 不许写成同一份 XML");
    }

    /// 只取 `connection-appearance-list` 那一段——保真槽里每次都有 `createdAt`，
    /// 整份 XML 天然不可能逐字节相等，比较必须落在这一节上。
    fn appearance_section(xml: &str) -> String {
        let start = xml.find("<connection-appearance-list>").expect("有箭头节");
        let end = xml.find("</connection-appearance-list>").expect("有箭头节");
        xml[start..end].to_string()
    }

    #[test]
    fn cxl_interop_parse_reads_arrowhead_direction_back() {
        use mw_core::model::ArrowHead;
        for head in [
            ArrowHead::Off,
            ArrowHead::Forward,
            ArrowHead::Backward,
            ArrowHead::Both,
        ] {
            let xml = save_cxl(&doc_with_head(head)).unwrap();
            let back = load_cxl(&strip_fidelity(&xml)).unwrap();
            let got = back.nodes["c-b"].relationships[0].head;
            assert_eq!(got, head, "降级解析要把 {head:?} 读回来");
        }
    }

    // ---- CM-23 §7 第 4 条：groups 只走保真槽，且真的走得通 ---------------------

    fn rich_concept_doc() -> Document {
        use mw_core::model::{ArrowHead, ConceptGroup, Position, Relationship, StyleOverride};
        let mut a = Node::new("c-a".into(), None, "植物");
        a.pos = Some(Position { x: 10.0, y: 20.0 });
        a.pinned = Some(true);
        a.style = Some(StyleOverride {
            fill: Some("#ffcc00".into()),
            stroke: Some("#123456".into()),
            font_size: Some(18),
            bold: Some(true),
            ..Default::default()
        });
        a.note = Some("概念备注".into());
        a.labels = vec!["标签A".into()];
        // 卡片必须真实存在（`validate` 会拒「节点链接的卡片缺失」），所以先建卡再挂。
        let card = mw_core::NoteCard::new("doc-1", Some("c-a".into()), "卡片", "卡片正文", 1);
        a.card_id = Some(card.id.clone());
        let mut b = Node::new("c-b".into(), None, "水");
        b.pos = Some(Position { x: 220.0, y: 20.0 });
        b.relationships.push(Relationship {
            id: "p1".into(),
            to: "c-a".into(),
            label: Some("需要".into()),
            color: Some("#3366ff".into()),
            head: ArrowHead::Backward,
            dashed: true,
            width: Some(2.5),
            note: Some("命题注释".into()),
        });
        let mut nodes = HashMap::new();
        nodes.insert("c-a".into(), a);
        nodes.insert("c-b".into(), b);
        let groups = vec![ConceptGroup {
            id: "g1".into(),
            label: "分组一".into(),
            members: vec!["c-a".into(), "c-b".into()],
            color: Some("#888888".into()),
        }];
        let meta = mw_core::DocMeta {
            title: "植物图".into(),
            theme: "mist".into(),
            theme_accent: Some("sand".into()),
            theme_overrides: None,
            layout: mw_core::LayoutSettings {
                kind: "conceptMap".into(),
                focus_question: Some("植物由什么构成？".into()),
                ..Default::default()
            },
        };
        Document::from_parts_full(
            Some("doc-1".into()),
            meta,
            "c-a".into(),
            nodes,
            vec![card],
            groups,
        )
        .unwrap()
    }

    #[test]
    fn cxl_fidelity_roundtrip_preserves_groups_pinned_and_line_styles() {
        let doc = rich_concept_doc();
        let xml = save_cxl(&doc).unwrap();
        // 互通视图里不许出现分组元素（CXL 没有分组；硬写就是发明格式）。
        assert!(!xml.contains("<group"), "互通视图不该有分组元素：{xml}");
        let back = load_cxl(&xml).unwrap();
        assert_eq!(back.meta.title, doc.meta.title);
        assert_eq!(back.meta.theme, doc.meta.theme);
        assert_eq!(back.meta.theme_accent, doc.meta.theme_accent);
        assert_eq!(back.groups, doc.groups, "分组必须随保真槽往返");
        let a = &back.nodes["c-a"];
        assert_eq!(a.pinned, Some(true));
        assert_eq!(a.style, doc.nodes["c-a"].style);
        let rel = &back.nodes["c-b"].relationships[0];
        assert_eq!(rel.color.as_deref(), Some("#3366ff"));
        assert_eq!(rel.head, mw_core::model::ArrowHead::Backward);
        assert!(rel.dashed);
        assert_eq!(rel.width, Some(2.5));
        assert_eq!(rel.note.as_deref(), Some("命题注释"));
    }

    /// CM-23 §7.5 的账：**保真槽里写着**节点的备注/标签/卡片，但读取侧走
    /// `mw-format` 的 `doc_from_file_doc`，概念图分支用 `concept_map::to_nodes` 重建节点，
    /// 而 `ConceptEntry` 只有 id/text/x/y/pinned/style —— 这几个字段就丢在读的一侧。
    ///
    /// 这条不是 `mw-io` 的 bug，修点在 `mw-format`（本轮禁改区），所以这里把它钉成
    /// 「写得出、读不回」的事实；那边修好后本测试应改成断言保留。
    #[test]
    fn cxl_fidelity_writes_node_extras_but_does_not_read_them_back() {
        let doc = rich_concept_doc();
        let xml = save_cxl(&doc).unwrap();
        assert!(xml.contains("概念备注"), "保真槽里应当写着备注：{xml}");
        assert!(xml.contains("标签A"), "保真槽里应当写着标签：{xml}");
        let back = load_cxl(&xml).unwrap();
        assert_eq!(back.nodes["c-a"].note, None, "已知缺陷：读取侧丢备注");
        assert!(
            back.nodes["c-a"].labels.is_empty(),
            "已知缺陷：读取侧丢标签"
        );
        assert_eq!(
            back.nodes["c-a"].card_id, None,
            "已知缺陷：读取侧丢 card_id"
        );
    }

    #[test]
    fn cxl_degraded_parse_drops_the_documented_fields_explicitly() {
        // 保真槽被抹掉后「丢什么」要写成断言：这是设计边界，不是意外。
        let doc = rich_concept_doc();
        let back = load_cxl(&strip_fidelity(&save_cxl(&doc).unwrap())).unwrap();
        assert_eq!(back.meta.title, "植物图", "标题现在能救回来（§7 第 1 条）");
        assert_eq!(back.meta.theme, "snow", "主题没有互通槽，只能回落");
        assert!(back.groups.is_empty(), "分组只在保真槽里");
        assert_eq!(back.nodes["c-a"].pinned, None);
        assert_eq!(back.nodes["c-a"].style, None);
        assert_eq!(back.nodes["c-a"].note, None);
        let rel = &back.nodes["c-b"].relationships[0];
        assert_eq!(rel.color, None);
        assert!(!rel.dashed);
        assert_eq!(rel.width, None);
        assert_eq!(rel.note, None);
    }
}
