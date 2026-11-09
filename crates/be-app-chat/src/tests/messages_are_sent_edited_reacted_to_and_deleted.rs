use be_sdk_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn messages_are_sent_edited_reacted_to_and_deleted() {
    let mut chat = Harness::new(&[(ROSA, "Is the build green?")]);
    let question = chat.id(0);

    chat.send("Not yet");
    chat.send("Almost thre");
    assert_eq!(
        chat.bodies(),
        ["Is the build green?", "Not yet", "Almost thre"]
    );
    let (not_yet, typo) = (chat.id(1), chat.id(2));
    let sent = chat.content().message(typo).expect("the message was sent");
    assert_eq!(sent.author, ME);
    assert_eq!(
        sent.seen,
        chat.content().hash(CHAT, not_yet),
        "a message notes the last one its author saw"
    );

    chat.click(&format!("chat.message.{typo}.edit"));
    assert_eq!(chat.editor.label("chat.draft"), "Almost thre");
    chat.click("chat.draft");
    chat.editor.key(Key::End, Modifiers::NONE);
    chat.type_text("e there");
    chat.click("chat.send");
    assert_eq!(
        chat.bodies(),
        ["Is the build green?", "Not yet", "Almost three there"]
    );
    assert!(chat.row(typo).contains("edited"));

    chat.click(&format!("chat.message.{question}.react"));
    chat.click(&format!("chat.message.{question}.react.thumbs-up"));
    assert_eq!(
        chat.content().reactions(question),
        [("thumbs-up".to_owned(), vec![ME])]
    );
    chat.click(&format!("chat.message.{question}.reaction.thumbs-up"));
    assert!(
        chat.content().reactions(question).is_empty(),
        "pressing my reaction again takes it back"
    );

    chat.click(&format!("chat.message.{not_yet}.delete"));
    assert_eq!(chat.bodies(), ["Is the build green?", "Almost three there"]);
    assert!(chat.content().deletion_of(not_yet).is_some());
    assert!(
        !chat
            .editor
            .shown(&format!("chat.message.{question}.delete")),
        "only my own messages can be deleted"
    );
}
