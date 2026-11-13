use super::*;

#[test]
fn the_host_menu_opens_under_its_button() {
    let mut host = HostTest::start(Options::default());
    let button = "#file-tree.host-menu";
    // The menu's first row is the sync status: a host just started may still be sending what it
    // made, which the painting would show as syncing.
    host.synced();
    host.click(button);
    host.wait("#host.menu.status");
    host.wait("All changes saved");
    let (button_x, button_y, button_width, button_height) = host.place(button);
    let (menu_x, menu_y, menu_width, _) = host.place("#host.menu.status");
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
        !host.shown("#sheet.handle"),
        "a click opens the menu as a dropdown"
    );
    host.snapshot("the_host_menu_opens_under_its_button");
    host.key("Escape");
    host.gone("#host.menu.status");

    host.click(button);
    host.wait("#host.menu.status");
    host.click(button);
    assert!(
        !host.shown("#host.menu.status"),
        "a second click on the button closes its menu rather than closing it and opening it again"
    );

    host.click(button);
    host.wait("#host.menu.status");
    host.click("Tab \"Settings\"");
    host.gone("#host.menu.status");
    assert!(
        host.shown("Tab \"Settings\" selected"),
        "a press outside the menu closes it and goes on to what it landed on"
    );
    host.close();
}
