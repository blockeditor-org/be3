use super::*;

#[test]
fn a_card_edited_after_a_move_is_edited_where_it_went() {
    let document = board();
    let (_, done, write) = ids(&document);

    let document = edited(
        &document,
        [
            Column::CARDS.move_into(done, Anchor::End, write),
            Card::TEXT.set(write, &"write it".to_owned()),
            Card::DONE.set(write, &true),
        ],
    );

    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["review"]), ("Done", &["write it"])])
    );
    assert_eq!(
        document.read::<Card>(write),
        Some(Card {
            text: "write it".to_owned(),
            done: true,
        })
    );
}
