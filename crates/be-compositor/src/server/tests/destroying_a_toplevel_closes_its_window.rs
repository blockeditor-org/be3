use super::*;

#[test]
fn destroying_a_toplevel_closes_its_window() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    window.toplevel.as_ref().unwrap().destroy();
    window.xdg_surface.destroy();
    client.exchange(&mut server);

    assert!(
        server
            .state
            .take_events()
            .contains(&ServerEvent::Closed(id))
    );
    assert!(server.state.windows().is_empty());
}
