use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, zxdg_toplevel_decoration_v1::Mode,
};

use crate::test_client::*;

#[test]
fn asking_for_client_side_xdg_decorations_is_answered_server_side() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let manager: ZxdgDecorationManagerV1 = client.bind("zxdg_decoration_manager_v1", 1);
    let window = client.toplevel();
    let decoration =
        manager.get_toplevel_decoration(window.toplevel.as_ref().unwrap(), &client.handle, ());
    client.exchange(&mut server);
    assert_eq!(client.received.xdg_decoration, Some(Mode::ServerSide));

    client.received.xdg_decoration = None;
    decoration.set_mode(Mode::ClientSide);
    client.exchange(&mut server);

    assert_eq!(client.received.xdg_decoration, Some(Mode::ServerSide));
}
