use super::*;
use crate::LayerPolicy;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn an_overlay_past_the_windows_edge_is_cut_off() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(150.0, 10.0))
                    placement=Placement::At
                    policy=LayerPolicy::Pass
                    traps_focus=false
                    open=true
                >
                    <Frame @test_id="past" width=100.0 height=20.0 />
                </Overlay>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(200.0, 100.0));
    harness.frame(Vec::new());

    let found: Vec<String> = harness
        .document()
        .overflows()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        found
            .iter()
            .any(|found| found.contains("#past") && found.contains("cut off")),
        "{found:#?}"
    );
}
