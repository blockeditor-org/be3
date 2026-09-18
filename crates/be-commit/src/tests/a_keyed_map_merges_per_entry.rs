use super::*;

use std::collections::BTreeMap;

#[test]
fn a_keyed_map_merges_per_entry() {
    let base = BTreeMap::from([(1, "one"), (2, "two"), (3, "three")]);
    let ours = BTreeMap::from([(1, "ONE"), (2, "two"), (3, "three"), (4, "four")]);
    let theirs = BTreeMap::from([(1, "one"), (3, "THREE")]);

    let merged = merge_map(&base, &ours, &theirs);
    assert!(merged.conflicts.is_empty());
    assert_eq!(
        merged.merged,
        BTreeMap::from([(1, "ONE"), (3, "THREE"), (4, "four")])
    );

    let contested = merge_map(
        &BTreeMap::from([(1, "one")]),
        &BTreeMap::from([(1, "ours")]),
        &BTreeMap::from([(1, "theirs")]),
    );
    assert_eq!(contested.conflicts, vec![1]);
    assert_eq!(contested.merged, BTreeMap::from([(1, "ours")]));

    let deleted_and_edited = merge_map(
        &BTreeMap::from([(1, "one")]),
        &BTreeMap::new(),
        &BTreeMap::from([(1, "edited")]),
    );
    assert_eq!(deleted_and_edited.conflicts, vec![1]);
    assert!(deleted_and_edited.merged.is_empty());
}
