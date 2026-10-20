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
    app.run(&["scroll", "Label value=\"Add block\"", "600"]);
    let (_, y, _, height) = app.place("#picker.close");
    assert!(
        y + height <= 480.0,
        "scrolling the dialog brings its close button into the window, at {y} + {height}"
    );
    app.click("#picker.close");
    app.gone("Dialog \"Add block\"");
    app.close();
}
