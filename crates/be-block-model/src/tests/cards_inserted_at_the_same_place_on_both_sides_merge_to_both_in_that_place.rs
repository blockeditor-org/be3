use super::*;

#[test]
fn cards_inserted_at_the_same_place_on_both_sides_merge_to_both_in_that_place() {
    let base = board();
    let (todo, _, write) = ids(&base);
    let ours = edited(
        &base,
        [Column::CARDS
            .insert(todo, Anchor::After(write), &card("ours"))
            .1],
    );
    let theirs = edited(
        &base,
        [Column::CARDS
            .insert(todo, Anchor::After(write), &card("theirs"))
            .1],
    );

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        columns(&merged),
        owned(&[
            ("Todo", &["write", "ours", "theirs", "review"]),
            ("Done", &[])
        ])
    );
}
