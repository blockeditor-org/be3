use super::*;

#[test]
fn a_draft_stays_in_the_editor_until_it_is_sent() {
    let mut chat = Harness::new(&[(ROSA, "Lunch?")]);
    let before = chat.content();

    chat.write("Sure, at noon");
    assert_eq!(chat.content(), before, "a draft is not in the block");

    chat.arrive(
        chat.content()
            .send(CHAT, Message::new(ROSA, MORNING + MINUTE, "Or one?"))
            .1,
    );
    assert_eq!(
        chat.editor.label("chat.draft"),
        "Sure, at noon",
        "a message arriving leaves the draft alone"
    );

    chat.click("chat.send");
    assert_eq!(chat.bodies(), ["Lunch?", "Or one?", "Sure, at noon"]);
    assert!(
        !chat.editor.label("chat.draft").contains("Sure"),
        "sending empties the draft"
    );
}
