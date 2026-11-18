use super::*;
use crate::LayerPolicy;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};
use beui_view::components::overlay::OverlaysWithin;

#[test]
fn overlays_within_hears_every_open_overlay_beneath_it_but_inert_ones() {
    let _root = crate::reactive::RootScope::new();
    let within = OverlaysWithin::new();
    let menu_open = create_signal(false);
    let tip_open = create_signal(false);
    let mounted = create_signal(true);
    let document = build_node(move || {
        provide_context(within);
        view! {
            <List spacing=0.0>
                <Show value={mounted}>
                    <List spacing=0.0>
                        <Overlay
                            anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                            placement=Placement::At
                            open={menu_open}
                        >
                            <Frame width=40.0 height=30.0 />
                        </Overlay>
                        <Overlay
                            anchor=OverlayAnchor::Point(pos2(60.0, 10.0))
                            placement=Placement::At
                            policy=LayerPolicy::Inert
                            open={tip_open}
                        >
                            <Frame width=40.0 height=30.0 />
                        </Overlay>
                    </List>
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let mut set = |signal: Signal<bool>, value: bool| {
        with_installed(harness.document_mut(), |_| signal.set(value));
        harness.frame(Vec::new());
        within.any()
    };
    assert!(!within.any());
    assert!(
        !set(tip_open, true),
        "an inert overlay, a tooltip, holds nothing"
    );
    assert!(set(menu_open, true), "an open menu is heard");
    assert!(!set(menu_open, false), "and heard closing");
    assert!(set(menu_open, true));
    assert!(
        !set(mounted, false),
        "an open overlay taken away with its opener counts as closed"
    );
}
