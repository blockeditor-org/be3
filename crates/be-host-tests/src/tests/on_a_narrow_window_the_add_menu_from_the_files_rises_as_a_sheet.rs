use super::*;

#[test]
fn on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet() {
    let mut host = HostTest::start(Options::default());
    host.run(&["resize", "390x800"]);
    open_add_to_my_files(&mut host);
    host.wait("#picker.more");
    host.wait_still("#sheet.handle");
    let (_, handle_y, _, _) = host.place("#sheet.handle");
    let (_, menu_y, _, _) = host.place("#picker.location");
    assert!(
        handle_y >= 399.0 && menu_y > handle_y,
        "the menu rises from the bottom of the window as a sheet, no higher than the middle, \
         its grip at {handle_y} and its first row at {menu_y}"
    );
    host.snapshot("on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet");
    host.click("#picker.more");
    host.wait("#picker.search");
    host.wait_still("#sheet.handle");
    host.snapshot("on_a_narrow_window_more_blocks_from_the_files_is_a_sheet");
    host.key("Escape");
    host.gone("#picker.search");
    host.close();
}
