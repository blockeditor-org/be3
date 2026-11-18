use super::*;

#[test]
fn the_focus_is_the_layer_a_press_landed_on() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    assert_eq!(windows.focused(None), None, "nothing is focused before a press");
    windows.route(None, vec![button(150.0, true), button(150.0, false)]);
    assert_eq!(
        windows.focused(None).map(|layer| layer.id),
        Some(SCREEN),
        "a press on the app's screen focuses it"
    );
    assert_eq!(windows.focused(Some(7)), None, "another window has no focus");
}
