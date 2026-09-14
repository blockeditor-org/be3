use super::*;

#[test]
fn a_frame_paints_its_outline_over_its_fill() {
    let interior = Color32::from_rgb(80, 80, 80);
    let mut document = Document::new();
    let frame = document.create_frame();
    document.set_frame_style(
        frame,
        crate::base::frame::FrameStyle {
            fill: interior,
            outline: Color32::WHITE,
            outline_width: 1.0,
            outline_visible: true,
            ..Default::default()
        },
    );
    document.set_root(frame);

    let capture = capture(Color32::BLACK, |painter| {
        document.show(
            painter.ctx(),
            Rect::from_min_max(pos2(16.0, 16.0), pos2(48.0, 48.0)),
        );
    });

    assert_eq!(capture.pixel(16, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(32, 16), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(47, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(32, 47), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(32, 32), [80, 80, 80, 255]);
}
