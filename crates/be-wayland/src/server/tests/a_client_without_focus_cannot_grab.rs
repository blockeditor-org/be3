use super::*;

const BUTTON_LEFT: u32 = 0x110;

#[test]
fn a_client_without_focus_cannot_grab() {
    let mut server = server();
    let mut focused = TestClient::connect(&mut server);
    let _pointer = focused.pointer();
    let _keyboard = focused.keyboard();
    let (focused_window, focused_id) = focused.open(&mut server);
    focused.attach(&mut server, &focused_window, 100, 60);
    let mut client = TestClient::connect(&mut server);
    let _other_keyboard = client.keyboard();
    let (window, _) = client.open(&mut server);
    client.attach(&mut server, &window, 100, 60);

    server.state.focus_keyboard(Some(focused_id));
    server
        .state
        .pointer_motion(Some((focused_id, (5.0, 5.0).into())));
    server.state.pointer_button(BUTTON_LEFT, true);
    focused.exchange(&mut server);
    client.received.serial = focused.received.serial;

    let menu = client.grabbing_popup(&mut server, &window, (0, 10), (40, 30));
    client.exchange(&mut server);
    assert!(
        client
            .received
            .seen
            .contains(&Seen::PopupDone(menu.popup.clone().unwrap())),
        "a grab from a client holding neither the keyboard nor the pointer is refused"
    );
    focused.exchange(&mut server);
    assert_eq!(
        focused.received.keyboard_surface.as_ref(),
        Some(&focused_window.surface),
        "the focused window keeps the keyboard"
    );
}
