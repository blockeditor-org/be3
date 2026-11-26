use super::*;

#[test]
fn the_tests_a_name_matches_are_found_from_their_files() {
    let sources = env::temp_dir().join(format!("stress-sources-{}", std::process::id()));
    let _ = fs::remove_dir_all(&sources);
    for file in [
        "tests/a_menu_opens.rs",
        "tests/a_tab_closes.rs",
        "graph/tests/a_node_moves.rs",
        "lib.rs",
        "tests.rs",
    ] {
        let path = sources.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }
    let tests = test_files(&sources).unwrap();
    let _ = fs::remove_dir_all(&sources);
    let test = |part: &str, libtest: &str| Test {
        part: part.to_owned(),
        libtest: libtest.to_owned(),
    };
    assert_eq!(
        tests,
        [
            test("a_menu_opens", "tests::a_menu_opens::a_menu_opens"),
            test("a_tab_closes", "tests::a_tab_closes::a_tab_closes"),
            test(
                "graph-tests-a_node_moves",
                "graph::tests::a_node_moves::a_node_moves"
            ),
        ],
        "each file in a tests/ directory is a part, named as cargo_test names it"
    );
    let parts = |name: &str| -> Vec<String> {
        tests_named(&tests, name)
            .into_iter()
            .map(|test| test.part)
            .collect()
    };
    assert_eq!(parts("menu"), ["a_menu_opens"]);
    assert_eq!(parts("graph::"), ["graph-tests-a_node_moves"]);
    assert_eq!(
        parts("tests::a_"),
        ["a_menu_opens", "a_tab_closes", "graph-tests-a_node_moves"]
    );
    assert_eq!(
        parts("a_tab_closes::a_tab_closes"),
        ["a_tab_closes"],
        "a name matches the test's full name, as libtest's filter does"
    );
    assert!(parts("nothing").is_empty());
}
