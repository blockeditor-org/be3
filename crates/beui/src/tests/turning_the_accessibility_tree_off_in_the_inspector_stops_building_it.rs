use super::*;
use crate::reactive::{build, view};

#[test]
fn turning_the_accessibility_tree_off_in_the_inspector_stops_building_it() {
    let document = build(|| {
        view! {
            <styled::Button variant=styled::ButtonVariant::Primary label="Save" on_click={|| {}} />
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    assert!(
        output.accessibility_tree("Test", VIEWPORT).nodes.len() > 1,
        "the accessibility tree is built without being asked for"
    );

    harness.disable_accessibility();
    for _ in 0..2 {
        let output = harness.frame(Vec::new());
        assert_eq!(
            harness.document().performance().latest.work.described_nodes,
            0
        );
        assert_eq!(output.accessibility_tree("Test", VIEWPORT).nodes.len(), 1);
    }

    harness.context.set_accessibility_active(true);
    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", VIEWPORT);
    assert!(
        tree.nodes
            .iter()
            .any(|(_, node)| node.role() == accesskit::Role::Button),
        "turning the tree back on sends the whole tree"
    );
}
