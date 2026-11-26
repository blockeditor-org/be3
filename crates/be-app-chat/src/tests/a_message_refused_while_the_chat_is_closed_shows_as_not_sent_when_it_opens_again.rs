use super::*;

/// A message kept offline that the server refuses while the chat is closed shows as not sent
/// once the chat opens again: the device keeps what the server refused until it is dismissed.
#[test]
fn a_message_refused_while_the_chat_is_closed_shows_as_not_sent_when_it_opens_again() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    let store = chat.editor.store();
    store.set_online(false);
    chat.send("I think so");
    assert!(chat.row(chat.kept(0)).contains("Sending"));

    // Closed, the drain back online is refused.
    store.close_replica(None);
    store.refuse_appends(Some("you may no longer write in this chat"));
    store.set_online(true);
    assert_eq!(store.refused_appends(None).len(), 1);

    let mut again = chat.reopened();
    let row = again.row("refused.0");
    assert!(
        row.contains("Not sent") && row.contains("I think so"),
        "the message refused while the chat was closed: {row}"
    );
    assert_eq!(again.bodies(), ["Can you still see this chat?"]);
    again.click("chat.message.refused.0.discard");
    assert!(!again.editor.shown("chat.message.refused.0"));
    assert!(
        store.refused_appends(None).is_empty(),
        "discarding it lets the device let go of it too"
    );
}
