use accesskit::{Role, Toggled};

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn an_indeterminate_switch_reads_as_mixed_and_a_click_turns_it_on() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());
    let (document, [switch]) = toolbar_of(|| {
        [view! {
            <Switch
                label="Every light"
                on=false
                indeterminate=true
                on_change={move |on| set_changes.update(|changes| changes.push(on))}
            />
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

    assert_eq!(changes.get_untracked(), [true]);
}
