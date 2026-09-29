use super::*;

#[test]
fn a_move_that_changes_nothing_has_no_step() {
    let document = board();
    let (todo, _, write) = ids(&document);

    assert_eq!(
        document.step(&Column::CARDS.move_into(todo, Anchor::Start, write).into()),
        None,
        "the card is already first"
    );
    assert_eq!(
        document.step(
            &Column::CARDS
                .move_into(todo, Anchor::After(write), write)
                .into()
        ),
        None,
        "a card cannot follow itself"
    );
}
