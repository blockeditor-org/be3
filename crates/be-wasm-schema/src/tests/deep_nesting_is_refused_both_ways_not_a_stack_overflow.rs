use super::*;

#[test]
fn deep_nesting_is_refused_both_ways_not_a_stack_overflow() {
    // Each level of a tree is two: its message and its list of children.
    let shallow = chain(MAX_DEPTH as usize / 2);
    assert_eq!(decode::<Tree>(&bytes(&shallow)), Ok(shallow));
    // Encoding refuses what decoding would.
    assert_eq!(
        kind(encode(&chain(MAX_DEPTH as usize / 2 + 1))),
        ErrorKind::TooDeep
    );
    assert_eq!(
        kind(encoded_len(&chain(MAX_DEPTH as usize / 2 + 1))),
        ErrorKind::TooDeep
    );
    // Bytes nesting far deeper, written by hand: a tree whose children (field 2) are a list of
    // one tree, a thousand times over.
    let mut deeper: Vec<u8> = Vec::new();
    for _ in 0..1000 {
        let mut list = vec![Wire::Message as u8];
        list.extend_from_slice(&1u32.to_le_bytes());
        list.extend_from_slice(&(deeper.len() as u32).to_le_bytes());
        list.extend_from_slice(&deeper);
        let mut tree = vec![(2 << 6) | Wire::List as u8, 0x01];
        tree.extend_from_slice(&(list.len() as u32).to_le_bytes());
        tree.extend_from_slice(&list);
        deeper = tree;
    }
    assert_eq!(kind(decode::<Tree>(&deeper)), ErrorKind::TooDeep);
    assert!(references(&deeper).is_err());
}
