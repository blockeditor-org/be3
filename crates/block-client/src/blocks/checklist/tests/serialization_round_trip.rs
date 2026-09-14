use super::*;

#[test]
fn serialization_round_trip() {
    let mut checklist = Checklist::new();
    Checklist::apply_operation(&mut checklist, &ChecklistOperation::add("buy milk"));
    let id = checklist.items()[0].id;
    Checklist::apply_operation(&mut checklist, &ChecklistOperation::SetDone { id, done: true });
    assert_eq!(
        checklist.items(),
        [ChecklistItem {
            id,
            text: "buy milk".to_owned(),
            done: true,
        }]
    );

    let encoded = serde_json::to_vec(&checklist).unwrap();
    assert_eq!(
        serde_json::from_slice::<Checklist>(&encoded).unwrap(),
        checklist
    );
}
