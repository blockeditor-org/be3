use super::*;

#[test]
fn keys_and_text_reach_the_host_as_input() {
    let mut harness = Harness::new(true);
    harness.show(&[prompt()]);
    harness.connect_host();
    harness.sent();

    harness.editor.click("terminal.screen");
    harness.editor.run();
    harness.editor.type_text("ls");
    harness.editor.run();
    harness.editor.key(Key::Enter, Modifiers::NONE);
    harness.editor.run();
    harness.editor.key(
        Key::C,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    harness.editor.run();

    let keys: Vec<_> = harness
        .sent()
        .into_iter()
        .filter_map(|message| match message {
            TerminalMessage::Key(key) => Some(key),
            _ => None,
        })
        .collect();
    let typed: String = keys
        .iter()
        .filter(|key| key.key == Key::Unidentified)
        .map(|key| key.text.as_str())
        .collect();
    assert_eq!(typed, "ls");
    let pressed: Vec<_> = keys
        .iter()
        .filter(|key| key.key != Key::Unidentified)
        .map(|key| (key.key, key.modifiers.ctrl))
        .collect();
    assert_eq!(pressed, vec![(Key::Enter, false), (Key::C, true)]);

    let mut viewer = Harness::new(false);
    viewer.show(&[prompt()]);
    viewer.connect_host();
    viewer.sent();
    viewer.editor.click("terminal.screen");
    viewer.editor.run();
    viewer.editor.type_text("rm");
    viewer.editor.run();
    assert!(
        viewer.sent().is_empty(),
        "someone who may only read the block cannot type into it"
    );
    assert_eq!(viewer.shown("terminal.status"), "\"View only\"");
}
