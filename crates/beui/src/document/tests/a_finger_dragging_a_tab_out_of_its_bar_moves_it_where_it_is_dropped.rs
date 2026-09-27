use super::*;
use crate::unstyled::{Entry, TabId, dock_state};

#[test]
fn a_finger_dragging_a_tab_out_of_its_bar_moves_it_where_it_is_dropped() {
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    harness.frame(vec![Event::Modifiers(Modifiers::NONE)]);
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let placed = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window has a rect");
    let over = harness.rect(dock).min + placed.center().to_vec2();
    let bottom = pos2(over.x, WIDE_VIEWPORT.y - 5.0);
    assert!(
        harness.rect(dock).min.y + placed.bottom() < bottom.y,
        "the finger lets go below the window"
    );

    harness.finger_drag(&[tab, tab + vec2(0.0, 20.0), over, bottom]);
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaves = state.leaves(state.main());
    assert_eq!(
        leaves.len(),
        2,
        "a tab pulled down out of its bar by a finger splits the pane it is dropped on"
    );
    assert_eq!(
        state.entries(leaves[1]),
        vec![Entry::Tab(TabId::new(3))],
        "the dragged tab lands below, where the finger let go"
    );
    assert_eq!(
        state.surface_tabs(window),
        vec![TabId::new(2)],
        "passing over a window on the way does not drop the tab into it"
    );
}
