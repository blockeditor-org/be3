use super::*;
use crate::unstyled::{Entry, TabId, dock_state};

#[test]
fn right_clicking_a_dock_tab_pops_it_out_into_a_window() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let pos = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());
    let row = text_within(harness.document(), dock, "Pop out into a window")
        .expect("the menu offers to pop the tab out");
    harness.click(harness.center(row));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    assert_eq!(
        state.windows().len(),
        1,
        "popping a tab out puts it in a window of its own"
    );
    assert_eq!(
        state.surface_tabs(state.windows()[0]),
        vec![TabId::new(2)],
        "the window holds the tab the menu was opened on"
    );
    assert_eq!(
        state.entries(state.leaves(state.main())[0]),
        vec![Entry::Tab(TabId::new(1))],
        "the pane it left keeps the rest of its tabs"
    );
}
