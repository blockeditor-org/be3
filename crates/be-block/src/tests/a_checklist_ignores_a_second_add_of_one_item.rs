use super::*;

#[test]
fn a_checklist_ignores_a_second_add_of_one_item() {
    let add = ChecklistOp::add("milk");
    let id = match &add {
        ChecklistOp::Add { id, .. } => *id,
        _ => unreachable!(),
    };

    let checklist = listed(
        &ChecklistContent::default(),
        &[
            add.clone(),
            ChecklistOp::SetDone { id, done: true },
            add,
            ChecklistOp::SetText {
                id: Uuid::new_v4(),
                text: "nobody".into(),
            },
        ],
    );

    assert_eq!(texts(&checklist), [("milk", true)]);
    assert_eq!(checklist.done_count(), 1);
}
