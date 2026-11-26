use super::*;
use crate::segment::decode_dictionary_block;

#[test]
fn a_merge_of_a_dictionary_out_of_order_is_corrupt() {
    let mut store = Store::default();
    let mut index = Index::default();
    for n in 0..4u32 {
        apply(
            &mut index,
            &mut store,
            id(n),
            Batch {
                put: vec![put(n, 0, i64::from(n))],
                ..Batch::default()
            },
        );
    }
    assert_eq!(index.segment_count(), 4);

    // The first segment's first dictionary entry written twice: a block starts afresh with it,
    // so the second reads as the same term again.
    let meta = &mut index.segments[0];
    let name = meta.part(PartKind::Dictionary);
    let bytes = store.parts.parts.borrow()[&name].clone();
    let first = (1..=bytes.len())
        .find(|&end| decode_dictionary_block(&bytes[..end]).is_ok_and(|terms| terms.len() == 1))
        .unwrap();
    let terms = decode_dictionary_block(&bytes[..meta.blocks[1] as usize]).unwrap();
    assert!(terms.len() > 1);
    let mut changed = bytes[..first].to_vec();
    changed.extend_from_slice(&bytes);
    for offset in &mut meta.blocks[1..] {
        *offset += first as u32;
    }
    let doubled = decode_dictionary_block(&changed[..meta.blocks[1] as usize]).unwrap();
    assert_eq!(doubled[0].0, doubled[1].0);
    store.parts.parts.get_mut().insert(name, changed);

    let compacted = run(index.compact(&store, &store, id(50), 0, usize::MAX, u64::MAX, |_| 0));
    assert_eq!(compacted, Err(Error::Corrupt("dictionary order")));
}
