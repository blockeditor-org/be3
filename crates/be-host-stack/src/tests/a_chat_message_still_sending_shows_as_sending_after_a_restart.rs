use super::*;

/// The messages the replica says the server lacks, once the editor follows them.
fn sending(editor: &Editor<be_type_chat::ChatContent>) -> Vec<String> {
    let content = editor.content();
    editor
        .mirror
        .lacking()
        .iter()
        .filter_map(|lacking| lacking.operation.as_ref())
        .flat_map(be_type_chat::ChatContent::sent_in)
        .filter_map(|id| content.body(id))
        .collect()
}

#[test]
fn a_chat_message_still_sending_shows_as_sending_after_a_restart() {
    let mut harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[]);

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    let mut editor = room.editor();
    editor.send(be_sdk_type_guest::editor::FromEditor::FollowLacking);
    room.send(&mut editor, "Running late", None);
    editor.until("shows the message as sending", |editor| !sending(editor).is_empty());
    assert_eq!(sending(&editor), ["Running late"]);
    drop(editor);
    stop();

    // The host and its type modules start again, still without the server.
    room.restart(&harness);
    let mut editor = room.editor();
    editor.send(be_sdk_type_guest::editor::FromEditor::FollowLacking);
    editor.until("was told what is sending", |editor| {
        editor
            .received
            .iter()
            .any(|told| matches!(told, be_sdk_type_guest::editor::ToEditor::Lacking { .. }))
    });
    assert_eq!(
        sending(&editor),
        ["Running late"],
        "a message the server never got still shows as sending after a restart"
    );
}
