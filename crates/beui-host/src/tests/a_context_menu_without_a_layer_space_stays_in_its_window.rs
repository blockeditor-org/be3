use super::*;
use crate::base::overlay::OverlayNode;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_without_a_layer_space_stays_in_its_window() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Cut" />
            <unstyled::MenuItem label="Copy" />
            <unstyled::MenuItem label="Paste" />
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
    harness.frame(Vec::new());

    let pos = harness.rect(region).max - vec2(10.0, 10.0);
    press_secondary(&mut harness, pos);

    assert!(harness.document().escaped_layers().is_empty());
    let output = harness.frame(Vec::new());
    assert!(output.escaped.is_empty(), "nothing escapes without a layer space");
    let overlay = kind_of::<OverlayNode>(
        harness.document(),
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(menu)
            .overlay(),
    );
    let content = harness
        .document()
        .overlay_content(overlay)
        .expect("the open menu has content");
    assert!(
        Rect::from_min_size(Pos2::ZERO, window).contains_rect(harness.rect(content)),
        "the menu is kept inside the window as before"
    );
}
