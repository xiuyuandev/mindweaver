// L1/L2/L3 圆角矩形实例：一个实例 = 一个四边形，片元里用带符号距离场判内外。
//
// 为什么不用三角化：节点盒、标签胶囊、选中环、圆点、边界框全是「圆角矩形」这一个
// 族，SDF 一次算完填充+描边，顶点数恒为 4，1.5px 的描边在任何缩放下都不糊。

struct Inst {
    center: vec2<f32>,
    half: vec2<f32>,
    params: vec4<f32>,
    fill: vec4<f32>,
    stroke: vec4<f32>,
};

@group(1) @binding(0) var<storage, read> insts: array<Inst>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) half: vec2<f32>,
    @location(2) radius: f32,
    @location(3) sw: f32,
    @location(4) fill: vec4<f32>,
    @location(5) stroke: vec4<f32>,
    @location(6) px: f32,
    @location(7) dash: vec2<f32>,
};

fn sd_round(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2(r, r);
    return length(max(q, vec2(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

/// 椭圆的近似距离场（精确解要解四次方程，这里用业内常用的两步归一化近似）。
///
/// 误差集中在长短轴差别极大的地方（本项目的概念盒最宽 240、最矮 36，比例 6.7:1，
/// 此时边缘距离最大偏差约 0.4px），对 1.5px 描边的抗锯齿够用；中心处 `k2 == 0` 要兜底。
fn sd_ellipse(p: vec2<f32>, ab: vec2<f32>) -> f32 {
    let k1 = length(p / ab);
    let k2 = length(p / (ab * ab));
    if (k2 < 1.0e-6) {
        return -min(ab.x, ab.y);
    }
    return k1 * (k1 - 1.0) / k2;
}

/// 圆角矩形描边的周长参数，起点 = 上边左端点（与 SVG rounded-rect 的路径起点一致），
/// 顺时针。虚线相位沿整圈连续，因此四个圆角不会各自重置节奏。
fn perimeter_s(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    if (r <= 1.0e-5) {
        return 0.0;
    }
    let sx = max(half.x - r, 0.0);
    let sy = max(half.y - r, 0.0);
    let quarter = 1.5707963267948966 * r;
    let edge_x = 2.0 * sx;
    let edge_y = 2.0 * sy;
    let b1 = edge_x;
    let b2 = b1 + quarter;
    let b3 = b2 + edge_y;
    let b4 = b3 + quarter;
    let b5 = b4 + edge_x;
    let b6 = b5 + quarter;
    let b7 = b6 + edge_y;

    let ax = abs(p.x);
    let ay = abs(p.y);
    if (ax <= sx) {
        if (p.y < 0.0) {
            return p.x + sx;
        }
        return b4 + (sx - p.x);
    }
    if (ay <= sy) {
        if (p.x > 0.0) {
            return b2 + (p.y + sy);
        }
        return b6 + (sy - p.y);
    }
    let c = vec2(select(-sx, sx, p.x > 0.0), select(-sy, sy, p.y > 0.0));
    let a = atan2(p.y - c.y, p.x - c.x);
    if (p.x > 0.0 && p.y < 0.0) {
        return b1 + (a + 1.5707963267948966) * r;
    }
    if (p.x > 0.0) {
        return b3 + a * r;
    }
    if (p.y >= 0.0) {
        return b5 + (a - 1.5707963267948966) * r;
    }
    return b7 + (a + 3.141592653589793) * r;
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    // TriangleStrip 顺序：左下、右下、左上、右上
    let uv = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0));
    let ins = insts[ii];
    let px = pixel();
    let e = ins.params.y * 0.5 + px;
    var o: VsOut;
    o.half = ins.half;
    o.p = uv[vi] * (ins.half + vec2(e, e));
    o.radius = ins.params.x;
    o.sw = ins.params.y;
    o.fill = ins.fill;
    o.stroke = ins.stroke;
    o.px = px;
    o.dash = ins.params.zw;
    o.pos = world_to_clip(ins.center + o.p);
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // 圆角半径 **取负** = 这个盒子画成椭圆（Rust 侧 `instances::corner()` 的唯一出口，
    // `Shape::Ellipse` 没有圆角概念，用符号把「形状种类」压进既有的 4 个 float 里，
    // 不改实例布局——多一个 f32 会让 WGSL 的 stride 对齐从 68 变成 80，和 Rust 侧对不上）。
    let ell = in.radius < -0.5;
    let r = min(max(in.radius, 0.0), min(in.half.x, in.half.y));
    var d: f32;
    if (ell) {
        d = sd_ellipse(in.p, in.half);
    } else {
        d = sd_round(in.p, in.half, r);
    }
    let fc = in.fill.a * cov(d, in.px);
    var sc = in.stroke.a * cov(abs(d) - in.sw * 0.5, in.px);
    if (!ell && in.dash.x > 0.0 && in.dash.y > 0.0) {
        let period = in.dash.x + in.dash.y;
        let s = perimeter_s(in.p, in.half, r);
        if (fract(s / period) * period > in.dash.x) {
            sc = 0.0;
        }
    }
    // 描边压在填充上，两个覆盖率各自带 alpha，输出预乘
    let a = sc + fc * (1.0 - sc);
    let rgb = in.stroke.rgb * sc + in.fill.rgb * fc * (1.0 - sc);
    return vec4<f32>(rgb, a);
}
