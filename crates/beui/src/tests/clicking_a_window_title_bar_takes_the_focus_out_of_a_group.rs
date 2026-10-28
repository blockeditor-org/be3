use super::*;
use crate::dock::unstyled::{DockApi, Entry, TabId};

#[test]
fn clicking_a_window_title_bar_takes_the_focus_out_of_a_group() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(4);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let pos = harness.center(dock_tab(harness.document(), dock, "Tab 4"));
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());
    let row = text_within(harness.document(), dock, "Pop out into a window")
        .expect("the menu offers to pop the tab out");
    harness.click(harness.center(row));
    harness.frame(Vec::new());
    let window = harness.rect(dock_tab(harness.document(), dock, "Tab 4"));
    let dragged = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    harness.drag(dragged, pos2(window.right() + 30.0, window.center().y));
    harness.frame(Vec::new());
    harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 2")));
    harness.frame(Vec::new());
    let dragged = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let onto = harness.rect(harness.find("content.2")).center();
    harness.drag(dragged, onto);
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    let surface = state.windows()[0];
    let outer = state.leaves(surface)[0];
    assert_eq!(
        state.entries(outer).len(),
        2,
        "the window holds a tab and a group: {:?}",
        state.entries(outer)
    );
    assert!(
        state
            .focused_leaf()
            .is_some_and(|leaf| state.is_nested(leaf)),
        "the focus starts inside the group"
    );

    let group = harness.rect(dock_tab(harness.document(), dock, "Tab 2, Tab 3"));
    harness.click(pos2(group.right() + 60.0, group.center().y));
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .focused_leaf(),
        Some(outer),
        "a press on the title bar of the window focuses its outer pane"
    );

    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .active_entry(outer),
        Some(Entry::Tab(TabId::new(4))),
        "Ctrl+Tab walks the tabs of the window rather than those of the group"
    );
}
