use super::*;

#[test]
fn drawing_on_the_canvas_paints_the_block() {
    let (mut editor, block) = editor();

    let canvas = editor.rect_of("pixel-art.canvas");
    editor.click_at(Pos2::new(canvas.left() + 2.5, canvas.top() + 1.5));
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
