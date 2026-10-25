use super::*;
use crate::{BlockRef, Series};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "mentions")]
struct Mentions {
    blocks: Series<Option<BlockRef>>,
}

#[test]
fn a_block_ref_appended_to_a_series_is_found_and_dropped_with_it() {
    let linked = Uuid::new_v4();
    let mut document = Document::new(&Mentions::default());
    assert!(document.block_refs().is_empty());
    let update = |document: &Document<Mentions>, wanted: &[Option<BlockRef>]| {
        let held = document
            .series(ObjectId::ROOT, Mentions::BLOCKS)
            .expect("the mentions hold their blocks");
        Mentions::BLOCKS
            .update(ObjectId::ROOT, held, wanted)
            .expect("the blocks changed")
    };

    let change = update(&document, &[None, Some(BlockRef(linked))]);
    document.apply(&Edit(vec![change]));
    assert_eq!(document.block_refs(), [linked]);

    let change = update(&document, &[None]);
    document.apply(&Edit(vec![change]));
    assert!(document.block_refs().is_empty());
}
