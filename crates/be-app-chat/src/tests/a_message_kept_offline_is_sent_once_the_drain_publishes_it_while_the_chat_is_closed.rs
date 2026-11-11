use super::*;

#[test]
fn a_message_kept_offline_is_sent_once_the_drain_publishes_it_while_the_chat_is_closed() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    chat.editor.store().set_online(false);
    chat.write("Running late");
    chat.click("chat.send");
    let late = chat.id(1);
    chat.editor.advance(Duration::from_secs(40));
    assert!(chat.row(late).contains("Sending"));

    // Another chat is open while the device reconnects and the outbox drains, so no replica
    // of this one hears it.
    chat.editor.store().close_replica(None);
    chat.editor.store().set_online(true);
    let again = chat.reopened();
    assert!(
        !again.row(late).contains("Sending"),
        "a chat opened after its commit drained shows it sent"
    );
}
