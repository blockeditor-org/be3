use super::*;

#[test]
fn frame_callbacks_wait_for_send_frames() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);

    window.surface.frame(&client.handle, ());
    window.surface.commit();
    client.exchange(&mut server);
    assert_eq!(
        client.received.frames, 0,
        "a frame callback is not answered before the window is painted"
    );

    server.state.send_frames(id);
    client.exchange(&mut server);
    assert_eq!(client.received.frames, 1);
}
