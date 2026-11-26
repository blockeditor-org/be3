use super::*;

const CONTROL: u32 = 29;
const CONTROL_MASK: u32 = 4;

#[test]
fn a_held_control_is_let_go_when_its_release_went_elsewhere() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _keyboard = client.keyboard();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);
    server.state.focus_keyboard(Some(id));

    server.state.key(CONTROL, true);
    server.state.key(CONTROL, true);
    server.state.key(CONTROL, false);
    client.exchange(&mut server);
    assert_eq!(
        client.received.keys,
        [(CONTROL, true), (CONTROL, false)],
        "a second press of a held key is dropped, so one release lets it go"
    );
    assert_eq!(client.received.depressed & CONTROL_MASK, 0);

    server.state.key(CONTROL, true);
    server.state.focus_keyboard(None);
    server.state.focus_keyboard(Some(id));
    client.exchange(&mut server);
    assert!(
        server.state.keyboard().pressed_keys().is_empty(),
        "the keyboard leaving every window lets go of what it held"
    );
    assert_eq!(client.received.depressed & CONTROL_MASK, 0);
}
