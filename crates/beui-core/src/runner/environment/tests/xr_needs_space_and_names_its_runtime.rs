use super::*;

#[test]
fn xr_needs_space_and_names_its_runtime() {
    assert_eq!(
        choose("winit", with(&[(ADAPTER, "space"), (XR, "openxr")])),
        Ok(Choice {
            space: true,
            xr: true,
            ..NATIVE
        })
    );
    assert_eq!(
        choose("winit", with(&[(XR, "openxr")])),
        Err("BEUI_XR is for BEUI_ADAPTER=space only".to_owned())
    );
    assert_eq!(
        choose("winit", with(&[(ADAPTER, "space"), (XR, "webxr")])),
        Err("BEUI_XR takes openxr, not webxr".to_owned())
    );
}
