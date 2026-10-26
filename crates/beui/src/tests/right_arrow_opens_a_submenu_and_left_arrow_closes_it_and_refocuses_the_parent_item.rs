use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn right_arrow_opens_a_submenu_and_left_arrow_closes_it_and_refocuses_the_parent_item() {
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

    assert!(
        !harness
            .document()
            .api::<unstyled::ButtonApi>(share_button)
            .focused()
            .get()
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(share_button)
            .focused()
            .get()
    );

    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.frame(Vec::new());

    let submenu = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_submenu_content(0)
        .expect("share's submenu is built once it opens");
    let email_button = harness
        .document()
        .api::<unstyled::MenuListApi>(submenu)
        .row_button(0);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(email_button)
            .focused()
            .get()
    );
    assert!(harness.document().node_rect(email_button).is_some());

    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(share_button)
            .focused()
            .get()
    );
    assert!(harness.document().node_rect(email_button).is_none());
}
