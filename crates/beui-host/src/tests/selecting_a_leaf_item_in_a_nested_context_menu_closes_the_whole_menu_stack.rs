use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn selecting_a_leaf_item_in_a_nested_context_menu_closes_the_whole_menu_stack() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let selected = create_signal(Vec::new());
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Share">
                <unstyled::MenuItem label="Email" />
                <unstyled::MenuItem label="Link" />
            </unstyled::MenuItem>
        };
        [view! {
            <ContextMenu
                items
                on_select={move |path| {
                    selected.update(|selected| selected.push(path));
                }}
            >
                <MenuRegion @node_ref=&region />
            </ContextMenu>
        }]
    });
    let region = region.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());

    let overlay = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .overlay();

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(selected.get_untracked().as_slice(), &[vec![0usize, 1usize]]);
    assert!(!harness.document().is_overlay_open(overlay));
}
