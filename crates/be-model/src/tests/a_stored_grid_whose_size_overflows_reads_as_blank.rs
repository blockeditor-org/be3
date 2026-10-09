use super::*;
use crate::Stored;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Picture {
    pixels: Grid<[u8; 4]>,
}

#[test]
fn a_stored_grid_whose_size_overflows_reads_as_blank() {
    let huge = Stored::from(1u64 << 31);
    let grid = Stored::Map(vec![
        (Stored::from("cell_size"), Stored::from(4)),
        (Stored::from("left"), Stored::from(0)),
        (Stored::from("top"), Stored::from(0)),
        (Stored::from("width"), huge.clone()),
        (Stored::from("height"), huge),
        (Stored::from("cells"), Stored::Bytes(Vec::new())),
    ]);
    let document = Stored::Map(vec![
        (Stored::from("format"), Stored::from(1)),
        (
            Stored::from("root"),
            Stored::Map(vec![(Stored::from("pixels"), grid)]),
        ),
    ]);

    let picture = Document::<Picture>::from_bytes(&crate::stored::encode(&document))
        .expect("the bytes decode");

    assert_eq!(picture.root().pixels.bounds(), Bounds::default());
}
