use super::*;

#[test]
fn clear_done_keeps_open_items() {
    let mut checklist = Checklist::new();
    for text in ["write", "review", "ship"] {
        Checklist::apply_operation(&mut checklist, &ChecklistOperation::add(text));
    }
    let review = checklist.items()[1].id;
    Checklist::apply_operation(
        &mut checklist,
        &ChecklistOperation::SetDone {
            id: review,
            done: true,
        },
    );
    assert_eq!(checklist.done_count(), 1);

    Checklist::apply_operation(&mut checklist, &ChecklistOperation::ClearDone);
    let texts: Vec<_> = checklist
        .items()
        .iter()
        .map(|item| item.text.as_str())
        .collect();
    assert_eq!(texts, ["write", "ship"]);
    assert_eq!(checklist.done_count(), 0);
}
