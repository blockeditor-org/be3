use super::*;

const BUTTON_LEFT: u32 = 0x110;

#[test]
fn dismissing_popups_gives_the_keyboard_back_to_the_window() {
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
    let menu = client.grabbing_popup(&mut server, &window, (0, 10), (40, 30));
    server.state.pointer_button(BUTTON_LEFT, false);
    client.exchange(&mut server);

    server.state.dismiss_popups();
    client.exchange(&mut server);
    assert!(
        client
            .received
            .seen
            .contains(&Seen::PopupDone(menu.popup.clone().unwrap()))
    );
    assert_eq!(
        client.received.keyboard_surface.as_ref(),
        Some(&window.surface),
        "the keyboard leaves the dismissed popup at once, before the client destroys it"
    );
}
