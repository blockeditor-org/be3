use super::*;

#[test]
fn a_texture_is_drawn_into_its_rect_and_nowhere_else() {
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
    let texture = |label, size: u32| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    };
    let picture = texture("picture", 2);
    let red = [255u8, 0, 0, 255];
    queue.write_texture(
        picture.as_image_copy(),
        &red.repeat(4),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(2),
        },
        wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
    );
    let target = texture("target", 8);
    let draw = TextureDraw::new(picture, ());
    let placement = at([0.0, 0.0, 4.0, 8.0], [0.0, 0.0, 8.0, 8.0]);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    draw.prepare(&device, &queue, &mut encoder, placement);
    {
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("target"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        draw.paint(&mut pass, placement);
    }
    queue.submit([encoder.finish()]);
    let read = beui::read_texture(&device, &queue, &target).expect("the target is read back");
    let pixel = |x: usize, y: usize| {
        let at = (y * 8 + x) * 4;
        read.rgba[at..at + 4].to_vec()
    };
    assert_eq!(pixel(1, 4), red.to_vec());
    assert_eq!(pixel(6, 4)[..3], [0, 0, 0]);
}
