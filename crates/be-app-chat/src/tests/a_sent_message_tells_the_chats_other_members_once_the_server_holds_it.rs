use super::*;

#[test]
fn a_sent_message_tells_the_chats_other_members_once_the_server_holds_it() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    assert!(
        chat.editor.take_announced().is_empty(),
        "opening a chat tells nobody"
    );

    chat.write("Running late");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert_eq!(
        chat.editor.take_announced(),
        [(CHAT, "Running late".to_owned(), Vec::new())],
        "the message told the other members once the server held it"
    );

    chat.editor.advance(Duration::from_secs(5));
    chat.run();
    assert!(chat.editor.take_announced().is_empty(), "and only once");
}
