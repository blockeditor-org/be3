use super::*;
use crate::Page;

const WINDOW: Vec2 = Vec2::new(420.0, 360.0);

#[test]
fn a_scrollbar_shows_over_a_page_as_it_scrolls_and_widens_to_a_track_under_the_pointer() {
    let mut test = alone(WINDOW, Page::Buttons);
    test.frame(Vec::new());
    test.snapshot("scrollbar_hidden_at_rest");

    test.scroll(over_the_page(WINDOW), Vec2::new(0.0, 120.0));
    test.frame(Vec::new());
    test.snapshot("scrollbar_shown_while_scrolling");

    test.frame(vec![Input::PointerMoved(pos2(
        WINDOW.x - 4.0,
        WINDOW.y / 2.0,
    ))]);
    test.frame(Vec::new());
    test.snapshot("scrollbar_track_under_the_pointer");
}
