use super::*;
use crate::primitives::{List, Text};
use crate::reactive::*;
use crate::styled::Popover;
use crate::unstyled::PopoverHandle;

/// A trapping popover closed and focus moved outside it in either order, by a button in it
/// or by its owner hearing Escape close it, leaves the focus outside: a focus asked for
/// beneath the popover lands once it closes.
#[test]
fn a_popover_closed_and_focus_moved_outside_in_either_order_leaves_the_focus_outside() {
    for (focus_first, by_escape) in [(true, false), (false, false), (true, true), (false, true)] {
        let _root = crate::reactive::RootScope::new();
        let popover = NodeRef::new();
        let inside = NodeRef::new();
        let outside = NodeRef::new();
        let wanted = create_signal(false);
        let shown = create_signal(false);
        let close = move || match focus_first {
            true => {
                wanted.set(true);
                shown.set(false);
            }
            false => {
                shown.set(false);
                wanted.set(true);
            }
        };
        let open = Binding::new(shown, move |now: bool| match now {
            true => shown.set(true),
            false => close(),
        });
        let document = build_node(move || {
            view! {
                <List spacing=0.0>
                    <unstyled::Button
                        @node_ref=&outside
                        label="Elsewhere"
                        focused={wanted}
                        on_focus_change={move |focused: bool| {
                            if !focused {
                                wanted.set(false);
                            }
                        }}
                    >|_: unstyled::ButtonHandle|
                        <Text>"Elsewhere"</Text>
                    </unstyled::Button>
                    <Popover @node_ref=&popover label="Filters" open>|handle: PopoverHandle|
                        <unstyled::Button
                            @node_ref=&inside
                            label="Go"
                            focused={handle.shown}
                            on_click={close}
                        >|_: unstyled::ButtonHandle|
                            <Text>"Go"</Text>
                        </unstyled::Button>
                    </Popover>
                </List>
            }
        });
        let mut harness = Harness::new(document);
        harness.frame(Vec::new());
        let trigger = harness
            .document()
            .api::<unstyled::PopoverApi>(popover.get())
            .trigger();
        harness.click(harness.center(trigger));
        harness.frame(Vec::new());
        match by_escape {
            true => harness.key(Key::Escape, Modifiers::NONE),
            false => harness.click(harness.center(inside.get())),
        }
        harness.frame(Vec::new());
        let order = match focus_first {
            true => "focused outside and then closed",
            false => "closed and then focused outside",
        };
        let by = match by_escape {
            true => "on Escape",
            false => "from a button in it",
        };
        assert!(!shown.get_untracked(), "{order} {by}: the popover closed");
        assert!(
            harness
                .document()
                .api::<unstyled::ButtonApi>(outside.get())
                .focused()
                .get_untracked(),
            "{order} {by}: the focus is on what it asked for outside"
        );
    }
}
