use mw_core::design::content;
use mw_core::design::contrast;
use mw_core::theme::{get_theme, themes, Rgba, THEME_IDS};

/// 把 `fg` 按 alpha 合成到 `bg` 上，取不透明结果（WCAG 算的是最终像素，不是 alpha 值）。
fn over(bg: [u8; 3], fg: [u8; 3], a: f32) -> [u8; 3] {
    let f = |i: usize| (bg[i] as f32 * (1.0 - a) + fg[i] as f32 * a).round() as u8;
    [f(0), f(1), f(2)]
}

fn rgb(c: Rgba) -> [u8; 3] {
    [c.0[0], c.0[1], c.0[2]]
}

/// 文字可能落在的几种底：页面、面板、面板内凹槽、悬停底、画布。
fn text_backdrops(t: &mw_core::theme::Theme) -> Vec<(&'static str, [u8; 3])> {
    vec![
        ("page", rgb(t.ui.bg)),
        ("panel", rgb(t.ui.panel)),
        ("panel2", rgb(t.ui.panel2)),
        ("hover", rgb(t.ui.hover)),
        ("canvas", rgb(t.ui.canvas_bg)),
    ]
}

/// `ui.dim` 是面板里**真的当正文用**的次要文字（说明、副行、路径）。
/// 以前它在 panel2 / hover 上只有 4.31–4.36:1，摸到 AA 线下方；现在五种底全部 ≥4.5。
#[test]
fn secondary_text_clears_aa_on_every_surface() {
    let mut bad = Vec::new();
    for t in themes() {
        let dim = rgb(t.ui.dim);
        for (name, bg) in text_backdrops(&t) {
            let cr = contrast(dim, bg);
            if cr < 4.5 {
                bad.push(format!("{} {} {cr:.2}", t.id, name));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "次要文字在某个底色上不到 4.5:1（AA）：\n{}",
        bad.join("\n")
    );
}

/// 内容表面的 faint 只做占位符/禁用/装饰，但 `docs/x100/typography-plan` §D6
/// 仍要求它对 page/panel ≥ 4.5:1（旧 0.48/0.56 只有 2.6–3.7，占位符发虚）。
/// 层级要在**同一套 content alpha** 里比：faint 必须明显淡于 secondary，
/// 不能拿画布的 `ui.dim` 当参照——那是另一套调色，两者不共享底色。
#[test]
fn faint_text_stays_visible_but_never_becomes_body_text() {
    let mut bad = Vec::new();
    for t in themes() {
        let alphas = content::alphas(t.appearance);
        let text = rgb(t.ui.text);
        for (name, bg) in text_backdrops(&t) {
            let faint = over(bg, text, alphas.text_faint);
            let secondary = over(bg, text, alphas.text_secondary);
            let cr = contrast(faint, bg);
            // page/panel/画布是阅读底，占位符也得过 AA；panel2/hover 是内凹与瞬时底，
            // 只按非文本 3:1 保底（0.68 时暖砂 panel2 为 4.39）。
            let floor = if matches!(name, "panel2" | "hover") {
                3.0
            } else {
                4.5
            };
            if cr < floor {
                bad.push(format!("{} {} faint {cr:.2} < {floor}", t.id, name));
            }
            let sec_cr = contrast(secondary, bg);
            // 层级不能塌：faint 必须明显比 secondary 淡，否则两档就并成一档了。
            if cr >= sec_cr {
                bad.push(format!(
                    "{} {}: faint {cr:.2} 不比 secondary {sec_cr:.2} 淡",
                    t.id, name
                ));
            }
        }
    }
    assert!(bad.is_empty(), "faint 文字验收失败：\n{}", bad.join("\n"));
}

#[test]
fn every_theme_keeps_branch_colours_visible_on_its_canvas() {
    let mut bad = Vec::new();
    for t in themes() {
        let bg = [
            t.ui.canvas_bg.0[0],
            t.ui.canvas_bg.0[1],
            t.ui.canvas_bg.0[2],
        ];
        // 卡片底色（一级）也要算：线是从卡片边缘出发的。
        // 一级卡本身透明时（雪枝那种"无卡片"主题），线其实压在画布上。
        let card = if t.levels[1].fill.0[3] > 8 {
            [
                t.levels[1].fill.0[0],
                t.levels[1].fill.0[1],
                t.levels[1].fill.0[2],
            ]
        } else {
            bg
        };
        for c in &t.palette {
            let rgb = [c.0[0], c.0[1], c.0[2]];
            for b in [bg, card] {
                let cr = contrast(rgb, b);
                if cr < 3.0 {
                    bad.push(format!(
                        "{} #{:02x}{:02x}{:02x} vs bg {cr:.2}",
                        t.id, rgb[0], rgb[1], rgb[2]
                    ));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "分支线在底色上不够清楚（WCAG 非文本对比度 3:1）：\n{}",
        bad.join("\n")
    );
}

#[test]
fn text_levels_keep_wcag_aa_contrast() {
    let mut bad = Vec::new();
    for t in themes() {
        for (i, lvl) in t.levels.iter().enumerate() {
            let fg = [lvl.text.0[0], lvl.text.0[1], lvl.text.0[2]];
            let bg = if lvl.fill.0[3] == 0 {
                [
                    t.ui.canvas_bg.0[0],
                    t.ui.canvas_bg.0[1],
                    t.ui.canvas_bg.0[2],
                ]
            } else {
                [lvl.fill.0[0], lvl.fill.0[1], lvl.fill.0[2]]
            };
            let cr = contrast(fg, bg);
            if cr < 4.5 {
                bad.push(format!("{} 第{i}层 {cr:.2}", t.id));
            }
        }
    }
    assert!(bad.is_empty(), "文字对比度不足 4.5:1：\n{}", bad.join("\n"));
}

/// WP7 的验收口径：主题库扩容后 `THEME_IDS`、`themes()` 与 `get_theme` 三处必须同步，
/// 少一处就会出现「检查器里有、命令面板里没有」的断头路。
#[test]
fn the_theme_library_is_four_and_every_id_resolves() {
    let all = themes();
    assert_eq!(
        all.len(),
        THEME_IDS.len(),
        "themes() 与 THEME_IDS 数量不一致"
    );
    assert_eq!(all.len(), 4);
    for (theme, id) in all.iter().zip(THEME_IDS) {
        assert_eq!(theme.id, id, "themes() 的顺序必须与 THEME_IDS 一致");
        assert_eq!(get_theme(id).id, id, "get_theme 不认识 {id}");
    }
    for id in ["snow", "ink", "mist", "sand"] {
        assert!(THEME_IDS.contains(&id), "主题 {id} 没进 THEME_IDS");
    }
}

#[test]
fn ink_theme_is_aa_on_its_canvas() {
    let t = get_theme("ink");
    for lvl in &t.levels {
        let bg = if lvl.fill.0[3] == 0 {
            [
                t.ui.canvas_bg.0[0],
                t.ui.canvas_bg.0[1],
                t.ui.canvas_bg.0[2],
            ]
        } else {
            [lvl.fill.0[0], lvl.fill.0[1], lvl.fill.0[2]]
        };
        assert!(
            contrast([lvl.text.0[0], lvl.text.0[1], lvl.text.0[2]], bg) >= 4.5,
            "墨夜层级文字必须 >= 4.5"
        );
    }
    for c in &t.palette {
        let rgb = [c.0[0], c.0[1], c.0[2]];
        assert!(
            contrast(rgb, [0x17, 0x19, 0x1c]) >= 3.0,
            "墨夜分支线对比不足"
        );
    }
}

/// 深色主题的四个指定令牌必须逐项落地：画布 / 卡片 / 正文 / 强调。
#[test]
fn the_new_dark_theme_uses_the_planned_tokens() {
    let t = get_theme("ink");
    assert_eq!(t.appearance, mw_core::theme::Appearance::Dark);
    assert_eq!(t.ui.canvas_bg, Rgba::rgb(0x17, 0x19, 0x1c));
    assert_eq!(t.levels[1].fill, Rgba::rgb(0x24, 0x28, 0x2e));
    assert_eq!(t.levels[1].text, Rgba::rgb(0xf2, 0xef, 0xe9));
    assert_eq!(t.ui.accent, Rgba::rgb(0xd8, 0xa7, 0x5b));
    let body = [0xf2, 0xef, 0xe9];
    for bg in [[0x17, 0x19, 0x1c], [0x24, 0x28, 0x2e]] {
        assert!(contrast(body, bg) >= 4.5, "墨夜正文对比度不足");
    }
}
