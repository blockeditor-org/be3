use accesskit::{Role, Toggled};

use super::*;
use crate::reactive::view;
use crate::styled::Switch;

#[test]
fn an_indeterminate_switch_reads_as_mixed_and_a_click_turns_it_on() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [switch]) = toolbar_of(|| {
        [view! {
            <Switch
                label="Every light"
                on=false
                indeterminate=true
                on_change={move |on| sink.borrow_mut().push(on)}
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

    assert_eq!(*changes.borrow(), [true]);
}
