use super::*;

#[test]
fn a_message_kept_offline_tells_the_other_members_even_when_the_chat_is_closed_as_it_drains() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    chat.editor.store().set_online(false);
    chat.write("Running late");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_secs(40));
    chat.run();
    assert!(chat.editor.take_announced().is_empty());

    // The chat is closed while the device reconnects and its outbox drains: the replica, not an
    // editor, knows the server holds the message when the chat opens again.
    chat.editor.store().close_replica(None);
    chat.editor.store().set_online(true);
    let mut again = chat.reopened();
    again.run();
    assert_eq!(
        again.editor.take_announced(),
        [(CHAT, "Running late".to_owned(), Vec::new())],
        "the message told the other members once, though no editor saw it leave sending"
    );
}
