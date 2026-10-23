use super::*;
use crate::base::overlay::OverlayNode;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn right_click_opens_a_context_menu_at_the_cursor_position() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
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
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.rect(region).min + Vec2::new(10.0, 10.0);
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
    assert_eq!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .row_count(),
        2
    );
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
    assert!((panel_rect.left() - pos.x).abs() < 0.5);
    assert!((panel_rect.top() - pos.y).abs() < 0.5);
}
