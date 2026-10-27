use super::*;

#[test]
fn fit_all_zooms_to_the_first_frame() {
    let slide = frame(
        1,
        CanvasPoint::new(4000.0, 3000.0),
        CanvasPoint::new(800.0, 450.0),
    );
    let mut stray = card();
    stray.id = Uuid::from_u128(2);
    stray.transform.center = CanvasPoint::new(-5000.0, 0.0);
    let mut editor = editor(&[slide, stray]);
    let canvas = editor.rect_of("infinite-canvas.canvas");

    editor.click("infinite-canvas.pill.fit");

    let shown = editor.rect_of(&format!("infinite-canvas.entity.{}", Uuid::from_u128(1)));
    assert!(
        (shown.center() - canvas.center()).length() < 2.0,
        "the frame is centred, at {shown:?} in {canvas:?}"
    );
    assert!(
        canvas.contains_rect(shown)
            && (shown.width() > canvas.width() - 60.0 || shown.height() > canvas.height() - 60.0),
        "and fills the canvas but for a margin, at {shown:?} in {canvas:?}"
    );
    editor.snapshot("fit_all_zooms_to_the_first_frame");
}
