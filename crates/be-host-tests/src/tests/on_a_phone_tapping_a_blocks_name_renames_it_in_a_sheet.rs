use super::*;

#[test]
fn on_a_phone_tapping_a_blocks_name_renames_it_in_a_sheet() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.run(&["resize", "390x800"]);
    host.click("#dock.title");
    host.wait("#rename.name");
    host.run(&["pause", "600"]);
    let (_, name_y, _, _) = host.place("#rename.name");
    let (_, handle_y, _, _) = host.place("#sheet.handle");
    assert!(
        handle_y > 100.0 && name_y > handle_y,
        "the form rises from the bottom of the screen in a sheet, its grip at {handle_y}"
    );
    host.snapshot("on_a_phone_tapping_a_blocks_name_renames_it_in_a_sheet");

    host.run(&["click", "195,40"]);
    host.gone("#rename.name");
    host.close();
}
