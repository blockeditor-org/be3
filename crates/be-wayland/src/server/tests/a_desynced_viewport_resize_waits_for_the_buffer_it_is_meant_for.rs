use super::*;

use smithay::utils::{Logical, Rectangle};

fn content(server: &crate::server::Server, id: crate::state::WindowId) -> Rectangle<f64, Logical> {
    server
        .state
        .layers(id)
        .into_iter()
        .find(|layer| layer.rect.loc == (10.0, 20.0).into())
        .expect("the content sub-surface is a layer")
        .rect
}

#[test]
fn a_desynced_viewport_resize_waits_for_the_buffer_it_is_meant_for() {
    let mut server = server();
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    client.attach(&mut server, &window, 400, 300);
    let child = client.surface();
    let subsurface = client.subsurface(&child, &window.surface);
    subsurface.set_desync();
    subsurface.set_position(10, 20);
    let viewport = client.viewport(&child);
    viewport.set_destination(200, 100);
    client.attach_surface(&child, 200, 100);
    window.surface.commit();
    client.exchange(&mut server);
    assert_eq!(content(&server, id).size, (200.0, 100.0).into());

    viewport.set_destination(300, 150);
    child.commit();
    client.exchange(&mut server);
    let layer = server
        .state
        .layers(id)
        .into_iter()
        .find(|layer| layer.rect.loc == (10.0, 20.0).into())
        .expect("the content sub-surface is a layer");
    assert_eq!(
        layer.rect.size,
        (200.0, 100.0).into(),
        "a destination committed without a new buffer does not stretch the old buffer, as Firefox's content does while its window is resized"
    );
    assert_eq!(layer.source.size, (200.0, 100.0).into());

    client.attach_surface(&child, 300, 150);
    client.exchange(&mut server);
    assert_eq!(
        content(&server, id).size,
        (300.0, 150.0).into(),
        "the destination applies once the buffer drawn for it arrives"
    );

    subsurface.set_sync();
    viewport.set_destination(150, 75);
    child.commit();
    window.surface.commit();
    client.exchange(&mut server);
    assert_eq!(
        content(&server, id).size,
        (150.0, 75.0).into(),
        "a synchronized sub-surface's destination applies with its parent's commit, buffer or not"
    );
}
