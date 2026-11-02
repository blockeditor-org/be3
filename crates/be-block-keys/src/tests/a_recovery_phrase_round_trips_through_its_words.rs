use super::*;

#[test]
fn a_recovery_phrase_round_trips_through_its_words() {
    let phrase = RecoveryPhrase::generate();
    assert_eq!(phrase.words().len(), 12);
    let typed = phrase.to_string().to_uppercase().replace(' ', "  \n");
    assert_eq!(RecoveryPhrase::parse(&typed).unwrap(), phrase);
    assert_eq!(
        RecoveryPhrase::parse(&typed).unwrap().secret().public(),
        phrase.secret().public()
    );

    let mut words = phrase.words();
    words.swap(0, 1);
    if words[0] != words[1] {
        assert_ne!(
            RecoveryPhrase::parse(&words.join(" ")).ok(),
            Some(phrase.clone()),
            "a phrase with its words swapped opened the same secret"
        );
    }
    assert_eq!(RecoveryPhrase::parse("not a phrase"), Err(KeyError::Phrase));
}
