use super::*;

#[test]
fn one_step_undoes_every_change_an_edit_made() {
    let mut document = board();
    let (todo, done, write) = ids(&document);
    let before = document.clone();
    let (_, insert) = Column::CARDS.insert(done, Anchor::End, &card("ship"));

    let step = undone(
        &mut document,
        &Edit(vec![
            Board::TITLE.set(ObjectId::ROOT, &"Launch".to_owned()),
            Board::VOTES.add(ObjectId::ROOT, 2),
            insert,
            Column::CARDS.move_into(done, Anchor::Start, write),
            Column::NAME.set(todo, &"Later".to_owned()),
        ]),
    );
    let after = document.clone();

    document.apply(&step.undo());
    assert_eq!(document, before);
    document.apply(&step.redo());
    assert_eq!(document, after);
}
