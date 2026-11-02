// L1 曲线/直线实例：一个实例 = 一条胶囊段（圆头线段）。
//
// 贝塞尔在 CPU 侧按屏幕容差细分（`flatten`）后交到这里；片元算到线段的带符号距离，
// 所以线宽是「世界单位 = CSS ÷ 缩放」，屏幕上永远一样粗，端头永远是圆的。

struct Inst {
    a: vec2<f32>,
    b: vec2<f32>,
    // 布局与 `SegInst::meta` 一致；WGSL 里 `meta` 是保留字，这里叫 pen
    pen: vec4<f32>,
    color: vec4<f32>,
};

@group(1) @binding(0) var<storage, read> insts: array<Inst>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) w: vec2<f32>,
    @location(1) a: vec2<f32>,
    @location(2) ba: vec2<f32>,
    @location(3) len: f32,
    @location(4) hw: f32,
    @location(5) dash: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) px: f32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    // TriangleStrip 顺序：左下、右下、左上、右上
    let uv = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0));
    let ins = insts[ii];
    let px = pixel();
    let ba = ins.b - ins.a;
    let len = length(ba);
    let t = select(vec2(1.0, 0.0), ba / vec2(len, len), len > 1.0e-6);
    let n = vec2(-t.y, t.x);
    let hw = ins.pen.x * 0.5 + px;
    let hl = len * 0.5 + hw;
    let local = uv[vi] * vec2(hl, hw);

    var o: VsOut;
    o.w = ins.a + ba * 0.5 + t * local.x + n * local.y;
    o.a = ins.a;
    o.ba = ba;
    o.len = len;
    o.hw = ins.pen.x * 0.5;
    o.dash = ins.pen;
    o.color = ins.color;
    o.px = px;
    o.pos = world_to_clip(o.w);
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // 到线段中心的距离（世界单位），减去半宽即胶囊的带符号距离
    let h = clamp(dot(in.w - in.a, in.ba) / max(dot(in.ba, in.ba), 1.0e-12), 0.0, 1.0);
    let nearest = in.a + in.ba * h;
    let d = length(in.w - nearest) - in.hw;
    var c = cov(d, in.px);
    if (in.dash.y > 0.5) {
        let period = in.dash.z + in.dash.w;
        if (period > 0.0) {
            let s = dot(in.w - in.a, normalize(in.ba));
            if (fract(s / period) * period > in.dash.z) {
                c = 0.0;
            }
        }
    }
    let a = in.color.a * c;
    return vec4<f32>(in.color.rgb * a, a);
}
