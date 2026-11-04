use accesskit::Role;

use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_context_menu_key_opens_the_context_menu_around_the_focused_control() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let items = view! {
            <unstyled::MenuItem row_test_id={"hide".to_owned()} label="Hide" />
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
            .into_iter()
            .filter(|node| node.role() == Role::MenuItem)
            .collect::<Vec<_>>()
    };

    harness.key(Key::ContextMenu, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(menu_items(&harness).is_empty(), "nothing has the focus yet");

    harness.click(harness.center(harness.find("held")));
    harness.key(Key::ContextMenu, Modifiers::CTRL);
    harness.frame(Vec::new());
    assert!(
        menu_items(&harness).is_empty(),
        "Ctrl+Menu is a chord of its own"
    );

    harness.key(Key::ContextMenu, Modifiers::NONE);
    harness.frame(Vec::new());
    let items = menu_items(&harness);
    assert_eq!(
        items.len(),
        2,
        "the Menu key opens the menu of the focused control"
    );
    let control = harness.rect(harness.find("held"));
    let first = harness.rect(harness.find("hide"));
    assert!(
        first.min.y >= control.max.y - 1.0,
        "the menu opens beneath the control, as on Shift+F10: {first:?} under {control:?}"
    );
}
