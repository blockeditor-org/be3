use super::*;

#[test]
fn every_member_of_the_worktree_gets_a_row() {
    let Fixture {
        mut test, members, ..
    } = editor(2);

    for member in &members {
        assert!(test.shown(&format!("worktree.member.{member}")));
    }
    test.snapshot("every_member_of_the_worktree_gets_a_row");
}
