use super::*;
use crate::KeyAt;

use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn an_intercepting_action_names_its_chords_and_answers_keys_intercepted_for_it() {
    let _root = crate::reactive::RootScope::new();
    let runs = create_signal(0);
    let open = create_signal(false);
    let document = build_node(move || {
        Action::new("test.switch", "Switch", move || {
            runs.update(|runs| *runs += 1)
        })
        .shortcut(Chord::key(Key::Tab).alt())
        .shortcut(Chord::key(Key::Tab))
        .at(KeyAt::Window)
        .capture()
        .register();
        let open = create_memo(move || open.get());
        Action::new("test.cancel", "Cancel", || {})
            .shortcut(Chord::key(Key::Escape).alt())
            .enabled(open)
            .at(KeyAt::Window)
            .capture()
            .register();
        Action::new("test.global", "Global", || {})
            .shortcut(Chord::ctrl(Key::G))
            .at(KeyAt::Window)
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
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .intercepted_keys,
        [tab],
        "only enabled intercepting actions name their chords, and never a chord that types"
    );
    open.set(true);
    assert_eq!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .intercepted_keys,
        [tab, escape],
        "a chord is named once its action is enabled"
    );

    let field = harness.find("field");
    harness.click(harness.center(field));
    let focused = harness.document().focused_node();
    let press = |pressed| Input::InterceptedKey {
        key: Key::Tab,
        pressed,
        repeat: false,
    };
    harness.frame(vec![
        Input::Modifiers(Modifiers::ALT),
        press(true),
        press(false),
        Input::Modifiers(Modifiers::NONE),
    ]);
    assert_eq!(
        runs.get_untracked(),
        1,
        "a key intercepted for the document runs its action"
    );
    assert_eq!(
        harness.document().focused_node(),
        focused,
        "the key never moves the focus from the control that has it"
    );
    assert_eq!(harness.document().api::<TextInputApi>(field).value(), "");
}
