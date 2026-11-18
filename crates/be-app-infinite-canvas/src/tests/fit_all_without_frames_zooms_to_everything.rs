use super::*;

#[test]
fn fit_all_without_frames_zooms_to_everything() {
    let mut first = card();
    first.transform.center = CanvasPoint::new(3000.0, -2000.0);
    let mut second = card();
    second.id = Uuid::from_u128(2);
    second.transform.center = CanvasPoint::new(3400.0, -1800.0);
    let mut editor = editor(&[first, second]);
    let canvas = editor.rect_of("infinite-canvas.canvas");

    editor.click("infinite-canvas.pill.fit");

    let shown = editor
        .rect_of(&format!("infinite-canvas.entity.{}", Uuid::from_u128(1)))
        .union(editor.rect_of(&format!("infinite-canvas.entity.{}", Uuid::from_u128(2))));
    assert!(
        (shown.center() - canvas.center()).length() < 2.0,
        "the shapes are centred together, at {shown:?} in {canvas:?}"
    );
    assert!(
        canvas.contains_rect(shown),
        "and all of them are on screen, at {shown:?} in {canvas:?}"
    );
    editor.snapshot("fit_all_without_frames_zooms_to_everything");
}
