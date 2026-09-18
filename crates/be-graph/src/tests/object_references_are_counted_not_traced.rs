use super::*;

use be_store::Hash;

#[test]
fn object_references_are_counted_not_traced() {
    let shared = Hash::of(b"shared chunk");
    let only_ours = Hash::of(b"chunk of one commit");
    let mut refs = ObjectRefs::new();

    assert_eq!(refs.retain(&[shared, only_ours]), vec![shared, only_ours]);
    assert_eq!(refs.retain(&[shared]), Vec::new());
    assert_eq!(refs.count(shared), 2);
    assert_eq!(refs.count(only_ours), 1);

    assert_eq!(refs.release(&[shared, only_ours]), vec![only_ours]);
    assert_eq!(refs.count(shared), 1);
    assert_eq!(refs.count(only_ours), 0);

    assert_eq!(refs.release(&[shared]), vec![shared]);
    assert!(refs.is_empty());
    assert_eq!(refs.release(&[shared]), Vec::new());
}
