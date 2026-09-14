use super::*;

#[test]
fn adding_the_same_item_twice_keeps_one_copy() {
    let mut checklist = Checklist::new();
    let add = ChecklistOperation::add("write the guide");
    Checklist::apply_operation(&mut checklist, &add);
    Checklist::apply_operation(&mut checklist, &add);

    assert_eq!(checklist.items().len(), 1);
    assert_eq!(checklist.items()[0].text, "write the guide");
}
