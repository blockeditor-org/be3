use std::collections::BTreeMap;

use be_store::{Manifest, decode_value, encode_value};
use serde::de::IgnoredAny;

use super::*;

#[test]
fn a_change_keeps_its_id_across_rewrites_and_an_older_commit_is_its_own_change() {
    let store = store();
    let older = commit(&store, "first\n", 1, None);
    let loaded = store.get(older).unwrap();
    assert_eq!(loaded.change, None);
    assert_eq!(loaded.change_id(older), ChangeId::of_commit(older));

    let change = ChangeId::new();
    let drafted = Commit::new(Manifest::default(), AUTHOR, 2)
        .with_parent(Some(older))
        .with_change(change);
    let draft = store.put(KEY, &drafted).unwrap();
    let rewritten = store
        .put(
            KEY,
            &Commit {
                time: 3,
                ..drafted.clone()
            },
        )
        .unwrap();
    assert_ne!(draft, rewritten);
    assert_eq!(store.get(draft).unwrap().change_id(draft), change);
    assert_eq!(store.get(rewritten).unwrap().change_id(rewritten), change);

    let fields: BTreeMap<String, IgnoredAny> =
        decode_value(&encode_value(&drafted).unwrap()).unwrap();
    assert!(fields.contains_key("change"));
}
