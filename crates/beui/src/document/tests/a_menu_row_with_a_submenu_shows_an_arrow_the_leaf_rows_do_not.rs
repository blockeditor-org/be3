use super::*;
use crate::icons::ICON_CHEVRON_RIGHT;
use crate::reactive::{NodeRef, view};
use crate::styled::ContextMenu;

#[test]
fn a_menu_row_with_a_submenu_shows_an_arrow_the_leaf_rows_do_not() {
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of({
        let region = region.clone();
        move || {
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
    let share = unstyled::menu_list_row_button(harness.document(), content, 1);

    let copy_arrow = arrow(harness.document(), copy).expect("the leaf row built an arrow");
    let share_arrow = arrow(harness.document(), share).expect("the parent row built an arrow");

    assert!(harness.document().node_rect(copy_arrow).is_none());
    assert!(harness.document().node_rect(share_arrow).is_some());
}

fn arrow(document: &Document, row: NodeId) -> Option<NodeId> {
    let wanted = format!("\"{ICON_CHEVRON_RIGHT}\"");
    if document.node_detail(row).as_deref() == Some(wanted.as_str()) {
        return Some(row);
    }
    document
        .children(row)
        .into_iter()
        .find_map(|child| arrow(document, child))
}
