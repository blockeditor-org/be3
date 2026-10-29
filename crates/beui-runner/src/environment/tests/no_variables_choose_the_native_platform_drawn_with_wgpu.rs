use super::*;

#[test]
fn no_variables_choose_the_native_platform_drawn_with_wgpu() {
    assert_eq!(choose("winit", with(&[])), Ok(NATIVE));
    assert_eq!(choose("winit", with(&[(RENDERER, "wgpu")])), Ok(NATIVE));
}
