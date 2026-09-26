use super::*;

#[test]
fn removing_touches_the_list_and_everything_that_was_inside() {
    let mut document = board();
    let (todo, _, write) = ids(&document);
    let review = card_id(&document, "review");

    let mut touched = Vec::new();
    document.apply_touching(&Change::remove(todo).into(), &mut touched);

    assert!(touched.contains(&Touched::Field(ObjectId::ROOT, Board::COLUMNS.index())));
    assert!(touched.contains(&Touched::Subtree(ObjectId::ROOT)));
    assert!(touched.contains(&Touched::Subtree(todo)));
    assert!(touched.contains(&Touched::Subtree(write)));
    assert!(touched.contains(&Touched::Subtree(review)));
}
