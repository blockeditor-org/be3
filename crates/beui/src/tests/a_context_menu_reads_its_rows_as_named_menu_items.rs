use accesskit::Role;

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_reads_its_rows_as_named_menu_items() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [_menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Copy" />
            <unstyled::MenuItem label="Paste" disabled=true />
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
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());

    let items: Vec<(Option<String>, bool)> = harness
        .accessible()
        .iter()
        .filter(|node| node.role() == Role::MenuItem)
        .map(|node| (node.label().map(str::to_owned), node.is_disabled()))
        .collect();
    assert_eq!(
        items,
        [
            (Some("Copy".to_owned()), false),
            (Some("Paste".to_owned()), true)
        ]
    );
}
