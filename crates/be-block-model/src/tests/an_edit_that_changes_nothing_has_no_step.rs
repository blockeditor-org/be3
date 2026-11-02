use super::*;

#[test]
fn an_edit_that_changes_nothing_has_no_step() {
    let document = board();
    let (todo, _, _) = ids(&document);

    assert_eq!(document.step(&Edit::default()), None);
    assert_eq!(
        document.step(&Board::TITLE.set(ObjectId::ROOT, &"Plan".to_owned()).into()),
        None
    );
    assert_eq!(
        document.step(&Board::VOTES.add(ObjectId::ROOT, 0).into()),
        None
    );
    assert_eq!(
        document.step(&Card::TEXT.set(ObjectId::new(), &"x".to_owned()).into()),
        None
    );
    assert_eq!(document.step(&Change::remove(ObjectId::new()).into()), None);
    assert_eq!(document.step(&Change::remove(ObjectId::ROOT).into()), None);
    assert_eq!(
        document.step(
            &Column::CARDS
                .move_into(todo, Anchor::Start, ObjectId::new())
                .into()
        ),
        None
    );
}
