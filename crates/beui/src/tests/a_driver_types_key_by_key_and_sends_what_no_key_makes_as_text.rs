use super::*;
use crate::reactive::*;
use crate::styled::*;
use beui_core::input::typing::typed;

#[test]
fn a_driver_types_key_by_key_and_sends_what_no_key_makes_as_text() {
    let mut events = Vec::new();
    typed("aB!é", Modifiers::default(), &mut events);
    let pressed: Vec<(Key, bool, Option<u32>, Option<String>)> =
        strokes(&events, Modifiers::default())
            .into_iter()
            .filter(|stroke| stroke.1)
            .map(|(key, _, shift, evdev, text)| (key, shift, evdev, text))
            .collect();
    assert_eq!(
        pressed,
        vec![
            (Key::A, false, Some(30), Some("a".to_owned())),
            (Key::Shift, true, Some(42), None),
            (Key::B, true, Some(48), Some("B".to_owned())),
            (Key::Shift, true, Some(42), None),
            (Key::One, true, Some(2), Some("!".to_owned())),
        ],
        "each character a key makes is that key pressed, with shift held where it needs it, so a \
         Wayland program's window, which reads keys rather than text, gets it too"
    );
    assert_eq!(
        events.last(),
        Some(&Input::Text("é".to_owned())),
        "a character no key makes arrives as text"
    );

    let mut held = Vec::new();
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };
    typed("a", shift, &mut held);
    let shifts: Vec<(Key, bool, bool)> = strokes(&held, shift)
        .into_iter()
        .map(|(key, pressed, shift, _, _)| (key, pressed, shift))
        .collect();
    assert_eq!(
        shifts,
        vec![
            (Key::Shift, false, false),
            (Key::A, true, false),
            (Key::A, false, false),
            (Key::Shift, true, true),
        ],
        "a held Shift is let go for a character typed without it, and held again after"
    );

    let document = build_node(|| {
        let name = create_signal(String::new());
        view! {
            <TextInput @test_id={"form.name"} value={name} placeholder="Name" />
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));
    driven
        .ask(&["click", "#form.name"])
        .expect("the field is clicked");
    let changed = driven
        .ask(&["type", "Hi, Ada! (é) {x}"])
        .expect("the text is typed");
    assert!(
        changed.contains("value=\"Hi, Ada! (é) {x}\""),
        "the field reads what was typed, keys and text alike:\n{changed}"
    );
}

type Stroke = (Key, bool, bool, Option<u32>, Option<String>);

fn strokes(events: &[Input], held: Modifiers) -> Vec<Stroke> {
    let mut shift = held.shift;
    events
        .iter()
        .filter_map(|event| match event {
            Input::Modifiers(modifiers) => {
                shift = modifiers.shift;
                None
            }
            Input::Key(Keystroke {
                key: Some(key),
                pressed,
                evdev,
                text,
                ..
            }) => {
                if *key == Key::Shift {
                    shift = *pressed;
                }
                Some((*key, *pressed, shift, *evdev, text.clone()))
            }
            _ => None,
        })
        .collect()
}
