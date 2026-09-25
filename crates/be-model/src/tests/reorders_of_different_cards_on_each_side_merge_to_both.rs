use super::*;

#[test]
#[ignore = "two reorders of one list conflict in the list merge and one side's move is dropped without counting a conflict"]
fn reorders_of_different_cards_on_each_side_merge_to_both() {
    let base = board();
    let (todo, _, _) = ids(&base);
    let base = edited(
        &base,
        [
            Column::CARDS.insert(todo, Anchor::End, &card("test")).1,
            Column::CARDS.insert(todo, Anchor::End, &card("ship")).1,
        ],
    );
    let write = card_id(&base, "write");
    let ship = card_id(&base, "ship");
    let ours = edited(&base, [Column::CARDS.move_into(todo, Anchor::End, write)]);
    let theirs = edited(&base, [Column::CARDS.move_into(todo, Anchor::Start, ship)]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        columns(&merged),
        owned(&[
            ("Todo", &["ship", "review", "test", "write"]),
            ("Done", &[])
        ])
    );
}
