use super::*;

#[test]
fn fit_all_holds_when_the_window_resizes() {
    let mut far = card();
    far.transform.center = CanvasPoint::new(3000.0, -2000.0);
    let mut editor = editor(std::slice::from_ref(&far));
    let entity = format!("infinite-canvas.entity.{}", far.id);

    editor.click("infinite-canvas.pill.fit");
    let zoom = editor.label("infinite-canvas.pill.zoom");
    editor.set_size(Vec2::new(1300.0, 860.0));

    let canvas = editor.rect_of("infinite-canvas.canvas");
    let shown = editor.rect_of(&entity);
    assert_eq!(
        editor.label("infinite-canvas.pill.zoom"),
        zoom,
        "the canvas keeps the zoom Fit all chose rather than going back to its origin"
    );
    assert!(
        (shown.center() - canvas.center()).length() < 2.0,
        "and the shape stays in the middle, at {shown:?} in {canvas:?}"
    );
}
