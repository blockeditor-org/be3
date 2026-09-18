use super::*;

#[test]
fn a_cut_and_paste_conflicts_instead_of_interleaving() {
    let base: String = (0..20).map(|line| format!("line {line}\n")).collect();
    let retyped: String = (0..20).map(|line| format!("rewritten {line}\n")).collect();
    let theirs = base.replace("line 7\n", "line seven, edited\n");

    let outcome = merge_lines(base.as_bytes(), retyped.as_bytes(), theirs.as_bytes());
    assert!(
        !outcome.is_clean(),
        "a full rewrite silently absorbed a concurrent edit"
    );
    let merged = text(&outcome);
    assert!(
        !merged.contains("line seven, edited"),
        "the remote edit was interleaved into an unrelated rewrite"
    );

    let conflict = &outcome.conflicts[0];
    assert!(
        conflict
            .ours
            .iter()
            .any(|line| line.starts_with(b"rewritten"))
    );
    assert!(
        conflict
            .theirs
            .iter()
            .any(|line| line == b"line seven, edited\n")
    );
}
