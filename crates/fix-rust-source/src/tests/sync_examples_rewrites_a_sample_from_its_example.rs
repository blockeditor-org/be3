use crate::sync_examples;

#[test]
fn sync_examples_rewrites_a_sample_from_its_example() {
    let markdown =
        "Intro.\n\n```rust example=checklist.adds\nold();\n```\n\n```rust\nuntouched();\n```\n";
    let mut asked = Vec::new();
    let mut lookup = |language: &str, id: &str| -> Result<String, String> {
        asked.push(format!("{language} {id}"));
        Ok("let a = 1;\n\nnew(a);\n".to_owned())
    };

    let synced = sync_examples(markdown, &mut lookup).unwrap();

    assert_eq!(
        synced,
        "Intro.\n\n```rust example=checklist.adds\nlet a = 1;\n\nnew(a);\n```\n\n```rust\nuntouched();\n```\n"
    );
    assert_eq!(asked, ["rust checklist.adds"]);
}
