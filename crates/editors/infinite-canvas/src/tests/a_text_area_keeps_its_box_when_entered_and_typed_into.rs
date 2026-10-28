use super::*;
use block_editor_beui::beui::{Key, Modifiers};
use infinite_canvas_types::{CanvasTextAlign, CanvasTextStyle, CanvasTextVerticalAlign};

#[test]
fn a_text_area_keeps_its_box_when_entered_and_typed_into() {
    let mut area = shaped(Uuid::from_u128(1), CanvasShape::Text);
    area.transform = CanvasTransform::new(
        CanvasPoint::new(0.0, -40.0),
        CanvasPoint::new(420.0, 110.0),
        0.0,
    );
    area.placeholder = "Title".into();
    area.text_style = CanvasTextStyle {
        alignment: CanvasTextAlign::Center,
        vertical_alignment: CanvasTextVerticalAlign::Middle,
        wrap: true,
        ..CanvasTextStyle::default()
    };
    let mut editor = editor(std::slice::from_ref(&area));

    editor.click(format!("infinite-canvas.entity.{}", area.id));
    editor.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        entities(&editor)[0].transform,
        area.transform,
        "entering an empty area leaves its box"
    );

    editor.type_text("Hi");
    editor.key(Key::Escape, Modifiers::NONE);

    let held = &entities(&editor)[0];
    assert_eq!(held.text.to_str_lossy(), "Hi");
    assert_eq!(held.transform, area.transform, "typing leaves its box");
    editor.snapshot("a_text_area_keeps_its_box_when_entered_and_typed_into");
}
