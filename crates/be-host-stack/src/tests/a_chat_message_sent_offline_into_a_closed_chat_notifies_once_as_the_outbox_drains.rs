use super::*;

#[test]
fn a_chat_message_sent_offline_into_a_closed_chat_notifies_once_as_the_outbox_drains() {
    let mut harness = Harness::pushing();
    let room = ChatRoom::start(&harness, &[]);
    let address = harness.address();

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    {
        let mut editor = room.editor();
        // A burst sent offline is one notification once it drains, not one each.
        for body in ["Running late", "Ten minutes", "Order for me"] {
            room.send(&mut editor, body, None);
        }
        editor.until("took the messages", |editor| {
            editor.content().messages().len() == 4
        });
    }
    flush();
    wait_until("kept the messages in the outbox", |shared| {
        shared.waiting > 0
    });

    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and drained the outbox", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    room.told_away("3 new messages");
    room.told_nothing_more(&mut room.editor());
}
