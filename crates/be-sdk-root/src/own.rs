//! A surface of the root's own, which it draws on the host's device as an app draws its screens
//! (`be3:gpu`'s `surface`), and names in its layer list as `Surface::Root`.

use be_lib_geometry::{Pos2, Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerId, LayerInput, Surface};
use be_wasm_gpu_abi::{device as abi, guest::device as gpu};

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
    surface: gpu::Surface,
}

impl OwnSurface {
    /// The surface `id`, `size` pixels, in RGBA with 8 bits a channel holding sRGB-encoded
    /// values, as an app's screens are. Nothing shows until it is drawn and presented.
    pub fn new(id: u32, size: (u32, u32)) -> Self {
        let surface = gpu::Surface::new(id);
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
    pub fn acquire(&self) -> Result<gpu::Texture, String> {
        self.surface.acquire().ok_or_else(last_error)
    }

    /// Shows what was drawn into the texture `acquire` gave.
    pub fn present(&self) {
        self.surface.present();
    }

    /// Fills the whole surface with `rgba`, sRGB-encoded with straight alpha, and presents it.
    pub fn fill(&self, rgba: [f64; 4]) -> Result<(), String> {
        let texture = self.acquire()?;
        let view = texture
            .create_view(&abi::TextureViewDescriptor {
                label: Some("root surface".to_owned()),
                format: None,
                dimension: None,
                usage: None,
                aspect: abi::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            })
            .ok_or_else(last_error)?;
        let encoder = gpu::create_command_encoder(&abi::CommandEncoderDescriptor {
            label: Some("root surface".to_owned()),
        })
        .ok_or_else(last_error)?;
        let [r, g, b, a] = rgba;
        let pass = encoder
            .begin_render_pass(&abi::RenderPassDescriptor {
                label: None,
                color_attachments: vec![Some(abi::ColorAttachment {
                    view: view.handle(),
                    resolve_target: None,
                    load: abi::ColorLoadOp::Clear(abi::Color { r, g, b, a }),
                    store: abi::StoreOp::Store,
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            })
            .ok_or_else(last_error)?;
        pass.end();
        drop(pass);
        let commands = encoder.finish().ok_or_else(last_error)?;
        gpu::submit(vec![commands]);
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

fn configure(surface: &gpu::Surface, (width, height): (u32, u32)) {
    surface.configure(abi::SurfaceConfiguration {
        width,
        height,
        format: abi::TextureFormat::Rgba8Unorm,
    });
}

/// Why the last gpu call failed, as the host kept it.
fn last_error() -> String {
    gpu::take_error().unwrap_or_else(|| "the gpu refused the call".to_owned())
}
