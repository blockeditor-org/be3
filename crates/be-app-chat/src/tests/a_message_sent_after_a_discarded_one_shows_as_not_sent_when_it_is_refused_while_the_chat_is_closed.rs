use super::*;

#[test]
fn a_message_sent_after_a_discarded_one_shows_as_not_sent_when_it_is_refused_while_the_chat_is_closed()
 {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    chat.editor.store().set_online(false);
    chat.write("I think so");
    chat.click("chat.send");
    let refused = chat.id(1);
    chat.editor.advance(Duration::from_secs(40));
    chat.editor.store().refuse_kept();
    chat.run();
    chat.click(&format!("chat.message.{refused}.discard"));
    chat.editor.advance(Duration::from_millis(500));

    chat.editor.store().set_online(false);
    chat.write("Back again");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_secs(40));
    chat.editor.store().close_replica(None);
    chat.editor.store().refuse_kept();

    let again = chat.reopened();
    let row = again.row(again.id(1));
    assert!(
        row.contains("Not sent"),
        "the later message was refused while the chat was closed: {row}"
    );
}
