use super::*;

#[test]
fn the_app_menu_opens_under_its_button() {
    let mut app = AppTest::start();
    let button = "#file-tree.desktop-app-menu";
    app.click(button);
    app.wait("#app.menu.status");
    let (button_x, button_y, button_width, button_height) = app.place(button);
    let (menu_x, menu_y, menu_width, _) = app.place("#app.menu.status");
    let below = button_y + button_height;
    assert!(
        menu_y >= below - 0.5 && menu_y < below + 20.0,
        "the menu hangs just under its button rather than where the click landed, at {menu_y} under {below}"
    );
    assert!(
        menu_x <= button_x + button_width && menu_x + menu_width >= button_x,
        "and lines up with the button, from {menu_x} wide {menu_width} against {button_x} wide {button_width}"
    );
    assert!(
        !app.shown("#sheet.handle"),
        "a click opens the menu as a dropdown"
    );
    app.snapshot("the_app_menu_opens_under_its_button");
    app.key("Escape");
    app.gone("#app.menu.status");

    app.click(button);
    app.wait("#app.menu.status");
    app.click("Tab \"Settings\"");
    app.gone("#app.menu.status");
    assert!(
        app.shown("Tab \"Settings\" selected"),
        "a press outside the menu closes it and goes on to what it landed on"
    );
    app.close();
}
