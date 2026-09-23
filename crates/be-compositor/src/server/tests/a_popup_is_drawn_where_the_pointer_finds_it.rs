use super::*;

#[test]
fn a_popup_is_drawn_where_the_pointer_finds_it() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let _pointer = client.pointer();
    let (window, id) = client.open(&mut server);
    window.xdg_surface.set_window_geometry(10, 10, 40, 30);
    client.attach(&mut server, &window, 60, 50);
    let popup = client.popup(&mut server, &window, (5, 5), (20, 10));

    let layers = server.state.layers(id);
    assert_eq!(layers.len(), 2);
    assert_eq!(
        layers[0].rect.loc,
        (-10.0, -10.0).into(),
        "the window's shadow sits outside the panel"
    );
    assert_eq!(
        layers[1].rect.loc,
        (5.0, 5.0).into(),
        "the popup hangs from the anchor, measured from the window's geometry"
    );

    server.state.pointer_motion(Some((id, (7.0, 8.0).into())));
    client.exchange(&mut server);
    assert_eq!(
        client.received.pointer_surface.as_ref(),
        Some(&popup.surface)
    );
    assert_eq!(client.received.pointer_entered, Some((2.0, 3.0)));
}
