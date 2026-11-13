use super::*;

#[test]
fn deleting_a_message_removes_it_with_its_edit_and_reactions() {
    let chat = sent(&ChatContent::default(), ADA, &["typo", "fine"]);
    let typo = first(&chat);
    let chat = edited(
        &chat,
        [
            chat.edit(typo, ADA, 5, "typo fixed"),
            chat.react(typo, BO, "thumbs-up", 6),
        ],
    );
    assert_eq!(chat.body(typo).as_deref(), Some("typo fixed"));
    assert_eq!(chat.reactions(typo), [("thumbs-up".to_owned(), vec![BO])]);

    let chat = edited(&chat, [chat.delete(typo, ADA, 7)]);

    assert_eq!(bodies(&chat), ["fine"]);
    assert_eq!(chat.edit_of(typo), None);
    assert!(chat.reactions(typo).is_empty());
    let deletion = chat.deletion_of(typo).expect("the deletion is recorded");
    assert_eq!((deletion.author, deletion.time), (ADA, 7));
    assert_eq!(round_trip(&chat), chat);
}
