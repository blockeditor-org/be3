use super::*;

#[test]
fn on_a_phone_tapping_a_blocks_name_renames_it_in_a_sheet() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.run(&["resize", "390x800"]);
    app.click("#dock.title");
    app.wait("#rename.name");
    app.run(&["pause", "600"]);
    let (_, name_y, _, _) = app.place("#rename.name");
    let (_, handle_y, _, _) = app.place("#sheet.handle");
    assert!(
        handle_y > 100.0 && name_y > handle_y,
        "the form rises from the bottom of the screen in a sheet, its grip at {handle_y}"
    );
    app.snapshot("on_a_phone_tapping_a_blocks_name_renames_it_in_a_sheet");

    app.run(&["click", "195,40"]);
    app.gone("#rename.name");
    app.close();
}
