use super::*;

#[test]
fn a_long_chat_closes_its_oldest_messages_into_a_segment_and_reads_them_back_on_request() {
    // About 150 KiB of messages: past what stays recent, so the oldest close into segments.
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
    let segments = content.segments();
    assert!(
        !segments.is_empty(),
        "the editor closed the oldest messages into segments"
    );
    let closed: u64 = segments.iter().map(|segment| segment.count).sum();
    assert_eq!(content.messages().len() as u64 + closed, 150);
    let newest = segments.last().expect("a segment").last;
    assert!(
        !chat.editor.shown(&format!("chat.message.{newest}")),
        "the closed messages are not read until the person goes back to them"
    );

    // Scrolled to the start of the chat, the newest closed segment is read; the button reads
    // each older one.
    let recent = content.messages()[0];
    let to_start = |chat: &mut Harness| {
        chat.editor
            .scroll(format!("chat.message.{recent}"), vec2(0.0, -1_000_000.0));
        chat.editor.advance(Duration::from_millis(100));
        chat.run();
    };
    to_start(&mut chat);
    assert!(
        chat.editor.shown(&format!("chat.message.{newest}")),
        "going back to the start of the chat reads the newest closed segment"
    );
    assert!(
        chat.row(newest).contains(": a line of chat"),
        "and shows its messages: {}",
        chat.row(newest)
    );
    chat.snapshot("a_long_chat_shows_the_closed_messages_it_read_and_offers_older_ones");
    while chat.editor.shown("chat.older") {
        chat.click("chat.older");
        to_start(&mut chat);
    }
    assert!(
        chat.row(segments[0].first).contains("0: a line of chat"),
        "the oldest message shows once every segment is read: {}",
        chat.row(segments[0].first)
    );
}
