use super::*;

#[test]
fn a_drain_naming_no_kept_commit_forgets_nothing() {
    let mut lacking = Lacking::default();
    lacking.add(vec![1], Until::Sequenced(1), Vec::new());
    lacking.sealed(1, false, commit(1));
    assert_eq!(sending(&lacking), 1);

    lacking.drained(false, &[commit(9)]);
    assert_eq!(
        sending(&lacking),
        1,
        "a drain of a commit this replica did not keep holds none of its edits"
    );
    lacking.drained(false, &[]);
    assert_eq!(sending(&lacking), 1, "nor does a drain naming none");
}
