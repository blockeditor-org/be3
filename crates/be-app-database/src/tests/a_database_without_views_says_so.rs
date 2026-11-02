use super::*;

#[test]
fn a_database_without_views_says_so() {
    let (mut harness, _editor) = editor();

    harness.snapshot("a_database_without_views_says_so");
}
