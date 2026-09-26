use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Everything {
    name: String,
    count: Count,
    rows: List<Card>,
    cells: Map<u32, String>,
    pixels: Grid<[u8; 2]>,
}

#[test]
fn a_document_round_trips_every_kind_of_field_through_bytes() {
    let mut document = Document::new(&Everything {
        name: "all".to_owned(),
        count: Count(4),
        rows: [card("one"), card("two")].into_iter().collect(),
        cells: [(7, "seven".to_owned())].into_iter().collect(),
        pixels: Grid::new(Bounds::new(-1, -1, 3, 3)),
    });
    document.apply(
        &Everything::PIXELS
            .paint(ObjectId::ROOT, [(-1, -1, [1, 2]), (1, 1, [3, 4])])
            .into(),
    );

    let decoded =
        Document::<Everything>::from_bytes(&document.to_bytes()).expect("the bytes decode");

    assert_eq!(decoded, document);
    assert_eq!(decoded.root(), document.root());
    assert_eq!(decoded.root().pixels.get(1, 1), Some([3, 4]));
    assert_eq!(decoded.root().rows.len(), 2);
}
