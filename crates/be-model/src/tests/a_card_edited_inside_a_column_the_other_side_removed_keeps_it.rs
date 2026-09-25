use super::*;

#[test]
fn a_card_edited_inside_a_column_the_other_side_removed_keeps_it() {
    let base = board();
    let (todo, _, write) = ids(&base);
    let ours = edited(&base, [Change::remove(todo)]);
    let theirs = edited(&base, [Card::TEXT.set(write, &"write it".to_owned())]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert!(conflicts >= 1);
    let todo_cards = columns(&merged)
        .into_iter()
        .find(|(name, _)| name == "Todo")
        .map(|(_, cards)| cards)
        .expect("the column holding the edit comes back");
    assert!(todo_cards.contains(&"write it".to_owned()));
}
