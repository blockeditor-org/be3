use super::*;

#[test]
fn a_shortcuts_inhibitor_is_active_only_while_its_window_has_the_keyboard() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _keyboard = client.keyboard();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);

    let _inhibitor = client.inhibit_shortcuts(&window.surface);
    client.exchange(&mut server);
    assert!(server.state.inhibits_shortcuts(id));
    assert_eq!(
        client.received.protocols.inhibitor_active, None,
        "a window without the keyboard inhibits nothing yet"
    );

    server.state.focus_keyboard(Some(id));
    client.exchange(&mut server);
    assert_eq!(client.received.protocols.inhibitor_active, Some(true));

    server.state.restore_shortcuts(id);
    client.exchange(&mut server);
    assert_eq!(
        client.received.protocols.inhibitor_active,
        Some(false),
        "the person can take the shortcuts back"
    );
    assert!(!server.state.inhibits_shortcuts(id));

    server.state.focus_keyboard(None);
    server.state.focus_keyboard(Some(id));
    client.exchange(&mut server);
    assert_eq!(
        client.received.protocols.inhibitor_active,
        Some(true),
        "focusing the window again lets it inhibit again"
    );

    server.state.focus_keyboard(None);
    client.exchange(&mut server);
    assert_eq!(client.received.protocols.inhibitor_active, Some(false));
    assert!(
        server.state.inhibits_shortcuts(id),
        "losing the keyboard does not take back what the window asked for"
    );
}
