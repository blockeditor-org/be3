use be_sdk_beui::beui::{Key, Modifiers};

use super::*;

/// Each is an entry of the chat's log: a message, its author's edit, a reaction kept or taken
/// back (which redacts the record that kept it), and a deletion (which redacts the message).
#[test]
fn messages_are_sent_edited_reacted_to_and_deleted() {
    let mut chat = Harness::new(&[(ROSA, "Is the build green?")]);
    let question = chat.seq(0);

    chat.send("Not yet");
    chat.send("Almost thre");
    assert_eq!(
        chat.bodies(),
        ["Is the build green?", "Not yet", "Almost thre"]
    );
    let (not_yet, typo) = (chat.seq(1), chat.seq(2));
    assert_eq!(chat.log().message(typo).unwrap().author, ME);

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
        chat.log().message(question).unwrap().reactions,
        [("thumbs-up".to_owned(), vec![ME])]
    );
    let (kept_at, _) = chat.log().reaction(question, ME, "thumbs-up").unwrap();
    chat.click(&format!("chat.message.{question}.reaction.thumbs-up"));
    assert!(
        chat.log().message(question).unwrap().reactions.is_empty(),
        "pressing my reaction again takes it back"
    );
    assert_eq!(
        chat.editor.store().log(None)[kept_at as usize].payload,
        None,
        "and the record that kept it is redacted"
    );

    chat.click(&format!("chat.message.{not_yet}.delete"));
    assert_eq!(chat.bodies(), ["Is the build green?", "Almost three there"]);
    assert_eq!(
        chat.editor.store().log(None)[not_yet as usize].payload,
        None,
        "a deleted message's entry is redacted"
    );
    assert!(!chat.editor.shown(&format!("chat.message.{not_yet}")));
    assert!(
        !chat
            .editor
            .shown(&format!("chat.message.{question}.delete")),
        "only my own messages can be deleted"
    );
}
