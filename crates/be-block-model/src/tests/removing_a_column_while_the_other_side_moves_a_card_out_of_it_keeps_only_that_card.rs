use super::*;

#[test]
fn removing_a_column_while_the_other_side_moves_a_card_out_of_it_keeps_only_that_card() {
    let base = board();
    let (todo, done, write) = ids(&base);
    let ours = edited(&base, [Change::remove(todo)]);
    let theirs = edited(&base, [Column::CARDS.move_into(done, Anchor::End, write)]);

    let (merged, _) = Document::merge(&base, &ours, &theirs);

    assert_eq!(columns(&merged), owned(&[("Done", &["write"])]));
}
