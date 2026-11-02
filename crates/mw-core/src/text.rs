//! 文本度量端口（前端排版权威的抽象，docs/07 §4 / ADR-003）。
//!
//! 布局只问「这段文本以这个字号能排成几行、多宽」，不问谁来排版：
//! 真实实现由渲染层接 epaint/DirectWrite（自带 CJK 禁则与系统字体回退），
//! mw-core 不依赖任何 UI crate，所以这里只有 trait 和一个确定性近似实现。

/// 不折行的最大宽度（对齐 TS 里的 `100000`：够大到不可能超过，又不会让浮点相加溢出）。
pub const NO_WRAP: f32 = 1.0e6;

/// 一次度量的结果：折行后的行、最宽行宽度、行高。
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub lines: Vec<String>,
    pub width: f32,
    pub line_height: f32,
}

/// 文本度量器。同一输入必须给出同一输出——快照比对与增量重排都依赖这点。
///
/// `weight` 是字重（400 / 500 / 600 / 700，SE-13）：不同字面的**字宽不同**，
/// 盒子必须按真实字面量，否则 600 的长标题会被按 400 量出的盒子裁掉。
/// 老调用点传 [`crate::theme::WEIGHT_REGULAR`] / [`crate::theme::WEIGHT_BOLD`]。
pub trait TextMeasurer {
    fn measure(&self, text: &str, font_size: f32, weight: u16, max_width: f32) -> Measured;
}

/// 行高 = 字号 × **分档倍数**（见 `design::text::line_height`）后取整。
///
/// v36 之前是写死的 1.45：小字太挤、大字太散。现在小字 1.42（更松）、
/// 正文 1.36、画布大字 1.22（更紧）——同一份令牌既管字号也管呼吸感。
pub fn line_height(font_size: f32) -> f32 {
    (font_size * crate::design::text::line_height(font_size)).round()
}

/// CJK 汉字、中日韩标点与全角形式（TS `CJK_RE` 的字符范围）。
pub fn is_cjk(ch: char) -> bool {
    let c = ch as u32;
    (0x2E80..=0x9FFF).contains(&c)
        || (0x3000..=0x303F).contains(&c)
        || (0xFF00..=0xFFEF).contains(&c)
        || matches!(c, 0x2018 | 0x2019 | 0x201C | 0x201D | 0x2026 | 0x2014)
}

/// 行首禁则：不允许独立起行的收尾标点（docs/07 §4 第一行，字符集与旧版 `measure.ts` 的
/// `NO_START` 一致——半角 `%` 与全角 `～` 是 DWrite 清单之外、必须自己兜底的那两个）。
pub const FORBIDDEN_LINE_START: &str = "，。、；：？！）】」』》〉…·—～%”’";

/// 行尾禁则：不允许留在行尾的开首括号（docs/07 §4 第二行）。
pub const FORBIDDEN_LINE_END: &str = "（【「『《〈";

/// 收尾标点不得行首。
pub fn forbidden_line_start(ch: char) -> bool {
    FORBIDDEN_LINE_START.contains(ch)
}

/// 开首括号不得行尾。
pub fn forbidden_line_end(ch: char) -> bool {
    FORBIDDEN_LINE_END.contains(ch)
}

/// 禁则兜底：把落到行首的收尾标点拉回上一行，把挂在行尾的开首括号推到下一行。
///
/// 排字引擎（现在是 DWrite）的 kinsoku 表覆盖大半，但产品清单比它宽；而且断行只按
/// 「不超出可用宽度」判断，不看字符性质。两种情况下都是**标点溢出**比另起一行更可读，
/// 所以移动后的行宽要由调用方重新度量，不能沿用断行时的宽度。
///
/// 只做单字符的移动，因此「……」「——」这类双字符号会被拆开——它们本来也各自成 token，
/// 视觉上仍连着上一行，可以接受。
pub fn enforce_kinsoku(lines: &mut Vec<String>) {
    let mut i = 1usize;
    while i < lines.len() {
        let mut moved = false;
        while let Some(ch) = lines[i].chars().next() {
            if !forbidden_line_start(ch) {
                break;
            }
            let ch = lines[i].remove(0);
            lines[i - 1].push(ch);
            moved = true;
        }
        // 整行只剩一个标点时它已经搬到上一行，别在下面留一个空行
        if moved && lines[i].is_empty() {
            lines.remove(i);
            continue;
        }
        i += 1;
    }

    let mut i = 0usize;
    while i + 1 < lines.len() {
        let mut moved = false;
        while let Some(ch) = lines[i].chars().next_back() {
            if !forbidden_line_end(ch) {
                break;
            }
            let ch = lines[i].pop().unwrap_or(ch);
            lines[i + 1].insert(0, ch);
            moved = true;
        }
        if moved && lines[i].is_empty() {
            lines.remove(i);
            continue;
        }
        i += 1;
    }
}

/// token 类别：1 = CJK 单字，2 = 连续空白，3 = 其余连续串。
fn class(ch: char) -> u8 {
    if is_cjk(ch) {
        1
    } else if ch.is_whitespace() {
        2
    } else {
        3
    }
}

/// 分词（TS `TOKEN_RE`）：CJK 每字一 token，空白串与西文串各自成 token。
fn tokenize(para: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_class = 0u8;
    for ch in para.chars() {
        let c = class(ch);
        if c == 1 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(ch.to_string());
            continue;
        }
        if !cur.is_empty() && c != cur_class {
            out.push(std::mem::take(&mut cur));
        }
        cur_class = c;
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// 收行：丢掉行尾空格，避免折行后的空串影响宽度与居中。
fn push_line(out: &mut Vec<String>, line: &mut String) {
    while line.ends_with(' ') || line.ends_with('\t') {
        line.pop();
    }
    out.push(std::mem::take(line));
}

/// 等宽近似度量器：**测试与无字体引擎时的占位实现**。
///
/// 汉字占 1 em、其他字符 0.55 em、空格 0.28 em，只做贪心折行：不含禁则（行首标点）、
/// 不做中英混排断词、忽略粗体。数值不代表最终渲染结果，只保证布局单测可断言、可复现。
pub struct MonoMeasurer;

fn approx_advance(ch: char, font_size: f32) -> f32 {
    match class(ch) {
        1 => font_size,
        2 => font_size * 0.28,
        _ => font_size * 0.55,
    }
}

fn approx_width(s: &str, font_size: f32) -> f32 {
    s.chars().map(|c| approx_advance(c, font_size)).sum()
}

impl TextMeasurer for MonoMeasurer {
    fn measure(&self, text: &str, font_size: f32, _weight: u16, max_width: f32) -> Measured {
        let mut lines: Vec<String> = Vec::new();
        let source = if text.is_empty() { " " } else { text };
        for para in source.split('\n') {
            wrap_paragraph(para, font_size, max_width, &mut lines);
        }
        enforce_kinsoku(&mut lines);
        let width = lines
            .iter()
            .map(|l| approx_width(l, font_size))
            .fold(0.0f32, f32::max);
        Measured {
            lines,
            width,
            line_height: line_height(font_size),
        }
    }
}

fn wrap_paragraph(para: &str, font_size: f32, max_width: f32, out: &mut Vec<String>) {
    if para.is_empty() {
        out.push(String::new());
        return;
    }
    let mut line = String::new();
    let mut w = 0.0f32;
    for raw in tokenize(para) {
        // 单个 token 比整行还宽 → 逐字符断
        let parts: Vec<String> = if approx_width(&raw, font_size) > max_width {
            raw.chars().map(|c| c.to_string()).collect()
        } else {
            vec![raw]
        };
        for tk in parts {
            let tw = approx_width(&tk, font_size);
            if !line.is_empty() && w + tw > max_width {
                // 溢出的是空白 token：直接断行，空白不带到下一行
                if tk.chars().all(char::is_whitespace) {
                    push_line(out, &mut line);
                    w = 0.0;
                    continue;
                }
                push_line(out, &mut line);
                line = tk;
                w = tw;
                continue;
            }
            line.push_str(&tk);
            w += tw;
        }
    }
    if !line.is_empty() {
        push_line(out, &mut line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(text: &str, size: f32, max: f32) -> Measured {
        MonoMeasurer.measure(text, size, crate::theme::WEIGHT_REGULAR, max)
    }

    #[test]
    fn line_height_loosens_for_small_text_and_tightens_for_large() {
        // 小字按 1.42、正文 1.36、大字 1.22：行高/字号 的比值必须单调下降。
        let mut prev = f32::MAX;
        for size in [11.0f32, 12.0, 13.0, 15.0, 20.0, 28.0] {
            let ratio = line_height(size) / size;
            assert!(
                ratio <= prev + 0.02,
                "字号 {size} 的行高比 {ratio:.3} 不该比更小的字更松"
            );
            prev = ratio;
        }
        // 具体值：口径变了要显式改这里，不能悄悄漂移
        assert_eq!(line_height(14.0), 20.0);
        assert_eq!(line_height(20.0), 29.0);
        assert_eq!(line_height(28.0), 39.0);
    }

    #[test]
    fn tokenise_splits_cjk_from_latin_runs() {
        assert_eq!(tokenize("中文abc def"), vec!["中", "文", "abc", " ", "def"]);
    }

    #[test]
    fn backstop_pulls_closing_punctuation_up() {
        let mut lines = vec!["中文中文".to_string(), "，中文".to_string()];
        enforce_kinsoku(&mut lines);
        assert_eq!(lines, vec!["中文中文，", "中文"]);
    }

    #[test]
    fn backstop_pushes_opening_bracket_down() {
        let mut lines = vec!["第一段（".to_string(), "括号里的内容".to_string()];
        enforce_kinsoku(&mut lines);
        assert_eq!(lines, vec!["第一段", "（括号里的内容"]);
    }

    #[test]
    fn backstop_leaves_no_blank_line_behind() {
        let mut lines = vec!["甲".to_string(), "。".to_string(), "乙".to_string()];
        enforce_kinsoku(&mut lines);
        assert_eq!(lines, vec!["甲。", "乙"]);
    }

    #[test]
    fn backstop_keeps_real_blank_paragraphs() {
        let mut lines = vec!["甲".to_string(), String::new(), "乙".to_string()];
        enforce_kinsoku(&mut lines);
        assert_eq!(lines, vec!["甲", "", "乙"]);
    }

    #[test]
    fn placeholder_measurer_also_obey_kinsoku() {
        // 14px 每汉字 14px：宽 56 刚好排到「中文中文」，逗号必须挂上去而不是起行
        let r = m("中文中文，中文", 14.0, 56.0);
        assert_eq!(r.lines, vec!["中文中文，", "中文"]);
    }

    #[test]
    fn wraps_cjk_by_character_cells() {
        // 14px 每个汉字 14px，宽 40 → 每行至多 2 字
        let r = m("思维导图", 14.0, 40.0);
        assert_eq!(r.lines, vec!["思维", "导图"]);
        assert_eq!(r.width, 28.0);
    }

    #[test]
    fn keeps_latin_words_whole_without_trailing_space() {
        let r = m("abc defg", 10.0, 25.0);
        assert_eq!(r.lines, vec!["abc", "defg"]);
    }

    #[test]
    fn empty_text_yields_one_line_and_zero_width() {
        let r = m("", 14.0, 100.0);
        assert_eq!(r.lines.len(), 1);
        assert_eq!(r.width, 0.0);
    }

    #[test]
    fn hard_newlines_split_paragraphs() {
        let r = m("甲\n乙", 14.0, 1000.0);
        assert_eq!(r.lines, vec!["甲", "乙"]);
    }

    #[test]
    fn no_wrap_when_width_is_unbounded() {
        let r = m("很长的中文标题文本内容示例", 14.0, NO_WRAP);
        assert_eq!(r.lines.len(), 1);
    }
}
