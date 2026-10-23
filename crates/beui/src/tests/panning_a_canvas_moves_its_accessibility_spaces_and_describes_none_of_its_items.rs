use accesskit::Role;
use beui_core::app::accessibility_dump::AccessibilityDump;

use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ITEMS: [&str; 3] = ["scaled", "scaled.text", "fixed"];

#[test]
fn panning_a_canvas_moves_its_accessibility_spaces_and_describes_none_of_its_items() {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(pos2(0.0, 0.0), 2.0)));
    let document = build_node(move || {
        view! {
            <Canvas view={view}>
                <CanvasItem x=10.0 y=10.0 width=80.0 height=20.0 scales=true>
                    <Frame @test_id="scaled" fill=Color32::WHITE>
                        <Text @test_id="scaled.text">"first"</Text>
                    </Frame>
                </CanvasItem>
                <CanvasItem x=10.0 y=40.0 width=80.0 height=20.0>
                    <Frame @test_id="fixed" fill=Color32::WHITE />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    let mut dump = AccessibilityDump::new();
    dump.update(
        harness
            .frame(Vec::new())
            .accessibility_tree("Test", VIEWPORT),
    );
    harness.frame(Vec::new());
    assert_bounds_match(&harness, &dump, "the first frame");

    for (step, moved) in [
        ("a pan", CanvasView::new(pos2(25.0, 15.0), 2.0)),
        ("another pan", CanvasView::new(pos2(-30.0, 40.0), 2.0)),
    ] {
        view.set(Some(moved));
        let update = harness
            .frame(Vec::new())
            .accessibility_tree("Test", VIEWPORT);
        let work = harness.document().performance().latest.work;
        assert!(
            work.described_nodes <= 1,
            "{step} describes the canvas that changed and none of its items, but described {}",
            work.described_nodes
        );
        assert!(
            update
                .nodes
                .iter()
                .all(|(_, node)| matches!(node.role(), Role::Window | Role::GenericContainer)),
            "{step} sends only the window and the canvas's spaces, not its items: {:?}",
            update.nodes
        );
        dump.update(update);
        assert_bounds_match(&harness, &dump, step);
    }

    view.set(Some(CanvasView::new(pos2(-30.0, 40.0), 1.5)));
    dump.update(
        harness
            .frame(Vec::new())
            .accessibility_tree("Test", VIEWPORT),
    );
    assert_bounds_match(&harness, &dump, "a zoom");
}

fn assert_bounds_match(harness: &Harness, dump: &AccessibilityDump, step: &str) {
    let rendered = dump.rendered();
    for test_id in ITEMS {
        let node = harness
            .document()
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("{test_id} was built"));
        let shown = harness.rect(node);
        let reported = rendered
            .test_ids()
            .rect(test_id)
            .unwrap_or_else(|| panic!("{test_id} is in the accessibility tree after {step}"));
        let close = (reported.min - shown.min).length() < 0.01
            && (reported.max - shown.max).length() < 0.01;
        assert!(
            close,
            "after {step}, {test_id} is reported at {reported:?} but shown at {shown:?}"
        );
    }
}
