use super::*;

/// Two merges made apart, of different sides (on two devices), each insert a line of their own:
/// what each inserts is named apart, so an edit made on one's merge, applied to the other's,
/// names nothing there rather than landing inside the other's line.
#[test]
fn an_edit_made_on_one_merge_lands_on_no_item_another_merge_inserted() {
    let base = note("one\n");
    let mut theirs = base.clone();
    typed_into(&mut theirs, 4, "two\n");
    let (here, _) = Document::merge(&base, &base, &theirs);
    let mut other = base.clone();
    typed_into(&mut other, 4, "six\n");
    let (mut there, _) = Document::merge(&base, &base, &other);
    assert_eq!(body(&here), "one\ntwo\n");
    assert_eq!(body(&there), "one\nsix\n");

    // Typed between "t" and "w" of the line the first merge inserted.
    let typed = body_edit(&here, |body| body.insert(ALICE, 5, b"Z".to_vec()));
    there.apply(&typed);
    assert_eq!(
        body(&there),
        "one\nsix\n",
        "it lands on nothing the other merge inserted"
    );
}
