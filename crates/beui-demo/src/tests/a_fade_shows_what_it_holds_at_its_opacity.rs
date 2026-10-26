use super::*;

#[test]
fn a_fade_shows_what_it_holds_at_its_opacity() {
    let mut test = alone(WIDE, Page::Layering);
    reveal(&mut test, "demo.layering.opacity");
    test.snapshot("a_fade_shows_what_it_holds_at_its_opacity");
}
