use super::*;

#[test]
fn resizing_the_main_panel_in_space_lays_the_main_window_out_at_its_new_size() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut space = InSpace::new(document);
    space.settle();
    let (window, _) = space.tab(dock, "Tab 3");
    let title = space
        .output()
        .windows
        .iter()
        .find(|shown| shown.id == window)
        .and_then(|shown| shown.window.appearance.title.clone());
    assert_eq!(
        title.as_deref(),
        Some("Tab 3"),
        "a dock window hands its host its tab's title for the chrome"
    );
    let panel = *space.space.borrow().panel(crate::WindowId::MAIN).unwrap();
    let start = panel.size;
    assert_eq!(space.output().main().size, start);

    let out = (beui_layer_space::chrome::GAP + beui_layer_space::chrome::THICKNESS / 2.0)
        * panel.chrome.scale
        / panel.metres_per_point;
    let side = pos2(start.x / 4.0, start.y + out);
    let grab = space.on_screen(crate::WindowId::MAIN, side);
    let to = space.on_screen(crate::WindowId::MAIN, side - vec2(0.0, 100.0));
    space.drag(grab, to);

    let size = space
        .space
        .borrow()
        .panel(crate::WindowId::MAIN)
        .unwrap()
        .size;
    assert_eq!(
        size,
        vec2(start.x, start.y - 100.0),
        "dragging the main panel's bottom side up makes it shorter"
    );
    assert_eq!(
        space.output().main().size,
        size,
        "the app lays its main window out at the panel's new size"
    );
    let shown = space
        .space
        .borrow()
        .panel(crate::WindowId::MAIN)
        .unwrap()
        .metres_per_point;
    assert_eq!(shown, panel.metres_per_point, "and keeps its scale");
}
