use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "note")]
struct Note {
    text: String,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "note")]
struct Tagged {
    text: String,
    colour: String,
    pinned: bool,
}

#[test]
fn properties_kept_for_a_newer_build_merge_key_by_key() {
    let saved = |colour: &str, pinned: bool| {
        Document::<Note>::from_bytes(
            &Document::new(&Tagged {
                text: "note".to_owned(),
                colour: colour.to_owned(),
                pinned,
            })
            .to_bytes(),
        )
        .expect("the bytes decode")
    };
    let base = saved("red", false);
    let ours = saved("blue", false);
    let theirs = saved("red", true);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    let back = Document::<Tagged>::from_bytes(&merged.to_bytes()).expect("the bytes decode");
    assert_eq!(back.root().colour, "blue");
    assert!(back.root().pinned);
}
