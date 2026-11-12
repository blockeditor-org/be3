use be_sdk_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn an_edited_message_tells_nobody_again_and_a_sent_again_one_tells_once() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    chat.write("Running late");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert_eq!(chat.editor.take_announced().len(), 1);

    // Editing a message the server holds is not a new message.
    let late = chat.id(1);
    chat.click(&format!("chat.message.{late}.edit"));
    chat.click("chat.draft");
    chat.editor.key(Key::End, Modifiers::NONE);
    chat.type_text(" again");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_secs(5));
    chat.run();
    assert!(
        chat.editor.take_announced().is_empty(),
        "an edit tells nobody again"
    );

    // A message the server refused and the person sent again tells once, as the new message.
    chat.editor.store().set_online(false);
    chat.write("Almost there");
    chat.click("chat.send");
    let refused = chat.id(2);
    chat.editor.advance(Duration::from_secs(40));
    chat.editor.store().refuse_kept();
    chat.run();
    assert!(chat.row(refused).contains("Not sent"));
    assert!(chat.editor.take_announced().is_empty());
    chat.editor.store().set_online(true);
    chat.click(&format!("chat.message.{refused}.retry"));
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert_eq!(
        chat.editor.take_announced(),
        [(CHAT, "Almost there".to_owned(), Vec::new())],
        "the old message tells nobody and the new one tells once"
    );
}
