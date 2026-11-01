use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_middle_click_on_a_dock_tab_closes_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.frame(vec![Input::PointerMoved(tab)]);
    harness.down(tab, PointerButton::Middle);
    harness.up(tab, PointerButton::Middle);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().api::<DockApi>(dock).state().all_tabs(),
        vec![TabId::new(1), TabId::new(3)],
        "a middle click closes the tab under the pointer and leaves the others open"
    );
}
