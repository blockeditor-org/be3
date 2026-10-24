use super::*;
use block_editor_beui::beui::{Key, Modifiers, Vec2};
use infinite_canvas_types::CanvasColor;

#[test]
fn every_transform_field_previews_what_is_typed() {
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
    editor.run();
    let resting = editor.rect_of(&drawn);

    for (field, typed) in [
        ("x", "40"),
        ("y", "40"),
        ("width", "60"),
        ("height", "60"),
        ("rotation", "45"),
    ] {
        editor.click(format!("infinite-canvas.transform.{field}"));
        editor.run();
        editor.type_text(typed);
        editor.run();

        assert_ne!(
            editor.rect_of(&drawn),
            resting,
            "typing into {field} is drawn before it is committed"
        );

        editor.key(Key::Escape, Modifiers::NONE);
        editor.run();

        assert_eq!(
            editor.rect_of(&drawn),
            resting,
            "escape in {field} puts it back"
        );
    }
}
