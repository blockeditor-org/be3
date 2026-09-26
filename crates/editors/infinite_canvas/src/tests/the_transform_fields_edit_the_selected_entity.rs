use super::*;
use block_editor_beui::be_block::canvas::CanvasColor;
use block_editor_beui::beui::Key;

#[test]
fn the_transform_fields_edit_the_selected_entity() {
    let rectangles: Vec<CanvasEntity> = (0..3u8)
        .map(|index| {
            let mut rectangle = entity(Uuid::from_u128(u128::from(index) + 1));
            rectangle.transform = CanvasTransform::new(
                CanvasPoint::new(f32::from(index) * 150.0 - 300.0, 0.0),
                CanvasPoint::new(100.0, 80.0),
                0.0,
            );
            rectangle.style.fill = Some(CanvasColor::Rgba {
                red: 60,
                green: 110,
                blue: 90,
                alpha: 255,
            });
            rectangle
        })
        .collect();
    let mut editor = editor(&rectangles);

    editor.click(&format!("infinite-canvas.entity.{}", rectangles[0].id));
    editor.run();
    editor.click(&format!("infinite-canvas.entity.{}", rectangles[2].id));
    editor.run();
    editor.click("infinite-canvas.transform.x");
    editor.run();
    editor.text("50");
    editor.run();
    editor.key_press(Key::Enter);
    editor.run();

    let held = entities(&editor);
    assert_eq!(held[0].transform.center.x, -300.0);
    assert_eq!(held[1].transform.center.x, -150.0);
    assert_eq!(held[2].transform.center.x, 50.0);
}
