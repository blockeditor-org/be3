use super::*;

#[test]
fn format_views_ignores_nested_markup_when_shifting_rust_expressions() {
    let source = r#"fn build() {
    view! { <Element>
{if empty {
                    view! { <Fragment /> }
                } else {
                    view! { <Element>
{detail}
</Element> }
                }}
</Element> }
}
"#;
    let output = formatted(source);
    assert!(output.contains("<Element>{detail}</Element>"));
    assert_eq!(formatted(&output), output);
}
