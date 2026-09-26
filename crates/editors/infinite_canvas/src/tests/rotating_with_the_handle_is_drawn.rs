use super::*;
use block_editor_beui::be_block::canvas::CanvasColor;
use block_editor_beui::beui::Vec2;

#[test]
fn rotating_with_the_handle_is_drawn() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(180.0, 120.0), 0.0);
    rectangle.style.fill = Some(CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    });
    let drawn = format!("infinite-canvas.entity.{}", rectangle.id);
    let mut editor = editor(std::slice::from_ref(&rectangle));
    editor.click(&drawn);
    editor.run();
    let resting = editor.rect_of(&drawn);
    let handle = block_editor_beui::beui::pos2(resting.center().x, resting.top() - 28.0);

    editor.drag(handle, handle + Vec2::new(120.0, 60.0));
    editor.run();

    assert_ne!(
        entities(&editor)[0].transform.rotation,
        0.0,
        "the handle rotates the entity"
    );
    assert_ne!(editor.rect_of(&drawn), resting, "the rotation is drawn");
}
