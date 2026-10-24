use super::*;

#[test]
fn keys_and_text_reach_the_host_as_input() {
    let mut harness = Harness::new(true);
    harness.connect_host();
    harness.snapshot_of("$ ");
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

    let typed: Vec<u8> = harness
        .sent()
        .into_iter()
        .filter_map(|message| match message {
            TerminalMessage::Input(bytes) => Some(bytes),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(typed, b"ls\r\x03");

    let mut viewer = Harness::new(false);
    viewer.connect_host();
    viewer.snapshot_of("$ ");
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
