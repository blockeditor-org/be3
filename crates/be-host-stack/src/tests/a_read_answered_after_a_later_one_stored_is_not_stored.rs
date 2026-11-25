/// Two reads of the offers to the account (or its mutes, or its notices) in flight answer in
/// either order: the older one's answer, landing after the newer one's, is not stored over it,
/// but landing first it is, as nothing newer was stored yet (`worker::Reads`).
#[test]
fn a_read_answered_after_a_later_one_stored_is_not_stored() {
    let reads = crate::worker::Reads::default();
    let older = reads.ask();
    let newer = reads.ask();
    assert!(newer.stores(), "the newest read's answer was not stored");
    assert!(!older.stores(), "an older answer landing last overwrote the newer");

    let older = reads.ask();
    let newer = reads.ask();
    assert!(older.stores(), "an answer landing first was not stored");
    assert!(newer.stores(), "a newer answer landing after an older was not stored");
}
