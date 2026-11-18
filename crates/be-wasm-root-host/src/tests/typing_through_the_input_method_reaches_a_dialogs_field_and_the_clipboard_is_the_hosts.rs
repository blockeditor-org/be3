use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{ClipItem, Key, Modifiers, clipboard::formats};

use super::*;

const CTRL: Modifiers = Modifiers {
    ctrl: true,
    ..Modifiers::NONE
};

#[test]
fn typing_through_the_input_method_reaches_a_dialogs_field_and_the_clipboard_is_the_hosts() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    open_dialog(&mut root, &mut time);

    // The platform's input method composes and commits into the focused field.
    let typed: Vec<_> = [
        ImeEvent::Enabled,
        ImeEvent::SetComposingText("Mar".to_owned()),
        ImeEvent::SetComposingText(String::new()),
        ImeEvent::CommitText("Mar".to_owned()),
    ]
    .into_iter()
    .map(|event| Input::Ime(event).into())
    .collect();
    root.input(None, be_lib_ui_types::input::encode(&typed))
        .expect("the root takes input");

    // Ctrl+V asks the host for what its clipboard holds, which it answers in a later frame.
    let pastes = (0..8)
        .find_map(|step_at| {
            if step_at == 2 {
                root.input(
                    None,
                    be_lib_ui_types::input::encode(&chord(Key::V, CTRL)),
                )
                .expect("the root takes input");
            }
            time += 1;
            step(&mut root, time);
            let pastes = root.take_pastes();
            (!pastes.is_empty()).then_some(pastes)
        })
        .expect("Ctrl+V in the field asks the host to paste");
    let [(id, formats)] = &pastes[..] else {
        panic!("one paste is asked for: {pastes:?}");
    };
    assert!(
        formats.iter().any(|format| format == formats::TEXT),
        "the field takes plain text: {formats:?}"
    );
    root.answer_paste(*id, Some(be_wasm_ops::encode(&ClipItem::text("ch"))));

    // Ctrl+A and Ctrl+C copy the field's text to the host's clipboard: what was typed, and
    // what was pasted after it.
    let mut copy = chord(Key::A, CTRL);
    copy.extend(chord(Key::C, CTRL));
    let copied = (0..8)
        .find_map(|step_at| {
            if step_at == 2 {
                root.input(None, be_lib_ui_types::input::encode(&copy))
                    .expect("the root takes input");
            }
            time += 1;
            step(&mut root, time);
            root.take_copies().pop()
        })
        .expect("Ctrl+C in the field copies to the host's clipboard");
    let text = copied
        .iter()
        .filter_map(|item| be_wasm_ops::decode::<ClipItem>(item))
        .find_map(|item| item.as_text())
        .expect("the copy holds plain text");
    assert_eq!(
        text, "March",
        "the field holds what the input method committed and what the host pasted"
    );
    assert!(
        root.take_pastes().is_empty(),
        "nothing else asked the host to paste"
    );
}
