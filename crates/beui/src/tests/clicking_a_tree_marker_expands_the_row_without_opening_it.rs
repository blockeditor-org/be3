use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TreeItem;

#[test]
fn clicking_a_tree_marker_expands_the_row_without_opening_it() {
    let _root = crate::reactive::RootScope::new();
    let opened = create_signal::<Vec<usize>>(Vec::new());
    let document = build_node(move || {
        let expanded = create_signal(false);
        let keys = create_memo(move || match expanded.get() {
            true => vec![0usize, 1],
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
                keys
                item
                selected=None
                on_select={move |key: usize| opened.update(|opened| opened.push(key))}
                on_expand={move |(_, open): (usize, bool)| expanded.set(open)}
            >|row: TreeRowFace<usize>|
                <Text @test_id={format!("row.{}", row.key)}>{format!("row {}", row.key)}</Text>
            </Tree>
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
        opened.with_untracked(|opened| opened.is_empty()),
        "clicking the marker must not open the row it belongs to"
    );

    harness.click(rect.center());
    harness.frame(Vec::new());

    assert_eq!(
        opened.get_untracked().as_slice(),
        [0],
        "clicking the row itself must open it"
    );
    assert!(
        harness.document().find_test_id("row.1").is_some(),
        "opening a row must leave its expansion alone"
    );
}
