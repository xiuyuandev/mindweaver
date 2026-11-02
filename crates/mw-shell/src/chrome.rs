//! 壳层配色：一处定义，窗口四周（顶栏 / 面板 / 弹层 / 状态栏）与 egui 全局视觉共用。
//!
//! 亮色保持 v2 基准的取值；暗色从主题令牌推导——否则墨夜 / 深林下画布是黑的、
//! 四周 chrome 还是白的，整窗像两个应用拼在一起。
//! 面板内部（大纲 / 检查器）也读这里，不再各自写死灰阶；浮层另有一档 `Tone`。

use eframe::egui::{self, Color32, FontFamily, Stroke};
use mw_core::design::shell;
use mw_core::theme::{Appearance, Rgba, Theme};

/// 壳层颜色（顶栏 / 面板 / 弹层 / 状态栏）。
#[derive(Clone, Copy)]
pub struct Chrome {
    pub panel: Color32,
    pub panel2: Color32,
    pub titlebar: Color32,
    pub sep: Color32,
    pub border: Color32,
    pub border_hot: Color32,
    /// 无框窗口最外圈那 1px：要压得住 Mica，又不能像亮色的灰边贴在暗色窗口上。
    pub edge: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub faint: Color32,
    pub hover: Color32,
    pub icon_off: Color32,
    /// 「未保存 / 已编辑」那支琥珀色：暗底上 #B46A1E 会糊掉，得提亮。
    pub warn: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
}

impl Chrome {
    /// 把 `c` 朝面板底色推：`dim=1.0` 原色，`dim<1.0` 变淡（标题栏失焦用）。
    pub fn faded(self, c: Color32, dim: f32) -> Color32 {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * (1.0 - dim)) as u8;
        Color32::from_rgb(
            m(c.r(), self.panel.r()),
            m(c.g(), self.panel.g()),
            m(c.b(), self.panel.b()),
        )
    }
}

/// 主题令牌 → egui 颜色（丢弃 alpha：这些令牌都是不透明的底色/字色）。
pub const fn c32(c: Rgba) -> Color32 {
    Color32::from_rgb(c.0[0], c.0[1], c.0[2])
}

/// 主题令牌 → egui 颜色，**保留 alpha**。
///
/// 与 `c32` 是两条不许互换的路径：`from_rgb` 会把不透明通道直接丢掉，
/// `from_rgba_unmultiplied` 会按 alpha 预乘；同一支半透明令牌走错一次，
/// 截图就差一个色阶。
pub fn c32a(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

/// `Rgba` 混合后再转 egui 颜色（主题令牌之间算色只能在这里做）。
fn mix(a: Rgba, b: Rgba, t: f32) -> Color32 {
    c32(a.mix(b, t))
}

pub fn chrome_of(theme: &Theme) -> Chrome {
    let u = &theme.ui;
    // 强调色两套外观都直接取主题令牌：暗色下它是唯一还能跳出来的那支笔。
    let accent = c32(u.accent);
    let accent_soft = c32(u.accent_soft);
    match theme.appearance {
        Appearance::Light => Chrome {
            panel: c32(shell::light::PANEL),
            panel2: c32(shell::light::PANEL2),
            titlebar: c32a(shell::light::TITLEBAR),
            sep: c32(shell::light::SEP),
            border: c32(shell::light::BORDER),
            border_hot: c32(shell::light::BORDER_HOT),
            edge: c32(shell::light::EDGE),
            text: c32(shell::light::TEXT),
            dim: c32(shell::light::DIM),
            faint: c32(shell::light::FAINT),
            hover: c32(shell::light::HOVER),
            icon_off: c32(shell::light::ICON_OFF),
            warn: c32(shell::light::WARN),
            accent,
            accent_soft,
        },
        Appearance::Dark => Chrome {
            panel: c32(u.panel),
            panel2: c32(u.panel2),
            titlebar: Color32::from_rgba_unmultiplied(
                u.panel.0[0],
                u.panel.0[1],
                u.panel.0[2],
                247,
            ),
            sep: c32(u.border),
            border: c32(u.border),
            border_hot: c32(u.border),
            edge: mix(u.border, u.dim, 0.45),
            text: c32(u.text),
            dim: c32(u.dim),
            faint: c32(shell::dark::FAINT),
            hover: c32(u.hover),
            icon_off: Color32::from_rgba_unmultiplied(
                u.dim.0[0],
                u.dim.0[1],
                u.dim.0[2],
                shell::dark::ICON_OFF_ALPHA,
            ),
            warn: c32(shell::dark::WARN),
            accent,
            accent_soft,
        },
    }
}

/// 壳层文字字阶。画布节点字号由 `mw_core::theme::LEVEL_TYPE` 决定，**不要混用**：
/// 改这里只该影响标题栏、面板、浮层、状态栏与开始页。
#[derive(Clone, Copy, Debug)]
pub struct Type;

impl Type {
    /// 角标、键帽、脚注、页签计数。
    pub const CAPTION: f32 = mw_core::design::ui::CAPTION;
    /// 次级说明、状态栏、提示句。
    pub const SMALL: f32 = mw_core::design::ui::SMALL;
    /// 面板正文、列表项、按钮默认。
    pub const BODY: f32 = mw_core::design::ui::BODY;
    /// 需要被读到的主行、输入框、菜单项。
    pub const LABEL: f32 = mw_core::design::ui::LABEL;
    /// 面板标题、节点名、开始页卡片标题。
    pub const HEADING: f32 = mw_core::design::ui::HEADING;
    /// 开始页品牌字；只给产品自己的名字用。
    pub const DISPLAY: f32 = mw_core::design::ui::DISPLAY;
}

/// 图标字形尺寸（不是文字字阶）：检查器图标、加子符号、演示条单字符、开始页徽标。
#[derive(Clone, Copy, Debug)]
pub struct Glyph;

impl Glyph {
    /// 检查器图标等自绘符号。
    pub const MEDIUM: f32 = mw_core::design::icon::GLYPH_MEDIUM;
    /// 演示控制条上的单字符按钮。
    pub const LARGE: f32 = mw_core::design::icon::GLYPH_LARGE;
}

/// 浮层配色（搜索条 / 命令面板 / 右键菜单 / 帮助）。
///
/// 卡片浮在画布上，是一块独立表面：底色走主题的 `panel`，正文走 `text` 令牌
/// 而不是面板灰阶——列表项是内容，不是标签。暗色下阴影加深、描边抬高对比，
/// 否则卡片边缘直接化进黑色画布。
#[derive(Clone, Copy)]
pub struct Tone {
    pub ch: Chrome,
    pub card: Color32,
    pub stroke: Color32,
    pub shadow: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub hairline: Color32,
    pub hover: Color32,
    pub current: Color32,
    /// 当前行文字：由强调色派生，浅底压深、深底提亮。
    pub hit: Color32,
    /// 关键词命中底色与字色（语义固定为暖黄，不跟主题走）。
    pub hit_bg: Color32,
    pub hit_ink: Color32,
    /// 禁用态图标。
    pub off: Color32,
    /// 图标悬停时的圆底。
    pub chip: Color32,
    /// 整页底色（开始页这种「没有画布铺底」的表面）。
    pub bg: Color32,
    /// 主按钮上的文字：浅色配白字，深色配近黑的字。
    pub on_accent: Color32,
    /// 主题是否深色外观（少量「淡色洗底」之类必须按外观调 alpha）。
    pub dark: bool,
}

impl Tone {
    pub fn of(theme: &Theme) -> Tone {
        let ch = chrome_of(theme);
        let u = &theme.ui;
        let dark = matches!(theme.appearance, Appearance::Dark);
        Tone {
            ch,
            card: c32(u.panel),
            stroke: if dark {
                mix(u.border, u.dim, 0.25)
            } else {
                c32(shell::tone::STROKE_LIGHT)
            },
            shadow: Color32::from_black_alpha(if dark {
                shell::tone::SHADOW_DARK_ALPHA
            } else {
                shell::tone::SHADOW_LIGHT_ALPHA
            }),
            ink: c32(u.text),
            muted: c32(u.dim),
            hairline: if dark {
                c32(u.border)
            } else {
                c32(shell::tone::HAIRLINE_LIGHT)
            },
            hover: c32(u.hover),
            current: c32(u.accent_soft),
            // 向本主题的字色靠，而不是 gamma_multiply：那支笔会连 alpha 一起乘，
            // 浅色下等于「45% 不透明的强调色」贴在 accent_soft 上，实测 (149,188,250)
            // 对 (234,242,255) 只有 1.4:1，选中行反而最看不清。
            hit: mix(u.accent, u.text, if dark { 0.35 } else { 0.55 }),
            hit_bg: if dark {
                c32(shell::tone::HIT_BG_DARK)
            } else {
                c32(shell::tone::HIT_BG_LIGHT)
            },
            hit_ink: if dark {
                c32(shell::tone::HIT_INK_DARK)
            } else {
                c32(shell::tone::HIT_INK_LIGHT)
            },
            off: if dark {
                mix(u.border, u.dim, 0.35)
            } else {
                c32(shell::tone::OFF_LIGHT)
            },
            chip: if dark {
                c32(u.panel2)
            } else {
                c32(shell::tone::CHIP_LIGHT)
            },
            bg: c32(u.bg),
            on_accent: if dark { c32(u.bg) } else { Color32::WHITE },
            dark,
        }
    }
}

/// 全局 egui 视觉：细边框、白底输入框、6px 圆角；颜色一律取自 `Chrome`。
///
/// 默认浅色主题的输入框是"深一点的白 + 深灰边"，在纯白面板上会显得脏；
/// 顶栏/面板/弹层统一成一套以后，才谈得上原生应用的整齐感。
pub fn ui_visuals_for(ch: Chrome) -> egui::Visuals {
    let mut v = egui::Visuals::light();
    // 正文色走 `widgets.*.fg_stroke`（见下），这里只补"弱文字 / 选中 / 链接"三类。
    // 默认 light() 的选中底色在半透明深灰上几乎看不见，暗色下更是完全消失。
    v.weak_text_color = Some(ch.dim);
    v.hyperlink_color = ch.accent;
    v.selection.bg_fill = ch.accent_soft;
    v.selection.stroke = Stroke::new(1.0, ch.text);
    // 输入光标与输入法下划线。默认取的是 egui light() 那支深蓝，贴在墨夜的近黑面板上
    // 等于隐形；中文输入时这根下划线是「IME 收到键了没」的唯一反馈，必须跟着字色走。
    v.text_cursor.stroke = Stroke::new(1.5, ch.text);
    v.ime_composition.active_underline_stroke = Stroke::new(1.5, ch.text);
    v.ime_composition.inactive_underline_stroke = Stroke::new(1.5, ch.dim);
    v.window_fill = ch.panel;
    v.panel_fill = ch.panel;
    v.extreme_bg_color = ch.panel;
    v.text_edit_bg_color = Some(ch.panel);
    v.faint_bg_color = ch.panel2;
    v.code_bg_color = ch.panel2;
    v.window_corner_radius = egui::CornerRadius::same(mw_core::design::radius::LG as u8);
    v.menu_corner_radius = egui::CornerRadius::same(mw_core::design::radius::PANEL as u8);
    v.window_stroke = Stroke::new(mw_core::design::stroke::HAIRLINE, ch.border);
    let sh = mw_core::design::elevation::WINDOW;
    v.window_shadow = egui::epaint::Shadow {
        offset: sh.offset,
        blur: sh.blur,
        spread: sh.spread,
        color: Color32::from_black_alpha(sh.alpha),
    };
    v.popup_shadow = v.window_shadow;
    v.slider_trailing_fill = true;

    let widgets = &mut v.widgets;
    let hairline = mw_core::design::stroke::HAIRLINE;
    let control_radius = egui::CornerRadius::same(mw_core::design::radius::CONTROL as u8);
    widgets.noninteractive.fg_stroke = Stroke::new(hairline, ch.text);
    widgets.noninteractive.bg_stroke = Stroke::new(hairline, ch.border);
    widgets.noninteractive.corner_radius = control_radius;
    // 默认按钮不再常驻灰底 + 灰框：那会让每个工具栏、每个弹窗都像 2015 年后台。
    // 平面文字按钮 + 悬停/按下底色，是 Xmind 2026 一类的克制做法。
    widgets.inactive.fg_stroke = Stroke::new(hairline, ch.text);
    widgets.inactive.bg_fill = Color32::TRANSPARENT;
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.inactive.corner_radius = control_radius;
    widgets.hovered.fg_stroke = Stroke::new(hairline, ch.text);
    widgets.hovered.bg_fill = ch.hover;
    widgets.hovered.weak_bg_fill = ch.hover;
    widgets.hovered.bg_stroke = Stroke::NONE;
    widgets.hovered.corner_radius = control_radius;
    widgets.active.fg_stroke = Stroke::new(hairline, ch.text);
    widgets.active.bg_fill = ch.accent_soft;
    widgets.active.weak_bg_fill = ch.accent_soft;
    widgets.active.bg_stroke = Stroke::NONE;
    widgets.active.corner_radius = control_radius;
    widgets.open = widgets.active;
    v
}

/// 把 `Chrome` 装进 egui 的两个主题桶（Light / Dark 各存一份 `Style`）。
///
/// 别用 `Context::set_visuals`：它只写 `options.theme()` 选中的那一个桶，而 `theme()`
/// 在系统主题还没报上来时退到 `fallback_theme = Dark`。于是启动时那一次调用把整套浅色
/// 设计写进了暗色桶，第一帧照旧用 egui 自带的 `Visuals::light()`——分隔线、输入框描边、
/// 禁用按钮底色全是默认灰，要等第一次主题同步才"忽然变好"。壳层配色由文档主题决定，
/// 与系统的深浅色无关，所以两个桶必须写同一份。
pub fn install_visuals(ctx: &egui::Context, ch: Chrome) {
    let v = ui_visuals_for(ch);
    ctx.set_visuals_of(egui::Theme::Light, v.clone());
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.all_styles_mut(|s| {
        let gap = mw_core::design::space::SM;
        let pad = mw_core::design::control::BUTTON_PAD;
        s.spacing.item_spacing = egui::vec2(gap, gap);
        s.spacing.button_padding = egui::vec2(pad[0], pad[1]);
        s.spacing.interact_size.y = mw_core::design::control::INPUT;
        s.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::proportional(Type::BODY),
        );
        s.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::proportional(Type::BODY),
        );
        s.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional(Type::SMALL),
        );
        s.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(
                Type::HEADING,
                FontFamily::Name(crate::fonts::SEMIBOLD.into()),
            ),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::theme::themes;

    fn luma(c: Color32) -> f32 {
        let lin = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (hi, lo) = if luma(a) >= luma(b) {
            (luma(a), luma(b))
        } else {
            (luma(b), luma(a))
        };
        (hi + 0.05) / (lo + 0.05)
    }

    /// 浮层里「当前行」是键盘导航的唯一指示，墨水糊掉就等于没有选中态。
    /// 之前用 `accent.gamma_multiply(0.45)` 派生，把 alpha 一起乘了，
    /// 素白下实测 1.4:1 —— 这一条把四套主题的底线钉住。
    #[test]
    fn the_selected_row_ink_stays_legible() {
        for theme in themes() {
            let tone = Tone::of(&theme);
            assert_eq!(tone.hit.a(), 255, "{}：选中行墨水必须不透明", theme.name);
            let ratio = contrast(tone.hit, tone.current);
            assert!(
                ratio >= 4.5,
                "{}：{:?} 压在 {:?} 上只有 {ratio:.1}:1",
                theme.name,
                tone.hit,
                tone.current
            );
        }
    }

    /// 自定义强调色转过的主题也得守住同一条线：色条上随便点一格，选中行都不能糊。
    #[test]
    fn a_spun_accent_keeps_the_selected_row_legible() {
        for theme in themes() {
            for step in (0..360).step_by(15) {
                let spun = theme.with_accent(Some(theme.accent_at_hue(step as f32)));
                let tone = Tone::of(&spun);
                assert_eq!(tone.hit.a(), 255, "{} {step}°：墨水不能被转透", theme.name);
                let ratio = contrast(tone.hit, tone.current);
                assert!(
                    ratio >= 4.5,
                    "{} 转到 {step}° 后选中行只剩 {ratio:.1}:1（{:?} 压在 {:?} 上）",
                    theme.name,
                    tone.hit,
                    tone.current
                );
            }
        }
    }

    /// egui 0.37 的 Style 按 Light / Dark 分两桶存，`Context::set_visuals` 只写
    /// `options.theme()` 选中的那一桶；系统主题还没报上来时它退到 `fallback_theme = Dark`。
    /// 于是启动那一次调用把整套浅色写进了没人读的暗色桶，第一帧仍是 egui 自带的默认灰，
    /// 要等第一次主题同步才「忽然变好」——真实输入截图实测：同一条分隔线 CECECE → EDEFF2。
    /// 壳层配色由文档主题决定，与系统深浅色无关，所以两桶必须是同一份。
    #[test]
    fn the_designed_visuals_land_in_both_theme_buckets() {
        let ctx = egui::Context::default();
        let ch = chrome_of(&mw_core::get_theme("snow"));
        install_visuals(&ctx, ch);
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let v = &ctx.style_of(theme).visuals;
            assert_eq!(v.panel_fill, ch.panel, "{theme:?} 桶里的面板色不是设计值");
            assert_eq!(
                v.widgets.noninteractive.bg_stroke.color, ch.border,
                "{theme:?} 桶里的分隔线还是 egui 默认灰"
            );
            assert_eq!(
                v.widgets.inactive.weak_bg_fill,
                Color32::TRANSPARENT,
                "{theme:?} 桶里的默认按钮应是无底平面按钮"
            );
            assert_eq!(
                v.widgets.inactive.bg_stroke,
                Stroke::NONE,
                "{theme:?} 桶里的默认按钮不该常驻灰框"
            );
        }
    }

    /// 六档必须严格递增：合并档位时最容易出现「两个名字同一个值」或顺序写反，
    /// 结果是一处标题看起来像正文、另一处永远读不出来。
    #[test]
    fn the_chrome_type_scale_has_six_monotonic_tiers() {
        let tiers = [
            Type::CAPTION,
            Type::SMALL,
            Type::BODY,
            Type::LABEL,
            Type::HEADING,
            Type::DISPLAY,
        ];
        assert_eq!(tiers.len(), 6);
        assert!(
            tiers.windows(2).all(|w| w[0] < w[1]),
            "chrome 字阶必须严格递增：{tiers:?}"
        );
    }

    /// `Type` 只是 `design::ui` 的别名：两处一旦漂移，改令牌就会「只生效一半」。
    #[test]
    fn chrome_type_is_the_core_ui_scale() {
        assert_eq!(
            [
                Type::CAPTION,
                Type::SMALL,
                Type::BODY,
                Type::LABEL,
                Type::HEADING,
                Type::DISPLAY,
            ],
            [
                mw_core::design::ui::CAPTION,
                mw_core::design::ui::SMALL,
                mw_core::design::ui::BODY,
                mw_core::design::ui::LABEL,
                mw_core::design::ui::HEADING,
                mw_core::design::ui::DISPLAY,
            ]
        );
    }

    fn collect_rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rust_sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// 壳层源码禁止写裸字号：字号只能来自 `Type` / `design`。这条测试是
    /// X11 令牌收敛的防回退门，新增文件同样受约束。
    ///
    /// 允许两条例外，都在这里写清楚理由：
    /// - `note_view.rs` 的 `FontId::new(0.1, …)` 不是文字字号，是把 Markdown 标记缩到
    ///   不可见、同时保留字符位置的版面哨兵；
    /// - 整数裸字号（如 `.size(22.0)`）仍允许，因为内容字阶里确实存在 22/17 这类档位；
    ///   本门只拦「0.5px 时代」的小数字号与假粗体。
    #[test]
    fn shell_sources_do_not_hardcode_font_sizes() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rust_sources(&src, &mut files);
        assert!(files.len() > 20, "源码扫描没找到文件，路径可能不对");
        let mut offenders = Vec::new();
        // 拼出来是为了让本文件自己不被扫到——这条测试的源码里不能出现那个方法名。
        let fake_bold = concat!(".", "strong()");
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            for (line_no, line) in text.lines().enumerate() {
                // 文档注释可以讨论字号；只查真正会编译的代码行。
                if line.trim_start().starts_with("//") {
                    continue;
                }
                if line.contains(fake_bold) {
                    offenders.push(format!("{}:{} 假粗体", file.display(), line_no + 1));
                }
                for needle in [
                    "FontId::proportional(",
                    "FontId::monospace(",
                    "FontId::new(",
                    ".size(",
                ] {
                    let mut rest = line;
                    while let Some(pos) = rest.find(needle) {
                        let tail = rest[pos + needle.len()..].trim_start();
                        let literal: String = tail
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '.')
                            .collect();
                        // 版面哨兵：0.1px 用来藏标记字符，不是可读字号。
                        let sentinel = needle == "FontId::new(" && literal == "0.1";
                        let fractional = literal
                            .split_once('.')
                            .is_some_and(|(_, f)| f.chars().any(|c| c != '0'));
                        if fractional && !sentinel {
                            offenders.push(format!(
                                "{}:{} 裸小数字号 {literal}",
                                file.display(),
                                line_no + 1
                            ));
                        }
                        rest = &rest[pos + needle.len()..];
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "字号必须走 Type/design；禁止裸小数字号与假粗体：{offenders:?}"
        );
    }

    /// `c32` 丢 alpha、`c32a` 保 alpha：两条路径互换一次就差一个色阶，
    /// T1 搬令牌时最容易踩的就是这一处。
    #[test]
    fn c32a_keeps_alpha() {
        let opaque = Rgba::rgb(0x3E, 0x44, 0x4C);
        assert_eq!(c32a(opaque), Color32::from_rgb(0x3E, 0x44, 0x4C));

        let translucent = Rgba::rgba(255, 255, 255, 247);
        assert_eq!(
            c32a(translucent),
            Color32::from_rgba_unmultiplied(255, 255, 255, 247)
        );
        assert_eq!(c32a(translucent).a(), 247);
        // 同一支令牌走 c32 会被抹成不透明——这正是两条路径不许互换的理由。
        assert_eq!(c32(translucent).a(), 255);
        assert_ne!(c32(translucent), c32a(translucent));
    }
}
