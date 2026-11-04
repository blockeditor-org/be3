use super::*;

#[test]
fn an_edit_touches_its_field_and_what_holds_it() {
    let mut document = board();
    let (todo, done, write) = ids(&document);

    let mut touched = Vec::new();
    document.apply_touching(
        &Card::TEXT.set(write, &"draft".to_owned()).into(),
        &mut touched,
    );
    assert_eq!(
        touched,
        [
            Touched::Field(write, Card::TEXT.index()),
            Touched::Subtree(write),
            Touched::Subtree(todo),
            Touched::Subtree(ObjectId::ROOT),
        ]
    );
    assert_eq!(document.field(write, Card::TEXT), "draft");

    let mut touched = Vec::new();
    document.apply_touching(
        &Column::CARDS.move_into(done, Anchor::End, write).into(),
        &mut touched,
    );
    assert!(touched.contains(&Touched::Field(todo, Column::CARDS.index())));
    assert!(touched.contains(&Touched::Field(done, Column::CARDS.index())));
    assert!(!touched.contains(&Touched::Field(write, Card::TEXT.index())));
    assert_eq!(document.ids(done, Column::CARDS), [write]);
}
