use super::*;
use block_editor_beui::be_block::canvas::CanvasColor;
use block_editor_beui::beui::Vec2;

#[test]
fn the_preview_shows_the_first_artboard() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform = CanvasTransform::new(
        CanvasPoint::new(400.0, 260.0),
        CanvasPoint::new(160.0, 100.0),
        0.0,
    );
    rectangle.style.fill = Some(CanvasColor::Rgba {
        red: 180,
        green: 90,
        blue: 40,
        alpha: 255,
    });
    let mut far = rectangle.clone();
    far.id = Uuid::from_u128(2);
    far.transform.center = CanvasPoint::new(3000.0, 3000.0);
    let board = artboard(
        3,
        rectangle.transform.center,
        CanvasPoint::new(320.0, 200.0),
    );
    let mut editor = open(&Canvas::with_entities([board, rectangle, far]), true, &[]);
    editor.run();

    assert_eq!(editor.intrinsic_size(), Some(Vec2::new(320.0, 200.0)));
    editor.snapshot("the_preview_shows_the_first_artboard");
}
