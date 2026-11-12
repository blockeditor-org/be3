use super::*;

#[test]
fn a_message_in_a_closed_segment_shows_an_edit_made_since_it_closed() {
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
    let segments = chat.content().segments();
    let newest = segments.last().expect("a segment").last;

    let recent = chat.content().messages()[0];
    chat.editor
        .scroll(format!("chat.message.{recent}"), vec2(0.0, -1_000_000.0));
    chat.editor.advance(Duration::from_millis(100));
    chat.run();
    assert!(chat.row(newest).contains(": a line of chat"));

    let edit = chat
        .content()
        .edit(newest, ROSA, MORNING + 1, "said again, better");
    chat.arrive(edit);
    assert!(
        chat.row(newest).contains("said again, better"),
        "an edit made to a closed message shows: {}",
        chat.row(newest)
    );
}
