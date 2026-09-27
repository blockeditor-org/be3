use super::*;

#[test]
fn a_narrow_canvas_opens_its_inspector_under_the_stage() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));

    assert!(
        !editor.shown("infinite-canvas.preview-region"),
        "a narrow canvas gives the stage the whole width"
    );
    editor.click(&format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.record();
    editor.click("chrome.sidebar");
    editor.run();
    editor.record();

    assert!(editor.shown("infinite-canvas.preview-region"));
    editor.snapshot("a_narrow_canvas_opens_its_inspector_under_the_stage");

    editor.click("chrome.sidebar");
    editor.run();
    assert!(!editor.shown("infinite-canvas.preview-region"));
}
