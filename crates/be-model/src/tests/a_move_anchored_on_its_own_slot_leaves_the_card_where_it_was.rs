use super::*;

use uuid::Uuid;

#[test]
fn a_move_anchored_on_its_own_slot_leaves_the_card_where_it_was() {
    let mut document = board();
    let (todo, done, _) = ids(&document);
    let moving = ObjectId::from_uuid(Uuid::from_u64_pair(1, 7));
    let twin = ObjectId::from_uuid(Uuid::from_u64_pair(2, 7));
    document.apply(
        &Column::CARDS
            .insert_as(moving, done, Anchor::End, &card("moving"))
            .into(),
    );
    document.apply(&Column::CARDS.move_into(todo, Anchor::End, moving).into());

    let anchored_on_itself = Change::Move {
        object: moving,
        place: Column::CARDS.of(done),
        anchor: Anchor::After(twin),
    };
    document.apply(&anchored_on_itself.into());

    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["write", "review", "moving"]), ("Done", &[])])
    );
}
