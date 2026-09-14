use super::*;

#[test]
fn operations_for_unknown_items_are_ignored() {
    let mut checklist = Checklist::new();
    Checklist::apply_operation(&mut checklist, &ChecklistOperation::add("only"));
    let expected = checklist.clone();
    let missing = Uuid::new_v4();

    for operation in [
        ChecklistOperation::SetText {
            id: missing,
            text: "missing".to_owned(),
        },
        ChecklistOperation::SetDone {
            id: missing,
            done: true,
        },
        ChecklistOperation::Remove { id: missing },
    ] {
        Checklist::apply_operation(&mut checklist, &operation);
    }

    assert_eq!(checklist, expected);
}
