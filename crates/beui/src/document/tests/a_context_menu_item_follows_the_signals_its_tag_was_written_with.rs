use super::*;
use crate::reactive::{NodeRef, create_signal, view, with_reactive_scope};
use crate::styled::ContextMenu;

#[test]
fn a_context_menu_item_follows_the_signals_its_tag_was_written_with() {
    let region = NodeRef::new();
    let (label, set_label) = create_signal("Paste".to_owned());
    let (blocked, set_blocked) = create_signal(true);
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let (document, [menu]) = toolbar_of({
        let region = region.clone();
        move || {
            let items = view! {
                <unstyled::MenuItem label disabled={blocked} />
            };
            [view! {
                <ContextMenu
                    items
                    on_select={move |path| {
                        sink.borrow_mut().push(path);
                    }}
                >
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
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());

    let content = unstyled::context_menu_menu(harness.document(), menu);
    let row = unstyled::menu_list_row_button(harness.document(), content, 0);
    assert_eq!(
        label_within(harness.document(), row).as_deref(),
        Some("Paste")
    );

    harness.click(harness.center(row));
    harness.frame(Vec::new());
    assert!(
        selected.borrow().is_empty(),
        "an item whose disabled prop reads true must not report a selection"
    );

    with_reactive_scope(harness.document_mut(), move || {
        set_label.set("Duplicate".to_owned());
        set_blocked.set(false);
    });
    harness.frame(Vec::new());
    assert_eq!(
        label_within(harness.document(), row).as_deref(),
        Some("Duplicate"),
        "a menu item written as a tag must follow the signal its label was given"
    );

    harness.click(harness.center(row));
    harness.frame(Vec::new());
    assert_eq!(
        selected.borrow().as_slice(),
        &[vec![0usize]],
        "the same item must become selectable once its disabled signal turns false"
    );
}

fn label_within(document: &Document, node: NodeId) -> Option<String> {
    if document.node_kind(node) == "text" {
        return Some(document.text(node).to_owned());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| label_within(document, child))
}
