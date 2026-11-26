use super::*;

#[test]
fn a_sent_message_tells_the_chats_other_members_once_the_server_holds_it() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    assert!(
        chat.editor.take_announced().is_empty(),
        "opening a chat tells nobody"
    );

    chat.send("Running late");
    assert_eq!(
        chat.editor.take_announced(),
        [(CHAT, "Running late".to_owned(), Vec::new())],
        "the message told the other members once the server held it"
    );

    let late = chat.seq(1);
    chat.click(&format!("chat.message.{late}.edit"));
    chat.type_text(", sorry");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_secs(5));
    chat.run();
    assert!(
        chat.editor.take_announced().is_empty(),
        "an edit tells nobody"
    );

    let rosas = chat.seq(0);
    chat.click(&format!("chat.message.{rosas}.reply"));
    chat.send("Yes, I can");
    assert_eq!(
        chat.editor.take_announced(),
        [(CHAT, "Yes, I can".to_owned(), vec![ROSA])],
        "a reply mentions the author of the message it answers"
    );
}
