use super::*;

#[test]
fn a_toplevel_icon_from_pixels_reaches_the_window() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    let manager = client.icon_manager();
    client.exchange(&mut server);
    assert_eq!(
        client.received.protocols.icon_sizes.len(),
        2,
        "the compositor asks for the sizes its tabs draw"
    );
    server.state.take_events();

    let icon = manager.create_icon(&client.handle, ());
    let (small, _small_file) = client.icon_buffer(16, [0, 0, 0xff, 0xff]);
    let (large, _large_file) = client.icon_buffer(128, [0, 0x40, 0, 0x80]);
    let (huge, _huge_file) = client.icon_buffer(1024, [0xff, 0, 0, 0xff]);
    icon.add_buffer(&small, 1);
    icon.add_buffer(&large, 1);
    icon.add_buffer(&huge, 1);
    manager.set_icon(window.toplevel.as_ref().unwrap(), Some(&icon));
    client.exchange(&mut server);
    assert_eq!(server.state.icon(id), None, "the icon waits for the commit");

    window.surface.commit();
    client.exchange(&mut server);
    let shown = server.state.icon(id).expect("the window has an icon");
    assert_eq!(
        (shown.width(), shown.height()),
        (64, 64),
        "the closest buffer at least as large as a tab's icon is taken, and shrunk, and one too large to read is passed over"
    );
    let [red, green, blue, alpha] = shown.pixels()[..4] else {
        unreachable!("an image carries four bytes per pixel")
    };
    assert!(
        red == 0 && green.abs_diff(0x80) <= 1 && blue == 0 && alpha.abs_diff(0x80) <= 1,
        "premultiplied BGRA becomes straight RGBA, not {:?}",
        [red, green, blue, alpha]
    );
    assert!(
        server
            .state
            .take_events()
            .contains(&ServerEvent::Changed(id))
    );

    manager.set_icon(window.toplevel.as_ref().unwrap(), None);
    window.surface.commit();
    client.exchange(&mut server);
    assert_eq!(server.state.icon(id), None);
}
