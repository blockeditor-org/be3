use crate::present::surface_format;

#[test]
fn surface_formats_requiring_optional_features_are_not_selected() {
    use wgpu::TextureFormat::{Bgra8UnormSrgb, Rgba16Unorm};

    assert_eq!(surface_format(&[Rgba16Unorm]), None);
    assert_eq!(surface_format(&[]), None);
    assert_eq!(
        surface_format(&[Rgba16Unorm, Bgra8UnormSrgb]),
        Some(Bgra8UnormSrgb),
    );
}
