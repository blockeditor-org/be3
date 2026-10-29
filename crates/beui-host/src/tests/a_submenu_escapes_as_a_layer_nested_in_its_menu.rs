use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_submenu_escapes_as_a_layer_nested_in_its_menu() {
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
    let mut harness = Harness::sized(document, vec2(160.0, 80.0));
    give_layer_space(&mut harness);
    let pos = harness.rect(region).max - vec2(10.0, 10.0);
    press_secondary(&mut harness, pos);

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let share = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(0);
    let share_rect = harness.rect(share);
    harness.frame(vec![Input::PointerMoved(share_rect.center())]);
    harness.frame(Vec::new());

    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 2, "the menu and its submenu each escape");
    assert_eq!(layers[0].parent, None);
    assert_eq!(
        layers[1].parent,
        Some(layers[0].id),
        "the submenu is nested in the menu it opened from"
    );
    let host = LAYER_SPACE.to_host;
    let row = host.rect(share_rect);
    match layers[1].placement {
        LayerPlacement::Anchored(anchor) => assert!(
            (anchor.top() - row.top()).abs() < 1.0 && (anchor.left() - row.left()).abs() < 1.0,
            "a submenu's anchor is in host coordinates, its parent being a layer: {anchor:?} for {row:?}"
        ),
        LayerPlacement::Edge(edge) => panic!("a submenu hangs from its row, not {edge:?}"),
    }
    assert!(
        layers[1].rect.left() >= layers[0].rect.left(),
        "the submenu opens beside the menu"
    );
}
