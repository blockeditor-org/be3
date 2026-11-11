use super::*;

#[test]
fn a_stale_card_does_not_make_a_replaced_signing_key_current() {
    let refs = refs::RefsDb::memory()
        .scope(refs::Scope {
            server: "http://server".to_owned(),
            account: Uuid::new_v4(),
            organization: Uuid::new_v4(),
        })
        .unwrap();
    let account = Uuid::new_v4();
    let (old, new) = ([1u8; 32], [2u8; 32]);
    refs.add_signer(account, old, true).unwrap();
    refs.add_signer(account, new, true).unwrap();
    // A server replays the card from before the new phrase.
    refs.add_signer(account, old, true).unwrap();
    let keys: Vec<[u8; 32]> = refs
        .signers(account)
        .iter()
        .map(|known| known.key)
        .collect();
    assert_eq!(keys, [old, new], "the new key stays the newest");
}
