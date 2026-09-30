use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{
    Align, ClickCatcher, Direction, ItemSize, List, NodeRef, Spacer, Text, build, view,
};
use crate::unstyled::{TreeItem, TreeRowHandle, tree_focused};

const MARGIN: f32 = 40.0;

#[test]
fn a_tree_row_decides_which_part_of_it_is_clickable() {
    let chosen: Rc<RefCell<Vec<usize>>> = Rc::default();
    let tree = NodeRef::new();
    let document = build({
        let (chosen, tree) = (Rc::clone(&chosen), tree.clone());
        move || {
            let item = |key: usize| TreeItem {
                label: format!("row {key}"),
                depth: 0,
                expandable: false,
                expanded: false,
            };
            view! {
                <unstyled::Tree
                    @node_ref=&tree
                    keys={vec![0usize, 1]}
                    item
                    selected=None
                    on_select={move |key: usize| chosen.borrow_mut().push(key)}
                    on_expand={move |_: (usize, bool)| {}}
                >
                    {move |handle: TreeRowHandle<usize>| {
                        let TreeRowHandle { key, select, .. } = handle;
                        view! {
                            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                                <Spacer @sizing=ItemSize::Fixed(MARGIN) />
                                <ClickCatcher
                                    @sizing=ItemSize::Percent(100.0)
                                    on_click={move || select()}
                                >
                                    <Text
                                        @test_id={format!("name.{key}")}
                                        string={format!("row {key}")}
                                    />
                                </ClickCatcher>
                            </List>
                        }
                    }}
                </unstyled::Tree>
            }
        }
    });

    let tree = tree.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let name = harness.document().find_test_id("name.0").expect("the name");
    let row = harness.rect(name);

    harness.click(Pos2::new(row.left() - MARGIN / 2.0, row.center().y));
    harness.frame(Vec::new());

    assert!(
        chosen.borrow().is_empty(),
        "the margin the row left outside its own catcher must not select it"
    );
    assert_eq!(
        tree_focused::<usize>(harness.document(), tree),
        Some(0),
        "pressing anywhere in the row still moves the tree's focus to it"
    );

    harness.click(row.center());
    harness.frame(Vec::new());

    assert_eq!(
        *chosen.borrow(),
        [0],
        "the part the row made clickable selects"
    );
}
