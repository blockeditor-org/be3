use super::*;

#[test]
fn setting_a_field_of_a_removed_card_changes_nothing() {
    let document = board();
    let (_, _, write) = ids(&document);
    let removed = edited(&document, [Change::remove(write)]);

    let edited_after = edited(
        &removed,
        [
            Card::TEXT.set(write, &"write it".to_owned()),
            Card::DONE.set(write, &true),
        ],
    );

    assert_eq!(edited_after, removed);
    assert_eq!(edited_after.read::<Card>(write), None);
}
