use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn a_tab_split_out_of_a_window_keeps_its_panel_on_screen() {
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    drag_with(
        &mut harness,
        tab,
        pos2(WIDE_VIEWPORT.x / 2.0, WIDE_VIEWPORT.y / 2.0),
        Modifiers::ALT,
    );
    harness.frame(Vec::new());
    let state = dock_state(harness.document(), dock);
    let window = state.windows()[0];
    let rect = state.window_rect(window).expect("the window has a rect");
    let origin = harness.rect(dock).min + rect.min.to_vec2();
    let floated = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    harness.drag(tab, pos2(floated.right() + 20.0, origin.y + 10.0));
    harness.frame(Vec::new());
    assert_eq!(
        dock_state(harness.document(), dock).surface_tabs(window),
        vec![TabId::new(2), TabId::new(3)],
        "the window holds both tabs before the split"
    );

    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    harness.drag(tab, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y - 40.0));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    assert_eq!(
        state.leaves(state.main()).len(),
        2,
        "dropping the tab on the edge of the pane splits it"
    );
    assert_eq!(
        state.surface_tabs(window),
        vec![TabId::new(2)],
        "the window keeps the tab that stayed behind"
    );
    let panel = harness.rect(harness.find("content.3"));
    assert!(
        panel.left() > WIDE_VIEWPORT.x / 2.0 - 20.0,
        "the tab's panel is shown in the new pane on the right"
    );
}
