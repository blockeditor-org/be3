use super::*;

#[test]
fn a_press_on_the_files_add_button_closes_a_canvas_menu_and_opens_theirs() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.click("#infinite-canvas.dock.block");
    app.wait("#picker.more");

    let (canvas_more_x, _, _, _) = app.place("#picker.more");
    app.click(ADD_TO_MY_FILES);
    app.wait("#picker.more");
    let (files_x, _, files_width, _) = app.place("#workspace.files");
    let (more_x, _, _, _) = app.place("#picker.more");
    assert_eq!(app.count("#picker.more"), 1, "the canvas's menu closes");
    assert!(
        more_x < files_x + files_width && more_x != canvas_more_x,
        "and the press goes on to the files' button, which opens their own menu at {more_x}"
    );
    app.snapshot("a_press_on_the_files_add_button_closes_a_canvas_menu_and_opens_theirs");
    app.close();
}
