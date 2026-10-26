use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn dragging_a_dock_tab_in_space_onto_another_panels_bar_moves_it_there() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut space = InSpace::new(document);
    space.settle();
    space.space.borrow_mut().head.position.z += 1.5;
    let (from, tab) = space.tab(dock, "Tab 3");
    let (to, onto) = space.tab(dock, "Tab 4");
    assert_ne!(from, to, "each dock window is a panel of its own");
    let target = space
        .document
        .borrow()
        .api::<DockApi>(dock)
        .state()
        .find(TabId::new(4))
        .expect("tab 4 is in the dock")
        .surface;

    let grab = space.on_screen(from, tab.center());
    let bar = space.on_screen(to, pos2(onto.right() + 20.0, onto.center().y));
    space.drag(grab, bar);

    let state = space.document.borrow().api::<DockApi>(dock).state();
    assert_eq!(
        state.surface_tabs(target),
        vec![TabId::new(4), TabId::new(3)],
        "the tab joins the bar of the panel it was dropped on"
    );
    assert_eq!(space.tab(dock, "Tab 3").0, to);
}
