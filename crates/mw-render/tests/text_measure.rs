//! M1b-2a 的验收：中文禁则断行 + 真实字体宽度（痛点②「换行禁则/标点错误」）。
//!
//! 这些用例**必须真跑**：本机没有 DirectWrite 就直接失败，不许静默回落到 `MonoMeasurer`
//! 然后报一片绿。期望值全部来自 `cargo run -p mw-render --example probe_text` 的实测输出。

use mw_core::text::{Measured, MonoMeasurer, TextMeasurer, NO_WRAP};
use mw_render::dwrite::{DWrite, DWriteMeasurer};

const SIZE: f32 = 14.0;
/// 4 个全角字刚好 56，留 2px 余量：第 5 个字符必定落在断行边界上。
const FIT_FOUR: f32 = 58.0;
/// 5 个全角字 70，留 2px：断行点落在第 5 个字符之后。
const FIT_FIVE: f32 = 72.0;

fn measurer() -> DWriteMeasurer {
    let Some(dw) = DWrite::new() else {
        panic!("本机没有 DirectWrite：禁则用例不能靠近似度量蒙过去");
    };
    assert_eq!(
        dw.family(),
        "Segoe UI",
        "字族候选第一个就该命中（拉丁/数字走 Segoe，中文由 DWrite 按脚本回退到雅黑）"
    );
    DWriteMeasurer::new(dw)
}

fn measure(m: &DWriteMeasurer, text: &str, max: f32) -> Measured {
    m.measure_result(text, SIZE, mw_core::theme::WEIGHT_REGULAR, max)
        .unwrap_or_else(|e| panic!("{text:?} 度量失败：{e}"))
}

/// 行首禁则：清单里的每个字符都不允许独立起行。用例按清单自动生成，加字符就加覆盖。
#[test]
fn no_start_punctuation_never_begins_a_line() {
    let m = measurer();
    for ch in mw_core::text::FORBIDDEN_LINE_START.chars() {
        let text = format!("一二三四{ch}五六七八");
        let r = measure(&m, &text, FIT_FOUR);
        for line in r.lines.iter().skip(1) {
            assert!(
                !line.starts_with(ch),
                "{text:?} @ {FIT_FOUR} → {:?}：{ch:?} 是行首禁则字符，不能起行",
                r.lines
            );
        }
        assert_eq!(r.lines.concat(), text, "{text:?}：兜底搬运不许吞字或重复");
    }
}

/// 行尾禁则：开首括号不许留在行尾。
#[test]
fn no_end_bracket_never_ends_a_line() {
    let m = measurer();
    for ch in mw_core::text::FORBIDDEN_LINE_END.chars() {
        let text = format!("一二三四{ch}五六七八");
        let r = measure(&m, &text, FIT_FIVE);
        let last = r.lines.len().saturating_sub(1);
        for (i, line) in r.lines.iter().enumerate() {
            if i == last {
                continue;
            }
            assert!(
                !line.ends_with(ch),
                "{text:?} @ {FIT_FIVE} → {:?}：{ch:?} 是行尾禁则字符，不能挂在行尾",
                r.lines
            );
        }
        assert_eq!(r.lines.concat(), text, "{text:?}：兜底搬运不许吞字或重复");
    }
}

/// 整篇真实文案：任何一行都不许踩两条禁则，且一字不丢。
#[test]
fn prose_corpus_keeps_every_line_legal() {
    let m = measurer();
    let corpus = [
        "今天天气很好，我们出去走走。",
        "他说：「你好吗？」她回答：「很好。」",
        "使用 Rust 编写布局引擎",
        "完成度 50%，进度正常",
        "价格 100元～200元",
        "这是省略号……后面还有",
        "破折号——后面还有字",
        "「引号」和『双层引号』",
        "（括号里的内容）后面还有字",
        "第一段（测试）第二段【补充】",
        "思维导图《入门》——第三版说明",
        "结论：见附录（A）与备注·注意",
    ];
    for width in [FIT_FOUR, FIT_FIVE, 96.0] {
        for text in corpus {
            let r = measure(&m, text, width);
            let last = r.lines.len().saturating_sub(1);
            for (i, line) in r.lines.iter().enumerate() {
                if i > 0 {
                    if let Some(ch) = line.chars().next() {
                        assert!(
                            !mw_core::text::forbidden_line_start(ch),
                            "{text:?} @ {width} → {:?}：行首 {ch:?}",
                            r.lines
                        );
                    }
                }
                if i != last {
                    if let Some(ch) = line.chars().next_back() {
                        assert!(
                            !mw_core::text::forbidden_line_end(ch),
                            "{text:?} @ {width} → {:?}：行尾 {ch:?}",
                            r.lines
                        );
                    }
                }
            }
            // 断行点上的空白会被整段吃掉，行中间的空白照留——比对去掉全部空白后的正文，
            // 既不放过丢字/重复字，也不苛求边界空白的取舍。
            let squash = |s: &str| -> String { s.chars().filter(|c| !c.is_whitespace()).collect() };
            assert_eq!(
                squash(&r.lines.concat()),
                squash(text),
                "{text:?} @ {width}：兜底与重折不许吞字或重复"
            );
        }
    }
}

#[test]
fn hanging_dash_refills_the_rest_of_the_paragraph() {
    // DWrite 自己会把「——」放行首；兜底挂回上一行后本行只剩「后」，
    // 重折一趟才得到浏览器的结果：悬挂标点 + 剩余内容重新填满。
    let m = measurer();
    // 56 是旧栈（雅黑优先）的精确 4 全角宽；Segoe 在前时累积宽会带一点浮点余量，
    // 留 2px 后仍是「4 字一行」的同一断点，测的是重折行为而不是卡边界。
    let r = measure(&m, "破折号——后面还有字", 58.0);
    assert_eq!(r.lines, ["破折号——", "后面还有", "字"]);
    assert!(
        r.width > 58.0,
        "标点悬挂本来就是允许溢出的：宽度要跟着挂出去的行走，实测 {}",
        r.width
    );
}

#[test]
fn hard_newlines_are_paragraph_breaks() {
    let m = measurer();
    assert_eq!(measure(&m, "甲\n乙", 1000.0).lines, ["甲", "乙"]);
    assert_eq!(measure(&m, "\n甲", 1000.0).lines, ["", "甲"]);
    // 尾部换行 DWrite 如实报出空行，光标那一行的盒高要留着
    assert_eq!(measure(&m, "甲\n", 1000.0).lines, ["甲", ""]);
    assert_eq!(measure(&m, "甲\n\n", 1000.0).lines, ["甲", "", ""]);
}

#[test]
fn empty_text_is_one_blank_line() {
    let m = measurer();
    let r = measure(&m, "", 1000.0);
    assert_eq!(r.lines, [""]);
    assert_eq!(r.width, 0.0);
    assert_eq!(r.line_height, 20.0);
}

#[test]
fn trailing_whitespace_is_trimmed_from_measured_width() {
    let m = measurer();
    let r = measure(&m, "中文中文 ", NO_WRAP);
    assert_eq!(r.lines, ["中文中文"]);
    assert_eq!(r.width, 56.0, "行尾空格不计入盒宽");
}

#[test]
fn unbounded_text_stays_on_one_line() {
    let m = measurer();
    let r = measure(&m, "很长的中文标题文本内容示例", NO_WRAP);
    assert_eq!(r.lines.len(), 1);
    assert_eq!(r.width, 182.0);
}

/// 宽度必须来自真实字体：汉字由 DWrite 按脚本回退到雅黑，步进量正好 1 em；
/// 拉丁走 Segoe UI，宽度与雅黑/近似占位都不同。这条是「度量器接到系统字体上」的硬证据。
#[test]
fn widths_come_from_real_font_metrics() {
    let m = measurer();
    assert_eq!(measure(&m, "中", NO_WRAP).width, SIZE);
    assert_eq!(measure(&m, "一二", NO_WRAP).width, SIZE * 2.0);
    let latin = measure(&m, "Rust", NO_WRAP).width;
    assert!(
        (latin - 26.981).abs() < 0.01,
        "拉丁串按 Segoe UI 真实度量，实测 {latin}"
    );
    assert!(
        measure(&m, "0", NO_WRAP).width < SIZE,
        "半角数字必须比全角字窄"
    );
    assert_ne!(
        latin,
        4.0 * SIZE * 0.55,
        "0.55 em 是 MonoMeasurer 的近似值，不该出现在真实度量里"
    );
}

#[test]
fn bold_selects_the_bold_face() {
    let dw = DWrite::new().expect("本机没有 DirectWrite");
    let m = DWriteMeasurer::new(dw);
    let normal = m
        .measure_result(
            "International",
            SIZE,
            mw_core::theme::WEIGHT_REGULAR,
            NO_WRAP,
        )
        .unwrap();
    let bold = m
        .measure_result(
            "International",
            SIZE,
            mw_core::theme::WEIGHT_BOLD_ALIAS,
            NO_WRAP,
        )
        .unwrap();
    assert!(
        bold.width > normal.width,
        "粗体没换到字面：normal={} bold={}",
        normal.width,
        bold.width
    );
}

#[test]
fn measurement_is_deterministic_and_cached() {
    let m = measurer();
    let text = "思维导图与中文排版";
    let a = measure(&m, text, FIT_FOUR);
    let b = measure(&m, text, FIT_FOUR);
    assert_eq!(
        a, b,
        "同一输入两次度量结果必须逐字节相同（快照比对依赖这点）"
    );
    // 换一张空缓存再量一次：命中与未命中两条路径必须给同一个结果
    let fresh = measurer();
    assert_eq!(measure(&fresh, text, FIT_FOUR), a);
}

/// 真实字体与近似占位的差别要一直看得见，否则「接到了 DWrite」这件事会悄悄退化。
#[test]
fn real_metrics_differ_from_the_placeholder() {
    let m = measurer();
    let real = measure(&m, "思维导图 rendering 引擎与中文排版", FIT_FOUR);
    let fake = MonoMeasurer.measure(
        "思维导图 rendering 引擎与中文排版",
        SIZE,
        mw_core::theme::WEIGHT_REGULAR,
        FIT_FOUR,
    );
    // 宽度可能巧合相等（最宽行正好是 4 个全角字），断行位置才是真实度量的硬证据。
    assert_ne!(real.lines, fake.lines, "两者断行一致说明真实度量没生效");
}
