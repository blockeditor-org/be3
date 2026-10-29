use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn hovering_a_menu_item_with_children_opens_its_submenu_without_a_click() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Share">
                <unstyled::MenuItem label="Email" />
                <unstyled::MenuItem label="Link" />
            </unstyled::MenuItem>
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
    let share_button = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(0);
    assert!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .row_submenu_content(0)
            .is_none()
    );

    let share_pos = harness.center(share_button);
    harness.frame(vec![Input::PointerMoved(share_pos)]);

    let submenu = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_submenu_content(0)
        .expect("share's submenu is built once it opens");
    let email_button = harness
        .document()
        .api::<unstyled::MenuListApi>(submenu)
        .row_button(0);

    assert!(harness.document().node_rect(email_button).is_some());
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(email_button)
            .focused()
            .get()
    );
}
