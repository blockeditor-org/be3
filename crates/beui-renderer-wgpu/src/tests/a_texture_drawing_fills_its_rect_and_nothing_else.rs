use super::*;

#[test]
fn a_texture_drawing_fills_its_rect_and_nothing_else() {
    let mut target = Target::new();
    let red = [255, 0, 0, 255];
    let picture = texture_of(&target, 2, &[red; 4]);
    let drawing = beui_draw::drawing(beui_draw::texture_quad::TextureDraw::new(picture));
    let rect = Rect::from_min_max(pos2(8.0, 8.0), pos2(24.0, 56.0));
    target.draw(Color32::TRANSPARENT, Repaint::Everything, |painter| {
        painter.drawing(rect, &drawing);
    });
    let capture = target.read();
    assert_eq!(capture.pixel(16, 32), red, "the texture fills its rect");
    assert_eq!(
        capture.pixel(40, 32),
        [0, 0, 0, 0],
        "nothing is drawn beside it"
    );
}
