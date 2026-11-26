use super::looking::*;
use super::web::*;

#[test]
fn a_canvas_configured_or_resized_again_is_shown_as_it_is_now() {
    let (mut gpu, device, context) = drawn_canvas(GpuTextureFormat::Rgba8unorm, |_| {});
    assert!(near(shown(&mut gpu, context, GRAY).0, [51, 51, 51, 255]));

    reconfigure(
        &mut gpu,
        device,
        context,
        GpuTextureFormat::Rgba8unorm,
        premultiplied,
    );
    let (pixel, own) = shown(&mut gpu, context, GRAY);
    assert!(near(pixel, [51, 51, 51, 128]), "{pixel:?}");
    assert!(own, "configured premultiplied, it is shown as it is again");

    gpu.resize(context, 8, 2);
    reconfigure(
        &mut gpu,
        device,
        context,
        GpuTextureFormat::Rgba8unorm,
        |_| {},
    );
    let (pixel, own) = shown(&mut gpu, context, GRAY);
    assert!(near(pixel, [51, 51, 51, 255]), "{pixel:?}");
    assert!(!own);
    let (texture, _) = gpu.presented_surface(0).unwrap();
    assert_eq!(
        (texture.width(), texture.height()),
        (8, 2),
        "what shows it has its new size"
    );
}
