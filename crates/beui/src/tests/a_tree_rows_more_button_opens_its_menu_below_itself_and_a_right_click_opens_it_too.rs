use super::*;
use crate::reactive::*;
use crate::styled::{Text, Tree, TreeMenu, TreeRowFace};
use crate::unstyled::TreeItem;

#[test]
fn a_tree_rows_more_button_opens_its_menu_below_itself_and_a_right_click_opens_it_too() {
    let _root = crate::reactive::RootScope::new();
    let chosen = create_signal(Vec::<(usize, Vec<usize>)>::new());
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
                menu={move |row: usize| Some(TreeMenu {
                    items: view! {
                        <unstyled::MenuItem
                            label="Rename"
                            row_test_id={"tree.menu.rename".to_owned()}
                        />
                        <unstyled::MenuItem label="Delete" />
                    },
                    on_select: Callback::new(move |path: Vec<usize>| {
                        chosen.update(|chosen| chosen.push((row, path)))
                    }),
                })}
            >|face: TreeRowFace<usize>|
                <Text>{format!("Row {}", face.key)}</Text>
            </Tree>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let more = harness.rect(harness.find("tree.0.more"));
    harness.click(more.center());
    harness.settle();
    let rename = harness.rect(harness.find("tree.menu.rename"));
    assert!(
        rename.top() >= more.bottom() - 0.5,
        "the menu hangs below the button that opened it, at {rename:?} for {more:?}"
    );
    harness.click(rename.center());
    harness.settle();
    assert_eq!(chosen.get_untracked(), vec![(0, vec![0])]);

    let row = harness.rect(harness.find("tree.1.row"));
    let at = row.center();
    harness.frame(vec![Input::PointerMoved(at)]);
    harness.frame(vec![Input::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(vec![Input::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: false,
    }]);
    harness.settle();
    let rename = harness.rect(harness.find("tree.menu.rename"));
    harness.click(rename.center());
    harness.settle();
    assert_eq!(chosen.get_untracked(), vec![(0, vec![0]), (1, vec![0])]);
}
