//! 设计系统：**唯一一份**间距 / 字阶 / 圆角 / 动效 / 语义色令牌。
//!
//! 立项理由：v8–v36 的字号、内距、圆角散落在十来个文件里各写各的，改一处不生效，
//! 层级感全靠"这次调大一点"。这里把它们收敛成常量 + 少量校验函数，
//! 让"看起来一致"变成可断言的事实，而不是靠记忆。
//!
//! 三条硬约束（都有单测）：
//! 1. 间距全部落在 4px 网格上；
//! 2. 字阶单调递增，行高按档位给（小字更松、大字更紧）；
//! 3. 默认主题的正文/次要文字对比度都 >= 4.5:1。

/// 间距：4px 网格。名字即用途，别再用裸数字。
pub mod space {
    /// 图标与文字之间、标签内边距的一半
    pub const XS: f32 = 4.0;
    /// 行内元素之间
    pub const SM: f32 = 8.0;
    /// 分组内部
    pub const MD: f32 = 12.0;
    /// 面板内边距
    pub const LG: f32 = 16.0;
    /// 分组之间
    pub const XL: f32 = 24.0;
    /// 大区块之间
    pub const XXL: f32 = 32.0;
}

/// 字阶：字号 + 行高。行高是**倍数**，渲染时乘字号。
pub mod text {
    /// 最小号：角标、计数
    pub const CAPTION: f32 = 11.0;
    /// 次要文字：面板说明、路径
    pub const SMALL: f32 = 12.0;
    /// 正文：面板条目、备注
    pub const BODY: f32 = 13.0;
    /// 强调正文：面板标题、按钮
    pub const LABEL: f32 = 15.0;
    /// 画布二级
    pub const CANVAS_SUB: f32 = 16.0;
    /// 画布最深层（三级及以下）
    ///
    /// 它必须比二级**略小**又不能小于 14，否则中英混排会挤。以前这个值直接裸写在
    /// `theme::LEVEL_TYPE` 里，字号阶梯就有一档没有令牌、也改不动。
    pub const CANVAS_LEAF: f32 = 15.0;
    /// 画布一级
    pub const CANVAS_MAIN: f32 = 20.0;
    /// 画布中心
    pub const CANVAS_CENTER: f32 = 28.0;
    /// 概念图普通概念（`docs/30` §5.3 的 14/20）。
    ///
    /// 概念图不走思维导图的四档层级：它只有「主概念 / 其余」两档。以前这两个值
    /// 是 `theme.rs` 里 `apply_concept_defaults` 的局部常量，属于 X25 G-25-07
    /// 「概念默认值散在第二处」——现在与画布字阶放在同一张表里，改档只改这里。
    pub const CONCEPT: f32 = 14.0;
    /// 概念图主概念（15/22）。
    pub const CONCEPT_PRIMARY: f32 = 15.0;
    /// 大标题：开始页
    pub const TITLE: f32 = 24.0;

    /// 画布层级字阶的**合同表**（X22 G01 × X25 G-25-07）：
    /// 字号 / 字重 / 行高倍数 / 内距，按中心 → 一级 → 二级 → 三级排序。
    ///
    /// 表里的行高倍数与内距**不另存数值**，而是由 [`line_height`] 与
    /// [`super::card_padding`] 现算——这样「合同表」与布局公式永远同一份，
    /// 不会出现「表上写 1.44、代码里是 1.45」这类对不上的账。
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct CanvasLevel {
        pub size: f32,
        pub weight: u16,
        /// 行高**倍数**（渲染时乘字号；DirectWrite 的实际行盒另由字体度量决定）。
        pub line_height: f32,
        /// `(水平, 垂直)` 内距，单位 DIP。
        pub pad: (f32, f32),
    }

    impl CanvasLevel {
        const fn new(size: f32, weight: u16) -> Self {
            Self {
                size,
                weight,
                line_height: line_height(size),
                pad: super::card_padding(size),
            }
        }

        /// 单行文字的合同行高（DIP）。只作验收对账用，不替代 DirectWrite 的实测行盒。
        pub const fn absolute_line_height(self) -> f32 {
            self.size * self.line_height
        }

        /// 单行卡片的合同盒高（DIP）：行高按整数像素取整，再加上下内距。
        ///
        /// 这与 `scene::text_height` 同一条公式，但这里用的是**合同行高**；
        /// 真正的 DirectWrite 行盒由字体度量给出，验收时以实测为准（X25 R-3）。
        pub fn single_line_box_height(self) -> f32 {
            self.absolute_line_height().round() + 2.0 * self.pad.1
        }
    }

    /// 四级画布层级：中心 28/600、一级 20/400、二级 16/400、三级 15/400。
    pub const CANVAS_LEVELS: [CanvasLevel; 4] = [
        CanvasLevel::new(CANVAS_CENTER, WEIGHT_SEMIBOLD),
        CanvasLevel::new(CANVAS_MAIN, WEIGHT_REGULAR),
        CanvasLevel::new(CANVAS_SUB, WEIGHT_REGULAR),
        CanvasLevel::new(CANVAS_LEAF, WEIGHT_REGULAR),
    ];

    /// 行高倍数：字越小越松，阅读才不挤。
    pub const fn line_height(size: f32) -> f32 {
        // 基准来自 XMind 2026 实测：20px 一级主题卡高约 46.7 CSS px，反推行高 ≈1.44。
        // 大字不再压紧（CJK 压紧会糊），小字略放松，整体只做轻微分档。
        if size <= 12.0 {
            1.5
        } else if size <= 15.0 {
            1.46
        } else if size <= 21.0 {
            1.44
        } else {
            1.4
        }
    }

    /// 字重档：常规 / 中等 / 半粗。
    pub const WEIGHT_REGULAR: u16 = 400;
    pub const WEIGHT_MEDIUM: u16 = 500;
    pub const WEIGHT_SEMIBOLD: u16 = 600;
}

/// 壳层字阶：标题栏、面板、浮层、状态栏与开始页。
///
/// 与画布的 `CANVAS_*` **是两条独立字阶**：画布字号随导图层级走，壳层字号随界面层级走。
/// 两者共用同一套行高规则，但不许互相借值——把面板标题改成 20px 不会让一级主题变大。
/// `chrome::Type` 只是这里的别名，不要再在壳层里写第二份数字。
pub mod ui {
    /// 角标、键帽、脚注、页签计数。
    pub const CAPTION: f32 = 12.0;
    /// 次级说明、状态栏、提示句。
    pub const SMALL: f32 = 13.0;
    /// 面板正文、列表项、按钮默认。
    pub const BODY: f32 = 14.0;
    /// 需要被读到的主行、输入框、菜单项。
    pub const LABEL: f32 = 15.0;
    /// 面板标题、节点名、开始页卡片标题。
    pub const HEADING: f32 = 16.0;
    /// 开始页品牌字；只给产品自己的名字用。
    pub const DISPLAY: f32 = 26.0;

    /// 壳层行高与画布共用同一分档，避免「面板正文比画布正文更挤」。
    pub const fn line_height(size: f32) -> f32 {
        super::text::line_height(size)
    }
}

/// 内容表面设计系统：全屏大纲、卡片笔记页、笔记库共用的第三条字阶。
///
/// 与 `text`（画布）、`ui`（外壳）并列：内容正文不得退回 chrome 的 13px，
/// 也不得在 14/15/16 之间「差不多」。版心、状态透明度、圆角、动效与滚动条
/// 同样只在这里定义一次，组件与页面直接消费，禁止各自写第二份数字。
pub mod content {
    use crate::theme::{Appearance, Rgba, Theme};

    /// 一个内容文字角色：字号、行高（绝对值，非倍数）、字重。
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct TypeStyle {
        pub size: f32,
        pub line_height: f32,
        pub weight: u16,
    }

    impl TypeStyle {
        /// 行高 / 字号。只用于校验与文档，不要拿它反算渲染行高。
        pub fn ratio(self) -> f32 {
            self.line_height / self.size
        }
    }

    /// 页面标题 H1：卡片标题、库预览标题。
    pub const PAGE_TITLE: TypeStyle = TypeStyle {
        size: 30.0,
        line_height: 42.0,
        weight: super::text::WEIGHT_SEMIBOLD,
    };
    /// 大纲页标题：全屏大纲的文档标题。
    pub const OUTLINE_TITLE: TypeStyle = TypeStyle {
        size: 28.0,
        line_height: 40.0,
        weight: super::text::WEIGHT_SEMIBOLD,
    };
    /// 卡片正文一级标题 H2。
    pub const HEADING_1: TypeStyle = TypeStyle {
        size: 22.0,
        line_height: 33.0,
        weight: super::text::WEIGHT_SEMIBOLD,
    };
    /// 卡片正文二级标题 H3。
    pub const HEADING_2: TypeStyle = TypeStyle {
        size: 18.0,
        line_height: 28.0,
        weight: super::text::WEIGHT_SEMIBOLD,
    };
    /// 卡片正文三级标题 H4。
    pub const HEADING_3: TypeStyle = TypeStyle {
        size: 16.0,
        line_height: 26.0,
        weight: super::text::WEIGHT_SEMIBOLD,
    };
    /// 大纲正文行。
    pub const OUTLINE_ROW: TypeStyle = TypeStyle {
        size: 16.0,
        line_height: 26.0,
        weight: super::text::WEIGHT_REGULAR,
    };
    /// 卡片正文、库预览正文。
    pub const READING_BODY: TypeStyle = TypeStyle {
        size: 16.0,
        line_height: 28.0,
        weight: super::text::WEIGHT_REGULAR,
    };
    /// 笔记库列表标题。
    pub const LIST_TITLE: TypeStyle = TypeStyle {
        size: 15.0,
        line_height: 22.0,
        weight: super::text::WEIGHT_SEMIBOLD,
    };
    /// 笔记库列表摘要。
    pub const LIST_EXCERPT: TypeStyle = TypeStyle {
        size: 13.0,
        line_height: 19.0,
        weight: super::text::WEIGHT_REGULAR,
    };
    /// 列表行标签/日期。
    pub const LIST_META: TypeStyle = TypeStyle {
        size: 12.0,
        line_height: 18.0,
        weight: super::text::WEIGHT_REGULAR,
    };
    /// 大纲说明行、库侧栏、斜杠菜单。
    pub const NAV: TypeStyle = TypeStyle {
        size: 14.0,
        line_height: 21.0,
        weight: super::text::WEIGHT_REGULAR,
    };
    /// 日期、来源、字数、面包屑。
    pub const META: TypeStyle = TypeStyle {
        size: 12.0,
        line_height: 18.0,
        weight: super::text::WEIGHT_REGULAR,
    };
    /// 计数、分组标题、快捷键提示。
    pub const TAG: TypeStyle = TypeStyle {
        size: 12.0,
        line_height: 18.0,
        weight: super::text::WEIGHT_SEMIBOLD,
    };
    /// 代码块与行内码放大档。
    pub const CODE: TypeStyle = TypeStyle {
        size: 14.0,
        line_height: 22.0,
        weight: super::text::WEIGHT_REGULAR,
    };

    /// 全部内容字阶，顺序与 `docs/28` §4.1 一致；测试与证据 dump 共用。
    pub const TYPE_SCALE: [TypeStyle; 14] = [
        PAGE_TITLE,
        OUTLINE_TITLE,
        HEADING_1,
        HEADING_2,
        HEADING_3,
        OUTLINE_ROW,
        READING_BODY,
        LIST_TITLE,
        LIST_EXCERPT,
        LIST_META,
        NAV,
        META,
        TAG,
        CODE,
    ];

    /// 一个内容表面的版心规则。
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Measure {
        /// 页面列最大宽度。
        pub page_max: f32,
        /// 正文最大宽度。
        pub text_max: f32,
        /// 页面顶部留白。
        pub pad_top: f32,
        /// 页面底部留白。
        pub pad_bottom: f32,
        /// 窄窗时左右至少留出的空间。
        pub narrow_side: f32,
    }

    impl Measure {
        /// 宽窗时页面列与正文之间的水平内缩。
        pub fn page_inset(self) -> f32 {
            ((self.page_max - self.text_max) * 0.5).max(0.0)
        }

        /// 给定可用宽度，算出页面列、正文列与实际左右留白。
        ///
        /// 宽窗先夹到 `page_max` 并居中；窄窗先保证 `narrow_side`，
        /// 再让正文尽量占满页面列。返回的 `side` 是**实际**左右外边距，
        /// 不是 `narrow_side` 本身。
        pub fn column(self, available: f32) -> Column {
            let available = if available.is_finite() {
                available.max(0.0)
            } else {
                0.0
            };
            let page = (available - 2.0 * self.narrow_side).clamp(0.0, self.page_max);
            let side = ((available - page) * 0.5).max(0.0);
            let text = page.min(self.text_max);
            let inset = ((page - text) * 0.5).max(0.0);
            Column {
                side,
                page,
                text,
                inset,
            }
        }
    }

    /// 一次版心计算的结果。
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Column {
        pub side: f32,
        pub page: f32,
        pub text: f32,
        pub inset: f32,
    }

    pub const OUTLINE: Measure = Measure {
        page_max: 800.0,
        text_max: 760.0,
        pad_top: 32.0,
        pad_bottom: 64.0,
        narrow_side: 32.0,
    };
    pub const CARD: Measure = Measure {
        page_max: 720.0,
        text_max: 640.0,
        pad_top: 48.0,
        pad_bottom: 96.0,
        narrow_side: 24.0,
    };
    pub const PREVIEW: Measure = Measure {
        page_max: 680.0,
        text_max: 600.0,
        pad_top: 40.0,
        pad_bottom: 64.0,
        narrow_side: 32.0,
    };

    /// 文字、状态与分隔线的透明度（0–1）。亮/暗各一套。
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ContentAlphas {
        pub text_secondary: f32,
        pub text_faint: f32,
        pub hover: f32,
        pub selected: f32,
        pub nav_selected: f32,
        pub editing: f32,
        pub hairline: f32,
        pub divider: f32,
    }

    pub const LIGHT_ALPHAS: ContentAlphas = ContentAlphas {
        text_secondary: 0.70,
        // 0.48/0.56 只有 2.6–3.7:1，占位符与空状态提示读起来发虚；0.65 在纸感上够 4.6，
        // 但雾青 / 暖砂的 page/panel 只有 4.28–4.45。0.68 让四套主题的阅读底全部 ≥4.5，
        // 仍比 secondary（0.70）淡一档。
        text_faint: 0.68,
        hover: 0.04,
        selected: 0.07,
        nav_selected: 0.04,
        editing: 0.03,
        hairline: 0.08,
        divider: 0.06,
    };
    pub const DARK_ALPHAS: ContentAlphas = ContentAlphas {
        text_secondary: 0.72,
        // 跟着亮色一起抬（暗色透明度必须 ≥ 亮色，见 content_alphas_match_the_contract）。
        // 暗底本来对比就宽裕，0.68 的合成色仍比 secondary 淡得多。
        text_faint: 0.68,
        hover: 0.06,
        selected: 0.12,
        nav_selected: 0.08,
        editing: 0.08,
        hairline: 0.12,
        divider: 0.10,
    };

    pub fn alphas(appearance: Appearance) -> ContentAlphas {
        match appearance {
            Appearance::Light => LIGHT_ALPHAS,
            Appearance::Dark => DARK_ALPHAS,
        }
    }

    /// 内容表面的语义色。带 alpha 的字段由主题主色/强调色派生，
    /// 绘制时可直接叠在 `page` / `panel` 上。
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ContentColors {
        pub page: Rgba,
        pub panel: Rgba,
        pub text_primary: Rgba,
        /// 摘要、说明与 12px 元信息；对 page/panel ≥ 4.5:1。
        pub text_secondary: Rgba,
        /// 仅占位、禁用与装饰性提示；不承载必须读到的信息。
        pub text_faint: Rgba,
        pub accent: Rgba,
        /// 链接与可点文字；亮色主题下会自动压暗到 ≥ 4.5:1。
        pub link: Rgba,
        pub hover: Rgba,
        pub selected: Rgba,
        pub nav_selected: Rgba,
        pub editing: Rgba,
        pub hairline: Rgba,
        pub divider: Rgba,
        pub nav_bar: Rgba,
        /// 侧栏标签圆点，仅 6px；取主题分支色前六支。
        pub tag_dots: [Rgba; 6],
        /// 瞬时告警（概念图落点重叠的 1px 描边）。只用于**短暂**反馈，
        /// 不承载常驻信息，所以不跟随强调色。
        pub warn: Rgba,
    }

    impl ContentColors {
        pub fn for_theme(theme: &Theme) -> Self {
            let a = alphas(theme.appearance);
            let text = theme.ui.text;
            let accent = theme.ui.accent;
            Self {
                page: theme.ui.bg,
                panel: theme.ui.panel,
                text_primary: text,
                text_secondary: with_alpha(text, a.text_secondary),
                text_faint: with_alpha(text, a.text_faint),
                accent,
                link: link_color(theme),
                hover: with_alpha(text, a.hover),
                selected: with_alpha(accent, a.selected),
                nav_selected: with_alpha(accent, a.nav_selected),
                editing: with_alpha(accent, a.editing),
                hairline: with_alpha(text, a.hairline),
                divider: with_alpha(text, a.divider),
                nav_bar: accent,
                tag_dots: tag_dots(theme),
                warn: warn_color(theme.appearance),
            }
        }
    }

    /// 非预乘 alpha 合成：把 `fg` 叠到 `bg` 上，返回不透明（背景不透明时）结果。
    pub fn composite(fg: Rgba, bg: Rgba) -> Rgba {
        let fa = fg.0[3] as f32 / 255.0;
        let ba = bg.0[3] as f32 / 255.0;
        let out_a = fa + ba * (1.0 - fa);
        if out_a <= f32::EPSILON {
            return Rgba::TRANSPARENT;
        }
        let ch = |i: usize| {
            let v = (fg.0[i] as f32 * fa + bg.0[i] as f32 * ba * (1.0 - fa)) / out_a;
            v.round().clamp(0.0, 255.0) as u8
        };
        Rgba::rgba(
            ch(0),
            ch(1),
            ch(2),
            (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
        )
    }

    /// 焦点环：1.5px 强调色 80%，外偏移 1px。
    pub mod focus {
        pub const RING: f32 = 1.5;
        pub const OFFSET: f32 = 1.0;
        pub const ALPHA: f32 = 0.80;
    }

    /// 内容圆角：行/标签 6、代码块/斜杠菜单/图片 10、弹窗 18。
    pub mod radius {
        pub const ROW: f32 = crate::design::radius::SM;
        pub const BLOCK: f32 = crate::design::radius::MD;
        pub const DIALOG: f32 = crate::design::radius::DIALOG;
    }

    /// 内容动效：只做透明度与 2–4px 位移，全部 ease-out。
    pub mod motion {
        pub const HOVER: f32 = 0.12;
        pub const SELECT: f32 = 0.14;
        pub const OVERLAY: f32 = 0.16;
        pub const FOCUS_SCROLL: f32 = 0.18;
    }

    /// 内容滚动条：默认透明，悬停/滚动淡入，空闲后淡出。
    pub mod scrollbar {
        pub const WIDTH: f32 = 6.0;
        pub const THUMB: f32 = 4.0;
        pub const RADIUS: f32 = 8.0;
        pub const FADE_IN: f32 = super::motion::HOVER;
        pub const IDLE_FADE: f32 = 0.30;
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ScrollbarMetrics {
        pub width: f32,
        pub thumb: f32,
        pub radius: f32,
        pub fade_in: f32,
        pub idle_fade: f32,
    }

    /// 当前主题 + DPI 下的一组内容表面度量。
    ///
    /// `hairline` 用 point 表示「恰好 1 个物理像素」：1.0 / dpi。
    /// 内容表面的 1px 分隔线应取这个值，而不是永远写 1.0 point。
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ContentMetrics {
        pub dpi: f32,
        pub hairline: f32,
        pub focus_ring: f32,
        pub focus_ring_offset: f32,
        pub focus_ring_alpha: f32,
        pub scrollbar: ScrollbarMetrics,
        pub colors: ContentColors,
    }

    /// 纯函数：不读全局、不写文件，四主题与任意 DPI 可重复。
    pub fn content_metrics(theme: &Theme, dpi: f32) -> ContentMetrics {
        let dpi = if dpi.is_finite() && dpi > 0.0 {
            dpi.clamp(0.5, 4.0)
        } else {
            1.0
        };
        ContentMetrics {
            dpi,
            hairline: 1.0 / dpi,
            focus_ring: focus::RING,
            focus_ring_offset: focus::OFFSET,
            focus_ring_alpha: focus::ALPHA,
            scrollbar: ScrollbarMetrics {
                width: scrollbar::WIDTH,
                thumb: scrollbar::THUMB,
                radius: scrollbar::RADIUS,
                fade_in: scrollbar::FADE_IN,
                idle_fade: scrollbar::IDLE_FADE,
            },
            colors: ContentColors::for_theme(theme),
        }
    }

    fn with_alpha(color: Rgba, alpha: f32) -> Rgba {
        Rgba::rgba(
            color.0[0],
            color.0[1],
            color.0[2],
            (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    /// 告警红：亮色主题下压到对纸白 ≥ 4.5:1 的深红，暗色主题下用亮 30% 的红，
    /// 保证 1px 描边在两种底色上都看得见。
    fn warn_color(appearance: crate::theme::Appearance) -> Rgba {
        match appearance {
            crate::theme::Appearance::Light => Rgba::rgb(0xc0, 0x2b, 0x2b),
            crate::theme::Appearance::Dark => Rgba::rgb(0xff, 0x7b, 0x72),
        }
    }

    /// 链接色优先用强调色；亮色主题强调色对页面不足 4.5:1 时，    /// 按 5% 步进向正文色压暗，直到可读。暗色主题通常原样通过。
    fn link_color(theme: &Theme) -> Rgba {
        let accent = theme.ui.accent;
        let page = theme.ui.bg;
        if accent.contrast_ratio(page) >= 4.5 {
            return accent;
        }
        for step in 1..=12 {
            let candidate = accent.mix(theme.ui.text, step as f32 * 0.05);
            if candidate.contrast_ratio(page) >= 4.5 {
                return candidate;
            }
        }
        theme.ui.text
    }

    fn tag_dots(theme: &Theme) -> [Rgba; 6] {
        if theme.palette.is_empty() {
            let mut dots = [theme.ui.dim; 6];
            dots[0] = theme.ui.accent;
            return dots;
        }
        let mut dots = [theme.ui.dim; 6];
        for (i, dot) in dots.iter_mut().enumerate() {
            *dot = theme.palette[i % theme.palette.len()];
        }
        dots
    }
}

/// 控件几何：所有可点击/可聚焦控件的尺寸都从这里取。
///
/// 数值不是随意挑的：命中区不小于 24，输入框高于图标按钮，行高落在两者之间，
/// 这样同一列里「图标按钮 / 输入框 / 普通行」不会忽高忽低。
pub mod control {
    /// 图标按钮与悬停工具条的命中边长。
    pub const HIT: f32 = 24.0;
    /// 关闭叉、页签加号这类紧凑图标按钮的绘制边长。
    pub const ICON_BUTTON: f32 = 22.0;
    /// 面板/菜单/检查器的单行高度。
    pub const ROW: f32 = 28.0;
    /// 输入框与 egui 默认交互高度。
    pub const INPUT: f32 = 30.0;
    /// 默认按钮的水平/垂直内边距。
    pub const BUTTON_PAD: [f32; 2] = [10.0, 5.0];
}

/// 组件几何：按钮、胶囊、工具按钮、分段和开关共用同一套度量。
///
/// 这些数字原先散落在卡片库、检查器和大纲三处各写一份，改一处就漏两处；
/// 收进令牌后，同一档控件在任何面板里都同高、同圆角、同命中区。
pub mod component {
    /// 文字按钮内边距。
    pub const BUTTON_PAD: [f32; 2] = [12.0, 6.0];
    /// 文字按钮圆角。
    pub const BUTTON_RADIUS: f32 = super::radius::SM;
    /// 胶囊（计数、状态）内边距。
    pub const PILL_PAD: [f32; 2] = [16.0, 6.0];
    /// 胶囊圆角：比按钮更圆，靠轮廓而不是底色区分状态。
    pub const PILL_RADIUS: f32 = 9.0;
    /// 描边按钮高度：开始页「返回文档」这类主入口。
    pub const OUTLINE_HEIGHT: f32 = 34.0;
    /// 描边按钮圆角。
    pub const OUTLINE_RADIUS: f32 = 9.0;
    /// 工具条按钮绘制边长。
    pub const TOOL: f32 = 28.0;
    /// 工具条按钮命中边长：中心距正好 28，热区相接不留缝。
    pub const TOOL_HIT: f32 = 32.0;
    /// 同组工具按钮的间距：命中区相切，视觉上留 4px。
    pub const TOOL_GAP: f32 = TOOL_HIT - TOOL;
    /// 工具组之间留给 1px 分隔线的水平空间。
    pub const TOOL_GROUP_SEP: f32 = 24.0;
    /// 自绘标题栏高度：48 是 Win11 应用栏的常见档位，给 30px 菜单留 9px 上下呼吸。
    pub const TITLEBAR_H: f32 = 48.0;
    /// 窗口控制按钮宽度：关闭/最大化/最小化三等分。
    pub const WINDOW_CONTROL_W: f32 = 46.0;
    /// 标题栏菜单触发器高度。
    pub const MENU_TRIGGER_H: f32 = 32.0;
    /// 菜单行高度。
    pub const MENU_ROW_H: f32 = 32.0;
    /// 保存状态芯片高度。
    pub const SAVE_CHIP_H: f32 = 24.0;

    /// 标题栏下方标签条高度。
    pub const TABBAR_H: f32 = 34.0;
    /// 标签绘制高度：条内上下各留 2px，13/19 的标签字有呼吸。
    pub const TAB_H: f32 = 30.0;
    /// 标签最小宽度：短标题也保留可点面积与关闭位。
    pub const TAB_MIN_W: f32 = 108.0;
    /// 标签最大宽度：再长就用省略号，别让一个标签吃掉整条。
    pub const TAB_MAX_W: f32 = 220.0;
    /// 标签之间的间隙。
    pub const TAB_GAP: f32 = 2.0;
    /// 标签条左右内缩。
    pub const TAB_INSET: f32 = 8.0;
    /// 标签顶圆角：只圆上半圈，底边贴住标签条的分隔线。
    pub const TAB_RADIUS: f32 = 6.0;
    /// 标签内关闭格的边长：图标 14，命中不跨到文字上。
    pub const TAB_CLOSE: f32 = 22.0;

    /// 模态对话框标题区高度：标题与关闭叉固定在这一条，内容再长也不推挤它。
    pub const DIALOG_HEADER_H: f32 = 44.0;
    /// 模态对话框按钮区高度。
    pub const DIALOG_FOOTER_H: f32 = 56.0;
    /// 对话框动作按钮高度。
    pub const DIALOG_ACTION_H: f32 = 32.0;
    /// 对话框动作按钮最小宽度：两字中文加左右内距仍显从容。
    pub const DIALOG_ACTION_MIN_W: f32 = 76.0;
    /// 对话框动作按钮间距。
    pub const DIALOG_ACTION_GAP: f32 = 8.0;
    /// 对话框最小宽度：再窄，标题会贴上关闭叉。
    pub const DIALOG_MIN_W: f32 = 376.0;
    /// 对话框最大宽度：再宽，正文行长会超出舒适阅读区间。
    pub const DIALOG_MAX_W: f32 = 720.0;
    /// 对话框内容区水平内边距。
    pub const DIALOG_PAD_X: f32 = 20.0;
    /// 对话框内容区顶部内边距。
    pub const DIALOG_PAD_TOP: f32 = 16.0;
    /// 对话框内容区底部内边距（内容与按钮区分隔线之间）。
    pub const DIALOG_PAD_BOTTOM: f32 = 12.0;

    /// 画布浮层（面包屑、缩放条）的统一高度：32px 玻璃条。
    pub const FLOATING_H: f32 = 32.0;
    /// 画布上下文工具条：36px = 28px 工具 + 上下各 4px。
    pub const TOOLBAR_H: f32 = 36.0;
    /// 分段控件外框圆角。
    pub const SEGMENT_RADIUS: f32 = 7.0;
    /// 分段控件选中块圆角：外框减 2px 内距后的视觉同心值。
    pub const SEGMENT_INNER_RADIUS: f32 = 5.0;
    /// 分段控件外框与选中块之间的内距。
    pub const SEGMENT_GAP: f32 = 2.0;
    /// 开关轨道尺寸。
    pub const SWITCH: [f32; 2] = [36.0, 20.0];
    /// 开关圆钮直径。
    pub const SWITCH_KNOB: f32 = 16.0;
    /// 开关圆钮与轨道边缘的间距。
    pub const SWITCH_PAD: f32 = 2.0;
    /// 自定义绘制控件的焦点环外扩距离。
    ///
    /// 与 `content::focus::OFFSET` 是同一件事：以前这里写 1.4/1.5、内容面写 1.5/1.0，
    /// 两条绘制路径的焦点环肉眼几乎一样却差 0.5px。现在统一引用内容面那一套。
    pub const FOCUS_RING_OFFSET: f32 = super::content::focus::OFFSET;
    /// 焦点环线宽。
    pub const FOCUS_RING: f32 = super::content::focus::RING;
}

/// 图标：几何写在 16 单位网格里，按实际边长归一；线宽保持屏幕空间恒定。
///
/// 图标不允许用字形（`▾ ⋯ ＋ ‹ › ✕`）：不同字体的基线、粗细和左右留白都不同，
/// 125% 缩放下会偏半像素，且无法统一悬停色。所有图标都由 `paint_icon` 画矢量。
pub mod icon {
    /// 菜单、工具条、面板标题的默认图标盒。
    pub const SIZE: f32 = 16.0;
    /// 行内小图标：角标、紧凑按钮。
    pub const SMALL: f32 = 14.0;
    /// 绘制网格边长：几何常量都写在这个坐标系里（中心为 0）。
    pub const GRID: f32 = 16.0;
    /// 主笔线宽，屏幕空间恒定；不随图标盒缩放。
    pub const STROKE: f32 = 1.5;
    /// 次级线条：文字线、桶沿、叠层。
    pub const STROKE_FINE: f32 = 1.1;
    /// Windows 标题栏控制按钮：遵循系统 1.2px 细线，不跟应用图标的 1.5。
    pub const STROKE_CHROME: f32 = 1.2;
    /// 只用于实心点（更多、色点）。
    pub const DOT: f32 = 1.5;
    /// 最小命中区，不得小于 `control::HIT`。
    pub const HIT: f32 = super::control::HIT;
    /// 图标字形尺寸：检查器图标等自绘符号（不是文字字阶）。
    pub const GLYPH_MEDIUM: f32 = 18.0;
    /// 演示控制条上的单字符按钮。
    pub const GLYPH_LARGE: f32 = 19.0;
}

/// 卡片内距：**随字号分档**，不是全应用一个常数。
///
/// 以前所有层级都用 18/9，结果中心卡显得挤、深层卡显得松，层级感全靠字号硬撑。
/// 现在按字号给三档，深层的卡更紧凑、中心的卡更舒展，一眼能看出层次。
pub const fn card_padding(font_size: f32) -> (f32, f32) {
    if font_size >= 26.0 {
        (26.0, 12.0)
    } else if font_size >= 19.0 {
        (18.0, 10.0)
    } else if font_size >= 14.5 {
        (14.0, 8.0)
    } else {
        (12.0, 7.0)
    }
}

/// 圆角：从小到大，**每一档都有明确归属**，不要再用「差不多」的裸数字。
pub mod radius {
    /// 键帽、小色块、悬停底
    pub const XS: f32 = 4.0;
    /// 小控件：色点、角标
    pub const SM: f32 = 6.0;
    /// 按钮、输入框、列表行
    pub const CONTROL: f32 = 8.0;
    /// 卡片与面板
    pub const MD: f32 = 10.0;
    /// 菜单、工具条浮层
    pub const PANEL: f32 = 12.0;
    /// 弹出层、对话框
    pub const LG: f32 = 14.0;
    /// 模态主窗
    pub const DIALOG: f32 = 18.0;
}

/// 模态暗幕：80/255 ≈ 31% 黑。浅色画布上足以压住背景，又不至于像黑屏；
/// 深色主题下仍能看出卡片比桌面亮一档。
pub mod scrim {
    pub const ALPHA: u8 = 80;
}

/// 描边：发丝线。
pub mod stroke {
    /// 描边宽度（逻辑像素）
    pub const HAIRLINE: f32 = 1.0;
    /// 描边透明度：浅色下 6% 的黑刚刚好，不抢内容。
    pub const HAIRLINE_ALPHA: f32 = 0.06;
}

/// 动效：时长、曲线与场景位移的时长带。
pub mod motion {
    /// 悬停、按下这类即时反馈
    pub const FAST: f32 = 0.12;
    /// 面板滑入、抽屉
    pub const BASE: f32 = 0.18;
    /// 模式切换、大块位移
    pub const SLOW: f32 = 0.24;
    /// 布局切换的最小可感知时长
    pub const SCENE_MIN: f32 = 0.20;
    /// 布局切换的上限：再长就拖沓
    pub const SCENE_MAX: f32 = 0.34;

    /// ease-out-cubic：起步快、收尾软，界面位移默认用它。
    pub fn ease_out_cubic(t: f32) -> f32 {
        let u = 1.0 - t.clamp(0.0, 1.0);
        1.0 - u * u * u
    }

    /// 实际时长：减少动画时直接落到终态，由调用方决定是否跳过。
    pub fn duration(base: f32, reduce: bool) -> f32 {
        if reduce {
            0.0
        } else {
            base.max(0.0)
        }
    }
}

/// 阴影：集中成三档，避免每个浮层各写一套 offset/blur/alpha。
pub mod elevation {
    /// 一个阴影的令牌值（与 egui `epaint::Shadow` 的字段一一对应）。
    pub struct Shadow {
        pub offset: [i8; 2],
        pub blur: u8,
        pub spread: u8,
        pub alpha: u8,
    }

    /// 面板/工具条：几乎只是一条深色分隔。
    pub const PANEL: Shadow = Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        alpha: 18,
    };
    /// 窗口：主浮层，和旧视觉一致（offset 6 / blur 22 / alpha 30）。
    pub const WINDOW: Shadow = Shadow {
        offset: [0, 6],
        blur: 22,
        spread: 0,
        alpha: 30,
    };
    /// 弹出层/菜单：比窗口再高一档。
    pub const POPOVER: Shadow = Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        alpha: 38,
    };
    /// 模态对话框：整窗最重的一层，压住画布与暗幕。
    pub const DIALOG: Shadow = Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        alpha: 46,
    };
}

/// sRGB 相对亮度（WCAG 定义）。
pub fn luminance(rgb: [u8; 3]) -> f32 {
    fn channel(v: u8) -> f32 {
        let c = v as f32 / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

/// 两色对比度（1..21），WCAG AA 正文要求 >= 4.5。
pub fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// 壳层 primitive：颜色与图标字号的**原始取值**，一处定义。
///
/// T1 只做搬运——这里的每一个数字都等于 2026-11-02 之前散落在 `mw-shell`
/// 各处的字面量，取值一字未改（`shell_tokens_match_the_old_literals` 逐项钉死）。
/// 语义层（canvas / panel / text / accent …）由 T2 补齐；在此之前壳层直接消费这里，
/// T2 起组件只许经语义层取色，不许再回到 primitive。
///
/// `primitive` 自身是 `pub(crate)`：下游 crate 只能按 `shell::light::PANEL`
/// 这样的**具体令牌名**取用，不能把整个 primitive 命名空间当公开 API 再导出。
pub mod shell {
    pub use primitive::*;

    pub(crate) mod primitive {
        use crate::theme::Rgba;

        /// 亮色壳层：顶栏、面板、浮层、状态栏（四套亮色主题共用这一套）。
        pub mod light {
            use crate::theme::Rgba;

            pub const PANEL: Rgba = Rgba::rgb(255, 255, 255);
            pub const PANEL2: Rgba = Rgba::rgb(0xF2, 0xF3, 0xF5);
            /// 标题栏：247/255 不透明，压住 Mica 又留一点透。
            pub const TITLEBAR: Rgba = Rgba::rgba(255, 255, 255, 247);
            pub const SEP: Rgba = Rgba::rgb(0xE8, 0xEA, 0xEE);
            pub const BORDER: Rgba = Rgba::rgb(0xE6, 0xE9, 0xED);
            pub const BORDER_HOT: Rgba = Rgba::rgb(0xD8, 0xDD, 0xE4);
            /// 无框窗口最外圈那 1px。
            pub const EDGE: Rgba = Rgba::rgb(0x96, 0x96, 0x96);
            pub const TEXT: Rgba = Rgba::rgb(0x3E, 0x44, 0x4C);
            pub const DIM: Rgba = Rgba::rgb(0x5C, 0x64, 0x6F);
            pub const FAINT: Rgba = Rgba::rgb(0x65, 0x6D, 0x78);
            pub const HOVER: Rgba = Rgba::rgb(0xF1, 0xF3, 0xF6);
            pub const ICON_OFF: Rgba = Rgba::rgb(0xC6, 0xCB, 0xD2);
            pub const WARN: Rgba = Rgba::rgb(0xB4, 0x6A, 0x1E);
        }

        /// 暗色壳层里**不跟主题走**的那几支（其余取 `ThemeUi`）。
        pub mod dark {
            use crate::theme::Rgba;

            pub const FAINT: Rgba = Rgba::rgb(0x8B, 0x92, 0x9B);
            pub const WARN: Rgba = Rgba::rgb(0xE9, 0xA0, 0x5C);
            /// 禁用图标：主题 `dim` 上的透明度。
            pub const ICON_OFF_ALPHA: u8 = 90;
        }

        /// 浮层（搜索条 / 命令面板 / 右键菜单 / 帮助）专用色。
        pub mod tone {
            use crate::theme::Rgba;

            pub const STROKE_LIGHT: Rgba = Rgba::rgb(0xE0, 0xE4, 0xEA);
            pub const HAIRLINE_LIGHT: Rgba = Rgba::rgb(0xEC, 0xEF, 0xF3);
            pub const SHADOW_DARK_ALPHA: u8 = 120;
            pub const SHADOW_LIGHT_ALPHA: u8 = 40;
            /// 关键词命中底：语义固定为暖黄，不跟主题。
            pub const HIT_BG_DARK: Rgba = Rgba::rgb(0x6B, 0x58, 0x1C);
            pub const HIT_BG_LIGHT: Rgba = Rgba::rgb(0xFF, 0xE7, 0xA3);
            pub const HIT_INK_DARK: Rgba = Rgba::rgb(0xFF, 0xE9, 0xA8);
            pub const HIT_INK_LIGHT: Rgba = Rgba::rgb(0x4A, 0x35, 0x00);
            /// 禁用态（`from_gray(212)` 的等价取值）。
            pub const OFF_LIGHT: Rgba = Rgba::rgb(212, 212, 212);
            /// 图标悬停圆底（`from_gray(242)` 的等价取值）。
            pub const CHIP_LIGHT: Rgba = Rgba::rgb(242, 242, 242);
        }

        /// Win11 关闭按钮悬停红。
        pub const CLOSE_HOT: Rgba = Rgba::rgb(0xC4, 0x2B, 0x1C);

        /// 标签色盘：同一路径根永远同色，不依赖文档/主题，重启也稳定。
        pub mod tag {
            use crate::theme::Rgba;

            pub const BLUE: Rgba = Rgba::rgb(0x7A, 0xA2, 0xF7);
            pub const GREEN: Rgba = Rgba::rgb(0x9E, 0xCE, 0x6A);
            pub const YELLOW: Rgba = Rgba::rgb(0xE0, 0xAF, 0x68);
            pub const RED: Rgba = Rgba::rgb(0xF7, 0x76, 0x8E);
            pub const PURPLE: Rgba = Rgba::rgb(0xBB, 0x9A, 0xF7);
            pub const CYAN: Rgba = Rgba::rgb(0x7D, 0xCF, 0xFF);
            pub const ORANGE: Rgba = Rgba::rgb(0xFF, 0x9E, 0x64);
            pub const TEAL: Rgba = Rgba::rgb(0x73, 0xDA, 0xCA);
        }

        /// Markdown 测试夹具的默认色（正式阅读面由 `content::ContentColors` 给）。
        pub mod markdown {
            use crate::theme::Rgba;

            pub const TEXT: Rgba = Rgba::rgb(30, 30, 30);
            pub const DIM: Rgba = Rgba::rgb(90, 90, 90);
            pub const FAINT: Rgba = Rgba::rgb(150, 150, 150);
            pub const ACCENT: Rgba = Rgba::rgb(40, 110, 220);
            pub const CODE_BG: Rgba = Rgba::rgb(240, 240, 240);
        }

        /// 性能浮层。
        pub mod stats {
            use crate::theme::Rgba;

            pub const SCRIM_ALPHA: u8 = 232;
            /// 底色与 alpha 同源：改一处即改整块。
            pub const SCRIM: Rgba = Rgba::rgba(24, 27, 31, SCRIM_ALPHA);
            pub const INK: Rgba = Rgba::rgb(0xE8, 0xEB, 0xEF);
        }

        /// 演示态的两块浮层（演讲者备注、演示控制条）。
        pub mod overlay {
            use crate::theme::Rgba;

            pub const NOTES_BG_ALPHA: u8 = 228;
            pub const NOTES_BG: Rgba = Rgba::rgba(18, 21, 26, NOTES_BG_ALPHA);
            pub const NOTES_STROKE_ALPHA: u8 = 22;
            pub const NOTES_STROKE: Rgba = Rgba::rgba(255, 255, 255, NOTES_STROKE_ALPHA);
            pub const BAR_BG_ALPHA: u8 = 238;
            pub const BAR_BG: Rgba = Rgba::rgba(26, 30, 36, BAR_BG_ALPHA);
            pub const BAR_STROKE_ALPHA: u8 = 20;
            pub const BAR_STROKE: Rgba = Rgba::rgba(255, 255, 255, BAR_STROKE_ALPHA);
            pub const BAR_SHADOW_ALPHA: u8 = 80;
        }

        /// 检查器：暗底上的内凹井与对比度「偏低」提醒。
        pub mod inspector {
            use crate::theme::Rgba;

            pub const RING_WHITE_ALPHA: u8 = 18;
            pub const RING_BLACK_ALPHA: u8 = 18;
            pub const WARN: Rgba = Rgba::rgb(0xC0, 0x5A, 0x4A);
        }
    }
}

#[cfg(test)]
mod tests {
    // 设计令牌全是 f32 常量，无法用 const 断言校验；这里的 assert! 是
    // 刻意的运行期不变量，不是恒真垃圾断言。
    #![allow(clippy::assertions_on_constants)]

    use super::*;

    #[test]
    fn spacing_sits_on_a_four_pixel_grid() {
        for v in [
            space::XS,
            space::SM,
            space::MD,
            space::LG,
            space::XL,
            space::XXL,
        ] {
            assert_eq!(v % 4.0, 0.0, "间距 {v} 不在 4px 网格上");
        }
    }

    #[test]
    fn type_scale_is_monotonic_and_line_height_loosens_for_small_text() {
        let scale = [
            text::CAPTION,
            text::SMALL,
            text::BODY,
            text::LABEL,
            text::CANVAS_SUB,
            text::CANVAS_MAIN,
            text::TITLE,
            text::CANVAS_CENTER,
        ];
        for w in scale.windows(2) {
            assert!(w[1] >= w[0], "字阶必须单调不减：{} -> {}", w[0], w[1]);
        }
        // 小字更松、大字更紧
        assert!(text::line_height(text::CAPTION) > text::line_height(text::BODY));
        assert!(text::line_height(text::BODY) > text::line_height(text::CANVAS_CENTER));
        for s in scale {
            let lh = text::line_height(s) * s;
            assert!(
                lh >= s * 1.15 && lh <= s * 1.6,
                "行高 {lh} 越界（字号 {s}）"
            );
        }
    }

    /// X22 G01 × X25 G-25-07：画布层级合同表必须与布局公式、令牌逐项一致。
    ///
    /// 这张表是「字阶 × 字重 × 行高 × 内距」的唯一出口；任何一项漂了都该在这里红，
    /// 而不是等到 178 场基线对不上才发现。
    #[test]
    fn canvas_level_table_agrees_with_the_formulas() {
        let sizes = [
            text::CANVAS_CENTER,
            text::CANVAS_MAIN,
            text::CANVAS_SUB,
            text::CANVAS_LEAF,
        ];
        for (i, size) in sizes.into_iter().enumerate() {
            let level = text::CANVAS_LEVELS[i];
            assert_eq!(level.size, size, "第 {i} 层字号偏离 CANVAS_* 令牌");
            assert_eq!(
                level.line_height,
                text::line_height(size),
                "第 {i} 层行高倍数不是 line_height() 的现值"
            );
            assert_eq!(
                level.pad,
                card_padding(size),
                "第 {i} 层内距不是 card_padding() 的现值"
            );
        }
        // 中心加粗（600）、一级及以下常规（400）——与 docs/28 §4.1 的三档字重口径一致。
        assert_eq!(
            text::CANVAS_LEVELS[0].weight,
            text::WEIGHT_SEMIBOLD,
            "中心主题必须是半粗"
        );
        assert!(
            text::CANVAS_LEVELS[1..]
                .iter()
                .all(|l| l.weight == text::WEIGHT_REGULAR),
            "一级及以下必须是常规体"
        );
        // 合同行高（单行）：28×1.4 / 20×1.44 / 16×1.44 / 15×1.46。
        let expected = [39.2, 28.8, 23.04, 21.9];
        for (level, want) in text::CANVAS_LEVELS.iter().zip(expected) {
            let got = level.absolute_line_height();
            assert!((got - want).abs() < 0.01, "单行合同行高 {got} 应为 {want}");
        }
        // 单行合同盒高：round(行高) + 2×垂直内距 ⇒ 63 / 49 / 39 / 38。
        // 这条把 X25 R-3（X22 §7.1 写 36 vs 源码复算 38）钉在合同侧；
        // DirectWrite 实测行盒若不同，以实测为准并单独记账。
        let expected_box = [63.0, 49.0, 39.0, 38.0];
        for (level, want) in text::CANVAS_LEVELS.iter().zip(expected_box) {
            assert_eq!(
                level.single_line_box_height(),
                want,
                "单行合同盒高应为 {want}"
            );
        }
        // 概念图两档：主概念比普通概念大 1，且都小于画布中心。
        assert_eq!(text::CONCEPT, 14.0);
        assert_eq!(text::CONCEPT_PRIMARY, 15.0);
        assert!(text::CONCEPT_PRIMARY > text::CONCEPT);
        assert!(text::CONCEPT_PRIMARY < text::CANVAS_CENTER);
    }

    #[test]
    fn default_palette_meets_wcag_aa_for_body_text() {
        // 「纸感」默认主题的三个文字层，对画布与卡片底都要够读。
        let canvas = [0xFB, 0xFA, 0xF7];
        let card = [0xFF, 0xFF, 0xFF];
        let ink = [0x2A, 0x2E, 0x33];
        let ink_soft = [0x55, 0x5B, 0x63];
        for bg in [canvas, card] {
            assert!(
                contrast(ink, bg) >= 4.5,
                "正文对比度不足：{:.2}",
                contrast(ink, bg)
            );
            assert!(
                contrast(ink_soft, bg) >= 4.5,
                "次要文字对比度不足：{:.2}",
                contrast(ink_soft, bg)
            );
        }
        // 纯白画布反而是"眩光"的来源：默认底色不该是纯白
        assert_ne!(canvas, [255, 255, 255]);
    }

    #[test]
    fn card_padding_shrinks_with_the_type_scale() {
        let sizes = [
            text::CANVAS_CENTER,
            text::CANVAS_MAIN,
            text::CANVAS_SUB,
            14.0,
        ];
        let mut prev = (f32::MAX, f32::MAX);
        for s in sizes {
            let (x, y) = card_padding(s);
            assert!(
                x <= prev.0 && y <= prev.1,
                "字号 {s} 的内距 ({x},{y}) 应该不大于更大的字"
            );
            assert!(x >= 8.0 && y >= 4.0, "内距不能小到贴边");
            assert!(x > y, "水平内距必须大于垂直内距，否则文字像被裁掉");
            prev = (x, y);
        }
        // 中心卡比子级卡明显舒展（差异要能被眼睛看出来，不是 1px）
        let (cx, cy) = card_padding(text::CANVAS_CENTER);
        let (sx, sy) = card_padding(14.0);
        assert!(cx - sx >= 8.0 && cy - sy >= 4.0, "层级之间的内距差太微弱");
    }

    #[test]
    fn contrast_helper_matches_known_pairs() {
        assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert!((contrast([255, 255, 255], [255, 255, 255]) - 1.0).abs() < 0.01);
        // 对称性
        let a = contrast([0x2A, 0x2E, 0x33], [0xFB, 0xFA, 0xF7]);
        let b = contrast([0xFB, 0xFA, 0xF7], [0x2A, 0x2E, 0x33]);
        assert!((a - b).abs() < 1e-6);
    }

    /// 图标线宽必须是屏幕空间常量，命中区必须容得下图标盒。
    #[test]
    fn icon_tokens_keep_stroke_constant_and_hit_zone_large_enough() {
        assert!(icon::SMALL < icon::SIZE);
        assert_eq!(icon::GRID, icon::SIZE, "默认图标盒必须正好是一格");
        assert!(icon::HIT >= icon::SIZE + 8.0, "命中区至少比图标大 8px");
        assert_eq!(icon::HIT, control::HIT);
        assert!(icon::STROKE >= 1.0 && icon::STROKE <= 2.0);
        assert!(icon::STROKE_FINE < icon::STROKE_CHROME);
        assert!(icon::STROKE_CHROME < icon::STROKE);
        assert!(icon::DOT < icon::SIZE / 4.0);
    }

    /// 壳层字阶必须严格递增，且行高落在可读区间；界面字号不能反超画布中心。
    #[test]
    fn ui_type_scale_is_its_own_monotonic_ladder() {
        let tiers = [
            ui::CAPTION,
            ui::SMALL,
            ui::BODY,
            ui::LABEL,
            ui::HEADING,
            ui::DISPLAY,
        ];
        for w in tiers.windows(2) {
            assert!(w[1] > w[0], "壳层字阶必须严格递增：{} -> {}", w[0], w[1]);
        }
        for s in tiers {
            let lh = ui::line_height(s) * s;
            assert!(
                lh >= s * 1.15 && lh <= s * 1.6,
                "壳层行高 {lh} 越界（字号 {s}）"
            );
        }
        assert!(
            ui::DISPLAY < text::CANVAS_CENTER,
            "界面品牌字不该比画布中心主题还大"
        );
        // 两条字阶共享行高规则，不共享字号：改画布令牌不该移动面板。
        assert_eq!(ui::line_height(ui::BODY), text::line_height(text::BODY));
    }

    /// 控件几何的包含关系：命中区不小于图标按钮，输入框不低于行高，
    /// 行高不低于图标按钮。这三条一旦破了，同一列控件就会高低跳动。
    #[test]
    fn control_metrics_are_nested_not_arbitrary() {
        assert!(control::HIT >= 24.0, "可点击目标小于 24px");
        assert!(control::ICON_BUTTON <= control::HIT);
        assert!(control::ICON_BUTTON <= control::ROW);
        assert!(control::ROW <= control::INPUT);
        assert!(
            control::BUTTON_PAD[0] > control::BUTTON_PAD[1],
            "按钮水平内边距应大于垂直内边距"
        );
    }

    /// 组件度量必须互相自洽：命中区容得下绘制尺寸，圆钮放得进轨道，
    /// 分段选中块的内圆角小于外框，否则视觉上会「内框戳出外框」。
    #[test]
    fn component_metrics_fit_inside_each_other() {
        assert_eq!(component::BUTTON_RADIUS, radius::SM);
        assert!(component::BUTTON_PAD[0] > component::BUTTON_PAD[1]);
        assert_eq!(component::PILL_PAD[1], component::BUTTON_PAD[1]);
        assert!(component::PILL_RADIUS > component::BUTTON_RADIUS);
        assert!(component::TOOL_HIT >= component::TOOL);
        assert!(component::TOOL_GAP > 0.0);
        assert!(component::TOOL >= control::HIT);
        assert!(component::TITLEBAR_H > component::MENU_TRIGGER_H);
        assert!(component::TITLEBAR_H - component::MENU_TRIGGER_H >= 16.0);
        assert!(component::WINDOW_CONTROL_W >= 40.0);
        assert!(component::SAVE_CHIP_H < component::MENU_TRIGGER_H);
        assert!(component::FLOATING_H >= control::HIT + 8.0);
        assert!(component::TOOLBAR_H > component::FLOATING_H);
        assert_eq!(component::TOOLBAR_H, component::TOOL + 8.0);
        assert!(component::SEGMENT_RADIUS > component::SEGMENT_INNER_RADIUS);
        assert_eq!(
            component::SEGMENT_RADIUS - component::SEGMENT_INNER_RADIUS,
            component::SEGMENT_GAP
        );
        assert!(component::SWITCH[0] > component::SWITCH[1]);
        assert_eq!(
            component::SWITCH_KNOB + component::SWITCH_PAD * 2.0,
            component::SWITCH[1],
            "圆钮加内距必须正好填满轨道高度"
        );
        assert!(component::FOCUS_RING_OFFSET > 0.0);
        assert!(component::FOCUS_RING >= stroke::HAIRLINE);
        // 弹窗三段式：标题区容得下命中区，按钮区容得下动作按钮，按钮不至于变成方块。
        assert!(component::DIALOG_HEADER_H >= control::HIT + 16.0);
        assert!(component::DIALOG_FOOTER_H > component::DIALOG_ACTION_H);
        assert!(component::DIALOG_ACTION_H >= control::HIT);
        assert!(component::DIALOG_ACTION_MIN_W > component::DIALOG_ACTION_H * 2.0);
        assert!(component::DIALOG_ACTION_GAP >= space::XS);
        assert!(component::DIALOG_MIN_W < component::DIALOG_MAX_W);
        assert!(component::DIALOG_PAD_X >= space::LG);
        for v in [
            component::DIALOG_PAD_X,
            component::DIALOG_PAD_TOP,
            component::DIALOG_PAD_BOTTOM,
        ] {
            assert_eq!(v % 4.0, 0.0, "弹窗内距 {v} 不在 4px 网格上");
        }
        // 标签条：标签放得进条内，关闭格放得进标签，最小宽容得下「脏点 + 关闭 + 一点字」。
        assert!(component::TAB_H >= control::ROW);
        assert!(component::TABBAR_H > component::TAB_H);
        assert!(component::TABBAR_H - component::TAB_H >= 4.0);
        assert!(component::TAB_CLOSE < component::TAB_H);
        assert!(component::TAB_MIN_W < component::TAB_MAX_W);
        assert!(component::TAB_MIN_W >= component::TAB_CLOSE + 12.0 + component::TAB_INSET * 2.0);
        assert!(component::TAB_MAX_W <= 240.0, "标签再宽会挤掉邻页");
        assert!(
            component::TAB_RADIUS < component::TAB_H * 0.5,
            "圆角不能吃掉直边"
        );
        assert_eq!(component::TAB_RADIUS, 6.0, "X19 验收口径：顶圆角 6");
    }

    /// 暗幕必须真的遮住背景，又不能黑到看不见底下的形状。
    #[test]
    fn scrim_is_dim_but_not_opaque() {
        assert!(scrim::ALPHA >= 64, "暗幕太浅，背景会和弹窗抢注意力");
        assert!(scrim::ALPHA <= 120, "暗幕太重，像整屏黑掉");
    }

    /// 圆角七档必须严格递增：出现「两个名字一个值」就说明有人在凑数。
    #[test]
    fn radius_scale_is_strictly_ordered() {
        let scale = [
            radius::XS,
            radius::SM,
            radius::CONTROL,
            radius::MD,
            radius::PANEL,
            radius::LG,
            radius::DIALOG,
        ];
        for w in scale.windows(2) {
            assert!(w[1] > w[0], "圆角必须严格递增：{} -> {}", w[0], w[1]);
        }
        assert!(scale.iter().all(|r| *r >= 0.0));
    }

    /// 阴影从面板到对话框逐级变重：模糊与不透明度都不能倒退。
    #[test]
    fn elevation_gets_heavier_toward_the_top_layer() {
        let layers = [
            elevation::PANEL,
            elevation::WINDOW,
            elevation::POPOVER,
            elevation::DIALOG,
        ];
        for w in layers.windows(2) {
            assert!(w[1].blur > w[0].blur, "阴影模糊没有继续变重");
            assert!(w[1].alpha > w[0].alpha, "阴影不透明度没有继续变重");
            assert!(w[1].offset[1] >= w[0].offset[1]);
        }
    }

    /// 缓动曲线端点固定、全程单调、不反弹；减少动画时时长直接归零。
    #[test]
    fn motion_easing_is_predictable_and_reducible() {
        assert_eq!(motion::ease_out_cubic(0.0), 0.0);
        assert_eq!(motion::ease_out_cubic(1.0), 1.0);
        // 越界输入按端点钳制，不产生负数或反弹
        assert_eq!(motion::ease_out_cubic(-1.0), 0.0);
        assert_eq!(motion::ease_out_cubic(2.0), 1.0);
        let mut prev = 0.0;
        for i in 0..=100 {
            let v = motion::ease_out_cubic(i as f32 / 100.0);
            assert!(v >= prev, "缓动曲线在第 {i} 步倒退");
            assert!((0.0..=1.0).contains(&v));
            prev = v;
        }
        assert!((motion::ease_out_cubic(0.5) - 0.875).abs() < 1e-6);
        assert_eq!(motion::duration(motion::BASE, true), 0.0);
        assert_eq!(motion::duration(motion::BASE, false), motion::BASE);
        assert_eq!(motion::duration(-1.0, false), 0.0, "负时长钳到 0");
        assert!(motion::SCENE_MIN <= motion::SCENE_MAX);
    }

    #[test]
    fn content_type_scale_is_monotonic_and_uses_three_weights() {
        let mut sizes: Vec<f32> = content::TYPE_SCALE.iter().map(|t| t.size).collect();
        sizes.sort_by(|a, b| a.partial_cmp(b).expect("字号不会是 NaN"));
        for pair in sizes.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "内容字阶必须单调：{} -> {}",
                pair[0],
                pair[1]
            );
        }
        assert!(sizes[0] >= 12.0, "内容正文不应低于 12px");
        assert!(*sizes.last().unwrap() <= 30.0, "内容标题不应超过 30px");

        for style in content::TYPE_SCALE {
            let ratio = style.ratio();
            assert!(
                (1.30..=1.80).contains(&ratio),
                "行高比例 {ratio} 越界（字号 {}）",
                style.size
            );
            assert!(
                matches!(style.weight, 400 | 500 | 600),
                "内容字阶只允许 400/500/600，出现 {}",
                style.weight
            );
            assert!(style.line_height > style.size);
        }

        // 与 docs/28 §4.1 逐项钉住，改字阶必须同时改契约。
        let expected = [
            (content::PAGE_TITLE, 30.0, 42.0, 600),
            (content::OUTLINE_TITLE, 28.0, 40.0, 600),
            (content::HEADING_1, 22.0, 33.0, 600),
            (content::HEADING_2, 18.0, 28.0, 600),
            (content::HEADING_3, 16.0, 26.0, 600),
            (content::OUTLINE_ROW, 16.0, 26.0, 400),
            (content::READING_BODY, 16.0, 28.0, 400),
            (content::LIST_TITLE, 15.0, 22.0, 600),
            (content::LIST_EXCERPT, 13.0, 19.0, 400),
            (content::LIST_META, 12.0, 18.0, 400),
            (content::NAV, 14.0, 21.0, 400),
            (content::META, 12.0, 18.0, 400),
            (content::TAG, 12.0, 18.0, 600),
            (content::CODE, 14.0, 22.0, 400),
        ];
        for (style, size, line_height, weight) in expected {
            assert_eq!(style.size, size);
            assert_eq!(style.line_height, line_height);
            assert_eq!(style.weight, weight);
        }
    }

    #[test]
    fn content_measures_centre_a_bounded_column() {
        for measure in [content::OUTLINE, content::CARD, content::PREVIEW] {
            assert!(measure.page_max > measure.text_max);
            assert!(measure.text_max >= 600.0);
            assert!(measure.pad_top >= 32.0 && measure.pad_bottom >= 64.0);
            assert!(measure.narrow_side == 24.0 || measure.narrow_side == 32.0);
        }
        assert_eq!(content::OUTLINE.page_inset(), 20.0);
        assert_eq!(content::CARD.page_inset(), 40.0);
        assert_eq!(content::PREVIEW.page_inset(), 40.0);

        for (measure, narrow_side) in [
            (content::OUTLINE, 32.0),
            (content::CARD, 24.0),
            (content::PREVIEW, 32.0),
        ] {
            let wide = measure.column(1000.0);
            assert_eq!(wide.page, measure.page_max);
            assert_eq!(wide.text, measure.text_max);
            assert_eq!(wide.inset, measure.page_inset());
            assert_eq!(wide.side, (1000.0 - measure.page_max) * 0.5);

            let narrow = measure.column(700.0);
            assert_eq!(narrow.side, narrow_side);
            assert_eq!(narrow.page, 700.0 - 2.0 * narrow_side);
            assert!(narrow.text <= measure.text_max);
            assert!(narrow.text <= narrow.page);

            let empty = measure.column(0.0);
            assert_eq!(
                (empty.side, empty.page, empty.text, empty.inset),
                (0.0, 0.0, 0.0, 0.0)
            );

            let invalid = measure.column(f32::NAN);
            assert_eq!((invalid.side, invalid.page, invalid.text), (0.0, 0.0, 0.0));
        }

        // 宽窗只夹上限，不把页面拉满整个窗口。
        let wide = content::OUTLINE.column(1400.0);
        assert_eq!(wide.page, 800.0);
        assert_eq!(wide.side, 300.0);
    }

    /// CER-31 F1/F4：卡片页必须走 `content::CARD.column`，不再自写
    /// 「窗口 ≥1040 就 720」的公式。三个断点把「正文 640」钉死：
    /// 700（窄窗，左右 24 + inset 6）、900、1200（宽窗，版心 720 / 正文 640）。
    #[test]
    fn card_column_keeps_the_contract_text_width_at_every_breakpoint() {
        let narrow = content::CARD.column(700.0);
        assert_eq!(
            (narrow.side, narrow.page, narrow.text),
            (24.0, 652.0, 640.0)
        );
        assert_eq!(narrow.inset, 6.0);

        for total in [900.0, 1200.0] {
            let c = content::CARD.column(total);
            assert_eq!((c.page, c.text, c.inset), (720.0, 640.0, 40.0));
            assert_eq!(c.side, (total - 720.0) * 0.5);
        }

        // 比 640 还窄的窗口：正文跟着版心收，不能溢出。
        let tiny = content::CARD.column(500.0);
        assert_eq!(
            (tiny.side, tiny.page, tiny.text, tiny.inset),
            (24.0, 452.0, 452.0, 0.0)
        );
    }

    #[test]
    fn content_alphas_match_the_contract() {
        let light = content::LIGHT_ALPHAS;
        let dark = content::DARK_ALPHAS;
        assert_eq!((light.text_secondary, light.text_faint), (0.70, 0.68));
        assert_eq!((light.hover, light.selected), (0.04, 0.07));
        assert_eq!((light.nav_selected, light.editing), (0.04, 0.03));
        assert_eq!((light.hairline, light.divider), (0.08, 0.06));

        assert_eq!((dark.text_secondary, dark.text_faint), (0.72, 0.68));
        assert_eq!((dark.hover, dark.selected), (0.06, 0.12));
        assert_eq!((dark.nav_selected, dark.editing), (0.08, 0.08));
        assert_eq!((dark.hairline, dark.divider), (0.12, 0.10));

        for pair in [
            (dark.text_secondary, light.text_secondary),
            (dark.text_faint, light.text_faint),
        ] {
            assert!(pair.0 >= pair.1, "暗色文字透明度不应低于亮色");
            assert!((0.4..=0.8).contains(&pair.0));
        }
        for pair in [
            (dark.hover, light.hover),
            (dark.selected, light.selected),
            (dark.nav_selected, light.nav_selected),
            (dark.editing, light.editing),
            (dark.hairline, light.hairline),
            (dark.divider, light.divider),
        ] {
            assert!(pair.0 >= pair.1, "暗色状态透明度不应低于亮色");
            assert!((0.0..=0.2).contains(&pair.0));
        }
    }

    #[test]
    fn content_colors_keep_text_readable_on_all_themes() {
        use crate::theme::{themes, Appearance};

        let themes = themes();
        assert!(themes.len() >= 4, "内容令牌至少要覆盖四套内置主题");
        for theme in &themes {
            let c = content::ContentColors::for_theme(theme);
            for surface in [c.page, c.panel] {
                let primary = content::composite(c.text_primary, surface);
                let secondary = content::composite(c.text_secondary, surface);
                let link = content::composite(c.link, surface);
                assert!(
                    primary.contrast_ratio(surface) >= 4.5,
                    "{} 正文对比度不足：{:.2}",
                    theme.id,
                    primary.contrast_ratio(surface)
                );
                assert!(
                    secondary.contrast_ratio(surface) >= 4.5,
                    "{} 次要文字对比度不足：{:.2}",
                    theme.id,
                    secondary.contrast_ratio(surface)
                );
                assert!(
                    link.contrast_ratio(surface) >= 4.5,
                    "{} 链接对比度不足：{:.2}",
                    theme.id,
                    link.contrast_ratio(surface)
                );
            }
            // 焦点环与强调色是 UI 组件，按 WCAG 非文本 3:1。
            assert!(
                c.accent.contrast_ratio(c.page) >= 3.0,
                "{} 焦点强调色与页面不足 3:1",
                theme.id
            );

            // 选中/悬停底上的文字仍要能读。
            let hover = content::composite(c.hover, c.page);
            let selected = content::composite(c.selected, c.page);
            let nav = content::composite(c.nav_selected, c.panel);
            for fill in [hover, selected] {
                assert!(
                    content::composite(c.text_primary, fill).contrast_ratio(fill) >= 4.5,
                    "{} 状态填充上的正文对比度不足",
                    theme.id
                );
                assert!(
                    content::composite(c.text_secondary, fill).contrast_ratio(fill) >= 4.5,
                    "{} 状态填充上的次要文字对比度不足",
                    theme.id
                );
            }
            assert!(
                content::composite(c.text_primary, nav).contrast_ratio(nav) >= 4.5,
                "{} 侧栏选中底上的正文对比度不足",
                theme.id
            );

            // faint 只做占位/禁用，必须比 secondary 更淡；这里防它被误当正文。
            assert!(c.text_faint.0[3] < c.text_secondary.0[3]);
            if theme.appearance == Appearance::Light {
                assert!(c.link.contrast_ratio(c.page) >= 4.5);
            }
            assert_eq!(c.tag_dots.len(), 6);
        }
    }

    #[test]
    fn content_metrics_snap_hairline_and_ignore_bad_dpi() {
        let theme = crate::theme::get_theme("snow");
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            let m = content::content_metrics(&theme, dpi);
            assert_eq!(m.dpi, dpi);
            assert!(
                (m.hairline * dpi - 1.0).abs() < 1e-6,
                "hairline 必须等于 1 物理像素"
            );
            assert_eq!(m.focus_ring, 1.5);
            assert_eq!(m.focus_ring_offset, 1.0);
            assert_eq!(m.focus_ring_alpha, 0.80);
            assert_eq!(m.scrollbar.width, 6.0);
            assert_eq!(m.scrollbar.thumb, 4.0);
            assert_eq!(m.scrollbar.radius, 8.0);
            assert_eq!(m.scrollbar.fade_in, 0.12);
            assert_eq!(m.scrollbar.idle_fade, 0.30);
            // 纯函数：同参数重复调用结果一致。
            assert_eq!(m, content::content_metrics(&theme, dpi));
        }
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let m = content::content_metrics(&theme, bad);
            assert_eq!(m.dpi, 1.0, "非法 DPI {bad} 应回退到 1.0");
            assert_eq!(m.hairline, 1.0);
        }
        let clamp = content::content_metrics(&theme, 10.0);
        assert_eq!(clamp.dpi, 4.0);
        assert_eq!(clamp.hairline, 0.25);
    }

    /// T1 零像素漂移的锚：`shell` primitive 的每一支都必须等于 2026-11-02 之前
    /// 散落在 `mw-shell` 里的那个字面量（颜色与 alpha 一起）。抄错一个色阶，
    /// 全量 golden 会差一片像素——这条测试要在门之前先把它拦住。
    #[test]
    fn shell_tokens_match_the_old_literals() {
        let colors: [(&str, crate::theme::Rgba, [u8; 4]); 44] = [
            ("light::PANEL", shell::light::PANEL, [255, 255, 255, 255]),
            (
                "light::PANEL2",
                shell::light::PANEL2,
                [0xF2, 0xF3, 0xF5, 255],
            ),
            (
                "light::TITLEBAR",
                shell::light::TITLEBAR,
                [255, 255, 255, 247],
            ),
            ("light::SEP", shell::light::SEP, [0xE8, 0xEA, 0xEE, 255]),
            (
                "light::BORDER",
                shell::light::BORDER,
                [0xE6, 0xE9, 0xED, 255],
            ),
            (
                "light::BORDER_HOT",
                shell::light::BORDER_HOT,
                [0xD8, 0xDD, 0xE4, 255],
            ),
            ("light::EDGE", shell::light::EDGE, [0x96, 0x96, 0x96, 255]),
            ("light::TEXT", shell::light::TEXT, [0x3E, 0x44, 0x4C, 255]),
            ("light::DIM", shell::light::DIM, [0x5C, 0x64, 0x6F, 255]),
            ("light::FAINT", shell::light::FAINT, [0x65, 0x6D, 0x78, 255]),
            ("light::HOVER", shell::light::HOVER, [0xF1, 0xF3, 0xF6, 255]),
            (
                "light::ICON_OFF",
                shell::light::ICON_OFF,
                [0xC6, 0xCB, 0xD2, 255],
            ),
            ("light::WARN", shell::light::WARN, [0xB4, 0x6A, 0x1E, 255]),
            ("dark::FAINT", shell::dark::FAINT, [0x8B, 0x92, 0x9B, 255]),
            ("dark::WARN", shell::dark::WARN, [0xE9, 0xA0, 0x5C, 255]),
            (
                "tone::STROKE_LIGHT",
                shell::tone::STROKE_LIGHT,
                [0xE0, 0xE4, 0xEA, 255],
            ),
            (
                "tone::HAIRLINE_LIGHT",
                shell::tone::HAIRLINE_LIGHT,
                [0xEC, 0xEF, 0xF3, 255],
            ),
            (
                "tone::HIT_BG_DARK",
                shell::tone::HIT_BG_DARK,
                [0x6B, 0x58, 0x1C, 255],
            ),
            (
                "tone::HIT_BG_LIGHT",
                shell::tone::HIT_BG_LIGHT,
                [0xFF, 0xE7, 0xA3, 255],
            ),
            (
                "tone::HIT_INK_DARK",
                shell::tone::HIT_INK_DARK,
                [0xFF, 0xE9, 0xA8, 255],
            ),
            (
                "tone::HIT_INK_LIGHT",
                shell::tone::HIT_INK_LIGHT,
                [0x4A, 0x35, 0x00, 255],
            ),
            (
                "tone::OFF_LIGHT",
                shell::tone::OFF_LIGHT,
                [212, 212, 212, 255],
            ),
            (
                "tone::CHIP_LIGHT",
                shell::tone::CHIP_LIGHT,
                [242, 242, 242, 255],
            ),
            ("CLOSE_HOT", shell::CLOSE_HOT, [0xC4, 0x2B, 0x1C, 255]),
            ("tag::BLUE", shell::tag::BLUE, [0x7A, 0xA2, 0xF7, 255]),
            ("tag::GREEN", shell::tag::GREEN, [0x9E, 0xCE, 0x6A, 255]),
            ("tag::YELLOW", shell::tag::YELLOW, [0xE0, 0xAF, 0x68, 255]),
            ("tag::RED", shell::tag::RED, [0xF7, 0x76, 0x8E, 255]),
            ("tag::PURPLE", shell::tag::PURPLE, [0xBB, 0x9A, 0xF7, 255]),
            ("tag::CYAN", shell::tag::CYAN, [0x7D, 0xCF, 0xFF, 255]),
            ("tag::ORANGE", shell::tag::ORANGE, [0xFF, 0x9E, 0x64, 255]),
            ("tag::TEAL", shell::tag::TEAL, [0x73, 0xDA, 0xCA, 255]),
            ("markdown::TEXT", shell::markdown::TEXT, [30, 30, 30, 255]),
            ("markdown::DIM", shell::markdown::DIM, [90, 90, 90, 255]),
            (
                "markdown::FAINT",
                shell::markdown::FAINT,
                [150, 150, 150, 255],
            ),
            (
                "markdown::ACCENT",
                shell::markdown::ACCENT,
                [40, 110, 220, 255],
            ),
            (
                "markdown::CODE_BG",
                shell::markdown::CODE_BG,
                [240, 240, 240, 255],
            ),
            ("stats::SCRIM", shell::stats::SCRIM, [24, 27, 31, 232]),
            ("stats::INK", shell::stats::INK, [0xE8, 0xEB, 0xEF, 255]),
            (
                "overlay::NOTES_BG",
                shell::overlay::NOTES_BG,
                [18, 21, 26, 228],
            ),
            (
                "overlay::NOTES_STROKE",
                shell::overlay::NOTES_STROKE,
                [255, 255, 255, 22],
            ),
            ("overlay::BAR_BG", shell::overlay::BAR_BG, [26, 30, 36, 238]),
            (
                "overlay::BAR_STROKE",
                shell::overlay::BAR_STROKE,
                [255, 255, 255, 20],
            ),
            (
                "inspector::WARN",
                shell::inspector::WARN,
                [0xC0, 0x5A, 0x4A, 255],
            ),
        ];
        assert_eq!(colors.len(), 44);
        for (name, actual, expected) in colors {
            assert_eq!(actual.0, expected, "shell::{name} 与旧字面量不一致");
        }

        let alphas: [(&str, u8, u8); 11] = [
            ("dark::ICON_OFF_ALPHA", shell::dark::ICON_OFF_ALPHA, 90),
            (
                "tone::SHADOW_DARK_ALPHA",
                shell::tone::SHADOW_DARK_ALPHA,
                120,
            ),
            (
                "tone::SHADOW_LIGHT_ALPHA",
                shell::tone::SHADOW_LIGHT_ALPHA,
                40,
            ),
            ("stats::SCRIM_ALPHA", shell::stats::SCRIM_ALPHA, 232),
            (
                "overlay::NOTES_BG_ALPHA",
                shell::overlay::NOTES_BG_ALPHA,
                228,
            ),
            (
                "overlay::NOTES_STROKE_ALPHA",
                shell::overlay::NOTES_STROKE_ALPHA,
                22,
            ),
            ("overlay::BAR_BG_ALPHA", shell::overlay::BAR_BG_ALPHA, 238),
            (
                "overlay::BAR_STROKE_ALPHA",
                shell::overlay::BAR_STROKE_ALPHA,
                20,
            ),
            (
                "overlay::BAR_SHADOW_ALPHA",
                shell::overlay::BAR_SHADOW_ALPHA,
                80,
            ),
            (
                "inspector::RING_WHITE_ALPHA",
                shell::inspector::RING_WHITE_ALPHA,
                18,
            ),
            (
                "inspector::RING_BLACK_ALPHA",
                shell::inspector::RING_BLACK_ALPHA,
                18,
            ),
        ];
        assert_eq!(alphas.len(), 11);
        for (name, actual, expected) in alphas {
            assert_eq!(actual, expected, "shell::{name} 与旧 alpha 不一致");
        }

        assert_eq!(icon::GLYPH_MEDIUM, 18.0);
        assert_eq!(icon::GLYPH_LARGE, 19.0);
    }
}
