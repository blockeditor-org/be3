use super::*;

#[test]
fn clipboard_keys_reach_the_app_except_the_paste_with_shift() {
    let ctrl = Modifiers::CTRL;
    let shifted = Modifiers {
        shift: true,
        ..Modifiers::CTRL
    };
    for (key, value) in [(Key::C, "c"), (Key::V, "v"), (Key::X, "x")] {
        assert!(
            !prevents_default(Some(key), value, ctrl, false),
            "{value}: the browser copies, pastes and cuts"
        );
        assert!(
            prevents_default(Some(key), value, ctrl, true),
            "{value}: a node that takes clipboard keys hears them as keys"
        );
    }
    assert!(prevents_default(Some(Key::A), "a", ctrl, false));
    assert!(prevents_default(Some(Key::A), "a", ctrl, true));
    assert!(
        !prevents_default(Some(Key::C), "C", shifted, false),
        "without the flag Ctrl+Shift+C is the browser's"
    );
    assert!(
        prevents_default(Some(Key::C), "C", shifted, true),
        "with it Ctrl+Shift+C is a key, not the inspector"
    );
    assert!(
        !prevents_default(Some(Key::V), "V", shifted, true),
        "Ctrl+Shift+V still pastes through the browser"
    );
    assert!(!prevents_default(Some(Key::C), "c", Modifiers::NONE, true));
    assert!(prevents_default(
        Some(Key::Tab),
        "Tab",
        Modifiers::NONE,
        true
    ));
}
