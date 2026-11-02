//! CER-01 证据 dump：内容表面令牌的四主题数值表。
//!
//! `cargo run -p mw-core --example content_tokens > shots/x100/CER-01/tokens.txt`

use mw_core::design::content;
use mw_core::theme::{themes, Rgba, Theme};

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0[0], c.0[1], c.0[2])
}

fn pct(alpha: u8) -> String {
    format!("{:.0}%", alpha as f32 / 255.0 * 100.0)
}

fn contrast(fg: Rgba, bg: Rgba) -> String {
    format!("{:.2}", content::composite(fg, bg).contrast_ratio(bg))
}

fn main() {
    println!("# CER-01 内容表面令牌 dump");
    println!();
    println!("生成命令：`cargo run -p mw-core --example content_tokens`");
    println!();

    println!("## 1. 内容字阶（docs/28 §4.1）");
    println!();
    println!("| 角色 | 字号 | 行高 | 比例 | 字重 |");
    println!("| --- | ---: | ---: | ---: | ---: |");
    for (name, style) in [
        ("页面标题 H1 / PAGE_TITLE", content::PAGE_TITLE),
        ("大纲页标题 / OUTLINE_TITLE", content::OUTLINE_TITLE),
        ("一级标题 H2 / HEADING_1", content::HEADING_1),
        ("二级标题 H3 / HEADING_2", content::HEADING_2),
        ("三级标题 H4 / HEADING_3", content::HEADING_3),
        ("大纲行 / OUTLINE_ROW", content::OUTLINE_ROW),
        ("阅读正文 / READING_BODY", content::READING_BODY),
        ("列表标题 / LIST_TITLE", content::LIST_TITLE),
        ("列表摘要 / LIST_EXCERPT", content::LIST_EXCERPT),
        ("列表元信息 / LIST_META", content::LIST_META),
        ("行内说明/导航 / NAV", content::NAV),
        ("辅助/元信息 / META", content::META),
        ("小标签/角标 / TAG", content::TAG),
        ("代码/等宽 / CODE", content::CODE),
    ] {
        println!(
            "| {name} | {:.1} | {:.1} | {:.2} | {} |",
            style.size,
            style.line_height,
            style.ratio(),
            style.weight
        );
    }
    println!();

    println!("## 2. 版心（docs/28 §4.2）");
    println!();
    println!("| 表面 | page_max | text_max | inset | 顶/底留白 | 窄窗左右 |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: |");
    for (name, m) in [
        ("大纲 OUTLINE", content::OUTLINE),
        ("卡片页 CARD", content::CARD),
        ("库预览 PREVIEW", content::PREVIEW),
    ] {
        println!(
            "| {name} | {:.0} | {:.0} | {:.0} | {:.0} / {:.0} | {:.0} |",
            m.page_max,
            m.text_max,
            m.page_inset(),
            m.pad_top,
            m.pad_bottom,
            m.narrow_side
        );
    }
    println!();

    println!("## 3. 版心实算（宽窗 1000 / 窄窗 700）");
    println!();
    println!("| 表面 | 可用宽 | side | page | text | inset |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: |");
    for (name, m) in [
        ("大纲", content::OUTLINE),
        ("卡片页", content::CARD),
        ("库预览", content::PREVIEW),
    ] {
        for available in [1000.0, 700.0] {
            let c = m.column(available);
            println!(
                "| {name} | {:.0} | {:.1} | {:.1} | {:.1} | {:.1} |",
                available, c.side, c.page, c.text, c.inset
            );
        }
    }
    println!();

    println!("## 4. 四主题语义色");
    println!();
    println!("| 主题 | 外观 | page | panel | primary | secondary | faint | link | 正文/page | 次要/page | 链接/page | 强调色/page |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |");
    for theme in themes() {
        let c = content::ContentColors::for_theme(&theme);
        println!(
            "| {} {} | {:?} | {} | {} | {} | {} {} | {} {} | {} | {} | {} | {} | {} |",
            theme.id,
            theme.name,
            theme.appearance,
            hex(c.page),
            hex(c.panel),
            hex(c.text_primary),
            hex(c.text_secondary),
            pct(c.text_secondary.0[3]),
            hex(c.text_faint),
            pct(c.text_faint.0[3]),
            hex(c.link),
            contrast(c.text_primary, c.page),
            contrast(c.text_secondary, c.page),
            contrast(c.link, c.page),
            contrast(c.accent, c.page),
        );
    }
    println!();

    println!("## 5. 状态透明度（docs/28 §4.3）");
    println!();
    println!("| 主题 | hover | selected | navSelected | editing | hairline | divider |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: |");
    for theme in themes() {
        let c = content::ContentColors::for_theme(&theme);
        println!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            theme.id,
            pct(c.hover.0[3]),
            pct(c.selected.0[3]),
            pct(c.nav_selected.0[3]),
            pct(c.editing.0[3]),
            pct(c.hairline.0[3]),
            pct(c.divider.0[3]),
        );
    }
    println!();

    println!("## 6. DPI 与滚动条（content_metrics）");
    println!();
    println!(
        "| DPI | hairline (point) | hairline (物理 px) | focus ring | scrollbar 宽/拇指/圆角 |"
    );
    println!("| ---: | ---: | ---: | ---: | ---: |");
    let theme: Theme = themes().remove(0);
    for dpi in [1.0, 1.25, 1.5, 2.0] {
        let m = content::content_metrics(&theme, dpi);
        println!(
            "| {:.2} | {:.4} | {:.1} | {:.1} | {:.0} / {:.0} / {:.0} |",
            m.dpi,
            m.hairline,
            m.hairline * m.dpi,
            m.focus_ring,
            m.scrollbar.width,
            m.scrollbar.thumb,
            m.scrollbar.radius
        );
    }
    println!();

    println!("## 7. 标签点（6px，主题分支色）");
    println!();
    println!("| 主题 | 1 | 2 | 3 | 4 | 5 | 6 |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for theme in themes() {
        let c = content::ContentColors::for_theme(&theme);
        let dots: Vec<String> = c.tag_dots.iter().map(|d| hex(*d)).collect();
        println!("| {} | {} |", theme.id, dots.join(" | "));
    }
}
