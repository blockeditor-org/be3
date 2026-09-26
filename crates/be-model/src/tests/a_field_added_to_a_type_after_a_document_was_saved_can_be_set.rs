use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Note {
    text: String,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct PinnableNote {
    text: String,
    pinned: bool,
}

#[test]
#[ignore = "an object saved before a field was added holds fewer fields, and a set of the new field is dropped"]
fn a_field_added_to_a_type_after_a_document_was_saved_can_be_set() {
    let saved = Document::new(&Note {
        text: "hello".to_owned(),
    })
    .to_bytes();

    let mut document = Document::<PinnableNote>::from_bytes(&saved).expect("the bytes decode");
    assert_eq!(document.root().text, "hello");
    assert!(!document.root().pinned);
    document.apply(&PinnableNote::PINNED.set(ObjectId::ROOT, &true).into());

    assert_eq!(
        document.root(),
        PinnableNote {
            text: "hello".to_owned(),
            pinned: true,
        }
    );
}
