use super::*;

#[test]
fn a_selected_row_stays_selected_under_the_pointer() {
    let mut test = alone(WIDE, Page::Buttons);
    reveal(&mut test, "rows.chat");
    test.click("rows.chat");
    test.frame(Vec::new());
    test.hover("rows.chat");
    test.frame(Vec::new());
    test.snapshot("a_selected_row_stays_selected_under_the_pointer");
}
