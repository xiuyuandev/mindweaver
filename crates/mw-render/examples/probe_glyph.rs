//! 探针：`IDWriteTextLayout::Draw` 的字形回调 + 两种 `CreateGlyphRunAnalysis` 光栅化路径。
//! 运行：`cargo run -p mw-render --example probe_glyph`
//!
//! 要回答四个问题：
//! 1. `#[implement(IDWriteTextRenderer)]` 在 windows-rs 0.62.2 能不能编译、`Draw` 能不能回调；
//! 2. 回调里的 `fontFace` 是不是已经做过字体回退（中英混排会不会出现两个 face）；
//! 3. GDI 互操作的 ClearType 3×1 纹理能否拿到（含亚像素缓冲区大小）；
//! 4. `IDWriteFactory2::CreateGlyphRunAnalysis(GRAYSCALE)` 是否支持灰度 alpha 纹理。

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::rc::Rc;

use mw_render::dwrite::{utf16_units, DWrite};
use windows::core::{implement, IUnknown, Interface, Ref, Result, BOOL};
use windows::Win32::Graphics::DirectWrite::*;

const SIZE: f32 = 14.0;
const PPD: f32 = 1.0;

#[derive(Clone)]
struct Item {
    face: IDWriteFontFace,
    glyph: u16,
    advance: f32,
    em: f32,
    x: f32,
    y: f32,
}

#[implement(IDWriteTextRenderer, Agile = false)]
struct Collector {
    items: Rc<RefCell<Vec<Item>>>,
}

impl IDWritePixelSnapping_Impl for Collector_Impl {
    fn IsPixelSnappingDisabled(&self, _: *const c_void) -> Result<BOOL> {
        Ok(BOOL(1))
    }
    fn GetCurrentTransform(&self, _: *const c_void, transform: *mut DWRITE_MATRIX) -> Result<()> {
        unsafe {
            *transform = DWRITE_MATRIX {
                m11: 1.0,
                m12: 0.0,
                m21: 0.0,
                m22: 1.0,
                dx: 0.0,
                dy: 0.0,
            };
        }
        Ok(())
    }
    fn GetPixelsPerDip(&self, _: *const c_void) -> Result<f32> {
        Ok(PPD)
    }
}

impl IDWriteTextRenderer_Impl for Collector_Impl {
    fn DrawGlyphRun(
        &self,
        _: *const c_void,
        baseline_x: f32,
        baseline_y: f32,
        _: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        _: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        let run = unsafe { &*run };
        let face =
            <Option<IDWriteFontFace> as Clone>::clone(&run.fontFace).expect("字形运行必须带字体面");
        let mut x = baseline_x;
        let mut items = self.items.borrow_mut();
        for i in 0..run.glyphCount as usize {
            let glyph = unsafe { *run.glyphIndices.add(i) };
            let advance = unsafe { *run.glyphAdvances.add(i) };
            let (dx, dy) = if run.glyphOffsets.is_null() {
                (0.0, 0.0)
            } else {
                let off = unsafe { *run.glyphOffsets.add(i) };
                (off.advanceOffset, -off.ascenderOffset)
            };
            items.push(Item {
                face: face.clone(),
                glyph,
                advance,
                em: run.fontEmSize,
                x: x + dx,
                y: baseline_y + dy,
            });
            x += advance;
        }
        Ok(())
    }
    fn DrawUnderline(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_UNDERLINE,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
    fn DrawStrikethrough(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_STRIKETHROUGH,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
    fn DrawInlineObject(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: Ref<IDWriteInlineObject>,
        _: BOOL,
        _: BOOL,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
}

fn main() {
    let Some(dw) = DWrite::new() else {
        println!("本机没有 DirectWrite");
        return;
    };
    let factory = dw.factory();
    let format = dw
        .format(SIZE, mw_core::theme::WEIGHT_REGULAR)
        .expect("创建文本格式");
    let mut seen = 0usize;

    for text in ["中文", "Rust 中文", "①★", "——", "Hello, world!"] {
        let units = utf16_units(text);
        let layout = unsafe {
            factory
                .CreateTextLayout(&units, &format, f32::MAX, f32::MAX)
                .expect("创建布局")
        };
        unsafe { layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP) }.expect("不折行");
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { layout.GetMetrics(&mut metrics) }.expect("布局度量");
        let mut line = [DWRITE_LINE_METRICS::default(); 1];
        let mut actual = 0u32;
        unsafe { layout.GetLineMetrics(Some(&mut line), &mut actual) }.expect("行度量");
        let shared = Rc::new(RefCell::new(Vec::new()));
        let renderer: IDWriteTextRenderer = Collector {
            items: shared.clone(),
        }
        .into();
        unsafe { layout.Draw(None, &renderer, 0.0, 0.0) }.expect("Draw 回调");

        let items = shared.borrow().clone();
        seen += items.len();
        println!(
            "\n## {text:?}  布局 {:.2}×{:.2}，首行基线 {:.2}，字形 {}",
            metrics.width,
            metrics.height,
            line[0].baseline,
            items.len()
        );
        for it in items.iter() {
            let family = it
                .face
                .cast::<IDWriteFontFace3>()
                .ok()
                .and_then(|f| unsafe { f.GetFamilyNames() }.ok())
                .map(|names| {
                    let len = unsafe { names.GetStringLength(0) }.unwrap_or(0);
                    let mut buf = vec![0u16; len as usize + 1];
                    unsafe { names.GetString(0, &mut buf) }.ok();
                    String::from_utf16_lossy(&buf[..len as usize])
                })
                .unwrap_or_else(|| "?".into());
            println!(
                "  glyph {:#06x}  face={family:<20} adv={:.3} em={:.2} @({:.2},{:.2})",
                it.glyph, it.advance, it.em, it.x, it.y
            );
        }

        if let Some(it) = items.first() {
            clear_type(factory, it);
            grayscale(factory, it);
        }
    }
    println!("\n共回调 {} 个字形", seen);
    direct_check(factory, dw.family());
}

/// 绕开 Draw 回调：直接向 font face 要 “R” 的字形 ID，再光栅化。
fn direct_check(factory: &IDWriteFactory, family: &str) {
    let mut collection = None;
    if unsafe { factory.GetSystemFontCollection(&mut collection, false) }.is_err() {
        return;
    }
    let Some(collection) = collection else {
        return;
    };
    let mut index = 0u32;
    let mut exists = BOOL(0);
    let name: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
    if unsafe {
        collection.FindFamilyName(
            windows::core::PCWSTR(name.as_ptr()),
            &mut index,
            &mut exists,
        )
    }
    .is_err()
        || exists.0 == 0
    {
        println!("直接校验：找不到字体族");
        return;
    }
    let Ok(fam) = (unsafe { collection.GetFontFamily(index) }) else {
        return;
    };
    let Ok(font) = (unsafe {
        fam.GetFirstMatchingFont(
            DWRITE_FONT_WEIGHT(400),
            DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
        )
    }) else {
        return;
    };
    let Ok(face) = (unsafe { font.CreateFontFace() }) else {
        return;
    };
    let cps: Vec<u32> = "R".chars().map(|c| c as u32).collect();
    let mut glyphs = vec![0u16; cps.len()];
    if let Err(e) =
        unsafe { face.GetGlyphIndices(cps.as_ptr(), cps.len() as u32, glyphs.as_mut_ptr()) }
    {
        println!("直接校验：GetGlyphIndices 失败 {e:?}");
        return;
    }
    let mut design = [DWRITE_GLYPH_METRICS::default(); 1];
    unsafe { face.GetDesignGlyphMetrics(glyphs.as_ptr(), 1, design.as_mut_ptr(), false) }.ok();
    let count = unsafe { face.GetGlyphCount() };
    println!(
        "\n直接校验：glyph('R') = {:#06x}，face 字形总数 {count}，设计 advance {} / em {}",
        glyphs[0], design[0].advanceWidth, design[0].advanceHeight
    );
    let it = Item {
        face,
        glyph: glyphs[0],
        advance: 9.14,
        em: SIZE,
        x: 0.0,
        y: 0.0,
    };
    clear_type(factory, &it);
    grayscale(factory, &it);
}

/// `DWRITE_GLYPH_RUN` 里的指针指向调用方栈上的 `glyph` / `advance`，
/// 所以这两个局部变量必须和分析调用活在同一个栈帧里 —— 不能把 run 从函数里返回。
fn run_for<'a>(it: &Item, glyph: &'a u16, advance: &'a f32) -> DWRITE_GLYPH_RUN {
    DWRITE_GLYPH_RUN {
        fontFace: ManuallyDrop::new(Some(it.face.clone())),
        fontEmSize: it.em,
        glyphCount: 1,
        glyphIndices: glyph,
        glyphAdvances: advance,
        glyphOffsets: std::ptr::null(),
        isSideways: BOOL(0),
        bidiLevel: 0,
    }
}

fn clear_type(factory: &IDWriteFactory, it: &Item) {
    let glyph = it.glyph;
    let advance = it.advance;
    let run = run_for(it, &glyph, &advance);
    let an = match unsafe {
        factory.CreateGlyphRunAnalysis(
            &run,
            PPD,
            None,
            DWRITE_RENDERING_MODE_GDI_NATURAL,
            DWRITE_MEASURING_MODE_NATURAL,
            0.0,
            0.0,
        )
    } {
        Ok(a) => a,
        Err(e) => {
            println!("  GDI 分析失败：{e:?}");
            return;
        }
    };
    let bounds = match unsafe { an.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1) } {
        Ok(b) => b,
        Err(e) => {
            println!("  GDI 边界失败：{e:?}");
            return;
        }
    };
    let w = (bounds.right - bounds.left).max(0) as usize;
    let h = (bounds.bottom - bounds.top).max(0) as usize;
    let mut buf = vec![0u8; w * h * 3];
    if let Err(e) =
        unsafe { an.CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bounds, &mut buf) }
    {
        println!("  GDI 纹理失败：{e:?}");
        return;
    }
    let max = buf.iter().copied().max().unwrap_or(0);
    let ink = buf.chunks(3).filter(|p| p.iter().any(|&v| v > 8)).count();
    println!(
        "  GDI_NATURAL：bounds {:?}，{}×{}，三通道 {} B，max {max}，有墨像素 {ink}",
        (bounds.left, bounds.top, bounds.right, bounds.bottom),
        w,
        h,
        buf.len()
    );
}

fn grayscale(factory: &IDWriteFactory, it: &Item) {
    let Ok(f2) = factory.cast::<IDWriteFactory2>() else {
        println!("  工厂不支持 IDWriteFactory2");
        return;
    };
    let glyph = it.glyph;
    let advance = it.advance;
    let run = run_for(it, &glyph, &advance);
    let an = unsafe {
        f2.CreateGlyphRunAnalysis(
            &run,
            None,
            DWRITE_RENDERING_MODE_NATURAL,
            DWRITE_MEASURING_MODE_NATURAL,
            DWRITE_GRID_FIT_MODE_DEFAULT,
            DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
            0.0,
            0.0,
        )
    };
    let an = match an {
        Ok(a) => a,
        Err(e) => {
            println!("  灰度分析失败：{e:?}");
            return;
        }
    };
    for (name, ty) in [
        ("ALIASED_1x1", DWRITE_TEXTURE_ALIASED_1x1),
        ("CLEARTYPE_3x1", DWRITE_TEXTURE_CLEARTYPE_3x1),
    ] {
        match unsafe { an.GetAlphaTextureBounds(ty) } {
            Ok(b) => println!(
                "  灰度 {name}：bounds {:?}",
                (b.left, b.top, b.right, b.bottom)
            ),
            Err(e) => println!("  灰度 {name}：{e:?}"),
        }
    }
    let bounds = match unsafe { an.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1) } {
        Ok(b) => b,
        Err(_) => return,
    };
    let w = (bounds.right - bounds.left).max(0) as usize;
    let h = (bounds.bottom - bounds.top).max(0) as usize;
    if w == 0 || h == 0 {
        return;
    }
    let mut buf = vec![0u8; w * h];
    if let Err(e) = unsafe { an.CreateAlphaTexture(DWRITE_TEXTURE_ALIASED_1x1, &bounds, &mut buf) }
    {
        println!("  灰度纹理失败：{e:?}");
        return;
    }
    let max = buf.iter().copied().max().unwrap_or(0);
    let ink = buf.iter().filter(|&&v| v > 8).count();
    let antialiased = buf.iter().filter(|&&v| v > 0 && v < 255).count();
    println!(
        "  灰度纹理：{}×{}，max {max}，有墨 {ink}，抗锯齿灰阶 {antialiased} 个（0<v<255）",
        w, h
    );
    for row in 0..h {
        let line: String = buf[row * w..(row + 1) * w]
            .iter()
            .map(|&v| match v {
                0..=8 => ' ',
                9..=64 => '░',
                65..=128 => '▒',
                129..=200 => '▓',
                _ => '█',
            })
            .collect();
        println!("   |{line}|");
    }
}
