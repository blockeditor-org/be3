use super::*;

#[test]
fn a_phone_canvas_unfolds_its_inspector_from_the_selection_bar() {
    let rectangle = card();
    let mut editor = vector_phone(std::slice::from_ref(&rectangle));
    editor.click("dock.tab.inspector");
    editor.settle();
    assert!(
        !editor.shown("infinite-canvas.selection"),
        "tapping the shown tab folds the inspector"
    );

    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.record();
    editor.click("infinite-canvas.inspect");
    editor.settle();
    editor.record();

    assert!(
        editor.shown("infinite-canvas.selection"),
        "the selection bar's inspect button unfolds the inspector"
    );
    assert!(
        editor.rect_of("infinite-canvas.canvas").height() > 760.0 / 2.0,
        "the inspector unfolds under the stage and leaves it most of the screen"
    );
    editor.snapshot("a_phone_canvas_unfolds_its_inspector_from_the_selection_bar");
}
