use super::*;

#[test]
fn resizing_a_window_in_space_lays_out_and_paints_only_that_window() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut space = InSpace::new(document);
    space.settle();
    space.space.borrow_mut().head.position.z += 1.5;
    space.settle();
    let (window, _) = space.tab(dock, "Tab 3");
    assert!(!window.is_main(), "tab 3 is in a window of its own");

    let size = space.space.borrow().panel(window).unwrap().size;
    let grip = space.on_screen(window, pos2(size.x - 2.0, size.y - 2.0));
    space.frame([
        Input::PointerMoved(grip),
        Input::PointerButton {
            pos: grip,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    space
        .space
        .borrow_mut()
        .begin_resize(window, crate::WindowEdge::BottomRight);
    space.frame([Input::PointerMoved(grip + vec2(4.0, 4.0))]);
    let main_nodes = {
        let document = space.document.borrow();
        let (_, main_tab) = space.tab(dock, "Tab 1");
        assert!(document.window_of(dock_tab(&document, dock, "Tab 1")).is_main());
        main_tab
    };

    for step in 1..=6 {
        let at = grip + vec2(4.0 + step as f32 * 12.0, 4.0 + step as f32 * 6.0);
        space.frame([Input::PointerMoved(at)]);
        let output = space.output();
        let main = output.main();
        let resized = output
            .windows
            .iter()
            .find(|shown| shown.id == window)
            .expect("the resized window is shown");
        assert_eq!(
            resized.size,
            space.space.borrow().panel(window).unwrap().size,
            "the window is laid out at its new size in the same frame"
        );
        assert!(resized.changed, "and painted again");
        assert!(
            !main.changed && main.damage.is_empty(),
            "the main window's painting is untouched while another window resizes: {:?}",
            main.damage
        );
    }
    assert_eq!(
        space.tab(dock, "Tab 1").1,
        main_nodes,
        "nothing in the main window moved"
    );
}
