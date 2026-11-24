use super::*;

#[test]
fn a_sticky_header_stays_at_the_top_of_its_scroll() {
    let mut test = alone(WIDE, Page::Lists);
    reveal(&mut test, "demo.lists.sticky");
    test.snapshot("a_sticky_header_stays_at_the_top_of_its_scroll");
}
