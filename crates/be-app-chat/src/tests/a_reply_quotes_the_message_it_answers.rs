use super::*;

#[test]
fn a_reply_quotes_the_message_it_answers() {
    let mut chat = Harness::new(&[
        (ROSA, "Who has the keys to the lab?"),
        (ROSA, "Also, coffee at three?"),
    ]);
    let keys = chat.id(0);

    chat.click(&format!("chat.message.{keys}.reply"));
    assert!(chat.editor.shown("chat.reply.cancel"));
    chat.send("I do, I'll leave them at the desk");

    let reply = chat.id(2);
    assert_eq!(
        chat.content()
            .message(reply)
            .and_then(|message| message.reply_to),
        Some(keys)
    );
    assert!(
        chat.row(reply)
            .contains("Member 7e1a: Who has the keys to the lab?")
    );
    assert!(
        !chat.editor.shown("chat.reply.cancel"),
        "sending ends the reply"
    );
    chat.snapshot("a_reply_quotes_the_message_it_answers");
}
