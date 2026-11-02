//! golden image 比对基建（docs/13 §7）。
//!
//! 从 `mw-shell::golden` 拆出来：M0 时只有壳层快照要比对，M1b 起画布纹理也要，
//! 于是「容差语义、基线读写、差异热力图」这三件事必须只有一份定义——否则画布
//! 那边放宽了容差、壳层那边没有，同一套验收会出现两种口径。
//!
//! 壳层特有的部分（`egui::Context::request_screenshot` 驱动、命令行解析、退出码）
//! 仍留在 `mw-shell`。

use std::fs::File;
use std::io::Write;
use std::path::Path;

/// 读一张基线，返回紧凑 RGBA（预乘，与 `image` 解码一致）。
pub fn read_png(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    if !path.exists() {
        return Err(format!("基线不存在：{}", path.display()));
    }
    let img = image::open(path).map_err(|e| format!("基线解码失败 {}：{e}", path.display()))?;
    let (w, h) = (img.width(), img.height());
    Ok((img.to_rgba8().into_raw(), w, h))
}

/// 把紧凑 RGBA 落成 PNG，父目录按需创建。
pub fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    image::write_buffer_with_format(
        &mut file,
        rgba,
        w,
        h,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())
}

/// 一次逐像素比对的结果。默认容差 0：驱动或 wgpu 版本变动本该被看见，而不是被容差吃掉。
pub struct Drift {
    /// 超容差的像素数
    pub bad: usize,
    pub total: usize,
    /// 单通道最大偏差
    pub max: u8,
    pub tol: u8,
    /// 超容差像素的包围盒 (x0, y0, x1, y1)
    pub box_: Option<(u32, u32, u32, u32)>,
}

impl Drift {
    pub fn measure(a: &[u8], g: &[u8], w: u32, h: u32, tol: u8) -> Self {
        let mut bad = 0usize;
        let mut max = 0u8;
        let mut box_: Option<(u32, u32, u32, u32)> = None;
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let worst = (0..4).map(|k| a[i + k].abs_diff(g[i + k])).max().unwrap();
                if worst > tol {
                    bad += 1;
                    max = max.max(worst);
                    box_ = Some(match box_ {
                        None => (x, y, x, y),
                        Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                    });
                }
            }
        }
        Self {
            bad,
            total: (w * h) as usize,
            max,
            tol,
            box_,
        }
    }

    /// 差异图：实拍像素压到白底（Mica 那片 alpha=0 否则看起来是纯黑），超容差的点涂纯红。
    /// 于是一眼能分清「字形变了」和「整块位移」。
    pub fn heatmap(&self, a: &[u8], g: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(a.len());
        for (p, q) in a.chunks(4).zip(g.chunks(4)) {
            if (0..4).any(|k| p[k].abs_diff(q[k]) > self.tol) {
                out.extend_from_slice(&[0xE8, 0x1A, 0x1A, 0xFF]);
            } else {
                // 预乘 → 白底：白底分量恰好是 (255 - alpha)，一次加法即可。
                for c in &p[..3] {
                    out.push(c.saturating_add(255 - p[3]));
                }
                out.push(0xFF);
            }
        }
        out
    }
}

impl std::fmt::Display for Drift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pct = 100.0 * self.bad as f64 / self.total.max(1) as f64;
        match self.box_ {
            Some((x0, y0, x1, y1)) => write!(
                f,
                "{}/{} 像素超容差（{pct:.3}%），最大差 {}，包围盒 x[{x0}..{x1}] y[{y0}..{y1}]",
                self.bad, self.total, self.max
            ),
            None => write!(f, "无差异"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一张 w×h 的 RGBA 缓冲，除 (ox,oy) 外全部为 `base`。
    fn plane(w: u32, h: u32, base: [u8; 4], spot: Option<(u32, u32, [u8; 4])>) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                match spot {
                    Some((sx, sy, px)) if sx == x && sy == y => v.extend_from_slice(&px),
                    _ => v.extend_from_slice(&base),
                }
            }
        }
        v
    }

    #[test]
    fn identical_buffers_report_no_drift() {
        let a = plane(8, 8, [10, 20, 30, 255], None);
        let d = Drift::measure(&a.clone(), &a, 8, 8, 0);
        assert_eq!((d.bad, d.max, d.box_), (0, 0, None));
    }

    #[test]
    fn a_one_unit_change_is_caught_at_zero_tolerance() {
        let g = plane(8, 8, [10, 20, 30, 255], None);
        let a = plane(8, 8, [10, 20, 30, 255], Some((3, 5, [11, 20, 30, 255])));
        let d = Drift::measure(&a, &g, 8, 8, 0);
        assert_eq!((d.bad, d.max), (1, 1));
        assert_eq!(d.box_, Some((3, 5, 3, 5)));
    }

    #[test]
    fn tolerance_absorbs_sub_threshold_noise() {
        let g = plane(8, 8, [10, 20, 30, 255], None);
        let a = plane(8, 8, [10, 20, 30, 255], Some((3, 5, [11, 20, 30, 255])));
        assert_eq!(Drift::measure(&a, &g, 8, 8, 1).bad, 0);
    }

    #[test]
    fn bounding_box_spans_every_offending_pixel() {
        let g = plane(16, 16, [0, 0, 0, 255], None);
        let mut a = plane(16, 16, [0, 0, 0, 255], Some((2, 3, [9, 0, 0, 255])));
        for p in [(14u32, 15u32), (7, 1)] {
            let i = ((p.1 * 16 + p.0) * 4) as usize;
            a[i] = 9;
        }
        assert_eq!(Drift::measure(&a, &g, 16, 16, 0).box_, Some((2, 1, 14, 15)));
    }

    #[test]
    fn heatmap_composites_over_white_and_reddens_only_the_diffs() {
        let g = plane(4, 4, [0, 0, 0, 0], None);
        let a = plane(4, 4, [0, 0, 0, 0], Some((1, 1, [0, 0, 0, 128])));
        let map = Drift::measure(&a, &g, 4, 4, 0).heatmap(&a, &g);

        // 超容差 → 纯红标记
        let i = 4 * (1 * 4 + 1);
        assert_eq!(&map[i..i + 4], &[0xE8, 0x1A, 0x1A, 0xFF]);
        // 预乘 + alpha=0 的空洞 → 压到白底后是白，而不是黑
        assert_eq!(&map[0..4], &[255, 255, 255, 255]);
    }
}
