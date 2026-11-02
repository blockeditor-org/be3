use super::*;

#[test]
fn a_series_grows_by_sending_only_what_was_appended() {
    let mut document = trace(&[1.0, 2.0]);
    let before = document.clone();

    let change = grown(&document, &[1.0, 2.0, 3.0, 4.0]);
    let Change::Series {
        op: SeqOp::Insert { items, .. },
        ..
    } = &change
    else {
        panic!("growing a series inserts at its end: {change:?}");
    };
    assert_eq!(items.len(), 2, "only the appended points are sent");
    let edit = Edit(vec![change]);
    let step = document.step(&edit).expect("the edit changes something");
    document.apply(&edit);
    assert_eq!(*document.root().points, [1.0, 2.0, 3.0, 4.0]);

    let change = grown(&document, &[1.0, 2.0, 3.0, 4.0, 5.0]);
    document.apply(&Edit(vec![change]));
    assert_eq!(*document.root().points, [1.0, 2.0, 3.0, 4.0, 5.0]);

    let change = grown(&document, &[9.0]);
    document.apply(&Edit(vec![change]));
    assert_eq!(
        *document.root().points,
        [9.0],
        "a series can be replaced whole"
    );

    let mut undoing = before.clone();
    undoing.apply(&edit);
    undoing.apply(&step.undo());
    assert_eq!(undoing, before, "undo takes back what was appended");
}
