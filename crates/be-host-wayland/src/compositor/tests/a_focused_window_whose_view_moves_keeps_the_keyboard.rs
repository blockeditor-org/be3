use super::*;

fn keyboard(harness: &mut Harness) -> Option<WindowId> {
    harness.compositor().server().state.keyboard_window()
}

#[test]
fn a_focused_window_whose_view_moves_keeps_the_keyboard() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (_window, id) = harness.open();
    let (_other, other) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    assert_eq!(keyboard(&mut harness), Some(id));

    harness.move_views();
    assert_eq!(
        keyboard(&mut harness),
        Some(id),
        "a view built again elsewhere, as when a window leaves fullscreen, takes the focus its old one had"
    );
    harness.client.received.keys.clear();
    harness.frame([f_key(true), f_key(false)].concat());
    harness.settle();
    assert_eq!(
        harness.client.received.keys,
        [(KEY_F, true), (KEY_F, false)],
        "and the window still gets what is typed"
    );

    harness.click(&format!("wayland.window.{}", other.0));
    harness.click("test.input");
    assert_eq!(keyboard(&mut harness), None);
    harness.move_views();
    assert_eq!(
        keyboard(&mut harness),
        None,
        "a window that was not focused when its view moved does not take the focus"
    );
}
