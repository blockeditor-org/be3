use super::*;

#[test]
fn a_beside_overlay_is_placed_from_its_measured_size_as_a_menu_is() {
    let styled = |placement: Placement| anchored_styles(placement, Some("--anchor"));
    for placement in [
        Placement::ABOVE_START,
        Placement::BELOW_CENTER.gap(4).margin(8),
        Placement::LEFT_END,
        Placement::RIGHT_START,
    ] {
        assert!(measured_placement(placement), "{placement:?}");
        assert_eq!(
            styled(placement),
            styled(Placement::Menu),
            "{placement:?} leaves where it goes to `layer::resolve_rect`, as a menu does"
        );
    }
    assert!(
        styled(Placement::Menu)
            .iter()
            .all(|(name, _)| !matches!(*name, "position-area" | "position-try-fallbacks")),
        "CSS fallbacks re-align a box rather than shift it, so none are asked for"
    );
    assert!(!measured_placement(Placement::At));
}
