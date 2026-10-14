use super::*;
use crate::reactive::{NodeRef, Text, build, create_memo, create_signal, view};
use crate::styled::{Tree, TreeRowFace, tree_focused};
use crate::unstyled::TreeItem;

#[test]
fn arrow_keys_walk_a_tree_after_its_chevron_is_clicked_and_enter_selects() {
    let _root = crate::reactive::RootScope::new();
    let (chosen, set_chosen) = create_signal::<Vec<usize>>(Vec::new());
    let tree = NodeRef::new();
    let document = build({
        move || {
            let (expanded, set_expanded) = create_signal(true);
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
                    @node_ref=&tree
                    keys
                    item
                    selected=None
                    row_test_id={|key: usize| format!("tree.{key}")}
                    on_select={move |key: usize| set_chosen.update(|chosen| chosen.push(key))}
                    on_expand={move |(_, open): (usize, bool)| set_expanded.set(open)}
                >
                    {move |row: TreeRowFace<usize>| view! {
                        <Text string={format!("row {}", row.key)} />
                    }}
                </Tree>
            }
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
        tree_focused::<usize>(harness.document(), tree.get()),
        Some(0),
        "clicking a chevron puts the keyboard on its row"
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        tree_focused::<usize>(harness.document(), tree.get()),
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
