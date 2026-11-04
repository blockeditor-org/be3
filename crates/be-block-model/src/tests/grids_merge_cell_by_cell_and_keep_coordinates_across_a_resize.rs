use super::*;
use crate::{Bounds, Grid};

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Picture {
    pixels: Grid<[u8; 1]>,
}

fn picture() -> Document<Picture> {
    Document::new(&Picture {
        pixels: Grid::new(Bounds::new(0, 0, 4, 4)),
    })
}

fn painted(document: &Document<Picture>, cells: &[(i32, i32, u8)]) -> Document<Picture> {
    let mut changed = document.clone();
    changed.apply(
        &Picture::PIXELS
            .paint(
                ObjectId::ROOT,
                cells.iter().map(|(x, y, value)| (*x, *y, [*value])),
            )
            .into(),
    );
    changed
}

fn pixel(document: &Document<Picture>, x: i32, y: i32) -> Option<u8> {
    document.root().pixels.get(x, y).map(|[value]| value)
}

#[test]
fn grids_merge_cell_by_cell_and_keep_coordinates_across_a_resize() {
    let base = painted(&picture(), &[(1, 1, 5)]);
    let ours = painted(&base, &[(0, 0, 1), (3, 3, 7)]);
    let mut theirs = painted(&base, &[(3, 0, 2), (3, 3, 8)]);
    theirs.apply(
        &Picture::PIXELS
            .reshape(ObjectId::ROOT, Bounds::new(-2, 0, 6, 4))
            .into(),
    );

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1, "both sides painted (3, 3)");
    assert_eq!(merged.root().pixels.bounds(), Bounds::new(-2, 0, 6, 4));
    assert_eq!(pixel(&merged, 0, 0), Some(1));
    assert_eq!(pixel(&merged, 1, 1), Some(5));
    assert_eq!(pixel(&merged, 3, 0), Some(2));
    assert_eq!(pixel(&merged, 3, 3), Some(7));
    assert_eq!(pixel(&merged, -2, 0), Some(0));
    assert_eq!(pixel(&merged, 4, 0), None);
}
