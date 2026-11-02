// L0 背景点阵：一个全屏三角形，逐片元算「离最近的格点有多远」。
//
// 三个参数全部来自主题令牌 `Theme::canvas_dots()`（X21b）：
// - `uni.grid.x` 间距是世界单位（随缩放），
// - `uni.grid.y` 点半径是 CSS px（屏幕恒定，屏幕上看永远是同样大的点），
// - `uni.dot` 是点色（`a == 0` 即「这套主题不铺点阵」，合成器此时连这一层都不画）。
// 历史：间距/半径曾是 `instances::GRID_SPACING / GRID_DOT` 两个全局常量，主题换不了；
// 搬进主题后 SVG/PDF 才能和画布读同一份（否则「导出同源」只是口头约定）。
//
// 缩放小到格距不足 6 物理像素时整层不画——这是 M1c LOD 的第一个开关。
// 它只管画布：导出是矢量的，没有「看不清」这件事。

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[vi], 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let spacing = uni.grid.x;
    let pitch = spacing * uni.scale * uni.ppp;
    if (pitch < 6.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let k = max(uni.scale * uni.ppp, 1.0e-5);
    let world = uni.center + (pos.xy - uni.viewport * 0.5) / k;
    let cell = world / spacing;
    let off = cell - floor(cell) - vec2(0.5, 0.5);
    // 到最近格点的距离（世界单位）。以所在格子中心近似，误差小于格子对角的 3%。
    let d = length(off) * spacing - uni.grid.y / max(uni.scale, 1.0e-5);
    let a = uni.dot.a * cov(d, pixel());
    return vec4<f32>(uni.dot.rgb * a, a);
}
