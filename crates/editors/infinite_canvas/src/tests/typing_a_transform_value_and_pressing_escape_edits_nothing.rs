use super::*;
use block_editor_plugin::be_block::canvas::CanvasColor;
use block_editor_plugin::beui::{Key, Vec2};

#[test]
fn typing_a_transform_value_and_pressing_escape_edits_nothing() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(120.0, 80.0), 0.0);
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
    let before = editor.applied(None);
    let resting = editor.rect_of(&drawn);
    editor.click("infinite-canvas.transform.x");
    editor.run();
    editor.text("4");
    editor.run();
    editor.text("0");
    editor.run();

    assert_eq!(
        editor.rect_of(&drawn).center().x - resting.center().x,
        40.0,
        "the canvas shows the typed value while it is typed"
    );

    editor.key_press(Key::Escape);
    editor.run();

    assert_eq!(editor.rect_of(&drawn), resting);
    assert_eq!(entities(&editor)[0].transform.center.x, 0.0);
    assert_eq!(
        editor.applied(None),
        before,
        "a cancelled edit must not reach the block, or it lands in its undo history"
    );

    let field = editor.rect_of("infinite-canvas.transform.y").center();
    editor.drag(field, field + Vec2::new(20.0, 0.0));
    editor.run();

    assert_eq!(entities(&editor)[0].transform.center.y, 20.0);
    assert_eq!(
        editor.rect_of(&drawn).center().y - resting.center().y,
        20.0,
        "a cancelled preview must not keep drawing over later edits"
    );
}
