/// Two reads of the offers to the account (or its mutes, or its notices) in flight answer in
/// either order: the older one's answer, landing after the newer one's, is not stored over it,
/// but landing first it is, as nothing newer was stored yet (`worker::Numbered`).
#[test]
fn a_read_answered_after_a_later_one_stored_is_not_stored() {
    let mut reads = crate::worker::Numbered::default();
    let older = reads.ask();
    let newer = reads.ask();
    assert!(
        reads.stores(newer),
        "the newest read's answer was not stored"
    );
    assert!(
        !reads.stores(older),
        "an older answer landing last overwrote the newer"
    );

    let older = reads.ask();
    let newer = reads.ask();
    assert!(
        reads.stores(older),
        "an answer landing first was not stored"
    );
    assert!(
        reads.stores(newer),
        "a newer answer landing after an older was not stored"
    );
}
