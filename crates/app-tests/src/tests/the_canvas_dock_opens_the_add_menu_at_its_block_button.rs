use super::*;

#[test]
fn the_canvas_dock_opens_the_add_menu_at_its_block_button() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let stage = host.place("#infinite-canvas.canvas");
    let (button_x, button_y, button_width, _) = host.place("#infinite-canvas.dock.block");

    host.click("#infinite-canvas.dock.block");
    host.wait("#picker.more");
    let (more_x, _, _, _) = host.place("#picker.more");
    let (_, import_y, _, import_height) = host.place("#picker.import");
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
        host.place("#infinite-canvas.canvas"),
        stage,
        "the open menu moves nothing under it"
    );
    host.snapshot("the_canvas_dock_opens_the_add_menu_at_its_block_button");
    host.key("Escape");
    host.gone("#picker.more");
    host.close();
}
