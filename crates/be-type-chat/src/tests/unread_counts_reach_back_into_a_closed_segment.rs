use super::*;

#[test]
fn unread_counts_reach_back_into_a_closed_segment() {
    let open = long(120);
    let place = open.messages()[0];
    let chat = closed(&open);
    assert!(
        chat.message(place).is_none(),
        "the place is in a closed segment"
    );

    let recent = chat.messages().len();
    assert_eq!(
        chat.unread([place], BO),
        recent,
        "someone who read only as far as a closed message has every recent one still to read"
    );
    let stranger = ObjectId::new();
    assert_eq!(
        chat.unread([stranger], BO),
        0,
        "a place this replica knows nothing of is one it has not received yet: all read"
    );
    assert!(
        chat.reads_further(chat.newest().expect("messages"), Some(place)),
        "the newest message is further than a place in a segment"
    );
}
