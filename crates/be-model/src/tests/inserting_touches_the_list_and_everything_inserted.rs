use super::*;

#[test]
fn inserting_touches_the_list_and_everything_inserted() {
    let mut document = board();
    let (column, insert) =
        Board::COLUMNS.insert(ObjectId::ROOT, Anchor::End, &column("Later", &["ship"]));

    let mut touched = Vec::new();
    document.apply_touching(&insert.into(), &mut touched);

    let ship = card_id(&document, "ship");
    assert!(touched.contains(&Touched::Field(ObjectId::ROOT, Board::COLUMNS.index())));
    assert!(touched.contains(&Touched::Subtree(ObjectId::ROOT)));
    assert!(touched.contains(&Touched::Subtree(column)));
    assert!(touched.contains(&Touched::Subtree(ship)));
}
