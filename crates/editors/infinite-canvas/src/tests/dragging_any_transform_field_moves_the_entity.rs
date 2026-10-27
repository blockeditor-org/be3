use super::*;
use block_editor_beui::beui::Vec2;
use infinite_canvas_types::CanvasColor;

#[test]
fn dragging_any_transform_field_moves_the_entity() {
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
    let mut editor = open_sized(
        &vector(vec![rectangle.clone()]),
        false,
        &[],
        Some(Vec2::new(800.0, 900.0)),
    );
    editor.click(&drawn);

    for field in ["x", "y", "width", "height", "rotation"] {
        let before = editor.rect_of(&drawn);
        let held = entities(&editor)[0].transform;
        let at = editor
            .rect_of(&format!("infinite-canvas.transform.{field}"))
            .center();
        editor.drag(at, at + Vec2::new(20.0, 0.0));

        assert_ne!(
            entities(&editor)[0].transform,
            held,
            "dragging {field} edits the entity"
        );
        assert_ne!(editor.rect_of(&drawn), before, "dragging {field} is drawn");
    }
}
