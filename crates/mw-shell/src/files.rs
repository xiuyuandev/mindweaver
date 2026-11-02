//! 原生文件对话框（Win32 `IFileOpenDialog` / `IFileSaveDialog`）。
//!
//! 不用 `rfd`：一个对话框不值得多一棵依赖树，而且它在 Windows 上包的也是这同一个 COM 接口。
//! 这里只做「拿到一个路径」，**.zmind 的读写全在 `mw-format`、会话状态全在 `session`**。

use std::path::PathBuf;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    Common::COMDLG_FILTERSPEC, FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog,
    ShellExecuteW, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::export::with_ext;

/// `HRESULT_FROM_WIN32(ERROR_CANCELLED)`。用户按"取消"走这条，不该报错。
const CANCELLED: i32 = 0x8007_04C7u32 as i32;

fn com_init() {
    // 已经初始化（含 MTA）时返回 S_FALSE / RPC_E_CHANGED_MODE，都不影响用对话框：
    // 我们只是需要一个已初始化的套间，不关心是 STA 还是 MTA。
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

fn err(e: windows::core::Error) -> String {
    format!("文件对话框失败：{e}")
}

/// 取对话框结果的**文件系统路径**（并释放 COM 分配的内存）。
///
/// `GetDisplayName` 返回的是 `CoTaskMemAlloc` 出来的字符串，不释放就是每次打开漏一点。
unsafe fn display_path(item: &windows::Win32::UI::Shell::IShellItem) -> Result<PathBuf, String> {
    let wide = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(err)?;
    let text = wide.to_string().unwrap_or_default();
    CoTaskMemFree(Some(wide.0 as *const core::ffi::c_void));
    Ok(PathBuf::from(text))
}

/// 把主窗口句柄转成对话框的 owner。
///
/// **必须给 owner**：不给的话对话框没有父子关系，会跑到主窗口后面去——探针第一版就是
/// 这样，截图里什么都没有，看起来像"Ctrl+O 没反应"。
fn owner(hwnd: Option<isize>) -> Option<HWND> {
    hwnd.map(|h| HWND(h as *mut core::ffi::c_void))
}

/// 打开 `.zmind`。`Ok(None)` = 用户取消。
pub fn open_zmind(hwnd: Option<isize>) -> Result<Option<PathBuf>, String> {
    com_init();
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        let opts = dialog.GetOptions().map_err(err)?;
        dialog
            .SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST)
            .map_err(err)?;
        dialog.SetTitle(w!("打开织思文档")).map_err(err)?;
        // 文档格式过滤器（开放格式 + 旧版），加「所有文件」兜底。
        let spec_name = HSTRING::from("织思文档 (*.xmind;*.cxl;*.zmind;*.md)");
        let spec = HSTRING::from("*.xmind;*.cxl;*.zmind;*.md");
        let all_name = HSTRING::from("所有文件 (*.*)");
        let all_spec = HSTRING::from("*.*");
        let filters = [
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(spec_name.as_ptr()),
                pszSpec: PCWSTR(spec.as_ptr()),
            },
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(all_name.as_ptr()),
                pszSpec: PCWSTR(all_spec.as_ptr()),
            },
        ];
        dialog.SetFileTypes(&filters).map_err(err)?;
        dialog.SetFileTypeIndex(1).map_err(err)?;
        match dialog.Show(owner(hwnd)) {
            Ok(()) => {}
            Err(e) if e.code().0 == CANCELLED => return Ok(None),
            Err(e) => return Err(err(e)),
        }
        let item = dialog.GetResult().map_err(err)?;
        display_path(&item).map(Some)
    }
}

/// 导入外部格式（MD / 大纲文本 / OPML / FreeMind / XMind）。
///
/// 和「打开」共用 `IFileOpenDialog`，差别只有标题与过滤器：把能吃的后缀列出来，
/// 用户才不用猜 `.mm`、`.opml` 能不能拖进来。对话框本身不解析文件——选完仍交给
/// `Session::import` 按扩展名分流。
pub fn open_import(hwnd: Option<isize>) -> Result<Option<PathBuf>, String> {
    com_init();
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        let opts = dialog.GetOptions().map_err(err)?;
        dialog
            .SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST)
            .map_err(err)?;
        dialog.SetTitle(w!("导入文件")).map_err(err)?;
        // 两条过滤器（可导入格式 / 所有文件）。名字与通配符字符串必须活到 `SetFileTypes` 之后。
        let spec_name = HSTRING::from("可导入的文件 (*.md;*.markdown;*.txt;*.opml;*.mm;*.xmind)");
        let spec = HSTRING::from("*.md;*.markdown;*.txt;*.opml;*.mm;*.xmind");
        let all_name = HSTRING::from("所有文件 (*.*)");
        let all_spec = HSTRING::from("*.*");
        let filters = [
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(spec_name.as_ptr()),
                pszSpec: PCWSTR(spec.as_ptr()),
            },
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(all_name.as_ptr()),
                pszSpec: PCWSTR(all_spec.as_ptr()),
            },
        ];
        dialog.SetFileTypes(&filters).map_err(err)?;
        dialog.SetFileTypeIndex(1).map_err(err)?;
        match dialog.Show(owner(hwnd)) {
            Ok(()) => {}
            Err(e) if e.code().0 == CANCELLED => return Ok(None),
            Err(e) => return Err(err(e)),
        }
        let item = dialog.GetResult().map_err(err)?;
        display_path(&item).map(Some)
    }
}

/// 剪贴板里的图片 → PNG 字节。没有图片（或剪贴板不可用）返回 `None`。
pub fn clipboard_image() -> Option<Vec<u8>> {
    let mut cb = arboard::Clipboard::new().ok()?;
    let img = cb.get_image().ok()?;
    Some(mw_core::image::encode_rgba_png(
        img.width as u32,
        img.height as u32,
        &img.bytes,
    ))
}

/// 选一张图片作为节点附件。只做文件选择，解码/存资源由调用方处理。
pub fn open_image(hwnd: Option<isize>) -> Result<Option<PathBuf>, String> {
    com_init();
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        let opts = dialog.GetOptions().map_err(err)?;
        dialog
            .SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST)
            .map_err(err)?;
        dialog.SetTitle(w!("选择图片")).map_err(err)?;
        let spec_name = HSTRING::from("图片 (*.png;*.jpg;*.jpeg;*.gif;*.webp)");
        let spec = HSTRING::from("*.png;*.jpg;*.jpeg;*.gif;*.webp");
        let all_name = HSTRING::from("所有文件 (*.*)");
        let all_spec = HSTRING::from("*.*");
        let filters = [
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(spec_name.as_ptr()),
                pszSpec: PCWSTR(spec.as_ptr()),
            },
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(all_name.as_ptr()),
                pszSpec: PCWSTR(all_spec.as_ptr()),
            },
        ];
        dialog.SetFileTypes(&filters).map_err(err)?;
        dialog.SetFileTypeIndex(1).map_err(err)?;
        match dialog.Show(owner(hwnd)) {
            Ok(()) => {}
            Err(e) if e.code().0 == CANCELLED => return Ok(None),
            Err(e) => return Err(err(e)),
        }
        let item = dialog.GetResult().map_err(err)?;
        display_path(&item).map(Some)
    }
}

/// 另存为文档。`ext` 按文档类型走：思维导图 `.xmind`、概念图 `.cxl`（`docs/34` §6）。
/// 用户没写扩展名就补上——否则存出一个没有后缀的文件，
/// 下次"打开"对话框按扩展名过滤时就看不见它了。
pub fn save_doc(
    default_name: &str,
    ext: &str,
    label: &str,
    hwnd: Option<isize>,
) -> Result<Option<PathBuf>, String> {
    com_init();
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        let opts = dialog.GetOptions().map_err(err)?;
        dialog
            .SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT)
            .map_err(err)?;
        dialog
            .SetTitle(&HSTRING::from(format!("保存为{label}")))
            .map_err(err)?;
        let filter_name = HSTRING::from(format!("{label} (*.{ext})"));
        let filter_spec = HSTRING::from(format!("*.{ext}"));
        let filters = [COMDLG_FILTERSPEC {
            pszName: PCWSTR(filter_name.as_ptr()),
            pszSpec: PCWSTR(filter_spec.as_ptr()),
        }];
        dialog.SetFileTypes(&filters).map_err(err)?;
        dialog.SetFileTypeIndex(1).map_err(err)?;
        dialog
            .SetDefaultExtension(&HSTRING::from(ext))
            .map_err(err)?;
        dialog
            .SetFileName(&HSTRING::from(default_name))
            .map_err(err)?;
        match dialog.Show(owner(hwnd)) {
            Ok(()) => {}
            Err(e) if e.code().0 == CANCELLED => return Ok(None),
            Err(e) => return Err(err(e)),
        }
        let item = dialog.GetResult().map_err(err)?;
        display_path(&item).map(|p| Some(with_ext(p, ext)))
    }
}

/// 导出的保存对话框。和 `save_doc` 共用同一个 `IFileSaveDialog`，差别只有三处：
/// 文件类型过滤器只列当前格式、标题带格式名、兜底后缀跟着格式走。
///
/// **过滤器是必要的**：没有它，另存为对话框的类型框会写「所有文件」，用户看不出
/// 自己正在存什么；而且 Windows 只有在类型框认得该后缀时才会帮忙补后缀。
pub fn save_export(
    default_name: &str,
    ext: &str,
    label: &str,
    hwnd: Option<isize>,
) -> Result<Option<PathBuf>, String> {
    com_init();
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        let opts = dialog.GetOptions().map_err(err)?;
        dialog
            .SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT)
            .map_err(err)?;
        dialog
            .SetTitle(&HSTRING::from(format!("导出为{label}")))
            .map_err(err)?;
        // 过滤器名与通配符是两条独立字符串，`SetFileTypes` 只借指针，必须活到调用之后。
        let filter_name = HSTRING::from(format!("{label} (*.{ext})"));
        let filter_spec = HSTRING::from(format!("*.{ext}"));
        let filters = [COMDLG_FILTERSPEC {
            pszName: PCWSTR(filter_name.as_ptr()),
            pszSpec: PCWSTR(filter_spec.as_ptr()),
        }];
        dialog.SetFileTypes(&filters).map_err(err)?;
        dialog.SetFileTypeIndex(1).map_err(err)?;
        dialog
            .SetDefaultExtension(&HSTRING::from(ext))
            .map_err(err)?;
        dialog
            .SetFileName(&HSTRING::from(default_name))
            .map_err(err)?;
        match dialog.Show(owner(hwnd)) {
            Ok(()) => {}
            Err(e) if e.code().0 == CANCELLED => return Ok(None),
            Err(e) => return Err(err(e)),
        }
        let item = dialog.GetResult().map_err(err)?;
        display_path(&item).map(|p| Some(with_ext(p, ext)))
    }
}

/// 用系统默认浏览器打开 http(s) 链接。
///
/// 入口在检查器，URL 可能来自导入文件——**只放行 http/https**，
/// 别让 `file:` / `javascript:` 之类的东西经 ShellExecute 落到系统上。
pub fn open_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("只支持 http/https 链接".into());
    }
    let file = HSTRING::from(url);
    let h = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &file,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW 的返回值 <= 32 是错误码，不是句柄。
    if h.0 as isize <= 32 {
        Err(format!("无法打开链接（{}）", h.0 as isize))
    } else {
        Ok(())
    }
}

/// 在资源管理器里定位文档（右键菜单「打开文档所在文件夹」）。
///
/// 用 `explorer.exe /select,"路径"`：直接 `open` 文件会让默认应用接管，
/// 而这里要的是「文件被选中」的窗口。路径带空格时引号必须保留——Windows
/// 的资源管理器把它当作一个整体参数。
pub fn reveal_in_explorer(path: &std::path::Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("文档不在磁盘上，可能已被移动或删除".into());
    }
    let exe = HSTRING::from("explorer.exe");
    let params = HSTRING::from(format!("/select,\"{}\"", path.display()));
    let h = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &exe,
            &params,
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if h.0 as isize <= 32 {
        Err(format!("无法打开资源管理器（{}）", h.0 as isize))
    } else {
        Ok(())
    }
}
