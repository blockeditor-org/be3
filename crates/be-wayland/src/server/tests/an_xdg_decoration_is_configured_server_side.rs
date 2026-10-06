use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, zxdg_toplevel_decoration_v1::Mode,
};

use crate::test_client::*;

#[test]
fn an_xdg_decoration_is_configured_server_side() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let manager: ZxdgDecorationManagerV1 = client.bind("zxdg_decoration_manager_v1", 1);
    let surface = client
        .compositor
        .as_ref()
        .unwrap()
        .create_surface(&client.handle, ());
    let xdg_surface =
        client
            .wm_base
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, &client.handle, ());
    let toplevel = xdg_surface.get_toplevel(&client.handle, ());
    manager.get_toplevel_decoration(&toplevel, &client.handle, ());
    surface.commit();
    client.exchange(&mut server);

    assert!(client.received.configured.is_some());
    assert_eq!(client.received.xdg_decoration, Some(Mode::ServerSide));
}
