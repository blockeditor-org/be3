use super::*;
use block_editor_beui::beui::Vec2;
use infinite_canvas_types::CanvasColor;

#[test]
fn dragging_a_transform_field_twice_keeps_the_first_drag() {
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

    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    let field = editor.point_of("infinite-canvas.transform.x");
    editor.drag(field, field + Vec2::new(20.0, 0.0));
    let field = editor.point_of("infinite-canvas.transform.x");
    editor.drag(field, field + Vec2::new(10.0, 0.0));

    assert_eq!(entities(&editor)[0].transform.center.x, 30.0);
}
