use super::*;
use block_editor_beui::beui::Vec2;
use infinite_canvas_types::CanvasColor;

#[test]
fn a_frame_background_is_picked_in_the_sidebar() {
    let board = frame(3, CanvasPoint::default(), CanvasPoint::new(320.0, 200.0));
    let id = board.id;
    let mut editor = open_sized(
        &vector([board, card()]),
        false,
        &[],
        Some(Vec2::new(900.0, 1400.0)),
    );
    let fill = |editor: &BeuiTest<CanvasApp>| {
        entities(editor)
            .into_iter()
            .find(|entity| entity.id == id)
            .and_then(|entity| entity.style.filled())
    };

    editor.click(format!("infinite-canvas.layer.{id}"));
    editor.run();
    assert_eq!(
        fill(&editor),
        None,
        "an frame follows the theme until one is picked"
    );

    editor.click("infinite-canvas.background.White");
    editor.run();
    assert_eq!(
        fill(&editor),
        Some(CanvasColor::Rgba {
            red: 255,
            green: 255,
            blue: 255,
            alpha: 255
        })
    );
    editor.snapshot("a_frame_background_is_picked_in_the_sidebar");

    editor.click("infinite-canvas.background.theme");
    editor.run();
    assert_eq!(
        fill(&editor),
        None,
        "the theme button goes back to following the theme"
    );
}
