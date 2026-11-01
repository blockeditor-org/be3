use beui_core::app::accessibility_dump::AccessibilityDump;

use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn an_overlay_whose_opener_moves_keeps_its_content_where_it_is_shown() {
    let _root = crate::reactive::RootScope::new();
    let above = create_signal(20.0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height={create_memo(move || Extent::px(above.get()))} />
                <Frame>
                    <Overlay
                        anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                        placement=Placement::At
                        open=true
                    >
                        <Frame @test_id="pinned" width=40.0 height=30.0 />
                    </Overlay>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let mut dump = AccessibilityDump::new();
    for height in [20.0, 60.0, 5.0] {
        above.set(height);
        dump.update(
            harness
                .frame(Vec::new())
                .accessibility_tree("Test", VIEWPORT),
        );
        let pinned = harness.find("pinned");
        let reported = dump
            .rendered()
            .test_ids()
            .rect("pinned")
            .expect("the overlay's content is in the tree");
        assert_eq!(
            reported,
            harness.rect(pinned),
            "with {height} above the overlay, its content is reported where it is shown"
        );
    }
}
