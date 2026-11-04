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
    under: List<Row>,
}

fn fresh(document: &Document<Links>) -> Vec<Uuid> {
    let mut found = Document::<Links>::from_bytes(&document.to_bytes())
        .expect("the bytes decode")
        .block_refs();
    found.sort();
    found
}

fn kept(document: &Document<Links>) -> Vec<Uuid> {
    let mut found = document.block_refs();
    found.sort();
    found
}

#[test]
fn block_refs_kept_through_edits_match_a_fresh_walk() {
    let [main, first, second, third] = [(); 4].map(|()| Uuid::new_v4());
    let row = |refs: &[Uuid]| Row {
        cells: [(1, refs.iter().copied().map(BlockRef).collect())]
            .into_iter()
            .collect(),
        under: List::default(),
    };
    let mut links = Document::new(&Links {
        main: Some(BlockRef(main)),
        rows: [row(&[first, main])].into_iter().collect(),
    });
    assert_eq!(links.block_refs(), [main, first]);

    let (outer, inner) = (ObjectId::new(), ObjectId::new());
    links.apply(&Edit(vec![
        Links::ROWS.insert_as(outer, ObjectId::ROOT, Anchor::End, &row(&[second])),
        Row::UNDER.insert_as(inner, outer, Anchor::End, &row(&[third, first])),
    ]));
    assert_eq!(kept(&links), fresh(&links));
    assert_eq!(kept(&links).len(), 4);

    links.apply(&Links::MAIN.set(ObjectId::ROOT, &None).into());
    assert_eq!(kept(&links), fresh(&links));

    links.apply(&Change::remove(outer).into());
    assert_eq!(kept(&links), fresh(&links));
    let mut left = vec![main, first];
    left.sort();
    assert_eq!(kept(&links), left);

    let held = links.root().rows[0].id;
    links.apply(&Row::CELLS.put(held, &1, None).into());
    assert!(links.block_refs().is_empty());
    assert_eq!(kept(&links), fresh(&links));
}
