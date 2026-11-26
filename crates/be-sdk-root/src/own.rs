//! A surface of the root's own, which it draws on the host's device as an app draws its screens
//! (`be3:gpu/host`'s `surface`), and names in its layer list as `Surface::Root`.

use std::mem::ManuallyDrop;

use be_lib_geometry::{Pos2, Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerId, LayerInput, Surface};
use be_wasm_gpu_abi::guest::{host, webgpu as gpu};
use be_wasm_gpu_abi::webgpu::webgpu as web;

/// One of the root's own surfaces: textures on the host's device the root draws into and
/// presents, which its host draws where the layer naming it (`OwnSurface::layer`) places it.
/// The pixels never leave the GPU. The root draws below, between and above the screens it
/// embeds with as many as it likes, each with an id of its own.
///
/// The host serves it only to a root loaded with its device (be-wasm-host's
/// `Host::with_display`); in any other a gpu call stops the root.
pub struct OwnSurface {
    id: u32,
    size: (u32, u32),
    surface: gpu::GpuCanvasContext,
}

impl OwnSurface {
    /// The surface `id`, `size` pixels, in RGBA with 8 bits a channel holding sRGB-encoded
    /// values, as an app's screens are. Nothing shows until it is drawn and presented.
    pub fn new(id: u32, size: (u32, u32)) -> Self {
        let surface = host::surface(id);
        configure(&surface, size);
        Self { id, size, surface }
    }

    /// Its id, which `Surface::Root` names.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Its size, in pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Makes it `size` pixels. What it showed stays until it is presented again.
    pub fn resize(&mut self, size: (u32, u32)) {
        if size != self.size {
            self.size = size;
            configure(&self.surface, size);
        }
    }

    /// The texture to draw its next frame into, whole, since it may hold an older frame:
    /// `present` shows it.
    pub fn acquire(&self) -> Result<gpu::GpuTexture, String> {
        let texture = self.surface.get_current_texture();
        match texture.width() {
            0 => Err(format!("root surface {} has no texture", self.id)),
            _ => Ok(texture),
        }
    }

    /// Shows what was drawn into the texture `acquire` gave.
    pub fn present(&self) {
        host::present(&self.surface);
    }

    /// Fills the whole surface with `rgba`, sRGB-encoded as an app's screens hold their pixels,
    /// and presents it.
    pub fn fill(&self, rgba: [f64; 4]) -> Result<(), String> {
        let texture = self.acquire()?;
        let device = device();
        let view = texture.create_view(&None);
        let encoder = device.create_command_encoder(&Some(web::GpuCommandEncoderDescriptor {
            label: Some("root surface".to_owned()),
        }));
        let [r, g, b, a] = rgba;
        let pass = encoder.begin_render_pass(&web::GpuRenderPassDescriptor {
            color_attachments: vec![Some(web::GpuRenderPassColorAttachment {
                view: view.handle(),
                depth_slice: None,
                resolve_target: None,
                clear_value: Some(web::GpuColor { r, g, b, a }),
                load_op: web::GpuLoadOp::Clear,
                store_op: web::GpuStoreOp::Store,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            max_draw_count: None,
            label: None,
        });
        pass.end();
        drop(pass);
        let commands = encoder.finish(&None);
        device.queue().submit(&[&commands]);
        self.present();
        Ok(())
    }

    /// The layer `id` showing the whole surface with its top left at `origin` in its window's
    /// points, at `raster` pixels a point. It takes input over all of it.
    pub fn layer(&self, id: LayerId, origin: Pos2, raster: f32) -> Layer {
        let points = 1.0 / raster;
        let (width, height) = self.size;
        Layer {
            id,
            surface: Surface::Root(self.id),
            size: self.size,
            transform: Affine::from_axes(vec2(points, 0.0), vec2(0.0, points), origin),
            clip: Rect::from_min_max(pos2(0.0, 0.0), pos2(width as f32, height as f32)),
            corner_radius: 0.0,
            opacity: 1.0,
            shown: true,
            input: LayerInput::default(),
        }
    }
}

/// The device the root draws its surfaces with: the app device its wgpu draws with, or one it
/// asks for through these bindings when it draws with none.
fn device() -> ManuallyDrop<gpu::GpuDevice> {
    let device = be_wasm_gpu_abi::app_device().unwrap_or_else(|| {
        let mut adapter = gpu::get_gpu().request_adapter(&None);
        let adapter = loop {
            gpu::settle(true);
            if let Some(adapter) = adapter.take() {
                break adapter.expect("the host offers an adapter");
            }
        };
        let adapter = gpu::GpuAdapter::from_handle(adapter);
        let mut device = adapter.request_device(&None);
        let device = loop {
            gpu::settle(true);
            if let Some(device) = device.take() {
                break device.unwrap_or_else(|error| panic!("{}", error.message));
            }
        };
        be_wasm_gpu_abi::set_app_device(device);
        device
    });
    // The device lives as long as the root: dropping this view of it lets nothing go.
    ManuallyDrop::new(gpu::GpuDevice::from_handle(device))
}

fn configure(surface: &gpu::GpuCanvasContext, (width, height): (u32, u32)) {
    host::resize(surface, width, height);
    surface.configure(&web::GpuCanvasConfiguration {
        device: device().handle(),
        format: web::GpuTextureFormat::Rgba8unorm,
        usage: None,
        view_formats: None,
        color_space: None,
        tone_mapping: None,
        // beui draws premultiplied, and a root's surface may be translucent.
        alpha_mode: Some(web::GpuCanvasAlphaMode::Premultiplied),
    });
}
