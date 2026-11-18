use super::*;

/// What a message says is its author's latest edit: an edit by anyone else changes nothing,
/// however late it comes.
#[test]
fn a_message_shows_its_authors_latest_edit_and_no_one_elses() {
    let edit = |body: &str| ChatEntry::Edit {
        message: 0,
        time: 0,
        body: body.to_owned(),
    };
    let entries = [
        item(0, ADA, &message("hello")),
        item(1, ADA, &edit("hello there")),
        item(2, BO, &edit("ada says nothing")),
    ];
    let log = ChatLog::of(&entries);
    let shown = log.message(0).unwrap();
    assert_eq!(shown.body.as_deref(), Some("hello there"));
    assert!(shown.edited);
}
