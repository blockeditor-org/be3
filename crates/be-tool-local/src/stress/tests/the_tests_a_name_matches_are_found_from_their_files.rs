use super::*;

#[test]
fn the_tests_a_name_matches_are_found_from_their_files() {
    let stems = ["a_menu_opens".to_owned(), "a_tab_closes".to_owned()];
    assert_eq!(tests_named(&stems, "menu"), ["a_menu_opens"]);
    assert_eq!(tests_named(&stems, "tests::a_"), stems);
    assert_eq!(
        tests_named(&stems, "a_tab_closes::a_tab_closes"),
        ["a_tab_closes"],
        "a name matches the test's full name, as libtest's filter does"
    );
    assert!(tests_named(&stems, "nothing").is_empty());
}
