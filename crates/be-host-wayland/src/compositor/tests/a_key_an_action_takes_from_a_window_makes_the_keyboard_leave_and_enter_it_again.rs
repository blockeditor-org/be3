use super::*;

fn alt(pressed: bool) -> Vec<Input> {
    let held = match pressed {
        true => beui::Modifiers::ALT,
        false => beui::Modifiers::NONE,
    };
    vec![
        stroke(Some(Key::Alt), KEY_LEFTALT, pressed),
        Input::Modifiers(held),
    ]
}

fn tab(pressed: bool) -> Vec<Input> {
    vec![stroke(Some(Key::Tab), KEY_TAB, pressed)]
}

#[test]
fn a_key_an_action_takes_from_a_window_makes_the_keyboard_leave_and_enter_it_again() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (_window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    harness.client.received.keyboard.clear();

    harness.frame(alt(true));
    harness.frame([tab(true), tab(false)].concat());
    harness.frame(alt(false));
    harness.settle();
    assert_eq!(SWITCHED.with(std::cell::Cell::get), 1);
    assert_eq!(
        harness.client.received.keyboard,
        [
            Heard::Key(KEY_LEFTALT, true),
            Heard::Leave,
            Heard::Enter(vec![KEY_LEFTALT]),
            Heard::Key(KEY_LEFTALT, false),
        ],
        "the window never hears Tab, and the keyboard leaving between Alt's press and release \
         keeps it from taking Alt for a tap (Firefox shows its menu bar for one)"
    );

    harness.client.received.keyboard.clear();
    harness.frame([alt(true), alt(false)].concat());
    harness.settle();
    assert_eq!(
        harness.client.received.keyboard,
        [
            Heard::Key(KEY_LEFTALT, true),
            Heard::Key(KEY_LEFTALT, false)
        ],
        "Alt on its own reaches the window whole"
    );
}
