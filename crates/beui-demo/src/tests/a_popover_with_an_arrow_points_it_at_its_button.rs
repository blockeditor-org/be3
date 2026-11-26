use super::*;

#[test]
fn a_popover_with_an_arrow_points_it_at_its_button() {
    let mut test = alone(WIDE, Page::Overlays);
    reveal(&mut test, "demo.share");
    let button = test.rect_of("demo.share");
    test.click(button.center());
    test.settle();
    test.snapshot("a_popover_with_an_arrow");
}
