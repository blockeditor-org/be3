use crate::{ChatOp, OFFLINE_AFTER};

use super::*;

/// `edit` as the session owner sequences it, stamped with when the server took it.
fn stamped_at(edit: Edit, received: i64) -> ChatOp {
    let mut operation = ChatOp::from(edit);
    *ChatContent::arrival(&mut operation).expect("an edit keeps its arrival") = Some(received);
    operation
}

/// A message keeps its operation's stamp (`LiveEdit::arrival`), into a closed segment too; one
/// sent unstamped shows no arrival, never its claimed time; and a member cannot write one,
/// by sending a message that holds it, by setting it after, or by sending again under a
/// message's id.
#[test]
fn a_message_keeps_the_arrival_the_owner_stamped_and_no_member_writes_one() {
    let mut chat = ChatContent::default();
    let (offline, send) = chat.send(CHAT, Message::new(ADA, 1_000, "written offline"));
    chat.apply(&stamped_at(send, 1_000 + OFFLINE_AFTER));
    let (online, send) = chat.send(CHAT, Message::new(BO, 2_000, "written online"));
    chat.apply(&stamped_at(send, 2_500));
    let (own, send) = chat.send(CHAT, Message::new(ADA, 3_000, "the owner's own"));
    chat.apply(&send.into());

    let message = |chat: &ChatContent, id| chat.message(id).expect("sent");
    assert_eq!(message(&chat, offline).arrival, Some(1_000 + OFFLINE_AFTER));
    assert_eq!(
        message(&chat, offline).arrived_late(),
        Some(1_000 + OFFLINE_AFTER)
    );
    assert_eq!(message(&chat, online).arrival, Some(2_500));
    assert_eq!(
        message(&chat, online).arrived_late(),
        None,
        "it came at once"
    );
    assert_eq!(
        message(&chat, own).arrival,
        None,
        "the owner's own is unknown"
    );
    assert_eq!(message(&chat, own).arrived_late(), None);
    assert_eq!(
        message(&round_trip(&chat), offline).arrival,
        Some(1_000 + OFFLINE_AFTER),
        "the stamp is saved with the message"
    );

    // A member sends a message claiming an arrival, and sets another's.
    let (forged, send) = chat.send(
        CHAT,
        Message {
            arrival: Some(1),
            ..Message::new(BO, 4_000_000, "claims it came long after")
        },
    );
    chat.apply(&send.into());
    chat.apply(&Edit::from(Message::ARRIVAL.set(online, &Some(9_999_999))).into());
    chat.apply(&Edit::from(Message::ARRIVAL.set(own, &Some(9_999_999))).into());
    assert_eq!(message(&chat, forged).arrival, None);
    assert_eq!(message(&chat, online).arrival, Some(2_500));
    assert_eq!(message(&chat, own).arrival, None);

    // Or sends again under an id the chat holds, which inserts nothing, stamped or not.
    let again = |id| ChatContent::send_as(id, &Message::new(BO, 0, "again"));
    chat.apply(&stamped_at(again(own), 8_000_000));
    chat.apply(&again(online).into());
    assert_eq!(
        message(&chat, own).arrival,
        None,
        "a stamp landed on another message"
    );
    assert_eq!(
        message(&chat, online).arrival,
        Some(2_500),
        "a stamp was erased"
    );
    assert_eq!(message(&chat, online).body, "written online");

    // Two operations the server took at different times never fold into one.
    let (_, first) = chat.send(CHAT, Message::new(ADA, 5_000, "one"));
    let (_, second) = chat.send(CHAT, Message::new(ADA, 5_001, "two"));
    let mut folded = stamped_at(first.clone(), 6_000);
    assert!(
        ChatContent::absorb_operation(&mut folded, stamped_at(second.clone(), 7_000)).is_some(),
        "folding would give the second message the first's stamp"
    );
    let mut unstamped = ChatOp::from(first);
    assert!(ChatContent::absorb_operation(&mut unstamped, second.into()).is_none());

    // Closing a segment keeps each message's stamp in it.
    let body = "a line of chat ".repeat(70);
    let mut long = ChatContent::default();
    let (first, send) = long.send(CHAT, Message::new(ADA, 0, body.as_str()));
    long.apply(&stamped_at(send, OFFLINE_AFTER));
    let long = sent(&long, BO, &vec![body.as_str(); 119]);
    let ChatOp::Close(closing) = long.closing().expect("a chat this long closes") else {
        panic!("a close");
    };
    let mut long = long;
    long.apply(&ChatOp::Close(closing.clone()));
    let read = long
        .read_segment(0, &closing.bytes)
        .expect("the segment reads back");
    assert_eq!(read[0].0, first);
    assert_eq!(read[0].1.arrival, Some(OFFLINE_AFTER));
}
