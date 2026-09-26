use super::*;
use block_editor_plugin::be_block::canvas::CanvasColor;
use block_editor_plugin::beui::{Key, Vec2};

#[test]
fn a_typed_transform_value_is_one_edit() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(120.0, 80.0), 0.0);
    rectangle.style.fill = Some(CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    });
    let mut editor = editor(std::slice::from_ref(&rectangle));

    let drawn = format!("infinite-canvas.entity.{}", rectangle.id);
    editor.click(&drawn);
    editor.run();
    let before = editor.applied(None);
    editor.click("infinite-canvas.transform.x");
    editor.run();
    for digit in ["1", "2", "5"] {
        editor.text(digit);
        editor.run();
    }
    editor.key_press(Key::Enter);
    editor.run();

    assert_eq!(entities(&editor)[0].transform.center.x, 125.0);
    assert_eq!(editor.applied(None), before + 1);

    let resting = editor.rect_of(&drawn);
    let field = editor.rect_of("infinite-canvas.transform.y").center();
    editor.drag(field, field + Vec2::new(20.0, 0.0));
    editor.run();

    assert_eq!(
        editor.rect_of(&drawn).center().y - resting.center().y,
        20.0,
        "a committed preview must not keep drawing over later edits"
    );
}
