use super::*;

/// A deletion of a closed message, which keeps nothing of it since the segment still holds it,
/// is checked against the segment's copy once it is read: one someone else signed as themselves
/// shows as forged where the message was.
#[test]
fn a_closed_message_deleted_by_someone_else_shows_as_forged() {
    let sam = Uuid::from_u128(0x5a30_0000_0000_4000_8000_0000_0000_0005);
    let line = "a line of chat ".repeat(70);
    let messages: Vec<(Uuid, String)> = (0..150)
        .map(|index| (ROSA, format!("{index}: {line}")))
        .collect();
    let initial: Vec<(Uuid, &str)> = messages
        .iter()
        .map(|(author, body)| (*author, body.as_str()))
        .collect();
    let mut chat = Harness::new(&initial);
    chat.editor.advance(Duration::from_millis(500));
    chat.editor.run();
    chat.run();
    let content = chat.content();
    let newest = content.segments().last().expect("a segment").last;

    let deletion = deleted_by(&content, newest, sam, MORNING + 500 * MINUTE);
    chat.arrive(deletion);

    let recent = chat.content().messages()[0];
    chat.editor
        .scroll(format!("chat.message.{recent}"), vec2(0.0, -1_000_000.0));
    chat.editor.advance(Duration::from_millis(100));
    chat.editor.run();
    chat.run();
    assert!(
        chat.row(newest).contains("forged Message deleted"),
        "the closed message's row stays, marked forged: {}",
        chat.row(newest)
    );
}
