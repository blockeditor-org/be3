use super::*;

#[test]
fn a_menu_opened_from_a_popup_is_drawn_above_it() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 100, 60);
    let folder = client.popup(&mut server, &window, (0, 10), (40, 30));
    let context = client.popup(&mut server, &folder, (10, 10), (30, 20));
    let other = client.popup(&mut server, &window, (60, 10), (20, 20));

    let surfaces: Vec<_> = server
        .state
        .layers(id)
        .into_iter()
        .map(|layer| layer.surface)
        .collect();
    let at = |popup: &TestWindow| {
        surfaces
            .iter()
            .position(|surface| {
                smithay::reexports::wayland_server::Resource::id(surface).protocol_id()
                    == wayland_client::Proxy::id(&popup.surface).protocol_id()
            })
            .expect("every popup is a layer")
    };
    assert!(
        at(&folder) < at(&context),
        "a context menu opened from a folder's popup is drawn in front of it"
    );
    assert!(
        at(&context) < at(&other),
        "a popup opened later is drawn in front of the earlier ones and theirs"
    );
}
