use super::*;

#[test]
fn a_reaction_made_offline_to_a_message_moved_into_a_segment_survives_the_merge() {
    let base = long(120);
    let target = base.messages()[0];

    // Online, the oldest messages close into a segment; offline, someone reacts to one of them
    // and writes another message.
    let online = closed(&base);
    assert!(
        online.message(target).is_none(),
        "the message left the recent ones for a segment"
    );
    let offline = edited(
        &base,
        [
            base.react(target, BO, "heart", 7),
            base.send(CHAT, Message::new(BO, 8, "offline words")).1,
        ],
    );

    for (ours, theirs) in [(&offline, &online), (&online, &offline)] {
        let (merged, conflicts) = merged(&base, ours, theirs);
        assert_eq!(conflicts, 0);
        assert_eq!(
            merged.reactions(target),
            [("heart".to_owned(), vec![BO])],
            "the reaction is kept by message id though the message moved into a segment"
        );
        assert_eq!(merged.segments().len(), 1, "the closed segment stays");
        assert!(
            bodies(&merged)
                .last()
                .is_some_and(|last| last == "offline words"),
            "and the offline message follows the recent ones"
        );
        assert_eq!(
            merged.messages().len() as u64 + merged.segments()[0].count,
            121,
            "every message is recent or in the segment, none twice"
        );
    }
}
