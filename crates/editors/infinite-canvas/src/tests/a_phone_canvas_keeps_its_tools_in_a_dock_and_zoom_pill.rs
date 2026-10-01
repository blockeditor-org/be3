use super::*;

#[test]
fn a_phone_canvas_keeps_its_tools_in_a_dock_and_zoom_pill() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));

    assert!(!editor.shown("chrome.sidebar"));
    assert!(editor.shown("infinite-canvas.dock.block"));
    assert!(editor.shown("infinite-canvas.zoom-pill"));

    let before = editor.label("infinite-canvas.pill.zoom");
    editor.click("infinite-canvas.pill.zoom-in");
    editor.run();
    assert_ne!(editor.label("infinite-canvas.pill.zoom"), before);
    editor.click("infinite-canvas.pill.fit");
    editor.run();

    editor.click(&format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    assert!(editor.shown("infinite-canvas.selection-bar"));
    assert!(
        editor.rect_of("infinite-canvas.selection-bar").bottom()
            <= editor.rect_of("infinite-canvas.dock").top(),
        "the selection bar sits above the dock"
    );
    editor.snapshot("a_phone_canvas_keeps_its_tools_in_a_dock_and_zoom_pill");

    editor.set_more(true);
    assert!(
        editor
            .label("editor.more.item.canvas.delete")
            .ends_with("Delete")
    );
    let handle = editor.rect_of("sheet.handle").center();
    editor.drag(handle, handle - Vec2::new(0.0, 400.0));
    editor.run();
    editor.click("editor.more.item.canvas.delete");
    editor.run();
    assert!(entities(&editor).is_empty());
}
