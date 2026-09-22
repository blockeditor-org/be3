use super::*;

#[test]
fn a_checklist_round_trips_through_its_bytes() {
    let checklist = listed(
        &ChecklistContent::default(),
        &[ChecklistOp::add("milk"), ChecklistOp::add("eggs")],
    );

    let bytes = checklist.encode();

    assert_eq!(ChecklistContent::decode(&bytes), Ok(checklist));
    assert_eq!(
        ChecklistContent::decode(&[0xff]),
        Err(ContentError::Malformed("checklist"))
    );
}
