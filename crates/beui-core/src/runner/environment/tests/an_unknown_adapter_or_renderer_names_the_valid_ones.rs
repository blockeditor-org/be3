use super::*;

#[test]
fn an_unknown_adapter_or_renderer_names_the_valid_ones() {
    assert_eq!(
        choose("winit", with(&[(ADAPTER, "vr")])),
        Err("BEUI_ADAPTER takes winit, headless or space, not vr".to_owned())
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(ADAPTER, "space"), (SPACE_ADAPTER, "space")])
        ),
        Err("BEUI_SPACE_ADAPTER takes winit or headless, not space".to_owned())
    );
    assert_eq!(
        choose("winit", with(&[(RENDERER, "vulkan")])),
        Err("BEUI_RENDERER takes wgpu or none, not vulkan".to_owned())
    );
}
