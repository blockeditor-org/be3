use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Opens the host menu from the files' button and paints it as `painted`, then closes it with
/// Escape, a second click on the button, and a press outside it, as
/// `the_host_menu_opens_under_its_button` does.
fn open_the_host_menu(host: &mut HostTest, painted: &str) {
    let button = "#file-tree.host-menu";
    host.click(button);
    host.wait("#host.menu.status");
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
    host.synced();
    let (_, menu_top, _, menu_height) = host.place("#host-menu.child");
    let (_, last_y, _, last_height) = host.place("#host.menu.inspector");
    assert!(
        last_y + last_height <= menu_top + menu_height + 0.5,
        "the menu grows to fit every row rather than scrolling: its last row ends at {} inside {}",
        last_y + last_height,
        menu_top + menu_height
    );
    host.snapshot(painted);
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
    host.click("Tab \"Files\"");
}

#[test]
fn be_root_in_place_of_the_hosts_ui_opens_the_host_menu_under_its_button() {
    // be-host's own UI opens its menu under the files' button, grown to fit every row
    // (`the_host_menu_opens_under_its_button`). Started again with the root in place of its
    // UI, the root draws the same menu in the same place, closed the same ways.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    open_the_host_menu(&mut host, "the_host_opens_its_menu_under_its_button");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    open_the_host_menu(&mut host, "be_root_opens_the_host_menu_under_its_button");
    host.close();
}
