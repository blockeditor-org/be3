use crate::test::test_targets;

/// crates/beui makes only an `html-test`, so a run of one of its tests asks for that, not a
/// `:test` it lacks; a crate making both runs both.
#[test]
fn a_crate_whose_only_tests_are_named_runs_them_by_their_name() {
    let html_only = "cargo_library()\n\ncargo_test(\n    name = \"html-test\",\n    default_target_platform = \"root//buck/platforms:linux_x86_64_html\",\n    paints = True,\n)\n";
    assert_eq!(test_targets(html_only), ["html-test"]);
    let both = "cargo_test()\n\ncargo_test(\n    name = \"html-test\",\n    paints = True,\n)\n";
    assert_eq!(test_targets(both), ["test", "html-test"]);
}
