use super::*;

#[test]
fn on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet() {
    let mut app = AppTest::start();
    app.run(&["resize", "390x800"]);
    open_add_to_my_files(&mut app);
    app.wait("#picker.more");
    app.run(&["pause", "600"]);
    let (_, menu_y, _, _) = app.place("#picker.location");
    let (_, import_y, _, import_height) = app.place("#picker.import");
    assert!(
        menu_y > 400.0 && import_y + import_height <= 800.0,
        "the menu rises from the bottom of the window as a sheet: {menu_y} to {}",
        import_y + import_height
    );
    app.snapshot("on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet");
    app.click("#picker.more");
    app.wait("#picker.search");
    app.run(&["pause", "600"]);
    app.snapshot("on_a_narrow_window_more_blocks_from_the_files_is_a_sheet");
    app.key("Escape");
    app.gone("#picker.search");
    app.close();
}
