use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

#[test]
fn every_demo_page_paints_as_accepted_on_a_phone() {
    paint_every_page(PHONE, "_phone");
}
