use super::*;
use block_editor_beui::beui::{Key, Modifiers, Vec2};
use infinite_canvas_types::CanvasColor;

#[test]
fn a_typed_transform_value_is_one_edit() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(120.0, 80.0), 0.0);
    rectangle.style.fill = CanvasFill::Solid;
    rectangle.style.fill_color = CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    };
    let mut editor = vector_editor(std::slice::from_ref(&rectangle));

    let drawn = format!("infinite-canvas.entity.{}", rectangle.id);
    editor.click(&drawn);
    editor.click("infinite-canvas.transform.x");
    for digit in ["1", "2", "5"] {
        editor.type_text(digit);
    }
    editor.key(Key::Enter, Modifiers::NONE);

    assert_eq!(entities(&editor)[0].transform.center.x, 125.0);
    undo(&mut editor);
    assert_eq!(
        entities(&editor)[0].transform.center.x,
        0.0,
        "every digit is written as it is typed, and one undo takes the value back"
    );
    editor.click("infinite-canvas.transform.x");
    editor.type_text("125");
    editor.key(Key::Enter, Modifiers::NONE);

    let resting = editor.rect_of(&drawn);
    let field = editor.point_of("infinite-canvas.transform.y");
    editor.drag(field, field + Vec2::new(20.0, 0.0));

    assert_eq!(
        editor.rect_of(&drawn).center().y - resting.center().y,
        20.0,
        "a committed preview must not keep drawing over later edits"
    );
}
