//! A composited window (beui_draw::compose; plans/host_as_platform_adapter.md, The layer list):
//! the window's UI is rendered into textures of its own, the root's surfaces: `Surface::Root(0)`,
//! and above each anchored view the window shows, one more for what the UI paints after that
//! view (`Root(1)` for the first, as the web stacks `#layers` over the host's canvas). Screens and
//! program windows are drawn into the roots in place, in the UI's paint order
//! (`Placed::inline`), and listed for damage and input. The window is then drawn from its layer
//! list in a pass of its own: `Root(0)`, the first anchored view, `Root(1)`, the next, and so
//! on, so what the UI paints between two anchored views lies between them.

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
    fn new(
        device: &wgpu::Device,
        pipelines: &Pipelines,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Self {
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

/// What a drawing placed when it was last prepared, where it was.
pub(crate) struct Known {
    /// Held so the identity it is kept under is never another drawing's: while it is cached,
    /// no new drawing can be made at the address its identity names.
    #[allow(dead_code)]
    pub(crate) drawing: beui_core::drawing::Drawing,
    pub(crate) at: beui_draw::DrawAt,
    pub(crate) placed: Vec<Placed>,
}

impl Known {
    /// A drawing shown in several places is known in each.
    pub(crate) fn key(
        drawing: &beui_core::drawing::Drawing,
        at: &beui_draw::DrawAt,
    ) -> (usize, u64, [u32; 4]) {
        let (draw, revision) = drawing.identity();
        (draw, revision, at.rect.map(f32::to_bits))
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
    /// `Root(0)` and one root above each anchored view the window shows, which holds what the
    /// UI paints after that view and before the next.
    pub(crate) roots: Vec<Root>,
    /// The layers the roots after the first begin at, one per anchored view: when they change,
    /// painted after it moves between the roots, so they are painted whole.
    pub(crate) splits_at: Vec<LayerId>,
    /// Where each root after the first begins in the frame's steps.
    pub(crate) splits: Vec<usize>,
    /// The window's layers this frame and last, with the texture of each that is not a root's.
    list: WindowLayers,
    previous: WindowLayers,
    shown: be_lib_stable_hash::HashMap<LayerId, Placed>,
    /// What each drawing the window showed last placed, so one that is where it was need not
    /// be prepared again (`Renderer::place`).
    pub(crate) known: be_lib_stable_hash::HashMap<(usize, u64, [u32; 4]), Known>,
    /// The drawings (by `Drawing::identity`'s first half) that placed a composited layer, which
    /// the next frame visits whether or not they are damaged.
    pub(crate) lifted: be_lib_stable_hash::HashSet<usize>,
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
pub(crate) fn root_layer(
    index: u32,
    size: (u32, u32),
    pixels_per_point: f32,
    origin: Vec2,
) -> Layer {
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
            roots: vec![Root::new(device, &pipelines, format, size)],
            hole: Hole::new(device, queue),
            fill: (fill.clone(), pipelines.bind(&fill), None),
            pipelines,
            format,
            size,
            splits_at: Vec::new(),
            splits: Vec::new(),
            list: WindowLayers::default(),
            previous: WindowLayers::default(),
            shown: be_lib_stable_hash::HashMap::default(),
            known: be_lib_stable_hash::HashMap::default(),
            lifted: be_lib_stable_hash::HashSet::default(),
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

    /// Makes a root for each of this frame's splits, and drops those it no longer has.
    pub(crate) fn fit_roots(&mut self, device: &wgpu::Device) {
        let wanted = self.splits.len() + 1;
        self.roots.truncate(wanted);
        while self.roots.len() < wanted {
            let root = Root::new(device, &self.pipelines, self.format, self.size);
            self.roots.push(root);
        }
    }

    /// Takes the frame's layers: `placed`, each in the target's points with the segment it lies
    /// in (how many anchored views lie under it), and the roots, with `damaged` the roots'
    /// damage in the target's pixels (`None`: painted whole). Finds where the window is drawn
    /// again (`composed`): the list's damage against last frame's, the roots' own damage through
    /// their layers.
    pub(crate) fn take(
        &mut self,
        placed: Vec<(Placed, usize)>,
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
        // Each layer lies under the root of its segment: Root(0) over what is placed before the
        // first anchored view, Root(n) over the nth anchored view and what follows it.
        let mut segments: Vec<Vec<Layer>> = vec![Vec::new(); self.splits_at.len() + 1];
        self.shown.clear();
        for (mut placed, segment) in placed {
            placed.layer.transform = placed.layer.transform.then(&shift);
            let layer = placed.layer.clone();
            self.shown.insert(layer.id, placed);
            let last = segments.len() - 1;
            segments[segment.min(last)].push(layer);
        }
        let mut layers = Vec::new();
        for (index, segment) in segments.into_iter().enumerate() {
            layers.extend(segment);
            layers.push(root_layer(index as u32, self.size, pixels_per_point, origin));
        }
        let below = layers;
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
                    // The UI damages a drawing whose surface changed, as it did when every
                    // surface was drawn in place, so a surface's own damage is the roots'.
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
        // Each root reports the frame's damage, and a layer's old and new bounds often
        // overlap: rects that overlap are drawn as one, so nothing is drawn twice and the
        // display hears of each part once.
        if let Some(composed) = &mut self.composed {
            *composed = merged(std::mem::take(composed));
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
                    Surface::Root(index) => Some((&self.roots.get(index as usize)?.bound, quad)),
                    _ => {
                        let placed = self.shown.get(&layer.id).filter(|placed| !placed.inline)?;
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
        self.roots.iter().map(|root| &root.texture)
    }
}

/// `rects` (`[left, top, width, height]`), with any that overlap merged into their bounds until
/// none do.
fn merged(mut rects: Vec<[u32; 4]>) -> Vec<[u32; 4]> {
    let overlap = |[x0, y0, w0, h0]: [u32; 4], [x1, y1, w1, h1]: [u32; 4]| {
        x0 < x1 + w1 && x1 < x0 + w0 && y0 < y1 + h1 && y1 < y0 + h0
    };
    let union = |[x0, y0, w0, h0]: [u32; 4], [x1, y1, w1, h1]: [u32; 4]| {
        let (left, top) = (x0.min(x1), y0.min(y1));
        let (right, bottom) = ((x0 + w0).max(x1 + w1), (y0 + h0).max(y1 + h1));
        [left, top, right - left, bottom - top]
    };
    let mut merging = true;
    while merging {
        merging = false;
        let mut index = 0;
        while index < rects.len() {
            let found = (index + 1..rects.len()).find(|other| overlap(rects[index], rects[*other]));
            match found {
                Some(other) => {
                    let taken = rects.swap_remove(other);
                    rects[index] = union(rects[index], taken);
                    merging = true;
                }
                None => index += 1,
            }
        }
    }
    rects.sort_unstable();
    rects
}
