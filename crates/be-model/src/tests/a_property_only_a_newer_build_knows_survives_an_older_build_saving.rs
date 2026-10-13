use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "note")]
struct Note {
    text: String,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "note")]
struct PinnedNote {
    text: String,
    pinned: bool,
}

#[test]
fn a_property_only_a_newer_build_knows_survives_an_older_build_saving() {
    let newer = Document::new(&PinnedNote {
        text: "hello".to_owned(),
        pinned: true,
    });

    let mut older = Document::<Note>::from_bytes(&newer.to_bytes()).expect("the bytes decode");
    assert!(!older.is_locked(ObjectId::ROOT));
    older.apply(&Note::TEXT.set(ObjectId::ROOT, &"edited".to_owned()).into());

    let back = Document::<PinnedNote>::from_bytes(&older.to_bytes()).expect("the bytes decode");
    assert_eq!(
        back.root(),
        PinnedNote {
            text: "edited".to_owned(),
            pinned: true,
        }
    );
}
