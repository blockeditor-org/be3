use super::*;

#[test]
fn a_chat_message_unsent_across_a_restart_notifies_once_after_reconnecting() {
    let mut harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[]);
    let address = harness.address();

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    let mut editor = room.editor();
    room.send(&mut editor, "Running late", None);
    editor.until("took the message", |editor| {
        editor.content().messages().len() == 2
    });
    drop(editor);
    stop();

    room.restart(&harness);
    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and drained the outbox", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    room.told_away("Running late");
    room.told_nothing_more(&mut room.editor());
}
