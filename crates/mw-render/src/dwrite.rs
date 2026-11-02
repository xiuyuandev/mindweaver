//! DirectWrite 文本度量：真实字形宽度与中文断行（docs/07 §4/§5，痛点 C2 的排版半边）。
//!
//! 单位是 **DIP**（等于旧版的 CSS px）：世界坐标用 DIP，物理像素由 `Camera` 乘
//! `zoom × ppp`。断行、禁则、字体回退交给 DWrite——docs/07 §4 的「实现方」一列原本写的是
//! 「浏览器排版引擎，无需自研」，浏览器退出后接手这三件事的就是 DirectWrite。
//! 项目自己的禁则清单比 DWrite 的 kinsoku 表宽（半角 `%`、全角 `～`），所以断行结果再过一遍
//! [`mw_core::text::enforce_kinsoku`] 兜底，**兜底后按新行重新量宽**（标点挂出行尾是规范
//! 允许的溢出，盒宽必须跟着走）。
//!
//! 线程：DWrite 对象本身线程安全，但这里的格式缓存与度量缓存是`RefCell`，与 winit 事件循环
//! 同线程使用即可。

use std::cell::RefCell;
use std::collections::HashMap;

use mw_core::text::{enforce_kinsoku, line_height, Measured, MonoMeasurer, TextMeasurer, NO_WRAP};
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
    IDWriteTextLayout, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};

/// 中文 UI 字体候选，取系统里第一个真实存在的族（docs/07 §5 的栈）。
///
/// Segoe UI 在前：拉丁/数字拿 Segoe 的真面，中文字形由 DWrite 按脚本回退到雅黑。
/// 若混排标点或基线与雅黑优先相比明显劣化，退回「雅黑优先」——决策样张见
/// `docs/x100/typography-plan-2026-11-01.md` §D1。
pub const FONT_CANDIDATES: &[&str] = &[
    "Segoe UI",
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Noto Sans SC",
];

/// 候选全都不在时用的西文族：DWrite 会替缺字形做自动回退，中文照样量得准。
pub const FALLBACK_FAMILY: &str = "Segoe UI";

/// 断行与禁则按简体中文语境判断，DWrite 的 kinsoku 表随 locale 变。
pub const LOCALE: &str = "zh-Hans";

/// 粗体 = 600：旧版 CSS 写的是 `600`，沿用才能让盒宽与退休前的 SVG 可比。
/// 与 `mw_core::theme::WEIGHT_BOLD_ALIAS` 同值——`bold: true` 的旧文件读出来就是它。
pub const WEIGHT_NORMAL: u16 = mw_core::theme::WEIGHT_REGULAR;
pub const WEIGHT_BOLD: u16 = mw_core::theme::WEIGHT_BOLD_ALIAS;

/// 高度不设限：D2D/DWrite 的惯例是传 FLT_MAX，我们只关心宽度。
const UNBOUNDED_HEIGHT: f32 = f32::MAX;

/// 度量缓存上限，超了就整张丢（与旧版 `measure.ts` 同策略：文本短、命中率高、不值得做 LRU）。
const CACHE_LIMIT: usize = 20_000;

/// 禁则兜底后的重折上限：一次悬挂标点最多牵动一段，留出 3 趟纯属保险，正常第 2 趟就收敛。
const REFINE_PASSES: usize = 3;

/// NUL 结尾的 UTF-16 缓冲。`PCWSTR` 只是指针，得有人替它保管内存活过整个调用。
pub struct Wide(Vec<u16>);

impl Wide {
    pub fn new(text: &str) -> Self {
        let mut buf: Vec<u16> = text.encode_utf16().collect();
        buf.push(0);
        Self(buf)
    }

    pub fn pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

/// DirectWrite 门面：工厂 + 实际命中的字族 + 字体格式缓存。
pub struct DWrite {
    factory: IDWriteFactory,
    family: String,
    formats: RefCell<HashMap<(u32, u16), IDWriteTextFormat>>,
}

impl DWrite {
    /// 建工厂。失败（非 Windows、`dwrite.dll` 不可用）返回 `None`，上层回落到 `MonoMeasurer`。
    pub fn new() -> Option<Self> {
        let factory =
            unsafe { DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED) }.ok()?;
        let family = pick_family(&factory);
        Some(Self {
            factory,
            family,
            formats: RefCell::new(HashMap::new()),
        })
    }

    /// 实际用于度量的字体族名（诊断与文档取证用）。
    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn factory(&self) -> &IDWriteFactory {
        &self.factory
    }

    /// 同一 `(字号, 字重)` 只建一次：`IDWriteTextFormat` 建好即不可变，可以放心共享。
    ///
    /// `weight` 是真实字重（400 / 500 / 600 / 700）：SE-13 起检查器能选三档，
    /// 绘制与度量都从这里取格式，所以**同一个 weight 在两边落到同一张字面**。
    pub fn format(&self, font_size: f32, weight: u16) -> windows::core::Result<IDWriteTextFormat> {
        let weight = weight.clamp(1, 999);
        let key = (font_size.to_bits(), weight);
        if let Some(hit) = self.formats.borrow().get(&key).cloned() {
            return Ok(hit);
        }
        let family = Wide::new(&self.family);
        let locale = Wide::new(LOCALE);
        let format = unsafe {
            self.factory.CreateTextFormat(
                family.pcwstr(),
                None,
                DWRITE_FONT_WEIGHT(weight as i32),
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                locale.pcwstr(),
            )
        }?;
        self.formats.borrow_mut().insert(key, format.clone());
        Ok(format)
    }
}

/// 按 docs/07 §5 的栈挑第一个系统里真有的族。
fn pick_family(factory: &IDWriteFactory) -> String {
    let mut collection: Option<IDWriteFontCollection> = None;
    if unsafe { factory.GetSystemFontCollection(&mut collection, true) }.is_err() {
        return FALLBACK_FAMILY.to_string();
    }
    let Some(collection) = collection else {
        return FALLBACK_FAMILY.to_string();
    };
    for candidate in FONT_CANDIDATES {
        let name = Wide::new(candidate);
        let mut index = 0u32;
        let mut exists = BOOL(0);
        if unsafe { collection.FindFamilyName(name.pcwstr(), &mut index, &mut exists) }.is_err() {
            continue;
        }
        if exists.0 != 0 {
            return (*candidate).to_string();
        }
    }
    FALLBACK_FAMILY.to_string()
}

/// [`TextMeasurer`] 的 Windows 实现：真实字体宽度 + DWrite 断行 + 本项目禁则兜底。
pub struct DWriteMeasurer {
    dwrite: DWrite,
    cache: RefCell<HashMap<Key, Measured>>,
}

#[derive(PartialEq, Eq, Hash)]
struct Key {
    text: String,
    font_size: u32,
    weight: u16,
    max_width: u32,
}

impl DWriteMeasurer {
    pub fn new(dwrite: DWrite) -> Self {
        Self {
            dwrite,
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn dwrite(&self) -> &DWrite {
        &self.dwrite
    }

    /// 度量，但把 DWrite 的失败原样交出去。测试与「必须用真实字体」的调用点走这里，
    /// 免得错误被 [`TextMeasurer`] 的近似回落吞掉、看起来像通过。
    pub fn measure_result(
        &self,
        text: &str,
        font_size: f32,
        weight: u16,
        max_width: f32,
    ) -> windows::core::Result<Measured> {
        let key = Key {
            text: text.to_string(),
            font_size: font_size.to_bits(),
            weight,
            max_width: max_width.to_bits(),
        };
        if let Some(hit) = self.cache.borrow().get(&key) {
            return Ok(hit.clone());
        }
        let measured = self.measure_uncached(text, font_size, weight, max_width)?;
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, measured.clone());
        Ok(measured)
    }

    fn measure_uncached(
        &self,
        text: &str,
        font_size: f32,
        weight: u16,
        max_width: f32,
    ) -> windows::core::Result<Measured> {
        let line_height = line_height(font_size);
        if text.is_empty() {
            return Ok(Measured {
                lines: vec![String::new()],
                width: 0.0,
                line_height,
            });
        }
        let format = self.dwrite.format(font_size, weight)?;
        let unbounded = max_width >= NO_WRAP;
        let limit = if unbounded {
            NO_WRAP
        } else {
            max_width.max(1.0)
        };
        // 硬换行自己切（与旧版 measure.ts 同构）：DWrite 只见无换行的自然段，
        // 尾部空行它会如实报出来，所以这里不再手工补行。
        let mut lines = Vec::new();
        for para in text.split('\n') {
            if para.is_empty() {
                lines.push(String::new());
                continue;
            }
            lines.extend(self.wrap_paragraph(para, &format, limit, unbounded)?);
        }
        let width = self.width_of(&lines, &format)?;
        Ok(Measured {
            lines,
            width,
            line_height,
        })
    }

    /// 折一个自然段：DWrite 断行 → 本项目禁则兜底 → 被搬空的短行连同以下文本重折。
    ///
    /// 兜底把行首标点挂回上一行是**允许溢出**的，但一次挂两个字符（「……」「——」）就会
    /// 把本行掏成一个字，读起来像坏了。把这一字起的后半段重新交给 DWrite 折一次，得到的
    /// 就是浏览器的结果：悬挂标点 + 剩余内容重新填满行。只在真的动了字符时才多花一次布局。
    fn wrap_paragraph(
        &self,
        para: &str,
        format: &IDWriteTextFormat,
        limit: f32,
        unbounded: bool,
    ) -> windows::core::Result<Vec<String>> {
        let mut lines = self.split_once(para, format, limit, unbounded)?;
        for _ in 0..REFINE_PASSES {
            let before = lines.clone();
            enforce_kinsoku(&mut lines);
            // 禁则搬走标点后，从**跨行边界第一次变化**的位置起重新填满。不能看「哪一行
            // 文本变了」——标点搬进上一行时，上一行也变，但该重折的是被搬空的那一侧。
            let moved = first_reflow(&lines, &before);
            if let Some(i) = orphan_index(&lines).or(moved) {
                let tail = lines[i..].concat();
                let rest = self.split_once(&tail, format, limit, unbounded)?;
                // DWrite 自己不认识禁则；重折结果如果又把标点顶到行首/行尾，就不能采纳，
                // 否则会和下一轮 enforce 来回拉锯，最后停在违规状态。
                if !rest.is_empty() && rest.as_slice() != &lines[i..] && kinsoku_legal(&rest) {
                    lines.truncate(i);
                    lines.extend(rest);
                }
            }
            if lines == before {
                break;
            }
        }
        Ok(lines)
    }

    fn split_once(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        limit: f32,
        unbounded: bool,
    ) -> windows::core::Result<Vec<String>> {
        let layout = unsafe {
            let layout = self.dwrite.factory().CreateTextLayout(
                &utf16_units(text),
                format,
                limit,
                UNBOUNDED_HEIGHT,
            )?;
            if unbounded {
                layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            }
            layout
        };
        split_lines(text, &layout)
    }

    /// 兜底之后的每行都当成不折行的单行重新量：行数很少，结果整条缓存，成本摊得掉。
    fn width_of(&self, lines: &[String], format: &IDWriteTextFormat) -> windows::core::Result<f32> {
        let mut width = 0.0f32;
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let units = utf16_units(line);
            let one = unsafe {
                let one = self.dwrite.factory().CreateTextLayout(
                    &units,
                    format,
                    NO_WRAP,
                    UNBOUNDED_HEIGHT,
                )?;
                one.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                one
            };
            let mut metrics = DWRITE_TEXT_METRICS::default();
            unsafe { one.GetMetrics(&mut metrics)? };
            width = width.max(metrics.width.max(0.0));
        }
        Ok(width)
    }
}

impl TextMeasurer for DWriteMeasurer {
    fn measure(&self, text: &str, font_size: f32, weight: u16, max_width: f32) -> Measured {
        self.measure_result(text, font_size, weight, max_width)
            .unwrap_or_else(|_| MonoMeasurer.measure(text, font_size, weight, max_width))
    }
}

pub fn utf16_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// 取 DWrite 的断行结果，逐行还原本行**去掉行尾空白与换行符**的内容。
///
/// `DWRITE_LINE_METRICS.length` 是 UTF-16 计数且包含行尾空白，所以要带着一张字节偏移表；
/// 代理对（emoji、生僻字）正是靠这张表才不会被切在半个字上。
fn split_lines(text: &str, layout: &IDWriteTextLayout) -> windows::core::Result<Vec<String>> {
    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe { layout.GetMetrics(&mut metrics)? };
    let count = (metrics.lineCount as usize).max(1);

    let mut rows = vec![DWRITE_LINE_METRICS::default(); count];
    let mut actual = 0u32;
    unsafe { layout.GetLineMetrics(Some(&mut rows), &mut actual)? };
    let map = byte_map(text);

    let mut lines = Vec::with_capacity(count);
    let mut pos = 0usize;
    for row in rows.iter().take(actual as usize) {
        // 实测（Win11 / YaHei UI）：`trailingWhitespaceLength` **已经包含** `newlineLength`，
        // 按文档把两者相加会把硬换行前那一个字吃掉。
        let content = row.length.saturating_sub(row.trailingWhitespaceLength) as usize;
        let from = byte_at(&map, pos, text);
        let to = byte_at(&map, pos + content, text);
        lines.push(text.get(from..to).unwrap_or_default().to_string());
        pos += row.length as usize;
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    Ok(lines)
}

/// 兜底搬走行首标点后，本行只剩一个字、下一行还很满——说明这一行该重新填过。
/// 第 0 行不参与：它上面没有可挂的行。
fn orphan_index(lines: &[String]) -> Option<usize> {
    if lines.len() < 3 {
        return None;
    }
    (1..lines.len() - 1)
        .find(|i| lines[*i].chars().count() == 1 && lines[i + 1].chars().count() >= 2)
}

/// 第一处「后半段拼起来不一样」的下标。标点从行首搬到上一行时，上一行也变，
/// 但真正被掏空的是下一行；用尾巴比较才能落到该重折的边界上。
fn first_reflow(lines: &[String], before: &[String]) -> Option<usize> {
    let n = lines.len().max(before.len());
    (0..n).find(|&i| {
        let now: String = lines.iter().skip(i).map(String::as_str).collect();
        let old: String = before.iter().skip(i).map(String::as_str).collect();
        now != old
    })
}

/// 重折结果是否已经满足行首/行尾禁则；最后一行不查行尾（段落可以以标点收尾）。
fn kinsoku_legal(lines: &[String]) -> bool {
    for (i, line) in lines.iter().enumerate() {
        if i > 0
            && line
                .chars()
                .next()
                .is_some_and(mw_core::text::forbidden_line_start)
        {
            return false;
        }
        if i + 1 < lines.len()
            && line
                .chars()
                .next_back()
                .is_some_and(mw_core::text::forbidden_line_end)
        {
            return false;
        }
    }
    true
}

/// UTF-16 下标 → 字节下标。表长 = UTF-16 单元数 + 1，末项是文本长度。
fn byte_map(text: &str) -> Vec<u32> {
    let mut map = Vec::with_capacity(text.encode_utf16().count() + 1);
    for (byte, ch) in text.char_indices() {
        for _ in 0..ch.len_utf16() {
            map.push(byte as u32);
        }
    }
    map.push(text.len() as u32);
    map
}

/// 越界一律按「文本末尾」处理：驱动的返回值不可信，但绝不能把主线程炸掉。
fn byte_at(map: &[u32], unit: usize, text: &str) -> usize {
    map.get(unit)
        .map(|v| *v as usize)
        .unwrap_or(text.len())
        .min(text.len())
}
