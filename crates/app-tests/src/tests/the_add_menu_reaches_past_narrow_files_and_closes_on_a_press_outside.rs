use super::*;

#[test]
fn the_add_menu_reaches_past_narrow_files_and_closes_on_a_press_outside() {
    let mut app = AppTest::start();
    app.run(&["resize", "720x640"]);
    app.wait(ADD_TO_MY_FILES);
    app.click(ADD_TO_MY_FILES);
    app.wait("#picker.more");
    let (files_x, _, files_width, _) = app.place("#workspace.files");
    let (more_x, _, more_width, _) = app.place("#picker.more");
    assert!(
        more_x + more_width > files_x + files_width,
        "the menu reaches past the narrow files: {more_x} + {more_width} past {files_x} + {files_width}"
    );
    app.snapshot("the_add_menu_reaches_past_narrow_files_and_closes_on_a_press_outside");

    app.click("Tab \"Settings\"");
    app.gone("#picker.more");
    assert!(
        app.shown("Tab \"Settings\" selected"),
        "the press outside that closes the menu goes on to the tab it landed on"
    );
    app.close();
}
