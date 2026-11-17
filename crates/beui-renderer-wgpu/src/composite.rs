//! A composited window (beui_draw::compose; plans/host_as_platform_adapter.md, The layer list):
//! the window's UI is rendered into textures of its own, the root's surfaces `Surface::Root(0)`
//! and, while an anchored view shows, `Surface::Root(1)` for what is painted after the first
//! one, as the web stacks `#layers` over the host's canvas. The window is then drawn from its
//! layer list in a pass of its own: the layers drawings placed before that anchored view, which
//! show through the holes they punched in the lower root, the lower root, the layers placed
//! from the anchored view on, and the upper root.

use be_lib_layers::{Affine, Layer, LayerId, LayerInput, Surface, WindowLayers};
use beui_draw::compose::Placed;
use beui_draw::texture_quad::{Bound, Hole, Pipelines, Quad, Quads};
use beui_types::{Color32, Rect, Vec2, pos2};

/// A root surface: a texture the window's UI is rendered into.
pub(crate) struct Root {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    bound: Bound,
}

impl Root {
    fn new(device: &wgpu::Device, pipelines: &Pipelines, format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui root surface"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            bound: pipelines.bind(&texture),
            texture,
            view,
        }
    }
}

/// What a composited renderer keeps from frame to frame.
pub(crate) struct Composition {
    pipelines: Pipelines,
    /// Erases where the window is drawn again.
    hole: Hole,
    /// A pixel of the window's background, and which colour it holds.
    fill: (wgpu::Texture, Bound, Option<Color32>),
    format: wgpu::TextureFormat,
    size: (u32, u32),
    pub(crate) lower: Root,
    pub(crate) upper: Option<Root>,
    /// The layer the upper root begins at, the first anchored view's: when it changes, what is
    /// painted after it moves between the roots, so they are painted whole.
    pub(crate) splits_at: Option<LayerId>,
    /// Where the upper root's steps begin, this frame.
    pub(crate) split: Option<usize>,
    /// The window's layers this frame and last, with the texture of each that is not a root's.
    list: WindowLayers,
    previous: WindowLayers,
    shown: be_lib_stable_hash::HashMap<LayerId, Placed>,
    /// Where the window is drawn again this frame, in the target's pixels (`None`: whole).
    pub(crate) composed: Option<Vec<[u32; 4]>>,
    background: Color32,
    pixels_per_point: f32,
    origin: Vec2,
    quads: Quads,
    erased: Quads,
    filled: Quads,
}

fn pixel(device: &wgpu::Device, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write_pixel(queue: &wgpu::Queue, texture: &wgpu::Texture, rgba: [u8; 4]) {
    queue.write_texture(
        texture.as_image_copy(),
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
}

/// The layer that shows root surface `index`, `size` pixels at `pixels_per_point`, whose top
/// left is the window's `origin` (in pixels): over the whole target.
pub(crate) fn root_layer(index: u32, size: (u32, u32), pixels_per_point: f32, origin: Vec2) -> Layer {
    let per = pixels_per_point.max(f32::EPSILON).recip();
    Layer {
        id: LayerId(u64::from(index)),
        surface: Surface::Root(index),
        size,
        transform: Affine {
            x: origin.x * per,
            y: origin.y * per,
            ..Affine::scaling(per)
        },
        clip: Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    }
}

impl Composition {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Self {
        let pipelines = Pipelines::new(device);
        let fill = pixel(device, "beui composite background");
        Self {
            lower: Root::new(device, &pipelines, format, size),
            upper: None,
            hole: Hole::new(device, queue),
            fill: (fill.clone(), pipelines.bind(&fill), None),
            pipelines,
            format,
            size,
            splits_at: None,
            split: None,
            list: WindowLayers::default(),
            previous: WindowLayers::default(),
            shown: be_lib_stable_hash::HashMap::default(),
            composed: None,
            background: Color32::TRANSPARENT,
            pixels_per_point: 1.0,
            origin: Vec2::ZERO,
            quads: Quads::default(),
            erased: Quads::default(),
            filled: Quads::default(),
        }
    }

    /// Whether the roots are the target's size and format, so what they hold can be kept.
    pub(crate) fn fits(&self, format: wgpu::TextureFormat, size: (u32, u32)) -> bool {
        self.format == format && self.size == size
    }

    pub(crate) fn upper(&mut self, device: &wgpu::Device) -> &Root {
        let (format, size) = (self.format, self.size);
        let pipelines = &self.pipelines;
        self.upper
            .get_or_insert_with(|| Root::new(device, pipelines, format, size))
    }

    /// Takes the frame's layers: `placed`, each in the target's points with whether it lies
    /// over the lower root, and the roots, with `damaged` the roots' damage in the target's
    /// pixels (`None`: painted whole). Finds where the window is drawn again (`composed`): the
    /// list's damage against last frame's, the roots' own damage through their layers.
    pub(crate) fn take(
        &mut self,
        placed: Vec<(Placed, bool)>,
        pixels_per_point: f32,
        origin: Vec2,
        damaged: Option<&[[u32; 4]]>,
        background: Option<Color32>,
    ) {
        if let Some(background) = background {
            self.background = background;
        }
        self.pixels_per_point = pixels_per_point;
        self.origin = origin;
        let per = pixels_per_point.max(f32::EPSILON).recip();
        let shift = Affine {
            x: origin.x * per,
            y: origin.y * per,
            ..Affine::IDENTITY
        };
        let mut below = Vec::new();
        let mut above = Vec::new();
        self.shown.clear();
        for (mut placed, lifted) in placed {
            placed.layer.transform = placed.layer.transform.then(&shift);
            let layer = placed.layer.clone();
            self.shown.insert(layer.id, placed);
            match lifted {
                true => above.push(layer),
                false => below.push(layer),
            }
        }
        below.push(root_layer(0, self.size, pixels_per_point, origin));
        below.extend(above);
        if self.split.is_some() {
            below.push(root_layer(1, self.size, pixels_per_point, origin));
        }
        self.previous = std::mem::replace(
            &mut self.list,
            WindowLayers {
                window: None,
                layers: below,
            },
        );
        self.composed = damaged.map(|damaged| {
            let roots: Vec<Rect> = damaged
                .iter()
                .map(|[left, top, width, height]| {
                    Rect::from_min_max(
                        pos2(*left as f32, *top as f32),
                        pos2((left + width) as f32, (top + height) as f32),
                    )
                })
                .collect();
            self.previous
                .damage(&self.list, |layer| match layer.surface {
                    Surface::Root(_) => Some(roots.clone()),
                    // A drawing that shows a surface is painted again, with its hole, when what
                    // it shows changes, so a surface's own damage is the roots'.
                    _ => Some(Vec::new()),
                })
                .into_iter()
                .filter_map(|rect| {
                    let left = (rect.min.x * pixels_per_point - origin.x).floor().max(0.0);
                    let top = (rect.min.y * pixels_per_point - origin.y).floor().max(0.0);
                    let right = (rect.max.x * pixels_per_point - origin.x)
                        .ceil()
                        .min(self.size.0 as f32);
                    let bottom = (rect.max.y * pixels_per_point - origin.y)
                        .ceil()
                        .min(self.size.1 as f32);
                    (right > left && bottom > top).then(|| {
                        [
                            left as u32,
                            top as u32,
                            (right - left) as u32,
                            (bottom - top) as u32,
                        ]
                    })
                })
                .collect::<Vec<_>>()
        });
        // Each root reports the frame's damage, so it comes once per root.
        if let Some(composed) = &mut self.composed {
            composed.sort_unstable();
            composed.dedup();
        }
    }

    /// The window's layers this frame.
    pub(crate) fn list(&self) -> &WindowLayers {
        &self.list
    }

    /// Draws the window from its layers into `target`, whole or where it changed.
    pub(crate) fn compose(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
    ) {
        let (pixels_per_point, origin) = (self.pixels_per_point, self.origin);
        if self.fill.2 != Some(self.background) {
            write_pixel(queue, &self.fill.0, self.background.to_array());
            self.fill.2 = Some(self.background);
        }
        let screen = [self.size.0 as f32, self.size.1 as f32];
        let origin = [origin.x, origin.y];
        let whole = [0.0, 0.0, screen[0], screen[1]];
        let quads: Vec<(&Bound, Quad)> = self
            .list
            .layers
            .iter()
            .filter(|layer| layer.shown)
            .filter_map(|layer| {
                let quad = beui_draw::compose::quad(layer, pixels_per_point, origin);
                match layer.surface {
                    Surface::Root(0) => Some((&self.lower.bound, quad)),
                    Surface::Root(_) => Some((&self.upper.as_ref()?.bound, quad)),
                    _ => {
                        let placed = self.shown.get(&layer.id)?;
                        Some((
                            &placed.texture,
                            Quad {
                                opaque: placed.opaque,
                                srgb_encoded: placed.srgb_encoded,
                                ..quad
                            },
                        ))
                    }
                }
            })
            .collect();
        self.quads
            .prepare(&self.pipelines, queue, self.format, screen, quads);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("beui composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let Some(composed) = &self.composed else {
            self.quads.paint(&mut pass);
            return;
        };
        // Where the window is drawn again, what was there goes: it is erased, given the
        // window's background, and drawn over.
        let fill = Quad {
            rect: whole,
            srgb_encoded: true,
            ..Quad::default()
        };
        self.erased.prepare(
            &self.hole.pipelines,
            queue,
            self.format,
            screen,
            [(&self.hole.white, Quad { rect: whole, ..Quad::default() })],
        );
        self.filled
            .prepare(&self.pipelines, queue, self.format, screen, [(&self.fill.1, fill)]);
        for [left, top, width, height] in composed {
            if *width == 0 || *height == 0 {
                continue;
            }
            pass.set_scissor_rect(*left, *top, *width, *height);
            self.erased.paint(&mut pass);
            if self.background.to_array()[3] > 0 {
                self.filled.paint(&mut pass);
            }
            self.quads.paint(&mut pass);
        }
    }

    pub(crate) fn roots(&self) -> impl Iterator<Item = &wgpu::Texture> {
        std::iter::once(&self.lower.texture).chain(self.upper.as_ref().map(|root| &root.texture))
    }
}
