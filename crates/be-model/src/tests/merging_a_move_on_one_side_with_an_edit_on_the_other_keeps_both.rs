use super::*;

#[test]
fn merging_a_move_on_one_side_with_an_edit_on_the_other_keeps_both() {
    let base = board();
    let (todo, done, write) = ids(&base);
    let ours = edited(&base, [Column::CARDS.move_into(done, Anchor::End, write)]);
    let (_, add) = Column::CARDS.insert(todo, Anchor::End, &card("ship"));
    let theirs = edited(&base, [Card::TEXT.set(write, &"write it".to_owned()), add]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        columns(&merged),
        owned(&[("Todo", &["review", "ship"]), ("Done", &["write it"])])
    );
}
