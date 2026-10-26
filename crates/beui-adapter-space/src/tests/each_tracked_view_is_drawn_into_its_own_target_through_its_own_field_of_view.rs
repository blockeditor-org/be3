use super::*;
use crate::ViewTarget;
use crate::head::{Fov, View, Views};

#[test]
fn each_tracked_view_is_drawn_into_its_own_target_through_its_own_field_of_view() {
    let gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        Default::default(),
    ))
    .expect("no graphics adapter is available");
    let mut setup = beui_core::app::Setup::new(beui_core::app::Waker::new(|| {}));
    setup.provide(gpu.setup());
    let (mut space, now) = space();
    space.setup(&setup);
    let background = beui_core::color::Color32::from_rgb(200, 60, 40);
    let mut output = app_frame(CursorIcon::Default);
    for window in &mut output.windows {
        window.size = PLATFORM;
        window.changed = true;
        window.window.appearance.clear_color = background;
    }
    space.frame(&output, &view(now));

    let size = vec2(64.0, 48.0);
    let ahead = space.head.view(size);
    let upper_right = View {
        fov: Fov::tangents(0.7, 1.0, 0.9, 0.6),
        ..ahead
    };
    space.set_views(Some(Views {
        display_time: now,
        views: vec![ahead, upper_right],
    }));
    let target = || {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("eye"),
            size: wgpu::Extent3d {
                width: size.x as u32,
                height: size.y as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let eyes = [target(), target()];
    let views: Vec<wgpu::TextureView> = eyes
        .iter()
        .map(|eye| eye.create_view(&wgpu::TextureViewDescriptor::default()))
        .collect();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let targets: Vec<ViewTarget> = views
        .iter()
        .enumerate()
        .map(|(view, texture)| ViewTarget {
            view,
            texture,
            format: gpu.format,
        })
        .collect();
    space.draw(&mut encoder, &targets);
    gpu.queue.submit([encoder.finish()]);

    let middle = |eye: &wgpu::Texture| {
        let capture = beui_renderer_wgpu::present::read_texture(&gpu.device, &gpu.queue, eye)
            .expect("the eye's target can be read");
        let at = ((capture.height / 2 * capture.width + capture.width / 2) * 4) as usize;
        [
            capture.rgba[at],
            capture.rgba[at + 1],
            capture.rgba[at + 2],
            capture.rgba[at + 3],
        ]
    };
    let near = |a: [u8; 4], b: beui_core::color::Color32| {
        a.iter().zip(b.to_array()).all(|(a, b)| a.abs_diff(b) <= 2)
    };
    let (looking, beside) = (middle(&eyes[0]), middle(&eyes[1]));
    assert!(
        near(looking, background),
        "the view straight ahead shows the main panel in its middle: {looking:?}, not {background:?}"
    );
    assert!(
        near(beside, crate::SKY),
        "the view whose field lies up and to the right sees past the panel into the sky: \
         {beside:?}, not {:?}",
        crate::SKY
    );
}
