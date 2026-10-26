use crate::sync_examples;

#[test]
fn sync_examples_indents_a_sample_inside_a_list() {
    let markdown = "- Step:\n\n  ```rust example=a.b\n  ```\n";
    let mut lookup =
        |_: &str, _: &str| -> Result<String, String> { Ok("one();\n\ntwo();\n".to_owned()) };

    let synced = sync_examples(markdown, &mut lookup).unwrap();

    assert_eq!(
        synced,
        "- Step:\n\n  ```rust example=a.b\n  one();\n\n  two();\n  ```\n"
    );
}
