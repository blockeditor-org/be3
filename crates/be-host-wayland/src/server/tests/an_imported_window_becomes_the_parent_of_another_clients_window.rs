use super::*;

#[test]
fn an_imported_window_becomes_the_parent_of_another_clients_window() {
    let mut server = server();
    let mut program = TestClient::connect(&mut server);
    let mut portal = TestClient::connect(&mut server);
    let (window, parent) = program.open(&mut server);
    program.attach(&mut server, &window, 40, 30);
    let (dialog, child) = portal.open(&mut server);

    let exported = program.export(&window);
    program.exchange(&mut server);
    let handle = program
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
        server
            .state
            .take_events()
            .contains(&ServerEvent::Changed(child)),
        "the ui hears of the new parent"
    );

    exported.destroy();
    program.exchange(&mut server);
    portal.exchange(&mut server);
    assert_eq!(
        server.state.parent(child),
        None,
        "a window that is no longer exported parents nothing"
    );
}
