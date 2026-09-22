use super::*;

#[test]
fn filtering_to_open_hides_the_items_that_are_done() {
    let mut checklist = Harness::new(&[("buy milk", false), ("call the vet", false)]);
    let second = checklist.id(1);

    checklist.click(&format!("checklist.item.{second}.done"));
    checklist.click("checklist.filter.open");

    assert_eq!(
        checklist.items(),
        [
            ("buy milk".to_owned(), false),
            ("call the vet".to_owned(), true)
        ]
    );
    assert_eq!(
        checklist
            .document()
            .find_test_id(&format!("checklist.item.{second}.done")),
        None,
        "the filtered out item must not leave its test id behind"
    );
    checklist.snapshot("filtering_to_open_hides_the_items_that_are_done");
}
