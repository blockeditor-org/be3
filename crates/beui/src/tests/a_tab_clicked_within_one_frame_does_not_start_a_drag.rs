use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_tab_clicked_within_one_frame_does_not_start_a_drag() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.frame(vec![
        Input::PointerMoved(tab),
        Input::PointerButton {
            pos: tab,
            button: PointerButton::Primary,
            pressed: true,
        },
        Input::PointerButton {
            pos: tab,
            button: PointerButton::Primary,
            pressed: false,
        },
    ]);
    harness.frame(vec![Input::PointerMoved(pos2(
        WIDE_VIEWPORT.x - 20.0,
        WIDE_VIEWPORT.y / 2.0,
    ))]);
    harness.frame(Vec::new());

    assert!(
        !harness.document().dragging(),
        "a click that lets go in the frame it pressed leaves nothing carried"
    );
    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(2)),
        "the click still selects the tab"
    );
}
