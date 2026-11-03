use super::*;

const BUTTON_LEFT: u32 = 0x110;
const KEY_ESC: u32 = 1;

#[test]
fn a_submenu_grabbing_with_its_menus_serial_takes_the_keyboard() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _pointer = client.pointer();
    let _keyboard = client.keyboard();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 100, 60);
    server.state.focus_keyboard(Some(id));
    server.state.pointer_motion(Some((id, (5.0, 5.0).into())));
    server.state.pointer_button(BUTTON_LEFT, true);
    server.state.pointer_button(BUTTON_LEFT, false);
    client.exchange(&mut server);

    let menu = client.grabbing_popup(&mut server, &window, (0, 10), (40, 30));
    let submenu = client.grabbing_popup(&mut server, &menu, (40, 0), (40, 30));
    assert_eq!(
        client.received.keyboard_surface.as_ref(),
        Some(&submenu.surface),
        "the submenu takes the keyboard from its menu"
    );

    server.state.key(KEY_ESC, true);
    client.exchange(&mut server);
    assert_eq!(
        client.received.seen.last(),
        Some(&Seen::Key(submenu.surface.clone(), KEY_ESC)),
        "keys reach the submenu"
    );
    server.state.key(KEY_ESC, false);

    submenu.destroy();
    client.exchange(&mut server);
    assert_eq!(
        client.received.keyboard_surface.as_ref(),
        Some(&menu.surface),
        "the keyboard goes back to the menu once its submenu is gone"
    );
}
