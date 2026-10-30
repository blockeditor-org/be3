use super::*;
use crate::native::Incoming;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn an_escaped_menu_repaints_the_row_the_pointer_leaves() {
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
    give_layer_space(&mut harness);
    let pos = harness.center(region);
    press_secondary(&mut harness, pos);
    let layer = harness.document().escaped_layers()[0].clone();
    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let row = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(1);
    let button = harness.document().api::<unstyled::ButtonApi>(row);
    let at = LAYER_SPACE.to_host.point(harness.center(row));
    harness
        .runner()
        .push(Incoming::layer(layer.id, Input::PointerMoved(at)));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        button.hovered().get_untracked() || button.focused().get_untracked(),
        "the row under the pointer is highlighted"
    );

    harness
        .runner()
        .push(Incoming::layer(layer.id, Input::PointerLeft));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        !button.hovered().get_untracked() && !button.focused().get_untracked(),
        "the row the pointer left is no longer highlighted"
    );
}
