use super::*;
use crate::{Gap, Segment};

fn removed(chat: &ChatContent, message: ObjectId) -> ChatContent {
    edited(
        chat,
        [[be_block_model::Change::remove(message)]
            .into_iter()
            .collect()],
    )
}

/// A message removed outright, not deleted, is missing before the first later message that
/// names it: as the last one its author saw, or as its author's own message before it, which
/// tells even when another's message came between. A deletion is no gap.
#[test]
fn a_message_removed_outright_is_missing_before_the_next_that_names_it() {
    let chat = sent(&ChatContent::default(), ADA, &["one", "two", "three"]);
    let [two, three] = [chat.messages()[1], chat.messages()[2]];
    assert_eq!(chat.gaps(), [], "nothing is missing");
    assert_eq!(
        removed(&chat, two).gaps(),
        [gap(three, two, &[])],
        "the next message names it"
    );
    assert_eq!(
        edited(&chat, [chat.delete(two, ADA, 9)]).gaps(),
        [],
        "a deletion is no gap"
    );

    // Bo's message came between Ada's two, so Ada's second saw Bo's, and names her own first
    // only as her own.
    let ada = sent(&ChatContent::default(), ADA, &["mine"]);
    let mine = ada.messages()[0];
    let bo = ChatContent::send_as(
        ObjectId::new(),
        &Message::new(BO, 1, "his, written before hers arrived"),
    );
    let both = edited(&ada, [bo]);
    let his = both.messages()[1];
    let next = sent(&both, ADA, &["mine again"]);
    let again = next.messages()[2];
    let sent_again = next.message(again).expect("the message was sent");
    assert_eq!(sent_again.seen.map(|seen| seen.id), Some(his));
    assert_eq!(sent_again.own_previous.map(|own| own.id), Some(mine));
    assert_eq!(removed(&next, mine).gaps(), [gap(again, mine, &[])]);

    // The newest recent message names the last one that closed: the segment's filter says it
    // may be there, and only a read of the segment tells.
    let closed = closed(&long(120));
    let first_recent = closed.messages()[0];
    let last_closed = closed
        .message(first_recent)
        .and_then(|message| message.seen)
        .expect("the first recent message saw the one before it")
        .id;
    assert_eq!(closed.gaps(), [gap(first_recent, last_closed, &[0])]);

    // A filter too full to tell excuses nothing: one made by `Segment::of` sets about half.
    let segment = closed.segments()[0].clone();
    assert!(!segment.over_full());
    let full = Segment {
        members: vec![0xff; segment.members.len()],
        ..segment
    };
    assert!(full.over_full() && full.may_hold(ObjectId::new()));
}

fn gap(naming: ObjectId, gone: ObjectId, closed_in: &[usize]) -> Gap {
    Gap {
        naming,
        gone,
        closed_in: closed_in.to_vec(),
    }
}
