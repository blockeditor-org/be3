use super::*;

#[test]
fn a_region_lays_out_over_its_whole_rect_not_just_what_shows() {
    let laid = Rc::new(Cell::new(None));
    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Recording {
            laid: Rc::clone(&laid),
            copy: None,
        },
    );
    let rect = Rect::from_min_size(pos2(-10.0, -20.0), vec2(100.0, 50.0));
    surface.update(&region(rect, [80, 30]));
    assert_eq!(
        laid.get(),
        Some(rect),
        "a region scrolled partly out of view is laid out where the host placed it, with the surface showing the part still in view"
    );
}
