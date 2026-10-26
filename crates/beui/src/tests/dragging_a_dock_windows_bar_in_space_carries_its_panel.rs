use super::*;

#[test]
fn dragging_a_dock_windows_bar_in_space_carries_its_panel() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut space = InSpace::new(document);
    space.settle();
    space.space.borrow_mut().head.position.z += 1.5;
    let (window, tab) = space.tab(dock, "Tab 3");
    assert!(!window.is_main(), "the dock window is a panel of its own");
    let before = space.space.borrow().panel(window).unwrap().pose;

    let bar = space.on_screen(window, pos2(tab.right() + 40.0, tab.center().y));
    space.drag(bar, bar + vec2(0.0, 120.0));

    let after = space.space.borrow().panel(window).unwrap().pose;
    assert!(
        after.position.y < before.position.y - 0.1,
        "the panel follows the pointer down: {before:?} -> {after:?}"
    );
    assert_eq!(
        space.tab(dock, "Tab 3").0,
        window,
        "the tab stays in its window"
    );
    let placement = space
        .document
        .borrow()
        .context()
        .and_then(|context| context.window_placement(window))
        .expect("the host reports where it put the window");
    assert_eq!(
        placement,
        space.space.borrow().panel(window).unwrap().placement()
    );
}
