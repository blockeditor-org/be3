use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TreeItem;

#[test]
fn arrow_keys_walk_a_tree_after_its_chevron_is_clicked_and_enter_selects() {
    let _root = crate::reactive::RootScope::new();
    let chosen = create_signal::<Vec<usize>>(Vec::new());
    let tree = ComponentRef::new();
    let document = build_node(move || {
        let expanded = create_signal(true);
        let keys = create_memo(move || match expanded.get() {
            true => vec![0usize, 1, 2],
            false => vec![0usize],
        });
        let item = move |key: usize| TreeItem {
            label: format!("row {key}"),
            depth: usize::from(key > 0),
            expandable: key == 0,
            expanded: expanded.get(),
        };
        view! {
            <Tree
                api=tree
                keys
                item
                selected=None
                row_test_id={|key: usize| format!("tree.{key}")}
                on_select={move |key: usize| chosen.update(|chosen| chosen.push(key))}
                on_expand={move |(_, open): (usize, bool)| expanded.set(open)}
            >|row: TreeRowFace<usize>|
                <Text>{format!("row {}", row.key)}</Text>
            </Tree>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let chevron = |harness: &Harness| {
        let chevron = harness
            .document()
            .find_test_id("tree.0.chevron")
            .expect("the chevron");
        harness.center(chevron)
    };

    let at = chevron(&harness);
    harness.click(at);
    harness.frame(Vec::new());
    let at = chevron(&harness);
    harness.click(at);
    harness.frame(Vec::new());

    assert_eq!(
        tree.get().focused(),
        Some(0),
        "clicking a chevron puts the keyboard on its row"
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        tree.get().focused(),
        Some(2),
        "the arrow keys walk the rows"
    );
    assert!(
        chosen.with_untracked(|chosen| chosen.is_empty()),
        "walking the rows selects none of them"
    );

    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        chosen.get_untracked().as_slice(),
        [2],
        "Enter selects the row the keyboard is on"
    );
}
