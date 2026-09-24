use super::*;
use crate::reactive::{build, view};

#[test]
fn without_an_assistive_technology_no_accessibility_tree_is_built() {
    let document = build(|| {
        view! {
            <styled::Button variant=styled::ButtonVariant::Primary label="Save" on_click={|| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.context.set_accessibility_active(false);

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
        "turning an assistive technology on sends the whole tree"
    );
}
