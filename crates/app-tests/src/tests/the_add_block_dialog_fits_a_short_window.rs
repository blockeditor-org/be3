use super::*;

#[test]
fn the_add_block_dialog_fits_a_short_window() {
    let mut app = AppTest::start();
    app.run(&["resize", "1100x480"]);
    app.click(ADD_TO_MY_FILES);
    app.wait("Dialog \"Add block\"");
    let (_, y, _, height) = app.place("Dialog \"Add block\"");
    assert!(
        y >= 0.0 && y + height <= 480.0,
        "the dialog sits inside the window, at {y} + {height}"
    );
    app.snapshot("the_add_block_dialog_fits_a_short_window");
    let (_, y, _, height) = app.place("#picker.cancel");
    assert!(
        y + height <= 480.0,
        "the close button stays in the window while the body scrolls, at {y} + {height}"
    );
    app.click("#picker.cancel");
    app.gone("Dialog \"Add block\"");
    app.close();
}
