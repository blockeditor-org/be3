use super::*;

#[test]
fn the_add_menu_opens_at_the_files_button_that_asked() {
    let mut host = HostTest::start();
    open_add_to_my_files(&mut host);
    host.wait("#picker.more");
    let (button_x, button_y, _, button_height) = host.place(NEW);
    let (menu_x, menu_y, _, _) = host.place("#picker.location");
    assert!(
        menu_y >= button_y + button_height && menu_y < button_y + button_height + 40.0,
        "the menu hangs just under the button, at {menu_y} under {button_y} + {button_height}"
    );
    assert!(
        (menu_x - button_x).abs() < 8.0,
        "and starts under the button, at {menu_x} beside {button_x}"
    );
    host.snapshot("the_add_menu_opens_at_the_files_button_that_asked");
    host.key("Escape");
    host.gone("#picker.more");
    host.close();
}
