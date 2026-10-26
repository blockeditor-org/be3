use super::*;

#[test]
fn a_space_adapter_outside_space_is_refused() {
    let refused = Err("BEUI_SPACE_ADAPTER is for BEUI_ADAPTER=space only".to_owned());
    assert_eq!(
        choose("winit", with(&[(SPACE_ADAPTER, "headless")])),
        refused
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(ADAPTER, "headless"), (SPACE_ADAPTER, "headless")])
        ),
        refused
    );
}
