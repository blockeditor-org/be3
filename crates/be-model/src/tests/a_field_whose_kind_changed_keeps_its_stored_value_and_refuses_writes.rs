use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Note {
    text: String,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Tally {
    text: Count,
}

#[test]
fn a_field_whose_kind_changed_keeps_its_stored_value_and_refuses_writes() {
    let saved = Document::new(&Note {
        text: "hello".to_owned(),
    })
    .to_bytes();

    let mut document = Document::<Tally>::from_bytes(&saved).expect("the bytes decode");
    assert_eq!(document.root().text, Count(0));
    document.apply(&Tally::TEXT.add(ObjectId::ROOT, 3).into());
    assert_eq!(document.root().text, Count(0));

    let reread = Document::<Note>::from_bytes(&document.to_bytes()).expect("the bytes decode");
    assert_eq!(reread.root().text, "hello");
}
