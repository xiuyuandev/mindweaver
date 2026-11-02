//! egui 侧的系统中文字体。
//!
//! 画布文字走 DirectWrite（`msyh.ttc` 索引 1 = Microsoft YaHei UI）；egui 自带字体
//! 没有 CJK，不装这一层，行内编辑器和标题栏里的中文都会变成豆腐块。
//! 粗体另注册一族 `zhisi-bold`，节点处于加粗层级时编辑器可以直接用。
//!
//! 字体文件用**只读内存映射**而不是 `fs::read`：epaint 0.37 的 `skrifa::FontRef`
//! 直接借用 blob（`self_cell`），映射后两套雅黑（约 37 MB）走系统页缓存，
//! 不再复制进私有堆——这是 P13 内存预算的关键一环。

use std::os::windows::io::AsRawHandle;
use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS,
    PAGE_READONLY,
};

/// 编辑器粗体族名。
pub const BOLD: &str = "zhisi-bold";
/// 强调族名：拉丁用 Segoe UI Semibold 真面，中文没有 600 面、落回雅黑常规。
pub const SEMIBOLD: &str = "zhisi-semibold";

const SEGOE: &str = "zhisi-segoe";
const SEGOE_SEMIBOLD: &str = "zhisi-segoe-semibold";
const YAHEI: &str = "zhisi-yahei";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let Some(found) = system_fonts() else {
        // 非 Windows 或字体文件缺失：保留 egui 默认字体，至少能看英文。
        ctx.set_fonts(fonts);
        return;
    };

    fonts.font_data.insert(YAHEI.into(), found.regular.into());
    if let Some(bold) = found.bold {
        fonts.font_data.insert(BOLD.into(), bold.into());
    }
    if let Some(latin) = found.latin {
        fonts.font_data.insert(SEGOE.into(), latin.into());
    }
    if let Some(semibold) = found.latin_semibold {
        fonts
            .font_data
            .insert(SEGOE_SEMIBOLD.into(), semibold.into());
    }
    let proportional = fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("egui 默认必有 proportional 族");
    proportional.insert(0, YAHEI.into());
    if fonts.font_data.contains_key(SEGOE) {
        // Segoe 只覆盖拉丁/数字，中文按字形回退到雅黑。
        proportional.insert(0, SEGOE.into());
    }
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .expect("egui 默认必有 monospace 族")
        .push(YAHEI.into());

    let emph_tail = |fonts: &FontDefinitions| {
        let mut family: Vec<String> = Vec::new();
        if fonts.font_data.contains_key(SEGOE_SEMIBOLD) {
            family.push(SEGOE_SEMIBOLD.into());
        }
        family
    };
    let mut bold_family = emph_tail(&fonts);
    if fonts.font_data.contains_key(BOLD) {
        bold_family.push(BOLD.into());
    }
    bold_family.push(YAHEI.into());
    for fallback in ["Ubuntu-Light", "NotoEmoji-Regular", "emoji-icon-font"] {
        if fonts.font_data.contains_key(fallback) {
            bold_family.push(fallback.into());
        }
    }
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), bold_family.clone());

    let mut semibold_family = emph_tail(&fonts);
    semibold_family.push(YAHEI.into());
    for fallback in ["Ubuntu-Light", "NotoEmoji-Regular", "emoji-icon-font"] {
        if fonts.font_data.contains_key(fallback) {
            semibold_family.push(fallback.into());
        }
    }
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), semibold_family);

    ctx.set_fonts(fonts);
}

struct SystemFonts {
    regular: FontData,
    bold: Option<FontData>,
    latin: Option<FontData>,
    latin_semibold: Option<FontData>,
}

/// 雅黑 TTC 的索引 1 是 UI 版，与 DirectWrite 的族名一致；
/// 雅黑缺失时退回等线（单文件 TTF，索引 0）。
fn system_fonts() -> Option<SystemFonts> {
    let dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("Fonts");

    let read = |name: &str, index: u32| -> Option<FontData> {
        let blob = map_font(&dir.join(name))?;
        Some(FontData::from_blob(blob, index))
    };

    let regular = read("msyh.ttc", 1).or_else(|| read("Deng.ttf", 0))?;
    Some(SystemFonts {
        regular,
        bold: read("msyhbd.ttc", 1),
        latin: read("segoeui.ttf", 0).or_else(|| read("SegUIVar.ttf", 0)),
        latin_semibold: read("seguisb.ttf", 0),
    })
}

/// 只读映射的字体文件。字节由系统页缓存提供，`Drop` 时解除映射。
struct MappedFont {
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    len: usize,
}

// SAFETY: 视图以 PAGE_READONLY + FILE_MAP_READ 创建，存活期内内容不可变；
// 解除映射只发生在最后一个 `Arc` 引用释放时。
unsafe impl Send for MappedFont {}
unsafe impl Sync for MappedFont {}

impl AsRef<[u8]> for MappedFont {
    fn as_ref(&self) -> &[u8] {
        // SAFETY: 构造时保证 `view` 非空且覆盖 `len` 字节。
        unsafe { std::slice::from_raw_parts(self.view.Value.cast::<u8>(), self.len) }
    }
}

impl Drop for MappedFont {
    fn drop(&mut self) {
        // SAFETY: `view` 由 `MapViewOfFile` 创建，且只在这里解除一次。
        let _ = unsafe { UnmapViewOfFile(self.view) };
    }
}

fn map_font(path: &std::path::Path) -> Option<Arc<MappedFont>> {
    let file = std::fs::File::open(path).ok()?;
    let len = usize::try_from(file.metadata().ok()?.len()).ok()?;
    if len == 0 {
        return None;
    }
    // SAFETY: 文件句柄在调用期间有效；只读、匿名映射。
    let mapping = unsafe {
        CreateFileMappingW(
            HANDLE(file.as_raw_handle()),
            None,
            PAGE_READONLY,
            0,
            0,
            PCWSTR::null(),
        )
    }
    .ok()?;
    // SAFETY: 映射句柄有效；长度 0 = 映射整个文件。
    let view = unsafe { MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0) };
    // 视图持有映射对象的引用，句柄可以立即关闭。
    let _ = unsafe { CloseHandle(mapping) };
    if view.Value.is_null() {
        return None;
    }
    Some(Arc::new(MappedFont { view, len }))
}
