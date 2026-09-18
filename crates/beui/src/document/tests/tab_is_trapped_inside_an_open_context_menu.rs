use super::*;
use crate::reactive::{NodeRef, view};
use crate::styled::ContextMenu;

#[test]
fn tab_is_trapped_inside_an_open_context_menu() {
    let region = NodeRef::new();
    let (document, [before, menu, after]) = toolbar_of({
        let region = region.clone();
        move || {
            let items = view! {
                <unstyled::MenuItem label="Copy" />
                <unstyled::MenuItem label="Paste" />
            };
            [
                view! {
                    <LabelledButton label="Before" />
                },
                view! {
                    <ContextMenu items>
                        <MenuRegion @node_ref=&region />
                    </ContextMenu>
                },
                view! {
                    <LabelledButton label="After" />
                },
            ]
        }
    });
    let region = region.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());

    let inner = menu;
    let content = unstyled::context_menu_menu(harness.document(), inner);
    let root_focusable = unstyled::menu_list_root_focusable(harness.document(), content);

    assert_eq!(harness.document().focused_node(), Some(root_focusable));

    for _ in 0..3 {
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
        assert_eq!(harness.document().focused_node(), Some(root_focusable));
        assert!(!harness.document().focus_is_within(before));
        assert!(!harness.document().focus_is_within(after));
    }
}
