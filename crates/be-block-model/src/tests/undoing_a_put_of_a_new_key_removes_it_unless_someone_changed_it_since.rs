use super::*;

#[test]
fn undoing_a_put_of_a_new_key_removes_it_unless_someone_changed_it_since() {
    let mut document = sheet(&[]);
    let add_one: Edit = Sheet::CELLS
        .put(ObjectId::ROOT, &1, Some(&"one".to_owned()))
        .into();
    let add_two: Edit = Sheet::CELLS
        .put(ObjectId::ROOT, &2, Some(&"two".to_owned()))
        .into();
    let one = document
        .step(&add_one)
        .expect("adding a key changes something");
    document.apply(&add_one);
    let two = document
        .step(&add_two)
        .expect("adding a key changes something");
    document.apply(&add_two);
    document.apply(
        &Sheet::CELLS
            .put(ObjectId::ROOT, &2, Some(&"deux".to_owned()))
            .into(),
    );

    document.apply(&one.undo());
    document.apply(&two.undo());

    assert_eq!(cell(&document, 1), None);
    assert_eq!(cell(&document, 2).as_deref(), Some("deux"));
    document.apply(&one.redo());
    assert_eq!(cell(&document, 1).as_deref(), Some("one"));
}
