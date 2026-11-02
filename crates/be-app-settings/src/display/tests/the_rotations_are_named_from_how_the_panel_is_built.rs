use super::*;

#[test]
fn the_rotations_are_named_from_how_the_panel_is_built() {
    let tall = HostDisplay {
        current: mode(1080, 2340, 60_000),
        modes: vec![mode(1080, 2340, 60_000)],
        default: mode(1080, 2340, 60_000),
        ..gaming_monitor()
    };
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    let names = |settings: &Harness| {
        Rotation::ALL.map(|rotation| {
            settings
                .editor
                .accessible_label(&tile(rotation))
                .unwrap_or_default()
        })
    };
    assert_eq!(
        names(&settings),
        [
            "Landscape",
            "Portrait",
            "Landscape, flipped",
            "Portrait, flipped"
        ]
    );

    settings.editor.set_watched::<DisplaysWatch>(&vec![tall]);
    settings.run();
    assert_eq!(
        names(&settings),
        [
            "Portrait",
            "Landscape",
            "Portrait, flipped",
            "Landscape, flipped"
        ]
    );
}
