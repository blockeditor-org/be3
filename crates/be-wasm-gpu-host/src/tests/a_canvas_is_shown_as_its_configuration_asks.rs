use super::looking::*;
use super::web::*;

#[test]
fn a_canvas_is_shown_as_its_configuration_asks() {
    let (mut gpu, _, context) = drawn_canvas(GpuTextureFormat::Rgba8unorm, premultiplied);
    let (pixel, own) = shown(&mut gpu, context, GRAY);
    assert!(near(pixel, [51, 51, 51, 128]), "{pixel:?}");
    assert!(own, "a premultiplied sRGB canvas is shown as it is");

    let (mut gpu, _, context) = drawn_canvas(GpuTextureFormat::Rgba8unorm, |_| {});
    let (pixel, own) = shown(&mut gpu, context, GRAY);
    assert!(
        near(pixel, [51, 51, 51, 255]),
        "an opaque canvas's alpha is ignored: {pixel:?}"
    );
    assert!(!own);

    let p3 = |configuration: &mut GpuCanvasConfiguration| {
        premultiplied(configuration);
        configuration.color_space = Some(PredefinedColorSpace::DisplayP3);
    };
    // display-p3's (0.6, 0.3, 0.3), decoded, taken to linear sRGB and encoded again.
    let (mut gpu, _, context) = drawn_canvas(GpuTextureFormat::Rgba8unorm, p3);
    let red = wgpu::Color {
        r: 0.6,
        g: 0.3,
        b: 0.3,
        a: 1.0,
    };
    let (pixel, _) = shown(&mut gpu, context, red);
    assert!(
        near(pixel, [164, 71, 74, 255]),
        "a display-p3 canvas is shown in sRGB: {pixel:?}"
    );
    // The same color half covering, premultiplied: converted as the color it covers with.
    let half = wgpu::Color {
        r: 0.3,
        g: 0.15,
        b: 0.15,
        a: 0.5,
    };
    let (pixel, _) = shown(&mut gpu, context, half);
    assert!(
        near(pixel, [82, 35, 37, 128]),
        "and its coverage kept: {pixel:?}"
    );

    let (mut gpu, _, context) = drawn_canvas(GpuTextureFormat::Rgba16float, premultiplied);
    let bright = wgpu::Color {
        r: 2.0,
        g: 0.5,
        b: -1.0,
        a: 1.0,
    };
    let (pixel, own) = shown(&mut gpu, context, bright);
    assert!(
        near(pixel, [255, 128, 0, 255]),
        "a float canvas is clamped: {pixel:?}"
    );
    assert!(!own);
}
