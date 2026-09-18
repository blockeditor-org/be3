use super::*;

#[test]
fn the_checked_out_branch_is_marked_in_the_sidebar() {
    let Fixture { mut test, .. } = editor(1);

    assert!(!test.shown(&format!("worktree.switch.{MAIN_BRANCH}")));
    test.snapshot("the_checked_out_branch_is_marked_in_the_sidebar");
}
