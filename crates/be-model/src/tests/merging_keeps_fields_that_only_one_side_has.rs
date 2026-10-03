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
fn merging_keeps_fields_that_only_one_side_has() {
    let base = Document::new(&Note {
        text: "hello".to_owned(),
    });
    let mut ours = base.clone();
    ours.apply(
        &Note::TEXT
            .set(ObjectId::ROOT, &"hello there".to_owned())
            .into(),
    );

    let mut newer = Document::<PinnableNote>::from_bytes(&base.to_bytes()).unwrap();
    newer.apply(&PinnableNote::PINNED.set(ObjectId::ROOT, &true).into());
    let theirs = Document::<Note>::from_bytes(&newer.to_bytes()).unwrap();

    for (ours, theirs) in [(&ours, &theirs), (&theirs, &ours)] {
        let (merged, conflicts) = Document::merge(&base, ours, theirs);
        assert_eq!(conflicts, 0);
        let merged = Document::<PinnableNote>::from_bytes(&merged.to_bytes()).unwrap();
        assert_eq!(
            merged.root(),
            PinnableNote {
                text: "hello there".to_owned(),
                pinned: true,
            }
        );
    }
}
