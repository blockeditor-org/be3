use super::*;
use crate::KeyPress;
use crate::reactive::{
    Action, Chord, build, create_memo, create_signal, view, with_reactive_scope,
};
use crate::styled::{TextInput, text_input_value};

#[test]
fn an_intercepting_action_names_its_chords_and_answers_keys_intercepted_for_it() {
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let (open, set_open) = create_signal(false);
    let document = build(move || {
        Action::new("test.switch", "Switch", move || {
            counted.set(counted.get() + 1)
        })
        .shortcut(Chord::key(Key::Tab).alt())
        .shortcut(Chord::key(Key::Tab))
        .intercepts()
        .register();
        let open = create_memo(move || open.get());
        Action::new("test.cancel", "Cancel", || {})
            .shortcut(Chord::key(Key::Escape).alt())
            .enabled(open)
            .intercepts()
            .register();
        Action::new("test.global", "Global", || {})
            .shortcut(Chord::ctrl(Key::G))
            .global()
            .register();
        view! {
            <List spacing=0.0>
                <TextInput @test_id={"field"} value="" />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let tab = Chord::key(Key::Tab).alt().key_chord();
    let escape = Chord::key(Key::Escape).alt().key_chord();

    assert_eq!(
        harness.frame(Vec::new()).intercepted_keys,
        [tab],
        "only enabled intercepting actions name their chords, and never a chord that types"
    );
    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    assert_eq!(
        harness.frame(Vec::new()).intercepted_keys,
        [tab, escape],
        "a chord is named once its action is enabled"
    );

    let field = harness.find("field");
    harness.click(harness.center(field));
    let focused = harness.document().focused_node();
    let press = |pressed| {
        Event::InterceptedKey(KeyPress {
            key: Key::Tab,
            pressed,
            repeat: false,
            modifiers: Modifiers::ALT,
        })
    };
    harness.frame(vec![press(true), press(false)]);
    assert_eq!(
        runs.get(),
        1,
        "a key intercepted for the document runs its action"
    );
    assert_eq!(
        harness.document().focused_node(),
        focused,
        "the key never moves the focus from the control that has it"
    );
    assert_eq!(text_input_value(harness.document(), field), "");
}
