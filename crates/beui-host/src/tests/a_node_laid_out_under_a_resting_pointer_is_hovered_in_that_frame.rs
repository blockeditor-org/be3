use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_node_laid_out_under_a_resting_pointer_is_hovered_in_that_frame() {
    let _root = crate::reactive::RootScope::new();
    let tall = create_signal(10.0_f32);
    let hovered = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dynamic value={tall}>|tall: f32|
                    <Frame width=100.0 height=tall />
                </Dynamic>
                <Interactive on_hover_change={hovered}>
                    <Frame width=100.0 height=40.0 />
                </Interactive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(vec![Input::PointerMoved(pos2(50.0, 55.0))]);
    assert!(!hovered.get_untracked(), "the pointer rests below the node");

    tall.set(20.0);
    harness.frame(Vec::new());
    assert!(
        hovered.get_untracked(),
        "the node a layout moves under the resting pointer is hovered in the frame that moved it, not whenever another frame happens to run"
    );
}
