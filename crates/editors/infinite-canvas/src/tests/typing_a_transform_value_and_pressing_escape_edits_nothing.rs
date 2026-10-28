use super::*;
use block_editor_beui::beui::{Key, Modifiers, Vec2};
use infinite_canvas_types::CanvasColor;

#[test]
fn typing_a_transform_value_and_pressing_escape_edits_nothing() {
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
    let drawn = format!("infinite-canvas.entity.{}", rectangle.id);
    let mut editor = vector_editor(std::slice::from_ref(&rectangle));

    editor.click(&drawn);
    editor.click("infinite-canvas.transform.width");
    editor.type_text("150");
    editor.key(Key::Enter, Modifiers::NONE);
    let resting = editor.rect_of(&drawn);
    editor.click("infinite-canvas.transform.x");
    editor.type_text("4");
    editor.type_text("0");

    assert_eq!(
        editor.rect_of(&drawn).center().x - resting.center().x,
        40.0,
        "the canvas shows the typed value while it is typed"
    );

    editor.key(Key::Escape, Modifiers::NONE);

    assert_eq!(editor.rect_of(&drawn), resting);
    assert_eq!(entities(&editor)[0].transform.center.x, 0.0);
    undo(&mut editor);
    assert_eq!(
        entities(&editor)[0].transform.size.x,
        120.0,
        "a cancelled edit leaves nothing in the undo history, so undo takes back the width"
    );
    editor.click("infinite-canvas.transform.width");
    editor.type_text("150");
    editor.key(Key::Enter, Modifiers::NONE);

    let field = editor.point_of("infinite-canvas.transform.y");
    editor.drag(field, field + Vec2::new(20.0, 0.0));

    assert_eq!(entities(&editor)[0].transform.center.y, 20.0);
    assert_eq!(
        editor.rect_of(&drawn).center().y - resting.center().y,
        20.0,
        "a cancelled preview must not keep drawing over later edits"
    );
}
