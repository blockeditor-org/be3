use super::*;

const WINDOW: Vec2 = Vec2::new(1100.0, 800.0);

#[test]
fn every_demo_page_paints_as_accepted() {
    paint_every_page(WINDOW, "");
}
