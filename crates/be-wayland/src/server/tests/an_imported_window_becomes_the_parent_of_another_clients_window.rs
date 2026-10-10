use super::*;

#[test]
fn an_imported_window_becomes_the_parent_of_another_clients_window() {
    let mut server = server();
    let mut app = TestClient::connect(&mut server);
    let mut portal = TestClient::connect(&mut server);
    let (window, parent) = app.open(&mut server);
    let (dialog, child) = portal.open(&mut server);

    let exported = app.export(&window);
    app.exchange(&mut server);
    let handle = app
        .received
        .protocols
        .exported
        .clone()
        .expect("the exporter names a handle");

    let imported = portal.import(handle);
    imported.set_parent_of(&dialog.surface);
    portal.exchange(&mut server);
    assert_eq!(server.state.parent(child), Some(parent));
    assert!(
        server.state.take_events().contains(&ServerEvent::Changed(child)),
        "the ui hears of the new parent"
    );

    exported.destroy();
    app.exchange(&mut server);
    portal.exchange(&mut server);
    assert_eq!(
        server.state.parent(child),
        None,
        "a window that is no longer exported parents nothing"
    );
}
