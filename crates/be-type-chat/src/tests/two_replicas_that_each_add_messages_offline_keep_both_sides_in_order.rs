use super::*;

#[test]
fn two_replicas_that_each_add_messages_offline_keep_both_sides_in_order() {
    let base = sent(&ChatContent::default(), ADA, &["hello", "anyone here?"]);
    let offline = sent(&base, ADA, &["I'll be offline", "back soon"]);
    let published = sent(&base, BO, &["hi Ada", "yes", "on the train"]);

    // The device that was offline merges its messages onto the head others published.
    let (chat, conflicts) = merged(&base, &offline, &published);

    assert_eq!(conflicts, 0);
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
        "what was published stays where everyone saw it, and the offline run follows it"
    );
    let (chat, _) = merged(&base, &published, &offline);
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
        ],
        "whichever side is published comes first"
    );
}
