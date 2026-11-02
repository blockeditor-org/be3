use super::*;

#[test]
fn rich_text_changed_on_the_same_line_on_each_side_conflicts_with_markers() {
    let base = caption("one\ntwo", Vec::new());
    let mut ours = base.clone();
    ours.apply(&format(&base, 1, 4..7, "bold", Some(MarkValue::Bool(true))));
    let mut theirs = base.clone();
    theirs.apply(&type_into(&base, 2, 7, "!"));

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    let text = "one\n<<<<<<< local\ntwo\n=======\ntwo!\n>>>>>>> remote\n";
    assert_eq!(words(&merged), text);
    let ours_line = text.find("two").expect("our line is kept");
    assert_eq!(seeds(&merged), vec![bold(ours_line..ours_line + 3)]);
}
