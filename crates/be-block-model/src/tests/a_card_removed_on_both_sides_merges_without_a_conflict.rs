use super::*;

#[test]
fn a_card_removed_on_both_sides_merges_without_a_conflict() {
    let base = board();
    let (_, _, write) = ids(&base);
    let ours = edited(&base, [Change::remove(write)]);
    let theirs = edited(
        &base,
        [
            Change::remove(write),
            Board::TITLE.set(ObjectId::ROOT, &"Ship".to_owned()),
        ],
    );

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(merged.root().title, "Ship");
    assert_eq!(
        columns(&merged),
        owned(&[("Todo", &["review"]), ("Done", &[])])
    );
}
