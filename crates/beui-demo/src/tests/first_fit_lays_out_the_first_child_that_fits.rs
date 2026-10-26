use super::*;

#[test]
fn first_fit_lays_out_the_first_child_that_fits() {
    let mut test = alone(WIDE, Page::Lists);
    reveal(&mut test, "demo.lists.first_fit");
    test.snapshot("first_fit_lays_out_the_first_child_that_fits");
}
