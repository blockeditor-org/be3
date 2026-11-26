use std::collections::HashSet;

use super::*;

/// A store that cannot tell which objects it is (`ObjectStore::identity`) may be shared by
/// vaults this one cannot know of, so a part let go of over it is never collected.
#[test]
fn a_vault_over_a_store_with_no_identity_never_collects() {
    let store = CountingStore::default();
    let vault = Vault::new(store.clone(), keys(1)).with_chunker(ChunkerConfig::SMALL);
    let mut writer = vault.part_writer(id(1)).unwrap();
    writer.write(&pseudorandom(8 * 1024, 13)).unwrap();
    drop(writer.finish().unwrap());
    let unnamed = vault.unnamed_count();
    assert!(unnamed > 0, "let go of, unnamed");
    assert_eq!(vault.collect(|_| Ok(HashSet::new())).unwrap(), 0);
    assert_eq!(vault.unnamed_count(), unnamed, "still noted");
    assert_eq!(store.inner.count(), unnamed, "every object stays");
}
