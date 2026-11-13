use be_type_chat::Deletion;

use super::*;

#[test]
fn a_forged_deletion_of_a_closed_message_shows_where_it_was() {
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
    chat.run();
    let content = chat.content();
    let newest = content.segments().last().expect("a segment").last;

    // Someone deletes a closed message in Rosa's name.
    let deletion = content.deletion(newest, ROSA, MORNING + 500 * MINUTE);
    let deletion = Deletion {
        signature: Signature(be_sdk_test::signature(
            sam,
            &deletion.signed_bytes(CHAT, newest),
        )),
        ..deletion
    };
    chat.arrive(content.put_deletion(newest, &deletion));

    let recent = chat.content().messages()[0];
    chat.editor
        .scroll(format!("chat.message.{recent}"), vec2(0.0, -1_000_000.0));
    chat.editor.advance(Duration::from_millis(100));
    chat.run();
    assert!(
        chat.row(newest).contains("forged Message deleted"),
        "the closed message's row stays, marked forged: {}",
        chat.row(newest)
    );
}
