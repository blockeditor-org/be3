use super::*;

#[test]
fn an_unknown_layer_is_refused() {
    assert_eq!(
        choose("winit", with(&[(LAYERS, "space,vr")])),
        Err(
            "BEUI_LAYERS takes a comma-separated list of space, xr or drive, bottom first, not vr"
                .to_owned()
        )
    );
    assert_eq!(
        choose("winit", with(&[(PLATFORM, "space")])),
        Err(
            "BEUI_PLATFORM takes winit or headless, not space; space is a layer: BEUI_LAYERS=space"
                .to_owned()
        )
    );
}
