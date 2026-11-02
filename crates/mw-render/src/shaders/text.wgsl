// L2/L3 文本实例：一个实例 = 一个字形四边形。
//
// 图集是 R8 灰度覆盖率；采样后乘文字色。输出预乘 alpha，与其余几何族同一混合约定。
// 采样用线性过滤：缩放档位与光栅化档位不完全相等时靠它过渡，字不会跳变。

struct Inst {
    pos: vec2<f32>,
    size: vec2<f32>,
    uv: vec4<f32>,
    color: vec4<f32>,
};

@group(1) @binding(0) var<storage, read> insts: array<Inst>;
@group(1) @binding(1) var atlas: texture_2d<f32>;
@group(1) @binding(2) var atlas_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    // TriangleStrip：左上、右上、左下、右下
    let corner = array<vec2<f32>, 4>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0));
    let ins = insts[ii];
    let local = corner[vi];
    var o: VsOut;
    o.pos = world_to_clip(ins.pos + local * ins.size);
    o.uv = mix(ins.uv.xy, ins.uv.zw, local);
    o.color = ins.color;
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let cov = textureSample(atlas, atlas_sampler, in.uv).r;
    let a = cov * in.color.a;
    return vec4<f32>(in.color.rgb * a, a);
}
