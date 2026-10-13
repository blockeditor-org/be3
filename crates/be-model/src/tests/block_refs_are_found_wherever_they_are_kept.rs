use super::*;
use crate::BlockRef;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "links")]
struct Links {
    main: Option<BlockRef>,
    rows: List<Row>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "row")]
struct Row {
    cells: Map<u32, Vec<BlockRef>>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "links")]
struct Older {
    rows: List<Row>,
}

#[test]
fn block_refs_are_found_wherever_they_are_kept() {
    let [main, first, second] = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let links = Document::new(&Links {
        main: Some(BlockRef(main)),
        rows: [Row {
            cells: [(1, vec![BlockRef(first), BlockRef(second), BlockRef(main)])]
                .into_iter()
                .collect(),
        }]
        .into_iter()
        .collect(),
    });
    assert_eq!(links.block_refs(), [main, first, second]);

    let older = Document::<Older>::from_bytes(&links.to_bytes()).expect("the bytes decode");
    assert_eq!(older.block_refs(), [first, second, main]);
}
