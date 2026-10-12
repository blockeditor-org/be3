use super::*;

#[test]
fn a_dirty_build_names_the_commit_it_started_from() {
    assert_eq!(commit("abc+1f2e"), "abc");
    assert_eq!(commit("abc"), "abc");
    assert_eq!(normalized(" abc\n"), "abc");
    assert_eq!(normalized("\n"), UNKNOWN);
}
