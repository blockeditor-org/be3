use super::*;

#[test]
fn a_segment_closed_twice_or_over_messages_that_changed_closes_once() {
    let chat = long(120);
    let closing = chat.closing().expect("a chat this long closes a segment");

    // Two editors propose the same close: the second finds the messages already gone.
    let mut once = chat.clone();
    once.apply(&closing);
    let after_one = once.clone();
    once.apply(&closing);
    assert_eq!(once.segments().len(), 1, "the second close changes nothing");
    assert_eq!(once.messages(), after_one.messages());

    // A close made over a message another replica deleted meanwhile is ignored whole.
    let mut deleted = chat.clone();
    let gone = chat.messages()[0];
    deleted.apply(&chat.delete(gone, ADA, 9).into());
    deleted.apply(&closing);
    assert!(
        deleted.segments().is_empty(),
        "the segment holds a message that is gone here, so it does not close"
    );
    assert!(
        deleted.closing().is_some(),
        "the oldest messages that are left close instead"
    );
}
