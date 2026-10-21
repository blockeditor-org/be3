use super::*;

#[test]
fn the_canvas_dock_opens_the_add_menu_at_its_block_button() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("#infinite-canvas.canvas");
    let stage = app.place("#infinite-canvas.canvas");
    let (button_x, button_y, button_width, _) = app.place("#infinite-canvas.dock.block");

    app.click("#infinite-canvas.dock.block");
    app.wait("#picker.more");
    let (more_x, _, _, _) = app.place("#picker.more");
    let (_, import_y, _, import_height) = app.place("#picker.import");
    assert!(
        import_y + import_height <= button_y && import_y + import_height > button_y - 40.0,
        "the menu stands just above the button, ending at {} over {button_y}",
        import_y + import_height
    );
    assert!(
        more_x >= button_x && more_x < button_x + button_width,
        "and starts at the button, at {more_x} beside {button_x}"
    );
    assert_eq!(
        app.place("#infinite-canvas.canvas"),
        stage,
        "the open menu moves nothing under it"
    );
    app.snapshot("the_canvas_dock_opens_the_add_menu_at_its_block_button");
    app.key("Escape");
    app.gone("#picker.more");
    app.close();
}
