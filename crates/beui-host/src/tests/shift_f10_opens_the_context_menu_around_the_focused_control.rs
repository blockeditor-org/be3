use accesskit::Role;

use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn shift_f10_opens_the_context_menu_around_the_focused_control() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let items = view! {
            <unstyled::MenuItem label="Hide" />
            <unstyled::MenuItem label="Close" />
        };
        view! {
            <List spacing=0.0>
                <ContextMenu items>
                    <Button @test_id={"held"} label="Window" />
                </ContextMenu>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let menu_items = |harness: &Harness| {
        harness
            .accessible()
            .iter()
            .filter(|node| node.role() == Role::MenuItem)
            .count()
    };
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };

    harness.key(Key::F10, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(menu_items(&harness), 0, "nothing has the focus yet");

    harness.click(harness.center(harness.find("held")));
    harness.key(Key::F10, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(menu_items(&harness), 0, "F10 alone is not the menu key");

    harness.key(Key::F10, shift);
    harness.frame(Vec::new());
    assert_eq!(
        menu_items(&harness),
        2,
        "Shift+F10 opens the menu of the focused control"
    );
}
