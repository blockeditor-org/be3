use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{NodeRef, Text, build, clone, create_memo, create_signal, view};
use crate::styled::{Tree, TreeRowFace, tree_focused};
use crate::unstyled::TreeItem;

#[test]
fn arrow_keys_walk_a_tree_after_its_chevron_is_clicked_and_enter_selects() {
    let chosen: Rc<RefCell<Vec<usize>>> = Rc::default();
    let tree = NodeRef::new();
    let document = build({
        let chosen = Rc::clone(&chosen);
        let tree = tree.clone();
        move || {
            let (expanded, set_expanded) = create_signal(true);
            let keys = create_memo(clone!(expanded -> move || match expanded.get() {
                true => vec![0usize, 1, 2],
                false => vec![0usize],
            }));
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
                    on_select={move |key: usize| chosen.borrow_mut().push(key)}
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
        chosen.borrow().is_empty(),
        "walking the rows selects none of them"
    );

    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        chosen.borrow().as_slice(),
        [2],
        "Enter selects the row the keyboard is on"
    );
}
