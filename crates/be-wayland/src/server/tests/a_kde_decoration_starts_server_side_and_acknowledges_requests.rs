use wayland_protocols_misc::server_decoration::client::{
    org_kde_kwin_server_decoration::Mode,
    org_kde_kwin_server_decoration_manager::{
        Mode as DefaultMode, OrgKdeKwinServerDecorationManager,
    },
};

use crate::test_client::*;

#[test]
fn a_kde_decoration_starts_server_side_and_acknowledges_requests() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let manager: OrgKdeKwinServerDecorationManager =
        client.bind("org_kde_kwin_server_decoration_manager", 1);
    let window = client.toplevel();
    let decoration = manager.create(&window.surface, &client.handle, ());
    client.exchange(&mut server);
    assert_eq!(
        client.received.kde_default_decoration,
        Some(DefaultMode::Server)
    );
    assert_eq!(client.received.kde_decoration, Some(Mode::Server));

    client.received.kde_decoration = None;
    decoration.request_mode(Mode::Client);
    client.exchange(&mut server);
    assert_eq!(client.received.kde_decoration, Some(Mode::Client));

    decoration.request_mode(Mode::Server);
    client.exchange(&mut server);

    assert_eq!(client.received.kde_decoration, Some(Mode::Server));
}
