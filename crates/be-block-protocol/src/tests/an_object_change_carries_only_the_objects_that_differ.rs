use be_block_store::Hash;

use super::*;

#[test]
fn an_object_change_carries_only_the_objects_that_differ() {
    let hashes: Vec<Hash> = (0..100u32)
        .map(|seed| Hash::of(&seed.to_le_bytes()))
        .collect();
    let before = hashes[..60].to_vec();
    let mut after = hashes[..58].to_vec();
    after.extend_from_slice(&hashes[90..]);
    let change = ObjectChange::between(&before, &after);
    assert_eq!(change.removed, hashes[58..60]);
    assert_eq!(change.added, hashes[90..]);
    let mut applied = change.apply(&before);
    applied.sort_unstable();
    after.sort_unstable();
    assert_eq!(applied, after);
    assert_eq!(
        ObjectChange::between(&before, &before),
        ObjectChange::default()
    );
}
