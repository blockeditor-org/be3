use super::*;

#[test]
fn a_chat_message_still_sending_shows_as_sending_after_a_restart() {
    let mut harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[]);

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    let mut editor = room.editor();
    room.send(&mut editor, "Running late", None);
    editor.until("shows the message as sending", |editor| {
        !editor.kept().is_empty()
    });
    assert_eq!(editor.kept(), ["Running late"]);
    drop(editor);
    stop();

    // The host and its type modules start again, still without the server: a read of the log
    // tells of what waits in the outbox.
    room.restart(&harness);
    let mut editor = room.editor();
    assert_eq!(
        editor.kept(),
        ["Running late"],
        "a message the server never got still shows as sending after a restart"
    );

    // Back online, the outbox drains and the message is sent.
    harness.start_server();
    retry_now();
    wait_until("reconnected and drained the outbox", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    editor.until("shows nothing as sending", |editor| {
        editor.kept().is_empty() && editor.messages().contains(&"Running late".to_owned())
    });
}
