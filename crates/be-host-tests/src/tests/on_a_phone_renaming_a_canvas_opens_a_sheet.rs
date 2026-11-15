use super::*;

/// A canvas's dock joins the workspace's, so the Rename its stacked bar's title asks for is drawn
/// by the canvas, which then has to know it is on a phone to open it as a sheet.
#[test]
fn on_a_phone_renaming_a_canvas_opens_a_sheet() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.dock");
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
    host.close();
}
