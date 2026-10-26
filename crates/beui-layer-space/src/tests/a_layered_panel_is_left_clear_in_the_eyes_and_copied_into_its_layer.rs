use super::*;
use crate::head::Views;
use crate::layers::Allowance;
use crate::{LayerTarget, ViewTarget};

#[test]
fn a_layered_panel_is_left_clear_in_the_eyes_and_copied_into_its_layer() {
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
    space.set_views(Some(Views {
        display_time: now,
        views: vec![space.head.view(size)],
    }));

    let layers = space.layers(&Allowance {
        most: 4,
        cylinders: false,
        largest: (8192, 8192),
    });
    let [layer] = layers[..] else {
        panic!("the main panel straight ahead is layered: {layers:?}");
    };
    assert_eq!(layer.window, WindowId::MAIN);

    let texture = |label, (width, height)| {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
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
    let eye = texture("eye", (size.x as u32, size.y as u32));
    let quad = texture("layer", layer.pixels);
    let (eye_view, quad_view) = (
        eye.create_view(&wgpu::TextureViewDescriptor::default()),
        quad.create_view(&wgpu::TextureViewDescriptor::default()),
    );
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    space.draw(
        &mut encoder,
        &[ViewTarget {
            view: 0,
            texture: &eye_view,
            format: gpu.format,
        }],
        &[LayerTarget {
            window: WindowId::MAIN,
            fill: Some((&quad_view, gpu.format)),
        }],
    );
    gpu.queue.submit([encoder.finish()]);

    let middle = |texture: &wgpu::Texture| {
        let capture = beui_renderer_wgpu::present::read_texture(&gpu.device, &gpu.queue, texture)
            .expect("the target can be read");
        let at = ((capture.height / 2 * capture.width + capture.width / 2) * 4) as usize;
        [
            capture.rgba[at],
            capture.rgba[at + 1],
            capture.rgba[at + 2],
            capture.rgba[at + 3],
        ]
    };
    assert_eq!(
        middle(&eye)[..3],
        [0, 0, 0],
        "the eye leaves the layered panel clear (a capture reads alpha as opaque), for the \
         layer behind to show through"
    );
    let copied = middle(&quad);
    assert!(
        copied
            .iter()
            .zip(background.to_array())
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "and the layer gets the panel's pixels: {copied:?}, not {background:?}"
    );
}
