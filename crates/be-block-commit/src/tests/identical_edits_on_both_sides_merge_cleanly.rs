use super::*;

#[test]
fn identical_edits_on_both_sides_merge_cleanly() {
    let base = "one\ntwo\nthree\n";
    let same = "one\nTWO\nthree\n";
    let outcome = merge_lines(base.as_bytes(), same.as_bytes(), same.as_bytes());
    assert!(outcome.is_clean(), "{:?}", outcome.conflicts);
    assert_eq!(text(&outcome), same);

    let untouched = merge_lines(base.as_bytes(), base.as_bytes(), "one\ntwo\n".as_bytes());
    assert!(untouched.is_clean());
    assert_eq!(text(&untouched), "one\ntwo\n");

    let empty = merge_lines(b"", b"only ours\n", b"");
    assert!(empty.is_clean());
    assert_eq!(text(&empty), "only ours\n");
}
