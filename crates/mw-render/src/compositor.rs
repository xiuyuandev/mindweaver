//! 四层合成器：只碰 GPU 资源与绘制顺序，不含任何思维导图语义。
//!
//! 输入是 [`crate::instances::build_layers`] 产出的四层实例 + 相机，输出一张
//! `Rgba8Unorm` 纹理（离屏时回读成 [`Image]`）。接进壳层后这张纹理交给 egui 与面板
//! 一起合成，那步属于 M1b-3；这里先把「几何正确落到像素」这件事钉死。
//!
//! 两条已经定死、不要再商量的事：
//! - **混合发生在 sRGB 编码（gamma）空间**：目标格式用 `Rgba8Unorm` 而不是 `...Srgb`，
//!   片元一律输出**预乘** alpha，配 `BlendState{One, OneMinusSrcAlpha}`。这样同一套颜色
//!   令牌在 egui / Direct2D / 旧 SVG / 这里得到同一个灰度，换后端不会出现灰度漂移。
//! - **不用 MSAA**：矩形与线段走解析 SDF 抗锯齿，三角形用重心距离抗锯齿。1.5px 的铬件
//!   再叠一层 MSAA 会被二次模糊，分析法更锐利，还省掉 resolve 纹理。

use std::borrow::Cow;
use std::sync::mpsc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUni};
use crate::glyph::{AtlasUpdate, ATLAS_SIZE};
use crate::instances::{rgba, Frame, LayerGeom, LAYERS};

const LATTICE: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/lattice.wgsl")
);
const RECT: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/rect.wgsl")
);
const SEG: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/seg.wgsl")
);
const TRI: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/tri.wgsl")
);
const TEXT: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/text.wgsl")
);

/// 目标纹理格式。刻意不用 srgb 变体，见模块头。
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// `copy_texture_to_buffer` 要求行字节数对齐到 256。
const ROW_ALIGN: usize = 256;
const MIN_SLOT: usize = 256;

#[derive(Debug)]
pub enum RenderError {
    NoAdapter(String),
    Device(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter(s) => write!(f, "没有可用的 GPU 适配器：{s}"),
            Self::Device(s) => write!(f, "GPU 设备创建失败：{s}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// 回读出来的一张图。行按 256 字节对齐，所以 `stride` 通常大于 `width * 4`。
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub data: Vec<u8>,
}

impl Image {
    /// 去掉行填充，得到紧凑 RGBA（预乘）。
    pub fn rgba(&self) -> Vec<u8> {
        let row = self.width as usize * 4;
        let mut out = Vec::with_capacity(row * self.height as usize);
        for y in 0..self.height as usize {
            out.extend_from_slice(&self.data[y * self.stride..y * self.stride + row]);
        }
        out
    }

    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = y as usize * self.stride + x as usize * 4;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }
}

/// 一块逐帧重写、容量只增不减的实例缓冲。
struct Slot {
    buf: wgpu::Buffer,
    cap: usize,
}

impl Slot {
    fn new(device: &wgpu::Device, label: &str) -> Self {
        Self {
            buf: storage(device, label, MIN_SLOT),
            cap: MIN_SLOT,
        }
    }

    /// 数组绑定不允许留下「半个元素」的余量，所以容量按 16 向上取整（三种实例的步长都是 16 的倍数）。
    fn sync<T: Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        items: &[T],
    ) {
        let need = items.len() * std::mem::size_of::<T>();
        if need > self.cap {
            let cap = need.div_ceil(16) * 16;
            self.buf = storage(device, label, cap);
            self.cap = cap;
        }
        if need > 0 {
            queue.write_buffer(&self.buf, 0, bytemuck::cast_slice(items));
        }
    }
}

fn storage(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

struct Target {
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

pub struct Compositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
    uni: wgpu::Buffer,
    bg_cam: wgpu::BindGroup,
    bgl_inst: wgpu::BindGroupLayout,
    bgl_text: wgpu::BindGroupLayout,
    lattice: wgpu::RenderPipeline,
    rect: wgpu::RenderPipeline,
    seg: wgpu::RenderPipeline,
    tri: wgpu::RenderPipeline,
    text: wgpu::RenderPipeline,
    atlas_tex: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    atlas_sampler: wgpu::Sampler,
    atlas_gen: u64,
    /// 上次图集上传因代次断档而失去同步；调用方看到后应重置图集。
    atlas_desync: bool,
    slots: [[Slot; 4]; LAYERS],
    target: Option<Target>,
    /// 省渲染：省掉纯装饰的点阵背景（docs/08 §6），内容层一律不动。
    low_render: bool,
    /// 透明底（只给离屏导出用）：清屏与点阵底退成透明，**主题一动不动**。
    ///
    /// 关键是「不动主题」：`theme.ui.canvas_bg` 同时兼着挖空色——标签胶囊里的字、关系线
    /// 标签的底、折叠圆点的底都拿它当橡皮擦（`instances.rs` / `text.rs`）。早先的写法是导出前
    /// 把这条令牌改成 `TRANSPARENT`，于是透明底 PNG 里这些前景一起消失了：标签胶囊成了空壳
    /// （docs/14 教训 45）。清屏走独立开关，令牌留原值，两边才各得其所。
    transparent: bool,
}

impl Compositor {
    /// 同步打开实例（内部 `block_on`）。壳层接进来后可以改异步，离屏 harness 先要简单。
    pub fn new() -> Result<Self, RenderError> {
        futures_executor::block_on(open())
    }

    /// 复用壳层 / egui 的 device 与 queue（M1b-3）。管线与缓冲仍归合成器所有，
    /// 只有 GPU 连接是共享的——这样画布纹理可以直接注册给 egui 采样。
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue, info: wgpu::AdapterInfo) -> Self {
        let uni = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera"),
            contents: bytemuck::bytes_of(&CameraUni::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bgl_cam =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera-bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<CameraUni>() as u64
                        ),
                    },
                    count: None,
                }],
            });
        let bg_cam = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bg"),
            layout: &bgl_cam,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uni.as_entire_binding(),
            }],
        });
        let bgl_inst = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("insts-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bgl_text = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph-atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let layout_geom = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("geom-layout"),
            bind_group_layouts: &[Some(&bgl_cam), Some(&bgl_inst)],
            immediate_size: 0,
        });
        let layout_text = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text-layout"),
            bind_group_layouts: &[Some(&bgl_cam), Some(&bgl_text)],
            immediate_size: 0,
        });
        let layout_lattice = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lattice-layout"),
            bind_group_layouts: &[Some(&bgl_cam)],
            immediate_size: 0,
        });

        let m_lat = module(&device, "lattice", LATTICE);
        let m_rect = module(&device, "rect", RECT);
        let m_seg = module(&device, "seg", SEG);
        let m_tri = module(&device, "tri", TRI);
        let m_text = module(&device, "text", TEXT);

        let lattice = pipeline(&device, "lattice", &m_lat, &layout_lattice, 3);
        let rect = pipeline(&device, "rect", &m_rect, &layout_geom, 4);
        let seg = pipeline(&device, "seg", &m_seg, &layout_geom, 4);
        let tri = pipeline(&device, "tri", &m_tri, &layout_geom, 3);
        let text = pipeline(&device, "text", &m_text, &layout_text, 4);

        let slots = std::array::from_fn(|i| {
            [
                Slot::new(&device, &format!("L{i}/rect")),
                Slot::new(&device, &format!("L{i}/seg")),
                Slot::new(&device, &format!("L{i}/tri")),
                Slot::new(&device, &format!("L{i}/text")),
            ]
        });

        Self {
            device,
            queue,
            info,
            uni,
            bg_cam,
            bgl_inst,
            bgl_text,
            lattice,
            rect,
            seg,
            tri,
            text,
            atlas_tex,
            atlas_view,
            atlas_sampler,
            atlas_gen: 0,
            atlas_desync: false,
            slots,
            target: None,
            low_render: false,
            transparent: false,
        }
    }

    /// 省渲染开关：只关掉点阵背景，节点/连线/文字与交互一律照旧。
    pub fn set_low_render(&mut self, on: bool) {
        self.low_render = on;
    }

    /// 透明底开关：清屏色与点阵底一起退成透明，画什么、什么颜色都照旧。
    /// 上屏路径不许打开它——画布底是主题令牌的一部分，见字段注释。
    pub fn set_transparent(&mut self, on: bool) {
        self.transparent = on;
    }

    /// 这一帧的清屏色。透明底时彻底清零：底色留着 RGB 会让半透明的边缘（抗锯齿）
    /// 与那块残色混出一道浅边。
    fn clear_color(&self, f: &Frame) -> [f32; 4] {
        if self.transparent {
            [0.0; 4]
        } else {
            rgba(f.theme.ui.canvas_bg)
        }
    }

    /// 等 GPU 把已提交的命令执行完。**不上屏**——帧预算探针需要它，
    /// 画布路径会故意不等，让 CPU 与 GPU 重叠。
    pub fn wait_idle(&self) {
        if let Err(e) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        }) {
            panic!("等待 GPU 空闲失败：{e:?}");
        }
    }

    /// 底层 GPU 连接（壳层注册纹理时需要同一个 device）。
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// `后端 / 显卡 / 驱动`，golden 基线的元数据就记这一行。
    pub fn adapter_summary(&self) -> String {
        let i = &self.info;
        format!("{:?} · {} · {}", i.backend, i.device, i.driver_info)
    }

    /// 画一帧并回读。`viewport` 是**物理像素**。
    pub fn render(&mut self, f: &Frame, layers: &[LayerGeom; LAYERS], viewport: [u32; 2]) -> Image {
        assert!(viewport[0] > 0 && viewport[1] > 0, "视口尺寸必须为正");
        let _recreated = self.prepare(f, layers, viewport);
        let readback = self.padded_buffer(viewport);
        let encoder = self.encode(f, layers, viewport, Some(&readback));
        let sub = self.queue.submit(std::iter::once(encoder.finish()));

        let data = self.wait_for_readback(readback, viewport, sub);
        Image {
            width: viewport[0],
            height: viewport[1],
            stride: padded_row(viewport[0]),
            data,
        }
    }

    /// 画一帧到内部纹理且**不读回**（M1b-3 上屏路径）。返回纹理是否因尺寸变化重建：
    /// 是则壳层要重新把 `target_view()` 注册给 egui。
    pub fn render_texture(
        &mut self,
        f: &Frame,
        layers: &[LayerGeom; LAYERS],
        viewport: [u32; 2],
    ) -> bool {
        assert!(viewport[0] > 0 && viewport[1] > 0, "视口尺寸必须为正");
        let recreated = self.prepare(f, layers, viewport);
        let encoder = self.encode(f, layers, viewport, None);
        self.queue.submit(std::iter::once(encoder.finish()));
        recreated
    }

    /// 当前画布纹理视图。直到第一次 `render_texture` / `render` 之前为 `None`。
    pub fn target_view(&self) -> Option<&wgpu::TextureView> {
        self.target.as_ref().map(|t| &t.view)
    }

    fn prepare(&mut self, f: &Frame, layers: &[LayerGeom; LAYERS], viewport: [u32; 2]) -> bool {
        let px = [viewport[0] as f32, viewport[1] as f32];
        let (grid, dot) = dot_layer(f.theme, self.transparent, self.low_render);
        let uni = camera_uni(&f.camera, px, self.clear_color(f), grid, dot);
        self.queue
            .write_buffer(&self.uni, 0, bytemuck::bytes_of(&uni));

        for (i, layer) in layers.iter().enumerate() {
            let s = &mut self.slots[i];
            let tag = format!("L{i}");
            s[0].sync(&self.device, &self.queue, &tag, &layer.rects);
            s[1].sync(&self.device, &self.queue, &tag, &layer.segs);
            s[2].sync(&self.device, &self.queue, &tag, &layer.tris);
            s[3].sync(&self.device, &self.queue, &tag, &layer.texts);
        }

        self.ensure_target(viewport)
    }

    fn encode(
        &self,
        f: &Frame,
        layers: &[LayerGeom; LAYERS],
        viewport: [u32; 2],
        readback: Option<&wgpu::Buffer>,
    ) -> wgpu::CommandEncoder {
        let target = self.target.as_ref().unwrap();
        let clear = self.clear_color(f);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("canvas"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("canvas"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            // wgpu 30 把分量原样交给后端（`cv.r as f32` → Vulkan 的 float clear
                            // 值），所以对 unorm 目标必须是 0..1；填 0..255 会被钳成纯白。
                            r: clear[0] as f64,
                            g: clear[1] as f64,
                            b: clear[2] as f64,
                            // alpha 走 [`Compositor::transparent`]：上屏 / 不透明导出用主题底色
                            // （不透明，逐像素与旧版一致），透明导出清成 0（docs/13 §4.23）。
                            a: clear[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bg_cam, &[]);
            // L0 只在真的有点阵时画（`dot_layer` 已把省渲染 / 透明底 / 无点阵主题都并成
            // `dot` 全 0）；这时整层透明度是 0，跳过它与画一遍结果相同，少一个全屏三角形。
            if !self.low_render && !self.transparent && f.theme.canvas_dots().is_some() {
                pass.set_pipeline(&self.lattice);
                pass.draw(0..3, 0..1);
            }
            // L1→L3：先矩形、再线段、再三角形，最后文字——同层内「面压线、字压面」
            for (i, layer) in layers.iter().enumerate().skip(1) {
                let s = &self.slots[i];
                self.draw(&mut pass, &self.rect, 4, &s[0].buf, layer.rects.len());
                self.draw(&mut pass, &self.seg, 4, &s[1].buf, layer.segs.len());
                self.draw(&mut pass, &self.tri, 3, &s[2].buf, layer.tris.len());
                self.draw_text(&mut pass, &s[3].buf, layer.texts.len());
            }
        }
        if let Some(buf) = readback {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target.tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_row(viewport[0]) as u32),
                        rows_per_image: Some(viewport[1]),
                    },
                },
                wgpu::Extent3d {
                    width: viewport[0],
                    height: viewport[1],
                    depth_or_array_layers: 1,
                },
            );
        }
        encoder
    }

    fn draw(
        &self,
        pass: &mut wgpu::RenderPass,
        pipeline: &wgpu::RenderPipeline,
        verts: u32,
        buf: &wgpu::Buffer,
        count: usize,
    ) {
        if count == 0 {
            return;
        }
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("insts"),
            layout: &self.bgl_inst,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &bg, &[]);
        pass.draw(0..verts, 0..count as u32);
    }

    /// 文本管线额外绑定图集与采样器，所以不能复用 [`Self::draw`] 的单绑定组。
    fn draw_text(&self, pass: &mut wgpu::RenderPass, buf: &wgpu::Buffer, count: usize) {
        if count == 0 {
            return;
        }
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text-atlas"),
            layout: &self.bgl_text,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.atlas_sampler),
                },
            ],
        });
        pass.set_pipeline(&self.text);
        pass.set_bind_group(1, &bg, &[]);
        pass.draw(0..4, 0..count as u32);
    }

    /// 把 `TextEngine` 的图集增量传到 GPU：每块像素自带紧排行跨度，直接写对应矩形。
    /// 代次断档（有更新被跳过）记一次「需要重置图集」，由调用方下一帧重排；
    /// 图集重排本身不用清 GPU 纹理——旧 UV 已失效，残留像素不会再被采到。
    pub fn upload_atlas(&mut self, update: &AtlasUpdate) {
        if update.generation != self.atlas_gen.wrapping_add(1) {
            self.atlas_desync = true;
        }
        for rect in &update.rects {
            if rect.w == 0 || rect.h == 0 {
                continue;
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: rect.x,
                        y: rect.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &rect.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(rect.w),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: rect.w,
                    height: rect.h,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.atlas_gen = update.generation;
    }

    /// 上一次上传发现图集代次断档。调用方应重置图集，下一帧会重排所有字形。
    pub fn take_atlas_desync(&mut self) -> bool {
        std::mem::take(&mut self.atlas_desync)
    }

    fn ensure_target(&mut self, viewport: [u32; 2]) -> bool {
        let same = self
            .target
            .as_ref()
            .is_some_and(|t| t.width == viewport[0] && t.height == viewport[1]);
        if same {
            return false;
        }
        {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("canvas-target"),
                size: wgpu::Extent3d {
                    width: viewport[0],
                    height: viewport[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            self.target = Some(Target {
                tex,
                view,
                width: viewport[0],
                height: viewport[1],
            });
        }
        true
    }

    fn padded_buffer(&self, viewport: [u32; 2]) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded_row(viewport[0]) * viewport[1] as usize) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn wait_for_readback(
        &self,
        buf: wgpu::Buffer,
        viewport: [u32; 2],
        sub: wgpu::SubmissionIndex,
    ) -> Vec<u8> {
        let bytes = padded_row(viewport[0]) * viewport[1] as usize;
        let (tx, rx) = mpsc::channel();
        buf.map_async(wgpu::MapMode::Read, 0..bytes as u64, move |r| {
            let _ = tx.send(r);
        });
        if let Err(e) = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(sub),
            timeout: Some(std::time::Duration::from_secs(10)),
        }) {
            panic!("等待 GPU 提交完成失败：{e:?}");
        }
        rx.recv()
            .expect("map_async 回调没有执行")
            .unwrap_or_else(|e| panic!("回读缓冲映射失败：{e:?}"));
        let range = buf
            .get_mapped_range(0..bytes as u64)
            .unwrap_or_else(|e| panic!("回读范围取用失败：{e:?}"));
        let out = range.to_vec();
        drop(range);
        buf.unmap();
        out
    }
}

/// 点阵那两件 uniform：`(grid = [间距·世界单位, 半径·CSS px], dot = 颜色)`。
///
/// **令牌只有一个出口**：[`mw_core::theme::Theme::canvas_dots`]。两个合成器开关叠在它之上——
/// 透明底不铺、省渲染不铺——但它们**不改主题**（`canvas_bg` 兼着挖空色，见 `set_transparent`
/// 的注释）。返回的 `dot` 全 0 即「这帧不画点阵」，`encode` 会直接跳过 L0。
fn dot_layer(
    theme: &mw_core::theme::Theme,
    transparent: bool,
    low_render: bool,
) -> ([f32; 2], [f32; 4]) {
    let dots = if transparent || low_render {
        None
    } else {
        theme.canvas_dots()
    };
    match dots {
        Some(d) => ([d.spacing, d.radius], rgba(d.color)),
        None => ([0.0, 0.0], [0.0; 4]),
    }
}

fn camera_uni(
    cam: &Camera,
    viewport: [f32; 2],
    bg: [f32; 4],
    grid: [f32; 2],
    dot: [f32; 4],
) -> CameraUni {
    // 间距是世界单位（随缩放），点半径是 CSS px（屏幕恒定）——着色器各自换算
    cam.uni(viewport, grid, bg, dot)
}

fn padded_row(width: u32) -> usize {
    (width as usize * 4).next_multiple_of(ROW_ALIGN)
}

async fn open() -> Result<Compositor, RenderError> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    // DX12 后端在 wgpu 里还嫩，Vulkan 打底、GL 兜底（虚拟机/远程桌面只有 WGL）
    desc.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
    let instance = wgpu::Instance::new(desc);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .map_err(|e| RenderError::NoAdapter(format!("{e:?}")))?;
    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("mw-render"),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .map_err(|e| RenderError::Device(format!("{e:?}")))?;

    Ok(Compositor::from_device(device, queue, info))
}

fn module(device: &wgpu::Device, label: &str, src: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(src)),
    })
}

/// 四个几何族共用一套管线模板：只有拓扑（三角形列表 / 四边形条带）和着色器不同。
fn pipeline(
    device: &wgpu::Device,
    label: &str,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    verts: u32,
) -> wgpu::RenderPipeline {
    let blend = Some(wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: if verts == 4 {
                wgpu::PrimitiveTopology::TriangleStrip
            } else {
                wgpu::PrimitiveTopology::TriangleList
            },
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::theme::{ThemeUi, DOT_RADIUS, DOT_SPACING};

    /// AC-5（X21b）：点阵 uniform 只由主题令牌决定；两个合成器开关叠在上面，
    /// 且**不改主题**（透明底导出之后同一份主题还能照常上屏）。
    #[test]
    fn dot_layer_reads_the_theme_tokens_and_respects_the_compositor_switches() {
        // ink 是四套里唯一铺点阵的主题；令牌值必须原样进 uniform。
        let ink = mw_core::theme::get_theme("ink");
        let (grid, dot) = dot_layer(&ink, false, false);
        assert_eq!(grid, [DOT_SPACING, DOT_RADIUS], "间距/半径必须来自主题令牌");
        assert_eq!(
            dot,
            rgba(ink.ui.dot),
            "uniform 里的点色就是令牌里的点色（预乘与 alpha 由着色器做）"
        );
        assert!(dot[3] > 0.0, "墨夜应该铺点阵");

        // 默认主题（亮色纸底）没有点阵：uniform 全 0，`encode` 连那一层都不画。
        let snow = mw_core::theme::get_theme("snow");
        assert_eq!(dot_layer(&snow, false, false), ([0.0, 0.0], [0.0; 4]));

        // 透明底：不画，但主题一动不动（`canvas_bg` 兼着挖空色，改了会擦掉标签胶囊的字）
        let before = ink.clone();
        assert_eq!(dot_layer(&ink, true, false), ([0.0, 0.0], [0.0; 4]));
        assert_eq!(ink, before, "合成器开关不许改主题");

        // 省渲染：只关点阵
        assert_eq!(dot_layer(&ink, false, true), ([0.0, 0.0], [0.0; 4]));

        // 概念图主题（纯纸）本来就没有点阵
        let cm = mw_core::theme::concept_theme("cm-paper");
        assert_eq!(dot_layer(&cm, false, false).1, [0.0; 4]);

        // 演示模式靠把 `ui.dot` 写成透明来关点阵——这条语义在令牌搬家后必须还在
        let demo = mw_core::theme::Theme {
            ui: ThemeUi {
                dot: mw_core::theme::Rgba::TRANSPARENT,
                ..ink.ui
            },
            ..ink
        };
        assert_eq!(dot_layer(&demo, false, false).1, [0.0; 4]);
    }
}
