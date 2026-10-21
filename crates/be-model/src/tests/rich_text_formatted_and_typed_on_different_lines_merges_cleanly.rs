use super::*;

#[test]
fn rich_text_formatted_and_typed_on_different_lines_merges_cleanly() {
    let base = caption("one\ntwo\nthree\nfour", vec![heading(0..3)]);
    let mut ours = base.clone();
    ours.apply(&format(&base, 1, 4..7, "bold", Some(MarkValue::Bool(true))));
    let mut theirs = base.clone();
    theirs.apply(&type_into(&base, 2, 18, "!"));

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(words(&merged), "one\ntwo\nthree\nfour!");
    assert_eq!(seeds(&merged), vec![bold(4..7), heading(0..3)]);
    assert!(
        merged
            .rich(ObjectId::ROOT, Caption::WORDS)
            .expect("the caption has words")
            .is_fresh()
    );
}
