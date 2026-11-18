use super::*;

/// As `an_edit_made_on_our_text_before_a_clean_merge_lands_on_the_merge`, for rich text, whose
/// formatting the merge keeps.
#[test]
fn an_edit_made_on_our_rich_text_before_a_clean_merge_lands_on_the_merge() {
    let base = caption("one\ntwo\nthree\n", vec![bold(4..7)]);
    let mut ours = base.clone();
    ours.apply(&type_into(&base, 1, 0, "A"));
    let mut theirs = base.clone();
    theirs.apply(&type_into(&base, 2, "one\ntwo\n".len(), "T"));
    let late = type_into(&ours, 1, 1, "B");

    let (mut merged, conflicts) = Document::merge(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    assert_eq!(words(&merged), "Aone\ntwo\nTthree\n");
    assert_eq!(seeds(&merged), vec![bold(5..8)]);
    merged.apply(&late);
    assert_eq!(words(&merged), "ABone\ntwo\nTthree\n", "the late edit lands");
}
