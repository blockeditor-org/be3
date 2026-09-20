use super::*;

use std::cell::RefCell;

use crate::drawing::{Draw, DrawAt, Drawing};

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
struct Green(RefCell<Option<Patch>>);

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

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        let held = self.0.borrow();
        let Some(patch) = held.as_ref() else {
            return;
        };
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

#[test]
fn a_drawing_paints_between_the_shapes_around_it() {
    let drawing = Drawing::new(Green::default());
    let scene = Rect::from_min_max(pos2(8.0, 8.0), pos2(56.0, 56.0));
    let over = Rect::from_min_max(pos2(40.0, 40.0), pos2(56.0, 56.0));
    let capture = capture(Color32::TRANSPARENT, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
        painter.drawing(scene, &drawing);
        painter.rect_filled(over, 0.0, Color32::from_rgb(255, 0, 0));
    });

    assert_eq!(
        capture.pixel(24, 24),
        [0, 255, 0, 255],
        "the drawing should have painted its own rectangle"
    );
    assert_eq!(
        capture.pixel(4, 4),
        [255, 255, 255, 255],
        "the fill beneath the drawing should have been left alone outside it"
    );
    assert_eq!(
        capture.pixel(48, 48),
        [255, 0, 0, 255],
        "the shape painted after the drawing should still have reached the pass"
    );
}
