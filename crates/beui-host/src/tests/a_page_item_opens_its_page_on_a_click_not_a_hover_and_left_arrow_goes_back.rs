use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_page_item_opens_its_page_on_a_click_not_a_hover_and_left_arrow_goes_back() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let stayed = create_signal(0);
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Move to" page=true>
                <unstyled::MenuItem
                    label="Count"
                    then=unstyled::MenuThen::Stay
                    on_click={move || stayed.update(|stayed| *stayed += 1)}
                />
                <unstyled::MenuItem label="Archive" />
            </unstyled::MenuItem>
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
    harness.click_with(pos, PointerButton::Secondary);
    harness.frame(Vec::new());

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let move_to = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(0);
    let at = harness.center(move_to);
    harness.frame(vec![Input::PointerMoved(at)]);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .page()
            .is_none(),
        "hovering a page item does not open its page"
    );

    harness.click(at);
    harness.frame(vec![Input::PointerLeft]);
    let page = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .page()
        .expect("clicking a page item opens its page");
    assert!(harness.document().node_rect(move_to).is_none());
    let count = harness
        .document()
        .api::<unstyled::MenuListApi>(page)
        .row_button(0);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(count)
            .focused()
            .get()
    );

    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(stayed.get_untracked(), 1);
    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(menu)
            .open(harness.document()),
        "an item that stays leaves the menu open on its page"
    );
    assert!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .page()
            .is_some()
    );

    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .page()
            .is_none()
    );
    let move_to = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(0);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(move_to)
            .focused()
            .get(),
        "going back lands on the item that opened the page"
    );
}
