use super::*;

#[test]
fn edits_and_reactions_made_offline_on_both_sides_are_kept_by_message() {
    let base = sent(&ChatContent::default(), ADA, &["lunch?", "noon"]);
    let [lunch, noon] = [base.messages()[0], base.messages()[1]];
    let ours = edited(
        &base,
        [
            base.edit(noon, ADA, 3, "half past twelve"),
            base.react(lunch, ADA, "heart", 3),
        ],
    );
    let theirs = sent(
        &edited(
            &base,
            [
                base.react(lunch, BO, "heart", 4),
                base.react(noon, BO, "thumbs-up", 4),
            ],
        ),
        BO,
        &["see you there"],
    );

    let (chat, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        bodies(&chat),
        ["lunch?", "half past twelve", "see you there"]
    );
    assert_eq!(chat.reactions(lunch), [("heart".to_owned(), vec![BO, ADA])]);
    assert_eq!(chat.reactions(noon), [("thumbs-up".to_owned(), vec![BO])]);
}
