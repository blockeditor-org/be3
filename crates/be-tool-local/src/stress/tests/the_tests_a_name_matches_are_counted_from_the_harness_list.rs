use super::*;

#[test]
fn the_tests_a_name_matches_are_counted_from_the_harness_list() {
    assert_eq!(listed_tests(""), 0);
    assert_eq!(listed_tests("0 tests, 0 benchmarks\n"), 0);
    assert_eq!(
        listed_tests("tests::a::a: test\ntests::b::b: test\n\n2 tests, 0 benchmarks\n"),
        2
    );
}
