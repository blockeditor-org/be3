use super::*;

#[test]
fn an_unknown_platform_or_renderer_names_the_valid_ones() {
    assert_eq!(
        choose("winit", with(&[(PLATFORM, "vr")])),
        Err("BEUI_PLATFORM takes winit or headless, not vr".to_owned())
    );
    assert_eq!(
        choose("winit", with(&[(RENDERER, "vulkan")])),
        Err("BEUI_RENDERER takes wgpu or none, not vulkan".to_owned())
    );
}
