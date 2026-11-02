use super::*;

#[test]
fn a_second_tearing_control_for_one_surface_is_an_error() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, _id) = client.open(&mut server);

    let _first = client.tearing_control(&window.surface);
    client.exchange(&mut server);
    assert!(!client.disconnected());

    let _second = client.tearing_control(&window.surface);
    client.connection.flush().expect("the client flushes");
    server.dispatch();
    client.receive();
    assert!(client.disconnected());
}
