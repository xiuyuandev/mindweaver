// 四层共用的相机 uniform。字段顺序必须与 `mw_render::camera::CameraUni` 逐字节一致。
struct Uni {
    center: vec2<f32>,
    viewport: vec2<f32>,
    scale: f32,
    ppp: f32,
    grid: vec2<f32>,
    bg: vec4<f32>,
    dot: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uni: Uni;

/// 世界坐标 → 裁剪空间。物理像素 y 轴向下，裁剪空间 y 轴向上，所以第二项取反。
fn world_to_clip(world: vec2<f32>) -> vec4<f32> {
    let k = max(uni.scale * uni.ppp, 1.0e-5);
    let phys = (world - uni.center) * k + uni.viewport * 0.5;
    return vec4<f32>(phys.x / uni.viewport.x * 2.0 - 1.0, 1.0 - phys.y / uni.viewport.y * 2.0, 0.0, 1.0);
}

/// 一个物理像素在世界单位下的边长。SDF 的抗锯齿带宽靠它换算，
/// 于是描边粗细、羽化宽度都只跟屏幕像素有关，跟缩放无关。
fn pixel() -> f32 {
    return 1.0 / max(uni.scale * uni.ppp, 1.0e-5);
}

/// 解析抗锯齿：d 为带符号距离（负=在内），返回覆盖率 0..1。
fn cov(d: f32, px: f32) -> f32 {
    return clamp(0.5 - d / px, 0.0, 1.0);
}
