use super::*;

#[test]
fn xr_is_a_layer_of_its_own() {
    assert_eq!(
        choose("winit", with(&[(LAYERS, "xr")])),
        Ok(Choice {
            layers: vec![Stacked::Xr],
            ..NATIVE
        })
    );
    assert_eq!(
        choose("winit", with(&[(PLATFORM, "xr")])),
        Err(
            "BEUI_PLATFORM takes winit or headless, not xr; xr is a layer: BEUI_LAYERS=xr"
                .to_owned()
        )
    );
}
