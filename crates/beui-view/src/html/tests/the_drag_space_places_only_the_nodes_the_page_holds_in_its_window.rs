use super::*;

#[test]
fn the_drag_space_places_only_the_nodes_the_page_holds_in_its_window() {
    let scope = ::beui_reactive::Scope::detached();
    scope.run(|| {
        let space = crate::drag::space();
        let held = swatch(BLUE, 10.0, 10.0);
        assert_eq!(space.window(held), Some(WindowId::MAIN));
        let stranger = NodeId::new(u32::MAX - 1, 7);
        assert_eq!(space.window(stranger), None);
    });
    scope.dispose();
}
