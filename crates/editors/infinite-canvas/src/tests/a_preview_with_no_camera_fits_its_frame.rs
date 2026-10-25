use super::*;
use infinite_canvas_types::CanvasColor;

#[test]
fn a_preview_with_no_camera_fits_its_frame() {
    let slide = frame(1, CanvasPoint::default(), CanvasPoint::new(960.0, 540.0));
    let mut corner = entity(Uuid::from_u128(2));
    corner.transform = CanvasTransform::new(
        CanvasPoint::new(-330.0, -170.0),
        CanvasPoint::new(240.0, 140.0),
        0.0,
    );
    corner.style.fill = CanvasFill::Solid;
    corner.style.fill_color = CanvasColor::Rgba {
        red: 180,
        green: 90,
        blue: 40,
        alpha: 255,
    };
    let host = EditorHost::default();
    let editor = OwnedEditor::new(host, Uuid::new_v4());
    let mut preview = BeuiTest::<CanvasApp>::preview(editor).with_size(Vec2::new(192.0, 108.0));
    preview.hold(
        None,
        CanvasContent::new(&Canvas::with_entities([slide, corner])),
    );
    preview.run();

    preview.snapshot("a_preview_with_no_camera_fits_its_frame");
}
