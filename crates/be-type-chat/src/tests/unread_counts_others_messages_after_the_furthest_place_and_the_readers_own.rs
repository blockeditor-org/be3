use super::*;

/// What is unread is the messages others sent after the furthest place read and after the
/// reader's own last message: answering is reading. Edits, reactions and deleted messages count
/// for nothing.
#[test]
fn unread_counts_others_messages_after_the_furthest_place_and_the_readers_own() {
    let entries = [
        item(0, BO, &message("one")),
        item(1, BO, &message("two")),
        item(
            2,
            BO,
            &ChatEntry::Edit {
                message: 1,
                time: 0,
                body: "two!".to_owned(),
            },
        ),
        item(3, BO, &message("three")),
        item(4, BO, &ChatEntry::Delete { message: 3, time: 0 }),
        item(5, BO, &message("four")),
    ];
    let log = ChatLog::of(&entries);
    assert_eq!(log.unread(None, ADA), 3);
    assert_eq!(log.unread(Some(0), ADA), 2);
    assert_eq!(log.unread(Some(5), ADA), 0);

    let mut answered = entries.to_vec();
    answered.push(item(6, ADA, &message("hi")));
    answered.push(item(7, BO, &message("five")));
    assert_eq!(ChatLog::of(&answered).unread(Some(0), ADA), 1);
}
