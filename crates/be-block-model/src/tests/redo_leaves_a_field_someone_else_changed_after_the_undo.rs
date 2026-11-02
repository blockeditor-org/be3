use super::*;

#[test]
fn redo_leaves_a_field_someone_else_changed_after_the_undo() {
    let mut document = board();
    let (_, _, write) = ids(&document);

    let step = undone(
        &mut document,
        &Card::TEXT.set(write, &"draft".to_owned()).into(),
    );
    document.apply(&step.undo());
    document.apply(&Card::TEXT.set(write, &"theirs".to_owned()).into());
    document.apply(&step.redo());

    assert_eq!(document.field(write, Card::TEXT), "theirs");
}
