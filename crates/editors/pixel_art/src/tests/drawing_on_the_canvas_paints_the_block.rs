use super::*;

#[test]
fn drawing_on_the_canvas_paints_the_block() {
    let (mut editor, block) = editor();

    let canvas = editor.rect_of("pixel-art.canvas");
    let artwork = editor.rect_of("pixel-art.artwork");
    assert!(
        artwork.width() > canvas.width() / 2.0 || artwork.height() > canvas.height() / 2.0,
        "the artwork must fill the space it is given rather than sitting in a corner"
    );

    let cell = artwork.width() / f32::from(block.read().unwrap().width());
    editor.click_at(Pos2::new(
        artwork.left() + cell * 2.5,
        artwork.top() + cell * 1.5,
    ));
    editor.run();
    editor.run();

    let art = block.read().unwrap();
    let painted = art
        .rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] != 0)
        .count();
    assert_eq!(painted, 1, "one pixel was painted where the pointer was");
    assert_eq!(art.pixel(2, 1), Some(PixelColor::new(0, 0, 0, 255)));
}
