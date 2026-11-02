// L3 实心三角形：关系线箭头。三条边各算一次重心坐标×高，取最小的当覆盖度，
// 于是尖角也是抗锯齿的，不需要 MSAA。

struct Inst {
    p: array<vec2<f32>, 3>,
    pad: vec2<f32>,
    color: vec4<f32>,
};

@group(1) @binding(0) var<storage, read> insts: array<Inst>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) bary: vec3<f32>,
    @location(1) alt: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) px: f32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let ins = insts[ii];
    let a = ins.p[0u];
    let b = ins.p[1u];
    let c = ins.p[2u];
    // 2×面积；某顶点到对边的距离 = 2×面积 / 对边长
    let area2 = abs((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x));
    let alt = vec3<f32>(
        area2 / max(length(c - b), 1.0e-6),
        area2 / max(length(c - a), 1.0e-6),
        area2 / max(length(b - a), 1.0e-6),
    );
    let bary = array<vec3<f32>, 3>(
        vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0));
    var o: VsOut;
    o.bary = bary[vi];
    o.alt = alt;
    o.color = ins.color;
    o.px = pixel();
    o.pos = world_to_clip(ins.p[vi]);
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let w = in.bary * in.alt;
    let d = min(w.x, min(w.y, w.z));
    let a = in.color.a * clamp(0.5 + d / in.px, 0.0, 1.0);
    return vec4<f32>(in.color.rgb * a, a);
}
