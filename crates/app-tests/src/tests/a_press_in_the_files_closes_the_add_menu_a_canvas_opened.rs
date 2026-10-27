use super::*;

#[test]
fn a_press_in_the_files_closes_the_add_menu_a_canvas_opened() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.click("#infinite-canvas.dock.block");
    app.wait("#picker.more");

    app.click(ADD_TO_MY_FILES);
    app.gone("#picker.more");
    assert!(
        !app.shown("#picker.more"),
        "the press that closes the menu goes no further, so the files open no menu of their own"
    );
    app.snapshot("a_press_in_the_files_closes_the_add_menu_a_canvas_opened");
    app.close();
}
