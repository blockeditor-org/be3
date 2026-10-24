use super::*;
use block_editor_beui::beui::{Input, Pasted};

fn keys(sent: Vec<TerminalMessage>) -> Vec<(Key, bool, String)> {
    sent.into_iter()
        .filter_map(|message| match message {
            TerminalMessage::Key(key) => Some((key.key, key.modifiers.ctrl, key.text)),
            _ => None,
        })
        .collect()
}

#[test]
fn keys_and_text_reach_the_host_as_input() {
    let mut harness = Harness::new(true);
    harness.show(&[prompt()]);
    harness.connect_host();
    assert!(resizes(&harness.sent()) >= 1, "it tells the host its size");

    harness.editor.click("terminal.screen");
    harness.editor.run();
    harness.editor.type_text("ls");
    harness.editor.run();
    harness.editor.key(Key::Enter, Modifiers::NONE);
    harness.editor.run();
    harness.editor.key(Key::C, ctrl());
    harness.editor.run();
    harness.editor.key(Key::V, ctrl());
    harness.editor.run();
    assert_eq!(
        keys(harness.sent()),
        [
            (Key::Unidentified, false, "l".to_owned()),
            (Key::Unidentified, false, "s".to_owned()),
            (Key::Enter, false, String::new()),
            (Key::C, true, String::new()),
            (Key::V, true, String::new()),
        ],
        "text, keys and Ctrl+C and Ctrl+V go to the shell"
    );

    harness.editor.key(Key::C, ctrl_shift());
    harness.editor.run();
    assert!(
        harness.sent().is_empty(),
        "Ctrl+Shift+C is the terminal's copy"
    );
    harness
        .editor
        .step(vec![Input::Paste(Pasted::Text("echo pasted".to_owned()))]);
    harness.editor.run();
    assert_eq!(
        harness.sent(),
        [TerminalMessage::Paste("echo pasted".to_owned())],
        "a paste reaches the shell as one paste"
    );

    harness.editor.set_size(Vec2::new(560.0, 300.0));
    harness.editor.run();
    assert_eq!(
        resizes(&harness.sent()),
        1,
        "a smaller view resizes the shell"
    );

    let stranger = TerminalHost {
        machine: "elsewhere".to_owned(),
        user: "bob".to_owned(),
    };
    harness.editor.set_peers(
        None,
        vec![PeerPresence {
            client: HOST + 1,
            kind: TerminalHost::ID,
            value: serde_json::to_vec(&stranger).unwrap(),
        }],
    );
    harness.editor.run();
    harness.editor.type_text("x");
    harness.editor.run();
    assert!(
        harness
            .editor
            .take_sent_to_peers::<TerminalMessage>()
            .is_empty(),
        "only the host the document names is sent input"
    );

    let mut viewer = Harness::new(false);
    viewer.show(&[prompt()]);
    viewer.connect_host();
    viewer.editor.click("terminal.screen");
    viewer.editor.run();
    viewer.editor.type_text("rm");
    viewer.editor.run();
    assert!(
        viewer.sent().is_empty(),
        "someone who may only read the block sends the host nothing"
    );
    assert_eq!(viewer.shown("terminal.status"), "\"View only\"");
}
