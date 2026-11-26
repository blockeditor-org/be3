use super::*;
use crate::segment::{Document, record};

/// A documents part holding `documents` in this order.
fn documents_part(documents: &[Document]) -> Vec<u8> {
    let mut bytes = (documents.len() as u32).to_le_bytes().to_vec();
    let mut records = Vec::new();
    bytes.extend(0u32.to_le_bytes());
    for document in documents {
        records.extend(record(document));
        bytes.extend((records.len() as u32).to_le_bytes());
    }
    bytes.extend(records);
    bytes
}

#[test]
fn a_merge_of_documents_out_of_order_is_corrupt() {
    let mut store = Store::default();
    let mut index = Index::default();
    for (batch, puts) in [vec![put(0, 0, 2), put(1, 0, 1)], vec![put(2, 0, 0)], vec![put(3, 0, 0)], vec![put(4, 0, 0)]]
        .into_iter()
        .enumerate()
    {
        apply(
            &mut index,
            &mut store,
            id(batch as u32),
            Batch {
                put: puts,
                ..Batch::default()
            },
        );
    }
    assert_eq!(index.segment_count(), 4);

    // The first segment's second document made newer than its first, which local order puts
    // first: the table still points at both where they are.
    let meta = index.segments[0].clone();
    let mut documents = run(meta.documents(&store)).unwrap();
    assert_eq!(documents[0].doc, doc(0));
    documents[1].commit = commit(3);
    store
        .parts
        .parts
        .get_mut()
        .insert(meta.part(PartKind::Documents), documents_part(&documents));

    let compacted = run(index.clone().compact(&store, &store, id(50), 0, usize::MAX, u64::MAX, |_| 0));
    assert_eq!(compacted, Err(Error::Corrupt("documents order")));
}
