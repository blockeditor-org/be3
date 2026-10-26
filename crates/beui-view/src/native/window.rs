use beui_core::base::overlay::{OverlayNode, WindowSpec};
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;
use beui_core::window::{WindowId, WindowPlacement, WindowReport, WindowRequest, WindowRole};

use crate::reactive::{Callback, Get, Prop, ReadSignal, create_binding, with_document};

pub fn request_window(request: WindowRequest) -> bool {
    with_document(|document| document.request_window(request))
}

pub fn opens_windows() -> ReadSignal<bool> {
    with_document(|document| document.watch_opens_windows())
}

pub fn placed_windows() -> ReadSignal<bool> {
    with_document(|document| document.watch_placed_windows())
}

pub fn host_window(
    node: NodeId,
    id: WindowId,
    size: Prop<Vec2>,
    placement: Prop<WindowPlacement>,
    title: Prop<Option<String>>,
    role: Prop<WindowRole>,
    on_report: Callback<WindowReport>,
) {
    let overlay = with_document(|document| document.arena.kind_of::<OverlayNode>(node))
        .expect("an Overlay builds an overlay node");
    create_binding(move || {
        let spec = WindowSpec {
            id,
            size: size.get(),
            placement: placement.get(),
            title: title.get(),
            role: role.get(),
            on_report,
        };
        with_document(|document| document.set_overlay_window(overlay, Some(spec)));
    });
}
