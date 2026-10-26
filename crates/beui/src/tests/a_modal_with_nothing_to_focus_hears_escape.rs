use super::*;
use crate::primitives::{List, Text};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::PopoverHandle;

#[test]
fn a_modal_with_nothing_to_focus_hears_escape() {
    let _root = crate::reactive::RootScope::new();
    let (popover, opener) = (NodeRef::new(), NodeRef::new());
    let open = create_signal(false);
    let shown = create_signal(false);
    let dismissed = create_signal(0);
    let aimed = create_signal(false);
    let document = build_node(move || {
        view! {
            <Popover @node_ref=&popover label="Tools" open={shown}>|_: PopoverHandle|
                <List spacing=8.0>
                    <Button
                        @node_ref=&opener
                        label="Open"
                        focused={aimed}
                        on_click={move || open.set(true)}
                    />
                    <Modal open={open} on_dismiss={move || dismissed.update(|n| *n += 1)}>
                        <Text color=Color32::WHITE>"Nothing here takes the focus"</Text>
                    </Modal>
                </List>
            </Popover>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let trigger = harness
        .document()
        .api::<unstyled::PopoverApi>(popover.get())
        .trigger();
    let at = harness.center(trigger);
    harness.click(at);
    harness.frame(Vec::new());
    aimed.set(true);
    harness.frame(Vec::new());
    assert!(harness.document().focus_is_within(opener.get()));
    open.set(true);
    harness.frame(Vec::new());
    assert!(open.get_untracked());

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(
        dismissed.get_untracked(),
        1,
        "Escape reaches the modal on top though the focus stayed in the popover it hangs from"
    );
    assert!(shown.get_untracked(), "and leaves the popover open");
}
