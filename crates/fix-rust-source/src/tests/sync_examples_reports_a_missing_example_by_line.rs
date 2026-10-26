use crate::sync_examples;

#[test]
fn sync_examples_reports_a_missing_example_by_line() {
    let markdown = "Text.\n\n```rust example=a.gone\nkept();\n```\n";
    let mut lookup = |_: &str, id: &str| -> Result<String, String> { Err(format!("no {id}")) };

    let errors = sync_examples(markdown, &mut lookup).unwrap_err();

    assert_eq!(errors, ["3: no a.gone"]);
}
