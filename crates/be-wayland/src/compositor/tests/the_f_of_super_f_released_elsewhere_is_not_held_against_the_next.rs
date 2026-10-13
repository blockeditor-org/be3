use super::*;

#[test]
fn the_f_of_super_f_released_elsewhere_is_not_held_against_the_next() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    let shown = format!("wayland.window.{}", id.0);
    window.toplevel.as_ref().unwrap().set_fullscreen(None);
    harness.acknowledge(&window);
    harness.click(&shown);

    harness.frame([super_key(true), f_key(true, beui::Modifiers::LOGO)].concat());
    harness.frame(super_key(false));
    harness.settle();
    assert!(!harness.client.received.fullscreen);
    harness.click("test.input");
    harness.frame(f_key(false, beui::Modifiers::NONE));
    harness.settle();

    harness.click(&shown);
    harness.client.received.keys.clear();
    harness.key(KEY_F);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_F, true), (KEY_F, false)],
        "a plain F after the shortcut reaches the window whole"
    );
}
