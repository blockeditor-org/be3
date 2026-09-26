use super::*;

#[test]
fn undoing_the_removal_of_several_cards_restores_their_order() {
    let document = board();
    let (todo, _, write) = ids(&document);
    let mut document = edited(
        &document,
        [Column::CARDS.insert(todo, Anchor::End, &card("ship")).1],
    );
    let before = document.clone();
    let ship = card_id(&document, "ship");

    let step = undone(
        &mut document,
        &Edit(vec![Change::remove(write), Change::remove(ship)]),
    );
    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["review"]), ("Done", &[])])
    );

    document.apply(&step.undo());
    assert_eq!(document, before);
    document.apply(&step.redo());
    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["review"]), ("Done", &[])])
    );
}
