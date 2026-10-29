use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_near_the_edge_of_its_region_escapes_into_its_layer_space() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <ForEach keys={(0..8).collect::<Vec<usize>>()}>|index: usize| {
                let label = format!("Item {index}");
                view! {
                    <unstyled::MenuItem label />
                }
            }</ForEach>
        };
        [view! {
            <ContextMenu items>
                <MenuRegion @node_ref=&region />
            </ContextMenu>
        }]
    });
    let region = region.get();
    let window = vec2(160.0, 80.0);
    let mut harness = Harness::sized(document, window);
    give_layer_space(&mut harness);

    let pos = harness.rect(region).max - vec2(10.0, 10.0);
    press_secondary(&mut harness, pos);

    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 1, "the open menu is one escaped layer");
    let layer = &layers[0];
    assert_eq!(layer.parent, None, "a context menu hangs from its region");
    let host = LAYER_SPACE.to_host;
    let pressed = host.point(pos);
    assert!(
        (layer.rect.left() - pressed.x).abs() <= 2.0 && (layer.rect.top() - pressed.y).abs() <= 2.0,
        "the menu opens at the press in host coordinates rather than shifting to fit the region: {:?} at {pressed:?}",
        layer.rect
    );
    let shown = host.rect(Rect::from_min_size(Pos2::ZERO, window));
    assert!(
        layer.rect.bottom() > shown.bottom() && layer.rect.right() > shown.right(),
        "the menu reaches past the region instead of being squeezed into it: {:?} past {shown:?}",
        layer.rect
    );
    assert!(layer.dismiss_on_outside_press && layer.keyboard && layer.blocks_owner);

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let last = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(7);
    assert!(
        host.rect(harness.rect(last)).bottom() <= layer.rect.bottom(),
        "every row fits in the escaped menu without scrolling"
    );

    let output = harness.frame(Vec::new());
    assert_eq!(output.escaped.len(), 1, "the frame hands the layer over");
    let painted = output
        .main()
        .layers
        .iter()
        .fold(Rect::NOTHING, |bounds, layer| bounds.union(layer.bounds()));
    assert!(
        Rect::from_min_size(Pos2::ZERO, window).contains_rect(painted),
        "the window's own painting leaves the escaped menu out: {painted:?}"
    );
}
