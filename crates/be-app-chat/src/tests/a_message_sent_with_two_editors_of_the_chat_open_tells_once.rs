use super::*;

#[test]
fn a_message_sent_with_two_editors_of_the_chat_open_tells_once() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    let mut beside = chat.reopened();
    chat.write("Running late");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    beside.run();
    assert_eq!(
        chat.editor.take_announced(),
        [(CHAT, "Running late".to_owned(), Vec::new())],
        "the replica both editors follow told the other members once"
    );
    beside.editor.advance(Duration::from_secs(5));
    beside.run();
    assert!(beside.editor.take_announced().is_empty());
}
