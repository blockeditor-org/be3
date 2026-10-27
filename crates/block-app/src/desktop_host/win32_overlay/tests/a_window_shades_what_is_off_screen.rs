use super::*;

#[test]
fn a_window_shades_what_is_off_screen() {
    painting(
        "a_windows_window_shades_what_is_off_screen",
        false,
        vec![part(0.0, 0.6, 1.0, 1.0), part(0.75, 0.0, 1.0, 0.6)],
    );
}
