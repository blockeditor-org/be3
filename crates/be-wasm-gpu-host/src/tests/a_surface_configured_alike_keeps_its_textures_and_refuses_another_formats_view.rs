use super::*;

fn configured(view: Option<abi::TextureFormat>) -> abi::SurfaceConfiguration {
    abi::SurfaceConfiguration {
        width: 64,
        height: 48,
        format: abi::TextureFormat::Rgba8UnormSrgb,
        view_format: view,
    }
}

#[test]
fn a_surface_configured_alike_keeps_its_textures_and_refuses_another_formats_view() {
    let mut gpu = gpu();
    let surface = gpu.surface_new(0);
    gpu.surface_configure(surface, configured(None));
    let made = gpu.surface(0).expect("the surface has textures").0.clone();
    gpu.surface_configure(
        surface,
        configured(Some(abi::TextureFormat::Rgba8UnormSrgb)),
    );
    assert_eq!(
        gpu.surface(0).map(|(texture, _)| texture.clone()),
        Some(made),
        "the format itself as its view changes nothing"
    );

    gpu.surface_configure(surface, configured(Some(abi::TextureFormat::Rgba8Unorm)));
    let viewed = gpu.surface(0).expect("the surface has textures").0.clone();
    gpu.surface_configure(surface, configured(Some(abi::TextureFormat::Rgba8Unorm)));
    assert_eq!(
        gpu.surface(0).map(|(texture, _)| texture.clone()),
        Some(viewed),
        "the same view keeps its textures"
    );
    assert_eq!(gpu.take_error(), None);

    gpu.surface_configure(surface, configured(Some(abi::TextureFormat::Bgra8Unorm)));
    assert!(
        gpu.take_error()
            .is_some_and(|error| error.contains("sRGB suffix")),
        "a view format that is another format is refused"
    );
}
