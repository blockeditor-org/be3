use super::*;

#[test]
fn two_inserts_after_the_same_card_at_once_both_land_after_it() {
    let base = board();
    let (todo, _, write) = ids(&base);
    let (_, ours) = Column::CARDS.insert(todo, Anchor::After(write), &card("ours"));
    let (_, theirs) = Column::CARDS.insert(todo, Anchor::After(write), &card("theirs"));

    let sequenced = edited(&base, [ours, theirs]);

    let todo_cards = &columns(&sequenced)[0].1;
    assert_eq!(todo_cards.len(), 4);
    assert_eq!(todo_cards[0], "write");
    assert_eq!(todo_cards[3], "review");
}
