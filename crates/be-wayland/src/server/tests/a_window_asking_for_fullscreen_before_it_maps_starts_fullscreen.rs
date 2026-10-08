use crate::state::ServerEvent;
use crate::test_client::*;

#[test]
fn a_window_asking_for_fullscreen_before_it_maps_starts_fullscreen() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let surface = client
        .compositor
        .as_ref()
        .unwrap()
        .create_surface(&client.handle, ());
    let xdg_surface = client
        .wm_base
        .as_ref()
        .unwrap()
        .get_xdg_surface(&surface, &client.handle, ());
    let toplevel = xdg_surface.get_toplevel(&client.handle, ());
    toplevel.set_fullscreen(None);
    surface.commit();
    client.exchange(&mut server);

    assert!(
        client.received.fullscreen,
        "the first configure already says the window is fullscreen"
    );
    let events = server.state.take_events();
    let id = events
        .iter()
        .find_map(|event| match event {
            ServerEvent::Opened(id) => Some(*id),
            _ => None,
        })
        .expect("the toplevel opened a window");
    assert!(
        events.contains(&ServerEvent::Fullscreen(id, true)),
        "the ui hears that the window wants the whole screen"
    );
    assert!(server.state.fullscreen(id));
}
