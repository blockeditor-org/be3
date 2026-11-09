use super::*;

#[test]
fn a_range_someone_split_meanwhile_does_not_move() {
    let (mut confirmed, todo) = todo_column(&["a", "b", "c", "d", "e", "f"]);
    let mut visible = confirmed.clone();
    let local = vec![Edit::from(Column::CARDS.move_range(
        todo,
        card_ids(&visible, &["b", "c", "d"]),
        Anchor::End,
    ))];
    for edit in &local {
        visible.apply(edit);
    }
    assert_eq!(
        columns(&visible),
        owned(&[("Todo", &["a", "e", "f", "b", "c", "d"])])
    );

    let incoming = Edit::from(Column::CARDS.move_into(
        todo,
        Anchor::After(card_id(&confirmed, "e")),
        card_id(&confirmed, "d"),
    ));
    let mut touched = Vec::new();
    visible.rebase(&mut confirmed, &incoming, &local, &mut touched);

    assert_eq!(
        columns(&visible),
        owned(&[("Todo", &["a", "b", "c", "e", "d", "f"])]),
        "the cards between `b` and `d` are no longer the ones moved, so nothing else goes along"
    );
    assert!(touched.contains(&Touched::Field(todo, Column::CARDS.index())));

    let (mut document, todo) = todo_column(&["a", "b", "c", "d", "e"]);
    let removed = card_id(&document, "c");
    document.apply(&Edit::from(Change::remove(removed)));
    let shift = Column::CARDS.move_range(todo, card_ids(&document, &["b", "d"]), Anchor::Start);
    let step = undone(&mut document, &shift.into());
    assert_eq!(
        columns(&document),
        owned(&[("Todo", &["b", "d", "a", "e"])]),
        "a card removed from between them does not keep them apart"
    );
    document.apply(&step.undo());
    assert_eq!(columns(&document), owned(&[("Todo", &["a", "b", "d", "e"])]));
}
