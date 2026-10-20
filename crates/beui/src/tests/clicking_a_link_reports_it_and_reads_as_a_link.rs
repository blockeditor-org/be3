use accesskit::Role;

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn clicking_a_link_reports_it_and_reads_as_a_link() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [link]) = toolbar_of(|| {
        [view! {
            <Link label="Open the grid" on_click={move || clicks.update(|clicks| *clicks += 1)} />
        }]
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let tree = output.accessibility_tree("Test", VIEWPORT);
    let link_node = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Link)
        .expect("link is absent from the accessibility tree");
    assert_eq!(link_node.1.label(), Some("Open the grid"));

    harness.click(harness.center(link));
    harness.frame(Vec::new());

    assert_eq!(clicks.get_untracked(), 1);
}
