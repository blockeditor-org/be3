use super::*;
use std::time::Duration;

#[test]
fn reordered_tiles_glide_from_their_old_places_to_their_new_ones() {
    let mut test = alone(WIDE, Page::Layering);
    reveal(&mut test, "demo.layering.glide");
    test.click("demo.glide.reverse");
    test.record();
    for _ in 0..3 {
        test.advance(Duration::from_millis(50));
        test.frame(Vec::new());
        test.record();
    }
    test.settle();
    test.snapshot("reordered_tiles_glide_from_their_old_places_to_their_new_ones");
}
