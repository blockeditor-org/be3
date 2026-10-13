use super::*;

#[test]
fn adopting_session_state_carries_text_positions_to_a_reloaded_document() {
    let mut owner = note("abc");
    typed_into(&mut owner, 3, "d");
    let delete = body_edit(&owner, |body| body.delete(1..2));
    owner.apply(&delete);
    let mut follower = Document::<Note>::from_bytes(&owner.to_bytes()).expect("the bytes decode");
    follower
        .adopt_session_state(&owner.session_state())
        .expect("the state decodes");

    let after_removed = Note::BODY.edit(
        ObjectId::ROOT,
        SeqOp::Insert {
            after: Some(crate::Pos {
                client: crate::LOADED,
                offset: 1,
            }),
            client: 2,
            start: 0,
            items: b"X".to_vec(),
        },
    );
    owner.apply(&after_removed.clone().into());
    follower.apply(&after_removed.into());

    assert_eq!(body(&owner), "aXcd");
    assert_eq!(body(&follower), "aXcd");

    follower
        .adopt_session_state(&[])
        .expect("an empty state resets");
    assert!(
        follower
            .text(ObjectId::ROOT, Note::BODY)
            .expect("the note has a body")
            .is_fresh()
    );
    assert_eq!(body(&follower), "aXcd");
}
