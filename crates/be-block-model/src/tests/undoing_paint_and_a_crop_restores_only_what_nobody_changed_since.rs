use super::*;
use crate::{Bounds, Grid};

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Picture {
    pixels: Grid<[u8; 1]>,
}

fn pixel(document: &Document<Picture>, x: i32, y: i32) -> Option<u8> {
    document.root().pixels.get(x, y).map(|[value]| value)
}

fn stroke(document: &mut Document<Picture>, cells: &[(i32, i32, u8)]) -> crate::Step {
    let edit: Edit = Picture::PIXELS
        .paint(
            ObjectId::ROOT,
            cells.iter().map(|(x, y, value)| (*x, *y, [*value])),
        )
        .into();
    let step = document.step(&edit).expect("the stroke paints something");
    document.apply(&edit);
    step
}

#[test]
fn undoing_paint_and_a_crop_restores_only_what_nobody_changed_since() {
    let mut document = Document::new(&Picture {
        pixels: Grid::new(Bounds::new(0, 0, 3, 3)),
    });
    let mut step = stroke(&mut document, &[(0, 0, 1)]);
    let next = stroke(&mut document, &[(1, 0, 1), (0, 0, 2)]);
    assert!(
        step.absorb(next).is_ok(),
        "strokes on one grid undo together"
    );
    let _ = stroke(&mut document, &[(1, 0, 9)]);

    document.apply(&step.undo());
    assert_eq!(pixel(&document, 0, 0), Some(0));
    assert_eq!(pixel(&document, 1, 0), Some(9), "someone painted it since");

    let _ = stroke(&mut document, &[(2, 2, 4)]);
    let crop: Edit = Picture::PIXELS
        .reshape(ObjectId::ROOT, Bounds::new(0, 0, 2, 2))
        .into();
    let cropped = document.step(&crop).expect("the crop drops a cell");
    document.apply(&crop);
    assert_eq!(pixel(&document, 2, 2), None);

    document.apply(&cropped.undo());
    assert_eq!(document.root().pixels.bounds(), Bounds::new(0, 0, 3, 3));
    assert_eq!(pixel(&document, 2, 2), Some(4));
}
