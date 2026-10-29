use accesskit::Role;

use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_test_id_reaches_the_accessibility_tree_as_its_author_id() {
    let document = build_node(|| {
        view! {
            <List spacing=0.0>
                <Frame @test_id="panel" height=20.0 />
                <Button @test_id="go" label="Go" variant=ButtonVariant::Primary on_click={|| {}} />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let tree = output.accessibility_tree("Test", VIEWPORT);
    let named = |id: &str| {
        tree.nodes
            .iter()
            .find(|(_, node)| node.author_id() == Some(id))
            .map(|(_, node)| node.role())
    };
    assert_eq!(
        named("panel"),
        Some(Role::GenericContainer),
        "a named node with nothing else to say still reaches the tree"
    );
    assert_eq!(named("go"), Some(Role::Button));
    assert!(output.test_id_rect("panel").is_some());
    assert!(output.test_id_rect("go").is_some());

    let idle = harness.frame(Vec::new());
    assert_eq!(
        idle.accessibility_tree("Test", VIEWPORT).nodes.len(),
        1,
        "an idle frame sends no test ids again"
    );
    assert!(
        idle.test_id_rect("go").is_some(),
        "the kept tree still answers"
    );
}
