use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Text, build, clone, create_memo, create_signal, view};
use crate::styled::{Tree, TreeRowFace};
use crate::unstyled::TreeItem;

#[test]
fn clicking_a_tree_marker_expands_the_row_without_opening_it() {
    let opened: Rc<RefCell<Vec<usize>>> = Rc::default();
    let document = build({
        let opened = Rc::clone(&opened);
        move || {
            let (expanded, set_expanded) = create_signal(false);
            let keys = create_memo(clone!(expanded -> move || match expanded.get() {
                true => vec![0usize, 1],
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
                    keys
                    item
                    selected=None
                    on_select={move |key: usize| opened.borrow_mut().push(key)}
                    on_expand={move |(_, open): (usize, bool)| set_expanded.set(open)}
                >
                    {move |row: TreeRowFace<usize>| view! {
                        <Text
                            @test_id={format!("row.{}", row.key)}
                            string={format!("row {}", row.key)}
                        />
                    }}
                </Tree>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let label = harness.document().find_test_id("row.0").expect("the row");
    let rect = harness.document().node_rect(label).expect("the row's rect");

    harness.click(Pos2::new(rect.left() - 13.0, rect.center().y));
    harness.frame(Vec::new());

    assert!(
        harness.document().find_test_id("row.1").is_some(),
        "clicking the marker must expand the row"
    );
    assert!(
        opened.borrow().is_empty(),
        "clicking the marker must not open the row it belongs to"
    );

    harness.click(rect.center());
    harness.frame(Vec::new());

    assert_eq!(
        opened.borrow().as_slice(),
        [0],
        "clicking the row itself must open it"
    );
    assert!(
        harness.document().find_test_id("row.1").is_some(),
        "opening a row must leave its expansion alone"
    );
}
