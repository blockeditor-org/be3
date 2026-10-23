use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn moving_off_a_context_menu_item_clears_its_highlight() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Copy" />
            <unstyled::MenuItem label="Paste" />
        };
        [view! {
            <ContextMenu items>
                <MenuRegion @node_ref=&region />
            </ContextMenu>
        }]
    });
    let region = region.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let copy = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(0);
    let copy_pos = harness.center(copy);
    harness.frame(vec![Input::PointerMoved(copy_pos)]);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(copy)
            .focused()
            .get()
    );

    let rect = harness
        .document()
        .node_rect(copy)
        .expect("the row is laid out");
    harness.frame(vec![Input::PointerMoved(pos2(
        rect.right() + 40.0,
        rect.bottom() + 80.0,
    ))]);
    harness.frame(Vec::new());

    assert!(
        !harness
            .document()
            .api::<unstyled::ButtonApi>(copy)
            .focused()
            .get(),
        "the row the pointer left is no longer highlighted"
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(copy)
            .focused()
            .get(),
        "the keyboard still walks the menu from its top"
    );
}
