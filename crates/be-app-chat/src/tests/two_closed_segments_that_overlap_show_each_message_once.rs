use std::collections::HashSet;

use be_block_commit::MergeResult;
use be_sdk_beui::be_block_content::Merge;

use super::*;

/// The messages of `order` that show: they run down the page in time order, none twice.
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
    // One device closed the oldest messages twice while apart, another once, and had deleted the
    // third first: the closes differ and overlap, and the merged index keeps all three entries,
    // the one of the second device after the first's two unless they are put in time order.
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
    other.apply(&ChatOp::from(deleted_by(&base, deleted, ROSA, 9)));
    let theirs = close(&other);
    for (a, b) in [(&ours, &theirs), (&theirs, &ours)] {
        let MergeResult::Clean(merged) = ChatContent::merge3(&base, a, b) else {
            panic!("a chat merge never conflicts");
        };
        assert_eq!(merged.segments().len(), 3, "all three segments stay");
        every_message_shows_once(&base, merged, deleted);
    }
}

fn every_message_shows_once(base: &ChatContent, merged: ChatContent, deleted: ObjectId) {
    let order: Vec<ObjectId> = base
        .messages()
        .into_iter()
        .filter(|id| *id != deleted)
        .collect();

    let mut chat = Harness::holding_parts(merged);
    // Tall enough that every row that is read is laid out, so nothing is hidden by the viewport.
    chat.editor.set_size(vec2(480.0, 60_000.0));
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    let recent = chat.content().messages()[0];

    // The segments are read one at a time, the entry nearest the recent messages first, and held
    // back between reads (the editor asks for one segment's reads at a time, so each step lets
    // one segment in). After each one what shows is a run of the messages in time order, none
    // missing between them, on to the newest message.
    chat.editor.store().hold_reads(true);
    chat.editor
        .scroll(format!("chat.message.{recent}"), vec2(0.0, -1_000_000.0));
    chat.editor.advance(Duration::from_millis(100));
    let mut seen: HashSet<ObjectId> = HashSet::new();
    for _ in 0..8 {
        // One segment: the reads it asked for go through, then the view settles.
        let mut released = false;
        while chat.editor.store().release_read() {
            released = true;
        }
        chat.run();
        if !released {
            match chat.editor.shown("chat.older") {
                true => {
                    chat.click("chat.older");
                    continue;
                }
                false => break,
            }
        }
        let shown = shown_in_order(&chat, &order);
        let first = order
            .iter()
            .position(|id| *id == shown[0])
            .expect("a shown message");
        assert_eq!(
            shown,
            order[first..first + shown.len()],
            "what shows has no gap in it"
        );
        assert_eq!(
            shown.last(),
            order.last(),
            "and it runs on to the newest message"
        );
        seen.extend(shown);
    }
    assert!(!chat.editor.shown("chat.older"), "every segment was read");
    assert_eq!(
        seen,
        order.iter().copied().collect::<HashSet<_>>(),
        "every message that was not deleted showed"
    );
}
