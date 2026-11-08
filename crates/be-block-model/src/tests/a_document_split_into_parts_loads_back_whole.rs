use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Book {
    pages: List<Line>,
    points: Series<f32>,
    pixels: Grid<[u8; 2]>,
    caption: RichText,
}

#[test]
fn a_document_split_into_parts_loads_back_whole() {
    let document = Document::new(&Book {
        pages: (0..40)
            .map(|index| Line {
                words: Text::from(prose(index, if index % 10 == 0 { 30_000 } else { 50 }).as_str()),
            })
            .collect(),
        points: (0..40_000).map(|index| index as f32).collect(),
        pixels: Grid::new(Bounds::new(0, 0, 300, 300)),
        caption: RichText::from(prose(3, 30_000).as_str()),
    });

    let root = document.to_parts();
    let large_page = root
        .all()
        .into_iter()
        .any(|part| !part.children().is_empty() && part.hash() != root.hash());
    assert!(large_page, "a large page is a part naming parts of its own");
    assert_eq!(from_parts::<Book>(&root), document);

    let mut bytes = Vec::new();
    root.write(&mut bytes);
    let child = root.children()[0].bytes();
    let at = bytes
        .windows(child.len())
        .position(|window| window == child)
        .expect("the child is in the stored bytes");
    bytes[at + child.len() - 1] ^= 1;
    assert!(Document::<Book>::from_bytes(&bytes).is_err());
}
