use super::*;

#[test]
#[ignore = "a removal's undo anchors after the card that preceded it, and falls to the end when that card is gone"]
fn undoing_a_removal_puts_the_card_back_after_its_neighbour_was_removed_since() {
    let document = board();
    let (todo, _, write) = ids(&document);
    let mut document = edited(
        &document,
        [Column::CARDS.insert(todo, Anchor::End, &card("ship")).1],
    );
    let review = card_id(&document, "review");

    let step = undone(&mut document, &Change::remove(review).into());
    document.apply(&Change::remove(write).into());
    document.apply(&step.undo());

    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["review", "ship"]), ("Done", &[])])
    );
}
