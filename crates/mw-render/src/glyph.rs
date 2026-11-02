//! 字形光栅化与 R8 图集（docs/13 §4.9、痛点 C3）。
//!
//! 只做灰度 1×1：画布会缩放，次像素 ClearType 一旦被缩放采样就变成彩边，
//! 灰度覆盖率才是「任何缩放都不发虚也不发彩」的那条路。两条路径：
//! 1. `IDWriteFactory2::CreateGlyphRunAnalysis` + `GRAYSCALE` + `ALIASED_1x1`（主路径，
//!    Win8.1+，本机实测 36 级覆盖率）；
//! 2. 老工厂 `GDI_NATURAL` + `CLEARTYPE_3x1`，把三通道按亮度合成灰度（兜底）。
//!
//! 图集键里的 ppd 是**量化后的像素/字距**：光栅化按该倍率把 `fontEmSize` 放大，
//! 于是 100% 时一个 texel 正好落在一个物理像素上；非整档缩放由采样器做轻微缩放。
//! 键里还必须带字号——同一个字形 14px 与 22px 的位图不同。

use std::collections::HashMap;
use std::mem::ManuallyDrop;

use windows::core::{Interface, BOOL};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_TEXTURE_ALIASED_1x1, DWRITE_TEXTURE_CLEARTYPE_3x1, IDWriteFactory, IDWriteFactory2,
    IDWriteFontFace, IDWriteGlyphRunAnalysis, DWRITE_GLYPH_RUN, DWRITE_GRID_FIT_MODE_DEFAULT,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_RENDERING_MODE_GDI_NATURAL,
    DWRITE_RENDERING_MODE_NATURAL, DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
};

/// 图集边长。3072² 单通道 = 9 MB，按 14px 字形（约 15×15 含间隙）约可放 4 万槽位：
/// 真实 10k 节点中文文档按三档字号、单 ppd 估约 1.2–1.5 万，留出两倍余量。
/// P13 内存预算下取 3072 而非 4096（省 7 MB）；行跨度 3072 仍是 256 的倍数，
/// wgpu 的分行上传对齐不受影响。
pub const ATLAS_SIZE: u32 = 3072;
/// 字形之间留 1 texel，避免双线性采样把邻居的边扫进来。
const GAP: u32 = 1;

/// 光栅化倍率阶梯。取「向上取整到最近一档」：宁可比屏幕多采样一点，
/// 也不要把字缩小后再放大（那才是真的糊）。
const PPD_BUCKETS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0];

pub fn quantize_ppd(ppd: f32) -> f32 {
    let p = ppd.max(0.01);
    PPD_BUCKETS
        .iter()
        .copied()
        .find(|b| *b >= p - 1e-4)
        .unwrap_or(*PPD_BUCKETS.last().unwrap())
}

/// 图集键。`font_size` 存 bits，避免 f32 不能 Hash 的问题。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub face: u32,
    pub glyph: u16,
    pub font_size: u32,
    /// 量化 ppd × 100（1.25 → 125）
    pub ppd: u16,
}

impl GlyphKey {
    pub fn new(face: u32, glyph: u16, font_size: f32, ppd: f32) -> Self {
        Self {
            face,
            glyph,
            font_size: font_size.to_bits(),
            ppd: (ppd * 100.0).round() as u16,
        }
    }

    pub fn ppd_f32(self) -> f32 {
        self.ppd as f32 / 100.0
    }
}

/// 一个字形在图集里的位置与基线偏移（像素，相对字形原点；top 向上为负）。
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphSlot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// 位图左边界相对笔位（像素）
    pub left: i32,
    /// 位图顶边相对基线（像素，负=基线之上）
    pub top: i32,
    /// 实际光栅化倍率
    pub ppd: f32,
}

impl GlyphSlot {
    pub fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// 世界单位（DIP）下的四边形左上角与尺寸。
    pub fn quad(self, pen_x: f32, baseline_y: f32) -> ([f32; 2], [f32; 2]) {
        let k = self.ppd.max(0.01);
        (
            [
                pen_x + self.left as f32 / k,
                baseline_y + self.top as f32 / k,
            ],
            [self.w as f32 / k, self.h as f32 / k],
        )
    }

    pub fn uv(self) -> [f32; 4] {
        let s = ATLAS_SIZE as f32;
        [
            self.x as f32 / s,
            self.y as f32 / s,
            (self.x + self.w) as f32 / s,
            (self.y + self.h) as f32 / s,
        ]
    }
}

/// 一次图集上传：纹素坐标 + **紧排**像素（行跨度 = `w`）。
///
/// CPU 侧不再保留整张 3072² 镜像（P13：那是 9MB 常驻私有内存），
/// 光栅化出的位图直接随脏矩形交给 GPU。
#[derive(Clone, Debug)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub pixels: Vec<u8>,
}

/// 一次绘制后需要上传给 GPU 的图集增量。
///
/// `reset` 表示图集已整体重排、旧 UV 全部失效；新字形仍以 `rects` 逐块上传。
/// GPU 纹理不需要清空：旧像素不再被任何 UV 引用，留着也采不到。
/// `generation` 不连续说明有更新被跳过（调用方只在失去同步时才会发生），
/// 合成器会记录一次「需要重置图集」，由调用方在下一帧重排。
#[derive(Clone, Debug, Default)]
pub struct AtlasUpdate {
    pub generation: u64,
    pub reset: bool,
    pub rects: Vec<AtlasRect>,
}

/// [`GlyphAtlas::insert`] 的结果。`Full` 表示图集需要重置重排。
pub enum Insert {
    Ready(GlyphSlot),
    Full,
    Failed,
}

pub struct GlyphAtlas {
    factory: IDWriteFactory,
    factory2: Option<IDWriteFactory2>,
    map: HashMap<GlyphKey, GlyphSlot>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    dirty: Vec<AtlasRect>,
    reset: bool,
    generation: u64,
}

impl GlyphAtlas {
    pub fn new(factory: &IDWriteFactory) -> Self {
        Self {
            factory: factory.clone(),
            factory2: factory.cast::<IDWriteFactory2>().ok(),
            map: HashMap::new(),
            shelf_x: 0,
            shelf_y: 0,
            shelf_h: 0,
            dirty: Vec::new(),
            reset: false,
            generation: 0,
        }
    }

    pub fn get(&self, key: &GlyphKey) -> Option<GlyphSlot> {
        self.map.get(key).copied()
    }

    pub fn contains(&self, key: &GlyphKey) -> bool {
        self.map.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// 取或光栅化一个字形。失败（字体拒绝该字形）返回零槽位并缓存，
    /// 这样每帧不会重复撞同一堵墙；`Full` 由调用方触发重置后重来。
    pub fn insert(&mut self, key: GlyphKey, face: &IDWriteFontFace) -> Insert {
        if let Some(hit) = self.map.get(&key) {
            return Insert::Ready(*hit);
        }
        let Some(bitmap) = self.rasterize(
            face,
            key.glyph,
            f32::from_bits(key.font_size),
            key.ppd_f32(),
        ) else {
            let empty = GlyphSlot {
                ppd: key.ppd_f32(),
                ..Default::default()
            };
            self.map.insert(key, empty);
            return Insert::Ready(empty);
        };
        if bitmap.w == 0 || bitmap.h == 0 {
            let empty = GlyphSlot {
                left: bitmap.left,
                top: bitmap.top,
                ppd: key.ppd_f32(),
                ..Default::default()
            };
            self.map.insert(key, empty);
            return Insert::Ready(empty);
        }
        if bitmap.w + GAP > ATLAS_SIZE || bitmap.h + GAP > ATLAS_SIZE {
            self.map.insert(key, GlyphSlot::default());
            return Insert::Failed;
        }
        let Some((x, y)) = self.alloc(bitmap.w, bitmap.h) else {
            return Insert::Full;
        };
        self.dirty.push(AtlasRect {
            x,
            y,
            w: bitmap.w,
            h: bitmap.h,
            pixels: bitmap.pixels,
        });
        let slot = GlyphSlot {
            x,
            y,
            w: bitmap.w,
            h: bitmap.h,
            left: bitmap.left,
            top: bitmap.top,
            ppd: key.ppd_f32(),
        };
        self.map.insert(key, slot);
        Insert::Ready(slot)
    }

    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.shelf_x + w + GAP > ATLAS_SIZE {
            self.shelf_y += self.shelf_h + GAP;
            self.shelf_x = 0;
            self.shelf_h = 0;
        }
        if self.shelf_y + h + GAP > ATLAS_SIZE {
            return None;
        }
        let at = (self.shelf_x, self.shelf_y);
        self.shelf_x += w + GAP;
        self.shelf_h = self.shelf_h.max(h);
        Some(at)
    }

    /// 清空重排。已发出的 UV 会失效，所以调用方必须在「收集完本帧全部字形」之前用它，
    /// 见 `TextEngine::paint` 的两趟结构。
    pub fn reset(&mut self) {
        self.map.clear();
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_h = 0;
        self.dirty.clear();
        self.reset = true;
    }

    pub fn take_update(&mut self) -> AtlasUpdate {
        self.generation += 1;
        let update = AtlasUpdate {
            generation: self.generation,
            reset: self.reset,
            rects: std::mem::take(&mut self.dirty),
        };
        self.reset = false;
        update
    }

    fn rasterize(
        &self,
        face: &IDWriteFontFace,
        glyph: u16,
        font_size: f32,
        ppd: f32,
    ) -> Option<Bitmap> {
        // 单字形分析：位图只由「face + 字形 + 放大的 em」决定，advance 传 em 即可。
        // 指针指向本栈帧里的 glyph/advance，绝不能把 run 返回出去。
        let glyph = glyph;
        let em = font_size * ppd;
        let advance = em;
        let run = DWRITE_GLYPH_RUN {
            fontFace: ManuallyDrop::new(Some(face.clone())),
            fontEmSize: em,
            glyphCount: 1,
            glyphIndices: &glyph,
            glyphAdvances: &advance,
            glyphOffsets: std::ptr::null(),
            isSideways: BOOL(0),
            bidiLevel: 0,
        };

        if let Some(f2) = &self.factory2 {
            if let Ok(analysis) = unsafe {
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
            } {
                if let Some(b) = grayscale_bitmap(&analysis) {
                    return Some(b);
                }
            }
        }
        let analysis = unsafe {
            self.factory.CreateGlyphRunAnalysis(
                &run,
                ppd,
                None,
                DWRITE_RENDERING_MODE_GDI_NATURAL,
                DWRITE_MEASURING_MODE_NATURAL,
                0.0,
                0.0,
            )
        }
        .ok()?;
        cleartype_bitmap(&analysis)
    }
}

struct Bitmap {
    w: u32,
    h: u32,
    left: i32,
    top: i32,
    pixels: Vec<u8>,
}

fn grayscale_bitmap(analysis: &IDWriteGlyphRunAnalysis) -> Option<Bitmap> {
    let bounds = unsafe { analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1) }.ok()?;
    let w = (bounds.right - bounds.left).max(0) as u32;
    let h = (bounds.bottom - bounds.top).max(0) as u32;
    if w == 0 || h == 0 {
        return Some(Bitmap {
            w: 0,
            h: 0,
            left: bounds.left,
            top: bounds.top,
            pixels: Vec::new(),
        });
    }
    let mut pixels = vec![0u8; (w * h) as usize];
    unsafe { analysis.CreateAlphaTexture(DWRITE_TEXTURE_ALIASED_1x1, &bounds, &mut pixels) }
        .ok()?;
    Some(Bitmap {
        w,
        h,
        left: bounds.left,
        top: bounds.top,
        pixels,
    })
}

/// 兜底：ClearType 三通道按亮度合成灰度。只有主路径不可用时才会走到这里。
fn cleartype_bitmap(analysis: &IDWriteGlyphRunAnalysis) -> Option<Bitmap> {
    let bounds = unsafe { analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1) }.ok()?;
    let w = (bounds.right - bounds.left).max(0) as u32;
    let h = (bounds.bottom - bounds.top).max(0) as u32;
    if w == 0 || h == 0 {
        return Some(Bitmap {
            w: 0,
            h: 0,
            left: bounds.left,
            top: bounds.top,
            pixels: Vec::new(),
        });
    }
    let mut rgb = vec![0u8; (w * h * 3) as usize];
    unsafe { analysis.CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bounds, &mut rgb) }.ok()?;
    let pixels = rgb
        .chunks_exact(3)
        .map(|p| ((299 * p[0] as u32 + 587 * p[1] as u32 + 114 * p[2] as u32) / 1000) as u8)
        .collect();
    Some(Bitmap {
        w,
        h,
        left: bounds.left,
        top: bounds.top,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dwrite::{DWrite, Wide};
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    };

    fn test_face(dw: &DWrite) -> IDWriteFontFace {
        face_by_name(dw, dw.family())
    }

    /// 画布主字族是 Segoe UI（拉丁/数字），汉字由 DWrite 排版时回退到雅黑；
    /// 图集单元测试绕过排版直接取 face，所以汉字必须显式拿雅黑面，否则拿到 .notdef。
    fn test_cjk_face(dw: &DWrite) -> IDWriteFontFace {
        for name in ["Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans SC"] {
            let mut collection = None;
            unsafe { dw.factory().GetSystemFontCollection(&mut collection, false) }.unwrap();
            let collection = collection.expect("系统字体集合");
            let wide = Wide::new(name);
            let mut index = 0u32;
            let mut exists = BOOL(0);
            unsafe { collection.FindFamilyName(wide.pcwstr(), &mut index, &mut exists) }.unwrap();
            if exists.as_bool() {
                return face_by_name(dw, name);
            }
        }
        panic!("系统里没有可用的中文字族");
    }

    fn face_by_name(dw: &DWrite, name: &str) -> IDWriteFontFace {
        let mut collection = None;
        unsafe { dw.factory().GetSystemFontCollection(&mut collection, false) }.unwrap();
        let collection = collection.expect("系统字体集合");
        let name = Wide::new(name);
        let mut index = 0u32;
        let mut exists = BOOL(0);
        unsafe { collection.FindFamilyName(name.pcwstr(), &mut index, &mut exists) }.unwrap();
        assert!(exists.as_bool(), "字体族必须存在");
        let family = unsafe { collection.GetFontFamily(index) }.unwrap();
        let font = unsafe {
            family.GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT(400),
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )
        }
        .unwrap();
        unsafe { font.CreateFontFace() }.unwrap()
    }

    fn glyph_id(face: &IDWriteFontFace, ch: char) -> u16 {
        let cp = [ch as u32];
        let mut glyph = [0u16; 1];
        unsafe { face.GetGlyphIndices(cp.as_ptr(), 1, glyph.as_mut_ptr()) }.unwrap();
        glyph[0]
    }

    fn ink(bits: &[u8]) -> usize {
        bits.iter().filter(|&&v| v > 8).count()
    }

    #[test]
    fn ppd_is_bucketed_upwards() {
        assert_eq!(quantize_ppd(1.0), 1.0);
        assert_eq!(quantize_ppd(1.1), 1.25);
        assert_eq!(quantize_ppd(1.5), 1.5);
        assert_eq!(quantize_ppd(1.51), 1.75);
        assert_eq!(quantize_ppd(3.9), 4.0);
        assert_eq!(quantize_ppd(9.0), 4.0);
        assert_eq!(quantize_ppd(0.01), 0.5);
    }

    /// 曾经把 `DWRITE_GLYPH_RUN` 从函数里返回，glyph/advance 指针悬空，
    /// 结果每个字形都变成 .notdef 方框。这条测试把「R ≠ 缺字方框」钉死。
    #[test]
    fn real_glyph_is_not_the_dotnotdef_box() {
        let Some(dw) = DWrite::new() else {
            panic!("本机没有 DirectWrite，字形测试无法成立");
        };
        let face = test_face(&dw);
        let mut atlas = GlyphAtlas::new(dw.factory());
        let factory = dw.factory();
        let bmp_r = {
            let glyph = glyph_id(&face, 'R');
            assert_ne!(glyph, 0, "R 必须有字形");
            let em = 14.0;
            let advance = em;
            let run = DWRITE_GLYPH_RUN {
                fontFace: ManuallyDrop::new(Some(face.clone())),
                fontEmSize: em,
                glyphCount: 1,
                glyphIndices: &glyph,
                glyphAdvances: &advance,
                glyphOffsets: std::ptr::null(),
                isSideways: BOOL(0),
                bidiLevel: 0,
            };
            let f2 = factory.cast::<IDWriteFactory2>().unwrap();
            let analysis = unsafe {
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
            }
            .unwrap();
            grayscale_bitmap(&analysis).unwrap()
        };
        assert!(
            bmp_r.w >= 6 && bmp_r.h >= 8,
            "R 的位图 {}×{} 太小",
            bmp_r.w,
            bmp_r.h
        );
        assert!(ink(&bmp_r.pixels) > 20, "R 应该有墨");

        let mut key = GlyphKey::new(0, glyph_id(&face, 'R'), 14.0, 1.0);
        key.ppd = 100;
        let Insert::Ready(slot) = atlas.insert(key, &face) else {
            panic!("R 应能入图集");
        };
        assert_eq!((slot.w, slot.h), (bmp_r.w, bmp_r.h));
        assert_eq!(slot.ppd, 1.0);
    }

    #[test]
    fn cjk_glyph_scales_with_ppd() {
        let Some(dw) = DWrite::new() else {
            panic!("本机没有 DirectWrite");
        };
        let face = test_cjk_face(&dw);
        let glyph = glyph_id(&face, '中');
        let mut atlas = GlyphAtlas::new(dw.factory());
        let key1 = GlyphKey::new(0, glyph, 14.0, 1.0);
        let key2 = GlyphKey::new(0, glyph, 14.0, 2.0);
        let (Insert::Ready(a), Insert::Ready(b)) =
            (atlas.insert(key1, &face), atlas.insert(key2, &face))
        else {
            panic!("中 应能入图集");
        };
        assert!(a.w >= 10 && a.h >= 10, "14px 的汉字段位图 {a:?}");
        // 光栅化在高倍率下会做像素对齐，允许几个像素的取整差
        assert!((b.w as i32 - a.w as i32 * 2).abs() <= 4, "{a:?} → {b:?}");
        assert!((b.h as i32 - a.h as i32 * 2).abs() <= 4, "{a:?} → {b:?}");
        // 两个倍率必须是不同键，不能互相顶掉
        assert!(atlas.get(&key1).is_some() && atlas.get(&key2).is_some());
    }

    #[test]
    fn atlas_packs_distinct_slots_and_reports_dirty_rects() {
        let Some(dw) = DWrite::new() else {
            panic!("本机没有 DirectWrite");
        };
        let face = test_cjk_face(&dw);
        let mut atlas = GlyphAtlas::new(dw.factory());
        let mut slots = Vec::new();
        for ch in ['中', '文', '排', '版', '测', '试'] {
            let key = GlyphKey::new(0, glyph_id(&face, ch), 14.0, 1.0);
            let Insert::Ready(slot) = atlas.insert(key, &face) else {
                panic!("{ch} 应能入图集");
            };
            assert!(!slot.is_empty(), "{ch} 位图不该为空");
            slots.push(slot);
        }
        for i in 0..slots.len() {
            for j in (i + 1)..slots.len() {
                let (a, b) = (slots[i], slots[j]);
                let overlap =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!overlap, "槽位重叠：{a:?} / {b:?}");
            }
        }
        let update = atlas.take_update();
        assert_eq!(update.rects.len(), slots.len(), "每张位图一个脏矩形");
        assert!(!update.reset);
        assert!(atlas.take_update().rects.is_empty(), "取走后不重复上报");
    }
}
