use super::*;

/// An entry redacted by someone else (a manager, the server) shows in its place as removed,
/// since its author named it nowhere; one its author took back (a deletion, a reaction taken
/// back) shows no such row, and a take-back naming another's entry counts for nothing.
#[test]
fn an_entry_redacted_by_anyone_but_its_author_shows_removed() {
    let redacted = |seq: u64, author: Uuid| {
        let mut item = item(seq, author, &message("gone"));
        item.payload = None;
        item.signer = LogSigner::Redacted;
        item
    };
    let react = |kept: bool, takes_back: Option<u64>| ChatEntry::React {
        message: 0,
        reaction: "heart".to_owned(),
        kept,
        takes_back,
        time: 0,
    };
    let entries = [
        item(0, ADA, &message("hello")),
        // Bo's message, redacted by a manager.
        redacted(1, BO),
        // Ada's reaction, taken back by Ada.
        redacted(2, ADA),
        item(3, ADA, &react(false, Some(2))),
        // Ada's message, which Bo's deletion and take-back name: neither counts.
        redacted(4, ADA),
        item(
            5,
            BO,
            &ChatEntry::Delete {
                message: 4,
                time: 0,
            },
        ),
        item(6, BO, &react(false, Some(4))),
    ];
    let log = ChatLog::of(&entries);
    let removed: Vec<(u64, Uuid)> = log
        .messages()
        .filter(|shown| shown.removed)
        .map(|shown| (shown.seq, shown.author))
        .collect();
    assert_eq!(removed, [(1, BO), (4, ADA)]);
    assert!(log.message(2).is_none(), "a reaction taken back shows no row");
    assert_eq!(log.message(1).unwrap().body, None);
}
