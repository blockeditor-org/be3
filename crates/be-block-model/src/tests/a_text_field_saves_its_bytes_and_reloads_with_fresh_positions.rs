use super::*;

#[test]
fn a_text_field_saves_its_bytes_and_reloads_with_fresh_positions() {
    let mut document = note("hello");
    typed_into(&mut document, 5, " world");

    let reloaded = Document::<Note>::from_bytes(&document.to_bytes()).expect("the bytes decode");

    assert_eq!(body(&reloaded), "hello world");
    assert_eq!(reloaded, document);
    assert!(
        !document
            .text(ObjectId::ROOT, Note::BODY)
            .expect("the note has a body")
            .is_fresh()
    );
    assert!(
        reloaded
            .text(ObjectId::ROOT, Note::BODY)
            .expect("the note has a body")
            .is_fresh()
    );
    assert!(reloaded.session_state().is_empty());
}
