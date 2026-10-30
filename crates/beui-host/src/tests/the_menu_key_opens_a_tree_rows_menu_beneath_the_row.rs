use super::*;
use crate::styled::{Text, Tree, TreeMenu, TreeRowFace};
use crate::unstyled::TreeItem;

#[test]
fn the_menu_key_opens_a_tree_rows_menu_beneath_the_row() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <Tree
                keys={vec![0usize, 1]}
                item={|row: usize| TreeItem {
                    label: format!("Row {row}"),
                    ..TreeItem::default()
                }}
                selected=None
                row_test_id={|row: usize| format!("tree.{row}")}
                menu={move |_: usize| Some(TreeMenu {
                    items: view! {
                        <unstyled::MenuItem
                            label="Rename"
                            row_test_id={"tree.menu.rename".to_owned()}
                        />
                        <unstyled::MenuItem label="Delete" />
                    },
                })}
            >|face: TreeRowFace<usize>|
                <Text>{format!("Row {}", face.key)}</Text>
            </Tree>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    harness.click(harness.center(harness.find("tree.1.row")));
    harness.settle();
    let row = harness.rect(harness.find("tree.1.row"));

    for (key, modifiers) in [(Key::ContextMenu, Modifiers::NONE), (Key::F10, Modifiers::SHIFT)] {
        harness.key(key, modifiers);
        harness.settle();
        let rename = harness
            .document()
            .find_test_id("tree.menu.rename")
            .unwrap_or_else(|| panic!("{key:?} opens the focused row's menu"));
        let rename = harness.rect(rename);
        assert!(
            rename.min.y >= row.max.y - 1.0,
            "the menu opens beneath the row: {rename:?} under {row:?}"
        );
        harness.key(Key::Escape, Modifiers::NONE);
        harness.settle();
        assert!(harness.document().find_test_id("tree.menu.rename").is_none());
    }
}
