//! 证据放大镜：把金样里的一块矩形裁出来并按整数倍放大，存成新 PNG。
//!
//! `cargo run -p mw-golden --example crop -- <in.png> <out.png> <x> <y> <w> <h> [scale]`
//!
//! 只按最近邻放大（不做插值）：证据图要的是「这个像素到底是不是那颗字」，
//! 插值会把 0.1px 的残留糊成一团灰，反而看不清。

use mw_golden::{read_png, write_png};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 6 {
        eprintln!("用法: crop <in.png> <out.png> <x> <y> <w> <h> [scale]");
        std::process::exit(2);
    }
    let num = |i: usize, default: f32| -> f32 {
        args.get(i)
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(default)
    };
    let (x, y, w, h) = (num(2, 0.0), num(3, 0.0), num(4, 100.0), num(5, 100.0));
    let scale = num(6, 1.0).max(1.0);

    let (src, iw, ih) = read_png(std::path::Path::new(&args[0])).expect("读入 PNG 失败");
    let (x, y) = (x as i32, y as i32);
    let (w, h) = (w as i32, h as i32);

    let ow = (w as f32 * scale) as u32;
    let oh = (h as f32 * scale) as u32;
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    for oy in 0..oh {
        for ox in 0..ow {
            let sx = x + (ox as f32 / scale) as i32;
            let sy = y + (oy as f32 / scale) as i32;
            let di = ((oy * ow + ox) * 4) as usize;
            // 越界留透明而不是黑：边界一眼能看出来，不会伪装成内容。
            if sx < 0 || sy < 0 || sx >= iw as i32 || sy >= ih as i32 {
                continue;
            }
            let si = ((sy as u32 * iw + sx as u32) * 4) as usize;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    write_png(std::path::Path::new(&args[1]), &out, ow, oh).expect("写出 PNG 失败");
    println!("{} → {} ({}x{} @{scale}x)", args[0], args[1], w, h);
}
