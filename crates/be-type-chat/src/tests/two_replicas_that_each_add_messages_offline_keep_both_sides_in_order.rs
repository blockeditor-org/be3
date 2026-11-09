use super::*;

#[test]
fn two_replicas_that_each_add_messages_offline_keep_both_sides_in_order() {
    let base = sent(&ChatContent::default(), ADA, &["hello", "anyone here?"]);
    let ours = sent(&base, ADA, &["I'll be offline", "back soon"]);
    let theirs = sent(&base, BO, &["hi Ada", "yes", "on the train"]);

    let (chat, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        bodies(&chat),
        [
            "hello",
            "anyone here?",
            "I'll be offline",
            "back soon",
            "hi Ada",
            "yes",
            "on the train"
        ]
    );
    let (chat, _) = merged(&base, &theirs, &ours);
    assert_eq!(
        bodies(&chat),
        [
            "hello",
            "anyone here?",
            "hi Ada",
            "yes",
            "on the train",
            "I'll be offline",
            "back soon"
        ],
        "merged from the other side, its own messages come first"
    );
}
