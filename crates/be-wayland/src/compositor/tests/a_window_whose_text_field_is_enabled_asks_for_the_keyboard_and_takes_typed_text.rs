use super::*;

fn asks(harness: &Harness) -> Option<u32> {
    let area = harness
        .output
        .as_ref()?
        .main()
        .window
        .requests
        .ime
        .clone()?;
    match area.keyboard {
        beui::Keyboard::Shown { asks } => Some(asks),
        beui::Keyboard::Held => None,
    }
}

#[test]
fn a_window_whose_text_field_is_enabled_asks_for_the_keyboard_and_takes_typed_text() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let text_input = harness.client.text_input();
    harness.click(&format!("wayland.window.{}", id.0));
    harness.settle();
    assert!(
        harness.client.received.protocols.text_entered,
        "the text input enters the window that has the keyboard"
    );
    assert_eq!(
        asks(&harness),
        None,
        "a window with no enabled field asks for nothing"
    );

    text_input.enable();
    text_input.commit();
    harness.settle();
    let first = asks(&harness).expect("an enabled field asks for the keyboard");
    text_input.enable();
    text_input.commit();
    harness.settle();
    assert!(
        asks(&harness).is_some_and(|again| again != first),
        "enabling it again asks again, as a tap on a field does"
    );

    harness.frame(vec![Input::Text("é".to_owned())]);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.text_committed,
        ["é"],
        "text no key types reaches the field as committed text"
    );

    text_input.disable();
    text_input.commit();
    harness.settle();
    assert_eq!(asks(&harness), None, "a disabled field asks for nothing");
}
