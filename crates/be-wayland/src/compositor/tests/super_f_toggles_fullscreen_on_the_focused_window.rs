use super::*;

fn super_f(harness: &mut Harness) {
    let logo = beui::Modifiers::LOGO;
    harness.frame(
        [
            super_key(true),
            f_key(true, logo),
            f_key(false, logo),
            super_key(false),
        ]
        .concat(),
    );
    harness.settle();
}

#[test]
fn super_f_toggles_fullscreen_on_the_focused_window() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));

    super_f(&mut harness);
    assert!(harness.client.received.fullscreen);
    assert_eq!(harness.window(id).size(), SCREEN);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_LEFTMETA, true), (KEY_LEFTMETA, false)],
        "the F that toggled it never reaches the window, though Super does"
    );

    harness.acknowledge(&window);
    super_f(&mut harness);
    assert!(!harness.client.received.fullscreen);
    assert_eq!(harness.window(id).size(), SHOWN);

    harness.client.received.keys.clear();
    harness.frame(
        [
            f_key(true, beui::Modifiers::NONE),
            f_key(false, beui::Modifiers::NONE),
        ]
        .concat(),
    );
    harness.settle();
    assert_eq!(
        harness.client.received.keys,
        [(KEY_F, true), (KEY_F, false)],
        "an F without Super is the window's"
    );
}
