use super::*;
use crate::reactive::{NodeRef, view};
use crate::styled::ContextMenu;

#[test]
fn moving_off_a_context_menu_item_clears_its_highlight() {
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of({
        let region = region.clone();
        move || {
            let items = view! {
                <unstyled::MenuItem label="Copy" />
                <unstyled::MenuItem label="Paste" />
            };
            [view! {
                <ContextMenu items>
                    <MenuRegion @node_ref=&region />
                </ContextMenu>
            }]
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

    let content = unstyled::context_menu_menu(harness.document(), menu);
    let copy = unstyled::menu_list_row_button(harness.document(), content, 0);
    let copy_pos = harness.center(copy);
    harness.frame(vec![Event::PointerMoved(copy_pos)]);
    assert!(unstyled::button_focused(harness.document(), copy).get());

    let rect = harness
        .document()
        .node_rect(copy)
        .expect("the row is laid out");
    harness.frame(vec![Event::PointerMoved(pos2(
        rect.right() + 40.0,
        rect.bottom() + 80.0,
    ))]);
    harness.frame(Vec::new());

    assert!(
        !unstyled::button_focused(harness.document(), copy).get(),
        "the row the pointer left is no longer highlighted"
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        unstyled::button_focused(harness.document(), copy).get(),
        "the keyboard still walks the menu from its top"
    );
}
