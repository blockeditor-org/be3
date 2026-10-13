use super::*;

const BUTTON_LEFT: u32 = 0x110;
const KEY_ESC: u32 = 1;

#[test]
fn a_grabbing_popup_holds_the_keyboard_until_it_closes() {
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
    assert_eq!(
        client.received.keyboard_surface.as_ref(),
        Some(&menu.surface),
        "the grabbing popup takes the keyboard"
    );

    server.state.key(KEY_ESC, true);
    server.state.key(KEY_ESC, false);
    client.exchange(&mut server);
    assert_eq!(
        client.received.seen.last(),
        Some(&Seen::Key(menu.surface.clone(), KEY_ESC)),
        "Escape reaches the popup, which closes itself"
    );

    menu.destroy();
    client.exchange(&mut server);
    assert_eq!(
        client.received.keyboard_surface.as_ref(),
        Some(&window.surface),
        "the keyboard goes back to the window once the popup is gone"
    );
    assert!(
        !client
            .received
            .seen
            .iter()
            .any(|seen| matches!(seen, Seen::PopupDone(_))),
        "a popup the client closed is never dismissed"
    );

    server.state.key(KEY_ESC, true);
    client.exchange(&mut server);
    assert_eq!(
        client.received.seen.last(),
        Some(&Seen::Key(window.surface.clone(), KEY_ESC))
    );
}
