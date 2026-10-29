use super::*;
use crate::native::Document;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_show_adds_and_removes_a_menu_item_among_the_items_beside_it() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let pasteable = create_signal(false);
    let selected = create_signal(Vec::new());
    let (document, [menu]) = toolbar_of(move || {
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
    assert_eq!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .row_count(),
        2,
        "a hidden show puts no item between the items written around it"
    );

    pasteable.set(true);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .row_count(),
        3,
        "a show that turns visible adds its item to the run"
    );
    let rows: Vec<String> = (0..3)
        .map(|index| {
            let button = harness
                .document()
                .api::<unstyled::MenuListApi>(content)
                .row_button(index);
            label_within(harness.document(), button).expect("a row shows its label")
        })
        .collect();
    assert_eq!(
        rows,
        vec!["Copy".to_owned(), "Paste".to_owned(), "Delete".to_owned()],
        "the item a show builds keeps its place among the items written around it"
    );

    let delete = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(2);
    harness.click(harness.center(delete));
    harness.frame(Vec::new());
    assert_eq!(
        selected.get_untracked().as_slice(),
        &[vec![2usize]],
        "the items after a shown one report the index they now sit at"
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
