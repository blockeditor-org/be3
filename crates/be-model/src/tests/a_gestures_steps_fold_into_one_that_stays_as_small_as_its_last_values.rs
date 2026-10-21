use super::*;

#[test]
fn a_gestures_steps_fold_into_one_that_stays_as_small_as_its_last_values() {
    let mut document = board();
    let before = document.clone();
    let (todo, _, write) = ids(&document);

    let (ship, insert) = Column::CARDS.insert(todo, Anchor::End, &card("s"));
    let mut gesture = undone(&mut document, &insert.into());
    let mut typed = String::from("s");
    for letter in "hip it".chars() {
        typed.push(letter);
        let edit = match letter {
            ' ' => Edit(vec![
                Card::TEXT.set(ship, &typed),
                Card::DONE.set(write, &true),
            ]),
            _ => Card::TEXT.set(ship, &typed).into(),
        };
        gesture.then(undone(&mut document, &edit));
    }
    gesture.then(undone(&mut document, &Change::remove(write).into()));
    assert_eq!(
        gesture.redo().0.len(),
        4,
        "the insert, the latest text, the latest done and the removal"
    );
    let after = document.clone();

    document.apply(&gesture.undo());
    assert_eq!(document, before, "one undo takes back the whole gesture");
    document.apply(&gesture.redo());
    assert_eq!(document, after);
}
