use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TreeItem;

#[test]
fn a_tree_given_on_open_opens_a_row_on_enter_and_a_second_click() {
    let _root = crate::reactive::RootScope::new();
    let chosen = create_signal::<Vec<usize>>(Vec::new());
    let opened = create_signal::<Vec<usize>>(Vec::new());
    let document = build_node(move || {
        let item = move |key: usize| TreeItem {
            label: format!("row {key}"),
            depth: 0,
            expandable: false,
            expanded: false,
        };
        view! {
            <Tree
                keys={vec![0usize, 1, 2]}
                item
                selected=None
                row_test_id={|key: usize| format!("tree.{key}")}
                on_select={move |key: usize| chosen.update(|chosen| chosen.push(key))}
                on_open={move |key: usize| opened.update(|opened| opened.push(key))}
            >|row: TreeRowFace<usize>|
                <Text>{format!("row {}", row.key)}</Text>
            </Tree>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let row = |harness: &Harness, key: usize| {
        let row = harness
            .document()
            .find_test_id(&format!("tree.{key}.row"))
            .expect("the row");
        harness.center(row)
    };

    let at = row(&harness, 1);
    harness.click(at);
    harness.frame(Vec::new());
    assert_eq!(chosen.get_untracked(), vec![1], "a click selects the row");
    assert!(opened.with_untracked(Vec::is_empty), "and opens nothing");

    harness.click(at);
    harness.frame(Vec::new());
    assert_eq!(
        opened.get_untracked(),
        vec![1],
        "a second click on it opens it"
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        opened.get_untracked(),
        vec![1, 2],
        "Enter opens the row the keyboard is on"
    );
    assert_eq!(
        chosen.get_untracked().last(),
        Some(&2),
        "and selects it first"
    );
}
