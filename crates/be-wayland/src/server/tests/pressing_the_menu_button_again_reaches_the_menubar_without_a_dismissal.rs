use super::*;

const BUTTON_LEFT: u32 = 0x110;

#[test]
fn pressing_the_menu_button_again_reaches_the_menubar_without_a_dismissal() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _pointer = client.pointer();
    let _keyboard = client.keyboard();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 100, 60);
    server.state.focus_keyboard(Some(id));
    server.state.pointer_motion(Some((id, (5.0, 5.0).into())));
    server.state.pointer_button(BUTTON_LEFT, true);
    client.exchange(&mut server);
    let _menu = client.grabbing_popup(&mut server, &window, (0, 10), (40, 30));
    server.state.pointer_button(BUTTON_LEFT, false);
    client.exchange(&mut server);
    client.received.seen.clear();

    server.state.pointer_button(BUTTON_LEFT, true);
    server.state.pointer_button(BUTTON_LEFT, false);
    client.exchange(&mut server);
    assert_eq!(
        client.received.seen,
        [
            Seen::Button(window.surface.clone(), true),
            Seen::Button(window.surface.clone(), false),
        ],
        "the menubar sees the click on its open menu's button and closes the menu itself; \
         a popup_done before the press would make it open the menu again"
    );
}
