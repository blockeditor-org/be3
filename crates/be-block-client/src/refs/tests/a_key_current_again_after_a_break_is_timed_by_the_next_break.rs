use uuid::Uuid;

use super::*;

/// A key whose chain broke and that a link then verified again is current again with no
/// replacement time: a second break is timed by itself, not by the first, so what the key
/// signed in between still answers as the account's.
#[test]
fn a_key_current_again_after_a_break_is_timed_by_the_next_break() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let refs = RefsDb::open(&path)
        .unwrap()
        .scope(Scope {
            server: "server".to_owned(),
            account: Uuid::new_v4(),
            organization: Uuid::new_v4(),
        })
        .unwrap();
    let (contact, key) = (Uuid::new_v4(), [3; 32]);
    refs.add_signer(contact, key, true, Some(10)).unwrap();
    refs.supersede_signers(contact, Some(20)).unwrap();
    assert_eq!(refs.signers(contact)[0].replaced, Some(20));

    refs.add_signer(contact, key, true, Some(30)).unwrap();
    let known = refs.signers(contact)[0];
    assert!(!known.superseded, "the link made the key current again");
    assert_eq!(known.replaced, None, "a current key is replaced by nothing");

    refs.supersede_signers(contact, Some(40)).unwrap();
    assert_eq!(
        refs.signers(contact)[0].replaced,
        Some(40),
        "the second break is timed by itself"
    );

    drop(refs);
    let _ = std::fs::remove_file(&path);
}
