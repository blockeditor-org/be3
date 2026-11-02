use super::*;

#[test]
fn each_tile_is_drawn_turned_from_the_rotation_shown_now() {
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    for (current, shown) in [
        (Rotation::Normal, Rotation::Normal),
        (Rotation::Clockwise90, Rotation::Clockwise90),
        (Rotation::Clockwise180, Rotation::Clockwise180),
        (Rotation::Clockwise270, Rotation::Clockwise270),
    ] {
        settings
            .editor
            .set_watched::<DisplaysWatch>(&vec![turned(shown)]);
        settings.run();
        let document = settings.editor.document();
        for rotation in Rotation::ALL {
            let turns =
                turns_within(&document, &tile(rotation)).expect("each tile turns its screen");
            let on_the_panel = (current.degrees() / 90 + u32::from(turns)) % 4;
            assert_eq!(
                on_the_panel,
                rotation.degrees() / 90,
                "the {rotation:?} tile reaches the panel turned as {rotation:?} would turn the screen, while it shows {current:?}"
            );
            if rotation == current {
                assert_eq!(turns, 0, "the tile for what shows now is drawn upright");
            }
        }
    }
}
