use super::*;

#[test]
fn three_way_merge_conflicts_when_both_sides_rewrite_a_region() {
    let base = "one\ntwo\nthree\n";
    let outcome = merge_lines(
        base.as_bytes(),
        "one\nours\nthree\n".as_bytes(),
        "one\ntheirs\nthree\n".as_bytes(),
    );
    assert_eq!(outcome.conflicts.len(), 1);
    let conflict = &outcome.conflicts[0];
    assert_eq!(conflict.base, vec![b"two\n".to_vec()]);
    assert_eq!(conflict.ours, vec![b"ours\n".to_vec()]);
    assert_eq!(conflict.theirs, vec![b"theirs\n".to_vec()]);
    assert_eq!(text(&outcome), "one\nours\nthree\n");

    let rendered = merge::render_conflicts(&outcome, "local", "remote");
    assert_eq!(
        String::from_utf8(rendered).unwrap(),
        "one\n<<<<<<< local\nours\n=======\ntheirs\n>>>>>>> remote\nthree\n"
    );
}
