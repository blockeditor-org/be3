use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::PopoverHandle;

#[test]
fn clicking_a_popover_trigger_again_closes_it() {
    let _root = crate::reactive::RootScope::new();
    let popover = NodeRef::new();
    let document = build({
        move || {
            view! {
                <Popover @node_ref=&popover label="Filters">
                    |_: PopoverHandle| <Text string="inside" />
                </Popover>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let trigger = unstyled::popover_trigger(harness.document(), popover.get());
    let at = harness.center(trigger);

    harness.click(at);
    harness.frame(Vec::new());
    assert!(
        unstyled::popover_open(harness.document(), popover.get()),
        "the first click opens it"
    );

    harness.click(at);
    harness.frame(Vec::new());
    assert!(
        !unstyled::popover_open(harness.document(), popover.get()),
        "a second click on the trigger closes it rather than closing and opening it again"
    );

    harness.click(at);
    harness.frame(Vec::new());
    assert!(
        unstyled::popover_open(harness.document(), popover.get()),
        "a third click opens it again"
    );
}
