use super::*;

use beui::Repainting as _;
use std::io::Write as _;
use std::os::fd::AsFd;

use crate::test_client::pattern;

#[test]
fn a_shown_dmabuf_is_released_once_a_newer_one_is_painted() {
    let (mut harness, device, queue) = Harness::with_gpu();
    let (window, _) = harness.open();
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let mut renderer = beui::Renderer::new(&device, format);
    let size = (SCREEN.x as u32, SCREEN.y as u32);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screen"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut paint = |harness: &mut Harness| {
        let output = harness.output.as_ref().expect("a frame ran");
        let repaint = output.repaint(beui::Color32::BLACK);
        renderer.prepare(&device, &queue, output, SCREEN, 1.0, repaint);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer.render(
            &device,
            &queue,
            &mut encoder,
            &view,
            size,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        let index = queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("the frame finishes");
    };

    let committed = 6;
    let mut files = Vec::new();
    for _ in 0..committed {
        let fd = rustix::fs::memfd_create("dmabuf", rustix::fs::MemfdFlags::CLOEXEC)
            .expect("a memfd opens");
        let mut file = std::fs::File::from(fd);
        file.write_all(&pattern(64, 16))
            .expect("the pixels are written");
        harness
            .client
            .attach_dmabuf_unsent(&window, file.as_fd(), 64, 16);
        files.push(file);
        harness.frame(Vec::new());
        paint(&mut harness);
    }
    harness.frame(Vec::new());

    assert!(
        harness.client.received.released >= committed - 1,
        "{} of {committed} dmabufs were released; only the one on screen may be held",
        harness.client.received.released
    );
}
