use super::*;

#[test]
fn escape_returns_to_the_grid() {
    let mut review = Review::with(mixed());
    review.test.click("paint_review.painting.map.broken");
    assert!(review.test.shown("paint_review.stage"));
    review.press(beui::Key::Escape);
    assert!(!review.test.shown("paint_review.stage"));
    assert!(review.test.shown("paint_review.painting.map.broken"));
}
