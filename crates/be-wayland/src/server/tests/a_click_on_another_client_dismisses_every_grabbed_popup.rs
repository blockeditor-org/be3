use super::*;

const BUTTON_LEFT: u32 = 0x110;

#[test]
fn a_click_on_another_client_dismisses_every_grabbed_popup() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _pointer = client.pointer();
    let _keyboard = client.keyboard();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 100, 60);
    let mut other = TestClient::connect(&mut server);
    let _other_pointer = other.pointer();
    let (other_window, other_id) = other.open(&mut server);
    other.attach(&mut server, &other_window, 100, 60);

    server.state.focus_keyboard(Some(id));
    server.state.pointer_motion(Some((id, (5.0, 5.0).into())));
    server.state.pointer_button(BUTTON_LEFT, true);
    client.exchange(&mut server);
    let menu = client.grabbing_popup(&mut server, &window, (0, 10), (40, 30));
    server.state.pointer_button(BUTTON_LEFT, false);
    server.state.pointer_motion(Some((id, (10.0, 20.0).into())));
    server.state.pointer_button(BUTTON_LEFT, true);
    client.exchange(&mut server);
    assert_eq!(
        client.received.seen.last(),
        Some(&Seen::Button(menu.surface.clone(), true)),
        "a press inside the menu reaches it"
    );
    let submenu = client.grabbing_popup(&mut server, &menu, (40, 0), (40, 30));
    server.state.pointer_button(BUTTON_LEFT, false);
    client.exchange(&mut server);
    assert_eq!(
        client.received.keyboard_surface.as_ref(),
        Some(&submenu.surface),
        "the keyboard follows the topmost popup"
    );
    client.received.seen.clear();

    server.state.pointer_motion(Some((other_id, (5.0, 5.0).into())));
    server.state.pointer_button(BUTTON_LEFT, true);
    server.state.pointer_button(BUTTON_LEFT, false);
    client.exchange(&mut server);
    other.exchange(&mut server);
    for popup in [&menu, &submenu] {
        let done = Seen::PopupDone(popup.popup.clone().unwrap());
        assert!(
            client.received.seen.contains(&done),
            "a press on another client dismisses the whole chain"
        );
    }
    assert!(
        !client.received.seen.iter().any(|seen| matches!(
            seen,
            Seen::Button(surface, true) if *surface == window.surface
        )),
        "the press outside reaches none of the menu's client's surfaces"
    );
}
