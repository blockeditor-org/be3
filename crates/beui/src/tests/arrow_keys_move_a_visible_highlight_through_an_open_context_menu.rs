use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn arrow_keys_move_a_visible_highlight_through_an_open_context_menu() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [_menu]) = toolbar_of(move || {
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

    let pos = harness.center(region);
    harness.frame(vec![Input::PointerMoved(pos)]);
    let opened = harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);

    let highlights = |output: &crate::native::FrameOutput| {
        output
            .main()
            .shapes()
            .iter()
            .filter(|shape| {
                matches!(shape, crate::native::Shape::Rect { color, stroke_width, .. }
                    if *color == styled::Theme::DARK.accent_soft && *stroke_width == 0.0)
            })
            .count()
    };
    assert_eq!(highlights(&opened), 0, "nothing should be highlighted yet");

    let moved = harness.frame(vec![key_event(Key::ArrowDown, true)]);
    assert_eq!(
        highlights(&moved),
        1,
        "first arrow press should highlight an item"
    );

    let moved_again = harness.frame(vec![key_event(Key::ArrowDown, true)]);
    assert_eq!(
        highlights(&moved_again),
        1,
        "highlight should move rather than disappear"
    );
}
