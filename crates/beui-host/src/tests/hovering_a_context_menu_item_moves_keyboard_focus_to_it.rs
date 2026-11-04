use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn hovering_a_context_menu_item_moves_keyboard_focus_to_it() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Copy" />
            <unstyled::MenuItem label="Paste" />
            <unstyled::MenuItem label="Delete" />
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
    let paste = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(1);
    let delete = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(2);

    let paste_pos = harness.center(paste);
    harness.frame(vec![Input::PointerMoved(paste_pos)]);

    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(paste)
            .focused()
            .get()
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(delete)
            .focused()
            .get()
    );
}
