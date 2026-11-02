//! 卡片笔记的 Markdown 解析：**纯 CPU、无 GUI**。
//!
//! 为什么自己写而不是引第三方：阅读页、编辑器预览、笔记库预览、导出必须共用同一棵
//! AST，否则同一个 `**粗体**` 在四个地方会长得不一样。语法范围只覆盖卡片笔记真正
//! 会用的子集（P10），表格/任务列表/图片/脚注在 P11 扩展。
//!
//! 设计取向：
//! - 行内样式用 `Span` 扁平化，渲染器不必递归处理嵌套强调；
//! - 行内代码只认反引号，不做嵌套；
//! - 未闭合的 `**`/`*`/`` ` ``/`[` 原样保留，不吞字；
//! - 列表按缩进递归，缩进内的内容交给 `parse_blocks`，天然支持嵌套列表。

/// 图片引用。卡片正文里的图片只存 URL（本地库/导出时再决定是否内嵌）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    pub url: String,
    pub title: Option<String>,
}

/// 一段带样式的行内文字。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    /// `[文字](url)` 的 url；普通文字为 None。
    pub link: Option<String>,
    /// `![alt](url)`：`text` 存 alt。
    pub image: Option<ImageRef>,
}

impl Span {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            code: false,
            link: None,
            image: None,
        }
    }
}

/// 表格列对齐。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// 列表项：任务列表用 `task` 存勾选态，普通列表为 None。
#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// 块级节点。
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        items: Vec<ListItem>,
    },
    Code {
        lang: Option<String>,
        text: String,
    },
    Table {
        headers: Vec<Vec<Span>>,
        aligns: Vec<Align>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    Rule,
}

impl Block {
    fn plain(&self, out: &mut String) {
        match self {
            Block::Heading { spans, .. } | Block::Paragraph(spans) => {
                for s in spans {
                    out.push_str(&s.text);
                }
                out.push('\n');
            }
            Block::Quote(blocks) => {
                for b in blocks {
                    b.plain(out);
                }
            }
            Block::List { items, .. } => {
                for item in items {
                    match item.task {
                        Some(true) => out.push_str("- [x] "),
                        Some(false) => out.push_str("- [ ] "),
                        None => out.push_str("- "),
                    }
                    for b in &item.blocks {
                        b.plain(out);
                    }
                }
            }
            Block::Table { headers, rows, .. } => {
                for cell in headers {
                    push_cell(out, cell);
                }
                out.push('\n');
                for row in rows {
                    for cell in row {
                        push_cell(out, cell);
                    }
                    out.push('\n');
                }
            }
            Block::Code { text, .. } => {
                out.push_str(text);
                out.push('\n');
            }
            Block::Rule => out.push_str("---\n"),
        }
    }
}

fn push_cell(out: &mut String, cell: &[Span]) {
    for s in cell {
        out.push_str(&s.text);
    }
    out.push_str(" | ");
}

/// 解析结果。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Document {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// 纯文本（保留块之间的换行，去掉 Markdown 标记）。列表用 `- ` 前缀。
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for b in &self.blocks {
            b.plain(&mut out);
        }
        out
    }

    /// 单行摘要：压掉空行与多余空白，按字符截断。
    pub fn excerpt(&self, max_chars: usize) -> String {
        let text = self.plain_text();
        let mut out = String::new();
        for word in text.split_whitespace() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(word);
            if out.chars().count() >= max_chars {
                break;
            }
        }
        out.chars().take(max_chars).collect()
    }

    /// 正文里的所有链接（导出/反链用）。
    pub fn links(&self) -> Vec<String> {
        fn walk(blocks: &[Block], out: &mut Vec<String>) {
            for b in blocks {
                match b {
                    Block::Heading { spans, .. } | Block::Paragraph(spans) => {
                        for s in spans {
                            if let Some(u) = &s.link {
                                out.push(u.clone());
                            }
                        }
                    }
                    Block::Quote(inner) => walk(inner, out),
                    Block::List { items, .. } => {
                        for item in items {
                            walk(&item.blocks, out);
                        }
                    }
                    Block::Table { headers, rows, .. } => {
                        for cell in headers {
                            for s in cell {
                                if let Some(u) = &s.link {
                                    out.push(u.clone());
                                }
                            }
                        }
                        for row in rows {
                            for cell in row {
                                for s in cell {
                                    if let Some(u) = &s.link {
                                        out.push(u.clone());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.blocks, &mut out);
        out
    }

    /// 正文里的所有图片（导出/预览用）。
    pub fn images(&self) -> Vec<ImageRef> {
        fn span_images(spans: &[Span], out: &mut Vec<ImageRef>) {
            for s in spans {
                if let Some(img) = &s.image {
                    out.push(img.clone());
                }
            }
        }
        fn walk(blocks: &[Block], out: &mut Vec<ImageRef>) {
            for b in blocks {
                match b {
                    Block::Heading { spans, .. } | Block::Paragraph(spans) => {
                        span_images(spans, out);
                    }
                    Block::Quote(inner) => walk(inner, out),
                    Block::List { items, .. } => {
                        for item in items {
                            walk(&item.blocks, out);
                        }
                    }
                    Block::Table { headers, rows, .. } => {
                        for cell in headers {
                            span_images(cell, out);
                        }
                        for row in rows {
                            for cell in row {
                                span_images(cell, out);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.blocks, &mut out);
        out
    }
}

/// 解析 Markdown。
pub fn parse(text: &str) -> Document {
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = norm.split('\n').collect();
    Document {
        blocks: parse_blocks(&lines),
    }
}

fn parse_blocks(lines: &[&str]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        if let Some((fence_char, fence_len, lang)) = fence_open(line) {
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() {
                if is_fence_close(lines[i], fence_char, fence_len) {
                    i += 1;
                    break;
                }
                code.push(lines[i]);
                i += 1;
            }
            out.push(Block::Code {
                lang,
                text: code.join("\n"),
            });
            continue;
        }
        if let Some((level, text)) = heading(line) {
            out.push(Block::Heading {
                level,
                spans: parse_inline(text),
            });
            i += 1;
            continue;
        }
        if is_rule(line) {
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some(stripped) = quote_line(line) {
            let mut inner: Vec<&str> = Vec::new();
            while i < lines.len() {
                let Some(s) = quote_line(lines[i]) else {
                    break;
                };
                inner.push(s);
                i += 1;
            }
            out.push(Block::Quote(parse_blocks(&inner)));
            let _ = stripped;
            continue;
        }
        if list_marker(line).is_some() {
            let (block, next) = parse_list(lines, i);
            out.push(block);
            i = next;
            continue;
        }
        // 表格：表头行 + 对齐分隔行。必须成对出现，否则当普通段落。
        if line.contains('|') && i + 1 < lines.len() && table_aligns(lines[i + 1]).is_some() {
            let (block, next) = parse_table(lines, i);
            out.push(block);
            i = next;
            continue;
        }
        // 段落：直到空行或下一个块起点。
        let mut para: Vec<&str> = Vec::new();
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() || (!para.is_empty() && is_block_start(l)) {
                break;
            }
            para.push(l);
            i += 1;
        }
        let mut spans = Vec::new();
        for (k, l) in para.iter().enumerate() {
            if k > 0 {
                spans.push(Span::plain("\n"));
            }
            spans.extend(parse_inline(l.trim_end()));
        }
        out.push(Block::Paragraph(spans));
    }
    out
}

fn is_block_start(line: &str) -> bool {
    fence_open(line).is_some()
        || heading(line).is_some()
        || is_rule(line)
        || quote_line(line).is_some()
        || list_marker(line).is_some()
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if rest.is_empty() {
        return Some((hashes as u8, ""));
    }
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    Some((hashes as u8, rest.trim_start()))
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    let c = t.chars().next().unwrap();
    if !matches!(c, '-' | '*' | '_') {
        return false;
    }
    t.chars().all(|x| x == c || x == ' ' || x == '\t')
}

fn split_row(line: &str) -> Vec<String> {
    let mut t = line.trim();
    if let Some(x) = t.strip_prefix('|') {
        t = x;
    }
    if let Some(x) = t.strip_suffix('|') {
        t = x;
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for ch in t.chars() {
        if esc {
            cur.push(ch);
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '|' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

fn table_aligns(line: &str) -> Option<Vec<Align>> {
    let cells = split_row(line);
    if cells.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(cells.len());
    for cell in cells {
        let c = cell.trim();
        let left = c.starts_with(':');
        let right = c.ends_with(':');
        let dashes = c.trim_matches(':');
        if dashes.len() < 3 || !dashes.chars().all(|x| x == '-') {
            return None;
        }
        out.push(match (left, right) {
            (true, true) => Align::Center,
            (true, false) => Align::Left,
            (false, true) => Align::Right,
            _ => Align::None,
        });
    }
    Some(out)
}

fn parse_table(lines: &[&str], start: usize) -> (Block, usize) {
    let headers: Vec<Vec<Span>> = split_row(lines[start])
        .into_iter()
        .map(|c| parse_inline(&c))
        .collect();
    let aligns = table_aligns(lines[start + 1]).expect("调用方已确认分隔行");
    let mut rows = Vec::new();
    let mut i = start + 2;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() || !l.contains('|') || is_block_start(l) {
            break;
        }
        let mut cells: Vec<Vec<Span>> =
            split_row(l).into_iter().map(|c| parse_inline(&c)).collect();
        while cells.len() < headers.len() {
            cells.push(Vec::new());
        }
        cells.truncate(headers.len());
        rows.push(cells);
        i += 1;
    }
    (
        Block::Table {
            headers,
            aligns,
            rows,
        },
        i,
    )
}

fn fence_open(line: &str) -> Option<(char, usize, Option<String>)> {
    let t = line.trim_start();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|x| *x == c).count();
    if n < 3 {
        return None;
    }
    let info = t[n..].trim();
    let lang = if info.is_empty() {
        None
    } else {
        Some(info.split_whitespace().next().unwrap_or(info).to_string())
    };
    Some((c, n, lang))
}

fn is_fence_close(line: &str, c: char, n: usize) -> bool {
    let t = line.trim_start();
    let count = t.chars().take_while(|x| *x == c).count();
    count >= n && t[count..].trim().is_empty()
}

fn quote_line(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

#[derive(Clone, Copy)]
struct Marker {
    indent: usize,
    ordered: bool,
    start: u64,
    /// 内容在原始行里的列（用于续行 dedent）。
    content_col: usize,
    /// 标记之后的内容。
    content_start: usize,
}

fn list_marker(line: &str) -> Option<Marker> {
    let indent = line.chars().take_while(|c| *c == ' ').count();
    let rest = &line[indent.min(line.len())..];
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    // 无序：- * + 后跟空白
    if matches!(bytes[0], b'-' | b'*' | b'+') {
        if bytes.len() == 1 {
            return Some(Marker {
                indent,
                ordered: false,
                start: 0,
                content_col: indent + 1,
                content_start: indent + 1,
            });
        }
        if bytes[1].is_ascii_whitespace() {
            let skip = 1 + bytes[1..]
                .iter()
                .take_while(|b| b.is_ascii_whitespace())
                .count();
            return Some(Marker {
                indent,
                ordered: false,
                start: 0,
                content_col: indent + 1,
                content_start: indent + skip,
            });
        }
        return None;
    }
    // 有序：1. / 1) 后跟空白
    let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || digits > 9 || digits >= bytes.len() {
        return None;
    }
    if !matches!(bytes[digits], b'.' | b')') {
        return None;
    }
    if digits + 1 < bytes.len() && !bytes[digits + 1].is_ascii_whitespace() {
        return None;
    }
    let start = rest[..digits].parse::<u64>().ok()?;
    let skip = digits
        + 1
        + bytes[digits + 1..]
            .iter()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
    Some(Marker {
        indent,
        ordered: true,
        start,
        content_col: indent + digits + 1,
        content_start: indent + skip,
    })
}

fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

fn dedent(line: &str, cols: usize) -> String {
    let mut removed = 0;
    let mut out = String::new();
    for c in line.chars() {
        if removed < cols && (c == ' ' || c == '\t') {
            removed += 1;
            continue;
        }
        out.push(c);
    }
    out
}

/// `[ ] 待办` / `[x] 完成`：返回勾选态与去掉标记后的正文。
fn task_marker(s: &str) -> (Option<bool>, String) {
    let t = s.trim_start();
    for (mark, value) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
        if let Some(rest) = t.strip_prefix(mark) {
            if rest.is_empty() || rest.starts_with([' ', '\t']) {
                return (Some(value), rest.trim_start().to_string());
            }
        }
    }
    (None, s.to_string())
}

fn parse_list(lines: &[&str], start: usize) -> (Block, usize) {
    let first = list_marker(lines[start]).expect("调用方已确认是列表");
    let mut items: Vec<ListItem> = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let Some(m) = list_marker(lines[i]) else {
            break;
        };
        if m.indent != first.indent || m.ordered != first.ordered {
            break;
        }
        let raw = &lines[i][m.content_start.min(lines[i].len())..];
        let (task, first_content) = task_marker(raw);
        let mut item: Vec<String> = vec![first_content];
        let mut j = i + 1;
        while j < lines.len() {
            let line = lines[j];
            if line.trim().is_empty() {
                // 空行：后面还是这一项的内容才保留；否则列表结束。
                let next = lines[j + 1..]
                    .iter()
                    .position(|l| !l.trim().is_empty())
                    .map(|p| j + 1 + p);
                let Some(n) = next else { break };
                let nm = list_marker(lines[n]);
                if leading_spaces(lines[n]) > first.indent
                    || nm.is_some_and(|x| x.indent == first.indent && x.ordered == first.ordered)
                {
                    item.push(String::new());
                    j += 1;
                    continue;
                }
                break;
            }
            if let Some(nm) = list_marker(line) {
                if nm.indent == first.indent && nm.ordered == first.ordered {
                    break;
                }
                if nm.indent <= first.indent {
                    break;
                }
            } else if leading_spaces(line) <= first.indent {
                break;
            }
            item.push(dedent(line, m.content_col));
            j += 1;
        }
        let refs: Vec<&str> = item.iter().map(|s| s.as_str()).collect();
        items.push(ListItem {
            task,
            blocks: parse_blocks(&refs),
        });
        i = j;
        // 空行后如果还是同级列表项，跳过空行继续。
        if i < lines.len() && lines[i].trim().is_empty() {
            let next = lines[i + 1..]
                .iter()
                .position(|l| !l.trim().is_empty())
                .map(|p| i + 1 + p);
            if let Some(n) = next {
                if list_marker(lines[n])
                    .is_some_and(|x| x.indent == first.indent && x.ordered == first.ordered)
                {
                    i = n;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
    (
        Block::List {
            ordered: first.ordered,
            start: first.start,
            items,
        },
        i,
    )
}

fn parse_inline(text: &str) -> Vec<Span> {
    parse_inline_styled(text, false, false)
}

fn parse_inline_styled(text: &str, bold: bool, italic: bool) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                push_span(
                    &mut out,
                    Span {
                        text: std::mem::take(&mut buf),
                        bold,
                        italic,
                        code: false,
                        link: None,
                        image: None,
                    },
                );
            }
        };
    }
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(j) = find_char(&chars, i + 1, '`') {
                flush!();
                push_span(
                    &mut out,
                    Span {
                        text: chars[i + 1..j].iter().collect(),
                        bold,
                        italic,
                        code: true,
                        link: None,
                        image: None,
                    },
                );
                i = j + 1;
                continue;
            }
        }
        if c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(j) = find_seq(&chars, i + 2, "**") {
                flush!();
                let inner: String = chars[i + 2..j].iter().collect();
                for mut s in parse_inline_styled(&inner, true, italic) {
                    s.link = None;
                    push_span(&mut out, s);
                }
                i = j + 2;
                continue;
            }
        }
        if (c == '*' || c == '_') && i + 1 < chars.len() {
            if let Some(j) = find_char(&chars, i + 1, c) {
                if j > i + 1 {
                    flush!();
                    let inner: String = chars[i + 1..j].iter().collect();
                    for mut s in parse_inline_styled(&inner, bold, true) {
                        s.link = None;
                        push_span(&mut out, s);
                    }
                    i = j + 1;
                    continue;
                }
            }
        }
        if c == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((text_end, url_end, raw)) = parse_link(&chars, i + 1) {
                flush!();
                let alt: String = chars[i + 2..text_end].iter().collect();
                let (url, title) = split_url_title(&raw);
                push_span(
                    &mut out,
                    Span {
                        text: alt,
                        bold,
                        italic,
                        code: false,
                        link: None,
                        image: Some(ImageRef { url, title }),
                    },
                );
                i = url_end;
                continue;
            }
        }
        if c == '[' {
            if let Some((text_end, url_end, url)) = parse_link(&chars, i) {
                flush!();
                let inner: String = chars[i + 1..text_end].iter().collect();
                for mut s in parse_inline_styled(&inner, bold, italic) {
                    s.link = Some(url.clone());
                    push_span(&mut out, s);
                }
                i = url_end;
                continue;
            }
        }
        buf.push(c);
        i += 1;
    }
    flush!();
    out
}

fn push_span(out: &mut Vec<Span>, s: Span) {
    if s.text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut() {
        if last.bold == s.bold
            && last.italic == s.italic
            && last.code == s.code
            && last.link == s.link
            && last.image.is_none()
            && s.image.is_none()
        {
            last.text.push_str(&s.text);
            return;
        }
    }
    out.push(s);
}

fn find_char(chars: &[char], from: usize, c: char) -> Option<usize> {
    (from..chars.len()).find(|&i| chars[i] == c)
}

fn find_seq(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || chars.len() < n.len() {
        return None;
    }
    (from..=chars.len() - n.len()).find(|&i| chars[i..i + n.len()] == n[..])
}

/// `[text](url)`：返回 (文字结束, 整段结束, url)。
fn parse_link(chars: &[char], at: usize) -> Option<(usize, usize, String)> {
    let close = find_char(chars, at + 1, ']')?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = find_char(chars, close + 2, ')')?;
    let url: String = chars[close + 2..end].iter().collect();
    let url = url.trim().to_string();
    if url.is_empty() {
        return None;
    }
    Some((close, end + 1, url))
}

/// `url "标题"` → (url, Some(标题))；没有空白就当纯 url。
fn split_url_title(raw: &str) -> (String, Option<String>) {
    let mut parts = raw.splitn(2, char::is_whitespace);
    let url = parts.next().unwrap_or(raw).trim().to_string();
    let title = parts
        .next()
        .map(str::trim)
        .map(|t| t.trim_matches(['"', '\'']).to_string())
        .filter(|t| !t.is_empty());
    (url, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_paragraphs() {
        let d = parse("# 一级\n## 二级\n\n正文第一行\n第二行");
        assert_eq!(d.blocks.len(), 3);
        match &d.blocks[0] {
            Block::Heading { level, spans } => {
                assert_eq!(*level, 1);
                assert_eq!(spans[0].text, "一级");
            }
            other => panic!("{other:?}"),
        }
        match &d.blocks[2] {
            Block::Paragraph(spans) => {
                assert!(spans.iter().any(|s| s.text == "\n"), "软换行要保留");
                assert!(d.plain_text().contains("正文第一行"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn inline_styles_and_links() {
        let d = parse("**粗** *斜* `code` [链接](https://example.com)");
        let Block::Paragraph(spans) = &d.blocks[0] else {
            panic!()
        };
        assert!(spans.iter().any(|s| s.bold && s.text == "粗"));
        assert!(spans.iter().any(|s| s.italic && s.text == "斜"));
        assert!(spans.iter().any(|s| s.code && s.text == "code"));
        assert!(spans
            .iter()
            .any(|s| s.link.as_deref() == Some("https://example.com") && s.text == "链接"));
        assert_eq!(d.links(), vec!["https://example.com"]);
    }

    #[test]
    fn malformed_markup_stays_literal() {
        let d = parse("**没有闭合 和 `代码");
        let text = d.plain_text();
        assert!(text.contains("**没有闭合"), "{text}");
        assert!(text.contains("`代码"), "{text}");
    }

    #[test]
    fn unordered_and_ordered_lists() {
        let d = parse("- 甲\n- 乙\n  - 乙一\n\n1. 一\n2. 二");
        let Block::List {
            ordered,
            items,
            start,
        } = &d.blocks[0]
        else {
            panic!("{d:?}")
        };
        assert!(!*ordered && *start == 0);
        assert_eq!(items.len(), 2);
        assert!(
            items[1]
                .blocks
                .iter()
                .any(|b| matches!(b, Block::List { .. })),
            "嵌套列表"
        );
        let Block::List {
            ordered,
            items,
            start,
        } = &d.blocks[1]
        else {
            panic!("{d:?}")
        };
        assert!(*ordered && *start == 1);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn quotes_can_contain_blocks() {
        let d = parse("> 引用第一行\n> - 引用里的列表\n\n普通段落");
        let Block::Quote(inner) = &d.blocks[0] else {
            panic!("{d:?}")
        };
        assert!(inner.iter().any(|b| matches!(b, Block::List { .. })));
        assert!(d.plain_text().contains("引用第一行"));
    }

    #[test]
    fn fenced_code_keeps_text_and_language() {
        let d = parse("```rust\nfn main() {\n    1\n}\n```\n后文");
        let Block::Code { lang, text } = &d.blocks[0] else {
            panic!("{d:?}")
        };
        assert_eq!(lang.as_deref(), Some("rust"));
        assert!(text.contains("fn main()"));
        assert!(text.contains("    1"), "代码缩进不能被吃掉");
        assert_eq!(d.blocks.len(), 2);
    }

    #[test]
    fn rules_and_blank_documents() {
        let d = parse("---\n\n***\n");
        assert_eq!(d.blocks, vec![Block::Rule, Block::Rule], "分隔线与空行");
        assert!(parse("").is_empty());
        assert!(parse("  \n\n  ").is_empty());
    }

    #[test]
    fn escapes_and_cjk() {
        let d = parse(r"不是 \*斜体\*，是星号。中文标点：，。！？");
        let text = d.plain_text();
        assert!(text.contains("*斜体*"), "{text}");
        assert!(text.contains("中文标点：，。！？"));
    }

    #[test]
    fn excerpt_collapses_lines_and_counts_chars() {
        let d = parse("# 标题\n\n第一段\n\n第二段");
        assert_eq!(d.excerpt(20), "标题 第一段 第二段");
        assert_eq!(d.excerpt(2), "标题");
    }

    #[test]
    fn crlf_is_normalized() {
        let d = parse("# 标题\r\n\r\n正文\r\n");
        assert_eq!(d.blocks.len(), 2);
        assert!(d.plain_text().contains("正文"));
    }

    #[test]
    fn tables_keep_alignment_and_inline_markup() {
        let d = parse("| 名称 | 值 |\n| :--- | ---: |\n| **甲** | `1` |\n| 乙 | 2 |");
        let Block::Table {
            headers,
            aligns,
            rows,
        } = &d.blocks[0]
        else {
            panic!("{d:?}")
        };
        assert_eq!(aligns, &vec![Align::Left, Align::Right]);
        assert_eq!(headers.len(), 2);
        assert_eq!(rows.len(), 2);
        assert!(rows[0][0].iter().any(|s| s.bold && s.text == "甲"));
        assert!(rows[0][1].iter().any(|s| s.code && s.text == "1"));
        let text = d.plain_text();
        assert!(text.contains("名称") && text.contains("乙"), "{text}");
    }

    #[test]
    fn a_pipe_line_without_delimiter_stays_a_paragraph() {
        let d = parse("a | b\nc | d");
        assert!(matches!(d.blocks[0], Block::Paragraph(_)), "{d:?}");
    }

    #[test]
    fn task_lists_expose_checked_state() {
        let d = parse("- [ ] 待办\n- [x] 完成\n- 普通");
        let Block::List { items, .. } = &d.blocks[0] else {
            panic!("{d:?}")
        };
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].task, Some(false));
        assert_eq!(items[1].task, Some(true));
        assert_eq!(items[2].task, None);
        let text = d.plain_text();
        assert!(
            text.contains("[ ] 待办") && text.contains("[x] 完成"),
            "{text}"
        );
    }

    #[test]
    fn images_keep_alt_url_and_title() {
        let d = parse("看图 ![示意图](https://example.com/a.png \"标题\") 结束");
        let images = d.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].url, "https://example.com/a.png");
        assert_eq!(images[0].title.as_deref(), Some("标题"));
        assert!(d.plain_text().contains("示意图"));
        assert!(d.links().is_empty(), "图片不算普通链接");
    }

    #[test]
    fn malformed_images_are_left_alone() {
        let d = parse("![没有地址]()");
        assert!(d.images().is_empty());
        assert!(d.plain_text().contains("![没有地址]()"));
    }
}
