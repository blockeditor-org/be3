use super::*;

#[test]
fn configure_sends_the_panel_size() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);

    server.state.configure(id, (300, 200).into(), true);
    client.exchange(&mut server);

    assert_eq!(client.received.size, Some((300, 200)));
    assert!(client.received.activated, "the focused window is activated");
    assert!(
        client.received.configured.is_some(),
        "the new size arrives as a configure the client acknowledges"
    );

    server.state.configure(id, (300, 200).into(), false);
    client.exchange(&mut server);
    assert!(!client.received.activated);
}
