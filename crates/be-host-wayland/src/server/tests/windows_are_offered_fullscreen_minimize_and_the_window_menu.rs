use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::WmCapabilities;

use super::*;

#[test]
fn windows_are_offered_fullscreen_minimize_and_the_window_menu() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    client.open(&mut server);

    assert_eq!(
        client.received.protocols.wm_capabilities,
        Some(vec![
            WmCapabilities::Fullscreen as u32,
            WmCapabilities::Minimize as u32,
            WmCapabilities::WindowMenu as u32,
        ]),
        "a window can go fullscreen, minimize to its entry and open the window menu, but has no maximized size of its own"
    );
}
