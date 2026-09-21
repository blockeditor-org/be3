use super::*;

mod a_blur_repaints_the_light_it_spreads_outside_the_damaged_region;
mod a_blur_thinner_than_a_pixel_spreads_less_light_than_a_whole_one;
mod a_blurred_region_spreads_light_past_the_shape_that_made_it;
mod a_clip_rectangle_hides_what_falls_outside_it;
mod a_colour_vision_filter_recolours_the_region_it_covers;
mod a_drawing_paints_between_the_shapes_around_it;
mod a_fill_with_fractional_bounds_lands_on_whole_pixels;
mod a_filled_rectangle_covers_its_bounds;
mod a_filter_leaves_the_painting_outside_its_region_alone;
mod a_frame_paints_its_outline_over_its_fill;
mod a_layer_painted_above_a_filter_keeps_its_own_colours;
mod a_punch_clears_what_it_covers;
mod a_rotated_rectangle_covers_the_corners_it_turned_onto;
mod an_icon_glyph_paints_over_the_background;
mod reducing_contrast_pulls_the_filtered_region_toward_grey;
mod repainting_a_damaged_region_keeps_the_rest_of_the_retained_frame;
mod repainting_covers_every_region_gathered_since_the_last_draw;
mod repeated_partial_repaints_under_a_blur_match_a_full_one;
mod rotated_text_lands_where_the_upright_run_was_turned_to;
mod text_at_a_fractional_origin_lands_on_whole_pixels;
mod text_paints_glyphs_over_the_background;
mod turning_a_filter_off_repaints_the_frame_it_had_blurred;

use crate::context::Context;
use crate::document::Document;
use crate::filter::{ColorVision, Filter};
use crate::font::FontId;
use crate::geometry::{Pos2, Rect, pos2, vec2};
use crate::input::RawInput;
use crate::painter::Painter;

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub(crate) struct Capture {
    pixels: Vec<u8>,
}

impl Capture {
    pub(crate) fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * SIZE + x) * 4) as usize;
        let mut pixel = [0; 4];
        pixel.copy_from_slice(&self.pixels[offset..offset + 4]);
        pixel
    }

    pub(crate) fn brightest(&self, rect: Rect) -> u8 {
        let mut brightest = 0;
        for y in rect.top() as u32..rect.bottom() as u32 {
            for x in rect.left() as u32..rect.right() as u32 {
                brightest = brightest.max(self.pixel(x, y)[0]);
            }
        }
        brightest
    }
}

pub(crate) fn everything() -> Rect {
    Rect::from_min_max(Pos2::ZERO, pos2(SIZE as f32, SIZE as f32))
}

pub(crate) fn capture(background: Color32, paint: impl FnOnce(&Painter)) -> Capture {
    let mut target = Target::new();
    target.draw(background, Repaint::Everything, paint);
    target.read()
}

pub(crate) struct Target {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    cleared: bool,
}

impl Target {
    pub(crate) fn new() -> Self {
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
        let renderer = Renderer::new(&device, FORMAT);
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
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            device,
            queue,
            renderer,
            texture,
            view,
            cleared: false,
        }
    }

    pub(crate) fn draw(
        &mut self,
        background: Color32,
        repaint: Repaint,
        paint: impl FnOnce(&Painter),
    ) {
        let context = Context::new();
        let output = context.run(RawInput::default(), |context| paint(&context.painter()));
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
            _ => wgpu::LoadOp::Clear(clear_color(background)),
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

    pub(crate) fn read(&self) -> Capture {
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
