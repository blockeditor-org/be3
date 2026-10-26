use crate::example_body;

#[test]
fn example_body_is_the_function_body_dedented() {
    let source = "mod tests {\n    fn example_adds(test: &mut Test) {\n        test.click(\"add\");\n\n        if test.done() {\n            test.run();\n        }\n    }\n}\n";

    let body = example_body(source, "example_adds").unwrap();

    assert_eq!(
        body.as_deref(),
        Some("test.click(\"add\");\n\nif test.done() {\n    test.run();\n}\n")
    );
    assert_eq!(example_body(source, "example_other").unwrap(), None);
}
