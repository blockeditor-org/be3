use super::*;

#[test]
fn the_same_edit_or_reaction_made_on_two_devices_keeps_the_later_without_a_conflict() {
    let base = sent(&ChatContent::default(), ADA, &["lunch at noon?"]);
    let lunch = first(&base);
    let laptop = edited(
        &base,
        [
            base.edit(CHAT, lunch, ADA, 10, "lunch at one?"),
            base.react(lunch, ADA, "heart", 10),
        ],
    );
    let phone = edited(
        &base,
        [
            base.edit(CHAT, lunch, ADA, 20, "lunch at two?"),
            base.react(lunch, ADA, "heart", 20),
        ],
    );

    for (ours, theirs) in [(&laptop, &phone), (&phone, &laptop)] {
        let (chat, conflicts) = merged(&base, ours, theirs);

        assert_eq!(conflicts, 0);
        assert_eq!(bodies(&chat), ["lunch at two?"], "the later edit wins");
        assert_eq!(chat.reactions(lunch), [("heart".to_owned(), vec![ADA])]);
    }

    let unreacted = edited(&laptop, [laptop.unreact(lunch, ADA, "heart", 30)]);
    let (chat, conflicts) = merged(&base, &unreacted, &phone);
    assert_eq!(conflicts, 0);
    assert!(
        chat.reactions(lunch).is_empty(),
        "taking a reaction back after both devices made it removes it"
    );
}
