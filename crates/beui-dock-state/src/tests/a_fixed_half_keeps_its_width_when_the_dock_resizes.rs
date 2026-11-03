use super::*;

#[test]
fn a_fixed_half_keeps_its_width_when_the_dock_resizes() {
    let widths = |size: SplitSize, width: f32| {
        let mut state = DockState::default();
        state.reconcile(&spec(sized(
            "root",
            Direction::Horizontal,
            size,
            pane("files", &[1]),
            pane("editors", &[2]),
        )));
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(width + 6.0, 300.0));
        let layout = layout_surface(&state, state.main(), area, 6.0);
        (layout.leaves[0].1.width(), layout.leaves[1].1.width())
    };

    assert_eq!(widths(SplitSize::First(260.0), 800.0), (260.0, 540.0));
    assert_eq!(
        widths(SplitSize::First(260.0), 1400.0),
        (260.0, 1140.0),
        "a fixed first half keeps its points and the second takes the rest"
    );
    assert_eq!(
        widths(SplitSize::Second(300.0), 1000.0),
        (700.0, 300.0),
        "a fixed second half keeps its points"
    );
    assert_eq!(
        widths(SplitSize::First(260.0), 300.0),
        (180.0, MIN_PANE_LENGTH),
        "in a dock too small for it a fixed half leaves the other half room"
    );
    assert_eq!(
        widths(SplitSize::Second(260.0), 200.0),
        (100.0, 100.0),
        "in a dock too small for two panes the halves share it"
    );
}
