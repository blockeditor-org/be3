use super::*;

#[test]
fn adopting_session_state_carries_marks_to_a_reloaded_document() {
    let mut owner = caption("hello world", Vec::new());
    let emboldened = format(&owner, 1, 0..5, "bold", Some(MarkValue::Bool(true)));
    owner.apply(&emboldened);
    owner.apply(&type_into(&owner, 1, 11, "!"));
    let mut follower =
        Document::<Caption>::from_bytes(&owner.to_bytes()).expect("the bytes decode");
    follower
        .adopt_session_state(&owner.session_state())
        .expect("the state decodes");
    assert_eq!(follower, owner);

    let Some(Change::Mark { mark, .. }) = emboldened.0.first() else {
        panic!("formatting is a mark");
    };
    let unbold: Edit = Caption::WORDS.unmark(ObjectId::ROOT, mark.id).into();
    owner.apply(&unbold);
    follower.apply(&unbold);

    assert_eq!(seeds(&owner), Vec::new());
    assert_eq!(follower, owner);
    assert_eq!(words(&follower), "hello world!");
}
