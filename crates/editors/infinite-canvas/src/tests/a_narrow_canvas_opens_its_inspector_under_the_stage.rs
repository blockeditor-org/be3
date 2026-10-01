use super::*;

#[test]
fn a_narrow_canvas_opens_its_inspector_under_the_stage() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));

    assert!(
        !editor.shown("infinite-canvas.selection"),
        "a narrow canvas gives the stage the whole width"
    );
    editor.click(&format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.record();
    editor.click("infinite-canvas.inspect");
    editor.run();
    editor.record();

    assert!(editor.shown("infinite-canvas.selection"));
    assert!(
        editor.rect_of("infinite-canvas.canvas").height() > 760.0 / 2.0,
        "the inspector opens low enough to leave the stage most of the screen"
    );
    assert!(
        !editor.shown("infinite-canvas.dock"),
        "the tool dock steps aside rather than covering the inspector"
    );
    editor.snapshot("a_narrow_canvas_opens_its_inspector_under_the_stage");

    let handle = editor.rect_of("sheet.handle").center();
    editor.drag(handle, handle + Vec2::new(0.0, 700.0));
    editor.settle_until("the inspector to close", |editor| {
        editor.shown("infinite-canvas.dock")
    });
    assert!(!editor.shown("infinite-canvas.selection"));
    assert!(
        editor.shown("infinite-canvas.dock"),
        "closing the inspector brings the tools back"
    );
}
