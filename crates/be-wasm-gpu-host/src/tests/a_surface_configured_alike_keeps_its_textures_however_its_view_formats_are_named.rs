use super::web::*;
use super::*;

#[test]
fn a_surface_configured_alike_keeps_its_textures_however_its_view_formats_are_named() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = canvas(
        &mut gpu,
        device,
        0,
        (8, 8),
        vec![GpuTextureFormat::Rgba8unormSrgb],
    );
    let made = gpu.surface(0).expect("the surface has textures").0.clone();
    let refused = scoped(&mut gpu, device, |gpu| {
        configure(
            gpu,
            device,
            context,
            vec![
                GpuTextureFormat::Rgba8unormSrgb,
                GpuTextureFormat::Rgba8unorm,
                GpuTextureFormat::Rgba8unorm,
            ],
        );
        configure(gpu, device, context, vec![GpuTextureFormat::Rgba8unormSrgb]);
    });
    assert_eq!(refused, None);
    assert_eq!(
        gpu.surface(0).map(|(texture, _)| texture.clone()),
        Some(made),
        "the format itself and a format named twice change nothing"
    );
    let refused = scoped(&mut gpu, device, |gpu| {
        configure(gpu, device, context, vec![GpuTextureFormat::Bgra8unorm]);
    });
    assert!(
        refused.is_some_and(|error| error.contains("sRGB suffix")),
        "a view format that is another format is refused"
    );
}
