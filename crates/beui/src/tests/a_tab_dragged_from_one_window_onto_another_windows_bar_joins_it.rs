use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_tab_dragged_from_one_window_onto_another_windows_bar_joins_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    for (title, to) in [("Tab 2", pos2(250.0, 150.0)), ("Tab 3", pos2(700.0, 380.0))] {
        let tab = harness.center(dock_tab(harness.document(), dock, title));
        drag_with(&mut harness, tab, to, Modifiers::ALT);
        harness.frame(Vec::new());
    }
    let state = harness.document().api::<DockApi>(dock).state();
    let [first, second] = [state.windows()[0], state.windows()[1]];
    assert_eq!(state.surface_tabs(second), [TabId::new(3)]);

    let from = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let target = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let to = pos2(target.max.x + 60.0, target.center().y);
    harness.frame(vec![Input::PointerMoved(from)]);
    harness.frame(vec![Input::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    for step in 1..=8 {
        let at = from + (to - from) * (step as f32 / 8.0);
        harness.frame(vec![Input::PointerMoved(at)]);
    }
    harness.frame(vec![Input::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    assert!(
        state.surface_tabs(first).contains(&TabId::new(3)),
        "the dragged tab joins the window whose bar it was dropped on: {:?}",
        state.surface_tabs(first)
    );
}
