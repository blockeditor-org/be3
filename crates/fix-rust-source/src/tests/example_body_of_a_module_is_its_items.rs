use crate::example_body;

#[test]
fn example_body_of_a_module_is_its_items() {
    let source = "#[test]\nfn counts() {}\n\nmod example_counter {\n    use beui::*;\n\n    #[component]\n    fn Counter() -> NodeId {\n        view! { <Text>{\"0\"}</Text> }\n    }\n}\n";

    let body = example_body(source, "example_counter").unwrap();

    assert_eq!(
        body.as_deref(),
        Some(
            "use beui::*;\n\n#[component]\nfn Counter() -> NodeId {\n    view! { <Text>{\"0\"}</Text> }\n}\n"
        )
    );
}
