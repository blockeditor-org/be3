use super::*;

#[test]
fn the_add_block_dialog_fits_a_short_window() {
    let mut app = AppTest::start();
    app.run(&["resize", "1100x480"]);
    app.click(ADD_TO_MY_FILES);
    app.wait("Dialog \"Add block\"");
    app.wait("#picker.close");
    let (_, y, _, height) = app.place("#picker.close");
    assert!(
        y + height <= 480.0,
        "the close button sits inside the window, at {y} + {height}"
    );
    app.snapshot("the_add_block_dialog_fits_a_short_window");
    app.click("#picker.close");
    app.gone("Dialog \"Add block\"");
    app.close();
}
