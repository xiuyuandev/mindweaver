//! 视觉令牌与样式级联（自 ui/src/lib/themes.ts + scene.ts:resolveAll 移植，文档 02 §6、05 §3）。
//!
//! 颜色在核心侧就解析成 RGBA：画布与导出因此拿到同一份已判定结果，不必各自解析十六进制串。
//! 节点覆盖里非法的颜色串按「未设置」处理，回退到上一级——脏数据不该让一个节点变成黑的。

use crate::document::Document;
use crate::model::{DocMeta, StyleOverride, ThemeOverrides};

/// RGBA 颜色，0–255，sRGB 非预乘。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const TRANSPARENT: Self = Self([0, 0, 0, 0]);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// WCAG 2.1 相对亮度（sRGB 线性化后加权）。
    pub fn relative_luminance(self) -> f32 {
        fn channel(c: u8) -> f32 {
            let s = c as f32 / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.0[0]) + 0.7152 * channel(self.0[1]) + 0.0722 * channel(self.0[2])
    }

    /// WCAG 对比度，1.0–21.0。前景/背景顺序不影响结果。
    pub fn contrast_ratio(self, other: Self) -> f32 {
        let a = self.relative_luminance();
        let b = other.relative_luminance();
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// 解析 `#rgb` / `#rrggbb` / `#rrggbbaa`（可省略 `#`）；其余形式返回 `None`。
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        let byte = |i: usize, w: usize| u8::from_str_radix(&s[i..i + w], 16).ok();
        match s.len() {
            3 => Some(Self::rgb(
                byte(0, 1)? << 4 | byte(0, 1)?,
                byte(1, 1)? << 4 | byte(1, 1)?,
                byte(2, 1)? << 4 | byte(2, 1)?,
            )),
            6 => Some(Self::rgb(byte(0, 2)?, byte(2, 2)?, byte(4, 2)?)),
            8 => Some(Self::rgba(
                byte(0, 2)?,
                byte(2, 2)?,
                byte(4, 2)?,
                byte(6, 2)?,
            )),
            _ => None,
        }
    }

    /// 解析节点覆盖里的颜色字符串。
    pub fn parse_opt(s: &Option<String>) -> Option<Self> {
        s.as_deref().and_then(Self::parse)
    }

    /// 把 `accent` 按 `t`（0–1）混进自身，alpha 保持自身。
    ///
    /// 用途：二三级卡片跟着所属分支色做**极淡**染色（`t=0.07`）与描边染色（`t=0.20`），
    /// 让一条分支看起来是一个色系，而不是一排白卡片配一根彩色线。
    pub fn mix(self, accent: Rgba, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let ch = |i: usize| {
            let a = self.0[i] as f32;
            let b = accent.0[i] as f32;
            (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
        };
        Self([ch(0), ch(1), ch(2), self.0[3]])
    }

    /// sRGB → OKLCH（`h` 为 0–360 度，`c` 彩度、`l` 感知明度均 0–1 上下），alpha 不参与。
    ///
    /// 为什么不是 HSL：HSL 的 `L` 不是感知量——`#ffff00` 的 HSL 明度 0.5，相对亮度却是
    /// 0.757。按 HSL 转色相会让一批卡片整体变亮或变暗（实测一级卡片对比度从 9.57 漂到
    /// 6.46），而「换个强调色」绝不该顺手改掉对比度。OKLab 的 `L` 是感知明度，转色相时
    /// 相对亮度基本钉住。
    fn to_oklch(self) -> (f32, f32, f32) {
        // 整段在 f64 里算：下面这些常数取自 OKLab 推导的 f64 参考值，塞进 f32 只是把
        // 常数截短（`float has excessive precision` 三十几条警告就是这么来的），而截短
        // 带来的色相抖动会顺着「换强调色」传到每一张卡片上。
        let [r, g, b, _] = self.0;
        let (r, g, b) = (
            f64::from(srgb_to_linear(r)),
            f64::from(srgb_to_linear(g)),
            f64::from(srgb_to_linear(b)),
        );
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024611 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        let light = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
        let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
        let bb = 0.0259040371 * l + 0.7825920550 * m - 0.8088973420 * s;
        let hue = bb.atan2(a).to_degrees().rem_euclid(360.0);
        (hue as f32, (a * a + bb * bb).sqrt() as f32, light as f32)
    }

    /// OKLCH → sRGB。色相与感知明度是硬承诺，出界时只允许**彩度往回收**。
    fn from_oklch(hue: f32, chroma: f32, light: f32) -> Self {
        let light = light.clamp(0.0, 1.0);
        if let Some(c) = oklch_to_srgb(hue, chroma, light) {
            return c;
        }
        // 沿「同色相、同明度」这条半径二分最大可用彩度。逐通道钳位不行：它把色相和明度一起
        // 改掉——素白强调色（彩度 0.198）转到 60° 时钳位版本落到 46.2°，回收版本落在 60.2°，
        // 代价只是彩度从 0.198 收到 0.142。色相是用户挑的东西，明度是对比度的底子，都不能动。
        let (mut lo, mut hi) = (0.0f32, chroma);
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            if oklch_to_srgb(hue, mid, light).is_some() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        // 二分结束时 `lo` 必定是可行端点：起点 0 是纯灰，永远在色域内。
        oklch_to_srgb(hue, lo, light).expect("彩度回收后的落点必定在色域内")
    }

    /// 色相沿环转 `delta` 度，彩度与感知明度逐令牌保住。
    ///
    /// 彩度低于 `CHROMA_FLOOR` 的令牌视为无彩色，原样返回：灰没有色相，转它只会平白无故
    /// 冒出彩来，而且 8 bit 量化在近灰上能把色相抖出十几度，谈不上方向。
    fn hue_rotated(self, delta: f32) -> Self {
        let (h, c, l) = self.to_oklch();
        if c < CHROMA_FLOOR || delta.abs() < 1e-3 {
            return self;
        }
        let mut out = Self::from_oklch(h + delta, c, l);
        out.0[3] = self.0[3];
        out
    }

    /// OKLCH 色相（0–360 度）。
    pub fn hue(self) -> f32 {
        self.to_oklch().0
    }

    /// OKLCH 彩度；低于 `CHROMA_FLOOR` 按无彩色看待。
    pub fn chroma(self) -> f32 {
        self.to_oklch().1
    }
}

/// 彩度门槛：低于它的令牌当无彩色处理（不参与色相旋转）。
const CHROMA_FLOOR: f32 = 0.012;

/// OKLCH → 线性 sRGB；任一通道越界就判 `None`（留 1e-4 量化余量）。
/// 彩度为 0 时是纯灰，必定在色域内。
fn oklch_to_srgb(hue: f32, chroma: f32, light: f32) -> Option<Rgba> {
    // 与 `to_oklch` 同一个理由：参考矩阵是 f64 常数。
    let rad = f64::from(hue).to_radians();
    let (c, li) = (f64::from(chroma), f64::from(light));
    let (a, b) = (c * rad.cos(), c * rad.sin());
    let l = (li + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (li - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (li - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    let (mut r, mut g, mut bl) = (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    );
    const EPS: f64 = 1e-4;
    for v in [&mut r, &mut g, &mut bl] {
        if *v < -EPS || *v > 1.0 + EPS {
            return None;
        }
        *v = v.clamp(0.0, 1.0);
    }
    let ch = |v: f64| (linear_to_srgb(v as f32) * 255.0).round().clamp(0.0, 255.0) as u8;
    Some(Rgba::rgb(ch(r), ch(g), ch(bl)))
}

fn srgb_to_linear(v: u8) -> f32 {
    let s = v as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// 节点形状（文档 02 §4）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Rounded,
    Rect,
    Capsule,
    /// 只有一条下划线，没有填充与描边（XMind `org.xmind.topicShape.underline`，
    /// Classic II 这类主题的一级主题就是它）。
    Underline,
    /// 正椭圆（概念图的默认形状，`docs/30` §5.3 / §12 第 9 条）。
    ///
    /// 椭圆的「盒」由布局外扩得到（文本盒按 1.22×1.16 外扩），所以这里没有圆角概念；
    /// 渲染层用**圆角半径取负**表示「这个盒子画成椭圆」，见 `mw-render/src/shaders/rect.wgsl`。
    Ellipse,
}

impl Shape {
    pub fn from_name(s: &str) -> Self {
        match s {
            "rect" => Self::Rect,
            "capsule" => Self::Capsule,
            "underline" => Self::Underline,
            "ellipse" => Self::Ellipse,
            _ => Self::Rounded,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rounded => "rounded",
            Self::Rect => "rect",
            Self::Capsule => "capsule",
            Self::Underline => "underline",
            Self::Ellipse => "ellipse",
        }
    }

    /// 画出来是不是椭圆：命中判定、导出、布局外扩都要问这一句，
    /// 不要让各处各自 `== Shape::Ellipse` 一遍。
    pub fn is_ellipse(self) -> bool {
        matches!(self, Self::Ellipse)
    }
}

/// 明暗外观（壳层据此切换 Fluent 深色）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// 四级字号阶梯（DIP）：中心主题 28 / 一级 20 / 二级 15 / 三级及以下 14。
///
/// v6 起四套主题共用这一处：XMind 144 DPI 截图实测中心主题 ≈32 CSS px、一级主题
/// ≈18.7 CSS px，且只有中心主题加粗、分支是常规体（docs/13 §4.41）。调字号只改这里，
/// 不要让四份 `levels` 各自漂移。
///
/// （上面那句「实测 ≈32/≈18.7」是当年量 XMind 的原始读数；实际落地的令牌是
/// `design::text::CANVAS_*` 的 28/20/15/14——截图口径与 DIP 口径差着一点换算，
/// 别再拿 32/18/16/14 去对代码，那是这段注释漂移之前的旧值。）
pub const LEVEL_TYPE: [(f32, bool); 4] = [
    (crate::design::text::CANVAS_CENTER, true),
    (crate::design::text::CANVAS_MAIN, false),
    (crate::design::text::CANVAS_SUB, false),
    (crate::design::text::CANVAS_LEAF, false),
];

/// 一个层级的默认样式。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelStyle {
    pub fill: Rgba,
    pub text: Rgba,
    pub font_size: f32,
    /// 字重：true = Bold，false = Regular。
    pub bold: bool,
    pub shape: Shape,
    /// 层级默认描边；一级主题通常不描边（靠实色填充区分）。
    pub stroke: Rgba,
    pub stroke_width: f32,
}

/// 界面令牌（面板、边框、强调色、画布底色与点阵）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeUi {
    pub bg: Rgba,
    pub panel: Rgba,
    pub panel2: Rgba,
    pub text: Rgba,
    pub dim: Rgba,
    pub border: Rgba,
    pub accent: Rgba,
    pub accent_soft: Rgba,
    pub hover: Rgba,
    /// 画布底色。画布合成器的清屏色、SVG 的底 `<rect>`、PDF 的页背景都只读这一份。
    pub canvas_bg: Rgba,
    /// 点阵颜色；**alpha == 0 就是「这套主题不铺点阵」**（演示模式靠写透明关掉它）。
    ///
    /// 现状：四套内置主题里只有 `ink`（墨夜）铺点阵，三套亮色纸底是纯色——依据
    /// `docs/06` §2.1 的实拍结论表「画布背景 纯色近白，没有点阵」，那张表把
    /// 「点阵 24px、处处可见」明确标成被否掉的**旧实现**。
    pub dot: Rgba,
    /// 点阵格距，世界单位（随缩放）。见 [`CanvasDots`]。
    pub dot_spacing: f32,
    /// 点阵半径，CSS px（屏幕恒定）。见 [`CanvasDots`]。
    pub dot_radius: f32,
}

/// 点阵格距的基准（世界单位）。四套内置主题的令牌都用它——这是 M1b 以来画布的唯一一份几何；
/// 搬进主题是为了让主题**能**换（`docs/02` 「背景样式…随主题」），不是为了让四套长得不一样。
/// **不是** `docs/06` 里那个 24：那是被否掉的旧实现的读数。
pub const DOT_SPACING: f32 = 28.0;
/// 点半径的基准（CSS px，屏幕恒定）。导出按 1 CSS px = 1 世界单位画（= 100% 缩放的观感）。
pub const DOT_RADIUS: f32 = 1.1;

/// 画布点阵的**已判定结果**——画布合成器、SVG、PDF 三条派生路径都只认 [`Theme::canvas_dots`]。
///
/// 一点代两处：① 主题能不能画点阵（`None` = 不画）；② 画成什么样。这样「底色/点阵同源」
/// 不靠三处各自读三个字段来保证，而是靠它们读同一个返回值。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasDots {
    /// 格距，世界单位（随缩放）。
    pub spacing: f32,
    /// 半径，CSS px（屏幕恒定；导出按 100% 缩放换算成矢量半径）。
    pub radius: f32,
    /// 颜色，alpha 就是令牌里的「透明度」。
    pub color: Rgba,
}

impl CanvasDots {
    /// 这套令牌画不画：alpha > 0 且格距/半径为正且有限。
    ///
    /// 脏主题（`0` / `-1` / `NaN`）一律当「不画」——宁可少一层背景，也不要在画布上铺出
    /// 一片 `NaN` 个点或者糊成一整块的颜色。
    pub fn is_on(self) -> bool {
        self.color.0[3] > 0
            && self.spacing.is_finite()
            && self.spacing > 0.0
            && self.radius.is_finite()
            && self.radius > 0.0
    }

    /// 把点色按自身 alpha 叠到 `bg` 上，返回**不透明**结果。
    ///
    /// 画布（gamma 空间预乘混）、SVG（查看器混）、PDF（没有透明度，只能先混成实色）三处
    /// 算的必须是同一个值，否则同一套令牌在三张图里会是三种灰。
    pub fn blended(self, bg: Rgba) -> Rgba {
        let a = self.color.0[3] as f32 / 255.0;
        let ch = |i: usize| {
            (bg.0[i] as f32 * (1.0 - a) + self.color.0[i] as f32 * a)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Rgba::rgb(ch(0), ch(1), ch(2))
    }

    /// 画成矢量时用的半径：导出没有「当前缩放」，按 1 CSS px = 1 世界单位。
    /// 半径超过半格时钳到 `spacing/2` 内，否则相邻的点会连成一片。
    pub fn vector_radius(self) -> f32 {
        self.radius.min(self.spacing * 0.5 - 0.01).max(0.01)
    }
}

/// 一套主题。
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub appearance: Appearance,
    pub ui: ThemeUi,
    /// 分支色循环表（深度 1 的兄弟节点按序号取色）
    pub palette: Vec<Rgba>,
    /// 一级主题的实色填充（与 `palette` 同序；亮色是提亮版，暗色是压暗版）
    pub palette_fill: Vec<Rgba>,
    /// 层级默认样式；比它更深的层级复用最后一档
    pub levels: Vec<LevelStyle>,
    /// 一级主题的文字取分支色。XMind 的 `mainTopic` 不写 `fo:color`，正文颜色跟着分支走；
    /// 打开后一级节点就是「彩色文字」，而不是一条彩线配一张灰卡。
    pub branch_text: bool,
    /// 一级主题是否有实色卡片。`false` 时一级节点只有文字（XMind Snowbrush 的默认外观），
    /// 显式填充覆盖仍然生效。
    pub main_card: bool,
    /// 分支线是否收细。XMind 的 `line-tapered: tapered`；M14 这类主题不写该属性，线是等粗的。
    pub taper: bool,
    pub edge_width: f32,
    pub edge_width_root: f32,
}

impl Theme {
    /// 第 `d` 层的默认样式；超出层数用最后一档。层表为空时退化为界面令牌，
    /// 这样主题数据被改坏也只是变朴素，不会 panic。
    pub fn level(&self, d: usize) -> LevelStyle {
        match self.levels.len() {
            0 => LevelStyle {
                fill: self.ui.panel,
                text: self.ui.text,
                font_size: LEVEL_TYPE[3].0,
                bold: LEVEL_TYPE[3].1,
                shape: Shape::Rounded,
                stroke: self.ui.border,
                stroke_width: 1.0,
            },
            len => self.levels[d.min(len - 1)],
        }
    }

    /// 画布点阵的唯一出口：`None` = 这套主题不铺点阵。
    ///
    /// **三条派生路径（画布合成器 / SVG / PDF）只能读这一句**，不许各自再去摸
    /// `ui.dot` / `ui.dot_spacing` / `ui.dot_radius`——那样「同源」就只剩口头约定。
    pub fn canvas_dots(&self) -> Option<CanvasDots> {
        let dots = CanvasDots {
            spacing: self.ui.dot_spacing,
            radius: self.ui.dot_radius,
            color: self.ui.dot,
        };
        dots.is_on().then_some(dots)
    }

    /// 一条父子连线的起止宽度（CSS px），子节点深度 `child_depth`。
    ///
    /// 2026 基准（XMind 主题里 `centralTopic.line-width=4`、`mainTopic.line-width=3`、
    /// 二级 2）：线从父端到子端收细，整条分支读起来像一根从根流向末梢的锥形笔画，
    /// 而不是一串等粗的管子。父端宽度取上一档，子端取本档。
    pub fn edge_widths(&self, child_depth: u32) -> (f32, f32) {
        let start = self.edge_width_at(child_depth.saturating_sub(1));
        let end = if self.taper {
            self.edge_width_at(child_depth)
        } else {
            start
        };
        (start, end)
    }

    /// 深度 `depth` 上的基准线宽：根 4 → 一级 3 → 二级 2 → 更深 1.5。
    fn edge_width_at(&self, depth: u32) -> f32 {
        match depth {
            0 => self.edge_width_root,
            1 => self.edge_width_root * 0.75,
            2 => self.edge_width,
            _ => (self.edge_width * 0.75).max(1.0),
        }
    }

    /// 第 `i` 个分支色（循环取色）。
    pub fn palette_at(&self, i: usize) -> Option<Rgba> {
        if self.palette.is_empty() {
            return None;
        }
        Some(self.palette[i % self.palette.len()])
    }

    /// 第 `i` 个一级主题填充色；缺失时回退到描边色表。
    pub fn palette_fill_at(&self, i: usize) -> Option<Rgba> {
        if self.palette_fill.is_empty() {
            return self.palette_at(i);
        }
        Some(self.palette_fill[i % self.palette_fill.len()])
    }

    /// 由一支显式分支色派生一级实色卡片：亮色向白提亮、暗色向面板底压暗，
    /// 与内置 `palette`→`palette_fill` 的手工关系同量级。用户只改分支色时，
    /// 卡片也跟着换色，而不是留下一条新色线配一张旧色卡。
    pub fn branch_fill(&self, branch: Rgba) -> Rgba {
        match self.appearance {
            Appearance::Light => Rgba::rgb(0xff, 0xff, 0xff).mix(branch, 0.5),
            Appearance::Dark => self.ui.panel.mix(branch, 0.2),
        }
    }

    /// 自定义强调色：只取用户挑的那支颜色的**色相**，把强调色、软强调色与两张分支色表
    /// 一起沿色相环**刚体旋转**到那个色相。
    ///
    /// 为什么只认色相：逐令牌保住自己的彩度与感知明度，等于卡片文字与底色的对比度**基本没动**
    /// ——挑到 `#ffff00` 也不会得到一堆读不清的黄卡片；而且切主题时同一份覆盖自动落到新主题
    /// 自己的明度上，不必为四套主题各存一支颜色。
    /// 无彩色（彩度低于门槛）没有色相可用，按「未设置」处理；与原强调色同色相时恒等返回。
    pub fn with_accent(&self, accent: Option<Rgba>) -> Theme {
        let Some(accent) = accent.filter(|c| c.chroma() >= CHROMA_FLOOR) else {
            return self.clone();
        };
        self.accented_to(accent.hue())
    }

    /// 文档级微调：只改用户点名的那几个槽位，其余保持主题自己的值。
    ///
    /// 非法颜色串按「没设置」处理——一份坏 meta 不该把画布变黑。
    /// 一级卡有两个来源（主题的 `palette_fill` 与层级默认填充），所以两边一起改，
    /// 否则「换了一级卡颜色」在无卡片主题上会毫无反应。
    pub fn with_overrides(&self, o: Option<&ThemeOverrides>) -> Theme {
        let Some(o) = o else {
            return self.clone();
        };
        let mut out = self.clone();
        if let Some(c) = Rgba::parse_opt(&o.canvas_bg) {
            out.ui.canvas_bg = c;
        }
        if let Some(c) = Rgba::parse_opt(&o.central_fill) {
            if let Some(level) = out.levels.get_mut(0) {
                level.fill = c;
            }
        }
        if let Some(c) = Rgba::parse_opt(&o.level1_fill) {
            if let Some(level) = out.levels.get_mut(1) {
                level.fill = c;
            }
            out.palette_fill = vec![c; out.palette_fill.len().max(1)];
        }
        // 文字色的优先级：分层字段 > 老的整体字段 > 主题原值。
        // 先整体刷一遍，再让分层字段覆盖——这样老文档里的 text 仍然生效。
        if let Some(c) = Rgba::parse_opt(&o.text) {
            for level in &mut out.levels {
                level.text = c;
            }
        }
        if let Some(c) = Rgba::parse_opt(&o.text_central) {
            if let Some(level) = out.levels.get_mut(0) {
                level.text = c;
            }
        }
        if let Some(c) = Rgba::parse_opt(&o.text_main) {
            if let Some(level) = out.levels.get_mut(1) {
                level.text = c;
            }
        }
        if let Some(c) = Rgba::parse_opt(&o.text_sub) {
            for level in out.levels.iter_mut().skip(2) {
                level.text = c;
            }
        }
        if let Some(scale) = o.edge_scale {
            let scale = scale.clamp(0.4, 2.5);
            out.edge_width *= scale;
            out.edge_width_root *= scale;
        }
        // 分支线颜色来自调色板（每支一种），所以「统一线色」= 把调色板刷成一个颜色。
        // 节点填充用的是 palette_fill，不受影响；带 branch_text 的主题文字会跟着变，
        // 这也是单色线主题应有的效果。
        if let Some(taper) = o.edge_taper {
            out.taper = taper;
        }
        if let Some(c) = Rgba::parse_opt(&o.edge_color) {
            out.palette = vec![c; out.palette.len().max(1)];
        }
        out
    }

    /// 这套主题把强调色放在色相 `hue` 上时**实际会长成什么样**——取色条要画的就是它，
    /// 而不是用户挑的那支原始颜色：覆盖只借色相，彩度与明度归主题自己，画布与色条才是同一个颜色。
    pub fn accent_at_hue(&self, hue: f32) -> Rgba {
        self.ui
            .accent
            .hue_rotated(hue_delta(self.ui.accent.hue(), hue))
    }

    /// 把整套配色刚体旋转到色相 `hue`（旋转量由强调色自己定，其余令牌跟着走同样的角度）。
    fn accented_to(&self, hue: f32) -> Theme {
        let delta = hue_delta(self.ui.accent.hue(), hue);
        if delta.abs() < 1e-3 {
            return self.clone();
        }
        let mut out = self.clone();
        out.ui.accent = out.ui.accent.hue_rotated(delta);
        out.ui.accent_soft = out.ui.accent_soft.hue_rotated(delta);
        for c in &mut out.palette {
            *c = c.hue_rotated(delta);
        }
        for c in &mut out.palette_fill {
            *c = c.hue_rotated(delta);
        }
        out
    }
}

/// 色相环上从 `from` 到 `to` 的最短弧（−180..=180）；终点色相相同，走哪条弧都一样，
/// 取短的只为让旋转量这个数字好读。
fn hue_delta(from: f32, to: f32) -> f32 {
    let d = (to - from).rem_euclid(360.0);
    if d > 180.0 {
        d - 360.0
    } else {
        d
    }
}

pub const THEME_IDS: [&str; 4] = ["snow", "ink", "mist", "sand"];

/// 纸感（亮）——**默认主题**。暖白纸底 + 墨色文字 + 发丝边卡片，长时间看不刺眼。
///
/// 三条硬指标（`design` 模块的单测看着它们）：
/// 1. 画布 **不是纯白**（暖白 `#FCFBF8`，WP2「纸感」的刻意取值），避免大面积高亮度眩光；
/// 2. 正文 #2A2E33 对画布与卡片都 >= 4.5:1；
/// 3. 卡片靠**发丝边**分层，不堆阴影——XMind 的卡片本来就是平的。
fn snow() -> Theme {
    Theme {
        id: "snow",
        name: "纸感",
        appearance: Appearance::Light,
        ui: ThemeUi {
            bg: Rgba::rgb(0xf4, 0xf1, 0xeb),
            panel: Rgba::rgb(0xfc, 0xfb, 0xf8),
            panel2: Rgba::rgb(0xf0, 0xec, 0xe5),
            text: Rgba::rgb(0x22, 0x26, 0x2a),
            dim: Rgba::rgb(0x64, 0x6d, 0x6a),
            border: Rgba::rgb(0xe7, 0xe2, 0xd9),
            accent: Rgba::rgb(0x2f, 0x6f, 0x5e),
            accent_soft: Rgba::rgb(0xe6, 0xf0, 0xea),
            hover: Rgba::rgb(0xf2, 0xee, 0xe7),
            canvas_bg: Rgba::rgb(0xfc, 0xfb, 0xf8),
            // 亮色纸底**不铺点阵**：XMind 2026 的画布是「纯色近白，没有点阵」
            // （`docs/06` §2.1 的实拍结论表；同表末尾还写明「旧文档里『点阵背景 8%』的
            // 示意已作废」）。令牌照样给足——开不开只由 `dot` 的 alpha 决定。
            dot: Rgba::TRANSPARENT,
            dot_spacing: DOT_SPACING,
            dot_radius: DOT_RADIUS,
        },
        // 同一色域里的八支：青绿 / 陶土 / 赭石 / 橄榄 / 靛蓝 / 紫檀 / 玫瑰 / 石板。
        // 饱和度统一压低，落在纸底上不跳。
        palette: vec![
            Rgba::rgb(0x3a, 0x7d, 0x6c),
            Rgba::rgb(0xc4, 0x64, 0x4e),
            Rgba::rgb(0xb9, 0x8a, 0x3e),
            Rgba::rgb(0x6f, 0x8f, 0x52),
            Rgba::rgb(0x5a, 0x6f, 0xa8),
            Rgba::rgb(0x8a, 0x5f, 0x86),
            Rgba::rgb(0xb4, 0x63, 0x7a),
            Rgba::rgb(0x6e, 0x7a, 0x85),
        ],
        palette_fill: vec![
            Rgba::rgb(0xa9, 0xcb, 0xbf),
            Rgba::rgb(0xeb, 0xc0, 0xb4),
            Rgba::rgb(0xe9, 0xd3, 0xa8),
            Rgba::rgb(0xc6, 0xd3, 0xb0),
            Rgba::rgb(0xc3, 0xcc, 0xe3),
            Rgba::rgb(0xd6, 0xc2, 0xd3),
            Rgba::rgb(0xe3, 0xc0, 0xcb),
            Rgba::rgb(0xcf, 0xd5, 0xdb),
        ],
        levels: vec![
            LevelStyle {
                fill: Rgba::rgb(0x2c, 0x66, 0x58),
                text: Rgba::rgb(0xff, 0xff, 0xff),
                font_size: LEVEL_TYPE[0].0,
                bold: LEVEL_TYPE[0].1,
                shape: Shape::Rounded,
                stroke: Rgba::TRANSPARENT,
                stroke_width: 0.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xff, 0xff, 0xff),
                text: Rgba::rgb(0x2a, 0x2e, 0x33),
                font_size: LEVEL_TYPE[1].0,
                bold: LEVEL_TYPE[1].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xeb, 0xe6, 0xdd),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xfd, 0xfc, 0xf9),
                text: Rgba::rgb(0x3a, 0x3f, 0x45),
                font_size: LEVEL_TYPE[2].0,
                bold: LEVEL_TYPE[2].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xef, 0xea, 0xe1),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xfd, 0xfc, 0xf9),
                text: Rgba::rgb(0x45, 0x4a, 0x50),
                font_size: LEVEL_TYPE[3].0,
                bold: LEVEL_TYPE[3].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xef, 0xea, 0xe1),
                stroke_width: 1.0,
            },
        ],
        branch_text: false,
        main_card: true,
        taper: true,
        edge_width: 1.75,
        edge_width_root: 3.5,
    }
}

/// 把主题调到**看得清**为止，返回修正后的主题。
///
/// 两条 WCAG 底线（`crates/mw-core/tests/contrast.rs` 盯着）：
/// 1. 分支线对画布/卡片 >= 3:1（非文本对比度）——浅黄浅绿当线条等于没有；
/// 2. 每层文字对自己的底色 >= 4.5:1——白字压在亮红/亮蓝上是不够的。
///
/// 修法是**按需压暗**（保持色相，逐次乘系数），不是换颜色：官方调色板的色相是对的，
/// 只是亮度偏高。压暗后主题仍然"像自己"，但不再刺眼地浅。
fn legible(mut t: Theme) -> Theme {
    let darken = |c: Rgba, k: f32| {
        Rgba::rgb(
            (c.0[0] as f32 * k).round().clamp(0.0, 255.0) as u8,
            (c.0[1] as f32 * k).round().clamp(0.0, 255.0) as u8,
            (c.0[2] as f32 * k).round().clamp(0.0, 255.0) as u8,
        )
    };
    let rgb = |c: Rgba| [c.0[0], c.0[1], c.0[2]];
    let opaque = |c: Rgba| c.0[3] > 8;
    let canvas = rgb(t.ui.canvas_bg);
    // 一级卡如果本身透明（雪枝那种"无卡片"主题），线其实画在画布上。
    let card = if opaque(t.levels[1].fill) {
        rgb(t.levels[1].fill)
    } else {
        canvas
    };
    for c in t.palette.iter_mut() {
        for _ in 0..24 {
            let v = rgb(*c);
            if crate::design::contrast(v, canvas) >= 3.0 && crate::design::contrast(v, card) >= 3.0
            {
                break;
            }
            *c = darken(*c, 0.93);
        }
    }
    // 每层文字：先试"压暗填充"（浅字压深底时这才对），不行再压暗文字本身。
    for i in 0..t.levels.len() {
        let fill = t.levels[i].fill;
        let text = t.levels[i].text;
        let bg = if opaque(fill) { rgb(fill) } else { canvas };
        if crate::design::contrast(rgb(text), bg) >= 4.5 {
            continue;
        }
        if opaque(fill) && crate::design::luminance(rgb(text)) > crate::design::luminance(bg) {
            for _ in 0..24 {
                let v = rgb(t.levels[i].fill);
                if crate::design::contrast(rgb(text), v) >= 4.5 {
                    break;
                }
                t.levels[i].fill = darken(t.levels[i].fill, 0.93);
            }
        } else {
            for _ in 0..24 {
                let v = rgb(t.levels[i].text);
                if crate::design::contrast(v, bg) >= 4.5 {
                    break;
                }
                t.levels[i].text = darken(t.levels[i].text, 0.9);
            }
        }
    }
    t
}

/// 墨夜（深色）：炭灰画布、柔白正文、琥珀强调。
fn ink() -> Theme {
    Theme {
        id: "ink",
        name: "墨夜",
        appearance: Appearance::Dark,
        ui: ThemeUi {
            bg: Rgba::rgb(0x1a, 0x1d, 0x21),
            panel: Rgba::rgb(0x20, 0x24, 0x2a),
            panel2: Rgba::rgb(0x18, 0x1b, 0x1f),
            text: Rgba::rgb(0xea, 0xe7, 0xe1),
            dim: Rgba::rgb(0xa0, 0xa6, 0xae),
            border: Rgba::rgb(0x32, 0x38, 0x40),
            accent: Rgba::rgb(0xd8, 0xa7, 0x5b),
            accent_soft: Rgba::rgb(0x3a, 0x30, 0x21),
            hover: Rgba::rgb(0x27, 0x2c, 0x33),
            canvas_bg: Rgba::rgb(0x17, 0x19, 0x1c),
            // 四套里**唯一**铺点阵的主题，且色值与观感保持 X21 之前原样
            // （`theme-ink` / `inspector-ink` / `library-ink` 等基线就钉着它）。
            dot: Rgba::rgb(0x3c, 0x42, 0x4a),
            dot_spacing: DOT_SPACING,
            dot_radius: DOT_RADIUS,
        },
        palette: vec![
            Rgba::rgb(0x7f, 0xb6, 0x9a),
            Rgba::rgb(0xe0, 0xa4, 0x6e),
            Rgba::rgb(0xd8, 0xc0, 0x6a),
            Rgba::rgb(0x8f, 0xaa, 0xd9),
            Rgba::rgb(0xc5, 0x8b, 0xb5),
            Rgba::rgb(0x9a, 0xa7, 0xb5),
        ],
        palette_fill: vec![
            Rgba::rgb(0x24, 0x33, 0x2c),
            Rgba::rgb(0x3b, 0x2c, 0x22),
            Rgba::rgb(0x39, 0x34, 0x1f),
            Rgba::rgb(0x24, 0x2c, 0x3b),
            Rgba::rgb(0x34, 0x26, 0x31),
            Rgba::rgb(0x2a, 0x2f, 0x37),
        ],
        levels: vec![
            LevelStyle {
                fill: Rgba::rgb(0xd8, 0xa7, 0x5b),
                text: Rgba::rgb(0x1a, 0x1a, 0x1a),
                font_size: LEVEL_TYPE[0].0,
                bold: LEVEL_TYPE[0].1,
                shape: Shape::Rounded,
                stroke: Rgba::TRANSPARENT,
                stroke_width: 0.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0x24, 0x28, 0x2e),
                text: Rgba::rgb(0xf2, 0xef, 0xe9),
                font_size: LEVEL_TYPE[1].0,
                bold: LEVEL_TYPE[1].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0x39, 0x40, 0x49),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0x1e, 0x22, 0x27),
                text: Rgba::rgb(0xdd, 0xd9, 0xd2),
                font_size: LEVEL_TYPE[2].0,
                bold: LEVEL_TYPE[2].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0x30, 0x36, 0x3e),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0x1a, 0x1e, 0x22),
                text: Rgba::rgb(0xcb, 0xc7, 0xc0),
                font_size: LEVEL_TYPE[3].0,
                bold: LEVEL_TYPE[3].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0x2c, 0x32, 0x39),
                stroke_width: 1.0,
            },
        ],
        branch_text: false,
        main_card: true,
        taper: true,
        edge_width: 1.6,
        edge_width_root: 3.2,
    }
}

/// 雾青（浅冷）：冷灰绿画布、石板正文、青绿强调。
fn mist() -> Theme {
    Theme {
        id: "mist",
        name: "雾青",
        appearance: Appearance::Light,
        ui: ThemeUi {
            bg: Rgba::rgb(0xef, 0xf4, 0xf1),
            panel: Rgba::rgb(0xf9, 0xfc, 0xfa),
            panel2: Rgba::rgb(0xe5, 0xed, 0xe9),
            text: Rgba::rgb(0x24, 0x30, 0x2d),
            dim: Rgba::rgb(0x5e, 0x6d, 0x69),
            border: Rgba::rgb(0xd5, 0xe0, 0xdb),
            accent: Rgba::rgb(0x2f, 0x7d, 0x76),
            accent_soft: Rgba::rgb(0xdc, 0xeb, 0xe7),
            hover: Rgba::rgb(0xe8, 0xf0, 0xec),
            canvas_bg: Rgba::rgb(0xf3, 0xf8, 0xf6),
            // 亮色纸底不铺点阵（同 `snow`）。
            dot: Rgba::TRANSPARENT,
            dot_spacing: DOT_SPACING,
            dot_radius: DOT_RADIUS,
        },
        palette: vec![
            Rgba::rgb(0x3e, 0x7f, 0x76),
            Rgba::rgb(0x5a, 0x72, 0x9e),
            Rgba::rgb(0x7b, 0x6a, 0x9a),
            Rgba::rgb(0x4f, 0x8a, 0x5e),
            Rgba::rgb(0x9a, 0x6b, 0x52),
            Rgba::rgb(0x6d, 0x7f, 0x8e),
        ],
        palette_fill: vec![
            Rgba::rgb(0xb6, 0xd6, 0xce),
            Rgba::rgb(0xc7, 0xd2, 0xe6),
            Rgba::rgb(0xd6, 0xca, 0xe2),
            Rgba::rgb(0xc6, 0xd9, 0xc8),
            Rgba::rgb(0xe4, 0xce, 0xc2),
            Rgba::rgb(0xd2, 0xda, 0xe0),
        ],
        levels: vec![
            LevelStyle {
                fill: Rgba::rgb(0x2f, 0x6f, 0x68),
                text: Rgba::rgb(0xff, 0xff, 0xff),
                font_size: LEVEL_TYPE[0].0,
                bold: LEVEL_TYPE[0].1,
                shape: Shape::Rounded,
                stroke: Rgba::TRANSPARENT,
                stroke_width: 0.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xfd, 0xff, 0xfe),
                text: Rgba::rgb(0x24, 0x30, 0x2d),
                font_size: LEVEL_TYPE[1].0,
                bold: LEVEL_TYPE[1].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xdc, 0xe6, 0xe2),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xf9, 0xfc, 0xfa),
                text: Rgba::rgb(0x34, 0x41, 0x3d),
                font_size: LEVEL_TYPE[2].0,
                bold: LEVEL_TYPE[2].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xe2, 0xeb, 0xe7),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xf7, 0xfb, 0xf9),
                text: Rgba::rgb(0x3d, 0x49, 0x45),
                font_size: LEVEL_TYPE[3].0,
                bold: LEVEL_TYPE[3].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xe5, 0xed, 0xe9),
                stroke_width: 1.0,
            },
        ],
        branch_text: false,
        main_card: true,
        taper: true,
        edge_width: 1.65,
        edge_width_root: 3.3,
    }
}

/// 暖砂（暖浅）：米砂画布、咖啡正文、陶土强调。
fn sand() -> Theme {
    Theme {
        id: "sand",
        name: "暖砂",
        appearance: Appearance::Light,
        ui: ThemeUi {
            bg: Rgba::rgb(0xf2, 0xeb, 0xdf),
            panel: Rgba::rgb(0xfb, 0xf7, 0xf0),
            panel2: Rgba::rgb(0xeb, 0xe2, 0xd4),
            text: Rgba::rgb(0x35, 0x2b, 0x24),
            dim: Rgba::rgb(0x71, 0x63, 0x58),
            border: Rgba::rgb(0xe0, 0xd3, 0xc2),
            // 4.46:1 的旧值压不住主按钮 AA；加深到 4.69:1，同时把色相留在原位
            // （色相漂移会让色条预视与应用结果差 3 个色阶）。
            accent: Rgba::rgb(0xa4, 0x5c, 0x43),
            accent_soft: Rgba::rgb(0xf1, 0xe0, 0xd5),
            hover: Rgba::rgb(0xf0, 0xe7, 0xdc),
            canvas_bg: Rgba::rgb(0xf8, 0xf2, 0xe9),
            // 亮色纸底不铺点阵（同 `snow`）。
            dot: Rgba::TRANSPARENT,
            dot_spacing: DOT_SPACING,
            dot_radius: DOT_RADIUS,
        },
        palette: vec![
            Rgba::rgb(0xa4, 0x5c, 0x43),
            Rgba::rgb(0x8a, 0x6d, 0x4f),
            Rgba::rgb(0x6f, 0x7f, 0x58),
            Rgba::rgb(0x8d, 0x5f, 0x6a),
            Rgba::rgb(0x5d, 0x70, 0x88),
            Rgba::rgb(0x7a, 0x6f, 0x5e),
        ],
        palette_fill: vec![
            Rgba::rgb(0xe8, 0xc8, 0xb8),
            Rgba::rgb(0xdf, 0xce, 0xb8),
            Rgba::rgb(0xd2, 0xda, 0xbe),
            Rgba::rgb(0xe0, 0xc4, 0xca),
            Rgba::rgb(0xc8, 0xd2, 0xe0),
            Rgba::rgb(0xd9, 0xd1, 0xc4),
        ],
        levels: vec![
            LevelStyle {
                fill: Rgba::rgb(0x8e, 0x4f, 0x38),
                text: Rgba::rgb(0xff, 0xf8, 0xf1),
                font_size: LEVEL_TYPE[0].0,
                bold: LEVEL_TYPE[0].1,
                shape: Shape::Rounded,
                stroke: Rgba::TRANSPARENT,
                stroke_width: 0.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xff, 0xfb, 0xf6),
                text: Rgba::rgb(0x35, 0x2b, 0x24),
                font_size: LEVEL_TYPE[1].0,
                bold: LEVEL_TYPE[1].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xe6, 0xd8, 0xc7),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xfc, 0xf7, 0xef),
                text: Rgba::rgb(0x4a, 0x3b, 0x31),
                font_size: LEVEL_TYPE[2].0,
                bold: LEVEL_TYPE[2].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xec, 0xe0, 0xd0),
                stroke_width: 1.0,
            },
            LevelStyle {
                fill: Rgba::rgb(0xfa, 0xf4, 0xeb),
                text: Rgba::rgb(0x53, 0x43, 0x38),
                font_size: LEVEL_TYPE[3].0,
                bold: LEVEL_TYPE[3].1,
                shape: Shape::Rounded,
                stroke: Rgba::rgb(0xef, 0xe3, 0xd4),
                stroke_width: 1.0,
            },
        ],
        branch_text: false,
        main_card: true,
        taper: true,
        edge_width: 1.7,
        edge_width_root: 3.4,
    }
}

/// 全部内置主题（顺序即设置面板顺序）。
pub fn themes() -> Vec<Theme> {
    vec![snow(), ink(), mist(), sand()]
        .into_iter()
        .map(legible)
        .collect()
}

/// 概念图专属主题（`docs/33` §4）。
///
/// 概念图不该借思维导图的主题——那是「另一种布局」的做法。这四套有自己的
/// 名字、自己的色板、自己的概念盒形状，与 `THEME_IDS` 那四套**没有任何共享色**。
///
/// | id | 语义 | 概念盒 | 关系线 | 命题 |
/// | --- | --- | --- | --- | --- |
/// | `cm-paper` | 白纸 | 椭圆，白底 1px 灰描边 | 1px `#9AA0A6` | 13px，胶囊白底 |
/// | `cm-slate` | 深色 | 圆角矩形，`#2B2F36` 底 | 1.5px `#6B7280` | 浅字，胶囊 `#2B2F36` |
/// | `cm-mint` | 柔和 | 椭圆，`#EAF6F0` 底，`#3E8E6E` 描边 | 1.5px `#3E8E6E` | `#2A6B52` |
/// | `cm-clay` | 暖色 | 圆角矩形，`#F7EFE8` 底，`#B0714E` 描边 | 1.5px `#B0714E` | `#8A5537` |
pub const CONCEPT_THEME_IDS: [&str; 4] = ["cm-paper", "cm-slate", "cm-mint", "cm-clay"];

/// 这套主题是不是概念图专属（`cm-` 前缀）。
///
/// 概念图专属主题把概念盒的形状/填色/描边**自己带进来**，
/// [`apply_concept_defaults`] 就不再覆盖这几项（只补字号与加粗）。
pub fn is_concept_theme(theme: &Theme) -> bool {
    theme.id.starts_with("cm-")
}

/// 取一套概念图专属主题；不认识的 id 退回 `cm-paper`（与 [`get_theme`] 同口径）。
pub fn concept_theme(id: &str) -> Theme {
    match id {
        "cm-slate" => ConceptSpec::slate().theme(),
        "cm-mint" => ConceptSpec::mint().theme(),
        "cm-clay" => ConceptSpec::clay().theme(),
        _ => ConceptSpec::paper().theme(),
    }
}

/// 一套概念图主题的**唯一差异点**：底色 / 描边 / 关系线 / 概念盒形状。
///
/// 抽成一个 struct 是为了让四套主题共用同一个 [`Theme`] 组装代码——
/// 少一处抄错，`CONCEPT_THEME_IDS` 加一套也就多一行。
struct ConceptSpec {
    id: &'static str,
    name: &'static str,
    appearance: Appearance,
    bg: Rgba,
    panel: Rgba,
    text: Rgba,
    dim: Rgba,
    border: Rgba,
    accent: Rgba,
    /// 概念盒形状（`cm-paper` / `cm-mint` 椭圆；`cm-slate` / `cm-clay` 圆角矩形）。
    shape: Shape,
    /// 关系线粗细（颜色取 [`Self::border`]）。
    edge_width: f32,
}

impl ConceptSpec {
    fn theme(self) -> Theme {
        let panel = self.panel;
        let border = self.border;
        let text = self.text;
        Theme {
            id: self.id,
            name: self.name,
            appearance: self.appearance,
            ui: ThemeUi {
                bg: self.bg,
                panel,
                panel2: panel.mix(self.bg, 0.35),
                text,
                dim: self.dim,
                border,
                accent: self.accent,
                accent_soft: self.accent.mix(panel, 0.85),
                hover: panel.mix(self.accent, 0.06),
                canvas_bg: self.bg,
                // 概念图是**纯纸**：不铺点阵（`docs/30` §5 的视觉语言里没有背景纹理这一项）。
                dot: Rgba::TRANSPARENT,
                dot_spacing: DOT_SPACING,
                dot_radius: DOT_RADIUS,
            },
            // 概念图没有「分支色循环」这一说：一颗概念就是一颗概念，
            // 整张图用主题自己的强调色。只留一支，避免 `palette` 又长出彩条。
            palette: vec![self.accent],
            palette_fill: vec![panel],
            // 层级只有一档：概念图没有树深度（`docs/33` §3）。
            levels: vec![LevelStyle {
                fill: panel,
                text,
                font_size: LEVEL_TYPE[3].0,
                bold: false,
                shape: self.shape,
                stroke: border,
                stroke_width: 1.0,
            }],
            branch_text: false,
            main_card: true,
            taper: false,
            edge_width: self.edge_width,
            edge_width_root: self.edge_width,
        }
    }

    fn paper() -> Self {
        Self {
            id: "cm-paper",
            name: "白纸",
            appearance: Appearance::Light,
            bg: Rgba::rgb(0xff, 0xff, 0xff),
            panel: Rgba::rgb(0xff, 0xff, 0xff),
            text: Rgba::rgb(0x24, 0x28, 0x2c),
            dim: Rgba::rgb(0x5f, 0x66, 0x6c),
            border: Rgba::rgb(0x9a, 0xa0, 0xa6),
            accent: Rgba::rgb(0x9a, 0xa0, 0xa6),
            edge_width: 1.0,
            shape: Shape::Ellipse,
        }
    }

    fn slate() -> Self {
        Self {
            id: "cm-slate",
            name: "石板",
            appearance: Appearance::Dark,
            bg: Rgba::rgb(0x1f, 0x22, 0x28),
            panel: Rgba::rgb(0x2b, 0x2f, 0x36),
            text: Rgba::rgb(0xe8, 0xea, 0xed),
            dim: Rgba::rgb(0x9a, 0xa0, 0xa6),
            border: Rgba::rgb(0x6b, 0x72, 0x80),
            accent: Rgba::rgb(0x8a, 0xb4, 0xd8),
            edge_width: 1.5,
            shape: Shape::Rounded,
        }
    }

    fn mint() -> Self {
        Self {
            id: "cm-mint",
            name: "薄荷",
            appearance: Appearance::Light,
            bg: Rgba::rgb(0xf7, 0xfc, 0xf9),
            panel: Rgba::rgb(0xea, 0xf6, 0xf0),
            text: Rgba::rgb(0x1f, 0x4d, 0x3c),
            dim: Rgba::rgb(0x4a, 0x77, 0x66),
            border: Rgba::rgb(0x3e, 0x8e, 0x6e),
            accent: Rgba::rgb(0x3e, 0x8e, 0x6e),
            edge_width: 1.5,
            shape: Shape::Ellipse,
        }
    }

    fn clay() -> Self {
        Self {
            id: "cm-clay",
            name: "陶土",
            appearance: Appearance::Light,
            bg: Rgba::rgb(0xfc, 0xf7, 0xf2),
            panel: Rgba::rgb(0xf7, 0xef, 0xe8),
            text: Rgba::rgb(0x6b, 0x3c, 0x22),
            dim: Rgba::rgb(0x9a, 0x6c, 0x4e),
            border: Rgba::rgb(0xb0, 0x71, 0x4e),
            accent: Rgba::rgb(0xb0, 0x71, 0x4e),
            edge_width: 1.5,
            shape: Shape::Rounded,
        }
    }
}

/// 四套概念图专属主题（顺序与 [`CONCEPT_THEME_IDS`] 一致）。
pub fn concept_themes() -> Vec<Theme> {
    CONCEPT_THEME_IDS
        .iter()
        .map(|id| concept_theme(id))
        .collect()
}

/// 这份文档该看到的主题列表（`docs/33` §4）。
///
/// 概念图看概念图那四套，其余看思维导图那四套——**两套不混排**。
/// 混排等于「专属」没做：用户在概念图里挑到一个纸感主题，结果概念盒
/// 又被 `apply_concept_defaults` 改回椭圆加面板底，两边对不上。
pub fn themes_for(doc: &Document) -> Vec<Theme> {
    if doc.is_concept_map() {
        concept_themes()
    } else {
        themes()
    }
}

/// 按 id 取主题；未知 id 回退素白（与旧版 CSS 变量兜底一致）。
pub fn get_theme(id: &str) -> Theme {
    // 概念图专属主题**原样返回**，不过 `legible`：那四套的色板是照着
    // 「概念盒 + 关系线 + 短语胶囊」这套搭配调的，`legible` 按正文可读性
    // 再拧一遍色相，会把「与思维导图零共享色」这条硬指标拧掉
    // （`concept_themes_do_not_share_tokens_with_mindmap_themes` 会红）。
    if CONCEPT_THEME_IDS.contains(&id) {
        return concept_theme(id);
    }
    legible(match id {
        "ink" => ink(),
        "mist" => mist(),
        "sand" => sand(),
        // 旧文档里的旧主题 id 一律回退到纸感；不再保留旧色板。
        _ => snow(),
    })
}

/// 文档实际生效的主题：内置主题 + 文档级强调色覆盖。
///
/// 凡是「从文档出发取主题」的地方都必须走这里；直接 `get_theme(&meta.theme)` 会静默丢掉
/// 强调色覆盖，撤销/重做与存盘往返也就跟着丢。
pub fn theme_for(meta: &DocMeta) -> Theme {
    get_theme(&meta.theme)
        .with_accent(Rgba::parse_opt(&meta.theme_accent))
        .with_overrides(meta.theme_overrides.as_ref())
}

/// 节点最终样式（级联已判定完毕，渲染层直接用）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedStyle {
    pub fill: Rgba,
    pub text: Rgba,
    pub bold: bool,
    pub italic: bool,
    pub font_size: f32,
    pub shape: Shape,
    pub stroke: Rgba,
    pub stroke_width: f32,
    pub branch_color: Rgba,
    /// 圆角半径覆盖（0–24）；`None` 按 `Shape` 的默认值（SE-13）。
    pub radius: Option<f32>,
    /// 文本水平对齐覆盖；`None` 表示左对齐（概念图另由 `center_text` 居中）。
    pub align: Option<TextAlign>,
    /// 生效字重：`weight` 覆盖优先，否则 `bold ? 600 : 400`（SE-13；`bold` 是 600 的别名，
    /// 理由见 `shots/x100/SE-13/report.md` §3.1——TYPO-01 的粗体字面本来就是 Semibold 600）。
    pub weight: u16,
}

/// 节点文字的水平对齐（SE-13）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// 字重档位（SE-13）。DWrite 的实际字面：Segoe UI 有 400 / 600 / 700，
/// 500 由系统按最近的可用字面合成，但**度量与绘制都走同一个 weight**，
/// 所以盒子宽度与字形永远一致。
///
/// `bold: true` 是旧文件（以及 `ThemeLevel.bold`）的兼容别名，等价 **600**：
/// TYPO-01 定下的画布粗体字面就是 Semibold 600，这里若取 700，
/// 所有既有基线都会漂一格——「缺键即老行为」这条不变量优先。
pub const WEIGHT_REGULAR: u16 = 400;
pub const WEIGHT_MEDIUM: u16 = 500;
pub const WEIGHT_SEMIBOLD: u16 = 600;
/// 旧 `bold` 别名映射到的字重。渲染层 `dwrite::WEIGHT_BOLD` 必须与它同值。
pub const WEIGHT_BOLD_ALIAS: u16 = WEIGHT_SEMIBOLD;

impl TextAlign {
    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

/// 样式级联：主题层级默认 → 分支色继承 → 节点覆盖（文档 05 §3）。
///
/// 分支色规则：主根取强调色；深度 1 的兄弟按序号循环取调色板；节点显式覆盖优先；
/// 其余继承父链上最近的分支色。根节点默认不描边（`stroke = 透明`）。
/// 结果按节点 id 索引；自由主题各自按深度 0 独立级联。
pub fn resolve_all(
    doc: &Document,
    theme: &Theme,
) -> std::collections::HashMap<String, ResolvedStyle> {
    // 每个节点一条结果：先按节点数预分配，省掉 20k 文档反复扩容的重哈希。
    // 键全是内部节点 id，用快速哈希（打磨轮 10：20k 一轮重建要查几万次）。
    let mut out = std::collections::HashMap::with_capacity(doc.nodes.len());
    let mut roots: Vec<&str> = vec![doc.root_id.as_str()];
    // 自由主题按 id 排序遍历：HashMap 的哈希顺序会随进程变化，不排序则结果不可复现
    let mut floats: Vec<&str> = doc
        .nodes
        .values()
        .filter(|n| n.parent_id.is_none() && n.id != doc.root_id)
        .map(|n| n.id.as_str())
        .collect();
    floats.sort_unstable();
    roots.extend(floats);

    // (id, 深度, 继承的文字色, 父链分支色, 兄弟序号)
    let mut stack: Vec<(&str, u32, Option<Rgba>, Rgba, usize)> = Vec::new();
    // 逆序入栈：栈是 LIFO，逆序后主根先出，遍历顺序与文档顺序一致
    for id in roots.into_iter().rev() {
        stack.push((id, 0, None, theme.ui.accent, 0));
    }
    // 没有覆盖样式的节点共用一份「全空」——以前每个节点都 `clone()` 一次
    // `Option<StyleOverride>`，带样式的文档（XMind 导入）因此每轮重建多克隆
    // 几万个 `String`（打磨轮 10：20k 全带样式 10.5 → 7.6ms）。
    let no_style = StyleOverride::default();
    while let Some((id, depth, inherited_text, parent_branch, sibling_idx)) = stack.pop() {
        let Some(n) = doc.node(id) else { continue };
        let st = n.style.as_ref().unwrap_or(&no_style);
        let StyleOverride {
            fill,
            text_color,
            stroke,
            stroke_width,
            bold,
            italic,
            font_size,
            shape,
            radius,
            align,
            weight,
            branch_color,
            apply_text_to_subtree,
        } = st;

        let mut branch = parent_branch;
        let own_branch = Rgba::parse_opt(branch_color);
        if id == doc.root_id {
            branch = theme.ui.accent;
        } else if let Some(c) = own_branch {
            branch = c;
        } else if depth == 1 {
            if let Some(c) = theme.palette_at(sibling_idx) {
                branch = c;
            }
        }

        let level = theme.level(depth as usize);
        let own_text = Rgba::parse_opt(text_color);
        let mut text = own_text.or(inherited_text).unwrap_or(level.text);
        // 一级主题的文字直接取分支色：XMind 的 mainTopic 不写 fo:color，
        // 这一层本来就没有卡片，颜色全靠「一条彩线 + 一行同色字」表达。
        if depth == 1 && theme.branch_text && own_text.is_none() && inherited_text.is_none() {
            text = branch;
        }
        // 「文字样式应用到子树」只在勾选时向下传递，否则子树回退层级默认
        let child_text = if own_text.is_some() && apply_text_to_subtree.unwrap_or(false) {
            own_text
        } else {
            inherited_text
        };

        // 一级主题用实色卡片（2026 基准）：填充取同序的提亮表，仍可被节点覆盖。
        // 显式分支色覆盖时，卡片由那支颜色派生——否则会出现「线换了色、卡还是旧色」。
        let level_fill = if depth == 1 {
            if !theme.main_card {
                // 一级没有卡片（Snowbrush 这类主题）：填充就是主题给的透明。
                level.fill
            } else if own_branch.is_some() {
                theme.branch_fill(branch)
            } else {
                theme.palette_fill_at(sibling_idx).unwrap_or(level.fill)
            }
        } else if depth >= 2 && theme.main_card {
            // 二三级卡片跟着分支色极淡染色：一条分支读成一个色系，而不是一排白卡。
            // 只染没有显式覆盖的节点（下面 `fill` 非空时仍以覆盖为准）。
            // 无卡片主题不染：Snowbrush 的二级卡是中性 #F8F7F7，染色会破坏它的克制。
            level.fill.mix(branch, 0.07)
        } else {
            level.fill
        };
        let level_stroke = if depth == 1 && theme.main_card {
            // 一级是实色卡：描边从卡片色里自己长出来。用主题的奶白描边会在彩色卡外
            // 勾一圈灰晕，像贴纸没撕干净；同色系深一档才像纸张压边。
            level_fill.mix(branch, 0.22)
        } else if depth >= 2 {
            level.stroke.mix(branch, 0.20)
        } else {
            level.stroke
        };
        let effective_bold = bold.unwrap_or(level.bold);
        out.insert(
            id.to_string(),
            ResolvedStyle {
                fill: Rgba::parse_opt(fill).unwrap_or(level_fill),
                text,
                bold: effective_bold,
                italic: italic.unwrap_or(false),
                font_size: font_size.map(f32::from).unwrap_or(level.font_size),
                shape: shape
                    .as_deref()
                    .map(Shape::from_name)
                    .unwrap_or(level.shape),
                stroke: Rgba::parse_opt(stroke).unwrap_or(level_stroke),
                stroke_width: stroke_width.unwrap_or(level.stroke_width),
                branch_color: branch,
                radius: radius.map(|r| r.clamp(0.0, 24.0)),
                align: align.as_deref().and_then(TextAlign::from_name),
                weight: weight
                    .map(|w| w.clamp(WEIGHT_REGULAR, 700))
                    .unwrap_or(if effective_bold {
                        WEIGHT_BOLD_ALIAS
                    } else {
                        WEIGHT_REGULAR
                    }),
            },
        );

        // 子节点逆序入栈，弹出即文档顺序；序号决定调色板取色
        for (i, c) in n.children.iter().enumerate().rev() {
            if out.contains_key(c.as_str()) {
                continue; // 已有解析结果 = 环或重复引用，不再下探
            }
            stack.push((c.as_str(), depth + 1, child_text, branch, i));
        }
    }
    if doc.meta.layout.kind == "conceptMap" {
        apply_concept_defaults(doc, theme, &mut out);
    }
    out
}

/// 概念图的节点样式：**浅色面板 + 1px 描边 + 14/20**（`docs/30` §5.3）。
///
/// 概念图不走「一级主题实色卡」那套级联——那会把 6 个概念画成 6 块深色实底，
/// 看上去还是思维导图。这里在级联之后统一覆盖默认值，但**仍尊重节点显式覆盖**：
/// 用户在检查器里选了填充色，就不该被这里的默认值盖掉。
///
/// **概念图专属主题例外**（`docs/33` §4）：`cm-*` 主题自己带了概念盒的
/// 形状/填色/描边，这里只补字号与加粗，不再覆盖那三项——否则「专属主题」
/// 就名不副实，换完主题椭圆还是椭圆、圆角还是圆角。
fn apply_concept_defaults(
    doc: &Document,
    theme: &Theme,
    out: &mut std::collections::HashMap<String, ResolvedStyle>,
) {
    /// 概念文字：14/20。
    const CONCEPT_FONT: f32 = 14.0;
    /// 主概念文字：15/22。
    const PRIMARY_FONT: f32 = 15.0;
    let own_look = is_concept_theme(theme);
    for (id, style) in out.iter_mut() {
        let override_ = doc.node(id).and_then(|n| n.style.as_ref());
        let primary = id == &doc.root_id;
        if !own_look && override_.and_then(|s| s.fill.as_ref()).is_none() {
            style.fill = if primary {
                // 主概念：8% 强调底，仍然是面板色系，不是实色块。
                theme.ui.panel.mix(theme.ui.accent, 0.08)
            } else {
                theme.ui.panel
            };
        }
        if override_.and_then(|s| s.text_color.as_ref()).is_none() {
            style.text = theme.ui.text;
        }
        if override_.and_then(|s| s.font_size.as_ref()).is_none() {
            style.font_size = if primary { PRIMARY_FONT } else { CONCEPT_FONT };
        }
        if override_.and_then(|s| s.bold.as_ref()).is_none() {
            style.bold = primary;
            // `bold` 只是 600 的别名，渲染/导出/SVG 只认 `weight`（SE-13）：主概念要换粗体，
            // 就得把 `weight` 一起搬过去，否则 `cm-*` 主题（level[0].bold = false）下中心概念
            // 会画成 400——而 PDF 伪粗/行内编辑器还按 `bold` 算，两处口径打架。
            if override_.and_then(|s| s.weight).is_none() {
                style.weight = if primary {
                    WEIGHT_BOLD_ALIAS
                } else {
                    WEIGHT_REGULAR
                };
            }
        }
        if !own_look && override_.and_then(|s| s.shape.as_ref()).is_none() {
            style.shape = Shape::Ellipse;
        }
        if !own_look && override_.and_then(|s| s.stroke.as_ref()).is_none() {
            style.stroke = theme.ui.border;
            style.stroke_width = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DocMeta;
    use crate::tfix::{simple, Builder};

    #[test]
    fn parse_color_accepts_three_forms() {
        assert_eq!(Rgba::parse("#fff"), Some(Rgba::rgb(0xff, 0xff, 0xff)));
        assert_eq!(Rgba::parse("4f6bff"), Some(Rgba::rgb(0x4f, 0x6b, 0xff)));
        assert_eq!(
            Rgba::parse("#11223344"),
            Some(Rgba::rgba(0x11, 0x22, 0x33, 0x44))
        );
        assert_eq!(Rgba::parse("red"), None);
        assert_eq!(Rgba::parse("#12g456"), None);
    }

    /// SE-13：`bold: true` 是 600 的别名（TYPO-01 的粗体字面），不是 700。
    /// 这条是「既有基线零漂移」的地基——取 700 会把所有粗体文字的宽度换掉。
    #[test]
    fn bold_is_a_semibold_alias_not_a_bold_seven_hundred() {
        let d = Builder::root("根")
            .styled(
                "r",
                "a",
                "甲",
                StyleOverride {
                    bold: Some(true),
                    ..Default::default()
                },
            )
            .styled(
                "r",
                "b",
                "乙",
                StyleOverride {
                    weight: Some(WEIGHT_MEDIUM),
                    ..Default::default()
                },
            )
            .child("r", "c", "丙")
            .build();
        let styles = resolve_all(&d, &snow());
        assert_eq!(
            styles["a"].weight, WEIGHT_BOLD_ALIAS,
            "旧 bold 必须读成 600"
        );
        assert_eq!(
            styles["b"].weight, WEIGHT_MEDIUM,
            "显式 weight 覆盖优先级更高"
        );
        assert_eq!(
            styles["c"].weight, WEIGHT_REGULAR,
            "缺键 = 常规 400（老行为）"
        );
        // 根主题是 bold=true 的层级默认，同样落到 600 而不是 700
        assert_eq!(styles["r"].weight, WEIGHT_BOLD_ALIAS);
    }

    /// SE-13：新键缺省 = 老行为（不改形状、不改对齐、圆角按形状默认）。
    #[test]
    fn the_new_style_keys_default_to_the_old_look() {
        let d = Builder::root("根").child("r", "a", "甲").build();
        let theme = snow();
        let s = resolve_all(&d, &theme)["a"];
        assert_eq!(s.radius, None, "缺键不该凭空给一个圆角");
        assert_eq!(
            s.align, None,
            "缺键仍是左对齐（概念图另由 center_text 居中）"
        );
        assert_eq!(s.shape, theme.level(1).shape);
    }

    /// SE-13：三个新键的取值范围——圆角夹 0–24、字重夹 400–700、对齐只认三个词。
    #[test]
    fn the_new_style_keys_are_clamped_and_parsed() {
        let d = Builder::root("根")
            .styled(
                "r",
                "a",
                "甲",
                StyleOverride {
                    radius: Some(99.0),
                    align: Some("CENTER".into()),
                    weight: Some(120),
                    ..Default::default()
                },
            )
            .styled(
                "r",
                "b",
                "乙",
                StyleOverride {
                    radius: Some(-4.0),
                    align: Some("middle".into()),
                    weight: Some(900),
                    ..Default::default()
                },
            )
            .build();
        let styles = resolve_all(&d, &snow());
        assert_eq!(styles["a"].radius, Some(24.0), "圆角上限 24");
        assert_eq!(
            styles["a"].align,
            Some(TextAlign::Center),
            "对齐大小写不敏感"
        );
        assert_eq!(styles["a"].weight, WEIGHT_REGULAR, "低于 400 夹到 400");
        assert_eq!(styles["b"].radius, Some(0.0), "负圆角夹到 0");
        assert_eq!(styles["b"].align, None, "不认识的对齐词当作没写（老行为）");
        assert_eq!(styles["b"].weight, 700, "高于 700 夹到 700");
    }

    #[test]
    fn every_theme_has_the_declared_shape() {
        for t in themes() {
            assert_eq!(t.levels.len(), 4);
            assert!(t.palette.len() >= 6, "{}：分支色至少要够六条", t.id);
            assert_eq!(
                t.palette_fill.len(),
                t.palette.len(),
                "{}：提亮表与分支色表必须同序同长",
                t.id
            );
            assert_eq!(t.levels[0].shape, Shape::Rounded);
            assert!(t.edge_width > 0.0, "{}：线宽必须为正", t.id);
            assert!(
                t.edge_width_root >= t.edge_width,
                "{}：根的线不该比末梢细",
                t.id
            );
        }
        assert_eq!(get_theme("nope").id, "snow");
        assert_eq!(get_theme("ink").appearance, Appearance::Dark);
    }

    /// 四套主题必须共用同一条字阶，否则换主题会顺带“换字号”。
    /// v6 依据 XMind 144 DPI 截图：只有中心主题加粗，一级及以下都是常规体。
    #[test]
    fn contrast_ratios_match_wcag() {
        let black = Rgba::rgb(0, 0, 0);
        let white = Rgba::rgb(255, 255, 255);
        assert!((black.contrast_ratio(white) - 21.0).abs() < 0.01);
        assert!((black.contrast_ratio(black) - 1.0).abs() < 0.001);
        // 顺序无关
        assert!((white.contrast_ratio(black) - 21.0).abs() < 0.01);
        assert!(Rgba::rgb(0x33, 0x33, 0x33).contrast_ratio(white) > 7.0);
    }

    /// 四套主题的外壳正文、辅助文字与主按钮都要过 WCAG AA。
    /// 主按钮的前景是 `ui.panel`（见 `ButtonTone::Primary`），不是固定白字：
    /// 墨夜主题的强调色是浅金，压白字只有 2.19:1，必须用深色面板字。
    #[test]
    fn theme_ui_contrast_meets_wcag_aa() {
        for t in themes() {
            let text_panel = t.ui.text.contrast_ratio(t.ui.panel);
            let text_bg = t.ui.text.contrast_ratio(t.ui.bg);
            let dim_panel = t.ui.dim.contrast_ratio(t.ui.panel);
            let primary = t.ui.panel.contrast_ratio(t.ui.accent);
            let accent_panel = t.ui.accent.contrast_ratio(t.ui.panel);
            assert!(text_panel >= 4.5, "{} text/panel {text_panel:.2}", t.id);
            assert!(text_bg >= 4.5, "{} text/bg {text_bg:.2}", t.id);
            assert!(dim_panel >= 4.5, "{} dim/panel {dim_panel:.2}", t.id);
            assert!(primary >= 4.5, "{} primary {primary:.2}", t.id);
            // 强调色还用于 2px 下划线、焦点环和选中环：非文字 UI 元素按 3:1。
            assert!(
                accent_panel >= 3.0,
                "{} accent/panel {accent_panel:.2}",
                t.id
            );
        }
    }

    #[test]
    fn edge_taper_override_can_flatten_branch_lines() {
        let base = get_theme("snow");
        let o = ThemeOverrides {
            edge_taper: Some(false),
            ..Default::default()
        };
        let flat = base.with_overrides(Some(&o));
        assert!(!flat.taper, "等粗线");
        assert!(base.taper, "主题默认是收细");
        let untouched = base.with_overrides(Some(&ThemeOverrides::default()));
        assert_eq!(untouched.taper, base.taper, "空覆盖不动主题");
    }

    #[test]
    fn every_theme_shares_one_type_ladder() {
        for t in themes() {
            for (i, level) in t.levels.iter().enumerate() {
                assert_eq!(
                    (level.font_size, level.bold),
                    LEVEL_TYPE[i],
                    "{} 第 {i} 层偏离 LEVEL_TYPE",
                    t.id
                );
            }
        }
        assert!(LEVEL_TYPE[0].1, "中心主题必须加粗");
        assert!(
            LEVEL_TYPE[1..].iter().all(|(_, bold)| !bold),
            "一级及以下必须是常规体"
        );
    }

    #[test]
    fn depth_one_children_cycle_the_palette() {
        let d = simple();
        let theme = snow();
        let styles = resolve_all(&d, &theme);
        assert_eq!(styles["a"].branch_color, theme.palette[0]);
        assert_eq!(styles["b"].branch_color, theme.palette[1]);
        assert_eq!(styles["c"].branch_color, theme.palette[2]);
        assert_eq!(styles["r"].branch_color, theme.ui.accent);
        // 根与一层的描边：根走层级；一级实色卡的描边由卡色派生（同色系深一档），
        // 不再用主题的奶白边——灰晕会让彩色卡像贴纸。
        assert_eq!(styles["r"].stroke, theme.levels[0].stroke);
        assert_eq!(
            styles["b"].stroke,
            theme.palette_fill[1].mix(theme.palette[1], 0.22)
        );
        assert!(
            theme.levels[1].stroke_width > 0.0,
            "一级卡要有描边，否则白卡贴在纸底上分不清"
        );
        assert_eq!(styles["a"].fill, theme.palette_fill[0]);
        assert_eq!(styles["b"].fill, theme.palette_fill[1]);
        assert_eq!(styles["b"].font_size, theme.levels[1].font_size);
        assert_eq!(styles["b1"].font_size, theme.levels[2].font_size);
    }

    #[test]
    fn ninth_sibling_wraps_back_to_palette_zero() {
        let mut d = Builder::root("根");
        for i in 0..9 {
            d = d.child("r", &format!("n{i}"), "叶");
        }
        let doc = d.build();
        let theme = snow();
        let styles = resolve_all(&doc, &theme);
        assert_eq!(styles["n0"].branch_color, theme.palette[0]);
        assert_eq!(styles["n8"].branch_color, theme.palette[0]);
    }

    #[test]
    fn grandchildren_inherit_branch_color() {
        let d = simple();
        let styles = resolve_all(&d, &snow());
        assert_eq!(styles["a1"].branch_color, styles["a"].branch_color);
    }

    /// 二三级卡片跟着分支色做极淡染色：一条分支读成一个色系，不再是一排白卡。
    #[test]
    fn sub_level_cards_take_a_hint_of_their_branch_color() {
        let d = simple();
        let theme = snow();
        let styles = resolve_all(&d, &theme);
        let s = &styles["a1"];
        assert_eq!(s.fill, theme.levels[2].fill.mix(s.branch_color, 0.07));
        assert_ne!(s.fill, theme.levels[2].fill, "不能还是纯白");
        assert_eq!(s.stroke, theme.levels[2].stroke.mix(s.branch_color, 0.20));
        // 一级主题是实色卡片，不参与这层染色
        assert_eq!(styles["a"].fill, theme.palette_fill[0]);
    }

    #[test]
    fn mix_interpolates_and_keeps_alpha() {
        let base = Rgba::rgb(255, 255, 255);
        let accent = Rgba::rgb(0, 0, 0);
        assert_eq!(base.mix(accent, 0.0), base);
        assert_eq!(base.mix(accent, 1.0), accent);
        assert_eq!(base.mix(accent, 0.5), Rgba::rgb(128, 128, 128));
        assert_eq!(Rgba::rgba(10, 20, 30, 128).mix(accent, 0.5).0[3], 128);
        // 越界比例按端点钳制，不产生回绕
        assert_eq!(base.mix(accent, -1.0), base);
        assert_eq!(base.mix(accent, 2.0), accent);
    }

    /// 色相环上的距离（0–180），绕圈不算差。
    fn hue_gap(a: f32, b: f32) -> f32 {
        let d = (a - b).abs() % 360.0;
        d.min(360.0 - d)
    }

    fn luminance(c: Rgba) -> f32 {
        let f = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.0[0]) + 0.7152 * f(c.0[1]) + 0.0722 * f(c.0[2])
    }

    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn an_out_of_gamut_hue_still_lands_on_the_angle_you_asked_for() {
        // 素白强调色彩度 0.198，转到 60° 落在 sRGB 色域外。逐通道钳位会把色相偷走 14°
        // （实测落到 46.2°）；沿半径回收彩度才落在 60.2°，代价只有彩度 0.198 → 0.142。
        // 用固定样本色，避免主题调色后这条色域测试跟着漂。
        let base = Rgba::rgb(0x3a, 0x7d, 0x6c).to_oklch();
        let snow = Theme {
            ui: ThemeUi {
                accent: Rgba::rgb(0x3a, 0x7d, 0x6c),
                ..snow().ui
            },
            ..snow()
        };
        let spun = snow.with_accent(Some(snow.accent_at_hue(60.0)));
        assert!(
            hue_gap(spun.ui.accent.hue(), 60.0) < 2.0,
            "色相被色域钳位偷走了：落点 {}",
            spun.ui.accent.hue()
        );
        assert!(
            (spun.ui.accent.to_oklch().2 - base.2).abs() < 0.01,
            "越界回收顺手改掉了明度"
        );
        assert!(
            spun.ui.accent.chroma() < snow.ui.accent.chroma(),
            "出界时彩度该往回收，不该硬钳"
        );
    }

    #[test]
    fn oklch_round_trips_the_colors_we_actually_ship() {
        // 正变换必须能还原逆变换：这条存在的原因是曾经 `linear_to_srgb` 返回 0–1，
        // 而写回 u8 时漏了乘 255，于是每个通道都被四舍五入成 0 或 1（浅绿卡片变成近黑）。
        let mut samples: Vec<Rgba> = Vec::new();
        for t in themes() {
            samples.extend(t.palette.iter().copied());
            samples.extend(t.palette_fill.iter().copied());
            samples.push(t.ui.accent);
            samples.push(t.ui.accent_soft);
            samples.push(t.levels[1].fill);
        }
        for hex in [
            "#ffffff", "#000000", "#808080", "#14181f", "#e5484d", "#9fedbc",
        ] {
            samples.push(Rgba::parse(hex).unwrap());
        }
        for c in samples {
            let (h, ch, l) = c.to_oklch();
            let back = Rgba::from_oklch(h, ch, l);
            // 贴着 sRGB 边界的颜色（XMind Colorful 的 #FEB40B 黄就坐在边界上）
            // 在 8 bit 量化下允许一个通道差 2：再往里收一格就偏离官方色板了。
            let tol = if ch >= 0.15 { 2 } else { 1 };
            for i in 0..3 {
                assert!(
                    (c.0[i] as i16 - back.0[i] as i16).abs() <= tol,
                    "{:?} 过了 OKLCH 一圈变成 {:?}（第 {i} 通道）",
                    c.0,
                    back.0
                );
            }
        }
        // 只测「会被转的色」：进 `from_oklch` 的永远是主题自己的令牌（中低彩度），用户那支颜色
        // 只贡献一个色相数字。纯 `#0000ff` 这种色域顶点不测——它坐在 sRGB 立方体的角上，
        // 正逆矩阵的常数精度差一点就会被判越界，彩度回收 0.01 就能让绿色通道从 0 涨到 48。
    }

    #[test]
    fn the_shortest_hue_arc_is_the_one_you_can_read() {
        // 350° → 10° 走 +20° 而不是 −340°：终点相同，但旋转量该好读。
        assert!((hue_delta(350.0, 10.0) - 20.0).abs() < 1e-4);
        assert!((hue_delta(10.0, 350.0) + 20.0).abs() < 1e-4);
        assert!(hue_delta(0.0, 180.0).abs() <= 180.0);
    }

    #[test]
    fn an_accent_override_spins_the_whole_palette() {
        for base in themes() {
            let spun = base.with_accent(Some(Rgba::parse("#e5484d").unwrap()));
            assert_eq!(spun.palette.len(), base.palette.len());
            assert_eq!(spun.palette_fill.len(), base.palette_fill.len());
            let shift = hue_gap(spun.ui.accent.hue(), base.ui.accent.hue());
            let check = |before: Rgba, after: Rgba, what: &str| {
                let (h0, c0, l0) = before.to_oklch();
                let (h1, c1, l1) = after.to_oklch();
                if c0 < CHROMA_FLOOR {
                    // 无彩色令牌没色相可转，必须原样留着（转出彩来就是脏）
                    assert_eq!(before, after, "{what} 是无彩色，不该被转动");
                    return;
                }
                // 8 bit 量化会把低彩度色的色相抖开，彩度越低允许的角度误差越大。
                let tol = if c0 >= 0.05 { 2.0 } else { 12.0 };
                assert!(
                    (hue_gap(h1, h0) - shift).abs() < tol,
                    "{what} 转的角度和强调色对不上：{}→{}（强调色转了 {shift:.1}°）",
                    h0,
                    h1
                );
                // 彩度是唯一可以让步的量：色相转到 sRGB 色域外时只能沿同一条半径往回收，
                // 但绝不允许反向变浓（那会把一套配色的浓淡关系搅乱）。
                assert!(c1 <= c0 + 0.005, "{what} 的彩度变浓了：{c0:.3} → {c1:.3}");
                assert!((l1 - l0).abs() < 0.01, "{what} 的感知明度动了");
            };
            for (i, (b, a)) in base.palette.iter().zip(&spun.palette).enumerate() {
                check(*b, *a, &format!("palette[{i}]"));
            }
            for (i, (b, a)) in base.palette_fill.iter().zip(&spun.palette_fill).enumerate() {
                check(*b, *a, &format!("palette_fill[{i}]"));
            }
            check(base.ui.accent_soft, spun.ui.accent_soft, "accent_soft");
            // 中性令牌一律不跟着跑：素白的冷灰底不该因为一个红强调色变成粉底。
            assert_eq!(base.ui.text, spun.ui.text);
            assert_eq!(base.ui.canvas_bg, spun.ui.canvas_bg);
            assert_eq!(base.ui.panel, spun.ui.panel);
            assert_eq!(base.levels[1].text, spun.levels[1].text);
        }
    }

    #[test]
    fn the_override_lands_on_the_hue_the_user_picked() {
        // 覆盖只借色相：落点必须是用户那支颜色的色相，容差留给 8 bit 量化。
        for base in themes() {
            let picked = Rgba::parse("#ff0000").unwrap();
            let spun = base.with_accent(Some(picked));
            assert!(
                hue_gap(spun.ui.accent.hue(), picked.hue()) < 2.0,
                "{} 的强调色落到了 {}",
                base.id,
                spun.ui.accent.hue()
            );
            // 色条上画的预视，必须就是画布最后真用的那支——不然预览就是骗人。
            for h in [0.0f32, 75.0, 150.0, 225.0, 300.0] {
                let chip = base.accent_at_hue(h);
                let real = base.with_accent(Some(chip)).ui.accent;
                assert!(
                    hue_gap(chip.hue(), h) < 2.0,
                    "色相 {h} 没落到位：{}",
                    chip.hue()
                );
                for i in 0..4 {
                    let diff = (chip.0[i] as i16 - real.0[i] as i16).abs();
                    assert!(
                        diff <= 1,
                        "{} 色相 {h} 的预视与实际强调色第 {i} 通道差 {diff}：{:?} vs {:?}",
                        base.id,
                        chip.0,
                        real.0
                    );
                }
            }
        }
    }

    #[test]
    fn no_override_the_own_hue_or_a_grey_are_all_identity() {
        for base in themes() {
            assert_eq!(base.with_accent(None), base, "没覆盖就该是原主题");
            assert_eq!(
                base.with_accent(Some(base.ui.accent)),
                base,
                "挑回自己的色相不该产生第二套颜色"
            );
            // 无彩色没有色相可用：忽略它，而不是把整套配色拽到 0°。
            assert_eq!(
                base.with_accent(Some(Rgba::rgb(128, 128, 128))),
                base,
                "灰色不是色相"
            );
        }
    }

    #[test]
    fn spinning_the_hue_keeps_card_contrast_where_it_was() {
        // 这条是整套「只认色相」设计的立身之本：明度逐槽位保住 ⇒ 卡片文字对比度不变。
        // 转一圈（36 个色相）而不是只挑一个颜色：黄、青这些高视见亮度色相才是会出事的地方。
        for base in themes() {
            let text = base.levels[1].text;
            for step in (0..360).step_by(10) {
                let spun = base.with_accent(Some(base.accent_at_hue(step as f32)));
                for (b, a) in base.palette_fill.iter().zip(&spun.palette_fill) {
                    let (before, after) = (contrast(*b, text), contrast(*a, text));
                    assert!(
                        (before - after).abs() < 1.0,
                        "色相 {step}° 下一级卡片对比度从 {before:.2} 漂到 {after:.2}"
                    );
                    // 主题原本能读，转完色相也必须还能读（WCAG 大号文本线）。
                    assert!(after >= 3.0, "色相 {step}° 把对比度压到了 {after:.2}");
                }
            }
        }
    }

    #[test]
    fn theme_for_applies_the_meta_accent() {
        let mut meta = DocMeta::default();
        assert_eq!(theme_for(&meta), get_theme("snow"));
        meta.theme_accent = Some("#e5484d".into());
        let expected = get_theme("snow").with_accent(Rgba::parse_opt(&meta.theme_accent));
        assert_eq!(theme_for(&meta), expected);
        // 脏数据按「未设置」处理：不该让整份文档变成黑的。
        meta.theme_accent = Some("tomato".into());
        assert_eq!(theme_for(&meta), get_theme("snow"));
    }

    #[test]
    fn node_override_wins_and_can_stain_subtree() {
        let d = Builder::root("根")
            .styled(
                "r",
                "a",
                "甲",
                StyleOverride {
                    text_color: Some("#e5484d".into()),
                    apply_text_to_subtree: Some(true),
                    font_size: Some(20),
                    branch_color: Some("#123456".into()),
                    ..Default::default()
                },
            )
            .child("a", "a1", "乙")
            .build();
        let styles = resolve_all(&d, &snow());
        assert_eq!(styles["a"].font_size, 20.0);
        assert_eq!(styles["a1"].text, styles["a"].text);
        assert_eq!(styles["a"].branch_color, Rgba::parse("#123456").unwrap());
        assert_eq!(styles["a1"].branch_color, styles["a"].branch_color);
    }

    #[test]
    fn an_explicit_branch_color_repaints_its_level_one_card() {
        let d = Builder::root("根")
            .styled(
                "r",
                "a",
                "甲",
                StyleOverride {
                    branch_color: Some("#e4635b".into()),
                    ..Default::default()
                },
            )
            .build();
        let theme = snow();
        let branch = Rgba::parse("#e4635b").unwrap();
        let styles = resolve_all(&d, &theme);
        assert_eq!(styles["a"].branch_color, branch);
        assert_eq!(styles["a"].fill, theme.branch_fill(branch));
        assert_ne!(
            styles["a"].fill, theme.palette_fill[0],
            "覆盖分支色后不该继续用主题预设卡片色"
        );
        // 暗色主题的派生卡片要朝面板底压暗，而不是把整张卡提亮。
        let dark = ink();
        let dark_styles = resolve_all(&d, &dark);
        assert_eq!(dark_styles["a"].fill, dark.branch_fill(branch));
    }

    #[test]
    fn text_override_without_subtree_flag_does_not_propagate() {
        let d = Builder::root("根")
            .styled(
                "r",
                "a",
                "甲",
                StyleOverride {
                    text_color: Some("#e5484d".into()),
                    ..Default::default()
                },
            )
            .child("a", "a1", "乙")
            .build();
        let theme = snow();
        let styles = resolve_all(&d, &theme);
        assert_eq!(styles["a1"].text, theme.levels[2].text);
    }

    #[test]
    fn invalid_override_color_falls_back_to_level() {
        let d = Builder::root("根")
            .styled(
                "r",
                "a",
                "甲",
                StyleOverride {
                    fill: Some("not-a-color".into()),
                    ..Default::default()
                },
            )
            .build();
        let theme = snow();
        assert_eq!(
            resolve_all(&d, &theme)["a"].fill,
            theme.palette_fill[0],
            "非法填充回退到一级实色，而不是层级白底"
        );
    }

    #[test]
    fn floating_topics_resolve_at_depth_zero() {
        let d = Builder::root("根")
            .float("f", "自由主题", Some((300.0, 0.0)))
            .under("f", "f1", "子")
            .build();
        let theme = snow();
        let styles = resolve_all(&d, &theme);
        assert_eq!(styles["f"].font_size, theme.levels[0].font_size);
        assert_eq!(styles["f"].stroke, theme.levels[0].stroke);
        assert_eq!(styles["f1"].font_size, theme.levels[1].font_size);
        assert_eq!(
            styles["f1"].stroke,
            theme.palette_fill[0].mix(theme.palette[0], 0.22)
        );
        assert_eq!(styles["f1"].fill, theme.palette_fill[0]);
    }

    #[test]
    fn deep_levels_clamp_to_the_last_style() {
        let d = Builder::root("根")
            .child("r", "a", "甲")
            .child("a", "b", "乙")
            .child("b", "c", "丙")
            .child("c", "e", "丁")
            .build();
        let theme = snow();
        let styles = resolve_all(&d, &theme);
        assert_eq!(styles["e"].font_size, theme.levels[3].font_size);
    }

    /// 文档级微调只改点名的槽位；没点名的保持主题原值，非法值当作没设置。
    #[test]
    fn theme_overrides_only_touch_the_named_slots() {
        let base = get_theme("snow");
        let o = ThemeOverrides {
            canvas_bg: Some("#fad8df".into()),
            central_fill: Some("#318de1".into()),
            level1_fill: None,
            ..Default::default()
        };
        let t = base.with_overrides(Some(&o));
        assert_eq!(t.ui.canvas_bg, Rgba::rgb(0xfa, 0xd8, 0xdf));
        assert_eq!(t.level(0).fill, Rgba::rgb(0x31, 0x8d, 0xe1));
        assert_eq!(t.level(2).fill, base.level(2).fill);
        assert_eq!(
            t.palette_fill, base.palette_fill,
            "没点名一级卡就不动调色板"
        );

        // 文字色覆盖所有层级；线宽倍率按钳位后的值乘到两端
        let text = ThemeOverrides {
            text: Some("#102030".into()),
            edge_scale: Some(2.0),
            ..Default::default()
        };
        let t3 = base.with_overrides(Some(&text));
        for d in 0..4 {
            assert_eq!(
                t3.level(d).text,
                Rgba::rgb(0x10, 0x20, 0x30),
                "第 {d} 层文字色"
            );
        }
        assert!((t3.edge_width - base.edge_width * 2.0).abs() < 1e-4);
        assert!((t3.edge_width_root - base.edge_width_root * 2.0).abs() < 1e-4);
        // 分层优先于整体
        let layered = ThemeOverrides {
            text: Some("#010203".into()),
            text_central: Some("#111111".into()),
            text_sub: Some("#222222".into()),
            ..Default::default()
        };
        let t4 = base.with_overrides(Some(&layered));
        assert_eq!(
            t4.level(0).text,
            Rgba::rgb(0x11, 0x11, 0x11),
            "中心用分层值"
        );
        assert_eq!(
            t4.level(1).text,
            Rgba::rgb(0x01, 0x02, 0x03),
            "一级没给分层值，落到整体值"
        );
        assert_eq!(
            t4.level(2).text,
            Rgba::rgb(0x22, 0x22, 0x22),
            "子级用分层值"
        );
        assert_eq!(t4.level(3).text, Rgba::rgb(0x22, 0x22, 0x22));

        // 统一线色 = 调色板刷成一个颜色，节点填充不动
        let mono = ThemeOverrides {
            edge_color: Some("#123456".into()),
            ..Default::default()
        };
        let t5 = base.with_overrides(Some(&mono));
        assert!(
            t5.palette.iter().all(|c| *c == Rgba::rgb(0x12, 0x34, 0x56)),
            "所有分支线都该是同一个颜色"
        );
        assert_eq!(t5.palette_fill, base.palette_fill, "填充色不该被线色带跑");

        let clamped = ThemeOverrides {
            edge_scale: Some(99.0),
            ..Default::default()
        };
        assert!(
            (base.with_overrides(Some(&clamped)).edge_width - base.edge_width * 2.5).abs() < 1e-4,
            "倍率钳在 2.5"
        );
        assert!(ThemeOverrides::default().is_empty(), "全空 = 没有微调");
        assert!(!ThemeOverrides {
            text: Some("#000000".into()),
            ..Default::default()
        }
        .is_empty());

        let lvl1 = ThemeOverrides {
            level1_fill: Some("#112233".into()),
            ..Default::default()
        };
        let t2 = base.with_overrides(Some(&lvl1));
        assert_eq!(t2.level(1).fill, Rgba::rgb(0x11, 0x22, 0x33));
        assert!(
            t2.palette_fill
                .iter()
                .all(|c| *c == Rgba::rgb(0x11, 0x22, 0x33)),
            "一级卡的另一个来源 palette_fill 也要跟着改"
        );

        let bad = ThemeOverrides {
            canvas_bg: Some("tomato".into()),
            ..Default::default()
        };
        assert_eq!(
            base.with_overrides(Some(&bad)).ui.canvas_bg,
            base.ui.canvas_bg,
            "非法颜色按没设置处理"
        );
        assert_eq!(base.with_overrides(None), base);
    }

    /// 不收细的主题每层只有一个宽度；收细的主题子端必须比父端细。
    #[test]
    fn taper_decides_whether_an_edge_narrows() {
        for t in themes() {
            for d in 1..5u32 {
                let (w0, w1) = t.edge_widths(d);
                if t.taper {
                    assert!(w1 <= w0, "{} 第 {d} 层：锥形线只能收不能涨", t.id);
                } else {
                    assert_eq!(w0, w1, "{} 第 {d} 层：等粗主题不该收细", t.id);
                }
            }
        }
    }

    #[test]
    fn meta_default_theme_is_snow_and_shape_names_round_trip() {
        let m = DocMeta::default();
        assert_eq!(m.theme, "snow");
        assert_eq!(m.layout.kind, "mindmap");
        for name in ["rounded", "rect", "capsule", "ellipse"] {
            assert_eq!(Shape::from_name(name).as_str(), name);
        }
        assert_eq!(Shape::from_name("???").as_str(), "rounded");
    }

    /// 概念图的默认形状是椭圆（`docs/30` §5.3）；思维导图不受影响，显式覆盖仍然优先。
    #[test]
    fn concept_maps_default_to_ellipses_and_mindmaps_do_not() {
        let mut doc = Builder::root("根")
            .child("r", "a", "概念甲")
            .styled(
                "r",
                "b",
                "概念乙",
                crate::model::StyleOverride {
                    shape: Some("capsule".to_string()),
                    ..Default::default()
                },
            )
            .build();
        let theme = snow();
        doc.meta.layout.kind = "conceptMap".to_string();
        let styles = resolve_all(&doc, &theme);
        assert_eq!(styles["r"].shape, Shape::Ellipse);
        assert_eq!(styles["a"].shape, Shape::Ellipse, "没覆盖的概念都是椭圆");
        assert_eq!(styles["b"].shape, Shape::Capsule, "显式覆盖不被默认值盖掉");

        doc.meta.layout.kind = "mindmap".to_string();
        let styles = resolve_all(&doc, &theme);
        assert_ne!(styles["r"].shape, Shape::Ellipse, "思维导图不能跟着变椭圆");
    }

    /// `docs/33` §4：概念图专属主题与思维导图主题**不共享任何色**。
    ///
    /// 「专属」不是换个名字——色板、概念盒形状都得是自己的，
    /// 否则换完主题看上去还是那套思维导图。
    #[test]
    fn concept_themes_do_not_share_tokens_with_mindmap_themes() {
        let mind: Vec<Rgba> = THEME_IDS
            .iter()
            .flat_map(|id| get_theme(id).palette)
            .collect();
        for id in CONCEPT_THEME_IDS {
            let t = concept_theme(id);
            for c in t.palette.iter().chain(t.palette_fill.iter()) {
                assert!(
                    !mind.contains(c),
                    "{id} 的色板色 {c:?} 出现在思维导图主题里——那不叫专属"
                );
            }
            assert_eq!(t.id, id, "主题 id 必须与请求一致");
        }
        assert_eq!(CONCEPT_THEME_IDS.len(), 4);
    }

    /// 概念盒形状由**概念图主题自己带**，[`apply_concept_defaults`] 不再覆盖。
    #[test]
    fn concept_themes_carry_their_own_shapes() {
        let mut doc = Builder::root("根")
            .child("r", "a", "概念甲")
            .child("r", "b", "概念乙")
            .build();
        doc.meta.layout.kind = "conceptMap".to_string();
        // 椭圆那两套。
        for id in ["cm-paper", "cm-mint"] {
            let styles = resolve_all(&doc, &concept_theme(id));
            assert_eq!(styles["a"].shape, Shape::Ellipse, "{id} 该是椭圆");
        }
        // 圆角矩形那两套：改前这里被 `apply_concept_defaults` 硬压成椭圆。
        for id in ["cm-slate", "cm-clay"] {
            let styles = resolve_all(&doc, &concept_theme(id));
            assert_eq!(styles["a"].shape, Shape::Rounded, "{id} 该是圆角矩形");
        }
        // 显式覆盖仍然赢过主题默认。
        let mut doc = Builder::root("根")
            .child("r", "a", "概念甲")
            .styled(
                "r",
                "b",
                "概念乙",
                crate::model::StyleOverride {
                    shape: Some("capsule".to_string()),
                    ..Default::default()
                },
            )
            .build();
        doc.meta.layout.kind = "conceptMap".to_string();
        let styles = resolve_all(&doc, &concept_theme("cm-slate"));
        assert_eq!(styles["b"].shape, Shape::Capsule);
    }

    #[test]
    fn concept_theme_falls_back_to_paper() {
        assert_eq!(concept_theme("没这套").id, "cm-paper");
        assert_eq!(concept_theme("").id, "cm-paper");
        assert!(is_concept_theme(&concept_theme("cm-mint")));
        assert!(!is_concept_theme(&get_theme("snow")));
    }

    /// `docs/33` §4：主题列表按文档类型分流，两套**不混排**。
    #[test]
    fn themes_are_listed_by_document_kind() {
        let mut doc = Builder::root("根").child("r", "a", "概念甲").build();
        doc.meta.layout.kind = "conceptMap".to_string();
        let ids: Vec<&str> = themes_for(&doc).iter().map(|t| t.id).collect();
        assert_eq!(ids, CONCEPT_THEME_IDS.to_vec(), "概念图只列概念图那四套");
        doc.meta.layout.kind = "mindmap".to_string();
        let ids: Vec<&str> = themes_for(&doc).iter().map(|t| t.id).collect();
        assert_eq!(ids, THEME_IDS.to_vec(), "思维导图只列思维导图那四套");
        // `get_theme` 也要认 `cm-*` 的 id，否则选完主题取回来还是 snow。
        assert_eq!(get_theme("cm-slate").id, "cm-slate");
        assert_eq!(concept_themes().len(), 4);
    }
}

#[cfg(test)]
mod canvas_dots_tests {
    use super::*;

    /// AC-1（X21b）：四套内置主题的**点阵令牌**都合法——间距 > 0、直径 < 间距、alpha ∈ 0..=1。
    ///
    /// 这条同时钉住「四套共用同一条几何基准」：格距与半径是画布的唯一一份几何（M1b 起），
    /// 搬进主题只是为了让它**能**换，不是让四套各长一个样。
    #[test]
    fn all_builtin_themes_ship_legal_dot_tokens() {
        for id in THEME_IDS {
            let t = get_theme(id);
            let ui = &t.ui;
            assert!(
                ui.dot_spacing.is_finite() && ui.dot_spacing > 0.0,
                "{id}：格距必须为正"
            );
            assert!(
                ui.dot_radius.is_finite() && ui.dot_radius > 0.0,
                "{id}：半径必须为正"
            );
            assert!(
                ui.dot_radius * 2.0 < ui.dot_spacing,
                "{id}：直径 {} 顶到格距 {}，点会连成一片",
                ui.dot_radius * 2.0,
                ui.dot_spacing
            );
            assert_eq!(ui.dot_spacing, DOT_SPACING, "{id}：格距该用同一条基准");
            assert_eq!(ui.dot_radius, DOT_RADIUS, "{id}：半径该用同一条基准");
            let a = ui.dot.0[3];
            assert!(
                a == 0 || a == 255,
                "{id}：点色只能是「关」（0）或「满不透明」（255），实测 alpha = {a}"
            );
            if let Some(d) = t.canvas_dots() {
                assert!(d.is_on(), "{id}：`canvas_dots()` 出来的必须真的是「开」");
                assert_ne!(
                    d.blended(ui.canvas_bg),
                    ui.canvas_bg,
                    "{id}：点混完还是底色，等于没铺"
                );
            }
        }
    }

    /// AC-2（X21b）：**只有墨夜铺点阵**，其余三套亮色纸底是纯色。
    ///
    /// 依据 `docs/06` §2.1 的实拍结论表：「画布背景 纯色近白，**没有点阵**」；同表把
    /// 「点阵 24px、处处可见」标成被否掉的**旧实现**，并写明「旧文档里『点阵背景 8%』的
    /// 示意已作废」。所以本轮**不加**新点阵、不改底色——`snow` / `mist` / `sand` 保持
    /// `TRANSPARENT`，`ink` 保持现值（暗色画布上那层点从 M1b 起就在，全部暗色基线钉着它）。
    #[test]
    fn only_the_dark_theme_paints_dots_and_it_keeps_its_current_colour() {
        for id in ["snow", "mist", "sand"] {
            let t = get_theme(id);
            assert_eq!(
                t.ui.dot,
                Rgba::TRANSPARENT,
                "{id}：亮色纸底不铺点阵（docs/06 §2.1）"
            );
            assert_eq!(
                t.canvas_dots(),
                None,
                "{id}：`canvas_dots()` 必须说「没有」"
            );
        }

        let ink = get_theme("ink");
        let d = ink.canvas_dots().expect("墨夜有点阵");
        // 现值就是这支色（`theme-ink` 等基线钉着它）；改了它就是无谓的外观漂移。
        assert_eq!(d.color, Rgba::rgb(0x3c, 0x42, 0x4a), "墨夜的点色保持现值");
        assert_eq!((d.spacing, d.radius), (DOT_SPACING, DOT_RADIUS));
        let r = Rgba::contrast_ratio(d.blended(ink.ui.canvas_bg), ink.ui.canvas_bg);
        assert!(
            (1.40..2.10).contains(&r),
            "墨夜的点该「看得见但不抢戏」，实测对比度 {r:.3}"
        );
    }

    /// `canvas_bg` 与点色同源：**默认主题的底色保持暖白 `#FCFBF8`**（WP2「纸感」的刻意选择）。
    ///
    /// `docs/06` §2.2 那张令牌表里的 `#FBFBFC` 是「素白 v2」的历史口径；按 Lead 2026-11-02 的
    /// 裁决，本轮不改底色（零外观改动），这条测试就是防止下次有人照着 `docs/06` 又改一遍。
    #[test]
    fn the_default_canvas_stays_warm_paper_and_paints_no_dots() {
        let snow = get_theme("snow");
        assert_eq!(snow.ui.canvas_bg, Rgba::rgb(0xfc, 0xfb, 0xf8));
        assert_ne!(
            snow.ui.canvas_bg,
            Rgba::rgb(0xff, 0xff, 0xff),
            "画布不是纯白（避免大面积眩光）"
        );
        assert_eq!(snow.canvas_dots(), None);
    }

    /// AC-3（X21b）：关点阵只有两条来源——alpha 写成 0（演示模式就这么关）、概念图主题。
    #[test]
    fn turning_the_dot_colour_off_means_no_dots() {
        let mut t = get_theme("ink");
        assert!(t.canvas_dots().is_some());
        // `crates/mw-shell/src/demo.rs` 关点阵就是这一行；不许因为令牌搬家而失效。
        t.ui.dot = Rgba::TRANSPARENT;
        assert_eq!(t.canvas_dots(), None, "alpha = 0 必须等于不铺点阵");

        for id in CONCEPT_THEME_IDS {
            assert_eq!(
                concept_theme(id).canvas_dots(),
                None,
                "{id}：概念图是纯纸，不铺点阵"
            );
        }
    }

    /// AC-4（X21b）：脏令牌降级成「不画」——不许 panic，也不许在画布上铺 `NaN` 个点。
    #[test]
    fn dirty_dot_tokens_degrade_to_no_dots() {
        let base = get_theme("ink");
        let dirty = [
            (0.0, DOT_RADIUS),
            (-1.0, DOT_RADIUS),
            (f32::NAN, DOT_RADIUS),
            (f32::INFINITY, DOT_RADIUS),
            (DOT_SPACING, 0.0),
            (DOT_SPACING, -2.0),
            (DOT_SPACING, f32::INFINITY),
        ];
        for (spacing, radius) in dirty {
            let mut t = base.clone();
            t.ui.dot_spacing = spacing;
            t.ui.dot_radius = radius;
            assert_eq!(
                t.canvas_dots(),
                None,
                "spacing={spacing} radius={radius} 应该判成「不画」"
            );
        }
        // 半径大到超过半格不是「脏」，只是导出时要钳住（否则点连成一条实线）。
        let mut t = base;
        t.ui.dot_radius = DOT_SPACING;
        let d = t.canvas_dots().expect("令牌本身仍合法");
        assert!(
            d.vector_radius() < d.spacing * 0.5,
            "导出半径必须钳在半格以内：{}",
            d.vector_radius()
        );
    }

    /// 三条派生路径（画布 / SVG / PDF）算的是**同一个**等效色：不透明的点混完等于它自己。
    #[test]
    fn dot_blend_is_the_one_value_the_three_paths_share() {
        let ink = get_theme("ink");
        let d = ink.canvas_dots().unwrap();
        assert_eq!(
            d.blended(ink.ui.canvas_bg),
            d.color,
            "不透明的点，混完必须还是它自己（否则 PDF 与画布会差一个色阶）"
        );
        assert_eq!(d.color.0[3], 255);
        // 低 alpha 的令牌（假设将来有主题用）混完也必须是不透明色。
        let soft = CanvasDots {
            spacing: DOT_SPACING,
            radius: DOT_RADIUS,
            color: Rgba::rgba(0x2a, 0x2e, 0x33, 0x1c),
        };
        let e = soft.blended(Rgba::rgb(0xfc, 0xfb, 0xf8));
        assert_eq!(e.0[3], 255, "混完必须是不透明的");
        assert_ne!(e, Rgba::rgb(0xfc, 0xfb, 0xf8), "11% 的点要分得出来");
    }
}

#[cfg(test)]
mod se13_tests {
    use super::*;
    use crate::tfix::simple;

    /// `bold` 只是 600 的别名，而渲染/导出只认 `weight`。概念图换 `cm-*` 主题时
    /// `levels[0].bold = false`，随后 `apply_concept_defaults` 又把主概念改成粗体——
    /// 这里钉住「改 `bold` 的同时必须把 `weight` 搬过去」，否则中心概念会画成 400
    /// （PDF 伪粗与行内编辑器还按 `bold` 算，两处口径打架）。
    #[test]
    fn concept_themes_move_the_primary_concept_to_the_semibold_weight() {
        let mut d = simple();
        d.meta.layout.kind = "conceptMap".to_string();
        for id in ["cm-paper", "cm-slate", "cm-mint", "cm-clay"] {
            let styles = resolve_all(&d, &concept_theme(id));
            let root = &styles[&d.root_id];
            assert!(root.bold, "{id}：主概念应当是粗体");
            assert_eq!(
                root.weight, WEIGHT_BOLD_ALIAS,
                "{id}：主概念的 weight 必须跟着 bold 一起改成 600"
            );
            assert_eq!(styles["a"].weight, WEIGHT_REGULAR, "{id}：子概念不该被带粗");
        }
    }
}
