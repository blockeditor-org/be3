use super::*;

#[test]
fn the_add_menu_reaches_past_narrow_files_and_closes_on_a_press_outside() {
    let mut host = HostTest::start();
    host.run(&["resize", "720x640"]);
    host.wait(NEW);
    open_add_to_my_files(&mut host);
    host.wait("#picker.more");
    let (files_x, _, files_width, _) = host.place("#workspace.files");
    let (more_x, _, more_width, _) = host.place("#picker.more");
    assert!(
        more_x + more_width > files_x + files_width,
        "the menu reaches past the narrow files: {more_x} + {more_width} past {files_x} + {files_width}"
    );
    host.snapshot("the_add_menu_reaches_past_narrow_files_and_closes_on_a_press_outside");

    host.click("Tab \"Settings\"");
    host.gone("#picker.more");
    assert!(
        host.shown("Tab \"Settings\" selected"),
        "the press outside that closes the menu goes on to the tab it landed on"
    );
    host.close();
}
