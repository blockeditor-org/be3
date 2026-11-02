use super::*;

#[test]
fn text_changed_on_different_lines_on_each_side_merges_cleanly() {
    let base = note("one\ntwo\nthree\n");
    let mut ours = base.clone();
    let mut theirs = base.clone();
    typed_into(&mut ours, 3, "!");
    typed_into(&mut theirs, 13, "?");

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(body(&merged), "one!\ntwo\nthree?\n");
    assert!(
        merged
            .text(ObjectId::ROOT, Note::BODY)
            .expect("the note has a body")
            .is_fresh()
    );
}
