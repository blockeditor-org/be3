use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_row_loses_its_highlight_when_the_pointer_leaves_it() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Rename" />
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
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: false,
    }]);
    harness.frame(Vec::new());

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let row = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(1);
    let button = harness.document().api::<unstyled::ButtonApi>(row);
    harness.frame(vec![Input::PointerMoved(harness.center(row))]);
    harness.frame(Vec::new());
    assert!(
        button.hovered().get_untracked() || button.focused().get_untracked(),
        "the row under the pointer is highlighted"
    );

    let away = pos2(VIEWPORT.x - 4.0, VIEWPORT.y - 4.0);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(menu)
            .open(harness.document()),
        "moving away leaves the menu open"
    );
    assert!(
        !button.hovered().get_untracked() && !button.focused().get_untracked(),
        "the row the pointer left is no longer highlighted"
    );
}
