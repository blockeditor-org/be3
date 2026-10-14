use super::*;
use crate::reactive::{build, create_signal, view};
use crate::styled::TextInput;
use beui_core::app::automation::keys::typed;

#[test]
fn a_driver_types_key_by_key_and_sends_what_no_key_makes_as_text() {
    let mut events = Vec::new();
    typed("aB!é", Modifiers::default(), &mut events);
    let pressed: Vec<(Key, bool, Option<u32>, Option<String>)> = events
        .iter()
        .filter_map(|event| match event {
            Event::Key { press, code, text } if press.pressed => {
                Some((press.key, press.modifiers.shift, *code, text.clone()))
            }
            _ => None,
        })
        .collect();
    let code = |code: u32| cfg!(target_os = "linux").then_some(code);
    assert_eq!(
        pressed,
        vec![
            (Key::A, false, code(30), Some("a".to_owned())),
            (Key::Shift, true, code(42), None),
            (Key::B, true, code(48), Some("B".to_owned())),
            (Key::Shift, true, code(42), None),
            (Key::One, true, code(2), Some("!".to_owned())),
        ],
        "each character a key makes is that key pressed, with shift held where it needs it, so a \
         Wayland program's window, which reads keys rather than text, gets it too"
    );
    assert_eq!(
        events.last(),
        Some(&Event::Text("é".to_owned())),
        "a character no key makes arrives as text"
    );

    let document = build(|| {
        let (name, set_name) = create_signal(String::new());
        view! {
            <TextInput
                @test_id={"form.name"}
                value={name}
                placeholder="Name"
                on_change={move |next: String| set_name.set(next)}
            />
        }
    });
    let mut driven = Driven::new(document);
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
