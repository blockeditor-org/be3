use super::*;

#[test]
fn a_refused_message_tells_nobody_and_a_reply_mentions_its_author() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    chat.editor.store().set_online(false);
    chat.write("I think so");
    chat.click("chat.send");
    let refused = chat.id(1);
    chat.editor.advance(Duration::from_secs(40));
    chat.editor.store().refuse_kept();
    chat.run();
    assert!(chat.row(refused).contains("Not sent"));
    assert!(
        chat.editor.take_announced().is_empty(),
        "a message the server refused tells nobody"
    );

    chat.click(&format!("chat.message.{refused}.discard"));
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert!(
        chat.editor.take_announced().is_empty(),
        "and neither does discarding it"
    );

    let rosas = chat.id(0);
    chat.click(&format!("chat.message.{rosas}.reply"));
    chat.write("Yes, I can");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert_eq!(
        chat.editor.take_announced(),
        [(CHAT, "Yes, I can".to_owned(), vec![ROSA])],
        "a reply mentions the author of the message it answers"
    );
}
