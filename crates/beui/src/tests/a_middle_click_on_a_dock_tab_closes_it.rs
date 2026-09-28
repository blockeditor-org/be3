use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn a_middle_click_on_a_dock_tab_closes_it() {
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.frame(vec![Event::PointerMoved(tab)]);
    harness.middle_button(tab, true);
    harness.middle_button(tab, false);
    harness.frame(Vec::new());

    assert_eq!(
        dock_state(harness.document(), dock).all_tabs(),
        vec![TabId::new(1), TabId::new(3)],
        "a middle click closes the tab under the pointer and leaves the others open"
    );
}
