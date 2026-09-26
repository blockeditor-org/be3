use super::*;

#[test]
fn undoing_an_add_takes_back_only_that_add() {
    let mut document = board();

    let step = undone(&mut document, &Board::VOTES.add(ObjectId::ROOT, 3).into());
    document.apply(&Board::VOTES.add(ObjectId::ROOT, 10).into());
    document.apply(&step.undo());
    assert_eq!(document.root().votes, Count(10));

    document.apply(&step.redo());
    assert_eq!(document.root().votes, Count(13));
}
