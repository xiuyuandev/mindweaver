//! 窗口几何与面板开合的持久化。
//!
//! 存 `%APPDATA%\织思\window.json`。**窗口矩形是外框物理像素**——DPI 缩放变化时不用换算，
//! 多显示器之间也能原样搬回去。恢复前校验「标题栏中心点还落在某台显示器上」：
//! 显示器拔掉、分辨率改小、窗口被拖到屏幕外，都要能安全退回默认位置，而不是开在一个
//! 用户永远够不着的地方。
//!
//! `MW_WINDOW_STATE=<path>` 换存储位置，`MW_WINDOW_STATE=off` 关掉持久化——
//! 验收探针要的是可复现的初始窗口，不能受本机上次摆哪儿、上次关没关大纲影响。
//! golden harness 自己在 `Shell::new` 里走 [`WindowState::disabled`]，不依赖环境变量。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 外框物理像素 `[x, y, w, h]`。
pub type Rect = [i32; 4];

/// 比这还小的"窗口"多半是损坏的存档：最小内容区是 640×460 逻辑像素。
const MIN_W: i32 = 320;
const MIN_H: i32 = 240;
/// 坐标合法性上界：虚拟桌面再大也用不到 ±10 万像素。
const LIMIT: i32 = 100_000;
/// 从外框顶边往下取这么深的位置当"标题栏锚点"（56px 标题栏的中间）。
const TITLE_DEPTH: i32 = 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    rect: Option<Rect>,
    maximized: bool,
    /// 上次退出时是否停留在全屏大纲（旧版字段同名字段曾表示左侧抽屉）。
    show_outline: bool,
    show_inspector: bool,
    /// 右下角缩略卡（M30）**是否开**。出厂 `false`：浮层是新东西，
    /// 既有 205 张基线一张都不该因为多一块浮层而重拍——默认关才谈得上零漂移。
    show_minimap: bool,
    /// 检查器宽度（逻辑像素）。旧存档没有这个字段时用默认值。
    inspector_w: f32,
    /// 笔记库的侧栏与列表宽（`docs/28` §7.2「宽度写入会话状态」）。
    /// 旧存档缺键即默认 232/360，不升版本、不写迁移。
    library_sidebar_w: f32,
    library_list_w: f32,
    /// 检查器里**用户改过**的分组开合（`docs/30` §4.1「折叠状态记忆到设置」）。
    /// 每条是 `键=0/1`：没出现过的键走各组自己的默认（标记组默认折叠），
    /// 所以存档里只留用户真正动过的那几条。
    inspector_groups: Vec<String>,
    /// 大纲行密度：0 紧凑 / 1 标准 / 2 宽松。存序号而不是字符串，
    /// 旧存档读到坏值只回落标准，不会让整份窗口状态解析失败。
    outline_density: u8,
    /// 首次引导是否已看过/跳过（P42）。旧存档默认 false。
    onboarded: bool,
    /// 上次打开的文件标签（按顺序）与激活下标。
    tabs: Vec<String>,
    active_tab: usize,
}

impl Default for Store {
    /// 缺字段时与当前版本首次启动的默认布局一致：打开就是画布，检查器关。
    fn default() -> Self {
        Self {
            rect: None,
            maximized: false,
            show_outline: false,
            show_inspector: false,
            show_minimap: false,
            inspector_w: crate::panels::inspector::PANEL_W,
            library_sidebar_w: crate::note_library::SIDEBAR_W,
            library_list_w: crate::note_library::LIST_W,
            inspector_groups: Vec::new(),
            outline_density: 1,
            tabs: Vec::new(),
            active_tab: 0,
            onboarded: false,
        }
    }
}

pub struct WindowState {
    store: Store,
    /// 上次写到盘上的内容。窗口移动每帧都在变，没必要每 700ms 重写一个没变的文件。
    saved: Store,
    file: Option<PathBuf>,
}

impl WindowState {
    /// 读默认位置（或 `MW_WINDOW_STATE` 指定位置）。坏文件/读不到都退化成默认值：
    /// 一个记忆文件坏了，不该让应用起不来。
    pub fn load() -> Self {
        match std::env::var_os("MW_WINDOW_STATE") {
            Some(v) if v.eq_ignore_ascii_case("off") => Self::disabled(),
            Some(v) => Self::load_from(Path::new(&v)),
            None => match default_path() {
                Some(p) => Self::load_from(&p),
                None => Self::disabled(),
            },
        }
    }

    pub fn load_from(file: &Path) -> Self {
        let mut store = std::fs::read_to_string(file)
            .ok()
            .and_then(|s| serde_json::from_str::<Store>(&s).ok())
            .unwrap_or_default();
        // 存档里的矩形坏掉时只丢矩形，面板开关照常生效。
        if store.rect.is_some_and(|r| !sane(r)) {
            store.rect = None;
        }
        Self {
            saved: store.clone(),
            store,
            file: Some(file.to_path_buf()),
        }
    }

    /// 不读盘也不写盘。golden harness 用它保证基线与本机上次的窗口状态无关。
    pub fn disabled() -> Self {
        Self {
            store: Store::default(),
            saved: Store::default(),
            file: None,
        }
    }

    pub fn panels(&self) -> (bool, bool) {
        (self.store.show_outline, self.store.show_inspector)
    }

    /// 右下角缩略卡（M30）是否开。
    pub fn show_minimap(&self) -> bool {
        self.store.show_minimap
    }

    /// 记一次缩略卡开合。它不走每帧的 [`WindowState::remember`]：那条管几何与左右面板，
    /// 这条只在用户真按下那一刻写盘。
    pub fn set_show_minimap(&mut self, on: bool) {
        self.store.show_minimap = on;
        self.save_if_changed();
    }

    pub fn maximized(&self) -> bool {
        self.store.maximized
    }

    /// 检查器宽度，限制在可用区间内（`docs/30` §4.1：默认 288，可拖 240–360），
    /// 旧存档/坏值不会撑爆布局。
    pub fn inspector_width(&self) -> f32 {
        self.store.inspector_w.clamp(
            crate::panels::inspector::PANEL_W_MIN,
            crate::panels::inspector::PANEL_W_MAX,
        )
    }

    pub fn set_inspector_width(&mut self, w: f32) {
        self.store.inspector_w = w.clamp(
            crate::panels::inspector::PANEL_W_MIN,
            crate::panels::inspector::PANEL_W_MAX,
        );
        self.save_if_changed();
    }

    /// 笔记库两栏的宽（`docs/28` §7.2：侧栏 232 可拖 200–280，列表 360 可拖 320–440）。
    /// 与检查器同一条口径：读的时候夹一次，坏存档/手改 json 都不会撑爆分栏。
    pub fn library_widths(&self) -> (f32, f32) {
        (
            self.store.library_sidebar_w.clamp(
                crate::note_library::SIDEBAR_MIN,
                crate::note_library::SIDEBAR_MAX,
            ),
            self.store
                .library_list_w
                .clamp(crate::note_library::LIST_MIN, crate::note_library::LIST_MAX),
        )
    }

    /// 只在拖宽**落手**那一刻调用（`NoteLibrary::take_width_change`）：
    /// 每帧回写会在一手拖动里写几十次盘。
    pub fn set_library_widths(&mut self, sidebar_w: f32, list_w: f32) {
        self.store.library_sidebar_w = sidebar_w.clamp(
            crate::note_library::SIDEBAR_MIN,
            crate::note_library::SIDEBAR_MAX,
        );
        self.store.library_list_w =
            list_w.clamp(crate::note_library::LIST_MIN, crate::note_library::LIST_MAX);
        self.save_if_changed();
    }

    /// 用户记过的检查器分组开合；`None` = 没动过，按组的默认值来。
    pub fn inspector_groups(&self) -> &[String] {
        &self.store.inspector_groups
    }
    /// 记一个分组的开合。同一键只留一条（后写的覆盖先写的）。
    pub fn set_inspector_group(&mut self, key: &str, open: bool) {
        if self.put_group(key, open) {
            self.save_if_changed();
        }
    }

    /// 证据用（`docs/31` SE-06）：启动即预置分组开合，**只写内存不落盘**。
    ///
    /// 写完后把 `saved` 也对齐，于是 `save_if_changed` 认为「没变」——金样拍
    /// 「默认折叠的那一组」时，不会顺手改掉用户自己的记忆文件。
    pub fn seed_inspector_group(&mut self, key: &str, open: bool) {
        self.put_group(key, open);
        self.saved = self.store.clone();
    }

    /// 写一条分组开合，返回是否真的改了（没改就不用落盘）。
    fn put_group(&mut self, key: &str, open: bool) -> bool {
        let entry = format!("{key}={}", open as u8);
        let list = &mut self.store.inspector_groups;
        match list
            .iter()
            .position(|e| e.split_once('=').map(|(k, _)| k) == Some(key))
        {
            Some(i) => {
                if list[i] == entry {
                    return false;
                }
                list[i] = entry;
            }
            None => list.push(entry),
        }
        true
    }

    /// 大纲行密度序号（0 紧凑 / 1 标准 / 2 宽松）。坏值回落标准。
    pub fn outline_density(&self) -> u8 {
        self.store.outline_density.min(2)
    }

    pub fn set_outline_density(&mut self, density: u8) {
        self.store.outline_density = density.min(2);
        self.save_if_changed();
    }

    pub fn onboarded(&self) -> bool {
        self.store.onboarded
    }

    /// 上次的标签顺序（只含文件路径；未命名标签不恢复）。
    pub fn tab_paths(&self) -> &[String] {
        &self.store.tabs
    }

    pub fn active_tab(&self) -> usize {
        self.store.active_tab
    }

    pub fn remember_tabs(&mut self, tabs: Vec<String>, active: usize) {
        self.store.tabs = tabs;
        self.store.active_tab = active;
        self.save_if_changed();
    }

    /// 看完或跳过首次引导后调用；只写一次盘。
    pub fn set_onboarded(&mut self, done: bool) {
        self.store.onboarded = done;
        self.save_if_changed();
    }

    /// 可以安全恢复的矩形：格式合法且标题栏还够得着。`None` 时调用方走默认居中。
    pub fn restorable_rect(&self) -> Option<Rect> {
        self.store.rect.filter(|&r| sane(r) && on_screen(r))
    }

    /// 记一次窗口状态。`rect = None` 表示这次只存面板（比如专注模式退出时拿不到常态几何）。
    /// 内容没变就不落盘。
    pub fn remember(&mut self, rect: Option<Rect>, maximized: bool, panels: (bool, bool)) {
        if let Some(r) = rect.filter(|&r| sane(r)) {
            self.store.rect = Some(r);
            self.store.maximized = maximized;
        }
        self.store.show_outline = panels.0;
        self.store.show_inspector = panels.1;
        self.save_if_changed();
    }

    fn save_if_changed(&mut self) {
        if self.store == self.saved {
            return;
        }
        let Some(file) = self.file.as_ref() else {
            // 关掉持久化时也更新 saved，免得每帧走到这里再比较一遍大结构。
            self.saved = self.store.clone();
            return;
        };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.store) {
            // 写失败只是丢掉记忆，不影响窗口继续用。
            let _ = std::fs::write(file, json);
            self.saved = self.store.clone();
        }
    }
}

/// 矩形数值本身是否可信（尺寸、坐标范围）。与显示器无关，可单测。
fn sane(rect: Rect) -> bool {
    let [x, y, w, h] = rect;
    w >= MIN_W && h >= MIN_H && w <= LIMIT && h <= LIMIT && x.abs() <= LIMIT && y.abs() <= LIMIT
}

/// 标题栏上的"抓手"：居中、顶边往下 20px。恢复判定以它为准——只要这一点还在屏幕上，
/// 用户就抓得住窗口，哪怕左右或下半部分探出了屏幕。
fn title_anchor(rect: Rect) -> (i32, i32) {
    let [x, y, w, _] = rect;
    (x + w / 2, y + TITLE_DEPTH)
}

/// 锚点是否在 `[x, y, w, h]` 矩形内（半开区间）。纯函数，供单测与 [`on_screen`] 共用。
fn inside(rect: Rect, point: (i32, i32)) -> bool {
    let [x, y, w, h] = rect;
    point.0 >= x && point.0 < x + w && point.1 >= y && point.1 < y + h
}

/// 标题栏锚点是否落在某台显示器上。显示器被拔掉时这里返回 `false`，调用方退回默认布局。
fn on_screen(rect: Rect) -> bool {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONULL,
    };
    let (ax, ay) = title_anchor(rect);
    unsafe {
        let mon = MonitorFromPoint(POINT { x: ax, y: ay }, MONITOR_DEFAULTTONULL);
        if mon.is_invalid() {
            return false;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(mon, &mut info).as_bool() {
            return false;
        }
        let m = info.rcMonitor;
        inside(
            [m.left, m.top, m.right - m.left, m.bottom - m.top],
            (ax, ay),
        )
    }
}

/// 当前窗口的「常态几何 + 是否最大化」。
///
/// 最大化时 `GetWindowRect` 给的是铺满屏幕的矩形，直接存下去会把「还原」位置覆盖掉；
/// 这时改用 `GetWindowPlacement` 的 `rcNormalPosition`（就是双击标题栏还原后的位置）。
pub fn read_placement(hwnd: isize) -> Option<(Rect, bool)> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, GetWindowRect, IsZoomed, WINDOWPLACEMENT,
    };
    let h = HWND(hwnd as *mut core::ffi::c_void);
    unsafe {
        if IsZoomed(h).as_bool() {
            let mut p = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            GetWindowPlacement(h, &mut p).ok()?;
            let r = p.rcNormalPosition;
            return Some(([r.left, r.top, r.right - r.left, r.bottom - r.top], true));
        }
        let mut r = RECT::default();
        GetWindowRect(h, &mut r).ok()?;
        Some(([r.left, r.top, r.right - r.left, r.bottom - r.top], false))
    }
}

/// 把窗口外框搬到 `rect`。同步执行（不排队）：下一帧 DWM 合成之前位置就已经对了，
/// 不会先闪一下默认居中的位置再跳过去。
pub fn apply_rect(hwnd: isize, rect: Rect) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
    };
    let [x, y, w, h] = rect;
    unsafe {
        let _ = SetWindowPos(
            HWND(hwnd as *mut core::ffi::c_void),
            None,
            x,
            y,
            w,
            h,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    Some(base.join("织思").join("window.json"))
}

#[cfg(test)]
mod tests {
    use super::{inside, sane, title_anchor, Store, WindowState};
    use std::path::PathBuf;

    fn temp_file() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mw-window-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("window.json")
    }

    #[test]
    fn sane_rejects_tiny_and_absurd_rects() {
        assert!(sane([100, 100, 1080, 720]));
        assert!(!sane([100, 100, 100, 720]), "窄得抓不住");
        assert!(!sane([100, 100, 1080, 100]), "矮得抓不住");
        assert!(!sane([200_000, 0, 1080, 720]), "坐标离谱");
        assert!(!sane([0, 0, 200_000, 720]), "尺寸离谱");
    }

    #[test]
    fn tab_order_and_active_index_are_remembered() {
        let mut s = WindowState::disabled();
        s.remember_tabs(vec!["a.zmind".into(), "b.zmind".into()], 1);
        assert_eq!(s.tab_paths(), ["a.zmind", "b.zmind"]);
        assert_eq!(s.active_tab(), 1);
    }

    #[test]
    fn title_anchor_sits_in_the_top_center() {
        assert_eq!(title_anchor([100, 200, 800, 600]), (500, 220));
        // 负坐标显示器（主屏左侧）同样成立。
        assert_eq!(title_anchor([-1920, 0, 800, 600]), (-1520, 20));
    }

    #[test]
    fn inside_uses_half_open_bounds() {
        let mon = [-1920, 0, 1920, 1080];
        assert!(inside(mon, (-1000, 20)));
        assert!(!inside(mon, (0, 20)), "右边界不算在内");
        assert!(!inside(mon, (-2000, 20)), "左边界外");
    }

    #[test]
    fn a_missing_file_degrades_to_current_defaults() {
        let file = temp_file();
        let st = WindowState::load_from(&file);
        assert_eq!(st.panels(), (false, false));
        assert!(st.restorable_rect().is_none());
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn a_corrupt_file_keeps_defaults_instead_of_panicking() {
        let file = temp_file();
        std::fs::write(&file, "{ not json").unwrap();
        let st = WindowState::load_from(&file);
        assert_eq!(st.panels(), (false, false));
        assert!(st.restorable_rect().is_none());
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn stale_rect_is_dropped_but_panels_survive() {
        let file = temp_file();
        let json = serde_json::json!({
            "rect": [10, 10, 50, 50],
            "show_outline": false,
            "show_inspector": true
        });
        std::fs::write(&file, json.to_string()).unwrap();
        let st = WindowState::load_from(&file);
        assert_eq!(st.panels(), (false, true), "面板开关照常读回");
        assert!(st.restorable_rect().is_none(), "坏矩形要丢掉");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn remembering_round_trips_geometry_and_panels() {
        let file = temp_file();
        let mut st = WindowState::load_from(&file);
        st.remember(Some([120, 80, 1280, 860]), true, (false, true));

        let again = WindowState::load_from(&file);
        assert_eq!(again.panels(), (false, true));
        assert!(again.maximized());
        let raw: Store = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(raw.rect, Some([120, 80, 1280, 860]));
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn inspector_width_round_trips_and_clamps() {
        let file = temp_file();
        let mut st = WindowState::load_from(&file);
        // 默认 288（`docs/30` §4.1）。
        assert_eq!(st.inspector_width(), 288.0);
        st.set_inspector_width(333.0);
        let again = WindowState::load_from(&file);
        assert_eq!(again.inspector_width(), 333.0);
        let mut st = again;
        // 合约区间 240–360，超出按端点记。
        st.set_inspector_width(9999.0);
        assert_eq!(st.inspector_width(), 360.0);
        st.set_inspector_width(10.0);
        assert_eq!(st.inspector_width(), 240.0);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn collapsed_groups_round_trip_and_keep_one_entry_each() {
        let file = temp_file();
        let mut st = WindowState::load_from(&file);
        assert!(
            st.inspector_groups().is_empty(),
            "旧存档默认按各组自己的默认"
        );
        st.set_inspector_group("style/color", false);
        st.set_inspector_group("style/color", false);
        st.set_inspector_group("mark/annotations", true);
        assert_eq!(
            st.inspector_groups(),
            ["style/color=0", "mark/annotations=1"]
        );
        let again = WindowState::load_from(&file);
        assert_eq!(again.inspector_groups().len(), 2, "开合状态要落盘");
        let mut st = again;
        st.set_inspector_group("style/color", true);
        assert_eq!(
            st.inspector_groups(),
            ["style/color=1", "mark/annotations=1"]
        );
        // 同一个状态重复记不该写盘也不该报错。
        st.set_inspector_group("style/color", true);
        assert_eq!(
            st.inspector_groups(),
            ["style/color=1", "mark/annotations=1"]
        );
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn seeded_groups_stay_in_memory_and_never_touch_the_file() {
        let file = temp_file();
        // 先落一条真状态，否则「没写盘」和「文件不存在」分不清。
        let mut st = WindowState::load_from(&file);
        st.set_inspector_group("style/color", true);
        let before = std::fs::read_to_string(&file).unwrap();

        // 金样预置：颜色折叠、形状与文字展开。
        st.seed_inspector_group("style/color", false);
        st.seed_inspector_group("style/shape", true);
        assert_eq!(
            st.inspector_groups(),
            ["style/color=0", "style/shape=1"],
            "内存里立刻生效"
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            before,
            "证据预置不许动用户的记忆文件"
        );
        // 之后再真的点一下分组，才轮到落盘——而且带着预置值一起写。
        st.set_inspector_group("style/shape", false);
        let again = WindowState::load_from(&file);
        assert_eq!(again.inspector_groups(), ["style/color=0", "style/shape=0"]);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn the_minimap_defaults_off_and_remembers_being_on() {
        let file = temp_file();
        let st = WindowState::load_from(&file);
        assert!(
            !st.show_minimap(),
            "出厂必须关：既有 205 张基线不该因为多一块浮层而重拍"
        );

        let mut st = WindowState::load_from(&file);
        st.set_show_minimap(true);
        let again = WindowState::load_from(&file);
        assert!(again.show_minimap(), "开合要落盘");
        assert_eq!(
            again.panels(),
            (false, false),
            "记小地图不许顺手改掉左右面板"
        );

        let mut st = again;
        st.set_show_minimap(false);
        assert!(!WindowState::load_from(&file).show_minimap());
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn outline_density_round_trips_and_clamps() {
        let file = temp_file();
        let mut st = WindowState::load_from(&file);
        assert_eq!(st.outline_density(), 1, "旧存档默认标准密度");
        st.set_outline_density(2);
        let again = WindowState::load_from(&file);
        assert_eq!(again.outline_density(), 2);
        let mut st = again;
        st.set_outline_density(9);
        assert_eq!(st.outline_density(), 2);
        let raw: Store = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(raw.outline_density, 2);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn an_unchanged_state_is_not_rewritten() {
        let file = temp_file();
        let mut st = WindowState::load_from(&file);
        st.remember(Some([120, 80, 1280, 860]), false, (true, false));
        let first = std::fs::read_to_string(&file).unwrap();
        // 把文件改成不可能由 remember 写出的内容；没变化就不该覆盖它。
        std::fs::write(&file, "sentinel").unwrap();
        st.remember(Some([120, 80, 1280, 860]), false, (true, false));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "sentinel");
        assert!(!first.is_empty());
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn disabled_state_never_touches_disk() {
        let file = temp_file();
        let mut st = WindowState::disabled();
        st.remember(Some([0, 0, 1280, 720]), false, (false, false));
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }
}
