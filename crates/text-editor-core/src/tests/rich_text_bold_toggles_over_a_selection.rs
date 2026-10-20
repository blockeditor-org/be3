use super::*;

#[test]
fn rich_text_bold_toggles_over_a_selection() {
    let mut tester = EditorTester::rich(b"hello world");
    select(&mut tester, 0..5);
    tool(&mut tester, "bold");
    assert_eq!(formatted(&tester), "{bold}hello{} world");
    tester.expect_content(b"[hello| world");
    assert!(active(&tester, "bold"));

    select(&mut tester, 3..8);
    assert!(
        !active(&tester, "bold"),
        "a selection only part bold is not bold"
    );
    tool(&mut tester, "bold");
    assert_eq!(formatted(&tester), "{bold}hello wo{}rld");
    tool(&mut tester, "bold");
    assert_eq!(formatted(&tester), "{bold}hel{}lo world");
}
