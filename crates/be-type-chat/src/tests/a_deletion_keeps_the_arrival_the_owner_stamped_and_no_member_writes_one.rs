use crate::{ChatOp, Deletion};

use super::*;

/// A deletion keeps its operation's stamp (`Deletion::arrival`), as a message does, so one
/// signed with a replaced key can show it came after the key changed; one written unstamped
/// keeps none, and the arrival a member writes into a deletion never stays.
#[test]
fn a_deletion_keeps_the_arrival_the_owner_stamped_and_no_member_writes_one() {
    let mut chat = ChatContent::default();
    let (stamped, send) = chat.send(CHAT, Message::new(ADA, 1_000, "deleted live"));
    chat.apply(&send.into());
    let (unstamped, send) = chat.send(CHAT, Message::new(ADA, 2_000, "deleted unstamped"));
    chat.apply(&send.into());

    // The member claims an arrival in both deletions.
    let claiming = |chat: &ChatContent, message, time| Deletion {
        arrival: Some(1),
        ..chat.deletion(message, ADA, time)
    };
    let delete = claiming(&chat, stamped, 3_000);
    let mut operation = ChatOp::from(chat.put_deletion(stamped, &delete));
    *ChatContent::arrival(&mut operation).expect("an edit keeps its arrival") = Some(3_500);
    chat.apply(&operation);
    let delete = claiming(&chat, unstamped, 4_000);
    chat.apply(&chat.put_deletion(unstamped, &delete).into());

    let deletion = |chat: &ChatContent, id| chat.deletion_of(id).expect("deleted");
    assert_eq!(deletion(&chat, stamped).arrival, Some(3_500));
    assert_eq!(deletion(&chat, unstamped).arrival, None);
    assert_eq!(
        deletion(&round_trip(&chat), stamped).arrival,
        Some(3_500),
        "the stamp is saved with the deletion"
    );
    assert!(chat.message(stamped).is_none() && chat.message(unstamped).is_none());
}
