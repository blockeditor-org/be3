use beui_core::app::accessibility_dump::AccessibilityDump;

use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn an_escaped_overlay_under_a_moved_opener_is_described_where_it_is_shown() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=60.0 />
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame width=30.0 height=10.0 />
                    <Frame @test_id="opener" width=50.0 height=20.0>
                        <Overlay
                            anchor=OverlayAnchor::Point(pos2(40.0, 70.0))
                            placement=Placement::BelowStart
                            open=true
                        >
                            <Frame @test_id="escaped" width=40.0 height=30.0 />
                        </Overlay>
                    </Frame>
                </List>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness
        .context()
        .set_layer_space(WindowId::MAIN, Some(LAYER_SPACE));
    let mut dump = AccessibilityDump::new();
    for _ in 0..2 {
        dump.update(
            harness
                .frame(Vec::new())
                .accessibility_tree("Test", VIEWPORT),
        );
    }
    assert_eq!(harness.document().escaped_layers().len(), 1);
    let escaped = harness.find("escaped");
    let reported = dump
        .rendered()
        .test_ids()
        .rect("escaped")
        .unwrap_or_else(|| {
            panic!(
                "the escaped overlay's content is in the tree:\n{}",
                dump.rendered().text()
            )
        });
    assert_eq!(
        reported,
        harness.rect(escaped),
        "an escaped overlay is described in window coordinates, where to_host maps it to where it is drawn"
    );
}
