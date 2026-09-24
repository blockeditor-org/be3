use super::*;

#[test]
fn a_renderer_at_an_origin_paints_the_part_of_the_document_there() {
    let near = Rect::from_min_max(pos2(10.0, 10.0), pos2(20.0, 20.0));
    let far = Rect::from_min_max(pos2(74.0, 10.0), pos2(84.0, 20.0));
    let mut target = Target::new();
    target.renderer.set_origin(vec2(SIZE as f32, 0.0));

    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(near, 0.0, Color32::WHITE);
        painter.rect_filled(far, 0.0, Color32::WHITE);
    });
    let capture = target.read();

    assert_eq!(
        capture.pixel(15, 15)[0],
        255,
        "the rectangle past the origin lands at its offset from it"
    );
    assert_eq!(capture.pixel(5, 5)[0], 0, "nothing else is painted there");

    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: far,
            background: Color32::BLACK,
        },
        |painter| painter.rect_filled(near, 0.0, Color32::WHITE),
    );
    assert_eq!(
        target.read().pixel(15, 15)[0],
        0,
        "a damaged region past the origin repaints the pixels it covers here"
    );
}
