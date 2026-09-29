use super::*;
use block_editor_beui::be_block::canvas::CanvasColor;
use block_editor_beui::beui::Vec2;

#[test]
fn a_moved_entity_is_drawn_where_it_was_dropped() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(180.0, 120.0), 0.0);
    rectangle.style.fill = Some(CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    });
    let mut editor = editor(std::slice::from_ref(&rectangle));
    let test_id = format!("infinite-canvas.entity.{}", rectangle.id);
    let before = editor.rect_of(&test_id);

    editor.drag(before.center(), before.center() + Vec2::new(100.0, 0.0));
    BeuiTest::run(&mut editor);

    let after = editor.rect_of(&test_id);
    assert!(
        (after.min.x - before.min.x - 100.0).abs() < 2.0,
        "the frame the drag ended in drew the entity at {after:?}, not 100 right of {before:?}"
    );
}
