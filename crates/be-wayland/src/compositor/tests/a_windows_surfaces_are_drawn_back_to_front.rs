use smithay::reexports::wayland_server::Resource as _;
use wayland_client::Proxy as _;

use super::*;

#[test]
fn a_windows_surfaces_are_drawn_back_to_front() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();

    let (above, _above) = harness.client.subsurface(&window.surface, (10, 10));
    harness.client.attach_surface(&above, 40, 30);
    let (below, below_role) = harness.client.subsurface(&window.surface, (20, 20));
    below_role.place_below(&window.surface);
    harness.client.attach_surface(&below, 40, 30);
    window.surface.commit();
    harness.client.flush();
    harness.settle();

    let state = &harness.compositor().server().state;
    let parent = state.surface(id).expect("the window has a surface");
    let name = |surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface| {
        let id = surface.id().protocol_id();
        if *surface == parent {
            "window"
        } else if id == above.id().protocol_id() {
            "above"
        } else if id == below.id().protocol_id() {
            "below"
        } else {
            "other"
        }
    };
    let order: Vec<_> = state
        .layers(id)
        .iter()
        .map(|layer| name(&layer.surface))
        .collect();
    assert_eq!(
        order,
        ["below", "window", "above"],
        "layers are listed in the order they are painted, farthest first, so a GTK popover \
         drawn in a subsurface shows over its window"
    );
}
