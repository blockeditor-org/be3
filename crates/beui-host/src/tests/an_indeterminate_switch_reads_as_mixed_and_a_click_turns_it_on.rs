use accesskit::{Role, Toggled};

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn an_indeterminate_switch_reads_as_mixed_and_a_click_turns_it_on() {
    let _root = crate::reactive::RootScope::new();
    let on = create_signal(false);
    let (document, [switch]) = toolbar_of(|| {
        [view! {
            <Switch label="Every light" on={on} indeterminate=true />
        }]
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let tree = output.accessibility_tree("Test", VIEWPORT);
    let (_, node) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Switch)
        .expect("the switch is absent from the accessibility tree");
    assert_eq!(node.toggled(), Some(Toggled::Mixed));

    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    assert!(on.get_untracked());
}
