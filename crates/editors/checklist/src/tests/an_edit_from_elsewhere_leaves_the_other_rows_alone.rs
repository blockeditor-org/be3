use super::*;

#[test]
fn an_edit_from_elsewhere_leaves_the_other_rows_alone() {
    let mut checklist = Harness::new(&[("buy milk", false), ("call the vet", false)]);
    let (first, second) = (checklist.id(0), checklist.id(1));

    let rows = |checklist: &mut Harness| {
        [first, second].map(|item| {
            checklist
                .document()
                .find_test_id(&format!("checklist.item.{item}.done"))
                .expect("a checklist row was never drawn")
        })
    };
    let before = rows(&mut checklist);

    checklist.arrive(ChecklistOp::SetText {
        id: second,
        text: "call the vet back".to_owned(),
    });

    assert_eq!(
        checklist.items(),
        [
            ("buy milk".to_owned(), false),
            ("call the vet back".to_owned(), false)
        ]
    );
    assert_eq!(
        rows(&mut checklist),
        before,
        "editing one item must not rebuild any row"
    );
}
