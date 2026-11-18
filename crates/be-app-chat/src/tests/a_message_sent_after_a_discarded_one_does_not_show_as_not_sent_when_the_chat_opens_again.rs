use super::*;

/// What the device keeps of a refused message it let go of brings no failure back: a message
/// sent after it, still waiting, shows as sending when the chat opens again.
#[test]
fn a_message_sent_after_a_discarded_one_does_not_show_as_not_sent_when_the_chat_opens_again() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    let store = chat.editor.store();
    store.refuse_appends(Some("you may no longer write in this chat"));
    chat.send("I think so");
    assert!(chat.row("refused.0").contains("Not sent"));
    chat.click("chat.message.refused.0.discard");

    // Access back, the next one waits offline.
    store.refuse_appends(None);
    store.set_online(false);
    chat.send("Back again");
    assert!(chat.row(chat.kept(0)).contains("Sending"));

    store.close_replica(None);
    let again = chat.reopened();
    let row = again.row(again.kept(0));
    assert!(
        row.contains("Sending") && row.contains("Back again"),
        "nothing refused the later message: {row}"
    );
    assert!(!again.editor.shown("chat.message.refused.0"));
}
