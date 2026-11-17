use super::*;

#[test]
fn a_popover_with_an_arrow_points_it_at_its_button() {
    let mut test = alone(WIDE, Page::Overlays);
    reveal(&mut test, "demo.share");
    let button = test.rect_of("demo.share");
    test.click(button.center());
    test.settle();
    // The beak points a frame after the panel is placed, though nothing asks for that frame
    // (plans/backlog.md, A beak a frame late).
    let _ = test.frame(Vec::new());
    test.snapshot("a_popover_with_an_arrow");
}
