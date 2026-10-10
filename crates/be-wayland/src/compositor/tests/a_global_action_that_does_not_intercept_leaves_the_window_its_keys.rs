use super::*;

fn super_g(harness: &mut Harness) {
    let logo = beui::Modifiers::LOGO;
    let g = |pressed: bool| {
        vec![
            physical(KEY_G, pressed),
            Event::Key {
                key: Key::G,
                pressed,
                repeat: false,
                modifiers: logo,
            },
        ]
    };
    harness.frame([super_key(true), g(true), g(false), super_key(false)].concat());
    harness.settle();
}

#[test]
fn a_global_action_that_does_not_intercept_leaves_the_window_its_keys() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (_window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    harness.client.received.keys.clear();

    super_g(&mut harness);
    assert_eq!(
        harness.client.received.keys,
        [
            (KEY_LEFTMETA, true),
            (KEY_G, true),
            (KEY_G, false),
            (KEY_LEFTMETA, false)
        ],
        "the focused window gets the chord"
    );
    assert_eq!(GLOBAL_RAN.with(std::cell::Cell::get), 0);

    harness.click("test.input");
    super_g(&mut harness);
    assert_eq!(
        GLOBAL_RAN.with(std::cell::Cell::get),
        1,
        "with beui focused, the global action answers"
    );
}
