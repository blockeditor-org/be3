use super::*;

fn super_chord(harness: &mut Harness, code: u32, key: Key) {
    let chord = |pressed: bool| vec![stroke(Some(key), code, pressed)];
    harness.frame([super_key(true), chord(true), chord(false), super_key(false)].concat());
    harness.settle();
}

#[test]
fn an_app_that_inhibits_shortcuts_gets_super_chords_until_super_escape() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    let _inhibitor = harness.client.inhibit_shortcuts(&window.surface);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.inhibitor_active,
        Some(true)
    );

    super_chord(&mut harness, KEY_F, Key::F);
    assert!(!harness.client.received.fullscreen);
    assert_eq!(
        harness.client.received.keys,
        [
            (KEY_LEFTMETA, true),
            (KEY_F, true),
            (KEY_F, false),
            (KEY_LEFTMETA, false)
        ],
        "Super+F goes to the window that inhibits shortcuts"
    );

    harness.client.received.keys.clear();
    super_chord(&mut harness, KEY_ESC, Key::Escape);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_LEFTMETA, true), (KEY_LEFTMETA, false)],
        "Super+Escape always breaks out, and never reaches the window"
    );
    assert_eq!(
        harness.client.received.protocols.inhibitor_active,
        Some(false)
    );

    super_chord(&mut harness, KEY_F, Key::F);
    assert!(
        harness.client.received.fullscreen,
        "with its shortcuts back, the desktop's Super+F answers again"
    );
}
