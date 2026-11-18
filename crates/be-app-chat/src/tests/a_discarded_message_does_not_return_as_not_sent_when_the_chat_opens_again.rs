use super::*;

/// A message kept offline that the server refuses as the device drains shows as not sent at
/// once; discarded, it stays gone when the chat opens again.
#[test]
fn a_discarded_message_does_not_return_as_not_sent_when_the_chat_opens_again() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    let store = chat.editor.store();
    store.set_online(false);
    chat.send("I think so");
    let kept = chat.kept(0);
    assert!(chat.row(&kept).contains("Sending"));

    store.refuse_appends(Some("you may no longer write in this chat"));
    store.set_online(true);
    chat.run();
    assert!(
        !chat.editor.shown(&format!("chat.message.{kept}")),
        "it is no longer sending"
    );
    assert!(chat.row("refused.0").contains("Not sent"));

    chat.click("chat.message.refused.0.discard");
    assert!(!chat.editor.shown("chat.message.refused.0"));
    assert!(store.refused_appends(None).is_empty());

    store.close_replica(None);
    let again = chat.reopened();
    assert_eq!(
        again.bodies(),
        ["Can you still see this chat?"],
        "the discarded message stays gone"
    );
    assert!(!again.editor.shown("chat.message.refused.0"));
}
