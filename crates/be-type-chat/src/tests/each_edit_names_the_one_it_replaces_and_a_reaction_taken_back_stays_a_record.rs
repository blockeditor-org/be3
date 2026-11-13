use super::*;

#[test]
fn each_edit_names_the_one_it_replaces_and_a_reaction_taken_back_stays_a_record() {
    let chat = sent(&ChatContent::default(), ADA, &["lunch at noon?"]);
    let lunch = first(&chat);
    let of = chat.hash(CHAT, lunch).expect("the message is recent");

    let once = chat.next_edit(CHAT, lunch, ADA, 5, "lunch at one?");
    assert_eq!(once.previous, None, "a message's first edit replaces none");
    let chat = edited(&chat, [chat.put_edit(lunch, &once)]);
    let twice = chat.next_edit(CHAT, lunch, ADA, 6, "lunch at two?");
    assert_eq!(
        twice.previous,
        Some(once.hash(CHAT, lunch, of)),
        "a later edit names the one it replaces"
    );
    let chat = edited(&chat, [chat.put_edit(lunch, &twice)]);
    assert_eq!(chat.body(lunch).as_deref(), Some("lunch at two?"));
    assert_eq!(
        chat.superseded_of(lunch),
        [once.hash(CHAT, lunch, of)],
        "the replaced edit is recorded, so putting it back shows"
    );

    // Someone with Edit puts the first edit back: it is the latest, and it is one replaced.
    let replayed = edited(&chat, [chat.put_edit(lunch, &once)]);
    let shown = replayed
        .edit_of(lunch)
        .expect("the replayed edit is the latest");
    assert!(
        replayed
            .superseded_of(lunch)
            .contains(&shown.hash(CHAT, lunch, of)),
        "the edit shown was replaced before"
    );

    let chat = edited(&chat, [chat.react(lunch, BO, "heart", 8)]);
    assert_eq!(chat.reactions(lunch), [("heart".to_owned(), vec![BO])]);
    let chat = edited(&chat, [chat.unreact(lunch, BO, "heart", 9)]);
    assert!(
        chat.reactions(lunch).is_empty(),
        "a reaction taken back is not shown"
    );
    let records = chat.reaction_records(lunch);
    assert_eq!(records.len(), 1);
    assert!(
        records[0].1.as_ref().is_some_and(|reaction| !reaction.kept),
        "it stays as a record its author can sign"
    );
}
