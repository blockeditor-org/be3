use super::*;

#[test]
fn an_unknown_platform_or_renderer_names_the_valid_ones() {
    assert_eq!(
        choose("winit", with(&[(PLATFORM, "vr")])),
        Err("BEUI_PLATFORM takes winit, headless or space, not vr".to_owned())
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(PLATFORM, "space"), (SPACE_PLATFORM, "space")])
        ),
        Err("BEUI_SPACE_PLATFORM takes winit or headless, not space".to_owned())
    );
    assert_eq!(
        choose("winit", with(&[(RENDERER, "vulkan")])),
        Err("BEUI_RENDERER takes wgpu or none, not vulkan".to_owned())
    );
}
