use super::*;

#[test]
fn adopting_session_state_for_different_text_is_refused() {
    let original = note("abc");
    let mut owner = original.clone();
    typed_into(&mut owner, 3, "d");
    let mut behind = original.clone();
    behind
        .adopt_session_state(&[])
        .expect("an empty state resets");

    assert!(behind.adopt_session_state(&owner.session_state()).is_err());
    assert_eq!(body(&behind), "abc");
    assert!(
        behind
            .text(ObjectId::ROOT, Note::BODY)
            .expect("the note has a body")
            .is_fresh()
    );
}
