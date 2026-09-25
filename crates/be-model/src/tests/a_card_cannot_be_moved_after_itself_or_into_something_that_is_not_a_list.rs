use super::*;

#[test]
fn a_card_cannot_be_moved_after_itself_or_into_something_that_is_not_a_list() {
    let document = board();
    let (todo, done, write) = ids(&document);

    let unchanged = edited(
        &document,
        [
            Column::CARDS.move_into(todo, Anchor::After(write), write),
            Change::Move {
                object: write,
                place: Column::NAME.of(done),
                anchor: Anchor::End,
            },
            Change::Move {
                object: write,
                place: Column::CARDS.of(ObjectId::new()),
                anchor: Anchor::End,
            },
        ],
    );

    assert_eq!(unchanged, document);
}
