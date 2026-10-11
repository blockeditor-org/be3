use smithay::reexports::wayland_protocols::wp::content_type::v1::server::wp_content_type_v1::Type;
use wayland_protocols::wp::content_type::v1::client::wp_content_type_v1;

use super::*;

#[test]
fn a_content_type_hint_is_kept_for_its_window() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    assert_eq!(server.state.content_type(id), Type::None);

    let hint = client.content_type(&window.surface);
    hint.set_content_type(wp_content_type_v1::Type::Game);
    client.exchange(&mut server);
    assert_eq!(
        server.state.content_type(id),
        Type::None,
        "the hint waits for the commit"
    );

    window.surface.commit();
    client.exchange(&mut server);
    assert_eq!(server.state.content_type(id), Type::Game);
}
