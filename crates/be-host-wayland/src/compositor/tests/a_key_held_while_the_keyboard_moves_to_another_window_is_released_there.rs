use super::*;

const KEY_LEFTCTRL: u32 = 29;
const CONTROL_MASK: u32 = 4;

fn control(pressed: bool) -> Vec<Input> {
    let held = match pressed {
        true => beui::Modifiers::CTRL,
        false => beui::Modifiers::NONE,
    };
    vec![
        stroke(Some(Key::Ctrl), KEY_LEFTCTRL, pressed),
        Input::Modifiers(held),
    ]
}

#[test]
fn a_key_held_while_the_keyboard_moves_to_another_window_is_released_there() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_first, first) = harness.open();
    let (_second, second) = harness.open();
    harness.click(&format!("wayland.window.{}", first.0));
    harness.client.received.keyboard.clear();

    harness.frame(control(true));
    harness.click(&format!("wayland.window.{}", second.0));
    assert_eq!(
        harness.compositor().server().state.keyboard_window(),
        Some(second)
    );
    harness.frame(control(false));
    harness.settle();
    assert_eq!(
        harness.client.received.keyboard,
        [
            Heard::Key(KEY_LEFTCTRL, true),
            Heard::Leave,
            Heard::Enter(vec![KEY_LEFTCTRL]),
            Heard::Key(KEY_LEFTCTRL, false),
        ],
        "the window the click moved the keyboard to entered with Ctrl held, so it hears Ctrl \
         let go, though the window that took the press had the keyboard then"
    );
    assert_eq!(harness.client.received.depressed & CONTROL_MASK, 0);
    assert!(
        harness
            .compositor()
            .server()
            .state
            .keyboard()
            .pressed_keys()
            .is_empty()
    );
}
