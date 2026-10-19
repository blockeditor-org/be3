use super::*;

#[test]
fn format_views_preserves_generic_modifiers() {
    let source = r#"fn build() {
    view! { <Element @custom::decorate="one" @custom::decorate="two" @::custom::decorate="three" test_id="ordinary">
        {child} @custom::decorate="value"
        "text" @append="!"
    </Element> }
}
"#;
    let output = formatted(source);
    assert_ne!(output, source);
    assert!(output.contains("@custom::decorate=\"one\""));
    assert!(output.contains("@custom::decorate=\"two\""));
    assert!(output.contains("@::custom::decorate=\"three\""));
    assert!(output.contains("{child} @custom::decorate=\"value\""));
    assert!(output.contains("\"text\" @append=\"!\""));
    assert_eq!(formatted(&output), output);
}
