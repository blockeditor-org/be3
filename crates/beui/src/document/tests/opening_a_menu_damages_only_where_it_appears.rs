use super::*;
use crate::reactive::{NodeRef, view};
use crate::styled::ContextMenu;

#[test]
fn opening_a_menu_damages_only_where_it_appears() {
    let region = NodeRef::new();
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
    ];
    let (document, [_menu]) = toolbar_of({
        let region = region.clone();
        move || {
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
    let opened = harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);

    let damage = opened.damage().expect("opening a menu repaints");
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
