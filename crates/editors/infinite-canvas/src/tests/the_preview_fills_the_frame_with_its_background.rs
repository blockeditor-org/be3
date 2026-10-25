use super::*;
use infinite_canvas_types::CanvasColor;

#[test]
fn the_preview_fills_the_frame_with_its_background() {
    let mut board = frame(3, CanvasPoint::default(), CanvasPoint::new(320.0, 180.0));
    board.style.fill = CanvasFill::Solid;
    board.style.fill_color = CanvasColor::Rgba {
        red: 245,
        green: 240,
        blue: 225,
        alpha: 255,
    };
    let mut editor = open(&Canvas::with_entities([board, card()]), true, &[]);
    editor.run();
    editor.snapshot("the_preview_fills_the_frame_with_its_background");
}
