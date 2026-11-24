use super::*;

/// A message is deleted by its author's deletion alone: anyone else's changes nothing. Its
/// entry redacted after shows as the message deleted, with who signed the deletion.
#[test]
fn only_a_messages_author_deletes_it_and_its_redacted_entry_shows_deleted() {
    let delete = ChatEntry::Delete {
        message: 0,
        time: 5,
    };
    let entries = [item(0, ADA, &message("hello")), item(1, BO, &delete)];
    let log = ChatLog::of(&entries);
    assert_eq!(bodies(&log), [Some("hello".to_owned())]);

    let mut redacted = item(0, ADA, &message("hello"));
    redacted.payload = None;
    redacted.signer = Signer::Redacted;
    let entries = [
        redacted,
        signed(1, ADA, &delete, Signer::Replaced),
        item(2, BO, &message("still here")),
    ];
    let log = ChatLog::of(&entries);
    let deleted = log.message(0).unwrap();
    assert_eq!(deleted.author, ADA);
    assert_eq!(deleted.deleted, Some((5, Signer::Replaced)));
    assert_eq!(bodies(&log), [Some("still here".to_owned())]);
}
