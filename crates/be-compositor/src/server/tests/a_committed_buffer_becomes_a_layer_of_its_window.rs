use super::*;

#[test]
fn a_committed_buffer_becomes_a_layer_of_its_window() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 40, 30);

    assert!(
        server
            .state
            .take_events()
            .contains(&ServerEvent::Committed(id)),
        "a commit with a buffer tells the ui the window changed"
    );
    let layers = server.state.layers(id);
    assert_eq!(layers.len(), 1, "one surface with one buffer is one layer");
    assert_eq!(layers[0].rect.size, (40.0, 30.0).into());
    assert_eq!(layers[0].rect.loc, (0.0, 0.0).into());
    assert_eq!(Some(&layers[0].surface), server.state.surface(id).as_ref());
    assert_eq!(server.state.take_committed().len(), 2);
}
