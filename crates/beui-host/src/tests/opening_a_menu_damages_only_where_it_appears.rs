use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn opening_a_menu_damages_only_where_it_appears() {
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

    let damage = opened.main().damage().expect("opening a menu repaints");
    assert!(
        damage.height() < VIEWPORT.y * 0.5,
        "the dismissal scrim covers the viewport without painting any of it, \
         so it must not damage it: {damage:?}"
    );
    assert!(
        damage.contains(pos),
        "the menu that appeared under the pointer is damaged: {damage:?}"
    );
}
