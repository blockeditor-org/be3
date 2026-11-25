use be_wasm_gpu_abi::embed::host::Host as _;

use super::web::*;
use super::*;

fn configured(device: GpuDevice, views: Vec<GpuTextureFormat>) -> GpuCanvasConfiguration {
    GpuCanvasConfiguration {
        device,
        format: GpuTextureFormat::Rgba8unorm,
        usage: None,
        view_formats: Some(views),
        color_space: None,
        tone_mapping: None,
        alpha_mode: None,
    }
}

#[test]
fn a_webgpu_canvas_has_textures_once_resized_and_configured() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    let context = be_wasm_gpu_abi::embed::host::Host::surface(&mut gpu, 7);
    gpu.gpu_canvas_context_configure(context, configured(device, vec![GpuTextureFormat::Rgba8unormSrgb]));
    let early = gpu.gpu_canvas_context_get_current_texture(context);
    assert!(gpu.textures.get(early.0, "texture").is_err(), "no size, no texture");
    assert!(pop(&mut gpu, device, 1).unwrap().is_some(), "and an error says why");

    gpu.resize(context, 64, 48);
    let first = gpu.gpu_canvas_context_get_current_texture(context);
    let again = gpu.gpu_canvas_context_get_current_texture(context);
    assert_ne!(first, again, "each call is a fresh handle");
    assert_eq!(gpu.gpu_texture_width(first), 64);
    assert_eq!(gpu.gpu_texture_height(first), 48);
    assert_eq!(
        gpu.textures.get(first.0, "texture").unwrap(),
        gpu.textures.get(again.0, "texture").unwrap(),
        "of the same texture until the frame is presented"
    );
    assert_eq!(gpu.age(context), 0);
    gpu.present(context);
    assert_eq!(gpu.take_presented(), vec![7]);
    let next = gpu.gpu_canvas_context_get_current_texture(context);
    assert_ne!(
        gpu.textures.get(first.0, "texture").unwrap(),
        gpu.textures.get(next.0, "texture").unwrap(),
        "the next frame draws into the other texture"
    );

    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    gpu.gpu_canvas_context_configure(context, configured(device, vec![GpuTextureFormat::Bgra8unorm]));
    assert!(
        pop(&mut gpu, device, 2).unwrap().is_some(),
        "a view format other than its sRGB twin is refused"
    );
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    let mut float = configured(device, Vec::new());
    float.format = GpuTextureFormat::Rgba8unormSrgb;
    gpu.gpu_canvas_context_configure(context, float);
    assert!(
        pop(&mut gpu, device, 3).unwrap().is_some(),
        "a canvas is never an sRGB format"
    );
}
