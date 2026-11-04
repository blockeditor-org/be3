use super::*;
use crate::base::overlay::OverlayNode;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_as_tall_as_the_window_allows_shows_every_item() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <ForEach keys={(0..20).collect::<Vec<usize>>()}>|index: usize| {
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
    let viewport = vec2(400.0, 900.0);
    let mut harness = Harness::sized(document, viewport);
    harness.frame(Vec::new());

    let pos = harness.rect(region).min + vec2(8.0, 8.0);
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());

    let overlay = kind_of::<OverlayNode>(
        harness.document(),
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(menu)
            .overlay(),
    );
    let panel = harness
        .document()
        .overlay_content(overlay)
        .expect("open context menu has content");
    let panel_rect = harness.rect(panel);
    assert!(
        panel_rect.height() > 420.0,
        "a menu takes the room the window has: {panel_rect:?}"
    );
    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let last = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(19);
    let last_rect = harness.rect(last);
    assert!(
        panel_rect.contains_rect(last_rect),
        "its last item shows without scrolling: {last_rect:?} in {panel_rect:?}"
    );
}
