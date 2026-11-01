use super::*;

#[test]
fn fit_all_on_an_empty_canvas_returns_to_the_default_view() {
    let mut far = card();
    far.transform.center = CanvasPoint::new(3000.0, -2000.0);
    let mut editor = editor(std::slice::from_ref(&far));
    let canvas = editor.rect_of("infinite-canvas.canvas");

    editor.click("infinite-canvas.pill.fit");
    editor.run();
    editor.click("infinite-canvas.pill.zoom-in");
    editor.run();
    assert_ne!(editor.label("infinite-canvas.pill.zoom"), "100%");
    apply(
        &mut editor,
        InfiniteCanvasOperation::Remove { ids: vec![far.id] },
    );
    editor.run();
    assert!(entities(&editor).is_empty());

    editor.click("infinite-canvas.pill.fit");
    editor.run();

    assert_eq!(editor.label("infinite-canvas.pill.zoom"), "100%");
    let mut home = card();
    home.transform.center = CanvasPoint::new(0.0, 0.0);
    apply(
        &mut editor,
        InfiniteCanvasOperation::Add {
            entity: Box::new(home.clone()),
            pixels: None,
        },
    );
    editor.run();
    let shown = editor.rect_of(&format!("infinite-canvas.entity.{}", home.id));
    assert!(
        (shown.center() - canvas.center()).length() < 1.0,
        "the origin is back in the middle, a shape there at {shown:?} in {canvas:?}"
    );
    editor.snapshot("fit_all_on_an_empty_canvas_returns_to_the_default_view");
}
