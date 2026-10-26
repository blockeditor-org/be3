use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_dock_tab_in_a_window_opens_its_menu_over_the_window() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    right_click(&mut harness, dock, "Tab 2");
    let row = text_within(harness.document(), dock, "Pop out into a window")
        .expect("the menu offers to pop the tab out");
    harness.click(harness.center(row));
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .windows()
            .len(),
        1
    );

    right_click(&mut harness, dock, "Tab 2");
    let row = text_within(harness.document(), dock, "Close tab")
        .expect("the tab in the window opens its menu too");
    harness.click(harness.center(row));
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    assert!(
        !state.contains(TabId::new(2)),
        "the menu row over the window is the one the click reaches"
    );
    assert!(
        state.windows().is_empty(),
        "closing its only tab closes the window"
    );
}

fn right_click(harness: &mut Harness, dock: NodeId, title: &str) {
    let pos = harness.center(dock_tab(harness.document(), dock, title));
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: false,
    }]);
    harness.frame(Vec::new());
}
