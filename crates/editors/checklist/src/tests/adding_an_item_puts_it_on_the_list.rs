use super::*;

#[test]
fn adding_an_item_puts_it_on_the_list() {
    let mut checklist = Harness::new(&[]);

    checklist.click("checklist.draft");
    checklist.type_text("buy milk");
    checklist.click("checklist.add");

    assert_eq!(checklist.items(), [("buy milk".to_owned(), false)]);
    checklist.snapshot("adding_an_item_puts_it_on_the_list");
}
