use be_wasm_gpu_abi::embed::host::Host as _;

use super::web::*;
use super::*;

/// A canvas configuration the device refuses (storage on bgra8unorm, which needs a feature the
/// app did not ask for) is a validation error, and the canvas has no textures, not invalid ones.
#[test]
fn a_webgpu_canvas_its_device_cannot_make_is_refused() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = be_wasm_gpu_abi::embed::host::Host::surface(&mut gpu, 7);
    gpu.resize(context, 8, 8);
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    gpu.gpu_canvas_context_configure(
        context,
        GpuCanvasConfiguration {
            device,
            format: GpuTextureFormat::Bgra8unorm,
            usage: Some(GpuTextureUsage::STORAGE_BINDING),
            view_formats: None,
            color_space: None,
            tone_mapping: None,
            alpha_mode: None,
        },
    );
    let caught = pop(&mut gpu, device, 1)
        .expect("the scope pops")
        .expect("the configuration is refused");
    let message = gpu.gpu_error_message(caught);
    assert!(
        message.contains("cannot make"),
        "it was refused as {message}"
    );
    assert!(gpu.surface(7).is_none(), "and the canvas has no textures");
}
