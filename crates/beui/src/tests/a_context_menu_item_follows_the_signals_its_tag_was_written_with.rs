use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_item_follows_the_signals_its_tag_was_written_with() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let label = create_signal("Paste".to_owned());
    let blocked = create_signal(true);
    let selected = create_signal(Vec::new());
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label disabled={blocked} />
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

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let row = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(0);
    assert_eq!(
        label_within(harness.document(), row).as_deref(),
        Some("Paste")
    );

    harness.click(harness.center(row));
    harness.frame(Vec::new());
    assert!(
        selected.with_untracked(|selected| selected.is_empty()),
        "an item whose disabled prop reads true must not report a selection"
    );

    label.set("Duplicate".to_owned());
    blocked.set(false);
    harness.frame(Vec::new());
    assert_eq!(
        label_within(harness.document(), row).as_deref(),
        Some("Duplicate"),
        "a menu item written as a tag must follow the signal its label was given"
    );

    harness.click(harness.center(row));
    harness.frame(Vec::new());
    assert_eq!(
        selected.get_untracked().as_slice(),
        &[vec![0usize]],
        "the same item must become selectable once its disabled signal turns false"
    );
}

fn label_within(document: &Document, node: NodeId) -> Option<String> {
    if let Some(text) = document.arena.kind_of::<TextNode>(node) {
        return Some(document.text(text).to_owned());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| label_within(document, child))
}
