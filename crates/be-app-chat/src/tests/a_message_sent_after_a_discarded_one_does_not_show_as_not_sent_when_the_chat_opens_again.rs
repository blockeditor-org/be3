use super::*;

#[test]
fn a_message_sent_after_a_discarded_one_does_not_show_as_not_sent_when_the_chat_opens_again() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    chat.editor.store().set_online(false);
    chat.write("I think so");
    chat.click("chat.send");
    let refused = chat.id(1);
    chat.editor.advance(Duration::from_secs(40));
    chat.editor.store().refuse_kept();
    chat.run();
    assert!(chat.row(refused).contains("Not sent"));
    chat.click(&format!("chat.message.{refused}.discard"));
    chat.editor.advance(Duration::from_millis(500));

    // The refused commit is still on this device. A message sent after it is not refused.
    chat.editor.store().set_online(false);
    chat.write("Back again");
    chat.click("chat.send");
    let later = chat.id(1);
    chat.editor.advance(Duration::from_secs(40));
    assert!(chat.row(later).contains("Sending"));

    chat.editor.store().close_replica(None);
    let again = chat.reopened();
    let row = again.row(again.id(1));
    assert!(
        !row.contains("Not sent"),
        "nothing refused the later message, though the refused commit is still held: {row}"
    );
}
