use super::*;

#[test]
fn removals_are_kept_in_memory_and_never_saved() {
    let base = board();
    let (_, _, write) = ids(&base);

    let removed = edited(&base, [Change::remove(write)]);
    let reloaded = Document::<Board>::from_bytes(&removed.to_bytes()).expect("the bytes decode");

    assert!(!removed.removals().is_empty());
    assert!(reloaded.removals().is_empty());
    assert_eq!(reloaded, removed);
}
