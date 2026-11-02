use super::*;

#[test]
fn the_add_menu_opens_at_the_files_button_that_asked() {
    let mut app = AppTest::start();
    open_add_to_my_files(&mut app);
    app.wait("#picker.more");
    let (button_x, button_y, _, button_height) = app.place(NEW);
    let (menu_x, menu_y, _, _) = app.place("#picker.location");
    assert!(
        menu_y >= button_y + button_height && menu_y < button_y + button_height + 40.0,
        "the menu hangs just under the button, at {menu_y} under {button_y} + {button_height}"
    );
    assert!(
        (menu_x - button_x).abs() < 8.0,
        "and starts under the button, at {menu_x} beside {button_x}"
    );
    app.snapshot("the_add_menu_opens_at_the_files_button_that_asked");
    app.key("Escape");
    app.gone("#picker.more");
    app.close();
}
