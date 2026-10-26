use super::*;

#[test]
fn the_native_adapter_is_chosen_by_its_own_name() {
    assert_eq!(choose("drm", with(&[(ADAPTER, "drm")])), Ok(NATIVE));
    assert_eq!(
        choose("drm", with(&[(ADAPTER, "winit")])),
        Err("BEUI_ADAPTER takes drm, headless or space, not winit".to_owned())
    );
}
