use be_block_commit::MergeResult;
use be_sdk_beui::be_block::Merge;

use super::*;

#[test]
fn two_closed_segments_that_overlap_show_each_message_once() {
    // Two devices closed the oldest messages while apart; one had deleted the fourth first, so
    // the closes differ but open at the same message.
    let line = "a line of chat ".repeat(70);
    let mut base = ChatContent::default();
    for index in 0..120 {
        let (_, send) = base.send(CHAT, Message::new(ROSA, index, format!("{index}: {line}")));
        base.apply(&send.into());
    }
    let close = |chat: &ChatContent| {
        let mut chat = chat.clone();
        let closing = chat.closing().expect("a chat this long closes");
        chat.apply(&closing);
        chat
    };
    let ours = close(&base);
    let deleted = base.messages()[3];
    let mut other = base.clone();
    other.apply(&ChatOp::Edit(base.delete(deleted, ROSA, 9)));
    let theirs = close(&other);
    let MergeResult::Clean(merged) = ChatContent::merge3(&base, &ours, &theirs) else {
        panic!("a chat merge never conflicts");
    };
    let segments = merged.segments();
    assert_eq!(segments.len(), 2, "both segments stay");
    assert_eq!(
        segments[0].first, segments[1].first,
        "they open at the same message"
    );

    // The older messages are read one segment at a time, each told apart by its whole entry:
    // reading the newest does not mark the other, which opens at the same message, as read.
    let read_first: Vec<(be_type_chat::Segment, ())> = vec![(segments[1].clone(), ())];
    assert_eq!(
        crate::root::ui::unread_segment(&segments, &read_first),
        Some(0),
        "the segment that opens at the same message is still to read"
    );
    let read_both: Vec<(be_type_chat::Segment, ())> = segments
        .iter()
        .cloned()
        .map(|segment| (segment, ()))
        .collect();
    assert_eq!(crate::root::ui::unread_segment(&segments, &read_both), None);

    // What was read shows each message once, in order, whichever segment held it.
    let message = |id: ObjectId| (id, Message::new(ROSA, 0, ""));
    let order = base.messages();
    let of = |segment: &be_type_chat::Segment| -> Vec<(ObjectId, Message)> {
        let from = order
            .iter()
            .position(|id| *id == segment.first)
            .unwrap_or(0);
        let to = order.iter().position(|id| *id == segment.last).unwrap_or(0);
        order[from..=to].iter().copied().map(message).collect()
    };
    let (a, b) = (of(&segments[0]), of(&segments[1]));
    let woven: Vec<ObjectId> = ChatContent::weave(&[&a, &b])
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let mut once = woven.clone();
    once.dedup();
    assert_eq!(woven.len(), once.len(), "no message twice");
    for segment in &segments {
        assert!(
            woven.contains(&segment.first) && woven.contains(&segment.last),
            "every message either segment holds shows"
        );
    }
}
