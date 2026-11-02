use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::WmCapabilities;

use super::*;

#[test]
fn only_fullscreen_is_offered_to_windows() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    client.open(&mut server);

    assert_eq!(
        client.received.protocols.wm_capabilities,
        Some(vec![WmCapabilities::Fullscreen as u32]),
        "a tab can go fullscreen, but has nowhere to minimize to, no maximized size of its own and no window menu"
    );
}
