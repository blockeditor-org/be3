use std::collections::HashSet;

use be_block_commit::MergeResult;
use be_sdk_beui::be_block::Merge;

use super::*;

/// The messages of `order` that show, each with where: they must run down the page in time order
/// and none may show twice.
fn shown_in_order(chat: &Harness, order: &[ObjectId]) -> Vec<ObjectId> {
    let shown: Vec<ObjectId> = order
        .iter()
        .copied()
        .filter(|id| chat.editor.shown(&format!("chat.message.{id}")))
        .collect();
    let tops: Vec<f32> = shown
        .iter()
        .map(|id| chat.editor.rect_of(&format!("chat.message.{id}")).min.y)
        .collect();
    assert!(
        tops.windows(2).all(|pair| pair[0] < pair[1]),
        "the messages that show run down the page in time order"
    );
    let unique: HashSet<ObjectId> = shown.iter().copied().collect();
    assert_eq!(unique.len(), shown.len(), "no message shows twice");
    shown
}

#[test]
fn two_closed_segments_that_overlap_show_each_message_once() {
    // Two devices each closed the oldest messages twice while apart; one had deleted the third
    // first, so the closes differ and overlap, and the merged index keeps all four entries.
    let line = "a line of chat ".repeat(70);
    let mut base = ChatContent::default();
    for index in 0..260 {
        let (_, send) = base.send(CHAT, Message::new(ROSA, index, format!("{index}: {line}")));
        base.apply(&send.into());
    }
    let close = |chat: &ChatContent| {
        let mut chat = chat.clone();
        let closing = chat.closing().expect("a chat this long closes");
        chat.apply(&closing);
        chat
    };
    let ours = close(&close(&base));
    let deleted = base.messages()[2];
    let mut other = base.clone();
    other.apply(&ChatOp::Edit(base.delete(deleted, ROSA, 9)));
    let theirs = close(&close(&other));
    let MergeResult::Clean(merged) = ChatContent::merge3(&base, &ours, &theirs) else {
        panic!("a chat merge never conflicts");
    };
    let segments = merged.segments();
    assert_eq!(segments.len(), 4, "all four segments stay");
    let order = base.messages();

    let mut chat = Harness::holding_parts(merged);
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    let recent = chat.content().messages()[0];

    // Scrolled back to the start, the segments are read one at a time, newest first, and after
    // each read what shows is in order, none twice, whichever segments are still unread.
    let mut seen = HashSet::new();
    for _ in 0..8 {
        chat.editor
            .scroll(format!("chat.message.{recent}"), vec2(0.0, -1_000_000.0));
        chat.editor.advance(Duration::from_millis(100));
        chat.run();
        seen.extend(shown_in_order(&chat, &order));
        if !chat.editor.shown("chat.older") {
            break;
        }
        chat.click("chat.older");
        seen.extend(shown_in_order(&chat, &order));
    }
    assert!(!chat.editor.shown("chat.older"), "every segment was read");

    // At the very top, the oldest message shows, and the one only the later closes hold shows
    // too (above the recent ones), each once.
    chat.editor
        .scroll(format!("chat.message.{recent}"), vec2(0.0, -1_000_000.0));
    chat.editor.advance(Duration::from_millis(100));
    chat.run();
    assert!(
        shown_in_order(&chat, &order).contains(&order[0]),
        "the oldest message shows"
    );
}
