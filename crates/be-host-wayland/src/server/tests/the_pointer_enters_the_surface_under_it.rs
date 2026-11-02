use super::*;

const BUTTON_LEFT: u32 = 0x110;

#[test]
fn the_pointer_enters_the_surface_under_it() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _pointer = client.pointer();
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);

    server.state.pointer_motion(Some((id, (60.0, 10.0).into())));
    client.exchange(&mut server);
    assert_eq!(
        client.received.pointer_entered, None,
        "a point outside the surface enters nothing"
    );

    server.state.pointer_motion(Some((id, (5.0, 10.0).into())));
    server.state.pointer_button(BUTTON_LEFT, true);
    server.state.pointer_button(BUTTON_LEFT, false);
    client.exchange(&mut server);
    assert_eq!(client.received.pointer_entered, Some((5.0, 10.0)));
    assert_eq!(
        client.received.buttons,
        [(BUTTON_LEFT, true), (BUTTON_LEFT, false)]
    );
}
