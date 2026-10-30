use super::*;
use crate::surface::BlitPipeline;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SIZE: u32 = 64;

#[test]
fn a_surface_with_a_corner_radius_is_clipped_to_its_rounded_rect() {
    let adapter = pollster::block_on(wgpu::Instance::default().request_adapter(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        },
    ))
    .expect("no graphics adapter is available");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .expect("the adapter did not provide a device");
    let regions = RegionLayout::new(&device);
    let blit = BlitPipeline::new(&device, &regions, FORMAT);

    let usage = wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::COPY_DST;
    let texture = |label| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage,
            view_formats: &[],
        })
    };
    let surface = texture("plugin surface");
    let white = [255u8; 4];
    queue.write_texture(
        surface.as_image_copy(),
        &white.repeat((SIZE * SIZE) as usize),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SIZE * 4),
            rows_per_image: Some(SIZE),
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    let surface_group = blit.texture_group(
        &device,
        &surface.create_view(&wgpu::TextureViewDescriptor::default()),
    );

    let mut region = [0.0f32; (REGION_BYTES / 4) as usize];
    region[2..4].copy_from_slice(&[1.0, 1.0]);
    region[4..12].copy_from_slice(&[-1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0]);
    region[12] = 1.0;
    region[13..16].copy_from_slice(&[SIZE as f32, SIZE as f32, 16.0]);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("region"),
        size: REGION_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&region));
    let region_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("region"),
        layout: &regions.layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    let screen = texture("screen");
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let view = screen.create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&blit.pipeline);
        pass.set_bind_group(0, &surface_group, &[]);
        pass.set_bind_group(1, &region_group, &[0]);
        pass.draw(0..6, 0..1);
    }
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(SIZE * SIZE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        screen.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
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
    queue.submit(Some(encoder.finish()));
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the device never finished the frame");
    let pixels = readback.slice(..).get_mapped_range().to_vec();

    let red = |x: u32, y: u32| pixels[((y * SIZE + x) * 4) as usize];
    assert_eq!(red(0, 0), 0, "the corner outside the rounding stays black");
    assert_eq!(red(SIZE - 1, SIZE - 1), 0, "every corner is rounded");
    assert_eq!(
        red(SIZE / 2, 0),
        255,
        "the edge between the corners is covered"
    );
    assert_eq!(red(8, 8), 255, "inside the rounding is covered");
}
