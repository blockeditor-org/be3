use super::*;

#[test]
fn a_space_platform_outside_space_is_refused() {
    let refused = Err("BEUI_SPACE_PLATFORM is for BEUI_PLATFORM=space only".to_owned());
    assert_eq!(
        choose("winit", with(&[(SPACE_PLATFORM, "headless")])),
        refused
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(PLATFORM, "headless"), (SPACE_PLATFORM, "headless")])
        ),
        refused
    );
}
