use super::*;

#[test]
fn text_changed_on_the_same_line_on_each_side_conflicts_with_markers() {
    let base = note("password: hunter2\n");
    let mut ours = base.clone();
    let mut theirs = base.clone();
    typed_into(&mut ours, 17, "3");
    typed_into(&mut theirs, 17, "4");

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    let merged = body(&merged);
    assert!(merged.contains("hunter23"));
    assert!(merged.contains("hunter24"));
    assert!(merged.contains("<<<<<<<"));
}
