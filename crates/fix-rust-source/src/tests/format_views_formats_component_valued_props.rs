#[test]
fn format_views_formats_component_valued_props() {
    let source = b"fn main() {\n    view! { <Button css=[<Flex /><Hover><Underline /></Hover>] content=[<Label string=\"hello\" />] /> }\n}\n";
    let formatted = crate::views::format_views(source).unwrap();
    let text = String::from_utf8(formatted.clone()).unwrap();
    assert!(text.contains("css=[\n"));
    assert!(text.contains("<Hover>\n"));
    assert!(text.contains("content=[\n"));
    assert_eq!(crate::views::format_views(&formatted).unwrap(), formatted);
}
