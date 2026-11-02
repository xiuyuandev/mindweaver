//! 视口相机：世界坐标（逻辑 px）↔ 物理像素。
//!
//! 一条约定贯穿整个渲染层：**Scene 里的数字永远是世界单位（逻辑 px）**，
//! 而「屏幕上恒定大小」的铬件（选中环、折叠圆点、吸附阈值）是 CSS px 常量，
//! 必须先 `css_to_world()` 除以缩放再写进实例（docs/13 §3、docs/12 D1）。

use bytemuck::{Pod, Zeroable};
use mw_core::Point;

/// 着色器 uniform。字段顺序与 `lattice.wgsl` / `rect.wgsl` 里的 `Uni` 严格一致。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CameraUni {
    /// 视口中心的世界坐标
    pub center: [f32; 2],
    /// 视口物理像素尺寸
    pub viewport: [f32; 2],
    pub scale: f32,
    pub ppp: f32,
    /// (点阵间距·世界单位, 点半径·CSS px)
    pub grid: [f32; 2],
    /// 画布底色（sRGB 编码值）
    pub bg: [f32; 4],
    /// 点阵颜色
    pub dot: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// 视口中心对准的世界坐标
    pub center: Point,
    /// 缩放：1.0 = 100%
    pub zoom: f32,
    /// 系统缩放（100% = 1.0，150% = 1.5）
    pub ppp: f32,
}

impl Camera {
    pub fn new(center: Point, zoom: f32, ppp: f32) -> Self {
        Self {
            center,
            zoom: zoom.max(0.005),
            ppp,
        }
    }

    /// 屏幕恒定尺寸（CSS px）→ 世界单位。2px 的选中环在任何缩放档位都是 2px。
    pub fn css_to_world(&self, css: f32) -> f32 {
        css / self.zoom
    }

    /// 世界单位长度是否有资格按 CSS px 常量绘制：缩放太小时恒定像素会把图形糊成一团，
    /// 这时改用世界单位下限（等同旧版 `Math.max(4.5, 8 * invScale)` 的写法）。
    pub fn chrome(&self, css: f32, world_floor: f32) -> f32 {
        self.css_to_world(css).max(world_floor)
    }

    pub fn uni(
        &self,
        viewport: [f32; 2],
        grid: [f32; 2],
        bg: [f32; 4],
        dot: [f32; 4],
    ) -> CameraUni {
        CameraUni {
            center: [self.center.x, self.center.y],
            viewport,
            scale: self.zoom,
            ppp: self.ppp,
            grid,
            bg,
            dot,
        }
    }

    /// 世界坐标 → 物理像素（视口左上角为原点，向下为正）。
    pub fn world_to_phys(&self, p: Point, viewport: [f32; 2]) -> Point {
        let k = self.zoom * self.ppp;
        Point::new(
            (p.x - self.center.x) * k + viewport[0] * 0.5,
            (p.y - self.center.y) * k + viewport[1] * 0.5,
        )
    }

    /// 世界坐标 → 逻辑视口坐标（egui 的 points，左上角为原点）。
    /// 与 [`Self::screen_to_world`] 互逆；行内编辑器就靠它把节点盒摆到屏幕上。
    pub fn world_to_screen(&self, p: Point, viewport: [f32; 2]) -> Point {
        Point::new(
            (p.x - self.center.x) * self.zoom + viewport[0] * 0.5,
            (p.y - self.center.y) * self.zoom + viewport[1] * 0.5,
        )
    }

    /// 逻辑视口坐标（egui 的 points，左上角为原点）→ 世界坐标。
    /// ppp 在两边约掉，所以这里只认缩放；指针命中沿用同一套换算。
    pub fn screen_to_world(&self, p: Point, viewport: [f32; 2]) -> Point {
        Point::new(
            self.center.x + (p.x - viewport[0] * 0.5) / self.zoom,
            self.center.y + (p.y - viewport[1] * 0.5) / self.zoom,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_screen_inverts_screen_to_world() {
        let cam = Camera::new(Point::new(-320.0, 90.0), 0.35, 1.75);
        let viewport = [1024.0, 768.0];
        for p in [
            Point::new(-2000.0, -500.0),
            Point::new(-320.0, 90.0),
            Point::new(1500.0, 1200.0),
        ] {
            let s = cam.world_to_screen(p, viewport);
            let back = cam.screen_to_world(s, viewport);
            assert!((back.x - p.x).abs() < 1e-2 && (back.y - p.y).abs() < 1e-2);
        }
        // 视口中心永远是相机中心
        let c = cam.world_to_screen(cam.center, viewport);
        assert!((c.x - viewport[0] * 0.5).abs() < 1e-4);
        assert!((c.y - viewport[1] * 0.5).abs() < 1e-4);
    }

    #[test]
    fn screen_to_world_round_trips_through_physical_pixels() {
        let cam = Camera::new(Point::new(120.0, -40.0), 1.75, 1.5);
        let viewport = [900.0, 600.0];
        for p in [
            Point::new(0.0, 0.0),
            Point::new(450.0, 300.0),
            Point::new(899.0, 12.0),
        ] {
            let w = cam.screen_to_world(p, viewport);
            let back = cam.world_to_phys(w, [viewport[0] * cam.ppp, viewport[1] * cam.ppp]);
            // 物理像素再除回 ppp 就是逻辑坐标
            let back = Point::new(back.x / cam.ppp, back.y / cam.ppp);
            assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);
        }
    }
}
