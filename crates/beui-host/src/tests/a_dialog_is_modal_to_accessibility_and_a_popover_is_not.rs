use accesskit::Role;

use super::*;
use crate::primitives::{List, Text};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::PopoverHandle;

#[test]
fn a_dialog_is_modal_to_accessibility_and_a_popover_is_not() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let popover = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Popover @node_ref=&popover label="Filters">|_: PopoverHandle|
                    <Text>"inside"</Text>
                </Popover>
                <Dialog open={open} title="Discard" on_dismiss={move || open.set(false)}>
                    <Text>"Discard the changes?"</Text>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let modal = |harness: &Harness, label: &str| {
        harness
            .accessible()
            .iter()
            .find(|node| node.role() == Role::Dialog && node.label() == Some(label))
            .map(|node| node.is_modal())
    };

    let trigger = harness
        .document()
        .api::<unstyled::PopoverApi>(popover.get())
        .trigger();
    harness.click(harness.center(trigger));
    harness.settle();
    assert_eq!(modal(&harness, "Filters"), Some(false), "a popover blocks nothing");

    open.set(true);
    harness.settle();
    assert_eq!(modal(&harness, "Discard"), Some(true), "a dialog blocks the whole app");
}
