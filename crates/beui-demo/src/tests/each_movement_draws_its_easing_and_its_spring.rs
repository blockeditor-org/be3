use super::*;

#[test]
fn each_movement_draws_its_easing_and_its_spring() {
    let mut test = alone(WIDE, Page::Layout);
    reveal(&mut test, "demo.layout.movements");
    test.snapshot("each_movement_draws_its_easing_and_its_spring");
}
