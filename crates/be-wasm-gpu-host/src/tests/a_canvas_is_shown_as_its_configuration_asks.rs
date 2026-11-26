use super::web::*;
use super::*;

/// A gpu on a real device, which draws what the host shows.
fn drawing() -> Gpu {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("the test has lavapipe");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("lavapipe gives a device");
    Gpu::new(device, queue)
}

/// What surface 0 shows once its canvas, configured as `configuration` asks, is cleared to
/// `clear` and presented: its first pixel, and whether it is the canvas's own texture.
fn shown(configure: impl FnOnce(&mut GpuCanvasConfiguration), clear: wgpu::Color) -> ([u8; 4], bool) {
    let mut gpu = drawing();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = be_wasm_gpu_abi::embed::host::Host::surface(&mut gpu, 0);
    gpu.resize(context, 4, 4);
    let mut configuration = GpuCanvasConfiguration {
        device,
        format: GpuTextureFormat::Rgba8unorm,
        usage: None,
        view_formats: None,
        color_space: None,
        tone_mapping: None,
        alpha_mode: None,
    };
    configure(&mut configuration);
    gpu.gpu_canvas_context_configure(context, configuration);
    let canvas = current(&mut gpu, context);
    let mut encoder = gpu.device().create_command_encoder(&Default::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &canvas.create_view(&Default::default()),
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    gpu.queue().submit([encoder.finish()]);
    gpu.present(context);
    let (texture, _) = gpu.presented_surface(0).expect("the canvas was presented");
    let texture = texture.clone();
    let readable = gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device().create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readable,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    gpu.queue().submit([encoder.finish()]);
    readable.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    gpu.device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let pixel: [u8; 4] = readable.slice(..4).get_mapped_range().unwrap()[..4].try_into().unwrap();
    assert!(gpu.take_logged().is_empty(), "{:?}", gpu.take_logged());
    (pixel, texture == canvas)
}

fn near(shown: [u8; 4], wanted: [u8; 4]) -> bool {
    shown.iter().zip(wanted).all(|(shown, wanted)| shown.abs_diff(wanted) <= 2)
}

#[test]
fn a_canvas_is_shown_as_its_configuration_asks() {
    let gray = wgpu::Color { r: 0.2, g: 0.2, b: 0.2, a: 0.5 };
    let (pixel, own) = shown(
        |configuration| configuration.alpha_mode = Some(GpuCanvasAlphaMode::Premultiplied),
        gray,
    );
    assert!(near(pixel, [51, 51, 51, 128]), "{pixel:?}");
    assert!(own, "a premultiplied sRGB canvas is shown as it is");

    let (pixel, own) = shown(|_| {}, gray);
    assert!(near(pixel, [51, 51, 51, 255]), "an opaque canvas's alpha is ignored: {pixel:?}");
    assert!(!own);

    let red = wgpu::Color { r: 0.6, g: 0.3, b: 0.3, a: 1.0 };
    let (pixel, _) = shown(
        |configuration| {
            configuration.alpha_mode = Some(GpuCanvasAlphaMode::Premultiplied);
            configuration.color_space = Some(PredefinedColorSpace::DisplayP3);
        },
        red,
    );
    // display-p3's (0.6, 0.3, 0.3), decoded, taken to linear sRGB and encoded again.
    assert!(
        near(pixel, [164, 71, 74, 255]),
        "a display-p3 canvas is shown in sRGB: {pixel:?}"
    );
}
