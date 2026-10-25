use crate::freehand::{LiveStroke, stroke};
use block_editor_beui::beui::{Path, PathElement, Pos2, pos2};

fn quads(paths: impl IntoIterator<Item = Path>) -> Vec<[u32; 4]> {
    let mut quads: Vec<[u32; 4]> = paths
        .into_iter()
        .flat_map(|path| path.elements().to_vec())
        .filter_map(|element| match element {
            PathElement::QuadTo(control, to) => {
                Some([control.x, control.y, to.x, to.y].map(f32::to_bits))
            }
            _ => None,
        })
        .collect();
    quads.sort_unstable();
    quads
}

#[test]
fn a_stroke_drawn_in_pieces_has_the_outline_of_the_whole_stroke() {
    let points: Vec<Pos2> = (0..600)
        .map(|step| {
            let along = step as f32 / 599.0;
            pos2(
                400.0 * along,
                150.0 * (along * 40.0).sin() + 30.0 * (along * 7.0).cos(),
            )
        })
        .collect();
    let pressures: Vec<f32> = (0..600)
        .map(|step| 0.3 + 0.5 * (step as f32 * 0.05).sin().abs())
        .collect();
    let size = 4.0;
    let mut live = LiveStroke::new(size);
    let mut pieces = 0;
    for count in 1..=points.len() {
        let drawn = live.draw(&points[..count], &pressures[..count]);
        pieces = pieces.max(drawn.len());
        let whole = stroke(&points[..count], &pressures[..count], size, false);
        assert_eq!(
            quads(drawn.iter().map(|piece| piece.path().clone())),
            quads([whole]),
            "the stroke of {count} points"
        );
    }
    assert!(
        pieces > 4,
        "a long stroke settles into pieces, not {pieces}"
    );
}
