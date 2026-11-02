use be_sdk_beui::beui::Vec2;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

#[test]
fn on_a_phone_the_schema_is_gathered_beside_the_views() {
    let (mut harness, editor) = editor();
    harness.set_size(PHONE);
    harness.settle();
    let gathered = harness.rect_of("dock.compact");
    assert!(
        harness.shown("database.schema") && editor.content_rect().bottom() <= gathered.top() + 1.0,
        "the schema is gathered below the views, {gathered:?}"
    );
    harness.snapshot("on_a_phone_the_schema_is_gathered_below_the_views");

    harness.set_size(LANDSCAPE_PHONE);
    harness.settle();
    let gathered = harness.rect_of("dock.compact");
    assert!(
        harness.shown("database.schema") && editor.content_rect().right() <= gathered.left() + 1.0,
        "on a phone on its side the schema is gathered right of the views, {gathered:?}"
    );
    harness.snapshot("on_a_phone_on_its_side_the_schema_is_gathered_right_of_the_views");
}
