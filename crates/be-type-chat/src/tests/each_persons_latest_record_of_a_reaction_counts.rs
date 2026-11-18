use super::*;

/// A reaction shows for whoever's latest record of it keeps it: one taken back is gone, one
/// made again is back, and each person counts once.
#[test]
fn each_persons_latest_record_of_a_reaction_counts() {
    let react = |kept: bool| ChatEntry::React {
        message: 0,
        reaction: "heart".to_owned(),
        kept,
        takes_back: None,
        time: 0,
    };
    let entries = [
        item(0, ADA, &message("hello")),
        item(1, ADA, &react(true)),
        item(2, BO, &react(true)),
        item(3, ADA, &react(false)),
        item(4, BO, &react(true)),
    ];
    let log = ChatLog::of(&entries);
    assert_eq!(
        log.message(0).unwrap().reactions,
        vec![("heart".to_owned(), vec![BO])]
    );
    assert_eq!(log.reaction(0, ADA, "heart"), Some((3, false)));
    assert_eq!(log.reaction(0, BO, "heart"), Some((4, true)));
}
