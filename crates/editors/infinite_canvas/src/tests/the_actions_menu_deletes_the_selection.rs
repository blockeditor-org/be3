use super::*;
use block_editor_plugin::be_block::canvas::CanvasColor;

#[test]
fn the_actions_menu_deletes_the_selection() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(180.0, 120.0), 0.0);
    rectangle.style.fill = Some(CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    });
    let mut editor = editor(std::slice::from_ref(&rectangle));

    editor.click(&format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.click("infinite-canvas.delete");
    editor.run();

    assert!(entities(&editor).is_empty());
}
