use super::*;
use crate::reactive::{Frame, NodeRef, build, view};

const ROWS: usize = 40;
const PICKED: usize = 28;

#[test]
fn picking_a_node_scrolls_the_inspector_tree_to_its_row() {
    let frames: Vec<NodeRef> = (0..ROWS).map(|_| NodeRef::new()).collect();
    let document = build({
        let frames = frames.clone();
        move || {
            let children: Vec<_> = frames
                .iter()
                .map(|frame| {
                    intrinsic(view! {
                        <Frame @node_ref=frame height=6.0 />
                    })
                })
                .collect();
            view! {
                <Column spacing=0.0 children=children />
            }
        }
    });
    let picked = frames[PICKED].get();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.toggle_picking();
    let target = harness
        .document
        .node_rect(picked)
        .expect("the frame was not laid out")
        .center();
    harness.click(target);
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state.selected.get(), Some(picked));

    let row = harness.inspector().row_node(PICKED + 1);
    let rect = harness
        .inspector()
        .document
        .node_rect(row)
        .expect("the row was not laid out");

    assert!(
        rect.top() >= 0.0 && rect.bottom() <= VIEWPORT.y,
        "the picked row stayed outside the panel at {rect:?}"
    );
}
