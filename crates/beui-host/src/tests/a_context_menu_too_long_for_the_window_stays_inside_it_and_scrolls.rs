use super::*;
use crate::base::overlay::OverlayNode;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_too_long_for_the_window_stays_inside_it_and_scrolls() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <ForEach keys={(0..40).collect::<Vec<usize>>()}>|index: usize| {
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
    let viewport = vec2(400.0, 300.0);
    let mut harness = Harness::sized(document, viewport);
    harness.frame(Vec::new());

    let pos = harness.center(region);
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
    let window = Rect::from_min_size(Pos2::ZERO, viewport);
    let panel_rect = harness.rect(panel);
    assert!(
        window.contains_rect(panel_rect),
        "a menu longer than the window is cut to fit inside it: {panel_rect:?}"
    );

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    harness.key(Key::End, Modifiers::NONE);
    harness.frame(Vec::new());
    let last = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(39);
    let last_rect = harness.rect(last);
    assert!(
        panel_rect.contains_rect(last_rect),
        "focusing the last item scrolls it into the menu: {last_rect:?} in {panel_rect:?}"
    );
}
