use super::web::*;
use super::*;

#[test]
fn a_canvas_answers_its_configuration_with_the_defaults_filled_in() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = be_wasm_gpu_abi::embed::host::Host::surface(&mut gpu, 0);
    assert!(gpu.gpu_canvas_context_get_configuration(context).is_none());
    gpu.resize(context, 4, 4);
    gpu.gpu_canvas_context_configure(
        context,
        GpuCanvasConfiguration {
            device,
            format: GpuTextureFormat::Bgra8unorm,
            usage: None,
            view_formats: None,
            color_space: Some(PredefinedColorSpace::DisplayP3),
            tone_mapping: None,
            alpha_mode: None,
        },
    );
    let configured = gpu
        .gpu_canvas_context_get_configuration(context)
        .expect("the canvas is configured");
    assert_eq!(configured.format, GpuTextureFormat::Bgra8unorm);
    assert_eq!(configured.color_space, Some(PredefinedColorSpace::DisplayP3));
    assert_eq!(configured.usage, Some(GpuTextureUsage::RENDER_ATTACHMENT));
    assert_eq!(configured.view_formats, Some(Vec::new()));
    assert_eq!(configured.alpha_mode, Some(GpuCanvasAlphaMode::Opaque));
    assert_eq!(
        configured.tone_mapping.and_then(|tone_mapping| tone_mapping.mode),
        Some(GpuCanvasToneMappingMode::Standard)
    );
    gpu.gpu_canvas_context_unconfigure(context);
    assert!(Gpu::surface(&gpu, 0).is_none(), "unconfigured, it has no textures");
    assert_eq!(gpu.take_released(), vec![0], "and the host lets it go");
}
