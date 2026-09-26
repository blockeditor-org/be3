use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Layers {
    back: Grid<[u8; 1]>,
    front: Grid<[u8; 1]>,
}

#[test]
fn strokes_into_different_grids_undo_separately() {
    let mut document = Document::new(&Layers {
        back: Grid::new(Bounds::new(0, 0, 2, 2)),
        front: Grid::new(Bounds::new(0, 0, 2, 2)),
    });
    let back: Edit = Layers::BACK.paint(ObjectId::ROOT, [(0, 0, [1])]).into();
    let front: Edit = Layers::FRONT.paint(ObjectId::ROOT, [(0, 0, [2])]).into();
    let mut step = document.step(&back).expect("the stroke paints something");
    document.apply(&back);
    let next = document.step(&front).expect("the stroke paints something");
    document.apply(&front);

    assert!(step.absorb(next.clone()).is_err());
    document.apply(&next.undo());
    assert_eq!(document.root().front.get(0, 0), Some([0]));
    assert_eq!(document.root().back.get(0, 0), Some([1]));
}
