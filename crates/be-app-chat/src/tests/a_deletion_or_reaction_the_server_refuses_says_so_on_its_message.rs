use super::*;

/// A deletion or a reaction the server refuses does not look done: the message's row says what
/// was not, and why, until dismissed.
#[test]
fn a_deletion_or_reaction_the_server_refuses_says_so_on_its_message() {
    let mut chat = Harness::new(&[(ROSA, "Lunch at noon?"), (ME, "Sounds good")]);
    let (question, answer) = (chat.seq(0), chat.seq(1));
    chat.editor.store().refuse_appends(Some("no access"));

    chat.click(&format!("chat.message.{answer}.delete"));
    assert!(
        chat.row(answer).contains("Sounds good"),
        "the message is still there"
    );
    assert_eq!(
        chat.editor.label(&format!("chat.message.{answer}.undone")),
        "Not deleted: no access"
    );

    chat.click(&format!("chat.message.{question}.react"));
    chat.click(&format!("chat.message.{question}.react.thumbs-up"));
    assert_eq!(
        chat.editor
            .label(&format!("chat.message.{question}.undone")),
        "Reaction not saved: no access"
    );
    chat.snapshot("a_deletion_or_reaction_the_server_refuses_says_so_on_its_message");

    chat.click(&format!("chat.message.{answer}.dismiss"));
    assert!(!chat.editor.shown(&format!("chat.message.{answer}.undone")));
    assert!(
        chat.editor
            .shown(&format!("chat.message.{question}.undone"))
    );
    assert_eq!(chat.editor.store().refused_appends(None).len(), 1);

    // Another refused on the same message joins the one caption, and goes with it.
    chat.click(&format!("chat.message.{question}.react"));
    chat.click(&format!("chat.message.{question}.react.heart"));
    assert_eq!(
        chat.editor
            .label(&format!("chat.message.{question}.undone")),
        "Reaction not saved: no access; Reaction not saved: no access"
    );
    chat.click(&format!("chat.message.{question}.dismiss"));
    assert!(
        !chat
            .editor
            .shown(&format!("chat.message.{question}.undone")),
        "{:?}",
        chat.editor
            .label(&format!("chat.message.{question}.undone"))
    );
    assert!(chat.editor.store().refused_appends(None).is_empty());
}
