use super::*;

#[test]
fn a_card_moved_to_the_same_place_on_both_sides_merges_cleanly() {
    let base = board();
    let (_, done, write) = ids(&base);
    let ours = edited(&base, [Column::CARDS.move_into(done, Anchor::End, write)]);
    let theirs = edited(
        &base,
        [
            Column::CARDS.move_into(done, Anchor::End, write),
            Card::DONE.set(write, &true),
        ],
    );

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        columns(&merged),
        owned(&[("Todo", &["review"]), ("Done", &["write"])])
    );
    assert_eq!(merged.read::<Card>(write).map(|card| card.done), Some(true));
}
