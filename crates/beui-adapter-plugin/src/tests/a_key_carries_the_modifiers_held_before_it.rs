use super::*;
use block_editor_plugin::{Key, Modifiers};

#[test]
fn a_key_carries_the_modifiers_held_before_it() {
    let context = beui::context();
    let whole = region(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 30.0)),
        [40, 30],
    );
    let mut input = Input::default();
    input.translate(
        &context,
        &whole,
        &InputEvent::Modifiers(Modifiers {
            control: true,
            ..Modifiers::default()
        }),
    );
    let events = input.translate(
        &context,
        &whole,
        &InputEvent::Key {
            key: Key::C,
            pressed: true,
            repeat: false,
        },
    );
    assert!(
        matches!(
            events.as_slice(),
            [Event::Key {
                modifiers: beui_core::input::Modifiers { ctrl: true, .. },
                pressed: true,
                ..
            }]
        ),
        "{events:?}"
    );
}
