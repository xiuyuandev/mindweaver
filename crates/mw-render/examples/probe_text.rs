//! 打印 DWrite 的**原始**行度量与折行结果，用来把测试用例写在事实上而不是猜测上。
//! 运行：`cargo run -p mw-render --example probe_text`

use mw_core::text::{line_height, NO_WRAP};
use mw_render::dwrite::{utf16_units, DWrite, DWriteMeasurer};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_WRAP,
};

const SIZE: f32 = 14.0;

fn main() {
    let Some(dw) = DWrite::new() else {
        println!("本机没有 DirectWrite");
        return;
    };
    println!(
        "字族 = {}，字号 = {SIZE}，line_height = {}",
        dw.family(),
        line_height(SIZE)
    );

    println!("\n## 原始行度量（max = 56）");
    for text in [
        "甲\n乙",
        "甲\n",
        "甲\n\n",
        "\n甲",
        "中文中文 ",
        "中文  中文",
        "破折号——后面还有字",
        "中文中文，中文",
    ] {
        dump(&dw, text, 56.0);
    }

    println!("\n## 度量器最终结果（max = 56）");
    let m = DWriteMeasurer::new(dw);
    for text in [
        "甲\n乙",
        "甲\n",
        "甲\n\n",
        "中文中文，中文",
        "他说：「你好吗？」她回答：「很好。」",
        "第一段（测试）第二段【补充】",
        "使用 Rust 编写布局引擎",
        "破折号——后面还有字",
        "完成度 50%，进度正常",
    ] {
        let r = m
            .measure_result(text, SIZE, mw_core::theme::WEIGHT_REGULAR, 56.0)
            .unwrap();
        println!(
            "{text:?} → w={:7.3} {}",
            r.width,
            r.lines
                .iter()
                .map(|l| format!("{l:?}"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }

    println!("\n## 单行宽度参照（NO_WRAP）");
    for text in [
        "中", "一", "a", "ab", "abc", "Rust", "0", "%", "，", "～", "——", " ", "  ",
    ] {
        let r = m
            .measure_result(text, SIZE, mw_core::theme::WEIGHT_REGULAR, NO_WRAP)
            .unwrap();
        println!("{text:?} → w={:7.3} lines={:?}", r.width, r.lines);
    }
}

/// 直接用 DWrite 走一遍，把每一行的四个长度字段原样打出来。
fn dump(dw: &DWrite, text: &str, max: f32) {
    let format = dw.format(SIZE, mw_core::theme::WEIGHT_REGULAR).unwrap();
    let units = utf16_units(text);
    let layout = unsafe {
        dw.factory()
            .CreateTextLayout(&units, &format, max, f32::MAX)
            .unwrap()
    };
    unsafe { layout.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP).unwrap() };
    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe { layout.GetMetrics(&mut metrics).unwrap() };
    let mut rows = vec![DWRITE_LINE_METRICS::default(); metrics.lineCount as usize];
    let mut actual = 0u32;
    unsafe { layout.GetLineMetrics(Some(&mut rows), &mut actual).unwrap() };
    println!(
        "{text:?}  lineCount={} actual={} width={:.3} inclTrailing={:.3}",
        metrics.lineCount, actual, metrics.width, metrics.widthIncludingTrailingWhitespace
    );
    for (i, row) in rows.iter().take(actual as usize).enumerate() {
        println!(
            "   [{i}] length={} trailing={} newline={} height={:.2} baseline={:.2} trimmed={}",
            row.length,
            row.trailingWhitespaceLength,
            row.newlineLength,
            row.height,
            row.baseline,
            row.isTrimmed.0
        );
    }
}
