use super::*;
use crate::reactive::{ForEach, Frame, List, NodeRef, build, view};

const ROWS: usize = 40;
const PICKED: usize = 28;

#[test]
fn the_reveal_button_scrolls_the_inspector_tree_to_a_picked_row() {
    let frames: Vec<NodeRef> = (0..ROWS).map(|_| NodeRef::new()).collect();
    let document = build({
        let frames = frames.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <ForEach keys={indices(ROWS)}>
                        {move |index: usize| view! {
                            <Frame @node_ref={&frames[index]} height=6.0 />
                        }}
                    </ForEach>
                </List>
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
    assert!(
        harness.reveal_shown(),
        "a pick outside the panel offers to scroll to it"
    );

    harness.click(harness.reveal_center());
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        !harness.reveal_shown(),
        "the button goes once the row is in view"
    );
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
