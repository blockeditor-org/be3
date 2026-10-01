use super::*;

#[test]
fn a_sealed_key_opens_only_with_its_recovery_phrase() {
    let phrase = RecoveryPhrase::generate();
    let other = RecoveryPhrase::generate();
    let key = [7u8; 32];

    let sealed = phrase.secret().public().seal(&key);
    assert_ne!(sealed, phrase.secret().public().seal(&key));
    assert_eq!(phrase.secret().open(&sealed).unwrap(), key);
    assert_eq!(other.secret().open(&sealed), Err(KeyError::Corrupt));

    let mut flipped = sealed.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 1;
    assert_eq!(phrase.secret().open(&flipped), Err(KeyError::Corrupt));
    assert_eq!(phrase.secret().open(&sealed[..20]), Err(KeyError::Corrupt));
}
