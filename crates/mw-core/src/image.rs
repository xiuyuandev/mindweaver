//! 图片尺寸嗅探与节点图片盒计算。不依赖解码库：只读文件头拿宽高，
//! 坏数据返回 `None`，调用方回退到 16:9 占位。

/// 从 PNG / JPEG / GIF 头部读 `(宽, 高)`；未知格式返回 `None`。
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() >= 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return (w > 0 && h > 0).then_some((w, h));
    }
    if bytes.len() >= 10 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        return (w > 0 && h > 0).then_some((w, h));
    }
    jpeg_dimensions(bytes)
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        // SOF0..SOF3 / SOF5..SOF7 / SOF9..SOF11 / SOF13..SOF15
        if (0xC0..=0xC3).contains(&marker)
            || (0xC5..=0xC7).contains(&marker)
            || (0xC9..=0xCB).contains(&marker)
            || (0xCD..=0xCF).contains(&marker)
        {
            let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            return (w > 0 && h > 0).then_some((w, h));
        }
        if marker == 0xD8 || marker == 0xD9 {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 {
            return None;
        }
        i += 2 + len;
    }
    None
}

pub fn ratio(bytes: &[u8]) -> Option<f32> {
    dimensions(bytes).map(|(w, h)| w as f32 / h as f32)
}

/// RGBA → PNG（8 位、无滤波、zlib stored 块）。剪贴板图片没有现成 PNG 字节时用它。
pub fn encode_rgba_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let w = w.max(1);
    let h = h.max(1);
    let mut raw = Vec::with_capacity((w as usize * 4 + 1) * h as usize);
    for y in 0..h as usize {
        raw.push(0);
        let start = y * w as usize * 4;
        let end = (start + w as usize * 4).min(rgba.len());
        raw.extend_from_slice(&rgba[start..end]);
        raw.resize((y + 1) * (w as usize * 4 + 1), 255);
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = Crc32::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.finish().to_be_bytes());
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut rest = data;
    loop {
        let n = rest.len().min(65535);
        let last = n == rest.len();
        out.push(if last { 0x01 } else { 0x00 });
        out.extend_from_slice(&(n as u16).to_le_bytes());
        out.extend_from_slice(&(!(n as u16)).to_le_bytes());
        out.extend_from_slice(&rest[..n]);
        rest = &rest[n..];
        if last {
            break;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

struct Crc32(u32);
impl Crc32 {
    fn new() -> Self {
        Self(0xFFFF_FFFF)
    }
    fn update(&mut self, data: &[u8]) {
        for &b in data {
            self.0 ^= b as u32;
            for _ in 0..8 {
                self.0 = if self.0 & 1 != 0 {
                    (self.0 >> 1) ^ 0xEDB8_8320
                } else {
                    self.0 >> 1
                };
            }
        }
    }
    fn finish(self) -> u32 {
        self.0 ^ 0xFFFF_FFFF
    }
}

/// 图片盒：宽度吃满可用宽，高按比例算并夹在 `[min_h, max_h]`，最后加内距。
/// 返回 `(显示宽, 显示高, 含内距的占位高)`。
pub fn image_box(
    ratio: f32,
    available_w: f32,
    pad: f32,
    min_h: f32,
    max_h: f32,
) -> (f32, f32, f32) {
    let ratio = if ratio.is_finite() && ratio > 0.05 {
        ratio
    } else {
        16.0 / 9.0
    };
    let inner_w = (available_w - pad * 2.0).max(32.0);
    let h = (inner_w / ratio).clamp(min_h, max_h);
    (inner_w, h, h + pad * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_png_round_trips_through_the_sniffer() {
        let rgba = vec![255u8; 4 * 4 * 3];
        let png = encode_rgba_png(4, 3, &rgba);
        assert_eq!(dimensions(&png), Some((4, 3)));
        assert_eq!(&png[1..4], b"PNG");
    }

    #[test]
    fn png_and_gif_headers_are_read() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        png.extend_from_slice(&800u32.to_be_bytes());
        png.extend_from_slice(&600u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        assert_eq!(dimensions(&png), Some((800, 600)));
        let gif = b"GIF89a\x40\x01\xf0\x00";
        assert_eq!(dimensions(gif), Some((320, 240)));
    }

    #[test]
    fn image_box_keeps_ratio_and_padding() {
        let (w, h, total) = image_box(16.0 / 9.0, 200.0, 8.0, 56.0, 180.0);
        assert_eq!(w, 184.0);
        assert!((h - 103.5).abs() < 0.1, "184 / (16/9) ≈ 103.5");
        assert!((total - 119.5).abs() < 0.1);
        // 超宽图被最小高托住，超窄图被最大高夹住。
        let (_, tall, _) = image_box(0.2, 200.0, 8.0, 56.0, 180.0);
        assert_eq!(tall, 180.0);
        let (_, wide, _) = image_box(10.0, 200.0, 8.0, 56.0, 180.0);
        assert_eq!(wide, 56.0);
        // 坏比例回退 16:9。
        let (_, fallback, _) = image_box(f32::NAN, 200.0, 8.0, 56.0, 180.0);
        assert!((fallback - 103.5).abs() < 0.1);
    }
}
