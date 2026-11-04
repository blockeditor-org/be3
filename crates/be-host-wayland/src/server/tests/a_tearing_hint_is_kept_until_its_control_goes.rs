use wayland_protocols::wp::tearing_control::v1::client::wp_tearing_control_v1::PresentationHint;

use super::*;

#[test]
fn a_tearing_hint_is_kept_until_its_control_goes() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    assert!(!server.state.tearing(id));

    let control = client.tearing_control(&window.surface);
    control.set_presentation_hint(PresentationHint::Async);
    window.surface.commit();
    client.exchange(&mut server);
    assert!(server.state.tearing(id));

    control.destroy();
    window.surface.commit();
    client.exchange(&mut server);
    assert!(
        !server.state.tearing(id),
        "without its control the surface is back to vsync"
    );

    let again = client.tearing_control(&window.surface);
    again.set_presentation_hint(PresentationHint::Async);
    window.surface.commit();
    client.exchange(&mut server);
    assert!(
        server.state.tearing(id) && !client.disconnected(),
        "a surface whose control went may take a new one"
    );
}
