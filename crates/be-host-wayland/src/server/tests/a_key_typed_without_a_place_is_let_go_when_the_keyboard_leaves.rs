use super::*;

#[test]
fn a_key_typed_without_a_place_is_let_go_when_the_keyboard_leaves() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _keyboard = client.keyboard();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);
    server.state.focus_keyboard(Some(id));

    assert!(server.state.placeless_key(beui::Key::A, None, true));
    server.state.focus_keyboard(None);
    server.state.focus_keyboard(Some(id));
    client.exchange(&mut server);
    client.received.keys.clear();
    assert!(
        !server.state.placeless_key(beui::Key::A, None, false),
        "the keyboard leaving every window let go of the key, so its release is no one's"
    );
    client.exchange(&mut server);
    assert!(client.received.keys.is_empty());
}
