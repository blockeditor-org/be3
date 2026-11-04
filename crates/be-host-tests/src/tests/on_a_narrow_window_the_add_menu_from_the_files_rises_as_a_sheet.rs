use super::*;

#[test]
fn on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet() {
    let mut host = HostTest::start();
    host.run(&["resize", "390x800"]);
    open_add_to_my_files(&mut host);
    host.wait("#picker.more");
    host.run(&["pause", "600"]);
    let (_, menu_y, _, _) = host.place("#picker.location");
    let (_, import_y, _, import_height) = host.place("#picker.import");
    assert!(
        menu_y > 400.0 && import_y + import_height <= 800.0,
        "the menu rises from the bottom of the window as a sheet: {menu_y} to {}",
        import_y + import_height
    );
    host.snapshot("on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet");
    host.click("#picker.more");
    host.wait("#picker.search");
    host.run(&["pause", "600"]);
    host.snapshot("on_a_narrow_window_more_blocks_from_the_files_is_a_sheet");
    host.key("Escape");
    host.gone("#picker.search");
    host.close();
}
