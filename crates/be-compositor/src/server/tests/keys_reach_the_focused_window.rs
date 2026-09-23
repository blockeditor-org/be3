use super::*;

#[test]
fn keys_reach_the_focused_window() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _keyboard = client.keyboard();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);

    server.state.key(30, true);
    client.exchange(&mut server);
    assert!(
        client.received.keys.is_empty(),
        "a window without the keyboard hears no keys"
    );

    server.state.focus_keyboard(Some(id));
    server.state.key(30, true);
    server.state.key(30, false);
    client.exchange(&mut server);
    assert!(client.received.keyboard_entered);
    assert_eq!(client.received.keys, [(30, true), (30, false)]);

    server.state.focus_keyboard(None);
    client.exchange(&mut server);
    assert!(!client.received.keyboard_entered);
}
