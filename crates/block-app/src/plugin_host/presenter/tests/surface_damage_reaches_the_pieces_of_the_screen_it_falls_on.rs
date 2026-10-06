use super::*;

#[test]
fn surface_damage_reaches_the_pieces_of_the_screen_it_falls_on() {
    let placement = ScreenPlacement {
        screen: ScreenId(1),
        instance: EditorInstanceId(1),
        region: EditorRegion::Frame,
        surface: 1,
        width: 200,
        height: 100,
        scale_factor_millis: 1000,
    };
    let whole = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    let left_half = Rect::from_min_max(Pos2::ZERO, pos2(0.5, 1.0));
    let right_half = Rect::from_min_max(pos2(0.5, 0.0), pos2(1.0, 1.0));
    let rect = SurfaceRect {
        x: 50,
        y: 25,
        width: 20,
        height: 10,
    };

    assert_eq!(
        damaged_pieces(
            rect,
            placement,
            &[Piece {
                local: whole,
                source: whole
            }]
        ),
        [Rect::from_min_max(pos2(0.25, 0.25), pos2(0.35, 0.35))],
    );
    assert_eq!(
        damaged_pieces(
            rect,
            placement,
            &[Piece {
                local: whole,
                source: left_half
            }]
        ),
        [Rect::from_min_max(pos2(0.5, 0.25), pos2(0.7, 0.35))],
        "a piece showing part of the screen stretches its damage with it",
    );
    assert!(
        damaged_pieces(
            rect,
            placement,
            &[Piece {
                local: whole,
                source: right_half
            }]
        )
        .is_empty(),
        "damage on the part of the screen a piece leaves out is not shown",
    );
}
