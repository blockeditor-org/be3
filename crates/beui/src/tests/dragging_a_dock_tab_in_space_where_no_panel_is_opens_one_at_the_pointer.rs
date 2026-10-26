use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn dragging_a_dock_tab_in_space_where_no_panel_is_opens_one_at_the_pointer() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut space = InSpace::new(document);
    space.settle();
    space.space.borrow_mut().head.position.z += 1.5;
    let (main, tab) = space.tab(dock, "Tab 1");
    assert!(main.is_main());
    let panels = space.space.borrow().panels.len();

    let grab = space.on_screen(main, tab.center());
    let sky = pos2(SPACE_SCREEN.x / 2.0, 40.0);
    space.drag(grab, sky);

    let (window, _) = space.tab(dock, "Tab 1");
    assert!(!window.is_main(), "the tab left the main window");
    assert_eq!(
        space.space.borrow().panels.len(),
        panels + 1,
        "into a panel of its own"
    );
    let state = space.document.borrow().api::<DockApi>(dock).state();
    let surface = state
        .find(TabId::new(1))
        .expect("tab 1 is in the dock")
        .surface;
    assert_eq!(state.surface_tabs(surface), vec![TabId::new(1)]);
    let grabbed = space.on_screen(window, beui_layer_space::OPEN_GRAB);
    assert!(
        (grabbed - sky).length() < 2.0,
        "opened where the pointer let go: {grabbed:?} vs {sky:?}"
    );
}
