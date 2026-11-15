use super::*;

const CONTROL: u32 = 29;
const CONTROL_MASK: u32 = 4;

#[test]
fn closing_the_window_with_the_keyboard_lets_go_of_its_keys() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _keyboard = client.keyboard();
    let (first, first_id) = client.open(&mut server);
    client.attach(&mut server, &first, 40, 30);
    let (second, second_id) = client.open(&mut server);
    client.attach(&mut server, &second, 40, 30);
    server.state.focus_keyboard(Some(first_id));

    server.state.key(CONTROL, true);
    first.destroy();
    client.exchange(&mut server);
    assert_eq!(server.state.keyboard_window(), None);
    assert!(
        server.state.keyboard().pressed_keys().is_empty(),
        "the Ctrl of a Ctrl+W that closed the window is let go with it"
    );

    client.received.keyboard.clear();
    server.state.focus_keyboard(Some(second_id));
    client.exchange(&mut server);
    assert_eq!(client.received.keyboard, [Heard::Enter(Vec::new())]);
    assert_eq!(client.received.depressed & CONTROL_MASK, 0);
}
