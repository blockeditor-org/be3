use super::*;

#[test]
fn the_native_platform_is_chosen_by_its_own_name() {
    assert_eq!(choose("drm", with(&[(PLATFORM, "drm")])), Ok(NATIVE));
    assert_eq!(
        choose("drm", with(&[(PLATFORM, "winit")])),
        Err("BEUI_PLATFORM takes drm, headless or space, not winit".to_owned())
    );
}
