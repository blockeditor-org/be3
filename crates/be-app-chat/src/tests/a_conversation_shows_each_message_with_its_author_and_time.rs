use super::*;

#[test]
fn a_conversation_shows_each_message_with_its_author_and_time() {
    let mut chat = Harness::new(&[
        (ROSA, "Morning! Is the release still on for today?"),
        (ME, "It is. I'm writing the notes now."),
        (ROSA, "Great, I'll check the installers once they're up."),
    ]);
    let (first, second) = (chat.id(0), chat.id(1));

    assert_eq!(
        chat.row(first),
        "Member 7e1a 09:00 Morning! Is the release still on for today?"
    );
    assert_eq!(chat.row(second), "You 09:03 It is. I'm writing the notes now.");
    chat.snapshot("a_conversation_shows_each_message_with_its_author_and_time");
}
