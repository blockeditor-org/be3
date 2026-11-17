use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::PopoverHandle;

#[test]
fn a_kept_popover_gives_the_focus_back_to_its_trigger_through_its_layer() {
    let _root = crate::reactive::RootScope::new();
    let popover = NodeRef::new();
    let inside = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Popover @node_ref=&popover label="Filters">|handle: PopoverHandle|
                <unstyled::Button @node_ref=&inside label="Apply" focused={handle.shown}>|_: unstyled::ButtonHandle|
                    <Text>"Apply"</Text>
                </unstyled::Button>
            </Popover>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let trigger = harness
        .document()
        .api::<unstyled::PopoverApi>(popover.get())
        .trigger();

    for round in ["first", "kept"] {
        harness.click(harness.center(trigger));
        harness.frame(Vec::new());
        assert!(
            harness
                .document()
                .api::<unstyled::ButtonApi>(inside.get())
                .focused()
                .get_untracked(),
            "the {round} opening focuses what is inside"
        );

        harness.key(Key::Escape, Modifiers::NONE);
        harness.frame(Vec::new());
        assert!(
            !harness
                .document()
                .api::<unstyled::PopoverApi>(popover.get())
                .open(),
            "Escape closes the {round} opening"
        );
        assert!(
            harness
                .document()
                .api::<unstyled::ButtonApi>(trigger)
                .focused()
                .get_untracked(),
            "the {round} close gives the focus back to the trigger"
        );
    }
}
