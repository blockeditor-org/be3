use super::*;
use crate::reactive::{NodeRef, Show, create_signal, view, with_reactive_scope};
use crate::styled::ContextMenu;

#[test]
fn a_show_adds_and_removes_a_menu_item_among_the_items_beside_it() {
    let region = NodeRef::new();
    let (pasteable, set_pasteable) = create_signal(false);
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let (document, [menu]) = toolbar_of({
        let region = region.clone();
        move || {
            let items = view! {
                <unstyled::MenuItem label="Copy" />
                <Show condition={pasteable}>
                    <unstyled::MenuItem label="Paste" />
                </Show>
                <unstyled::MenuItem label="Delete" />
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
    assert_eq!(
        unstyled::menu_list_len(harness.document(), content),
        2,
        "a hidden show puts no item between the items written around it"
    );

    with_reactive_scope(harness.document_mut(), move || set_pasteable.set(true));
    harness.frame(Vec::new());

    assert_eq!(
        unstyled::menu_list_len(harness.document(), content),
        3,
        "a show that turns visible adds its item to the run"
    );
    let rows: Vec<String> = (0..3)
        .map(|index| {
            let button = unstyled::menu_list_row_button(harness.document(), content, index);
            label_within(harness.document(), button).expect("a row shows its label")
        })
        .collect();
    assert_eq!(
        rows,
        vec!["Copy".to_owned(), "Paste".to_owned(), "Delete".to_owned()],
        "the item a show builds keeps its place among the items written around it"
    );

    let delete = unstyled::menu_list_row_button(harness.document(), content, 2);
    harness.click(harness.center(delete));
    harness.frame(Vec::new());
    assert_eq!(
        selected.borrow().as_slice(),
        &[vec![2usize]],
        "the items after a shown one report the index they now sit at"
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
