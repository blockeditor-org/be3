use super::*;

mod a_blur_repaints_the_light_it_spreads_outside_the_damaged_region;
mod a_blur_thinner_than_a_pixel_spreads_less_light_than_a_whole_one;
mod a_blurred_region_spreads_light_past_the_shape_that_made_it;
mod a_bounded_renderer_filters_and_paints_only_within_its_bounds;
mod a_clip_rectangle_hides_what_falls_outside_it;
mod a_colour_vision_filter_recolours_the_region_it_covers;
mod a_drawing_paints_between_the_shapes_around_it;
mod a_drawing_repaints_only_within_each_damaged_region;
mod a_fill_with_fractional_bounds_lands_on_whole_pixels;
mod a_filled_rectangle_covers_its_bounds;
mod a_filter_leaves_the_painting_outside_its_region_alone;
mod a_frame_paints_its_outline_over_its_fill;
mod a_layer_painted_above_a_filter_keeps_its_own_colours;
mod a_punch_clears_what_it_covers;
mod a_rect_rounds_each_corner_by_its_own_radius;
mod a_renderer_at_an_origin_paints_the_part_of_the_document_there;
mod a_repaint_of_two_regions_leaves_what_lies_between_them;
mod a_rotated_rectangle_covers_the_corners_it_turned_onto;
mod a_scrolled_document_moves_the_rows_it_already_encoded;
mod a_window_blends_a_thin_rounded_outline_as_dark_as_its_straight_edges;
mod an_icon_glyph_paints_over_the_background;
mod copying_a_scrolled_region_paints_what_a_full_repaint_would;
mod reducing_contrast_pulls_the_filtered_region_toward_grey;
mod repainting_a_damaged_region_keeps_the_rest_of_the_retained_frame;
mod repainting_covers_every_region_gathered_since_the_last_draw;
mod repeated_partial_repaints_under_a_blur_match_a_full_one;
mod rotated_text_lands_where_the_upright_run_was_turned_to;
mod text_at_a_fractional_origin_lands_on_whole_pixels;
mod text_paints_glyphs_over_the_background;
mod turning_a_filter_off_repaints_the_frame_it_had_blurred;

use std::cell::RefCell;

use crate::Draw;
use crate::DrawAt;
use beui_core::context::Context;
use beui_core::document::Document;
use beui_core::filter::{ColorVision, Filter};
use beui_core::font::FontId;
use beui_core::geometry::{Pos2, Rect, pos2, vec2};
use beui_core::input::RawInput;
use beui_core::painter::Painter;

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub struct Capture {
    pixels: Vec<u8>,
}

impl Capture {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * SIZE + x) * 4) as usize;
        let mut pixel = [0; 4];
        pixel.copy_from_slice(&self.pixels[offset..offset + 4]);
        pixel
    }

    pub fn brightest(&self, rect: Rect) -> u8 {
        let mut brightest = 0;
        for y in rect.top() as u32..rect.bottom() as u32 {
            for x in rect.left() as u32..rect.right() as u32 {
                brightest = brightest.max(self.pixel(x, y)[0]);
            }
        }
        brightest
    }
}

pub fn everything() -> Rect {
    Rect::from_min_max(Pos2::ZERO, pos2(SIZE as f32, SIZE as f32))
}

pub fn capture(background: Color32, paint: impl FnOnce(&Painter)) -> Capture {
    capture_in(FORMAT, background, paint)
}

pub fn capture_in(
    format: wgpu::TextureFormat,
    background: Color32,
    paint: impl FnOnce(&Painter),
) -> Capture {
    let mut target = Target::in_format(format);
    target.draw(background, Repaint::Everything, paint);
    target.read()
}

pub struct Target {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    cleared: bool,
    bounded: bool,
}

impl Target {
    pub fn new() -> Self {
        Self::in_format(FORMAT)
    }

    pub fn in_format(format: wgpu::TextureFormat) -> Self {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("no graphics adapter is available");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("beui test device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("the adapter did not provide a device");
        let renderer = Renderer::new(&device, format);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui test target"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            device,
            queue,
            renderer,
            texture,
            view,
            format,
            cleared: false,
            bounded: false,
        }
    }

    pub fn bound(&mut self, bounds: [u32; 4]) {
        self.renderer.set_bounds(Some(bounds));
        self.bounded = true;
    }

    pub fn draw(&mut self, background: Color32, repaint: Repaint, paint: impl FnOnce(&Painter)) {
        self.draw_in(
            &Context::new(beui_font_freetype::FreetypeFonts::default()),
            background,
            |_| repaint,
            paint,
        );
    }

    pub fn draw_moving(
        &mut self,
        context: &Context,
        background: Color32,
        paint: impl FnOnce(&Painter),
    ) -> Option<beui_core::context::Moved> {
        let output = context.run(RawInput::default(), |context| paint(&context.painter()));
        let (repaint, moved) = output.repaint_moving(background);
        let effective = self.renderer.prepare(
            &self.device,
            &self.queue,
            &output,
            vec2(SIZE as f32, SIZE as f32),
            1.0,
            repaint,
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui test encoder"),
            });
        let load = match effective {
            Repaint::Region { .. } => {
                if let Some(moved) = moved {
                    self.renderer
                        .shift(&self.device, &mut encoder, &self.texture, moved, 1.0);
                }
                wgpu::LoadOp::Load
            }
            Repaint::Everything => wgpu::LoadOp::Clear(clear_color_in(self.format, background)),
        };
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.view,
            (SIZE, SIZE),
            load,
        );
        self.queue.submit(Some(encoder.finish()));
        moved
    }

    pub fn draw_in(
        &mut self,
        context: &Context,
        background: Color32,
        repaint: impl FnOnce(&beui_core::context::FrameOutput) -> Repaint,
        paint: impl FnOnce(&Painter),
    ) {
        let output = context.run(RawInput::default(), |context| paint(&context.painter()));
        let repaint = repaint(&output);
        let effective = self.renderer.prepare(
            &self.device,
            &self.queue,
            &output,
            vec2(SIZE as f32, SIZE as f32),
            1.0,
            repaint,
        );
        let load = match (self.cleared, effective) {
            (true, Repaint::Region { .. }) => wgpu::LoadOp::Load,
            (true, _) if self.bounded => wgpu::LoadOp::Load,
            _ => wgpu::LoadOp::Clear(clear_color_in(self.format, background)),
        };
        self.cleared = true;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui test encoder"),
            });
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.view,
            (SIZE, SIZE),
            load,
        );
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn read(&self) -> Capture {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beui test readback"),
            size: (SIZE * SIZE * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui test readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * 4),
                    rows_per_image: Some(SIZE),
                },
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device never finished the frame");
        let pixels = buffer.slice(..).get_mapped_range().to_vec();
        Capture { pixels }
    }
}

const SHADER: &str = r"
struct Placement {
    rect: vec4<f32>,
    screen: vec2<f32>,
    padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> placement: Placement;

fn corner(index: u32) -> vec2<f32> {
    let right = index == 1u || index == 4u || index == 5u;
    let bottom = index == 2u || index == 3u || index == 5u;
    return vec2<f32>(select(0.0, 1.0, right), select(0.0, 1.0, bottom));
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let point = mix(placement.rect.xy, placement.rect.zw, corner(index));
    return vec4<f32>(
        point / placement.screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
";

const PLACEMENT_BYTES: u64 = 32;

struct Patch {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    placement: wgpu::Buffer,
}

#[derive(Default)]
pub struct Green(RefCell<Option<Patch>>);

impl Draw for Green {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let mut held = self.0.borrow_mut();
        let patch = held.get_or_insert_with(|| Patch::new(device, at.format));
        let mut placement = [0.0_f32; 8];
        placement[..4].copy_from_slice(&at.rect);
        placement[4] = at.screen.x;
        placement[5] = at.screen.y;
        queue.write_buffer(&patch.placement, 0, bytemuck::cast_slice(&placement));
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, at: DrawAt) {
        let held = self.0.borrow();
        let Some(patch) = held.as_ref() else {
            return;
        };
        let left = at.clip[0].max(at.rect[0]).max(0.0) as u32;
        let top = at.clip[1].max(at.rect[1]).max(0.0) as u32;
        let right = at.clip[2].min(at.rect[2]).min(at.screen.x) as u32;
        let bottom = at.clip[3].min(at.rect[3]).min(at.screen.y) as u32;
        pass.set_scissor_rect(left, top, right - left, bottom - top);
        pass.set_pipeline(&patch.pipeline);
        pass.set_bind_group(0, &patch.bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

impl Patch {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("beui test patch"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("beui test patch"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let placement = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beui test patch placement"),
            size: PLACEMENT_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beui test patch"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: placement.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            bind_group,
            placement,
        }
    }
}
