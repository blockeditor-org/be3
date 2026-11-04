use super::*;
use crate::icons::ICON_CHEVRON_RIGHT;
use crate::native::Document;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_menu_row_with_a_submenu_shows_an_arrow_the_leaf_rows_do_not() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Copy" />
            <unstyled::MenuItem label="Share">
                <unstyled::MenuItem label="Email" />
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
    let copy = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(0);
    let share = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(1);

    let share_arrow = arrow(harness.document(), share).expect("the parent row built an arrow");

    assert!(
        arrow(harness.document(), copy).is_none(),
        "a leaf row holds no arrow, so it keeps no gap for one"
    );
    assert!(harness.document().node_rect(share_arrow).is_some());
}

fn arrow(document: &Document, row: NodeId) -> Option<NodeId> {
    if document.text_icons(row) == [ICON_CHEVRON_RIGHT] {
        return Some(row);
    }
    document
        .children(row)
        .into_iter()
        .find_map(|child| arrow(document, child))
}
