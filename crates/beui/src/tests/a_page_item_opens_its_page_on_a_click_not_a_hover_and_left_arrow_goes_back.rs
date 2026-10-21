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
    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());

    let content = unstyled::context_menu_menu(harness.document(), menu);
    let move_to = unstyled::menu_list_row_button(harness.document(), content, 0);
    let at = harness.center(move_to);
    harness.frame(vec![Event::PointerMoved(at)]);
    harness.frame(Vec::new());
    assert!(
        unstyled::menu_list_page(harness.document(), content).is_none(),
        "hovering a page item does not open its page"
    );

    harness.click(at);
    harness.frame(vec![Event::PointerGone]);
    let page = unstyled::menu_list_page(harness.document(), content)
        .expect("clicking a page item opens its page");
    assert!(harness.document().node_rect(move_to).is_none());
    let count = unstyled::menu_list_row_button(harness.document(), page, 0);
    assert!(
        unstyled::button_state(harness.document(), count)
            .get()
            .focused
    );

    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(stayed.get_untracked(), 1);
    assert!(
        unstyled::context_menu_open(harness.document(), menu),
        "an item that stays leaves the menu open on its page"
    );
    assert!(unstyled::menu_list_page(harness.document(), content).is_some());

    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(unstyled::menu_list_page(harness.document(), content).is_none());
    assert!(
        unstyled::button_state(harness.document(), move_to)
            .get()
            .focused,
        "going back lands on the item that opened the page"
    );
}
