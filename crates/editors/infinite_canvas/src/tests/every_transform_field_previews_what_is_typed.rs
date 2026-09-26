use super::*;
use block_editor_beui::be_block::canvas::CanvasColor;
use block_editor_beui::beui::Key;

#[test]
fn every_transform_field_previews_what_is_typed() {
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
    let resting = editor.rect_of(&drawn);

    for (field, typed) in [
        ("x", "40"),
        ("y", "40"),
        ("width", "60"),
        ("height", "60"),
        ("rotation", "45"),
    ] {
        editor.click(&format!("infinite-canvas.transform.{field}"));
        editor.run();
        editor.text(typed);
        editor.run();

        assert_ne!(
            editor.rect_of(&drawn),
            resting,
            "typing into {field} is drawn before it is committed"
        );

        editor.key_press(Key::Escape);
        editor.run();

        assert_eq!(
            editor.rect_of(&drawn),
            resting,
            "escape in {field} puts it back"
        );
    }
}
