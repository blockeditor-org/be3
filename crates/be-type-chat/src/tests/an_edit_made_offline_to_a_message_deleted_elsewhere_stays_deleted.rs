use super::*;

#[test]
fn an_edit_made_offline_to_a_message_deleted_elsewhere_stays_deleted() {
    let base = sent(&ChatContent::default(), ADA, &["draft plan", "final plan"]);
    let draft = first(&base);
    let edited_offline = edited(
        &base,
        [
            base.edit(draft, ADA, 10, "draft plan, revised"),
            base.react(draft, BO, "eye", 10),
        ],
    );
    let deleted = edited(&base, [base.delete(draft, ADA, 5)]);

    for (ours, theirs) in [(&edited_offline, &deleted), (&deleted, &edited_offline)] {
        let (chat, conflicts) = merged(&base, ours, theirs);

        assert_eq!(conflicts, 0);
        assert_eq!(bodies(&chat), ["final plan"]);
        assert!(chat.deletion_of(draft).is_some());
        assert_eq!(chat.edit_of(draft), None);
        assert!(chat.reactions(draft).is_empty());
        assert_eq!(chat.shown().len(), 1);
    }
}
