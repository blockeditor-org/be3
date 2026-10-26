use super::*;

#[test]
fn headless_sizes_outside_the_headless_platform_are_refused() {
    assert_eq!(
        choose("winit", with(&[(HEADLESS_WINDOW, "800x600")])),
        Err("BEUI_HEADLESS_WINDOW is for the headless platform only".to_owned())
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(PLATFORM, "space"), (HEADLESS_SCREEN, "800x600")])
        ),
        Err("BEUI_HEADLESS_SCREEN is for the headless platform only".to_owned())
    );
}
