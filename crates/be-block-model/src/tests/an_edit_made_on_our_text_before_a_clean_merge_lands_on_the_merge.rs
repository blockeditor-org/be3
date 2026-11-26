use super::*;

/// A clean merge keeps our text's identities where the merge leaves it as it was: an edit made
/// on our side before the merge, applied after it (an edit the order numbers after a merge's
/// reload), still lands.
#[test]
fn an_edit_made_on_our_text_before_a_clean_merge_lands_on_the_merge() {
    let base = note("one\ntwo\nthree\n");
    let mut ours = base.clone();
    typed_into(&mut ours, 0, "A");
    let mut theirs = base.clone();
    typed_into(&mut theirs, "one\ntwo\n".len(), "T");
    let late = body_edit(&ours, |body| body.insert(ALICE, 1, b"B".to_vec()));

    let (mut merged, conflicts) = Document::merge(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    assert_eq!(body(&merged), "Aone\ntwo\nTthree\n");
    merged.apply(&late);
    assert_eq!(body(&merged), "ABone\ntwo\nTthree\n", "the late edit lands");
}
