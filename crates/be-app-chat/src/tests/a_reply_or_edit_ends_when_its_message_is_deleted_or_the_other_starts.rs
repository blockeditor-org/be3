use super::*;

#[test]
fn a_reply_or_edit_ends_when_its_message_is_deleted_or_the_other_starts() {
    let mut chat = Harness::new(&[(ROSA, "Anyone seen my charger?"), (ME, "Which one")]);
    let (charger, which) = (chat.id(0), chat.id(1));

    chat.click(&format!("chat.message.{which}.edit"));
    assert_eq!(chat.editor.label("chat.draft"), "Which one");
    chat.click(&format!("chat.message.{charger}.reply"));
    assert!(
        !chat.editor.label("chat.draft").contains("Which one"),
        "replying drops the edit and its text"
    );
    assert!(!chat.editor.shown("chat.edit.cancel"));
    assert!(chat.editor.shown("chat.reply.cancel"));

    chat.arrive(deleted_by(&chat.content(), charger, ROSA, MORNING + MINUTE));
    assert!(
        !chat.editor.shown("chat.reply.cancel"),
        "a reply ends when its message is deleted elsewhere"
    );
    chat.send("Found it?");
    assert_eq!(
        chat.content()
            .message(chat.id(1))
            .and_then(|message| message.reply_to),
        None
    );
}
