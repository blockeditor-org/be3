use super::*;

fn configured(views: &[abi::TextureFormat]) -> abi::SurfaceConfiguration {
    abi::SurfaceConfiguration {
        width: 64,
        height: 48,
        format: abi::TextureFormat::Rgba8UnormSrgb,
        view_formats: views.to_vec(),
    }
}

#[test]
fn a_surface_configured_alike_keeps_its_textures_however_its_view_formats_are_named() {
    let mut gpu = gpu();
    let surface = gpu.surface_new(0);
    gpu.surface_configure(surface, configured(&[abi::TextureFormat::Rgba8Unorm]));
    let made = gpu.surface(0).expect("the surface has textures").0.clone();

    gpu.surface_configure(
        surface,
        configured(&[
            abi::TextureFormat::Rgba8UnormSrgb,
            abi::TextureFormat::Rgba8Unorm,
            abi::TextureFormat::Rgba8Unorm,
        ]),
    );
    assert_eq!(
        gpu.surface(0).map(|(texture, _)| texture.clone()),
        Some(made),
        "the format itself and a format named twice change nothing"
    );
    assert_eq!(gpu.take_error(), None);

    gpu.surface_configure(surface, configured(&[abi::TextureFormat::Bgra8Unorm]));
    assert!(
        gpu.take_error()
            .is_some_and(|error| error.contains("sRGB suffix")),
        "a view format that is another format is refused"
    );
}
